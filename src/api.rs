mod requests;
mod responders;
mod responses;

use crate::{anilist, db, state, utils};
use rocket::futures::future::TryFutureExt;
use rocket::response::status;
use rocket::serde::json::Json;
use rocket_db_pools::{sqlx, Connection};
use std::collections::HashMap;

type OverrideMap = HashMap<anilist::MediaListIdentifier, (Option<String>, Option<i64>)>;

#[get("/api/anime")]
pub async fn anime_get(
    mut db: Connection<db::AnifunnelDatabase>,
    state: &rocket::State<state::Global>,
) -> Result<responders::APIResponse<Vec<responses::Anime>>, responders::ErrorResponder> {
    // Get the user ID and token from the application state or exit with an error.
    let user_info_lock = state.user.read().await;
    let Some(user_info) = &(*user_info_lock) else {
        warn!("Anilist token needs to be set through the management interface get watching list");
        return Err(responders::ErrorResponder::with_message(
            "No Anilist token found.".into(),
        ));
    };

    let mut overrides = sqlx::query!("SELECT id, title, episode_offset FROM overrides")
        .fetch_all(&mut **db)
        .map_ok(|rows| {
            rows.iter()
                .map(|row| (row.id, (row.title.clone(), row.episode_offset)))
                .collect::<OverrideMap>()
        })
        .await
        .unwrap_or_else(|e| {
            warn!("Failed to fetch overrides: {}", e);
            OverrideMap::with_capacity(0)
        });

    match anilist::get_watching_list(&user_info.token, user_info.user_id).await {
        Ok(media_list_group) => Ok(responders::APIResponse::new(responses::Anime::build(
            &media_list_group,
            &mut overrides,
        ))),
        Err(e) => Err(responders::ErrorResponder::with_message(format!(
            "Failed to fetch anime list: {}",
            e
        ))),
    }
}

#[post("/api/anime/<id>/edit", format = "json", data = "<data>")]
/// Set an anime override.
pub async fn anime_override(
    id: i64,
    mut db: Connection<db::AnifunnelDatabase>,
    data: Json<requests::Override<'_>>,
) -> Result<status::Accepted<()>, responders::ErrorResponder> {
    let title = match data.title {
        Some("") => None,
        title => title,
    };
    let episode_offset = match data.episode_offset {
        Some(0) => None,
        episode_offset => episode_offset,
    };
    let result = db::set_override(&mut **db, id, title, episode_offset).await;
    match result {
        Ok(result) => {
            log::info!(
                "Anime override saved successfully. Rows affected: {}",
                result.rows_affected()
            );
            Ok(status::Accepted(()))
        }
        Err(e) => Err(responders::ErrorResponder::with_message(format!(
            "Failed to save anime override: {}",
            e
        ))),
    }
}

#[get("/api/user")]
/// Return basic user information.
pub async fn user_get(
    mut db: Connection<db::AnifunnelDatabase>,
) -> responders::APIResponse<Option<responses::User>> {
    let result = sqlx::query!(
        "SELECT user_id, username, expiry FROM authentication WHERE expiry > unixepoch() LIMIT 1"
    )
    .fetch_optional(&mut **db)
    .await;

    let user = match result {
        Ok(Some(row)) => {
            let user = responses::User {
                id: row.user_id,
                name: row.username,
                expiry: row.expiry.and_utc().timestamp(),
            };
            debug!("Loaded user {} from database", user.id);
            Some(user)
        }
        Ok(None) => {
            debug!("No active user found");
            None
        }
        Err(err) => {
            error!("Failed to fetch user: {}", err);
            None
        }
    };

    responders::APIResponse::new(user)
}

#[post("/api/user", format = "json", data = "<data>")]
/// Authenticate the user with Anilist and store the token in the database.
pub async fn user_post(
    mut db: Connection<db::AnifunnelDatabase>,
    data: Json<requests::Authentication<'_>>,
    state: &rocket::State<state::Global>,
) -> Result<status::Accepted<()>, responders::ErrorResponder> {
    let expiry_timestamp = match utils::get_token_expiry(&data.token) {
        Ok(expiry) => expiry,
        Err(err) => {
            return Err(responders::ErrorResponder::with_message(format!(
                "Failed to parse Anilist token: {}. Ensure that you have a valid Anilist API token.",
                err
            )));
        }
    };
    let user = match anilist::get_user(data.token).await {
        Ok(user) => user,
        Err(anilist::AnilistError::InvalidToken) => {
            return Err(responders::ErrorResponder::with_message(
                "Invalid token. Ensure that you have a valid token. \
                    Tokens are valid for up to one year from authorization."
                    .into(),
            ));
        }
        Err(_) => {
            return Err(responders::ErrorResponder::with_message(
                "Could not retrieve Anilist user.".into(),
            ));
        }
    };
    let results = sqlx::query(
        "INSERT INTO authentication (token, user_id, username, expiry) VALUES (?, ?, ?, ?) RETURNING id"
    ).bind(data.token).bind(user.id).bind(user.name).bind(expiry_timestamp).execute(&mut **db).await;

    match results {
        Ok(result) if result.rows_affected() == 1 => {
            info!("Authentication data saved to the database");
        }
        Ok(result) => {
            warn!(
                "Error while inserting authentication data in the database. Rows affected: {}",
                result.rows_affected()
            );
            return Err(responders::ErrorResponder::with_message(
                "Failed to save authentication data in the database".into(),
            ));
        }
        Err(err) => {
            error!("Error while trying to INSERT token: {}", err);
            return Err(responders::ErrorResponder::with_message(
                "Error while saving authentication data".into(),
            ));
        }
    }

    // Update the state with the new token and user ID.
    let user_info = state::UserInfo {
        token: data.token.to_owned(),
        user_id: user.id,
    };
    let anifunnel_state: &state::Global = state.inner();
    {
        let mut writer = anifunnel_state.user.write().await;
        *writer = Some(user_info);
        info!("Application state updated with the new token");
    }
    Ok(status::Accepted(()))
}
