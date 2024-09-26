use std::fmt;

use log::{debug, info};
use regex::Regex;
use serde::{Deserialize, Serialize};
use strsim::normalized_levenshtein;

const MEDIALIST_MUTATION: &str = "
mutation($id: Int, $progress: Int) {
  SaveMediaListEntry(id: $id, progress: $progress) {
    progress
  }
}
";
const MEDIALIST_QUERY: &str = "
query MediaListCollection($user_id: Int) {
    MediaListCollection(userId: $user_id, status_in: [CURRENT, REPEATING], type: ANIME) {
        lists {
            entries {
                id
                progress
                media {
                    title {
                        romaji
                        english
                        native
                        userPreferred
                    }
                    synonyms
                }
            }
        }
    }
}
";
const USER_QUERY: &str = "
query {
    Viewer {
        id
        name
    }
}
";
const MINIMUM_CONFIDENCE: f64 = 0.8;

#[derive(Debug)]
pub enum AnilistError {
    RequestDataError,
    ConnectionError,
    ParsingError,
    InvalidToken,
}

#[derive(Clone, Debug, Deserialize)]
pub struct Media {
    pub title: MediaTitle,
}

#[derive(Clone, Debug, Deserialize)]
pub struct MediaList {
    pub id: i32,
    pub progress: i32,
    pub media: Media,
}

impl MediaList {
    pub async fn update(self: &Self, token: &String) -> Result<bool, AnilistError> {
        let variables = MediaListCollectionMutateVariables {
            id: self.id,
            progress: self.progress + 1,
        };
        let query = Query::<MediaListCollectionMutateVariables> {
            query: MEDIALIST_MUTATION,
            variables: Some(variables),
        };
        let response = send_query(token, query).await?;
        let data = QueryResponse::<SaveMediaListEntryData>::parse(response).await?;
        Ok(data.SaveMediaListEntry.progress == self.progress + 1)
    }
}

impl fmt::Display for MediaList {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "MediaList {{ id: {} }}", self.id)
    }
}

#[derive(Debug, Deserialize)]
struct MediaListCollection {
    lists: Vec<MediaListGroup>,
}

#[allow(non_snake_case)]
#[derive(Debug, Deserialize)]
struct MediaListCollectionData {
    MediaListCollection: MediaListCollection,
}

#[derive(Debug, Serialize, Deserialize)]
struct MediaListCollectionQueryVariables {
    user_id: i32,
}

#[derive(Debug, Serialize, Deserialize)]
struct MediaListCollectionMutateVariables {
    id: i32,
    progress: i32,
}

#[derive(Clone, Debug, Deserialize)]
pub struct MediaListGroup {
    entries: Vec<MediaList>,
}

impl MediaListGroup {
    pub fn find_id(self: &Self, id: &i32) -> Option<&MediaList> {
        debug!("Matching ID \"{}\"", &id);
        for media_list in self.entries.iter() {
            if &media_list.id == id {
                return Some(media_list);
            }
        }
        return None;
    }

    pub fn find_match(self: &Self, title: &String) -> Option<&MediaList> {
        let match_title = title.to_lowercase();
        debug!("Matching title \"{}\"", &match_title);
        let mut best_match: (f64, Option<&MediaList>) = (0.0, None);
        for media_list in self.entries.iter() {
            let confidence = media_list.media.title.find_match(&match_title);
            if confidence == 1.0 {
                info!(
                    "{} was an exact match for {:?}",
                    media_list.media.title, title
                );
                return Some(media_list);
            }
            if confidence > best_match.0 {
                best_match = (confidence, Some(media_list));
            }
        }
        if let Some(media_list) = best_match.1 {
            info!(
                "{} was the best match for \"{}\" ({})",
                media_list.media.title, title, best_match.0
            );
            if best_match.0 >= MINIMUM_CONFIDENCE {
                return Some(media_list);
            }
        }
        return None;
    }

    pub fn empty() -> Self {
        Self {
            entries: Vec::new(),
        }
    }

    pub fn get_context_values<'a>(self: &'a Self) -> impl Iterator<Item = (i32, String)> + 'a {
        return self
            .entries
            .iter()
            .map(|x| (x.id, x.media.title.userPreferred.clone()));
    }
}

#[allow(non_snake_case)]
#[derive(Clone, Debug, Deserialize)]
pub struct MediaTitle {
    romaji: Option<String>,
    english: Option<String>,
    native: Option<String>,
    userPreferred: String,
}

fn remove_special_surrounding_characters(value: &str) -> &str {
    let mut start_pos = 0;
    let mut end_pos = 0;
    for (pos, chr) in value.char_indices() {
        start_pos = pos;
        if chr.is_alphanumeric() || chr == '(' {
            break;
        }
    }
    for (pos, chr) in value.char_indices().rev() {
        end_pos = pos;
        if chr.is_alphanumeric() || chr == ')' {
            break;
        }
    }
    while !value.is_char_boundary(end_pos + 1) {
        end_pos += 1;
    }
    return &value[start_pos..=end_pos];
}

impl MediaTitle {
    fn find_match(self: &Self, string: &String) -> f64 {
        let mut titles: Vec<String> = Vec::new();
        for title in [&self.romaji, &self.english, &self.native] {
            if let Some(title) = title {
                titles.push(title.to_lowercase());
            }
        }

        // Try an exact match first..
        for title in titles.iter() {
            if title == string {
                return 1.0;
            }
        }

        let mut best_match: f64 = 0.0;

        // Regular case insensitive Levenshtein-based fuzzy matching.
        for title in titles.iter() {
            let confidence = normalized_levenshtein(string, &title);
            debug!("~ {} = {}", &title, &confidence);
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
        debug!("Matching fallback title \"{}\"", &massaged_string);
        for title in titles.iter() {
            let massaged_title = remove_regexes(&massaging_regexes, &title);
            let massaged_title = remove_special_surrounding_characters(&massaged_title);
            let confidence =
                (normalized_levenshtein(&massaged_string, &massaged_title) - 0.05).max(0.0);
            debug!("~ {} = {}", &massaged_title, &confidence);
            if confidence > best_match {
                best_match = confidence;
            }
        }

        return best_match;
    }
}

impl fmt::Display for MediaTitle {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{}", self.userPreferred)
    }
}

#[derive(Debug, Serialize)]
struct Query<'a, T> {
    query: &'a str,
    variables: Option<T>,
}

#[derive(Debug, Deserialize)]
struct QueryResponse<T> {
    data: T,
}

#[derive(Debug, Deserialize)]
struct ErrorResponse {
    errors: Option<Vec<Error>>,
}

#[derive(Debug, Deserialize)]
struct Error {
    message: String,
}

#[derive(Debug, Deserialize)]
struct SaveMediaListEntry {
    progress: i32,
}

#[allow(non_snake_case)]
#[derive(Debug, Deserialize)]
struct SaveMediaListEntryData {
    SaveMediaListEntry: SaveMediaListEntry,
}

impl<T> QueryResponse<T> {
    async fn parse(response: reqwest::Response) -> Result<T, AnilistError>
    where
        T: for<'a> Deserialize<'a>,
    {
        let status_code = response.status();
        let response_body = response
            .text()
            .await
            .map_err(|_| AnilistError::RequestDataError)?;
        if status_code == 400 {
            if let Ok(error_response) = serde_json::from_str::<ErrorResponse>(&response_body) {
                for error in error_response.errors.iter().flatten() {
                    if error.message == "Invalid token" {
                        return Err(AnilistError::InvalidToken);
                    }
                }
            }
        }
        let query_response: QueryResponse<T> = match serde_json::from_str(&response_body) {
            Ok(response) => response,
            Err(error) => {
                debug!("{}", &response_body);
                debug!("{}", error);
                return Err(AnilistError::ParsingError);
            }
        };
        return Ok(query_response.data);
    }
}

#[derive(Debug, Deserialize)]
pub struct User {
    pub id: i32,
    pub name: String,
}

#[allow(non_snake_case)]
#[derive(Debug, Deserialize)]
struct ViewerData {
    Viewer: User,
}

/// Remove parts of a given string using a collection of regular expressions.
fn remove_regexes(regexes: &[Regex], string: &String) -> String {
    return regexes
        .iter()
        .fold(string.clone(), |s, regex| regex.replace(&s, "").to_string());
}

pub async fn get_user(token: &String) -> Result<User, AnilistError> {
    let query = Query::<()> {
        query: USER_QUERY,
        variables: None,
    };
    let response = send_query(token, query).await?;
    let viewer_data = QueryResponse::<ViewerData>::parse(response).await?;
    debug!(
        "Found user {} ({})",
        &viewer_data.Viewer.name, &viewer_data.Viewer.id
    );
    return Ok(viewer_data.Viewer);
}

pub async fn get_watching_list(
    token: &String,
    user: &User,
) -> Result<MediaListGroup, AnilistError> {
    let variables = MediaListCollectionQueryVariables { user_id: user.id };
    let query = Query::<MediaListCollectionQueryVariables> {
        query: MEDIALIST_QUERY,
        variables: Some(variables),
    };
    let response = send_query(token, query).await?;
    let media_list_collection_data =
        QueryResponse::<MediaListCollectionData>::parse(response).await?;
    let mut collected_list = MediaListGroup::empty();
    for mut list in media_list_collection_data.MediaListCollection.lists {
        collected_list.entries.append(&mut list.entries);
    }
    Ok(collected_list)
}

async fn send_query<T>(
    token: &String,
    query: Query<'_, T>,
) -> Result<reqwest::Response, AnilistError>
where
    T: Serialize,
{
    let body = serde_json::to_string(&query).map_err(|_| AnilistError::RequestDataError)?;
    let client = reqwest::Client::new();
    return Ok(client
        .post("https://graphql.anilist.co/")
        .header("Content-Type", "application/json")
        .header("Accept", "application/json")
        .header("Authorization", format!("Bearer {}", token))
        .body(body)
        .send()
        .await
        .map_err(|_| AnilistError::ConnectionError)?);
}

#[cfg(test)]
mod tests {
    use super::*;

    use test_case::test_case;

    fn fake_media_list(id: i32, title: &str) -> MediaList {
        let title = String::from(title);
        return MediaList {
            id,
            progress: 3,
            media: Media {
                title: MediaTitle {
                    romaji: Some(title.clone()),
                    english: Some(title.clone()),
                    native: Some(title.clone()),
                    userPreferred: title.clone(),
                },
            },
        };
    }

    impl PartialEq for &MediaList {
        fn eq(&self, other: &Self) -> bool {
            self.media.title.romaji == other.media.title.romaji
                && self.media.title.english == other.media.title.english
                && self.media.title.native == other.media.title.native
                && self.media.title.userPreferred == other.media.title.userPreferred
        }
    }

    #[test]
    fn media_list_group_get_context_values() {
        let media_list_group = MediaListGroup {
            entries: vec![
                fake_media_list(146065, "Mushoku Tensei II"),
                fake_media_list(163132, "Horimiya -piece-"),
            ],
        };

        let values: Vec<(i32, String)> = media_list_group.get_context_values().collect();
        assert_eq!(
            values,
            vec![
                (146065, String::from("Mushoku Tensei II")),
                (163132, String::from("Horimiya -piece-"))
            ]
        );
    }

    #[test_case(146065, Some("Mushoku Tensei II") ; "valid ID")]
    #[test_case(163132, Some("Horimiya -piece-") ; "also valid ID")]
    #[test_case(163133, None ; "invalid ID")]
    fn media_list_group_get_id(id: i32, expected: Option<&str>) {
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

    #[test]
    // Test that remove_regexes() removes given regex patterns from a string.
    fn regex_removal() {
        let regexes = [
            Regex::new(r"\([A-z]+\) ").unwrap(),
            Regex::new(r"([0-9]+)?(1st|2nd|3rd|[4-90]th) ").unwrap(),
            Regex::new(r"\.+$").unwrap(),
        ];
        let input = String::from("This is (arguably) the day of the 21st century.");
        let output = remove_regexes(&regexes, &input);
        assert_eq!(output, "This is the day of the century");
    }

    #[test_case("(Oshi no Ko)", "(Oshi no Ko)" ; "surrounding parentheses")]
    #[test_case("2.5 Jigen no Ririsa", "2.5 Jigen no Ririsa" ; "leading numbers")]
    #[test_case("[Oshi no Ko]", "Oshi no Ko" ; "surrounding brackets")]
    #[test_case("\"Oshi no Ko\"", "Oshi no Ko" ; "surrounding quotes")]
    #[test_case("Anne Happy♪", "Anne Happy" ; "trailing note")]
    #[test_case("Black★Rock Shooter", "Black★Rock Shooter" ; "special character between")]
    #[test_case("Girlfriend (Kari)", "Girlfriend (Kari)" ; "trailing parenthesis")]
    #[test_case("らき☆すた", "らき☆すた" ; "special character between Japanese")]
    #[test_case("【推しの子】", "推しの子" ; "surrounding quotes Japanese")]
    fn special_surrounding_characters_removal(input: &str, expected: &str) {
        let output = remove_special_surrounding_characters(&input);
        assert_eq!(output, expected);
    }
}
