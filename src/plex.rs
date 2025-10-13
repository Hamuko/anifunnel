use serde::{Deserialize, Deserializer};
use serde_json::Value;

#[derive(Debug, Deserialize, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum MediaType {
    Album,
    Artist,
    Clip,
    Collection,
    Episode,
    Movie,
    Person,
    Photo,
    PhotoAlbum,
    Playlist,
    PlaylistFolder,
    Season,
    Show,
    Track,
    Trailer,
}

#[derive(Debug, Deserialize)]
pub struct Webhook {
    event: String,

    #[serde(rename = "Account")]
    pub account: WebhookAccount,

    #[serde(
        default,
        rename = "Metadata",
        deserialize_with = "webhook_metadata_wrapper"
    )]
    pub metadata: Option<WebhookMetadata>,
}

/// Convert incomplete metadata into None for non-scrobble events.
fn webhook_metadata_wrapper<'de, D>(deserializer: D) -> Result<Option<WebhookMetadata>, D::Error>
where
    D: Deserializer<'de>,
{
    let v: Value = Deserialize::deserialize(deserializer)?;
    Ok(Option::deserialize(v).unwrap_or_default())
}

#[derive(Debug, PartialEq)]
pub enum WebhookState {
    Actionable,
    NoMetadata,
    NonScrobbleEvent,
    IncorrectSeason,
    IncorrectType,
}

impl Webhook {
    pub fn is_actionable(&self, multi_season: bool) -> WebhookState {
        if self.event != "media.scrobble" {
            return WebhookState::NonScrobbleEvent;
        }
        let Some(metadata) = &self.metadata else {
            // Metadata may not be present (or complete, which results in Webhook.metadata
            // being None) in non-scrobble webhooks, but should always be present in
            // scrobble events, meaning that this should never happen after checking the
            // event, but we have it just in case.
            return WebhookState::NoMetadata;
        };
        if metadata.media_type != MediaType::Episode {
            return WebhookState::IncorrectType;
        }
        let allowed_season = match multi_season {
            true => metadata.season_number >= 1,
            false => metadata.season_number == 1,
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

#[derive(Debug, Deserialize, PartialEq)]
pub struct WebhookMetadata {
    #[serde(rename = "type")]
    pub media_type: MediaType,

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
    // admin.database.backup events have no Metadata object.
    fn deserialize_admin_database_backup() {
        let json = r#"
            {
                "event": "admin.database.backup",
                "user": true,
                "owner": true,
                "Account": {
                    "title": "Username"
                },
                "Server": {
                    "title": "Example"
                }
            }
        "#;
        let webhook = serde_json::from_str::<Webhook>(json.into()).unwrap();
        assert_eq!(webhook.event, "admin.database.backup");
        assert_eq!(webhook.account.name, "Username");
        assert_eq!(webhook.metadata, None);
    }

    #[test]
    // library.new events have missing fields in the Metadata object.
    fn deserialize_library_new() {
        let json = r#"
            {
                "event": "library.new",
                "user": true,
                "owner": true,
                "Account": {
                    "title": "Username"
                },
                "Server": {
                    "title": "Example"
                },
                "Metadata": {
                    "index": 1,
                    "title": "Chanto Suenai Kyuuketsuki-chan",
                    "type": "show"
                }
            }
        "#;
        let webhook = serde_json::from_str::<Webhook>(json).unwrap();
        assert_eq!(webhook.event, "library.new");
        assert_eq!(webhook.account.name, "Username");
        assert_eq!(webhook.metadata, None);
    }

    #[test]
    fn deserialize_scrobble() {
        let json = r#"
            {
                "event": "media.scrobble",
                "user": true,
                "owner": true,
                "Account": {
                    "title": "Username"
                },
                "Server": {
                    "title": "Example"
                },
                "Metadata": {
                    "grandparentTitle": "Chanto Suenai Kyuuketsuki-chan",
                    "index": 2,
                    "parentIndex": 1,
                    "type": "episode"
                }
            }
        "#;
        let webhook = serde_json::from_str::<Webhook>(json).unwrap();
        assert_eq!(webhook.event, "media.scrobble");
        assert_eq!(webhook.account.name, "Username");
        assert_eq!(
            webhook.metadata,
            Some(WebhookMetadata {
                media_type: MediaType::Episode,
                title: "Chanto Suenai Kyuuketsuki-chan".into(),
                season_number: 1,
                episode_number: 2,
            })
        );
    }

    #[test]
    // Hypothetical media.scrobble event with missing fields in the Metadata object.
    fn deserialize_scrobble_corrupted() {
        let json = r#"
            {
                "event": "media.scrobble",
                "user": true,
                "owner": true,
                "Account": {
                    "title": "Username"
                },
                "Server": {
                    "title": "Example"
                },
                "Metadata": {
                    "grandparentTitle": "Chanto Suenai Kyuuketsuki-chan",
                    "type": "episode"
                }
            }
        "#;
        let webhook = serde_json::from_str::<Webhook>(json).unwrap();
        assert_eq!(webhook.event, "media.scrobble");
        assert_eq!(webhook.account.name, "Username");
        assert_eq!(webhook.metadata, None);
    }

    #[test]
    fn webhook_actionable() {
        let webhook = Webhook {
            event: String::from("media.scrobble"),
            account: WebhookAccount {
                name: String::from("yukikaze"),
            },
            metadata: Some(WebhookMetadata {
                media_type: MediaType::Episode,
                title: String::from("Onii-chan wa Oshimai!"),
                season_number: 1,
                episode_number: 4,
            }),
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
            metadata: Some(WebhookMetadata {
                media_type: MediaType::Episode,
                title: String::from("Onii-chan wa Oshimai!"),
                season_number: 1,
                episode_number: 1,
            }),
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
            metadata: Some(WebhookMetadata {
                media_type: MediaType::Track,
                title: String::from("Onii-chan wa Oshimai!"),
                season_number: 1,
                episode_number: 4,
            }),
        };
        assert_eq!(webhook.is_actionable(false), WebhookState::IncorrectType);
    }

    #[test]
    // Scrobbles with unreadable Metadata objects are not actionable. Mostly hypothetical.
    fn webhook_actionable_missing_metadata() {
        let webhook = Webhook {
            event: String::from("media.scrobble"),
            account: WebhookAccount {
                name: String::from("yukikaze"),
            },
            metadata: None,
        };
        assert_eq!(webhook.is_actionable(false), WebhookState::NoMetadata);
    }

    #[test]
    // Only scrobble events trigger anifunnel.
    fn webhook_actionable_playback() {
        let webhook = Webhook {
            event: String::from("media.play"),
            account: WebhookAccount {
                name: String::from("yukikaze"),
            },
            metadata: Some(WebhookMetadata {
                media_type: MediaType::Episode,
                title: String::from("Onii-chan wa Oshimai!"),
                season_number: 1,
                episode_number: 4,
            }),
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
            metadata: Some(WebhookMetadata {
                media_type: MediaType::Episode,
                title: String::from("Kidou Senshi Gundam: Suisei no Majo"),
                season_number: 2,
                episode_number: 4,
            }),
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
            metadata: Some(WebhookMetadata {
                media_type: MediaType::Episode,
                title: String::from("Kidou Senshi Gundam: Suisei no Majo"),
                season_number: 2,
                episode_number: 4,
            }),
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
            metadata: Some(WebhookMetadata {
                media_type: MediaType::Episode,
                title: String::from("Bakemonogatari"),
                season_number: 0,
                episode_number: 3,
            }),
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
            metadata: Some(WebhookMetadata {
                media_type: MediaType::Episode,
                title: String::from("Bakemonogatari"),
                season_number: 0,
                episode_number: 3,
            }),
        };
        assert_eq!(webhook.is_actionable(true), WebhookState::IncorrectSeason);
    }
}
