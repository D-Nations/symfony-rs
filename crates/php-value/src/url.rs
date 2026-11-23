//! `urlencode()`, `rawurlencode()`, `urldecode()` and `rawurldecode()` on
//! bytes, written from the PHP manual: `rawurlencode()` keeps
//! `A-Z a-z 0-9 - _ . ~` and writes every other byte as `%XX` (upper-case
//! hex, RFC 3986); `urlencode()` keeps `A-Z a-z 0-9 - _ .`, writes a space
//! as `+` and every other byte as `%XX`. Decoding turns `%` and two hex
//! digits (either case) into that byte and leaves any other `%` as it is;
//! `urldecode()` also reads `+` as a space. Checked against PHP 8.4.27 by
//! crates/http-foundation's request vectors and the Cookie vectors.
//!
//! Upstream: none

use crate::array::{Key, PhpArray};
use crate::value::Value;

const HEX: &[u8; 16] = b"0123456789ABCDEF";

fn encode(bytes: &[u8], raw: bool) -> Vec<u8> {
    let mut out = Vec::with_capacity(bytes.len());
    for &byte in bytes {
        match byte {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' => out.push(byte),
            b'~' if raw => out.push(byte),
            b' ' if !raw => out.push(b'+'),
            _ => out.extend([
                b'%',
                HEX[usize::from(byte >> 4)],
                HEX[usize::from(byte & 0x0f)],
            ]),
        }
    }
    out
}

/// `rawurlencode($bytes)`.
pub fn rawurlencode(bytes: &[u8]) -> Vec<u8> {
    encode(bytes, true)
}

/// `urlencode($bytes)`.
pub fn urlencode(bytes: &[u8]) -> Vec<u8> {
    encode(bytes, false)
}

fn hex(digit: u8) -> Option<u8> {
    match digit {
        b'0'..=b'9' => Some(digit - b'0'),
        b'a'..=b'f' => Some(digit - b'a' + 10),
        b'A'..=b'F' => Some(digit - b'A' + 10),
        _ => None,
    }
}

fn decode(text: &[u8], plus_is_space: bool) -> Vec<u8> {
    let mut out = Vec::with_capacity(text.len());
    let mut at = 0;
    while let Some(&byte) = text.get(at) {
        let escaped = match (byte, text.get(at + 1..at + 3)) {
            (b'%', Some(&[high, low])) => hex(high).zip(hex(low)).map(|(h, l)| h << 4 | l),
            _ => None,
        };
        match escaped {
            Some(decoded) => {
                out.push(decoded);
                at += 3;
            }
            None => {
                out.push(if plus_is_space && byte == b'+' {
                    b' '
                } else {
                    byte
                });
                at += 1;
            }
        }
    }
    out
}

/// `http_build_query($data, '', $separator)` (`PHP_QUERY_RFC1738`), per
/// the PHP manual: `name=value` pairs, `urlencode()`d, joined by
/// `separator`; a nested array's entries are named `name[key]` with the
/// brackets encoded (`%5B`, `%5D`); `null` entries are skipped, `true` is
/// `1`, `false` is `0`. `None` for a float or an object entry, which this
/// port does not reproduce.
pub fn http_build_query(data: &PhpArray, separator: &[u8]) -> Option<Vec<u8>> {
    let mut pairs: Vec<Vec<u8>> = Vec::new();
    for (key, value) in data.iter() {
        let name = urlencode(&key_bytes(key));
        build_pairs(&mut pairs, name, value)?;
    }
    Some(pairs.join(separator))
}

fn key_bytes(key: &Key) -> Vec<u8> {
    match key {
        Key::Int(i) => i.to_string().into_bytes(),
        Key::Str(s) => s.as_bytes().to_vec(),
    }
}

fn build_pairs(pairs: &mut Vec<Vec<u8>>, name: Vec<u8>, value: &Value) -> Option<()> {
    let text: Vec<u8> = match value {
        Value::Null => return Some(()),
        Value::Bool(true) => b"1".to_vec(),
        Value::Bool(false) => b"0".to_vec(),
        Value::Int(i) => i.to_string().into_bytes(),
        Value::String(s) => urlencode(s.as_bytes()),
        Value::Array(children) => {
            for (key, child) in children.iter() {
                let mut child_name = name.clone();
                child_name.extend_from_slice(b"%5B");
                child_name.extend(urlencode(&key_bytes(key)));
                child_name.extend_from_slice(b"%5D");
                build_pairs(pairs, child_name, child)?;
            }
            return Some(());
        }
        _ => return None,
    };
    let mut pair = name;
    pair.push(b'=');
    pair.extend(text);
    pairs.push(pair);
    Some(())
}

/// `bin2hex($bytes)`: lower-case hex digits.
pub fn bin2hex(bytes: &[u8]) -> Vec<u8> {
    const LOWER: &[u8; 16] = b"0123456789abcdef";
    bytes
        .iter()
        .flat_map(|&b| [LOWER[usize::from(b >> 4)], LOWER[usize::from(b & 0x0f)]])
        .collect()
}

/// `hex2bin($text)`: `None` (PHP's `false`, with a warning) for an odd
/// length or a non-hex digit.
pub fn hex2bin(text: &[u8]) -> Option<Vec<u8>> {
    if !text.len().is_multiple_of(2) {
        return None;
    }
    text.chunks(2)
        .map(|pair| Some((hex(pair[0])? << 4) | hex(pair[1])?))
        .collect()
}

/// `urldecode($text)`.
pub fn urldecode(text: &[u8]) -> Vec<u8> {
    decode(text, true)
}

/// `rawurldecode($text)`.
pub fn rawurldecode(text: &[u8]) -> Vec<u8> {
    decode(text, false)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn round_trips() {
        assert_eq!(rawurlencode(b"b a+r~"), b"b%20a%2Br~");
        assert_eq!(urlencode(b"b a+r~"), b"b+a%2Br%7E");
        assert_eq!(urldecode(b"a+b%20c%zz%4"), b"a b c%zz%4");
        assert_eq!(rawurldecode(b"a+b%2b"), b"a+b+");
    }
}
