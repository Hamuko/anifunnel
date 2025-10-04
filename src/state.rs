use crate::anilist::UserID;
use tokio::sync::RwLock;

#[derive(Debug)]
/// Global anifunnel application state.
pub struct Global {
    pub multi_season: bool,
    pub user: RwLock<Option<UserInfo>>,
    pub plex_user: Option<String>,
}

impl Global {
    pub fn from_args(multi_season: bool, plex_user: Option<String>) -> Self {
        Self {
            multi_season,
            plex_user,
            user: RwLock::new(None),
        }
    }
}

#[derive(Debug, Clone)]
/// User information needed to interact with the AniList API.
pub struct UserInfo {
    pub token: String,
    pub user_id: UserID,
}
