pub const MEDIALIST_MUTATION: &str = "
mutation($id: Int, $progress: Int) {
  SaveMediaListEntry(id: $id, progress: $progress) {
    progress
  }
}
";

pub const MEDIALIST_QUERY: &str = "
query MediaListCollection($user_id: Int) {
    MediaListCollection(userId: $user_id, status_in: [CURRENT, REPEATING], type: ANIME) {
        lists {
            entries {
                id
                progress
                media {
                    id
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

pub const USER_QUERY: &str = "
query {
    Viewer {
        id
        name
    }
}
";
