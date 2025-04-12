use std::collections::BTreeMap;

use acornbencode::common::BencodeValue;
use std::str;

type DecodingError = String;

// Helper functions to extract values from dictionaries
pub fn get_utf8_value(dict: &BTreeMap<&[u8], BencodeValue>, key: &[u8]) -> Result<String, DecodingError> {
    match dict.get(key) {
        Some(BencodeValue::ByteString(s)) => {
            str::from_utf8(s)
                .map(|s| s.to_string())
                .map_err(|_| format!("Field '{}' must be a valid UTF-8 string", str::from_utf8(key).unwrap()))
        },
        None => Err(format!("Missing field '{}'", str::from_utf8(key).unwrap())),
        _ => Err(format!("Field '{}' must be a byte string", str::from_utf8(key).unwrap())),
    }
}

pub fn get_optional_utf8_value(dict: &BTreeMap<&[u8], BencodeValue>, key: &[u8]) -> Result<Option<String>, DecodingError> {
    match dict.get(key) {
        Some(BencodeValue::ByteString(s)) => {
            str::from_utf8(s)
                .map(|s| s.to_string())
                .map_err(|_| format!("Field '{}' must be a valid UTF-8 string", str::from_utf8(key).unwrap()))
                .map(|s| Some(s))
        },
        _ => Ok(None),
    }
}
