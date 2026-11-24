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
use crate::numeric::float_to_string;
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

/// `http_build_query()`'s `$encoding_type`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum QueryEncoding {
    /// `PHP_QUERY_RFC1738` (the default): `urlencode()`, a space is `+`.
    Rfc1738,
    /// `PHP_QUERY_RFC3986`: `rawurlencode()`, a space is `%20`.
    Rfc3986,
}

impl QueryEncoding {
    fn encode(self, bytes: &[u8]) -> Vec<u8> {
        encode(bytes, self == Self::Rfc3986)
    }
}

/// `http_build_query($data, '', $separator)` (`PHP_QUERY_RFC1738`).
pub fn http_build_query(data: &PhpArray, separator: &[u8]) -> Option<Vec<u8>> {
    http_build_query_with(data, separator, QueryEncoding::Rfc1738)
}

/// `http_build_query($data, '', $separator, $encoding)`, per the PHP
/// manual: `name=value` pairs, encoded, joined by `separator`; a nested
/// array's entries are named `name[key]` with the brackets encoded (`%5B`,
/// `%5D`); `null` entries and empty arrays give no pair, `true` is `1`,
/// `false` is `0`, a float is its `(string)` form (`1.0E+20`, `NAN`).
/// `None` for an object entry, which this port does not reproduce. Checked
/// against PHP 8.4.27 (oracle/recorders/php_value_http_build_query.php).
pub fn http_build_query_with(
    data: &PhpArray,
    separator: &[u8],
    encoding: QueryEncoding,
) -> Option<Vec<u8>> {
    let mut pairs: Vec<Vec<u8>> = Vec::new();
    for (key, value) in data.iter() {
        let name = encoding.encode(&key_bytes(key));
        build_pairs(&mut pairs, name, value, encoding)?;
    }
    Some(pairs.join(separator))
}

fn key_bytes(key: &Key) -> Vec<u8> {
    match key {
        Key::Int(i) => i.to_string().into_bytes(),
        Key::Str(s) => s.as_bytes().to_vec(),
    }
}

fn build_pairs(
    pairs: &mut Vec<Vec<u8>>,
    name: Vec<u8>,
    value: &Value,
    encoding: QueryEncoding,
) -> Option<()> {
    let text: Vec<u8> = match value {
        Value::Null => return Some(()),
        Value::Bool(true) => b"1".to_vec(),
        Value::Bool(false) => b"0".to_vec(),
        Value::Int(i) => i.to_string().into_bytes(),
        Value::Float(f) => encoding.encode(float_to_string(*f).as_bytes()),
        Value::String(s) => encoding.encode(s.as_bytes()),
        Value::Array(children) => {
            for (key, child) in children.iter() {
                let mut child_name = name.clone();
                child_name.extend_from_slice(b"%5B");
                child_name.extend(encoding.encode(&key_bytes(key)));
                child_name.extend_from_slice(b"%5D");
                build_pairs(pairs, child_name, child, encoding)?;
            }
            return Some(());
        }
        Value::Object(_) => return None,
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

/// What `parse_url($url)` returns when it does not return `false`: the
/// components it found, each a byte string except the port.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Url {
    pub scheme: Option<Vec<u8>>,
    pub host: Option<Vec<u8>>,
    pub port: Option<i64>,
    pub user: Option<Vec<u8>>,
    pub pass: Option<Vec<u8>>,
    pub path: Option<Vec<u8>>,
    pub query: Option<Vec<u8>>,
    pub fragment: Option<Vec<u8>>,
}

impl Url {
    /// The array `parse_url()` returns, in its key order.
    pub fn to_array(&self) -> PhpArray {
        let mut out = PhpArray::new();
        let string = |out: &mut PhpArray, key: &str, v: &Option<Vec<u8>>| {
            if let Some(v) = v {
                out.insert(key, Value::String(v.as_slice().into()));
            }
        };
        string(&mut out, "scheme", &self.scheme);
        string(&mut out, "host", &self.host);
        if let Some(port) = self.port {
            out.insert("port", Value::Int(port));
        }
        string(&mut out, "user", &self.user);
        string(&mut out, "pass", &self.pass);
        string(&mut out, "path", &self.path);
        string(&mut out, "query", &self.query);
        string(&mut out, "fragment", &self.fragment);
        out
    }
}

/// The first index of any byte of `set` in `s`, or its length.
fn first_of(s: &[u8], set: &[u8]) -> usize {
    s.iter().position(|c| set.contains(c)).unwrap_or(s.len())
}

/// One to five digits, then the end or a `/`: what makes `a:80` a host
/// and port rather than a scheme and path.
fn port_like(t: &[u8]) -> bool {
    let digits = t.iter().take_while(|c| c.is_ascii_digit()).count();
    (1..=5).contains(&digits) && t.get(digits).is_none_or(|&c| c == b'/')
}

/// A port's text: `Ok(None)` when empty, `Err` where PHP returns `false`
/// (more than five bytes, no digits after optional spaces and a sign, or
/// outside 0 to 65535). Text after the digits is ignored.
fn parse_port(t: &[u8]) -> Result<Option<i64>, ()> {
    if t.is_empty() {
        return Ok(None);
    }
    if t.len() > 5 {
        return Err(());
    }
    let mut i = t
        .iter()
        .take_while(|c| matches!(c, b' ' | b'\t' | b'\n' | 0x0b | 0x0c | b'\r'))
        .count();
    let negative = match t.get(i) {
        Some(b'-') => {
            i += 1;
            true
        }
        Some(b'+') => {
            i += 1;
            false
        }
        _ => false,
    };
    let digits = t[i..].iter().take_while(|c| c.is_ascii_digit()).count();
    if digits == 0 {
        return Err(());
    }
    let mut value: i64 = 0;
    for d in &t[i..i + digits] {
        value = value * 10 + i64::from(d - b'0');
    }
    if negative {
        value = -value;
    }
    if (0..=65535).contains(&value) {
        Ok(Some(value))
    } else {
        Err(())
    }
}

/// Takes `user[:pass]@` off the front of an authority's host part.
fn user_info(url: &mut Url, host: &[u8]) -> Vec<u8> {
    let Some(at) = host.iter().rposition(|&c| c == b'@') else {
        return host.to_vec();
    };
    let info = &host[..at];
    match info.iter().position(|&c| c == b':') {
        Some(k) => {
            url.user = Some(info[..k].to_vec());
            url.pass = Some(info[k + 1..].to_vec());
        }
        None => url.user = Some(info.to_vec()),
    }
    host[at + 1..].to_vec()
}

/// The path, `?query` and `#fragment`: a path is kept when not empty.
fn path_query_fragment(url: &mut Url, t: &[u8]) {
    let mut t = t;
    if let Some(h) = t.iter().position(|&c| c == b'#') {
        url.fragment = Some(t[h + 1..].to_vec());
        t = &t[..h];
    }
    if let Some(q) = t.iter().position(|&c| c == b'?') {
        url.query = Some(t[q + 1..].to_vec());
        t = &t[..q];
    }
    if !t.is_empty() {
        url.path = Some(t.to_vec());
    }
}

/// After `//`: `[user[:pass]@]host[:port]`, up to the first `/`, `?` or
/// `#`, then the rest.
fn authority(mut url: Url, a: &[u8]) -> Option<Url> {
    let e = first_of(a, b"/?#");
    let auth = user_info(&mut url, &a[..e]);
    let bracketed = auth.first() == Some(&b'[') && auth.last() == Some(&b']');
    let host = match auth.iter().rposition(|&c| c == b':') {
        Some(c) if !bracketed => {
            url.port = parse_port(&auth[c + 1..]).ok()?;
            auth[..c].to_vec()
        }
        _ => auth,
    };
    if host.is_empty() {
        return None;
    }
    url.host = Some(host);
    path_query_fragment(&mut url, &a[e..]);
    Some(url)
}

/// `host:port` without `//` (`localhost:80/x`): the host from the start
/// up to the colon at `c` or the first `/`, `?` or `#`, the port from the
/// digits after that colon, then the rest from that first `/`, `?` or `#`.
fn host_port(s: &[u8], c: usize) -> Option<Url> {
    let mut url = Url::default();
    let e = first_of(s, b"/?#");
    let host = user_info(&mut url, &s[..c.min(e)]);
    let digits = s[c + 1..].iter().take_while(|d| d.is_ascii_digit()).count();
    url.port = parse_port(&s[c + 1..c + 1 + digits]).ok()?;
    if host.is_empty() {
        return None;
    }
    url.host = Some(host);
    path_query_fragment(&mut url, &s[e..]);
    Some(url)
}

/// Control bytes in a component read as `_`.
fn clean(v: &mut Option<Vec<u8>>) {
    for c in v.iter_mut().flatten() {
        if *c < 0x20 || *c == 0x7f {
            *c = b'_';
        }
    }
}

/// `parse_url($url)`: `None` where PHP returns `false`. Written from the
/// PHP manual and 12,119 answers recorded from PHP 8.4.27
/// (tests/data/parse_url.json), which fixed the rules the manual leaves
/// open: a scheme is `[A-Za-z0-9+.-]+` before `:`, unless one to five
/// digits and then the end or `/` follow, which makes it `host:port`; in a
/// string without a scheme, a colon before any `?` or `#` followed by such
/// digits makes it `host:port` too; `file:///` keeps the third slash in
/// the path; the port is the text after the authority's last colon (not
/// inside a `[...]` host), at most five bytes, read like `strtol()`; an
/// empty host is `false`; control bytes become `_`.
pub fn parse_url(s: &[u8]) -> Option<Url> {
    let p = s
        .iter()
        .take_while(|c| c.is_ascii_alphanumeric() || matches!(c, b'+' | b'-' | b'.'))
        .count();
    let mut url = if s.get(p) == Some(&b':') {
        let rest = &s[p + 1..];
        if p == 0 {
            // a host:port with an empty host
            if rest.is_empty() || port_like(rest) {
                return None;
            }
            let mut url = Url::default();
            path_query_fragment(&mut url, s);
            url
        } else if port_like(rest) {
            host_port(s, p)?
        } else {
            let mut url = Url {
                scheme: Some(s[..p].to_vec()),
                ..Url::default()
            };
            if rest.starts_with(b"///") && s[..p].eq_ignore_ascii_case(b"file") {
                path_query_fragment(&mut url, &rest[2..]);
                url
            } else if let Some(a) = rest.strip_prefix(b"//") {
                authority(url, a)?
            } else {
                path_query_fragment(&mut url, rest);
                url
            }
        }
    } else if let Some(a) = s.strip_prefix(b"//") {
        authority(Url::default(), a)?
    } else {
        let stop = first_of(s, b"?#");
        match s.iter().position(|&c| c == b':') {
            Some(c) if c < stop && port_like(&s[c + 1..]) => host_port(s, c)?,
            _ => {
                let mut url = Url::default();
                path_query_fragment(&mut url, s);
                if s.is_empty() {
                    url.path = Some(Vec::new());
                }
                url
            }
        }
    };
    for v in [
        &mut url.scheme,
        &mut url.host,
        &mut url.user,
        &mut url.pass,
        &mut url.path,
        &mut url.query,
        &mut url.fragment,
    ] {
        clean(v);
    }
    Some(url)
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
