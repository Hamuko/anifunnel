use rocket::http::Header;

#[derive(Responder)]
pub struct StaticContent<T> {
    inner: T,
    cache_control: Header<'static>,
}

impl<T> StaticContent<T> {
    pub fn new(inner: T) -> Self {
        Self {
            inner: inner,
            cache_control: Header::new("Cache-Control", "max-age=300"),
        }
    }
}
