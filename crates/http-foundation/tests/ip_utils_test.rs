//! IpUtilsTest.php at symfony/http-foundation
//! 30fa3efe6e736b0d06f5331cc6abe813582c0a26 (v8.1.11). Names and
//! assertions are PHPUnit's; a provider's cases run in a loop.
//! `testAnIpv6WithOptionDisabledIpv6` is `n/a`: IPv6 support is always
//! present here, and PHP skips it too unless built with `disable-ipv6`.
//!
//! Upstream: symfony/http-foundation Tests/IpUtilsTest.php
#![allow(non_snake_case)]

use symfony_http_foundation::ip_utils::{
    PRIVATE_RANGES, PRIVATE_SUBNETS, anonymize, check_ip, check_ip4, check_ip6, checked_ips_count,
    is_private_ip,
};

type TestResult = Result<(), Box<dyn std::error::Error>>;

#[test]
fn testSeparateCachesPerProtocol() -> TestResult {
    let ip = "192.168.52.1";
    let subnet = "192.168.0.0/16";

    assert!(!check_ip6(ip, subnet)?);
    assert!(check_ip4(ip, subnet)?);

    let ip = "2a01:198:603:0:396e:4789:8e99:890f";
    let subnet = "2a01:198:603:0::/65";

    assert!(!check_ip4(ip, subnet)?);
    assert!(check_ip6(ip, subnet)?);
    Ok(())
}

fn get_ipv4_data() -> Vec<(bool, &'static str, Vec<&'static str>)> {
    vec![
        (true, "192.168.1.1", vec!["192.168.1.1"]),
        (true, "192.168.1.1", vec!["192.168.1.1/1"]),
        (true, "192.168.1.1", vec!["192.168.1.0/24"]),
        (false, "192.168.1.1", vec!["1.2.3.4/1"]),
        (false, "192.168.1.1", vec!["192.168.1.1/33"]), // invalid subnet
        (true, "192.168.1.1", vec!["1.2.3.4/1", "192.168.1.0/24"]),
        (true, "192.168.1.1", vec!["192.168.1.0/24", "1.2.3.4/1"]),
        (false, "192.168.1.1", vec!["1.2.3.4/1", "4.3.2.1/1"]),
        (true, "1.2.3.4", vec!["0.0.0.0/0"]),
        (true, "1.2.3.4", vec!["192.168.1.0/0"]),
        (false, "1.2.3.4", vec!["256.256.256/0"]), // invalid CIDR notation
        (false, "an_invalid_ip", vec!["192.168.1.0/24"]),
        (false, "", vec!["1.2.3.4/1"]),
    ]
}

#[test]
fn testIpv4() -> TestResult {
    for (matches, remote_addr, cidr) in get_ipv4_data() {
        assert_eq!(
            matches,
            check_ip(remote_addr, &cidr)?,
            "{remote_addr} {cidr:?}"
        );
    }
    Ok(())
}

fn get_ipv6_data() -> Vec<(bool, &'static str, Vec<&'static str>)> {
    vec![
        (
            true,
            "2a01:198:603:0:396e:4789:8e99:890f",
            vec!["2a01:198:603:0::/65"],
        ),
        (
            false,
            "2a00:198:603:0:396e:4789:8e99:890f",
            vec!["2a01:198:603:0::/65"],
        ),
        (false, "2a01:198:603:0:396e:4789:8e99:890f", vec!["::1"]),
        (true, "0:0:0:0:0:0:0:1", vec!["::1"]),
        (false, "0:0:603:0:396e:4789:8e99:0001", vec!["::1"]),
        (true, "0:0:603:0:396e:4789:8e99:0001", vec!["::/0"]),
        (
            true,
            "0:0:603:0:396e:4789:8e99:0001",
            vec!["2a01:198:603:0::/0"],
        ),
        (
            true,
            "2a01:198:603:0:396e:4789:8e99:890f",
            vec!["::1", "2a01:198:603:0::/65"],
        ),
        (
            true,
            "2a01:198:603:0:396e:4789:8e99:890f",
            vec!["2a01:198:603:0::/65", "::1"],
        ),
        (
            false,
            "2a01:198:603:0:396e:4789:8e99:890f",
            vec!["::1", "1a01:198:603:0::/65"],
        ),
        (
            false,
            "}__test|O:21:&quot;JDatabaseDriverMysqli&quot;:3:{s:2",
            vec!["::1"],
        ),
        (false, "2a01:198:603:0:396e:4789:8e99:890f", vec!["unknown"]),
        (false, "", vec!["::1"]),
        (false, "::1\0", vec!["::1/128"]),
        (false, "127.0.0.1", vec!["::1"]),
        (false, "0.0.0.0/8", vec!["::1"]),
        (false, "::1", vec!["127.0.0.1"]),
        (false, "::1", vec!["0.0.0.0/8"]),
        (true, "::ffff:10.126.42.2", vec!["::ffff:10.0.0.0/0"]),
    ]
}

#[test]
fn testIpv6() -> TestResult {
    for (matches, remote_addr, cidr) in get_ipv6_data() {
        assert_eq!(
            matches,
            check_ip(remote_addr, &cidr)?,
            "{remote_addr:?} {cidr:?}"
        );
    }
    Ok(())
}

fn invalid_ip_address_data() -> Vec<(&'static str, &'static str, &'static str)> {
    vec![
        ("invalid proxy wildcard", "192.168.20.13", "*"),
        ("invalid proxy missing netmask", "192.168.20.13", "0.0.0.0"),
        (
            "invalid request IP with invalid proxy wildcard",
            "0.0.0.0",
            "*",
        ),
    ]
}

#[test]
fn testInvalidIpAddressesDoNotMatch() -> TestResult {
    for (name, request_ip, proxy_ip) in invalid_ip_address_data() {
        assert!(!check_ip4(request_ip, proxy_ip)?, "{name}");
    }
    Ok(())
}

fn anonymized_ip_data() -> Vec<(&'static str, &'static str)> {
    vec![
        ("192.168.1.1", "192.168.1.0"),
        ("1.2.3.4", "1.2.3.0"),
        ("2a01:198:603:0:396e:4789:8e99:890f", "2a01:198:603::"),
        ("2a01:198:603:10:396e:4789:8e99:890f", "2a01:198:603:10::"),
        ("::1", "::"),
        ("0:0:0:0:0:0:0:1", "::"),
        ("1:0:0:0:0:0:0:1", "1::"),
        ("0:0:603:50:396e:4789:8e99:0001", "0:0:603:50::"),
        ("[0:0:603:50:396e:4789:8e99:0001]", "[0:0:603:50::]"),
        ("[2a01:198::3]", "[2a01:198::]"),
        ("::ffff:123.234.235.236", "::ffff:123.234.235.0"), // IPv4-mapped IPv6 addresses
        ("::FFFF:123.234.235.236", "::ffff:123.234.235.0"),
        ("::ffff:7bea:ebec", "::ffff:123.234.235.0"),
        (
            "0000:0000:0000:0000:0000:ffff:7bea:ebec",
            "::ffff:123.234.235.0",
        ),
        ("[::FFFF:123.234.235.236]", "[::ffff:123.234.235.0]"),
        ("::123.234.235.236", "::123.234.235.0"), // deprecated IPv4-compatible IPv6 address
        ("::7bea:ebec", "::123.234.235.0"),
        ("::1:0:1", "::"), // neither mapped nor compatible: regular IPv6 anonymization applies
        ("0000:0000:0000:0000:0000:0001:0000:0001", "::"),
        ("fe80::1fc4:15d8:78db:2319%enp4s0", "fe80::"), // IPv6 link-local with RFC4007 scoping
    ]
}

#[test]
fn testAnonymize() -> TestResult {
    for (ip, expected) in anonymized_ip_data() {
        assert_eq!(expected, anonymize(ip, 1, 8)?, "{ip}");
    }
    Ok(())
}

fn anonymized_ip_data_with_bytes() -> Vec<(&'static str, &'static str, i64, i64)> {
    vec![
        ("192.168.1.1", "192.168.0.0", 2, 8),
        ("192.168.1.1", "192.0.0.0", 3, 8),
        ("192.168.1.1", "0.0.0.0", 4, 8),
        ("1.2.3.4", "1.2.3.0", 1, 8),
        ("1.2.3.4", "1.2.3.4", 0, 8),
        (
            "2a01:198:603:0:396e:4789:8e99:890f",
            "2a01:198:603:0:396e:4789:8e99:890f",
            1,
            0,
        ),
        (
            "2a01:198:603:0:396e:4789:8e99:890f",
            "2a01:198:603:0:396e:4789::",
            1,
            4,
        ),
        (
            "2a01:198:603:10:396e:4789:8e99:890f",
            "2a01:198:603:10:396e:4700::",
            1,
            5,
        ),
        ("2a01:198:603:10:396e:4789:8e99:890f", "2a00::", 1, 15),
        ("2a01:198:603:10:396e:4789:8e99:890f", "::", 1, 16),
        ("::1", "::", 1, 1),
        ("0:0:0:0:0:0:0:1", "::", 1, 1),
        ("1:0:0:0:0:0:0:1", "1::", 1, 1),
        ("0:0:603:50:396e:4789:8e99:0001", "0:0:603::", 1, 10),
        (
            "[0:0:603:50:396e:4789:8e99:0001]",
            "[::603:50:396e:4789:8e00:0]",
            1,
            3,
        ),
        ("[2a01:198::3]", "[2a01:198::]", 1, 2),
        ("::ffff:123.234.235.236", "::ffff:123.234.235.0", 1, 8), // IPv4-mapped IPv6 addresses
        ("::123.234.235.236", "::123.234.0.0", 2, 8), // deprecated IPv4-compatible IPv6 address
    ]
}

#[test]
fn testAnonymizeWithBytes() -> TestResult {
    for (ip, expected, bytes_for_v4, bytes_for_v6) in anonymized_ip_data_with_bytes() {
        assert_eq!(expected, anonymize(ip, bytes_for_v4, bytes_for_v6)?, "{ip}");
    }
    Ok(())
}

fn assert_invalid_argument(
    result: Result<String, symfony_http_foundation::exception::Error>,
    message: &str,
) {
    match result {
        Err(e) => {
            assert!(e.is_a("InvalidArgumentException"));
            assert_eq!(message, e.message().to_string_lossy());
        }
        Ok(value) => panic!("expected InvalidArgumentException, got {value:?}"),
    }
}

#[test]
fn testAnonymizeV4WithNegativeBytes() {
    assert_invalid_argument(
        anonymize("anything", -1, 8),
        "Cannot anonymize less than 0 bytes.",
    );
}

#[test]
fn testAnonymizeV6WithNegativeBytes() {
    assert_invalid_argument(
        anonymize("anything", 1, -1),
        "Cannot anonymize less than 0 bytes.",
    );
}

#[test]
fn testAnonymizeV4WithTooManyBytes() {
    assert_invalid_argument(
        anonymize("anything", 5, 8),
        "Cannot anonymize more than 4 bytes for IPv4 and 16 bytes for IPv6.",
    );
}

#[test]
fn testAnonymizeV6WithTooManyBytes() {
    assert_invalid_argument(
        anonymize("anything", 1, 17),
        "Cannot anonymize more than 4 bytes for IPv4 and 16 bytes for IPv6.",
    );
}

fn get_ip4_subnet_mask_zero_data() -> Vec<(bool, &'static str, &'static str)> {
    vec![
        (true, "1.2.3.4", "0.0.0.0/0"),
        (true, "1.2.3.4", "192.168.1.0/0"),
        (false, "1.2.3.4", "256.256.256/0"), // invalid CIDR notation
    ]
}

#[test]
fn testIp4SubnetMaskZero() -> TestResult {
    for (matches, remote_addr, cidr) in get_ip4_subnet_mask_zero_data() {
        assert_eq!(matches, check_ip4(remote_addr, cidr)?, "{cidr}");
    }
    Ok(())
}

fn get_is_private_ip_data() -> Vec<(&'static str, bool)> {
    vec![
        // private
        ("127.0.0.1", true),
        ("10.0.0.1", true),
        ("192.168.0.1", true),
        ("192.0.2.1", true),
        ("198.51.100.1", true),
        ("203.0.113.1", true),
        ("172.16.0.1", true),
        ("169.254.0.1", true),
        ("198.18.0.1", true),
        ("0.0.0.1", true),
        ("240.0.0.1", true),
        ("100.64.0.1", true),
        ("::1", true),
        ("fc00::1", true),
        ("fe80::1", true),
        ("::ffff:0:1", true),
        ("fd00::1", true),
        ("::7f00:1", true),
        ("2002:7f00:1::", true),
        ("2001::1", true),
        ("2001:db8::1", true),
        ("2001:0002::1", true),
        ("64:ff9b::7f00:1", true),
        ("64:ff9b:1::7f00:1", true),
        // public
        ("104.26.14.6", false),
        ("2606:4700:20::681a:e06", false),
        // just past the boundary of the ranges above, which pins their prefix lengths
        ("100.128.0.1", false),
        ("198.20.0.1", false),
        ("2001:2:1::1", false),
    ]
}

#[test]
fn testIsPrivateIp() -> TestResult {
    for (ip, matches) in get_is_private_ip_data() {
        assert_eq!(matches, is_private_ip(ip)?, "{ip}");
    }
    Ok(())
}

fn get_is_private_ip_invalid_data() -> Vec<(&'static str, &'static str)> {
    vec![
        ("decimal", "2130706433"),
        ("hexadecimal", "0x7f000001"),
        ("leading zero", "010.0.0.1"),
        ("short form", "127.1"),
        ("zone id", "fe80::1%eth0"),
        ("not an IP", "not-an-ip"),
        ("empty string", ""),
    ]
}

#[test]
fn testIsPrivateIpThrowsOnNonCanonicalIp() {
    for (name, ip) in get_is_private_ip_invalid_data() {
        match is_private_ip(ip) {
            Err(e) => {
                assert!(e.is_a("ValueError"), "{name}");
                assert_eq!(
                    format!("\"{ip}\" is not a valid IP address."),
                    e.message().to_string_lossy(),
                    "{name}"
                );
            }
            Ok(value) => panic!("{name}: expected ValueError, got {value}"),
        }
    }
}

#[test]
fn testCacheSizeLimit() -> TestResult {
    // Upstream reads the static before the loop (a copy of the array).
    let checked_ips = checked_ips_count();

    let max_checked_ips = 1000;

    for i in 1..(max_checked_ips * 3 / 2) {
        let ip = format!("192.168.1.{i:0<3}");

        check_ip4(&ip, "127.0.0.1")?;
    }

    assert!(checked_ips < max_checked_ips);
    Ok(())
}

/// Not upstream: after the loop above the cache itself stays bounded,
/// as `setCacheResult()` drops the oldest 500 past 1,000 entries.
#[test]
fn cache_stays_bounded() -> TestResult {
    for i in 1..1500 {
        check_ip4(&format!("192.168.1.{i:0<3}"), "127.0.0.1")?;
        assert!(checked_ips_count() <= 1001);
    }
    Ok(())
}

fn get_ipv4_mapped_data() -> Vec<(bool, &'static str, &'static str)> {
    vec![
        (true, "::ffff:10.0.0.5", "10.0.0.0/8"),
        (true, "::ffff:a00:5", "10.0.0.0/8"),
        (true, "::ffff:127.0.0.1", "127.0.0.1"),
        (true, "::ffff:10.0.0.5", "::ffff:10.0.0.0/104"),
        (false, "::ffff:8.8.8.8", "10.0.0.0/8"),
        (false, "::10.0.0.5", "10.0.0.0/8"),
        (false, "2002:a00:5::", "10.0.0.0/8"),
        (false, "10.0.0.5", "::ffff:10.0.0.0/104"),
        (false, "::ffff:10.0.0.5\0", "10.0.0.0/8"),
    ]
}

#[test]
fn testIpv4MappedAddressesAreCheckedAsTheIpv4HostTheyEmbed() -> TestResult {
    for (matches, remote_addr, cidr) in get_ipv4_mapped_data() {
        assert_eq!(
            matches,
            check_ip(remote_addr, &[cidr])?,
            "{remote_addr:?} {cidr}"
        );
    }
    Ok(())
}

fn get_addresses_a_remote_client_can_present() -> Vec<(&'static str, &'static str)> {
    vec![
        ("IPv4-mapped", "::ffff:8.8.8.8"),
        ("IPv4-compatible", "::8.8.8.8"),
        ("6to4", "2002:808:808::1"),
        ("Teredo", "2001:0:4136:e378:8000:63bf:3fff:fdd2"),
        ("NAT64", "64:ff9b::8.8.8.8"),
    ]
}

#[test]
fn testPrivateRangesDoNotCoverAddressesARemoteClientCanPresent() -> TestResult {
    for (name, ip) in get_addresses_a_remote_client_can_present() {
        assert!(!check_ip(ip, &PRIVATE_RANGES)?, "{name}");
        assert!(is_private_ip(ip)?, "{name}");
    }
    Ok(())
}

#[test]
fn testPrivateRangesAreASubsetOfPrivateSubnets() {
    let diff: Vec<&str> = PRIVATE_RANGES
        .iter()
        .filter(|range| !PRIVATE_SUBNETS.contains(range))
        .copied()
        .collect();
    assert_eq!(Vec::<&str>::new(), diff);
}
