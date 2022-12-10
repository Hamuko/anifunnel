use serde::Deserialize;

#[derive(Debug, Deserialize)]
pub struct Webhook {
    event: String,

    #[serde(rename = "Metadata")]
    pub metadata: WebhookMetadata,
}

impl Webhook {
    pub fn is_actionable(self: &Self) -> bool {
        return self.event == "media.scrobble"
            && self.metadata.media_type == "episode"
            && self.metadata.season_number == 1
            && self.metadata.episode_number > 1;
    }
}

#[derive(Debug, Deserialize)]
pub struct WebhookMetadata {
    #[serde(rename = "type")]
    pub media_type: String,

    #[serde(rename = "grandparentTitle")]
    pub title: String,

    #[serde(rename = "parentIndex")]
    pub season_number: i32,

    #[serde(rename = "index")]
    pub episode_number: i32,
}
