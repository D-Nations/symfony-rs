//! PHP's IP address functions, written from the PHP manual and the answers
//! recorded from PHP 8.4.27 on the oracle's glibc
//! (`oracle/recorders/http_foundation_ip_utils.php` ->
//! crates/http-foundation/tests/data/ip_utils.json):
//!
//! - `filter_var($s, FILTER_VALIDATE_IP, ...)`, `inet_pton()` and
//!   `ip2long()` accept the same addresses. IPv4 is four decimal parts
//!   from 0 to 255, without a leading zero (`010.0.0.1` fails), sign or
//!   surrounding space. IPv6 is eight groups of one to four hex digits,
//!   the last two of which may be an IPv4 address as above; one `::`
//!   stands for one or more zero groups (`1:2:3:4:5:6::7:8` fails). No
//!   brackets, zone (`%eth0`) or space.
//! - `inet_pton()` and `ip2long()` throw a `ValueError` for a NUL byte
//!   (`@` does not silence it); `filter_var()` just fails.
//! - `inet_ntop()` gives dotted IPv4 for four bytes. For sixteen it writes
//!   lowercase groups without leading zeros, replaces the first longest
//!   run of two or more zero groups with `::`, and writes the last four
//!   bytes as IPv4 when the first six groups are zero (`::1.2.3.4`, but
//!   `::1` and `::`) or the first five are and the sixth is `ffff`. Any
//!   other length is `false`.
//!
//! Upstream: none

use std::fmt::Write as _;
/// `inet_pton()` or `ip2long()` was given a NUL byte: PHP's `ValueError`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NulByte {
    function: &'static str,
}

impl std::fmt::Display for NulByte {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "{}(): Argument #1 ($ip) must not contain any null bytes",
            self.function
        )
    }
}

impl std::error::Error for NulByte {}

/// A dotted IPv4 address, as all three functions read it.
pub fn parse_ipv4(s: &[u8]) -> Option<[u8; 4]> {
    let mut out = [0u8; 4];
    let mut parts = s.split(|&b| b == b'.');
    for byte in &mut out {
        let part = parts.next()?;
        if part.is_empty()
            || part.len() > 3
            || !part.iter().all(u8::is_ascii_digit)
            || (part.len() > 1 && part[0] == b'0')
        {
            return None;
        }
        let value = part.iter().fold(0u16, |v, &d| v * 10 + u16::from(d - b'0'));
        *byte = u8::try_from(value).ok()?;
    }
    if parts.next().is_some() {
        return None;
    }
    Some(out)
}

fn hex_group(group: &[u8]) -> Option<u16> {
    if group.is_empty() || group.len() > 4 || !group.iter().all(u8::is_ascii_hexdigit) {
        return None;
    }
    let text = std::str::from_utf8(group).ok()?;
    u16::from_str_radix(text, 16).ok()
}

/// Groups separated by `:`, the last one possibly IPv4 (as two groups).
fn groups(s: &[u8], ipv4_tail: bool) -> Option<Vec<u16>> {
    let mut out = Vec::new();
    if s.is_empty() {
        return Some(out);
    }
    let parts: Vec<&[u8]> = s.split(|&b| b == b':').collect();
    for (i, part) in parts.iter().enumerate() {
        if ipv4_tail && i + 1 == parts.len() && part.contains(&b'.') {
            let v4 = parse_ipv4(part)?;
            out.push(u16::from_be_bytes([v4[0], v4[1]]));
            out.push(u16::from_be_bytes([v4[2], v4[3]]));
        } else {
            out.push(hex_group(part)?);
        }
    }
    Some(out)
}

/// An IPv6 address in text form, as all three functions read it.
pub fn parse_ipv6(s: &[u8]) -> Option<[u8; 16]> {
    let words = if let Some(at) = s.windows(2).position(|w| w == b"::") {
        let (left, right) = (&s[..at], &s[at + 2..]);
        let left = groups(left, false)?;
        let right = groups(right, true)?;
        if left.len() + right.len() > 7 {
            return None;
        }
        let mut words = left;
        words.resize(8 - right.len(), 0);
        words.extend(right);
        words
    } else {
        let words = groups(s, true)?;
        if words.len() != 8 {
            return None;
        }
        words
    };
    let mut out = [0u8; 16];
    for (i, word) in words.iter().enumerate() {
        out[2 * i..2 * i + 2].copy_from_slice(&word.to_be_bytes());
    }
    Some(out)
}

/// `false !== filter_var($s, FILTER_VALIDATE_IP, $flags)` with
/// `FILTER_FLAG_IPV4` (`v4`), `FILTER_FLAG_IPV6` (`v6`), both or neither
/// (either family).
pub fn validate_ip(s: &[u8], v4: bool, v6: bool) -> bool {
    let (v4, v6) = if v4 || v6 { (v4, v6) } else { (true, true) };
    (v4 && parse_ipv4(s).is_some()) || (v6 && parse_ipv6(s).is_some())
}

/// `@inet_pton($s)`: the packed address, `None` for `false`.
pub fn inet_pton(s: &[u8]) -> Result<Option<Vec<u8>>, NulByte> {
    if s.contains(&0) {
        return Err(NulByte {
            function: "inet_pton",
        });
    }
    if let Some(v4) = parse_ipv4(s) {
        return Ok(Some(v4.to_vec()));
    }
    Ok(parse_ipv6(s).map(|v6| v6.to_vec()))
}

/// `ip2long($s)`: `None` for `false`.
pub fn ip2long(s: &[u8]) -> Result<Option<i64>, NulByte> {
    if s.contains(&0) {
        return Err(NulByte {
            function: "ip2long",
        });
    }
    Ok(parse_ipv4(s).map(|b| i64::from(u32::from_be_bytes(b))))
}

/// `@inet_ntop($packed)`: `None` for `false`.
pub fn inet_ntop(packed: &[u8]) -> Option<String> {
    if let Ok(v4) = <[u8; 4]>::try_from(packed) {
        return Some(dotted(v4));
    }
    let bytes = <[u8; 16]>::try_from(packed).ok()?;
    let words: Vec<u16> = bytes
        .chunks(2)
        .map(|c| u16::from_be_bytes([c[0], c[1]]))
        .collect();
    // The first longest run of at least two zero groups.
    let (mut best, mut run) = ((0usize, 0usize), None::<usize>);
    for (i, &word) in words.iter().enumerate() {
        if word == 0 {
            let start = *run.get_or_insert(i);
            if i + 1 - start > best.1 {
                best = (start, i + 1 - start);
            }
        } else {
            run = None;
        }
    }
    let best = (best.1 >= 2).then_some(best);
    let mut out = String::new();
    let mut i = 0;
    while i < 8 {
        if let Some((start, len)) = best
            && i == start
        {
            out.push(':');
            if start + len == 8 {
                out.push(':');
            }
            i += len;
            continue;
        }
        if i > 0 {
            out.push(':');
        }
        if i == 6
            && let Some((0, len)) = best
            && (len == 6 || (len == 5 && words[5] == 0xffff))
        {
            out.push_str(&dotted([bytes[12], bytes[13], bytes[14], bytes[15]]));
            break;
        }
        let _ = write!(out, "{:x}", words[i]);
        i += 1;
    }
    Some(out)
}

fn dotted(b: [u8; 4]) -> String {
    format!("{}.{}.{}.{}", b[0], b[1], b[2], b[3])
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ntop_forms() {
        let ntop = |text: &str| inet_ntop(&parse_ipv6(text.as_bytes()).unwrap()).unwrap();
        assert_eq!("::", ntop("::"));
        assert_eq!("::1", ntop("::1"));
        assert_eq!("::1.2.3.4", ntop("::1.2.3.4"));
        assert_eq!("::ffff:1.2.3.4", ntop("::ffff:1.2.3.4"));
        assert_eq!("1::", ntop("1::"));
        assert_eq!("1:0:1:0:1:0:1:0", ntop("1:0:1:0:1:0:1:0"));
        assert_eq!("2a01:198:603::", ntop("2a01:198:603:0::"));
    }
}
