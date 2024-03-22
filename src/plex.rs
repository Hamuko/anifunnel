use serde::Deserialize;

#[derive(Debug, Deserialize)]
pub struct Webhook {
    event: String,

    #[serde(rename = "Account")]
    pub account: WebhookAccount,

    #[serde(rename = "Metadata")]
    pub metadata: WebhookMetadata,
}

impl Webhook {
    pub fn is_actionable(self: &Self, multi_season: bool) -> bool {
        return self.event == "media.scrobble"
            && self.metadata.media_type == "episode"
            && (self.metadata.season_number == 1
                || (multi_season && self.metadata.season_number >= 1));
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
            account: WebhookAccount { name: String::from("yukikaze") },
            metadata: WebhookMetadata {
                media_type: String::from("episode"),
                title: String::from("Onii-chan wa Oshimai!"),
                season_number: 1,
                episode_number: 4,
            },
        };
        assert_eq!(webhook.is_actionable(false), true);
    }

    #[test]
    // First episodes are also actionable to allow for offsets.
    fn webhook_actionable_first_episode() {
        let webhook = Webhook {
            event: String::from("media.scrobble"),
            account: WebhookAccount { name: String::from("yukikaze") },
            metadata: WebhookMetadata {
                media_type: String::from("episode"),
                title: String::from("Onii-chan wa Oshimai!"),
                season_number: 1,
                episode_number: 1,
            },
        };
        assert_eq!(webhook.is_actionable(false), true);
    }

    #[test]
    // Music scrobbles are not actionable.
    fn webhook_actionable_music() {
        let webhook = Webhook {
            event: String::from("media.scrobble"),
            account: WebhookAccount { name: String::from("yukikaze") },
            metadata: WebhookMetadata {
                media_type: String::from("track"),
                title: String::from("Onii-chan wa Oshimai!"),
                season_number: 1,
                episode_number: 4,
            },
        };
        assert_eq!(webhook.is_actionable(false), false);
    }

    #[test]
    // Only scrobble events trigger anifunnel.
    fn webhook_actionable_playback() {
        let webhook = Webhook {
            event: String::from("media.play"),
            account: WebhookAccount { name: String::from("yukikaze") },
            metadata: WebhookMetadata {
                media_type: String::from("episode"),
                title: String::from("Onii-chan wa Oshimai!"),
                season_number: 1,
                episode_number: 4,
            },
        };
        assert_eq!(webhook.is_actionable(false), false);
    }

    #[test]
    // Second seasons are not actionable.
    fn webhook_actionable_second_season() {
        let webhook = Webhook {
            event: String::from("media.scrobble"),
            account: WebhookAccount { name: String::from("yukikaze") },
            metadata: WebhookMetadata {
                media_type: String::from("episode"),
                title: String::from("Kidou Senshi Gundam: Suisei no Majo"),
                season_number: 2,
                episode_number: 4,
            },
        };
        assert_eq!(webhook.is_actionable(false), false);
    }

    #[test]
    // Second seasons are actionable with --multi-season.
    fn webhook_actionable_second_season_multi_season() {
        let webhook = Webhook {
            event: String::from("media.scrobble"),
            account: WebhookAccount { name: String::from("yukikaze") },
            metadata: WebhookMetadata {
                media_type: String::from("episode"),
                title: String::from("Kidou Senshi Gundam: Suisei no Majo"),
                season_number: 2,
                episode_number: 4,
            },
        };
        assert_eq!(webhook.is_actionable(true), true);
    }

    #[test]
    // Specials are not actionable.
    fn webhook_actionable_special() {
        let webhook = Webhook {
            event: String::from("media.scrobble"),
            account: WebhookAccount { name: String::from("yukikaze") },
            metadata: WebhookMetadata {
                media_type: String::from("episode"),
                title: String::from("Bakemonogatari"),
                season_number: 0,
                episode_number: 3,
            },
        };
        assert_eq!(webhook.is_actionable(false), false);
    }

    #[test]
    // Specials are not actionable even with --multi-season.
    fn webhook_actionable_special_multi_season() {
        let webhook = Webhook {
            event: String::from("media.scrobble"),
            account: WebhookAccount { name: String::from("yukikaze") },
            metadata: WebhookMetadata {
                media_type: String::from("episode"),
                title: String::from("Bakemonogatari"),
                season_number: 0,
                episode_number: 3,
            },
        };
        assert_eq!(webhook.is_actionable(true), false);
    }
}
