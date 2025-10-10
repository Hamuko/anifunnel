use crate::anilist::{MediaListIdentifier, UserIdentifier, MINIMUM_CONFIDENCE};
use crate::utils::{remove_regexes, remove_special_surrounding_characters};
use regex::Regex;
use serde::{Deserialize, Serialize};
use std::fmt;
use strsim::normalized_levenshtein;

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Media {
    pub id: i32,
    pub title: MediaTitle,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct MediaList {
    pub id: MediaListIdentifier,
    pub progress: i32,
    pub media: Media,
}

impl fmt::Display for MediaList {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "MediaList {{ id: {} }}", self.id)
    }
}

#[derive(Debug, Serialize, Deserialize)]
pub struct MediaListCollection {
    pub lists: Vec<MediaListGroup>,
}

#[allow(non_snake_case)]
#[derive(Debug, Serialize, Deserialize)]
pub struct MediaListCollectionData {
    pub MediaListCollection: MediaListCollection,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct MediaListGroup {
    pub entries: Vec<MediaList>,
}

impl MediaListGroup {
    pub fn find_id(&self, id: &MediaListIdentifier) -> Option<&MediaList> {
        log::debug!("Matching by ID \"{}\"", &id);
        self.entries.iter().find(|&media_list| &media_list.id == id)
    }

    pub fn find_match(&self, title: &String) -> Option<&MediaList> {
        let match_title = title.to_lowercase();
        log::debug!("Matching by title \"{}\"", &match_title);
        let mut best_match: (f64, Option<&MediaList>) = (0.0, None);
        for media_list in self.entries.iter() {
            let confidence = media_list.media.title.find_match(&match_title);
            if confidence == 1.0 {
                log::info!(
                    "{} was an exact match for {:?}",
                    media_list.media.title,
                    title
                );
                return Some(media_list);
            }
            if confidence > best_match.0 {
                best_match = (confidence, Some(media_list));
            }
        }
        if let Some(media_list) = best_match.1 {
            log::info!(
                "{} was the best match for \"{}\" ({})",
                media_list.media.title,
                title,
                best_match.0
            );
            if best_match.0 >= MINIMUM_CONFIDENCE {
                return Some(media_list);
            }
        }
        None
    }

    pub fn empty() -> Self {
        Self {
            entries: Vec::new(),
        }
    }
}

#[allow(non_snake_case)]
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct MediaTitle {
    pub romaji: Option<String>,
    pub english: Option<String>,
    pub native: Option<String>,
    pub userPreferred: String,
}

impl MediaTitle {
    fn find_match(&self, string: &String) -> f64 {
        let mut titles: Vec<String> = Vec::new();
        let available_titles = [&self.romaji, &self.english, &self.native]
            .into_iter()
            .flatten();
        for title in available_titles {
            titles.push(title.to_lowercase());
        }

        // Try an exact match first.
        for title in titles.iter() {
            if title == string {
                return 1.0;
            }
        }

        let mut best_match: f64 = 0.0;

        // Regular case insensitive Levenshtein-based fuzzy matching.
        for title in titles.iter() {
            let confidence = normalized_levenshtein(string, title);
            log::debug!("~ {} = {}", &title, &confidence);
            if confidence > best_match {
                best_match = confidence;
            }
        }

        if best_match >= MINIMUM_CONFIDENCE {
            return best_match;
        }

        // Levenshtein distance with cleaned up comparison to get rid of common
        // suffixes that might alter between AniDB and local libraries.
        let massaging_regexes = [
            Regex::new(r" \(?20[2-4]\d\)?$").unwrap(), // XXX (2023)
            Regex::new(r" \d+(st|nd|rd|th) season$").unwrap(), // XXX 2nd Season
            Regex::new(r" \(?cour \d\)?$").unwrap(),   // XXX Cour 2, XXX (Cour 2)
            Regex::new(r" \(?season \d\)?$").unwrap(), // XXX Season 2, XXX (Season 2)
            Regex::new(r" \(?part \d\)?$").unwrap(),   // XXX Part 2, XXX (Part 2)
            Regex::new(r" \d$").unwrap(),              // XXX 2
        ];
        let massaged_string = remove_regexes(&massaging_regexes, string);
        let massaged_string = remove_special_surrounding_characters(&massaged_string);
        log::debug!("Matching fallback title \"{}\"", &massaged_string);
        for title in titles.iter() {
            let massaged_title = remove_regexes(&massaging_regexes, title);
            let massaged_title = remove_special_surrounding_characters(&massaged_title);
            let confidence =
                (normalized_levenshtein(massaged_string, massaged_title) - 0.05).max(0.0);
            log::debug!("~ {} = {}", &massaged_title, &confidence);
            if confidence > best_match {
                best_match = confidence;
            }
        }

        best_match
    }
}

impl fmt::Display for MediaTitle {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{}", self.userPreferred)
    }
}

#[derive(Debug, Serialize, Deserialize)]
pub struct SaveMediaListEntry {
    pub progress: i32,
}

#[allow(non_snake_case)]
#[derive(Debug, Serialize, Deserialize)]
pub struct SaveMediaListEntryData {
    pub SaveMediaListEntry: SaveMediaListEntry,
}

#[derive(Debug, Deserialize)]
pub struct User {
    pub id: UserIdentifier,
    pub name: String,
}

#[allow(non_snake_case)]
#[derive(Debug, Deserialize)]
pub struct ViewerData {
    pub Viewer: User,
}

#[cfg(test)]
mod tests {
    use super::*;

    use test_case::test_case;

    impl PartialEq for &MediaList {
        fn eq(&self, other: &Self) -> bool {
            self.media.title.romaji == other.media.title.romaji
                && self.media.title.english == other.media.title.english
                && self.media.title.native == other.media.title.native
                && self.media.title.userPreferred == other.media.title.userPreferred
        }
    }

    fn fake_media_list(id: MediaListIdentifier, title: &str) -> MediaList {
        let title = String::from(title);
        return MediaList {
            id,
            progress: 3,
            media: Media {
                id: 1,
                title: MediaTitle {
                    romaji: Some(title.clone()),
                    english: Some(title.clone()),
                    native: Some(title.clone()),
                    userPreferred: title.clone(),
                },
            },
        };
    }

    #[test_case(146065, Some("Mushoku Tensei II") ; "valid ID")]
    #[test_case(163132, Some("Horimiya -piece-") ; "also valid ID")]
    #[test_case(163133, None ; "invalid ID")]
    fn media_list_group_get_id(id: MediaListIdentifier, expected: Option<&str>) {
        let correct_media_list = fake_media_list(146065, "Mushoku Tensei II");
        let incorrect_media_list = fake_media_list(163132, "Horimiya -piece-");
        let media_list_group = MediaListGroup {
            entries: vec![incorrect_media_list.clone(), correct_media_list.clone()],
        };

        let matched = media_list_group.find_id(&id);
        assert_eq!(
            matched.map(|x| x.media.title.userPreferred.clone()),
            expected.map(|x| x.to_string())
        );
    }

    #[test]
    // Test that an exact match is picked over a very close match.
    fn media_list_group_close_match_exact_match() {
        let correct_title = "To Aru Kagaku no Railgun";
        let incorrect_title = "To Aru Kagaku no Railgun S";
        let search_title = String::from("To Aru Kagaku no Railgun");

        let correct_media_list = fake_media_list(146065, correct_title);
        let incorrect_media_list = fake_media_list(5678, incorrect_title);
        let media_list_group = MediaListGroup {
            entries: vec![incorrect_media_list.clone(), correct_media_list.clone()],
        };

        let matched = media_list_group.find_match(&search_title).unwrap();
        assert_eq!(matched, &correct_media_list);
    }

    #[test]
    // Test that the fallback fuzzy matching is used when given two strings with
    // different ways of identifying seasons/parts.
    fn media_list_group_fuzzy_matching() {
        let correct_title = "Muv-Luv Alternative Season 2";
        let incorrect_title = "Muv-Luv Alternative: Total Eclipse";
        let search_title = String::from("Muv-Luv Alternative (2022)");

        let correct_media_list = fake_media_list(1234, correct_title);
        let incorrect_media_list = fake_media_list(5678, incorrect_title);
        let media_list_group = MediaListGroup {
            entries: vec![incorrect_media_list.clone(), correct_media_list.clone()],
        };

        let matched = media_list_group.find_match(&search_title).unwrap();
        assert_eq!(matched, &correct_media_list);
    }

    #[test]
    fn media_list_group_fuzzy_matching_nth_season() {
        let correct_title = "Kanojo, Okarishimasu 3rd Season";
        let incorrect_title = "Kanojo mo Kanojo";
        let search_title = String::from("Kanojo, Okarishimasu (2023)");

        let correct_media_list = fake_media_list(1234, correct_title);
        let incorrect_media_list = fake_media_list(5678, incorrect_title);
        let media_list_group = MediaListGroup {
            entries: vec![incorrect_media_list.clone(), correct_media_list.clone()],
        };

        let matched = media_list_group.find_match(&search_title).unwrap();
        assert_eq!(matched, &correct_media_list);
    }

    #[test]
    fn media_list_group_fuzzy_matching_nth_season_special_characters() {
        let anidb_title = "[Oshi no Ko] 2nd Season";
        let search_title = String::from("\"Oshi no Ko\" (2024)");

        let media_list = fake_media_list(1234, anidb_title);
        let media_list_group = MediaListGroup {
            entries: vec![media_list.clone()],
        };

        let matched = media_list_group.find_match(&search_title).unwrap();
        assert_eq!(matched, &media_list);
    }

    #[test]
    // Test that the better of two close matches is picked.
    fn media_list_group_multiple_close_matches() {
        let correct_title = "To Aru Kagaku no Railgun";
        let incorrect_title = "To Aru Kagaku no Railgun S";
        let search_title = String::from("Toaru Kagaku no Railgun");

        let correct_media_list = fake_media_list(1234, correct_title);
        let incorrect_media_list = fake_media_list(5678, incorrect_title);
        let media_list_group = MediaListGroup {
            entries: vec![incorrect_media_list.clone(), correct_media_list.clone()],
        };

        let matched = media_list_group.find_match(&search_title).unwrap();
        assert_eq!(matched, &correct_media_list);
    }

    #[test]
    // Test that no match is returned in case no good match exists.
    fn media_list_group_no_match() {
        let incorrect_title = " Soredemo Ayumu wa Yosetekuru";
        let search_title = String::from("Soredemo Machi wa Mawatteiru");

        let incorrect_media_list = fake_media_list(1234, incorrect_title);
        let media_list_group = MediaListGroup {
            entries: vec![incorrect_media_list.clone()],
        };

        let matched = media_list_group.find_match(&search_title);
        assert!(matched.is_none());
    }
}
