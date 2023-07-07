pub mod forms {
    #[derive(Debug, FromForm)]
    pub struct Scrobble<'r> {
        pub payload: &'r str,
    }
}

pub mod state {
    use crate::anilist;
    use std::collections::HashMap;
    use tokio::sync::RwLock;

    #[derive(Debug)]
    /// Global anifunnel application state.
    pub struct Global {
        pub multi_season: bool,
        pub token: String,
        pub user: anilist::User,
        pub title_overrides: RwLock<TitleOverrides>,
        pub episode_offsets: RwLock<EpisodeOverrides>,
    }

    #[derive(Debug)]
    pub struct EpisodeOverrides {
        inner: HashMap<i32, i32>,
    }

    #[derive(Debug)]
    pub struct TitleOverrides {
        inner: HashMap<String, i32>,
    }

    impl EpisodeOverrides {
        pub fn new() -> Self {
            Self {
                inner: HashMap::new(),
            }
        }

        pub fn get(self: &Self, key: &i32) -> Option<i32> {
            return self.inner.get(key).copied();
        }

        pub fn set(self: &mut Self, key: i32, value: i32) {
            self.inner.insert(key, value);
        }

        pub fn remove(self: &mut Self, key: &i32) {
            self.inner.remove(key);
        }
    }

    /// Title override map between titles (String) and Anilist IDs (i32).
    impl TitleOverrides {
        pub fn new() -> Self {
            Self {
                inner: HashMap::new(),
            }
        }

        pub fn get(self: &Self, key: &String) -> Option<i32> {
            return self.inner.get(key).copied();
        }

        pub fn get_key(self: &Self, value: &i32) -> Option<String> {
            for (key, inner_value) in self.inner.clone().iter() {
                if inner_value == value {
                    return Some(key.clone());
                }
            }
            return None;
        }

        /// Set a title override for an ID. Replaces existing title or ID.
        pub fn set(self: &mut Self, key: String, value: i32) {
            self.remove_value(&value);
            self.inner.insert(key, value);
        }

        /// Remove override by the ID.
        pub fn remove_value(self: &mut Self, value: &i32) {
            for (inner_key, inner_value) in self.inner.clone().iter() {
                if inner_value == value {
                    self.inner.remove(inner_key);
                }
            }
        }
    }

    #[cfg(test)]
    mod tests {
        use std::collections::HashMap;
        use test_case::test_case;

        use crate::data::state::{EpisodeOverrides, TitleOverrides};

        fn get_inner_contents<K: std::cmp::Ord, V: std::cmp::Ord>(
            inner: &HashMap<K, V>,
        ) -> Vec<(&K, &V)> {
            let mut values = inner.iter().collect::<Vec<(&K, &V)>>();
            values.sort();
            return values;
        }

        #[test_case(146065, Some(1) ; "valid positive key")]
        #[test_case(163132, Some(-12) ; "valid negative key")]
        #[test_case(160188, None ; "invalid key")]
        fn episode_override_get(key: i32, result: Option<i32>) {
            let episode_overrides = EpisodeOverrides {
                inner: HashMap::from([(146065, 1), (163132, -12)]),
            };
            assert_eq!(episode_overrides.get(&key), result);
        }

        #[test]
        fn episode_override_new() {
            let episode_overrides = EpisodeOverrides::new();
            assert!(episode_overrides.inner.is_empty());
        }

        #[test]
        fn episode_override_remove() {
            let mut episode_overrides = EpisodeOverrides {
                inner: HashMap::from([(146065, 1), (163132, -12)]),
            };
            episode_overrides.remove(&146065);
            assert_eq!(
                get_inner_contents(&episode_overrides.inner),
                [(&163132, &-12)]
            );
        }

        #[test]
        fn episode_override_set() {
            let mut episode_overrides = EpisodeOverrides {
                inner: HashMap::from([(146065, 1)]),
            };
            episode_overrides.set(163132, -12);
            assert_eq!(
                get_inner_contents(&episode_overrides.inner),
                [(&146065, &1), (&163132, &-12)]
            );
        }

        #[test_case("Mushoku Tensei II", Some(146065) ; "valid key")]
        #[test_case("Horimiya -piece-", Some(163132) ; "also valid key")]
        #[test_case("Mushoku Tensei S2", None ; "invalid key")]
        fn title_override_get(key: &str, result: Option<i32>) {
            let title_override = TitleOverrides {
                inner: HashMap::from([
                    (String::from("Horimiya -piece-"), 163132),
                    (String::from("Mushoku Tensei II"), 146065),
                ]),
            };
            assert_eq!(title_override.get(&String::from(key)), result);
        }

        #[test_case(146065, Some("Mushoku Tensei II") ; "valid key")]
        #[test_case(163132, Some("Horimiya -piece-") ; "also valid key")]
        #[test_case(160188, None ; "invalid key")]
        fn title_override_get_key(value: i32, result: Option<&str>) {
            let title_override = TitleOverrides {
                inner: HashMap::from([
                    (String::from("Horimiya -piece-"), 163132),
                    (String::from("Mushoku Tensei II"), 146065),
                ]),
            };
            assert_eq!(
                title_override.get_key(&value),
                result.map(|x| x.to_string())
            );
        }

        #[test]
        fn title_override_new() {
            let title_override = TitleOverrides::new();
            assert!(title_override.inner.is_empty());
        }

        #[test]
        fn title_override_remove_value() {
            let mut title_override = TitleOverrides {
                inner: HashMap::from([
                    (String::from("Horimiya -piece-"), 163132),
                    (String::from("Mushoku Tensei II"), 146065),
                ]),
            };
            title_override.remove_value(&146065);
            assert_eq!(
                get_inner_contents(&title_override.inner),
                [(&String::from("Horimiya -piece-"), &163132)]
            );
        }

        #[test]
        fn title_override_set() {
            let mut title_override = TitleOverrides {
                inner: HashMap::from([(String::from("Horimiya -piece-"), 163132)]),
            };
            let title = String::from("Mushoku Tensei II");
            title_override.set(title.clone(), 146065);
            assert_eq!(
                get_inner_contents(&title_override.inner),
                [
                    (&String::from("Horimiya -piece-"), &163132),
                    (&title, &146065)
                ]
            );
        }

        #[test]
        fn title_override_set_existing_id() {
            let mut title_override = TitleOverrides {
                inner: HashMap::from([
                    (String::from("Horimiya -piece-"), 163132),
                    (String::from("Mushoku Tensei II"), 146065),
                ]),
            };
            let title = String::from("Mushoku Tensei S2");
            title_override.set(title.clone(), 146065);
            assert_eq!(
                get_inner_contents(&title_override.inner),
                [
                    (&String::from("Horimiya -piece-"), &163132),
                    (&title, &146065),
                ]
            );
        }

        #[test]
        fn title_override_set_existing_title() {
            let title = String::from("Mushoku Tensei II");
            let mut title_override = TitleOverrides {
                inner: HashMap::from([
                    (String::from("Horimiya -piece-"), 163132),
                    (title.clone(), 127720),
                ]),
            };
            title_override.set(title.clone(), 146065);
            assert_eq!(
                get_inner_contents(&title_override.inner),
                [
                    (&String::from("Horimiya -piece-"), &163132),
                    (&title, &146065)
                ]
            );
        }
    }
}
