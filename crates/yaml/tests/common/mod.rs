//! Helpers for reading the recorded vectors.
//!
//! Upstream: none
#![allow(dead_code)]

use serde_json::Value as Json;

pub type TestResult = Result<(), Box<dyn std::error::Error>>;

pub fn load(text: &str) -> Result<Json, Box<dyn std::error::Error>> {
    Ok(serde_json::from_str(text)?)
}

pub fn rows<'a>(json: &'a Json, key: &str) -> Result<&'a Vec<Json>, Box<dyn std::error::Error>> {
    json[key]
        .as_array()
        .ok_or_else(|| format!("no {key}").into())
}

/// Decodes the base64 the recorders write.
pub fn base64(text: &Json) -> Result<Vec<u8>, Box<dyn std::error::Error>> {
    const ALPHABET: &[u8] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let text = text.as_str().ok_or("not base64")?;
    let mut out = Vec::new();
    let (mut buffer, mut bits) = (0u32, 0);
    for c in text.bytes().filter(|&c| c != b'=') {
        let v = ALPHABET.iter().position(|&a| a == c).ok_or("bad base64")? as u32;
        buffer = (buffer << 6) | v;
        bits += 6;
        if bits >= 8 {
            bits -= 8;
            out.push((buffer >> bits) as u8);
            buffer &= (1 << bits) - 1;
        }
    }
    Ok(out)
}

pub mod replay;
