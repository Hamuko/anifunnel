#[macro_use]
extern crate rocket;

mod anilist;
mod data;
mod plex;

use clap::Parser;
use data::context::Anime;
use log::{debug, error, info, warn, LevelFilter};
use rocket::data::{Limits, ToByteUnit};
use rocket::form::Form;
use rocket::response::Redirect;
use rocket_dyn_templates::{context, Template};
use simple_logger::SimpleLogger;
use std::{net::Ipv4Addr, vec};
use tempfile::tempdir;
use tokio::sync::RwLock;

#[derive(Parser, Debug)]
struct AnifunnelArgs {
    /// Anilist API token.
    #[clap(env = "ANILIST_TOKEN")]
    anilist_token: String,

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
async fn management(state: &rocket::State<data::state::Global>) -> Template {
    let title_overrides = state.title_overrides.read().await;
    let episode_offsets = state.episode_offsets.read().await;
    let watching_list = match anilist::get_watching_list(&state.token, &state.user).await {
        Ok(media_list_group) => Anime::build(&media_list_group, &title_overrides, &episode_offsets),
        Err(_) => vec![],
    };
    Template::render(
        "management.html",
        context! {
            watching_list: watching_list,
        },
    )
}

#[post("/admin/edit/<id>", data = "<form>")]
async fn management_edit(
    id: i32,
    form: Form<data::forms::AnimeOverride<'_>>,
    state: &rocket::State<data::state::Global>,
) -> Redirect {
    let anifunnel_state: &data::state::Global = state.inner();
    let mut title_overrides = anifunnel_state.title_overrides.write().await;
    let mut episode_offsets = anifunnel_state.episode_offsets.write().await;

    if let Some(title) = form.get_title() {
        debug!("Setting title override for ID {} to \"{}\"", id, title);
        title_overrides.set(title.to_string(), id);
    } else {
        debug!("Removing possible title override for ID {}", id);
        title_overrides.remove_value(&id);
    }

    if let Some(episode_offset) = form.get_episode_offset() {
        debug!("Setting episode offset for ID {} to {}", id, episode_offset);
        episode_offsets.set(id, episode_offset);
    } else {
        debug!("Removing possible episode offset for ID {}", id);
        episode_offsets.remove(&id);
    }
    Redirect::to(uri!(management))
}

#[get("/")]
async fn management_redirect() -> Redirect {
    Redirect::to(uri!(management))
}

#[post("/", data = "<form>")]
async fn scrobble(
    form: Form<data::forms::Scrobble<'_>>,
    state: &rocket::State<data::state::Global>,
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

    if let Ok(media_list_entries) = anilist::get_watching_list(&state.token, &state.user).await {
        let title_overrides = state.title_overrides.read().await;
        let matched_media_list = match title_overrides.get(&webhook.metadata.title) {
            Some(id) => media_list_entries.find_id(&id),
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
        let episode_offsets = state.episode_offsets.read().await;
        let episode_offset = episode_offsets.get(&matched_media_list.id).unwrap_or(0);
        if webhook.metadata.episode_number + episode_offset == matched_media_list.progress + 1 {
            match matched_media_list.update(&state.token).await {
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

    let user = match anilist::get_user(&args.anilist_token).await {
        Ok(user) => user,
        Err(anilist::AnilistError::InvalidToken) => {
            error!(
                "Invalid token. Ensure that you have a valid token. \
                Tokens are valid for up to one year from authorization."
            );
            return ();
        }
        Err(_) => {
            error!("Could not retrieve Anilist user.");
            return ();
        }
    };

    let state = data::state::Global {
        multi_season: args.multi_season,
        plex_user: args.plex_user,
        token: args.anilist_token,
        user: user,
        title_overrides: RwLock::new(data::state::TitleOverrides::new()),
        episode_offsets: RwLock::new(data::state::EpisodeOverrides::new()),
    };

    // Because Rocket *requires* a template directory even though we are embedding our
    // single template inside the binary, we need to make a dummy directory for anifunnel.
    let dir = tempdir().unwrap();

    // Increase the string limit from default since Plex might send the thumbnail in some
    // requests and we don't want those to cause unnecessary HTTP 413 Content Too Large
    // errors (even though we don't use those requests).
    let limits = Limits::default().limit("string", 24.kibibytes());

    // Launch the web server.
    let figment = rocket::Config::figment()
        .merge(("limits", limits))
        .merge(("port", args.port))
        .merge(("address", args.bind_address))
        .merge(("template_dir", dir.path()));
    let rocket = rocket::custom(figment)
        .manage(state)
        .mount(
            "/",
            routes![scrobble, management, management_edit, management_redirect],
        )
        .attach(Template::custom(|engines| {
            engines
                .tera
                .add_raw_template(
                    "management.html",
                    include_str!("../templates/management.html.tera"),
                )
                .expect("Could not load management template");
        }));
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
