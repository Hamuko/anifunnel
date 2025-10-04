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
use tempfile::tempdir;

#[derive(Parser, Debug)]
struct AnifunnelArgs {
    /// IP address to bind the server to.
    #[clap(long, default_value_t = Ipv4Addr::new(0, 0, 0, 0), env = "ANIFUNNEL_ADDRESS")]
    bind_address: Ipv4Addr,

    /// Port to bind the server to.
    #[clap(long, default_value_t = 8000, env = "ANIFUNNEL_PORT")]
    port: u16,

    /// Match against all Plex library seasons. May not accurately find matches.
    #[arg(long, env = "ANIFUNNEL_MULTI_SEASON")]
    multi_season: bool,

    /// Only process updates from a specific Plex username.
    #[clap(long, env = "ANILIST_PLEX_USER")]
    plex_user: Option<String>,
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
            warn!("Unable to parse payload");
            debug!("{}", error);
            return "ERROR";
        }
    };

    if !webhook.is_actionable(state.multi_season) {
        info!("Webhook is not actionable");
        return "NO OP";
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
    let user_info_lock = state.user.read().await;
    let Some(user_info) = &(*user_info_lock) else {
        warn!("Anilist token needs to be set through the management interface to update progress");
        return "ERROR";
    };

    if let Ok(media_list_entries) =
        anilist::get_watching_list(&user_info.token, user_info.user_id).await
    {
        let mut anime_override =
            db::get_override_by_title(&mut **db, &webhook.metadata.title).await;
        let matched_media_list = match &anime_override {
            Some(o) => media_list_entries.find_id(&o.id),
            None => media_list_entries.find_match(&webhook.metadata.title),
        };
        let matched_media_list = match matched_media_list {
            Some(media_list) => media_list,
            None => {
                debug!("Could not find a match for '{}'", &webhook.metadata.title);
                return "NO OP";
            }
        };
        debug!("Processing {}", matched_media_list);

        if anime_override.is_none() {
            anime_override = db::get_override_by_id(&mut **db, matched_media_list.id).await;
        }
        let episode_offset = match &anime_override {
            Some(o) => o.get_episode_offset(),
            None => 0,
        };
        if webhook.metadata.episode_number + episode_offset == matched_media_list.progress + 1 {
            match matched_media_list.update(&user_info.token).await {
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

#[rocket::main]
async fn main() {
    let args = AnifunnelArgs::parse();

    SimpleLogger::new()
        .with_level(LevelFilter::Info)
        .env()
        .init()
        .unwrap();

    let address = args.bind_address;
    let port = args.port;
    let state = state::Global::from_args(args);

    // Because Rocket *requires* a template directory even though we are embedding our
    // single template inside the binary, we need to make a dummy directory for anifunnel.
    let dir = tempdir().unwrap();

    // Increase the string limit from default since Plex might send the thumbnail in some
    // requests and we don't want those to cause unnecessary HTTP 413 Content Too Large
    // errors (even though we don't use those requests).
    let limits = Limits::default().limit("string", 24.kibibytes());

    let db_migrations = AdHoc::try_on_ignite("Database migrations", db::run_migrations);
    let load_state_from_db = AdHoc::try_on_ignite("Load state from database", db::load_state);

    // Launch the web server.
    let figment = rocket::Config::figment()
        .merge(("limits", limits))
        .merge(("port", port))
        .merge(("address", address))
        .merge(("template_dir", dir.path()))
        .merge((
            "databases.anifunnel",
            rocket_db_pools::Config {
                url: "anifunnel.sqlite".into(),
                min_connections: Some(1),
                max_connections: 10,
                connect_timeout: 5,
                idle_timeout: Some(120),
                extensions: None,
            },
        ));
    let rocket = rocket::custom(figment)
        .manage(state)
        .mount(
            "/",
            routes![
                scrobble,
                api::user_get,
                api::user_post,
                api::anime_get,
                api::anime_override,
                management,
                management_css,
                management_js,
                management_redirect
            ],
        )
        .attach(db::AnifunnelDatabase::init())
        .attach(db_migrations)
        .attach(load_state_from_db);

    let _ = rocket.launch().await;
}

#[cfg(test)]
mod test {
    use super::*;

    use rocket::http::{ContentType, Status};
    use rocket::local::blocking::Client;
    use test_case::test_case;

    fn build_client() -> Client {
        let state = data::state::Global {
            multi_season: false,
            plex_user: None,
            token: String::from("A"),
            user: anilist::User {
                id: 1,
                name: String::from("A"),
            },
            title_overrides: RwLock::new(data::state::TitleOverrides::new()),
            episode_offsets: RwLock::new(data::state::EpisodeOverrides::new()),
        };
        let rocket = rocket::build()
            .manage(state)
            .mount("/", routes![scrobble, management_edit, management_redirect]);
        return Client::tracked(rocket).expect("valid rocket instance");
    }

    #[test_case("Mushoku Tensei S2", "1", Some(146065), Some(1) ; "title, episode offset")]
    #[test_case("Mushoku Tensei S2", "", Some(146065), None ; "title, no episode offset")]
    #[test_case("", "1", None, Some(1) ; "no title, episode_offset")]
    #[test_case("", "", None, None ; "no title, no episode offset")]
    fn management_edit_add(
        title: &str,
        episode_offset: &str,
        expected_title_override: Option<i32>,
        expected_episode_offset: Option<i32>,
    ) {
        let client = build_client();
        let request = client
            .post(uri!(management_edit(146065)))
            .header(ContentType::Form)
            .body(format!("title={}&episode_offset={}", title, episode_offset));
        let state = request.rocket().state::<data::state::Global>().unwrap();
        let response = request.dispatch();
        assert_eq!(response.status(), Status::SeeOther);
        assert_eq!(
            state
                .title_overrides
                .blocking_read()
                .get(&String::from("Mushoku Tensei S2")),
            expected_title_override
        );
        assert_eq!(
            state.episode_offsets.blocking_read().get(&146065),
            expected_episode_offset
        );
    }

    #[test_case("Mushoku Tensei S2", "", Some(146065), None ; "title, no episode offset")]
    #[test_case("", "1", None, Some(1) ; "no title, episode_offset")]
    #[test_case("", "", None, None ; "no title, no episode offset")]
    fn management_edit_remove(
        title: &str,
        episode_offset: &str,
        expected_title_override: Option<i32>,
        expected_episode_offset: Option<i32>,
    ) {
        let client = build_client();
        let request = client
            .post(uri!(management_edit(146065)))
            .header(ContentType::Form)
            .body(format!("title={}&episode_offset={}", title, episode_offset));
        let state = request.rocket().state::<data::state::Global>().unwrap();
        state
            .title_overrides
            .blocking_write()
            .set(String::from("Mushoku Tensei S2"), 146065);
        state.episode_offsets.blocking_write().set(146065, 1);
        let response = request.dispatch();
        assert_eq!(response.status(), Status::SeeOther);
        assert_eq!(
            state
                .title_overrides
                .blocking_read()
                .get(&String::from("Mushoku Tensei S2")),
            expected_title_override
        );
        assert_eq!(
            state.episode_offsets.blocking_read().get(&146065),
            expected_episode_offset
        );
    }

    #[test]
    fn management_redirect() {
        let client = build_client();
        let response = client.get(uri!(management_redirect)).dispatch();
        assert_eq!(response.status(), Status::SeeOther);
        assert_eq!(response.headers().get_one("Location"), Some("/admin"));
    }

    #[test]
    fn scrobble() {
        let client = build_client();
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
        assert_eq!(response.into_string().unwrap(), "OK")
    }

    #[test_case("yukikaze", "OK" ; "correct username")]
    #[test_case("shiranui", "NO OP" ; "incorrect username")]
    fn scrobble_username_filter(plex_user: &str, expected_response: &str) {
        let state = data::state::Global {
            multi_season: false,
            plex_user: Some(String::from(plex_user)),
            token: String::from("A"),
            user: anilist::User {
                id: 1,
                name: String::from("A"),
            },
            title_overrides: RwLock::new(data::state::TitleOverrides::new()),
            episode_offsets: RwLock::new(data::state::EpisodeOverrides::new()),
        };
        let rocket = rocket::build().manage(state).mount("/", routes![scrobble]);
        let client = Client::tracked(rocket).expect("valid rocket instance");
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
        assert_eq!(response.into_string().unwrap(), expected_response)
    }

    #[test]
    fn scrobble_non_actionable() {
        let client = build_client();
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
        let client = build_client();
        let response = client.post(uri!(scrobble)).dispatch();
        assert_eq!(response.status(), Status::UnsupportedMediaType);
    }
}
