use base64::{engine::general_purpose::STANDARD_NO_PAD, Engine as _};
use serde::Deserialize;

type Timestamp = u32;

#[derive(Debug, Deserialize)]
struct JWTPayload {
    exp: Timestamp,
}

#[derive(Debug)]
pub enum TokenParsingError {
    Payload,
    Decode(base64::DecodeError),
    Parse,
}

impl std::fmt::Display for TokenParsingError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Payload => write!(f, "No payload in token"),
            Self::Decode(err) => write!(f, "Decode error: {}", err),
            Self::Parse => write!(f, "Could not parse JWT payload"),
        }
    }
}

fn get_jwt_payload(token: &str) -> Option<&str> {
    // JWT consists of a header, payload, and signature separated by dots, with
    // payload containing the expiration time, so we need to determine where the
    // payload starts and ends and grab that bit.
    let indices: Vec<usize> = token.match_indices(".").map(|(i, _)| i).collect();
    if indices.len() != 2 {
        return None;
    }
    let start = indices.get(0)? + 1;
    let end = indices.get(1)?;
    token.get(start..*end)
}

pub fn get_token_expiry(token: &str) -> Result<Timestamp, TokenParsingError> {
    let payload_slice = get_jwt_payload(token).ok_or(TokenParsingError::Payload)?;
    let decoded = STANDARD_NO_PAD
        .decode(payload_slice)
        .map_err(TokenParsingError::Decode)?;
    let payload =
        serde_json::from_slice::<JWTPayload>(&decoded).map_err(|_| TokenParsingError::Parse)?;
    Ok(payload.exp)
}
