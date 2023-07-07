#[macro_use]
extern crate rocket;

mod anilist;
mod data;
mod plex;

use clap::Parser;
use log::{debug, error, info, warn, LevelFilter};
use rocket::form::Form;
use simple_logger::SimpleLogger;
use std::net::Ipv4Addr;
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

    if let Ok(media_list_entries) = anilist::get_watching_list(&state.token, &state.user).await {
        let matched_media_list = match media_list_entries.find_match(&webhook.metadata.title) {
            Some(media_list) => media_list,
            None => {
                debug!("Could not find a match for '{}'", &webhook.metadata.title);
                return "NO OP";
            }
        };
        debug!("Processing {}", matched_media_list);
        if webhook.metadata.episode_number == matched_media_list.progress + 1 {
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
        Err(_) => {
            error!("Could not retrieve Anilist user.");
            return ();
        }
    };

    let state = data::state::Global {
        multi_season: args.multi_season,
        token: args.anilist_token,
        user: user,
        title_overrides: RwLock::new(data::state::TitleOverrides::new()),
        episode_offsets: RwLock::new(data::state::EpisodeOverrides::new()),
    };

    // Launch the web server.
    let rocket_config = rocket::Config {
        port: args.port,
        address: args.bind_address.into(),
        ..rocket::Config::debug_default()
    };
    let rocket = rocket::custom(&rocket_config)
        .manage(state)
        .mount("/", routes![scrobble]);
    let _ = rocket.launch().await;
}

#[cfg(test)]
mod test {
    use super::*;

    use rocket::http::{ContentType, Status};
    use rocket::local::blocking::Client;

    fn build_client() -> Client {
        let state = data::state::Global {
            multi_season: false,
            token: String::from("A"),
            user: anilist::User {
                id: 1,
                name: String::from("A"),
            },
            title_overrides: RwLock::new(data::state::TitleOverrides::new()),
            episode_offsets: RwLock::new(data::state::EpisodeOverrides::new()),
        };
        let rocket = rocket::build().manage(state).mount("/", routes![scrobble]);
        return Client::tracked(rocket).expect("valid rocket instance");
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
                \"parentIndex\": 1, \"index\": 2}}",
            )
            .dispatch();
        assert_eq!(response.status(), Status::Ok);
        assert_eq!(response.into_string().unwrap(), "OK")
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
                \"parentIndex\": 1, \"index\": 2}}",
            )
            .dispatch();
        assert_eq!(response.status(), Status::Ok);
        assert_eq!(response.into_string().unwrap(), "NO OP")
    }

    #[test]
    fn scrobble_empty_post() {
        let client = build_client();
        let response = client.post(uri!(scrobble)).dispatch();
        assert_eq!(response.status(), Status::NotFound);
    }

    #[test]
    fn scrobble_get() {
        let client = build_client();
        let response = client.get(uri!(scrobble)).dispatch();
        assert_eq!(response.status(), Status::NotFound);
    }
}
