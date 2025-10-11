use crate::anilist;
use crate::api::OverrideMap;
use anilist::MediaListIdentifier;
use serde::Serialize;

#[derive(Debug, PartialEq, Serialize)]
pub struct Anime {
    pub id: MediaListIdentifier,
    pub media_id: i32,
    pub title: String,
    pub episode_offset: Option<i64>,
    pub title_override: Option<String>,
}

impl Anime {
    pub fn build(
        media_list_group: &anilist::data::MediaListGroup,
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
                title: entry.media.title.to_string(),
                episode_offset,
                title_override,
            });
        }
        result.sort_by(|a, b| a.title.cmp(&b.title));
        result
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

#[cfg(test)]
mod tests {
    use super::*;

    fn make_media_title(title: &str) -> anilist::data::MediaTitle {
        anilist::data::MediaTitle {
            romaji: Some(String::from("irrelevant")),
            english: Some(String::from("irrelevant")),
            native: Some(String::from("irrelevant")),
            userPreferred: String::from(title),
        }
    }

    #[test]
    fn test_build() {
        let media_list_group = anilist::data::MediaListGroup {
            entries: vec![
                anilist::data::MediaList {
                    id: 12345,
                    progress: 1,
                    media: anilist::data::Media {
                        id: 1234,
                        title: make_media_title("Watashi wo Tabetai, Hitodenashi"),
                    },
                },
                anilist::data::MediaList {
                    id: 23456,
                    progress: 2,
                    media: anilist::data::Media {
                        id: 2345,
                        title: make_media_title("Boku no Hero Academia FINAL SEASON"),
                    },
                },
                anilist::data::MediaList {
                    id: 34567,
                    progress: 3,
                    media: anilist::data::Media {
                        id: 3456,
                        title: make_media_title("SPY×FAMILY Season 3"),
                    },
                },
            ],
        };
        let mut overrides = OverrideMap::from([
            (
                23456,
                (Some(String::from("Boku no Hero Academia (2025)")), None),
            ),
            (34567, (None, Some(-37))),
        ]);
        let result = Anime::build(&media_list_group, &mut overrides);
        assert_eq!(
            result,
            vec![
                Anime {
                    id: 23456,
                    media_id: 2345,
                    title: String::from("Boku no Hero Academia FINAL SEASON"),
                    episode_offset: None,
                    title_override: Some(String::from("Boku no Hero Academia (2025)")),
                },
                Anime {
                    id: 34567,
                    media_id: 3456,
                    title: String::from("SPY×FAMILY Season 3"),
                    episode_offset: Some(-37),
                    title_override: None,
                },
                Anime {
                    id: 12345,
                    media_id: 1234,
                    title: String::from("Watashi wo Tabetai, Hitodenashi"),
                    episode_offset: None,
                    title_override: None,
                },
            ]
        );
    }
}
