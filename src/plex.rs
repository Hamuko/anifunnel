use serde::Deserialize;

#[derive(Debug, Deserialize)]
pub struct Webhook {
    event: String,

    #[serde(rename = "Account")]
    pub account: WebhookAccount,

    #[serde(rename = "Metadata")]
    pub metadata: WebhookMetadata,
}

#[derive(Debug, PartialEq)]
pub enum WebhookState {
    Actionable,
    NonScrobbleEvent,
    IncorrectSeason,
    IncorrectType,
}

impl Webhook {
    pub fn is_actionable(&self, multi_season: bool) -> WebhookState {
        if self.event != "media.scrobble" {
            return WebhookState::NonScrobbleEvent;
        }
        if self.metadata.media_type != "episode" {
            return WebhookState::IncorrectType;
        }
        let allowed_season = match multi_season {
            true => self.metadata.season_number >= 1,
            false => self.metadata.season_number == 1,
        };
        if !allowed_season {
            return WebhookState::IncorrectSeason;
        }
        WebhookState::Actionable
    }
}

#[derive(Debug, Deserialize)]
pub struct WebhookAccount {
    #[serde(rename = "title")]
    pub name: String,
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn webhook_actionable() {
        let webhook = Webhook {
            event: String::from("media.scrobble"),
            account: WebhookAccount {
                name: String::from("yukikaze"),
            },
            metadata: WebhookMetadata {
                media_type: String::from("episode"),
                title: String::from("Onii-chan wa Oshimai!"),
                season_number: 1,
                episode_number: 4,
            },
        };
        assert_eq!(webhook.is_actionable(false), WebhookState::Actionable);
    }

    #[test]
    // First episodes are also actionable to allow for offsets.
    fn webhook_actionable_first_episode() {
        let webhook = Webhook {
            event: String::from("media.scrobble"),
            account: WebhookAccount {
                name: String::from("yukikaze"),
            },
            metadata: WebhookMetadata {
                media_type: String::from("episode"),
                title: String::from("Onii-chan wa Oshimai!"),
                season_number: 1,
                episode_number: 1,
            },
        };
        assert_eq!(webhook.is_actionable(false), WebhookState::Actionable);
    }

    #[test]
    // Music scrobbles are not actionable.
    fn webhook_actionable_music() {
        let webhook = Webhook {
            event: String::from("media.scrobble"),
            account: WebhookAccount {
                name: String::from("yukikaze"),
            },
            metadata: WebhookMetadata {
                media_type: String::from("track"),
                title: String::from("Onii-chan wa Oshimai!"),
                season_number: 1,
                episode_number: 4,
            },
        };
        assert_eq!(webhook.is_actionable(false), WebhookState::IncorrectType);
    }

    #[test]
    // Only scrobble events trigger anifunnel.
    fn webhook_actionable_playback() {
        let webhook = Webhook {
            event: String::from("media.play"),
            account: WebhookAccount {
                name: String::from("yukikaze"),
            },
            metadata: WebhookMetadata {
                media_type: String::from("episode"),
                title: String::from("Onii-chan wa Oshimai!"),
                season_number: 1,
                episode_number: 4,
            },
        };
        assert_eq!(webhook.is_actionable(false), WebhookState::NonScrobbleEvent);
    }

    #[test]
    // Second seasons are not actionable.
    fn webhook_actionable_second_season() {
        let webhook = Webhook {
            event: String::from("media.scrobble"),
            account: WebhookAccount {
                name: String::from("yukikaze"),
            },
            metadata: WebhookMetadata {
                media_type: String::from("episode"),
                title: String::from("Kidou Senshi Gundam: Suisei no Majo"),
                season_number: 2,
                episode_number: 4,
            },
        };
        assert_eq!(webhook.is_actionable(false), WebhookState::IncorrectSeason);
    }

    #[test]
    // Second seasons are actionable with --multi-season.
    fn webhook_actionable_second_season_multi_season() {
        let webhook = Webhook {
            event: String::from("media.scrobble"),
            account: WebhookAccount {
                name: String::from("yukikaze"),
            },
            metadata: WebhookMetadata {
                media_type: String::from("episode"),
                title: String::from("Kidou Senshi Gundam: Suisei no Majo"),
                season_number: 2,
                episode_number: 4,
            },
        };
        assert_eq!(webhook.is_actionable(true), WebhookState::Actionable);
    }

    #[test]
    // Specials are not actionable.
    fn webhook_actionable_special() {
        let webhook = Webhook {
            event: String::from("media.scrobble"),
            account: WebhookAccount {
                name: String::from("yukikaze"),
            },
            metadata: WebhookMetadata {
                media_type: String::from("episode"),
                title: String::from("Bakemonogatari"),
                season_number: 0,
                episode_number: 3,
            },
        };
        assert_eq!(webhook.is_actionable(false), WebhookState::IncorrectSeason);
    }

    #[test]
    // Specials are not actionable even with --multi-season.
    fn webhook_actionable_special_multi_season() {
        let webhook = Webhook {
            event: String::from("media.scrobble"),
            account: WebhookAccount {
                name: String::from("yukikaze"),
            },
            metadata: WebhookMetadata {
                media_type: String::from("episode"),
                title: String::from("Bakemonogatari"),
                season_number: 0,
                episode_number: 3,
            },
        };
        assert_eq!(webhook.is_actionable(true), WebhookState::IncorrectSeason);
    }
}
