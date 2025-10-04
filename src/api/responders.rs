use crate::api::responses;
use rocket::http::Header;
use rocket::serde::json::Json;

#[derive(Responder)]
#[response(status = 400, content_type = "json")]
pub struct ErrorResponder {
    inner: Json<responses::Error>,
}

impl ErrorResponder {
    pub fn with_message(message: String) -> Self {
        let error = responses::Error { error: message };
        let inner = Json(error);
        ErrorResponder { inner }
    }
}

#[derive(Responder)]
#[response(content_type = "json")]
pub struct APIResponse<T> {
    inner: Json<T>,
    allow_origin: Header<'static>,
}

impl<T> APIResponse<T> {
    pub fn new(inner: T) -> Self {
        Self {
            inner: Json(inner),
            allow_origin: Header::new("Access-Control-Allow-Origin", "*"),
        }
    }
}

// #[derive(Responder)]
// struct CorsResponse {
//     inner: (),
//     allow_origin: rocket::http::Header<'static>,
//     allow_methods: rocket::http::Header<'static>,
//     allow_headers: rocket::http::Header<'static>,
// }

// impl CorsResponse {
//     fn new() -> Self {
//         Self {
//             inner: (),
//             allow_origin: rocket::http::Header::new("Access-Control-Allow-Origin", "*"),
//             allow_methods: rocket::http::Header::new(
//                 "Access-Control-Allow-Methods",
//                 "GET, POST, OPTIONS",
//             ),
//             allow_headers: rocket::http::Header::new(
//                 "Access-Control-Allow-Headers",
//                 "Content-Type",
//             ),
//         }
//     }
// }
