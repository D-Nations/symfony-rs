//! Port of Symfony\Component\HttpFoundation\IpUtils.php (v8.1.11):
//! `checkIp()`, `checkIp4()`, `checkIp6()`, `anonymize()`,
//! `isPrivateIp()`, `PRIVATE_SUBNETS` and `PRIVATE_RANGES`.
//!
//! v8.1.11's hardening is ported: `PRIVATE_RANGES` (no prefix a remote
//! client can present as its own) and the IPv4-mapped check, where
//! `::ffff:a.b.c.d` is also matched against IPv4 subnets as that host.
//! The PHP built-ins it calls are `php_value::net`. A netmask is a string
//! compared with ints and used as a number under PHP 8's rules
//! (`php_value::numeric`): `"8.5"` acts as 8, and a leading-numeric `"1x"`
//! passed to `substr_compare()` is a `TypeError`, as in PHP.
//!
//! The results cache (`$checkedIps`, at most about 1,000 entries, least
//! recently used dropped 500 at a time) is per thread; PHP's lives for
//! one request. Since its key joins the two strings with `-`, two calls
//! can share an entry, as in PHP.
//!
//! IPv6 support is always present, so `checkIp6()` never throws PHP's
//! "Unable to check Ipv6" RuntimeException.
//!
//! Symfony source copyright (c) Fabien Potencier; distributed under MIT.
//!
//! Upstream: symfony/http-foundation IpUtils.php

use std::cell::RefCell;
use std::cmp::Ordering;
use std::collections::{BTreeMap, HashMap};

use php_value::Number;
use php_value::net;
use php_value::numeric::{compare_string_with_int, int_argument, leading_numeric_value};

use crate::exception::{Class, Error, Result};

/// `IpUtils::PRIVATE_SUBNETS`: ranges a server must not connect to. A deny
/// list for outgoing requests; not an allow list of trusted proxies.
pub const PRIVATE_SUBNETS: [&str; 24] = [
    "127.0.0.0/8",     // RFC1700 (Loopback)
    "10.0.0.0/8",      // RFC1918
    "192.168.0.0/16",  // RFC1918
    "192.0.2.0/24",    // Documentation Ranges TEST-NET-1 (RFC 5737)
    "198.51.100.0/24", // Documentation Ranges TEST-NET-2 (RFC 5737)
    "203.0.113.0/24",  // Documentation Ranges TEST-NET-3 (RFC 5737)
    "172.16.0.0/12",   // RFC1918
    "169.254.0.0/16",  // RFC3927
    "198.18.0.0/15",   // IPv4 Benchmarking (RFC 2544)
    "0.0.0.0/8",       // RFC5735
    "240.0.0.0/4",     // RFC1112
    "100.64.0.0/10",   // RFC6598
    "::1/128",         // Loopback
    "fc00::/7",        // Unique Local Address
    "fe80::/10",       // Link Local Address
    "::ffff:0:0/96",   // IPv4-mapped IPv6 addresses (RFC 4291 section 2.5.5.2)
    "::/128",          // Unspecified address
    "::/96",           // IPv4-compatible IPv6 addresses (RFC 4291 section 2.5.5.1)
    "2002::/16",       // 6to4 (RFC 3056)
    "2001::/32",       // Teredo tunneling (RFC 4380)
    "2001:db8::/32",   // Documentation Ranges (RFC 3849)
    "2001:0002::/48",  // IPv6 Benchmarking (RFC 5180 and corrections)
    "64:ff9b::/96",    // NAT64 well-known prefix (RFC 6052)
    "64:ff9b:1::/48",  // NAT64 local-use prefix (RFC 8215)
];

/// `IpUtils::PRIVATE_RANGES`: private networks a reverse proxy can live
/// in, usable as an allow list (`Request::setTrustedProxies()`'s
/// `private_ranges`).
pub const PRIVATE_RANGES: [&str; 9] = [
    "127.0.0.0/8",    // RFC1700 (Loopback)
    "10.0.0.0/8",     // RFC1918
    "172.16.0.0/12",  // RFC1918
    "192.168.0.0/16", // RFC1918
    "169.254.0.0/16", // RFC3927
    "100.64.0.0/10",  // RFC6598
    "::1/128",        // Loopback
    "fc00::/7",       // Unique Local Address
    "fe80::/10",      // Link Local Address
];

/// `self::$checkedIps`: insertion order kept by a sequence number, so an
/// entry can move last (LRU) and the oldest 500 can be dropped.
#[derive(Default)]
struct Cache {
    entries: HashMap<String, (u64, bool)>,
    order: BTreeMap<u64, String>,
    next: u64,
}

impl Cache {
    fn get(&mut self, key: &str) -> Option<bool> {
        let (seq, value) = *self.entries.get(key)?;
        self.order.remove(&seq);
        self.push(key.to_owned(), value);
        Some(value)
    }

    fn set(&mut self, key: String, value: bool) -> bool {
        if self.entries.len() > 1000 {
            // array_slice($checkedIps, 500, null, true)
            let dropped: Vec<u64> = self.order.keys().take(500).copied().collect();
            for seq in dropped {
                if let Some(old) = self.order.remove(&seq) {
                    self.entries.remove(&old);
                }
            }
        }
        // Assigning to a key already present keeps its place, as a PHP
        // array does.
        match self.entries.get_mut(&key) {
            Some(entry) => entry.1 = value,
            None => self.push(key, value),
        }
        value
    }

    fn push(&mut self, key: String, value: bool) {
        let seq = self.next;
        self.next += 1;
        self.order.insert(seq, key.clone());
        self.entries.insert(key, (seq, value));
    }
}

thread_local! {
    static CHECKED_IPS: RefCell<Cache> = RefCell::new(Cache::default());
}

fn cached(key: &str) -> Option<bool> {
    CHECKED_IPS.with(|cache| cache.borrow_mut().get(key))
}

fn cache(key: String, value: bool) -> bool {
    CHECKED_IPS.with(|cache| cache.borrow_mut().set(key, value))
}

/// The number of entries in this thread's results cache
/// (`IpUtils::$checkedIps`), for IpUtilsTest's `testCacheSizeLimit`.
#[doc(hidden)]
pub fn checked_ips_count() -> usize {
    CHECKED_IPS.with(|cache| cache.borrow().entries.len())
}

fn value_error(message: impl std::fmt::Display) -> Error {
    Error::new(Class::PhpValueError, message.to_string())
}

/// `IpUtils::checkIp($requestIp, $ips)`; a single subnet is a one-element
/// slice.
pub fn check_ip(request_ip: &str, ips: &[&str]) -> Result<bool> {
    let v6 = request_ip.matches(':').count() > 1;
    let mut embedded_ip4 = None;
    if v6 && net::validate_ip(request_ip.as_bytes(), false, true) {
        let packed = net::inet_pton(request_ip.as_bytes()).map_err(value_error)?;
        if let Some(packed) = packed
            && packed.len() == 16
            && packed[..12] == [0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0xff, 0xff]
        {
            embedded_ip4 = net::inet_ntop(&packed[12..]);
        }
    }
    for ip in ips {
        let matched = if v6 {
            check_ip6(request_ip, ip)?
        } else {
            check_ip4(request_ip, ip)?
        };
        if matched {
            return Ok(true);
        }
        if let Some(embedded) = &embedded_ip4
            && !ip.contains(':')
            && check_ip4(embedded, ip)?
        {
            return Ok(true);
        }
    }
    Ok(false)
}

/// `IpUtils::checkIp4($requestIp, $ip)`.
pub fn check_ip4(request_ip: &str, ip: &str) -> Result<bool> {
    let key = format!("{request_ip}-{ip}-v4");
    if let Some(value) = cached(&key) {
        return Ok(value);
    }
    if !net::validate_ip(request_ip.as_bytes(), true, false) {
        return Ok(cache(key, false));
    }
    let (address, netmask) = match ip.split_once('/') {
        Some((address, netmask)) => {
            if netmask == "0" {
                let valid = net::validate_ip(address.as_bytes(), true, false);
                return Ok(cache(key, valid));
            }
            let netmask = netmask.as_bytes();
            if compare_string_with_int(netmask, 0) == Ordering::Less
                || compare_string_with_int(netmask, 32) == Ordering::Greater
            {
                return Ok(cache(key, false));
            }
            (address, Some(netmask))
        }
        None => (ip, None),
    };
    let Some(long) = net::ip2long(address.as_bytes()).map_err(value_error)? else {
        return Ok(cache(key, false));
    };
    let request_long = net::ip2long(request_ip.as_bytes())
        .map_err(value_error)?
        .unwrap_or(0);
    let length = match netmask {
        None => 32,
        Some(netmask) => int_argument(netmask).ok_or_else(|| {
            Error::new(
                Class::PhpTypeError,
                "substr_compare(): Argument #4 ($length) must be of type ?int, string given",
            )
        })?,
    };
    // substr_compare(sprintf('%032b', ...), sprintf('%032b', ...), 0, $netmask)
    let bits = u32::try_from(length.clamp(0, 32)).unwrap_or(32);
    let shift = 32 - bits;
    let same = bits == 0 || (request_long >> shift) == (long >> shift);
    Ok(cache(key, same))
}

/// `IpUtils::checkIp6($requestIp, $ip)`.
pub fn check_ip6(request_ip: &str, ip: &str) -> Result<bool> {
    let key = format!("{request_ip}-{ip}-v6");
    if let Some(value) = cached(&key) {
        return Ok(value);
    }
    if !net::validate_ip(request_ip.as_bytes(), false, true) {
        return Ok(cache(key, false));
    }
    let (address, netmask) = match ip.split_once('/') {
        Some((address, netmask)) => {
            if !net::validate_ip(address.as_bytes(), false, true) {
                return Ok(cache(key, false));
            }
            if netmask == "0" {
                // (bool) unpack('n*', @inet_pton($address)), not cached
                return Ok(true);
            }
            let bytes = netmask.as_bytes();
            if compare_string_with_int(bytes, 1) == Ordering::Less
                || compare_string_with_int(bytes, 128) == Ordering::Greater
            {
                return Ok(cache(key, false));
            }
            let number = leading_numeric_value(bytes).ok_or_else(|| {
                Error::new(
                    Class::PhpTypeError,
                    "Unsupported operand types: string / int",
                )
            })?;
            (address, number)
        }
        None => {
            if !net::validate_ip(ip.as_bytes(), false, true) {
                return Ok(cache(key, false));
            }
            (ip, Number::Int(128))
        }
    };
    let (Some(bytes_addr), Some(bytes_test)) = (
        net::parse_ipv6(address.as_bytes()),
        net::parse_ipv6(request_ip.as_bytes()),
    ) else {
        return Ok(cache(key, false));
    };
    let word = |b: &[u8; 16], i: usize| u16::from_be_bytes([b[2 * i], b[2 * i + 1]]);
    // for ($i = 1, $ceil = ceil($netmask / 16); $i <= $ceil; ++$i)
    let groups = (netmask.as_f64() / 16.0).ceil();
    let mut i = 1u32;
    while f64::from(i) <= groups && i <= 8 {
        let left = match netmask {
            Number::Int(n) => (n - 16 * i64::from(i - 1)).min(16),
            // a float shift count is truncated toward zero
            Number::Float(n) => (n - 16.0 * f64::from(i - 1)).min(16.0).trunc() as i64,
        };
        let mask = !(0xFFFFu32 >> left.clamp(0, 31)) & 0xFFFF;
        let index = (i - 1) as usize;
        if u32::from(word(&bytes_addr, index)) & mask != u32::from(word(&bytes_test, index)) & mask
        {
            return Ok(cache(key, false));
        }
        i += 1;
    }
    Ok(cache(key, true))
}

/// `IpUtils::anonymize($ip, $v4Bytes, $v6Bytes)`.
pub fn anonymize(ip: &str, v4_bytes: i64, v6_bytes: i64) -> Result<String> {
    if v4_bytes < 0 || v6_bytes < 0 {
        return Err(Error::new(
            Class::PhpInvalidArgument,
            "Cannot anonymize less than 0 bytes.",
        ));
    }
    if v4_bytes > 4 || v6_bytes > 16 {
        return Err(Error::new(
            Class::PhpInvalidArgument,
            "Cannot anonymize more than 4 bytes for IPv4 and 16 bytes for IPv6.",
        ));
    }
    // RFC 4007 scope: only the address before `%` is kept.
    let mut ip = match ip.find('%') {
        Some(at) => &ip[..at],
        None => ip,
    };
    let wrapped = ip.starts_with('[') && ip.ends_with(']') && ip.len() >= 2;
    if wrapped {
        ip = &ip[1..ip.len() - 1];
    }
    // `@` silences inet_pton()'s warning but not its ValueError; `false`
    // then reaches `$packedAddress & inet_pton($mask)`, a TypeError.
    let Some(packed) = net::inet_pton(ip.as_bytes()).map_err(value_error)? else {
        return Err(Error::new(
            Class::PhpTypeError,
            "Unsupported operand types: bool & string",
        ));
    };
    // v4Bytes <= 4 and v6Bytes <= 16 here.
    let (v4, v6) = (v4_bytes as usize, v6_bytes as usize);
    let mapped_mask = |prefix: &str| {
        let hex = format!("{prefix}{}{}", "ff".repeat(4 - v4), "00".repeat(v4));
        let groups: Vec<&str> = (0..hex.len())
            .step_by(4)
            .map(|i| &hex[i..(i + 4).min(hex.len())])
            .collect();
        format!("::{}", groups.join(":"))
    };
    let mask = if packed.len() == 4 {
        let mut parts = vec!["255"; 4 - v4];
        parts.extend(std::iter::repeat_n("0", v4));
        parts.join(".")
    } else if packed.starts_with(&[0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0xff, 0xff]) {
        mapped_mask("ffff")
    } else if packed.starts_with(&[0; 12]) {
        mapped_mask("")
    } else {
        let hex = format!("{}{}", "ff".repeat(16 - v6), "00".repeat(v6));
        let groups: Vec<&str> = (0..32).step_by(4).map(|i| &hex[i..i + 4]).collect();
        groups.join(":")
    };
    let mask = net::inet_pton(mask.as_bytes())
        .map_err(value_error)?
        .unwrap_or_default();
    let anded: Vec<u8> = packed.iter().zip(&mask).map(|(a, b)| a & b).collect();
    let text = net::inet_ntop(&anded).unwrap_or_default();
    Ok(if wrapped { format!("[{text}]") } else { text })
}

/// `IpUtils::isPrivateIp($requestIp)`: whether it lies in
/// `PRIVATE_SUBNETS`; a `ValueError` for a string that is not an IP.
pub fn is_private_ip(request_ip: &str) -> Result<bool> {
    if !net::validate_ip(request_ip.as_bytes(), false, false) {
        return Err(value_error(format!(
            "\"{request_ip}\" is not a valid IP address."
        )));
    }
    check_ip(request_ip, &PRIVATE_SUBNETS)
}
