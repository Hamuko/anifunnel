use crate::state;
use rocket::fairing;
use rocket::Build;
use rocket::Rocket;
use rocket_db_pools::{sqlx, Database};
use sqlx::sqlite::SqliteQueryResult;
use sqlx::SqliteConnection;

#[derive(Database)]
#[database("anifunnel")]
pub struct AnifunnelDatabase(sqlx::SqlitePool);

#[derive(sqlx::FromRow)]
pub struct AnimeOverride {
    pub id: i64,
    pub episode_offset: Option<i32>,
}

impl AnimeOverride {
    pub fn get_episode_offset(&self) -> i32 {
        self.episode_offset.unwrap_or(0)
    }
}

/// Clean up expired tokens on start-up.
async fn remove_expired_tokens(db: &AnifunnelDatabase) {
    log::info!("Removing expired Anilist tokens...");
    let result = sqlx::query!("DELETE FROM authentication WHERE expiry <= unixepoch()")
        .execute(&**db)
        .await;
    match result {
        Ok(result) => match result.rows_affected() {
            0 => log::info!("No expired tokens found"),
            _ => log::info!("Removed {} expired tokens", result.rows_affected()),
        },
        Err(e) => log::error!("Failed to remove expired tokens: {}", e),
    }
}

/// Load user ID and token from database to application state.
async fn load_user_info(db: &AnifunnelDatabase, state: &state::Global) {
    log::info!("Loading user info from database...");
    let result = sqlx::query!(
        "SELECT token, user_id, username FROM authentication WHERE expiry > unixepoch() LIMIT 1"
    )
    .fetch_optional(&**db)
    .await;
    match result {
        Ok(Some(row)) => {
            let user_info = state::UserInfo {
                token: row.token,
                user_id: row.user_id,
            };
            let mut state_lock = state.user.write().await;
            *state_lock = Some(user_info);
            log::info!("Loaded user info for {} ({})", row.username, row.user_id);
        }
        Ok(None) => log::warn!(
            "No valid user info found. Make sure to authenticate the application before usage."
        ),
        Err(e) => log::error!("Failed to load user info from database: {}", e),
    }
}

/// Perform database cleanup operations and load application state from database.
pub async fn load_state(rocket: Rocket<Build>) -> fairing::Result {
    match AnifunnelDatabase::fetch(&rocket) {
        Some(db) => {
            remove_expired_tokens(&db).await;
            match rocket.state::<state::Global>() {
                Some(state) => load_user_info(&db, &state).await,
                None => log::error!(
                    "Failed to load application state. Application most likely does not work."
                ),
            }
            Ok(rocket)
        }
        None => Err(rocket),
    }
}

/// Run database migrations found in the /migrations directory.
pub async fn run_migrations(rocket: Rocket<Build>) -> fairing::Result {
    match AnifunnelDatabase::fetch(&rocket) {
        Some(db) => match sqlx::migrate!("./migrations").run(&**db).await {
            Ok(_) => {
                log::info!("Database migrated");
                Ok(rocket)
            }
            Err(e) => {
                error!("Failed to run database migrations: {}", e);
                Err(rocket)
            }
        },
        None => Err(rocket),
    }
}

pub async fn get_override_by_id(db: &mut SqliteConnection, id: i64) -> Option<AnimeOverride> {
    let result = sqlx::query_as::<_, AnimeOverride>("SELECT * FROM overrides WHERE id = ?")
        .bind(id)
        .fetch_optional(db)
        .await;
    match result {
        Ok(o) => o,
        Err(e) => {
            log::error!("Error retrieving override from database: {}", e);
            None
        }
    }
}

pub async fn get_override_by_title(
    db: &mut SqliteConnection,
    title: &str,
) -> Option<AnimeOverride> {
    let result = sqlx::query_as::<_, AnimeOverride>("SELECT * FROM overrides WHERE title = ?")
        .bind(title)
        .fetch_optional(db)
        .await;
    match result {
        Ok(o) => o,
        Err(e) => {
            log::error!("Error retrieving override from database: {}", e);
            None
        }
    }
}

/// Store an anime override in the database. Will replace existing override, either by anime ID or title.
pub async fn set_override(
    db: &mut SqliteConnection,
    id: i64,
    title: Option<&str>,
    episode_offset: Option<i64>,
) -> Result<SqliteQueryResult, sqlx::Error> {
    let query = match (title, episode_offset) {
        (None, None) => sqlx::query("DELETE FROM overrides WHERE id = ?").bind(id),
        _ => sqlx::query(
            "INSERT OR REPLACE INTO overrides (id, title, episode_offset) VALUES (?, ?, ?)",
        )
        .bind(id)
        .bind(title)
        .bind(episode_offset),
    };
    query.execute(db).await
}
