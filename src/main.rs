#[macro_use]
extern crate rocket;

mod anilist;
mod api;
mod db;
mod forms;
mod plex;
mod responders;
mod state;
mod utils;

use crate::anilist::AnilistClientTrait;
use clap::Parser;
use log::{debug, error, info, warn, LevelFilter};
use rocket::data::{Limits, ToByteUnit};
use rocket::fairing::AdHoc;
use rocket::form::Form;
use rocket::response::content::{RawCss, RawHtml, RawJavaScript};
use rocket::response::Redirect;
use rocket_db_pools::{Connection, Database};
use simple_logger::SimpleLogger;
use std::{net::Ipv4Addr, vec};

#[derive(Parser, Debug)]
struct AnifunnelArgs {
    /// IP address to bind the server to.
    #[clap(long, default_value_t = Ipv4Addr::new(0, 0, 0, 0), env = "ANIFUNNEL_ADDRESS")]
    bind_address: Ipv4Addr,

    /// Port to bind the server to.
    #[clap(long, default_value_t = 8000, env = "ANIFUNNEL_PORT")]
    port: u16,

    /// Path to the SQLite database file.
    #[clap(long, default_value = "anifunnel.sqlite", env = "ANIFUNNEL_DATABASE")]
    database: String,

    /// Enable the deprecated multi-season matching feature. No longer has any effect.
    #[arg(long, hide = true)]
    multi_season: bool,

    /// Only process updates from a specific Plex username.
    #[clap(long, env = "ANILIST_PLEX_USER")]
    plex_user: Option<String>,

    /// Set the logging level.
    #[clap(long, default_value_t = LevelFilter::Info, env = "ANIFUNNEL_LOG_LEVEL")]
    log_level: LevelFilter,
}

#[get("/admin")]
async fn management() -> responders::StaticContent<RawHtml<&'static str>> {
    responders::StaticContent::new(RawHtml(include_str!("../dist/index.html")))
}

#[get("/assets/index.css")]
async fn management_css() -> responders::StaticContent<RawCss<&'static str>> {
    responders::StaticContent::new(RawCss(include_str!("../dist/assets/index.css")))
}

#[get("/assets/index.js")]
async fn management_js() -> responders::StaticContent<RawJavaScript<&'static str>> {
    responders::StaticContent::new(RawJavaScript(include_str!("../dist/assets/index.js")))
}

#[get("/favicon.svg")]
async fn favicon_svg() -> responders::StaticContent<(rocket::http::ContentType, &'static str)> {
    responders::StaticContent::new((
        rocket::http::ContentType::SVG,
        include_str!("../dist/favicon.svg"),
    ))
}

#[get("/")]
async fn management_redirect() -> Redirect {
    Redirect::to(uri!(management))
}

#[post("/", data = "<form>")]
async fn scrobble(
    form: Form<forms::Scrobble<'_>>,
    mut db: Connection<db::AnifunnelDatabase>,
    state: &rocket::State<state::Global>,
) -> &'static str {
    let webhook: plex::Webhook = match serde_json::from_str(form.payload) {
        Ok(data) => data,
        Err(error) => {
            warn!("Unable to parse payload: {}", error);
            return "ERROR";
        }
    };

    // Check that the webhook is something anifunnel can handle.
    match webhook.is_actionable() {
        plex::WebhookState::Actionable => log::debug!("Webhook is actionable"),
        plex::WebhookState::NoMetadata => {
            error!("Webhook was a scrobble event but has no metadata. This should not happen.");
            return "ERROR";
        }
        plex::WebhookState::NonScrobbleEvent => {
            info!("Webhook is not a scrobble event");
            return "NO OP";
        }
        plex::WebhookState::IncorrectType => {
            let metadata = webhook.metadata.unwrap();
            info!(
                "Scrobble event for {} is for a non-episode media ({:?})",
                &metadata.title, &metadata.media_type
            );
            return "NO OP";
        }
        plex::WebhookState::IncorrectSeason => {
            let metadata = webhook.metadata.unwrap();
            info!(
                "Scrobble event for {} is for a non-first season ({}). \
                Enable multi-season matching if this is unexpected.",
                &metadata.title, &metadata.season_number
            );
            return "NO OP";
        }
    }

    // Check possible Plex username restriction.
    if let Some(plex_user) = &state.plex_user {
        if plex_user == &webhook.account.name {
            debug!("Update matches Plex username restriction '{}'", plex_user);
        } else {
            info!("Ignoring update for Plex user '{}'", webhook.account.name);
            return "NO OP";
        }
    }

    // Get the user ID and token from the application state or exit with an error.
    let client_lock = state.anilist_client.read().await;
    let Some(anilist_client) = &(*client_lock) else {
        warn!("Anilist token needs to be set through the management interface to update progress");
        return "ERROR";
    };

    if let Ok(media_list_entries) = anilist_client.get_watching_list().await {
        let metadata = webhook.metadata.unwrap();

        let mut anime_override = db::get_override_by_title(&mut db, &metadata.title).await;
        let matched_media_list = match &anime_override {
            Some(o) => media_list_entries.find_id(&o.id),
            None => media_list_entries.find_match(&metadata.title),
        };
        let matched_media_list = match matched_media_list {
            Some(media_list) => media_list,
            None => {
                debug!("Could not find a match for '{}'", &metadata.title);
                return "NO OP";
            }
        };
        debug!("Processing {}", matched_media_list);

        if anime_override.is_none() {
            anime_override = db::get_override_by_id(&mut db, matched_media_list.id).await;
        }
        let episode_offset = match &anime_override {
            Some(o) => o.get_episode_offset(),
            None => 0,
        };

        let episode_number = metadata.episode_number + episode_offset;
        if episode_number == matched_media_list.progress + 1 {
            match anilist_client.update_progress(matched_media_list).await {
                Ok(true) => info!("Updated '{}' progress", matched_media_list.media.title),
                Ok(false) => error!(
                    "Failed to update progress for '{}'",
                    matched_media_list.media.title
                ),
                Err(error) => error!("{:?}", error),
            }
        }
    }
    "OK"
}

fn build_server(
    address: Ipv4Addr,
    port: u16,
    database_url: String,
    state: state::Global,
) -> rocket::Rocket<rocket::Build> {
    // Increase the string limit from default since Plex might send the thumbnail in some
    // requests and we don't want those to cause unnecessary HTTP 413 Content Too Large
    // errors (even though we don't use those requests).
    let limits = Limits::default().limit("string", 24.kibibytes());

    let db_migrations = AdHoc::try_on_ignite("Database migrations", db::run_migrations);
    let load_state_from_db = AdHoc::try_on_ignite("Load state from database", db::load_state);

    let figment = rocket::Config::figment()
        .merge(("limits", limits))
        .merge(("port", port))
        .merge(("address", address))
        .merge((
            "databases.anifunnel",
            rocket_db_pools::Config {
                url: database_url,
                min_connections: Some(1),
                max_connections: 10,
                connect_timeout: 5,
                idle_timeout: Some(120),
                extensions: None,
            },
        ));
    rocket::custom(figment)
        .manage(state)
        .mount(
            "/",
            routes![
                scrobble,
                api::user_get,
                api::user_post,
                api::user_delete,
                api::anime_get,
                api::anime_override,
                favicon_svg,
                management,
                management_css,
                management_js,
                management_redirect
            ],
        )
        .attach(db::AnifunnelDatabase::init())
        .attach(db_migrations)
        .attach(load_state_from_db)
}

#[rocket::main]
async fn main() {
    let args = AnifunnelArgs::parse();

    SimpleLogger::new()
        .with_level(args.log_level)
        .env()
        .init()
        .unwrap();

    let address = args.bind_address;
    let port = args.port;
    let database_url = args.database;
    if args.multi_season {
        log::warn!(
            "--multi-season (ANIFUNNEL_MULTI_SEASON) has been deprecated; \
            multi-season matching is always enabled"
        );
    }
    let state = state::Global::from_args(args.plex_user);

    // Launch the web server.
    let rocket = build_server(address, port, database_url, state);
    let _ = rocket.launch().await;
}

#[cfg(test)]
mod test {
    use super::*;

    use httpmock::prelude::*;
    use rocket::http::{ContentType, Status};
    use rocket::local::{asynchronous, blocking};
    use test_case::test_case;
    use tokio::sync::RwLock;

    pub fn build_client(state: state::Global) -> blocking::Client {
        let database_url = String::from(":memory:");
        let rocket = build_server(Ipv4Addr::new(127, 0, 0, 1), 0, database_url, state);
        blocking::Client::tracked(rocket).expect("valid rocket instance")
    }

    pub async fn build_async_client(state: state::Global) -> asynchronous::Client {
        let database_url = String::from(":memory:");
        let rocket = build_server(Ipv4Addr::new(127, 0, 0, 1), 0, database_url, state);
        asynchronous::Client::tracked(rocket)
            .await
            .expect("valid rocket instance")
    }

    pub fn build_state(url: String) -> state::Global {
        let client = anilist::AnilistClient {
            token: String::from("fake"),
            user_id: 100,
            url: url,
        };
        state::Global {
            plex_user: None,
            anilist_client: RwLock::new(Some(client)),
        }
    }

    fn build_media_list_fetch_mock(
        server: &MockServer,
        response: anilist::QueryResponse<anilist::data::MediaListCollectionData>,
    ) -> httpmock::Mock<'_> {
        server.mock(|when, then| {
            let request = anilist::Query {
                query: anilist::queries::MEDIALIST_QUERY,
                variables: Some(anilist::MediaListCollectionQueryVariables { user_id: 100 }),
            };
            when.method(POST).path("/").json_body_obj(&request);
            then.status(200)
                .header("content-type", "application/json")
                .json_body_obj(&response);
        })
    }

    fn generate_media_list_response(
        entries: Vec<(i64, i32, i32, &str)>,
    ) -> anilist::QueryResponse<anilist::data::MediaListCollectionData> {
        let entries: Vec<anilist::data::MediaList> = entries
            .iter()
            .map(|(id, progress, media_id, title)| {
                let title = String::from(*title);
                anilist::data::MediaList {
                    id: *id,
                    progress: *progress,
                    media: anilist::data::Media {
                        id: *media_id,
                        title: anilist::data::MediaTitle {
                            romaji: Some(title.clone()),
                            english: Some(title.clone()),
                            native: Some(title.clone()),
                            userPreferred: title,
                        },
                    },
                }
            })
            .collect();
        anilist::QueryResponse {
            data: anilist::data::MediaListCollectionData {
                MediaListCollection: anilist::data::MediaListCollection {
                    lists: vec![anilist::data::MediaListGroup { entries: entries }],
                },
            },
        }
    }

    #[test]
    fn management_redirect() {
        let client = build_client(build_state("".into()));
        let response = client.get(uri!(management_redirect)).dispatch();
        assert_eq!(response.status(), Status::SeeOther);
        assert_eq!(response.headers().get_one("Location"), Some("/admin"));
    }

    #[test_case("/admin", "text/html; charset=utf-8" ; "front-end")]
    #[test_case("/assets/index.css", "text/css; charset=utf-8" ; "css")]
    #[test_case("/assets/index.js", "text/javascript" ; "javascript")]
    #[test_case("/favicon.svg", "image/svg+xml" ; "favicon")]
    fn management_static_content(url: &str, content_type: &str) {
        let client = build_client(build_state("".into()));
        let response = client.get(url).dispatch();
        let expected_cache_control =
            format!("max-age={}", responders::STATIC_CONTENT_CACHE_SECONDS);
        assert_eq!(response.status(), Status::Ok);
        assert_eq!(
            response.headers().get_one("Content-Type"),
            Some(content_type)
        );
        assert_eq!(
            response.headers().get_one("Cache-Control"),
            Some(expected_cache_control.as_str())
        );
    }

    #[test]
    /// Anime matches webhook and scrobble is successful.
    fn scrobble() {
        let server = MockServer::start();
        println!("Server URL: {}", server.url(""));
        let state = build_state(server.url(""));

        let response = generate_media_list_response(vec![
            (123456, 3, 1234, "Ao no Orchestra"),
            (234567, 1, 2345, "Onii-chan wa Oshimai!"),
        ]);
        let fetch_mock = build_media_list_fetch_mock(&server, response);
        let update_mock = server.mock(|when, then| {
            let request = anilist::Query {
                query: anilist::queries::MEDIALIST_MUTATION,
                variables: Some(anilist::MediaListCollectionMutateVariables {
                    id: 234567,
                    progress: 2,
                }),
            };
            when.method(POST).path("/").json_body_obj(&request);
            let response = anilist::QueryResponse {
                data: anilist::data::SaveMediaListEntryData {
                    SaveMediaListEntry: anilist::data::SaveMediaListEntry { progress: 2 },
                },
            };
            then.status(200)
                .header("content-type", "application/json")
                .json_body_obj(&response);
        });

        let client = build_client(state);
        let response = client
            .post(uri!(scrobble))
            .header(ContentType::Form)
            .body(
                "payload={\"event\": \"media.scrobble\", \"Metadata\": {\
                \"type\": \"episode\", \"grandparentTitle\": \"Onii-chan wa Oshimai!\", \
                \"parentIndex\": 1, \"index\": 2}, \"Account\": {\"title\": \"yukikaze\"}}",
            )
            .dispatch();
        assert_eq!(response.status(), Status::Ok);
        assert_eq!(response.into_string().unwrap(), "OK");
        fetch_mock.assert();
        update_mock.assert();
    }

    #[rocket::async_test]
    /// Anime matches webhook, has an episode offset, and scrobble is successful.
    async fn scrobble_episode_offset() {
        let server = MockServer::start();
        println!("Server URL: {}", server.url(""));
        let state = build_state(server.url(""));

        let response = generate_media_list_response(vec![
            (123456, 3, 1234, "Ao no Orchestra"),
            (234567, 2, 2345, "Sono Bisque Doll wa Koi wo Suru Season 2"),
            (345678, 1, 3456, "Onii-chan wa Oshimai!"),
        ]);
        let fetch_mock = build_media_list_fetch_mock(&server, response);
        let update_mock = server.mock(|when, then| {
            let request = anilist::Query {
                query: anilist::queries::MEDIALIST_MUTATION,
                variables: Some(anilist::MediaListCollectionMutateVariables {
                    id: 234567,
                    progress: 3,
                }),
            };
            when.method(POST).path("/").json_body_obj(&request);
            let response = anilist::QueryResponse {
                data: anilist::data::SaveMediaListEntryData {
                    SaveMediaListEntry: anilist::data::SaveMediaListEntry { progress: 2 },
                },
            };
            then.status(200)
                .header("content-type", "application/json")
                .json_body_obj(&response);
        });

        let client = build_async_client(state).await;

        let database = db::AnifunnelDatabase::fetch(client.rocket()).unwrap();
        let mut connection = database.acquire().await.unwrap();
        db::set_override(&mut connection, 234567, None, Some(-12))
            .await
            .expect("Could not set override");

        let response = client
            .post(uri!(scrobble))
            .header(ContentType::Form)
            .body(
                "payload={\"event\": \"media.scrobble\", \"Metadata\": {\
                \"type\": \"episode\", \"grandparentTitle\": \"Sono Bisque Doll wa Koi o Suru (2025)\", \
                \"parentIndex\": 1, \"index\": 15}, \"Account\": {\"title\": \"yukikaze\"}}",
            )
            .dispatch()
            .await;
        assert_eq!(response.status(), Status::Ok);
        assert_eq!(response.into_string().await.unwrap(), "OK");
        fetch_mock.assert();
        update_mock.assert();
    }

    #[rocket::async_test]
    /// Anime is matched with title override and scrobble is successful.
    async fn scrobble_override_title() {
        let server = MockServer::start();
        println!("Server URL: {}", server.url(""));
        let state = build_state(server.url(""));

        let response = generate_media_list_response(vec![
            (345678, 2, 3456, "Ao no Orchestra Season 2"),
            (456789, 2, 4567, "Boku no Hero Academia FINAL SEASON"),
        ]);
        let fetch_mock = build_media_list_fetch_mock(&server, response);
        let update_mock = server.mock(|when, then| {
            let request = anilist::Query {
                query: anilist::queries::MEDIALIST_MUTATION,
                variables: Some(anilist::MediaListCollectionMutateVariables {
                    id: 456789,
                    progress: 3,
                }),
            };
            when.method(POST).path("/").json_body_obj(&request);
            let response = anilist::QueryResponse {
                data: anilist::data::SaveMediaListEntryData {
                    SaveMediaListEntry: anilist::data::SaveMediaListEntry { progress: 3 },
                },
            };
            then.status(200)
                .header("content-type", "application/json")
                .json_body_obj(&response);
        });

        let client = build_async_client(state).await;

        let database = db::AnifunnelDatabase::fetch(client.rocket()).unwrap();
        let mut connection = database.acquire().await.unwrap();
        db::set_override(
            &mut connection,
            456789,
            Some("Boku no Hero Academia (2025)"),
            None,
        )
        .await
        .expect("Could not set override");

        let response = client
            .post(uri!(scrobble))
            .header(ContentType::Form)
            .body(
                "payload={\"event\": \"media.scrobble\", \"Metadata\": {\
                \"type\": \"episode\", \"grandparentTitle\": \"Boku no Hero Academia (2025)\", \
                \"parentIndex\": 1, \"index\": 3}, \"Account\": {\"title\": \"yukikaze\"}}",
            )
            .dispatch()
            .await;
        assert_eq!(response.status(), Status::Ok);
        assert_eq!(response.into_string().await.unwrap(), "OK");
        fetch_mock.assert();
        update_mock.assert();
    }

    #[rocket::async_test]
    /// Anime is matched with title override, has an episode offset, and scrobble is successful.
    async fn scrobble_override_title_episode_offset() {
        let server = MockServer::start();
        println!("Server URL: {}", server.url(""));
        let state = build_state(server.url(""));

        let response = generate_media_list_response(vec![
            (345678, 2, 3456, "Ao no Orchestra Season 2"),
            (456789, 2, 4567, "Boku no Hero Academia FINAL SEASON"),
        ]);
        let fetch_mock = build_media_list_fetch_mock(&server, response);
        let update_mock = server.mock(|when, then| {
            let request = anilist::Query {
                query: anilist::queries::MEDIALIST_MUTATION,
                variables: Some(anilist::MediaListCollectionMutateVariables {
                    id: 456789,
                    progress: 3,
                }),
            };
            when.method(POST).path("/").json_body_obj(&request);
            let response = anilist::QueryResponse {
                data: anilist::data::SaveMediaListEntryData {
                    SaveMediaListEntry: anilist::data::SaveMediaListEntry { progress: 3 },
                },
            };
            then.status(200)
                .header("content-type", "application/json")
                .json_body_obj(&response);
        });

        let client = build_async_client(state).await;

        let database = db::AnifunnelDatabase::fetch(client.rocket()).unwrap();
        let mut connection = database.acquire().await.unwrap();
        db::set_override(
            &mut connection,
            456789,
            Some("Boku no Hero Academia (2025)"),
            Some(-159),
        )
        .await
        .expect("Could not set override");

        let response = client
            .post(uri!(scrobble))
            .header(ContentType::Form)
            .body(
                "payload={\"event\": \"media.scrobble\", \"Metadata\": {\
                \"type\": \"episode\", \"grandparentTitle\": \"Boku no Hero Academia (2025)\", \
                \"parentIndex\": 1, \"index\": 162}, \"Account\": {\"title\": \"yukikaze\"}}",
            )
            .dispatch()
            .await;
        assert_eq!(response.status(), Status::Ok);
        assert_eq!(response.into_string().await.unwrap(), "OK");
        fetch_mock.assert();
        update_mock.assert();
    }

    #[test]
    /// Anime matches webhook but progress doesn't; scrobble performs no update.
    fn scrobble_incorrect_progress() {
        let server = MockServer::start();
        println!("Server URL: {}", server.url(""));
        let state = build_state(server.url(""));

        let response = generate_media_list_response(vec![
            (123456, 3, 1234, "Ao no Orchestra"),
            (234567, 1, 2345, "Onii-chan wa Oshimai!"),
        ]);
        let fetch_mock = build_media_list_fetch_mock(&server, response);
        let update_mock = server.mock(|when, _| {
            let request = anilist::Query {
                query: anilist::queries::MEDIALIST_MUTATION,
                variables: Some(anilist::MediaListCollectionMutateVariables {
                    id: 234567,
                    progress: 2,
                }),
            };
            when.method(POST).path("/").json_body_obj(&request);
        });

        let client = build_client(state);
        let response = client
            .post(uri!(scrobble))
            .header(ContentType::Form)
            .body(
                "payload={\"event\": \"media.scrobble\", \"Metadata\": {\
                \"type\": \"episode\", \"grandparentTitle\": \"Onii-chan wa Oshimai!\", \
                \"parentIndex\": 1, \"index\": 6}, \"Account\": {\"title\": \"yukikaze\"}}",
            )
            .dispatch();
        assert_eq!(response.status(), Status::Ok);
        assert_eq!(response.into_string().unwrap(), "OK");
        fetch_mock.assert();
        update_mock.assert_calls(0); // Update is never called.
    }

    #[test]
    fn scrobble_no_client() {
        let state = state::Global {
            plex_user: None,
            anilist_client: RwLock::new(None),
        };
        let client = build_client(state);
        let response = client
            .post(uri!(scrobble))
            .header(ContentType::Form)
            .body(
                "payload={\"event\": \"media.scrobble\", \"Metadata\": {\
                \"type\": \"episode\", \"grandparentTitle\": \"Onii-chan wa Oshimai!\", \
                \"parentIndex\": 1, \"index\": 2}, \"Account\": {\"title\": \"yukikaze\"}}",
            )
            .dispatch();
        assert_eq!(response.status(), Status::Ok);
        assert_eq!(response.into_string().unwrap(), "ERROR")
    }

    #[test]
    /// No anime matches webhook and scrobble is a NO-OP.
    fn scrobble_no_match() {
        let server = MockServer::start();
        let state = build_state(server.url(""));

        let response = generate_media_list_response(vec![(123456, 3, 1234, "Ao no Orchestra")]);
        let fetch_mock = build_media_list_fetch_mock(&server, response);

        let client = build_client(state);
        let response = client
            .post(uri!(scrobble))
            .header(ContentType::Form)
            .body(
                "payload={\"event\": \"media.scrobble\", \"Metadata\": {\
                \"type\": \"episode\", \"grandparentTitle\": \"Onii-chan wa Oshimai!\", \
                \"parentIndex\": 1, \"index\": 2}, \"Account\": {\"title\": \"yukikaze\"}}",
            )
            .dispatch();
        assert_eq!(response.status(), Status::Ok);
        assert_eq!(response.into_string().unwrap(), "NO OP");
        fetch_mock.assert();
    }

    #[test_case("yukikaze", "OK", 1 ; "correct username")]
    #[test_case("shiranui", "NO OP", 0 ; "incorrect username")]
    fn scrobble_username_filter(plex_user: &str, expected_response: &str, call_count: usize) {
        let server = MockServer::start();
        let state = state::Global {
            plex_user: Some(String::from(plex_user)),
            anilist_client: RwLock::new(Some(anilist::AnilistClient {
                token: String::from("fake"),
                user_id: 100,
                url: server.url("/"),
            })),
        };

        let response = generate_media_list_response(vec![
            (123456, 3, 1234, "Ao no Orchestra"),
            (234567, 1, 2345, "Onii-chan wa Oshimai!"),
        ]);
        let fetch_mock = build_media_list_fetch_mock(&server, response);
        let update_mock = server.mock(|when, _| {
            let request = anilist::Query {
                query: anilist::queries::MEDIALIST_MUTATION,
                variables: Some(anilist::MediaListCollectionMutateVariables {
                    id: 234567,
                    progress: 2,
                }),
            };
            when.method(POST).path("/").json_body_obj(&request);
        });

        let client = build_client(state);
        let response = client
            .post(uri!(scrobble))
            .header(ContentType::Form)
            .body(
                "payload={\"event\": \"media.scrobble\", \"Metadata\": {\
                \"type\": \"episode\", \"grandparentTitle\": \"Onii-chan wa Oshimai!\", \
                \"parentIndex\": 1, \"index\": 2}, \"Account\": {\"title\": \"yukikaze\"}}",
            )
            .dispatch();
        assert_eq!(response.status(), Status::Ok);
        assert_eq!(response.into_string().unwrap(), expected_response);
        fetch_mock.assert_calls(call_count);
        update_mock.assert_calls(call_count);
    }

    #[test]
    fn scrobble_non_scrobble_event() {
        let client = build_client(build_state("".into()));
        let response = client
            .post(uri!(scrobble))
            .header(ContentType::Form)
            .body(
                "payload={\"event\": \"library.new\", \"Metadata\": {\
                \"type\": \"episode\", \"grandparentTitle\": \"Onii-chan wa Oshimai!\", \
                \"parentIndex\": 1, \"index\": 2}, \"Account\": {\"title\": \"yukikaze\"}}",
            )
            .dispatch();
        assert_eq!(response.status(), Status::Ok);
        assert_eq!(response.into_string().unwrap(), "NO OP")
    }

    #[test]
    fn scrobble_empty_post() {
        let client = build_client(build_state("".into()));
        let response = client.post(uri!(scrobble)).dispatch();
        assert_eq!(response.status(), Status::UnsupportedMediaType);
    }
}
