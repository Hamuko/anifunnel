use crate::anilist;
use crate::api::OverrideMap;
use anilist::MediaListIdentifier;
use serde::Serialize;

#[derive(Serialize)]
pub struct Anime {
    pub id: MediaListIdentifier,
    pub media_id: i32,
    pub title: String,
    pub episode_offset: Option<i64>,
    pub title_override: Option<String>,
}

impl Anime {
    pub fn build(
        media_list_group: &anilist::MediaListGroup,
        overrides: &mut OverrideMap,
    ) -> Vec<Self> {
        let mut result: Vec<Self> = Vec::new();
        for entry in &media_list_group.entries {
            let mut title_override: Option<String> = None;
            let mut episode_offset: Option<i64> = None;
            if let Some(override_entry) = overrides.remove(&entry.id) {
                (title_override, episode_offset) = override_entry;
            }
            result.push(Self {
                id: entry.id,
                media_id: entry.media.id,
                title: entry.media.get_display_title(),
                episode_offset,
                title_override,
            });
        }
        result.sort_by(|a, b| a.title.cmp(&b.title));
        return result;
    }
}

#[derive(Serialize)]
pub struct Error {
    pub error: String,
}

#[derive(Serialize)]
pub struct User {
    pub id: i64,
    pub name: String,
    pub expiry: i64,
}
