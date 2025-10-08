use rocket::http::Header;

pub const STATIC_CONTENT_CACHE_SECONDS: i32 = 300;

#[derive(Responder)]
pub struct StaticContent<T> {
    inner: T,
    cache_control: Header<'static>,
}

impl<T> StaticContent<T> {
    pub fn new(inner: T) -> Self {
        Self {
            inner,
            cache_control: Header::new(
                "Cache-Control",
                format!("max-age={}", STATIC_CONTENT_CACHE_SECONDS),
            ),
        }
    }
}
