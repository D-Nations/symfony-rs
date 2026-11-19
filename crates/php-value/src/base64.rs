//! `base64_encode()` and strict `base64_decode($data, true)`.
//!
//! Strict decoding fails on any character outside the alphabet, on `=`
//! anywhere but at the end, and on more than two `=`; missing padding is
//! accepted, as PHP accepts it.
//!
//! Upstream: none

const ALPHABET: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";

/// `base64_encode($bytes)`.
pub fn encode(bytes: &[u8]) -> String {
    let mut out = String::with_capacity(bytes.len().div_ceil(3) * 4);
    for chunk in bytes.chunks(3) {
        let b = [
            chunk[0],
            *chunk.get(1).unwrap_or(&0),
            *chunk.get(2).unwrap_or(&0),
        ];
        let n = (u32::from(b[0]) << 16) | (u32::from(b[1]) << 8) | u32::from(b[2]);
        for (i, shift) in [18, 12, 6, 0].into_iter().enumerate() {
            if i <= chunk.len() {
                out.push(char::from(ALPHABET[((n >> shift) & 63) as usize]));
            } else {
                out.push('=');
            }
        }
    }
    out
}

/// `base64_decode($text, true)`; `None` where PHP returns `false`.
pub fn decode_strict(text: &[u8]) -> Option<Vec<u8>> {
    let data_len = text.iter().rposition(|&c| c != b'=').map_or(0, |p| p + 1);
    let padding = text.len() - data_len;
    if padding > 2 {
        return None;
    }
    let mut out = Vec::with_capacity(data_len * 3 / 4);
    let (mut buffer, mut bits) = (0u32, 0);
    for &c in &text[..data_len] {
        let v = ALPHABET.iter().position(|&a| a == c)? as u32;
        buffer = (buffer << 6) | v;
        bits += 6;
        if bits >= 8 {
            bits -= 8;
            out.push((buffer >> bits) as u8);
            buffer &= (1 << bits) - 1;
        }
    }
    Some(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn round_trip() {
        for input in [&b""[..], b"f", b"fo", b"foo", b"foob", b"\xff\x00\x10"] {
            assert_eq!(
                decode_strict(encode(input).as_bytes()).as_deref(),
                Some(input)
            );
        }
        assert_eq!(encode(b"foo"), "Zm9v");
        assert_eq!(decode_strict(b"Zm9v===="), None);
        assert_eq!(decode_strict(b"Zm=9v"), None);
        assert_eq!(decode_strict(b"Zm9"), Some(b"fo".to_vec()));
    }
}
