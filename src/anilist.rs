pub mod data;
pub mod queries;

use serde::{Deserialize, Serialize};

const MINIMUM_CONFIDENCE: f64 = 0.8;
const API_URL: &str = "https://graphql.anilist.co/";

pub type MediaListIdentifier = i64;
pub type UserIdentifier = i64;

#[derive(Debug)]
pub enum AnilistError {
    RequestDataError,
    ConnectionError,
    ParsingError,
    InvalidToken,
}

impl std::fmt::Display for AnilistError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::RequestDataError => write!(f, "Request data error"),
            Self::ConnectionError => write!(f, "Connection error"),
            Self::ParsingError => write!(f, "Parsing error"),
            Self::InvalidToken => write!(f, "Invalid token"),
        }
    }
}

#[derive(Debug, Serialize, Deserialize)]
pub struct MediaListCollectionQueryVariables {
    pub user_id: UserIdentifier,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct MediaListCollectionMutateVariables {
    pub id: MediaListIdentifier,
    pub progress: i32,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct Query<'a, T> {
    pub query: &'a str,
    pub variables: Option<T>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct QueryResponse<T> {
    pub data: T,
}

#[derive(Debug, Deserialize)]
struct ErrorResponse {
    errors: Option<Vec<Error>>,
}

#[derive(Debug, Deserialize)]
struct Error {
    message: String,
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
                log::debug!("{}", &response_body);
                log::debug!("{}", error);
                return Err(AnilistError::ParsingError);
            }
        };
        Ok(query_response.data)
    }
}

#[derive(Debug, PartialEq)]
pub struct AnilistClient {
    pub token: String,
    pub user_id: UserIdentifier,
}

pub trait AnilistClientTrait {
    async fn get_user(&self) -> Result<data::User, AnilistError>;
    async fn get_watching_list(&self) -> Result<data::MediaListGroup, AnilistError>;
    async fn update_progress(&self, media_list: &data::MediaList) -> Result<bool, AnilistError>;
}

impl AnilistClient {
    pub fn new(token: String, user_id: UserIdentifier) -> Self {
        Self { token, user_id }
    }

    /// Create a new Anilist client from only a token. Used for authentication only.
    pub fn new_from_token(token: String) -> Self {
        Self { token, user_id: 0 }
    }

    async fn send_query<T>(&self, query: Query<'_, T>) -> Result<reqwest::Response, AnilistError>
    where
        T: Serialize,
    {
        let body = serde_json::to_string(&query).map_err(|_| AnilistError::RequestDataError)?;
        let client = reqwest::Client::new();
        client
            .post(API_URL)
            .header("Content-Type", "application/json")
            .header("Accept", "application/json")
            .header("Authorization", format!("Bearer {}", self.token))
            .body(body)
            .send()
            .await
            .map_err(|_| AnilistError::ConnectionError)
    }
}

impl AnilistClientTrait for AnilistClient {
    async fn get_user(&self) -> Result<data::User, AnilistError> {
        let query = Query::<()> {
            query: queries::USER_QUERY,
            variables: None,
        };
        let response = self.send_query(query).await?;
        let viewer_data = QueryResponse::<data::ViewerData>::parse(response).await?;
        debug!(
            "Found user {} ({})",
            &viewer_data.Viewer.name, &viewer_data.Viewer.id
        );
        Ok(viewer_data.Viewer)
    }

    async fn get_watching_list(&self) -> Result<data::MediaListGroup, AnilistError> {
        let variables = MediaListCollectionQueryVariables {
            user_id: self.user_id,
        };
        let query = Query::<MediaListCollectionQueryVariables> {
            query: queries::MEDIALIST_QUERY,
            variables: Some(variables),
        };
        let response = self.send_query(query).await?;
        let media_list_collection_data =
            QueryResponse::<data::MediaListCollectionData>::parse(response).await?;
        let mut collected_list = data::MediaListGroup::empty();
        for mut list in media_list_collection_data.MediaListCollection.lists {
            collected_list.entries.append(&mut list.entries);
        }
        Ok(collected_list)
    }

    async fn update_progress(&self, media_list: &data::MediaList) -> Result<bool, AnilistError> {
        let variables = MediaListCollectionMutateVariables {
            id: media_list.id,
            progress: media_list.progress + 1,
        };
        let query = Query::<MediaListCollectionMutateVariables> {
            query: queries::MEDIALIST_MUTATION,
            variables: Some(variables),
        };
        let response = self.send_query(query).await?;
        let data = QueryResponse::<data::SaveMediaListEntryData>::parse(response).await?;
        Ok(data.SaveMediaListEntry.progress == media_list.progress + 1)
    }
}
