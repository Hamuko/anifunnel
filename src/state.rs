use crate::anilist::AnilistClient;
use tokio::sync::RwLock;

#[derive(Debug)]
/// Global anifunnel application state.
pub struct Global {
    pub plex_user: Option<String>,
    pub anilist_client: RwLock<Option<AnilistClient>>,
}

impl Global {
    pub fn from_args(plex_user: Option<String>) -> Self {
        Self {
            plex_user,
            anilist_client: RwLock::new(None),
        }
    }
}
