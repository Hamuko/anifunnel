use crate::anilist;
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
async fn remove_expired_tokens(db: &mut SqliteConnection) {
    log::info!("Removing expired Anilist tokens...");
    let result = sqlx::query!("DELETE FROM authentication WHERE expiry <= unixepoch()")
        .execute(db)
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
async fn load_user_info(db: &mut SqliteConnection, state: &state::Global) {
    log::info!("Loading user info from database...");
    let result = sqlx::query!(
        "SELECT token, user_id, username FROM authentication WHERE expiry > unixepoch() LIMIT 1"
    )
    .fetch_optional(db)
    .await;
    match result {
        Ok(Some(row)) => {
            let client = anilist::AnilistClient::new(row.token, row.user_id);
            let mut client_lock = state.anilist_client.write().await;
            *client_lock = Some(client);
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
            let mut connection = db
                .acquire()
                .await
                .expect("Failed to acquire database connection");
            remove_expired_tokens(&mut connection).await;
            match rocket.state::<state::Global>() {
                Some(state) => load_user_info(&mut connection, state).await,
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

#[cfg(test)]
mod tests {
    use super::*;

    use chrono::{DateTime, TimeDelta, Utc};
    use sqlx::sqlite::SqliteRow;
    use sqlx::Row;
    use sqlx::{self, SqlitePool};
    use test_case::test_case;

    type OverrideTuple = Vec<(i64, Option<String>, Option<i32>)>;

    async fn get_all_overrides(db: &mut SqliteConnection) -> sqlx::Result<OverrideTuple> {
        sqlx::query("SELECT id, title, episode_offset FROM overrides")
            .try_map(|row: SqliteRow| {
                Ok((row.get("id"), row.get("title"), row.get("episode_offset")))
            })
            .fetch_all(db)
            .await
    }

    #[test_case(None, 0 ; "none")]
    #[test_case(None, 0 ; "zero")]
    #[test_case(Some(1), 1 ; "positive")]
    #[test_case(Some(-12), -12 ; "negative")]
    fn anime_override_get_episode_offset(episode_offset: Option<i32>, expected: i32) {
        let anime_override = AnimeOverride {
            id: 1234,
            episode_offset,
        };
        assert_eq!(anime_override.get_episode_offset(), expected);
    }

    #[sqlx::test]
    async fn expired_token_removal(pool: SqlitePool) -> sqlx::Result<()> {
        let mut conn = pool.acquire().await?;
        let insert_query = "INSERT INTO authentication (token, user_id, username, expiry) \
            VALUES (?, 123, 'test', ?)";
        let now: DateTime<Utc> = Utc::now();

        let past = now - TimeDelta::days(100);
        sqlx::query(insert_query)
            .bind("old_token")
            .bind(past.timestamp())
            .execute(&mut *conn)
            .await?;

        let future = now + TimeDelta::days(250);
        sqlx::query(insert_query)
            .bind("new_token")
            .bind(future.timestamp())
            .execute(&mut *conn)
            .await?;

        remove_expired_tokens(&mut *conn).await;

        let results = sqlx::query("SELECT token FROM authentication")
            .try_map(|row: SqliteRow| row.try_get::<String, _>(0))
            .fetch_all(&mut *conn)
            .await?;
        assert_eq!(results, ["new_token"]);

        Ok(())
    }

    #[sqlx::test]
    async fn load_user_info_active(pool: SqlitePool) -> sqlx::Result<()> {
        let mut conn = pool.acquire().await?;

        let expiry = Utc::now() + TimeDelta::days(2);
        sqlx::query(
            "INSERT INTO authentication (token, user_id, username, expiry) \
            VALUES ('mytoken', 123, 'myname', ?)",
        )
        .bind(expiry.timestamp())
        .execute(&mut *conn)
        .await?;

        let state = state::Global::from_args(false, None);

        load_user_info(&mut *conn, &state).await;

        let client = state.anilist_client.read().await;
        assert_eq!(
            *client,
            Some(anilist::AnilistClient::new(String::from("mytoken"), 123))
        );

        Ok(())
    }

    #[sqlx::test]
    async fn load_user_info_empty(pool: SqlitePool) -> sqlx::Result<()> {
        let mut conn = pool.acquire().await?;

        let state = state::Global::from_args(false, None);

        load_user_info(&mut *conn, &state).await;

        let user_info = state.anilist_client.read().await;
        assert_eq!(*user_info, None);

        Ok(())
    }

    #[sqlx::test]
    async fn load_user_info_expired(pool: SqlitePool) -> sqlx::Result<()> {
        let mut conn = pool.acquire().await?;

        let expiry = Utc::now() - TimeDelta::days(2);
        sqlx::query(
            "INSERT INTO authentication (token, user_id, username, expiry) \
            VALUES ('mytoken', 123, 'myname', ?)",
        )
        .bind(expiry.timestamp())
        .execute(&mut *conn)
        .await?;

        let state = state::Global::from_args(false, None);

        load_user_info(&mut *conn, &state).await;

        let client = state.anilist_client.read().await;
        assert_eq!(*client, None);

        Ok(())
    }

    #[sqlx::test]
    async fn set_override_blank(pool: SqlitePool) -> sqlx::Result<()> {
        let mut conn = pool.acquire().await?;

        set_override(&mut *conn, 1234, None, None).await?;

        let results = get_all_overrides(&mut *conn).await?;
        assert_eq!(results, []);

        Ok(())
    }

    #[sqlx::test]
    async fn set_override_blank_remove(pool: SqlitePool) -> sqlx::Result<()> {
        let mut conn = pool.acquire().await?;

        sqlx::query("INSERT INTO overrides (id, title) VALUES (?, ?)")
            .bind(1234)
            .bind("Spy x Family (2025)")
            .execute(&mut *conn)
            .await?;

        set_override(&mut *conn, 1234, None, None).await?;

        let results = get_all_overrides(&mut *conn).await?;
        assert_eq!(results, []);

        Ok(())
    }

    #[sqlx::test]
    async fn set_override_new(pool: SqlitePool) -> sqlx::Result<()> {
        let mut conn = pool.acquire().await?;

        sqlx::query("INSERT INTO overrides (id, title) VALUES (?, ?)")
            .bind(1234)
            .bind("Spy x Family (2025)")
            .execute(&mut *conn)
            .await?;

        set_override(
            &mut *conn,
            2345,
            Some("Boku no Hero Academia (2025)"),
            Some(-123),
        )
        .await?;

        let results = get_all_overrides(&mut *conn).await?;
        assert_eq!(
            results,
            [
                (1234, Some(String::from("Spy x Family (2025)")), None),
                (
                    2345,
                    Some(String::from("Boku no Hero Academia (2025)")),
                    Some(-123)
                )
            ]
        );

        Ok(())
    }

    #[sqlx::test]
    /// Setting an override with an existing media ID removes the existing override.
    async fn set_override_overwrite_by_id(pool: SqlitePool) -> sqlx::Result<()> {
        let mut conn = pool.acquire().await?;

        sqlx::query("INSERT INTO overrides (id, title) VALUES (?, ?)")
            .bind(1234)
            .bind("SPY×FAMILY Season 3")
            .execute(&mut *conn)
            .await?;

        set_override(&mut *conn, 1234, Some("Spy x Family (2025)"), None).await?;

        let results = get_all_overrides(&mut *conn).await?;
        assert_eq!(
            results,
            [(1234, Some(String::from("Spy x Family (2025)")), None)]
        );

        Ok(())
    }

    #[sqlx::test]
    /// Setting an override with an existing title removes the existing override.
    async fn set_override_overwrite_by_title(pool: SqlitePool) -> sqlx::Result<()> {
        let mut conn = pool.acquire().await?;

        sqlx::query("INSERT INTO overrides (id, title) VALUES (?, ?)")
            .bind(9876)
            .bind("Spy x Family (2025)")
            .execute(&mut *conn)
            .await?;

        set_override(&mut *conn, 1234, Some("Spy x Family (2025)"), None).await?;

        let results = get_all_overrides(&mut *conn).await?;
        assert_eq!(
            results,
            [(1234, Some(String::from("Spy x Family (2025)")), None)]
        );

        Ok(())
    }
}
