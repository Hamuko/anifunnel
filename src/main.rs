#[macro_use]
extern crate rocket;

mod anilist;
mod plex;

use clap::Parser;
use log::{debug, error, info, warn, LevelFilter};
use rocket::form::Form;
use simple_logger::SimpleLogger;
use std::net::Ipv4Addr;

#[derive(Parser, Debug)]
struct PlexAnihookArgs {
    /// Anilist API token.
    #[clap(env = "ANILIST_TOKEN")]
    anilist_token: String,

    /// IP address to bind the server to.
    #[clap(long, default_value_t = Ipv4Addr::new(0, 0, 0, 0), env = "ANIHOOK_ADDRESS")]
    bind_address: Ipv4Addr,

    /// Port to bind the server to.
    #[clap(long, default_value_t = 8000, env = "ANIHOOK_PORT")]
    port: u16,
}

struct PlexAnihookState {
    token: String,
    user: anilist::User,
}

#[derive(Debug, FromForm)]
struct ScrobbleForm<'r> {
    payload: &'r str,
}

#[post("/", data = "<form>")]
async fn scrobble(
    form: Form<ScrobbleForm<'_>>,
    state: &rocket::State<PlexAnihookState>,
) -> &'static str {
    let webhook: plex::Webhook = match serde_json::from_str(form.payload) {
        Ok(data) => data,
        Err(error) => {
            warn!("Unable to parse payload");
            debug!("{}", error);
            return "ERROR";
        }
    };

    if !webhook.is_actionable() {
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
    let args = PlexAnihookArgs::parse();

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

    let state = PlexAnihookState {
        token: args.anilist_token,
        user: user,
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
