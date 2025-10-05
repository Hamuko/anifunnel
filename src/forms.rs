#[derive(Debug, FromForm)]
pub struct Scrobble<'r> {
    pub payload: &'r str,
}
