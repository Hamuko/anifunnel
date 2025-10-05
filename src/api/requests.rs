use serde::Deserialize;

#[derive(Debug, Deserialize)]
#[serde(crate = "rocket::serde")]
pub struct Authentication<'r> {
    pub token: &'r str,
}

#[derive(Debug, Deserialize)]
#[serde(crate = "rocket::serde")]
pub struct Override<'r> {
    pub title: Option<&'r str>,
    pub episode_offset: Option<i64>,
}
