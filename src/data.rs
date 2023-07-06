pub mod forms {
    #[derive(Debug, FromForm)]
    pub struct Scrobble<'r> {
        pub payload: &'r str,
    }
}

pub mod state {
    use crate::anilist;

    #[derive(Debug)]
    /// Global anifunnel application state.
    pub struct Global {
        pub multi_season: bool,
        pub token: String,
        pub user: anilist::User,
    }
}
