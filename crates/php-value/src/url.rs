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
