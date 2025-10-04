use crate::anilist::UserID;
use crate::AnifunnelArgs;
use tokio::sync::RwLock;

#[derive(Debug)]
/// Global anifunnel application state.
pub struct Global {
    pub multi_season: bool,
    pub user: RwLock<Option<UserInfo>>,
    pub plex_user: Option<String>,
}

impl Global {
    pub fn from_args(args: AnifunnelArgs) -> Self {
        Self {
            multi_season: args.multi_season,
            plex_user: args.plex_user,
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
