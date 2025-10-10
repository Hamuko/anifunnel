use base64::{engine::general_purpose::STANDARD_NO_PAD, Engine as _};
use regex::Regex;
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
    let start = indices.first()? + 1;
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

/// Remove parts of a given string using a collection of regular expressions.
pub fn remove_regexes(regexes: &[Regex], string: &str) -> String {
    regexes.iter().fold(string.to_owned(), |s, regex| {
        regex.replace(&s, "").to_string()
    })
}

pub fn remove_special_surrounding_characters(value: &str) -> &str {
    let mut start_pos = 0;
    let mut end_pos = 0;
    for (pos, chr) in value.char_indices() {
        start_pos = pos;
        if chr.is_alphanumeric() || chr == '(' {
            break;
        }
    }
    for (pos, chr) in value.char_indices().rev() {
        end_pos = pos;
        if chr.is_alphanumeric() || chr == ')' {
            break;
        }
    }
    while !value.is_char_boundary(end_pos + 1) {
        end_pos += 1;
    }
    &value[start_pos..=end_pos]
}

#[cfg(test)]
mod tests {
    use super::*;

    use test_case::test_case;

    #[test]
    fn token_expiry() {
        let token = "eyJ0eXAiOiJKV1QiLCJhbGciOiJFUzI1NiIsImtpZCI6ImU1MzBkOGU4YjcyYTAyZDA\
            4ZGUyZTdiNzdkODUzYzA4In0.eyJleHAiOjE3Nzk3NDI4MDB9.FWpXxOu12akm7b1DK\
            rzgeK33Qnl_PRpy67VXuz6qd1ezLOF4CbwFlT2o4rMGW7JXsgP0PbhdMVtGFRnnjHWhrg";
        let expiry = get_token_expiry(token);
        assert_eq!(expiry.unwrap(), 1779742800);
    }

    #[test]
    fn token_expiry_no_exp() {
        let token = "eyJ0eXAiOiJKV1QiLCJhbGciOiJFUzI1NiIsImtpZCI6ImU1MzBkOGU4Yjcy\
            YTAyZDA4ZGUyZTdiNzdkODUzYzA4In0.e30.axrvIXGrV_LI9PMUnv8th66U\
            e0UGv2lsb6r9U5IY-S7hJBwQMGCjEtGWE9p53ms-w6sVC4SRagwSsU_mUt4-KQ";
        let expiry = get_token_expiry(token);
        assert!(matches!(expiry, Err(TokenParsingError::Parse)))
    }

    #[test]
    fn token_expiry_not_jwt() {
        let token = "thisis.notjwt";
        let expiry = get_token_expiry(token);
        assert!(matches!(expiry, Err(TokenParsingError::Payload)))
    }

    #[test]
    fn token_expiry_undecodable() {
        let token = "youcant.decode.thispayload";
        let expiry = get_token_expiry(token);
        assert!(matches!(expiry, Err(TokenParsingError::Decode(_))))
    }

    #[test]
    // Test that remove_regexes() removes given regex patterns from a string.
    fn regex_removal() {
        let regexes = [
            Regex::new(r"\([A-z]+\) ").unwrap(),
            Regex::new(r"([0-9]+)?(1st|2nd|3rd|[4-90]th) ").unwrap(),
            Regex::new(r"\.+$").unwrap(),
        ];
        let input = String::from("This is (arguably) the day of the 21st century.");
        let output = remove_regexes(&regexes, &input);
        assert_eq!(output, "This is the day of the century");
    }

    #[test_case("(Oshi no Ko)", "(Oshi no Ko)" ; "surrounding parentheses")]
    #[test_case("2.5 Jigen no Ririsa", "2.5 Jigen no Ririsa" ; "leading numbers")]
    #[test_case("[Oshi no Ko]", "Oshi no Ko" ; "surrounding brackets")]
    #[test_case("\"Oshi no Ko\"", "Oshi no Ko" ; "surrounding quotes")]
    #[test_case("Anne Happy♪", "Anne Happy" ; "trailing note")]
    #[test_case("Black★Rock Shooter", "Black★Rock Shooter" ; "special character between")]
    #[test_case("Girlfriend (Kari)", "Girlfriend (Kari)" ; "trailing parenthesis")]
    #[test_case("らき☆すた", "らき☆すた" ; "special character between Japanese")]
    #[test_case("【推しの子】", "推しの子" ; "surrounding quotes Japanese")]
    fn special_surrounding_characters_removal(input: &str, expected: &str) {
        let output = remove_special_surrounding_characters(&input);
        assert_eq!(output, expected);
    }
}
