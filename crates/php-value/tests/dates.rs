//! strtotime(), createFromFormat(DATE_RFC2822) and gmdate() against the
//! answers PHP 8.4.27 gave in the oracle (tests/data/dates.json,
//! oracle/recorders/http_foundation_dates.php). A text the port refuses
//! must be listed in `REFUSED_*`, so a change in what is read shows up;
//! every other text must give PHP's answer, `false` included.
//!
//! Upstream: none

use php_value::{base64, dateparse, datetime::gmdate};
use serde_json::Value as Json;

fn data() -> Json {
    serde_json::from_str(include_str!("data/dates.json")).unwrap()
}

fn text(field: &Json) -> Vec<u8> {
    base64::decode_strict(field.as_str().unwrap().as_bytes()).unwrap()
}

/// Texts PHP reads that the port refuses (`Unsupported`).
const REFUSED_STRTOTIME: &[&str] = &[
    "a",
    "x",
    "EST",
    "CET",
    "0x10",
    "May",
    "may",
    "sept",
    "Sept",
    "next",
    "last",
    "ago",
    "first",
    "third",
    "week",
    "day",
    "monday next week",
    "next monday",
    "last friday",
    "this week",
    "Fri, 20 May 2011 15:25:52 GMT+2",
    "Fri, 20 May 2011 15:25:52 EST",
    "Fri, 20 May 2011 15:25:52 PST",
    "Fri, 20 May 2011 15:25:52 CET",
    "Fri, 20 May 2011 15:25:52 XYZ",
    "Fri, 20 May 2011 15:25:52 GMT GMT",
    "May 20 2011",
    "May 20, 2011",
    "Fri, 20 May 2011 15:25:52.5 GMT",
    "Fri, 20 Mai 2011 15:25:52 GMT",
    "Fri, 20 May 99999 15:25:52 GMT",
    "20110520",
    "2011/05/20",
    "05/20/2011",
    "20.05.2011",
    "+1 weekday",
    "1234567890",
    "0",
    "-",
    "+",
    "1",
    "12",
    "123",
    "1234",
    "12345",
    "1.5",
    "Tue,",
    ",",
    ";",
    "\"",
    "20",
    "Fri, 20 May 2011 15:25:52 GMT; path=/",
    "Fri, 20 May 2011 15:25:52 GMTx",
    "15:25:52",
    "15:25",
];

const REFUSED_RFC2822: &[&str] = &[
    "Fri, 20 May 2011 15:25:52 GMT+2",
    "Fri, 20 May 2011 15:25:52 EST",
    "Fri, 20 May 2011 15:25:52 PST",
    "Fri, 20 May 2011 15:25:52 CET",
    "Fri, 20 May 2011 15:25:52 XYZ",
    "Fri, 20 May 2011 15:25:52 GMT GMT",
    "Fri, 20 May 2011 15:25:52.5 GMT",
    "Fri, 20 Mai 2011 15:25:52 GMT",
    "Fri, 20 May 2011 15:25:52 GMTx",
    "Tue, 4 Sep 2012 20:00:00 EST",
    "Tue, 4 Sep 2012 20:00:00 Europe/Paris",
    "Xyz, 4 Sep 2012 20:00:00 +0200",
    "Tue, 4 Sep 2012 20:00:00 a",
];

#[test]
fn strtotime_matches_php() {
    let data = data();
    let base = data["base"].as_i64().unwrap();
    assert_eq!(data["timezone"], "UTC");
    let mut refused = Vec::new();
    for case in data["strtotime"].as_array().unwrap() {
        let input = text(&case[0]);
        let shown = String::from_utf8_lossy(&input).into_owned();
        match dateparse::strtotime(&input, base) {
            Ok(answer) => assert_eq!(answer, case[1].as_i64(), "strtotime({shown:?})"),
            Err(_) => refused.push(shown),
        }
    }
    assert_eq!(refused, REFUSED_STRTOTIME);
}

#[test]
fn rfc2822_matches_php() {
    let mut refused = Vec::new();
    for case in data()["rfc2822"].as_array().unwrap() {
        let input = text(&case[0]);
        let shown = String::from_utf8_lossy(&input).into_owned();
        match dateparse::create_from_format_rfc2822(&input) {
            Ok(None) => assert!(
                case[1].is_null(),
                "createFromFormat({shown:?}) is {}",
                case[1]
            ),
            Ok(Some(date)) => {
                let expected = &case[1];
                assert!(
                    !expected.is_null(),
                    "createFromFormat({shown:?}) is false in PHP"
                );
                assert_eq!(date.timestamp(), expected[0].as_i64().unwrap(), "{shown:?}");
                assert_eq!(date.timezone().name(), expected[2], "{shown:?}");
                assert_eq!(date.format("Y-m-d H:i:s.u T P"), expected[3], "{shown:?}");
            }
            Err(_) => {
                if !REFUSED_RFC2822.contains(&shown.as_str()) {
                    refused.push(shown);
                }
            }
        }
    }
    // Every refusal is listed; texts PHP rejects as well may be refused
    // too when their first error is in a word.
    let unexpected: Vec<_> = refused
        .into_iter()
        .filter(|shown| {
            data()["rfc2822"]
                .as_array()
                .unwrap()
                .iter()
                .any(|case| text(&case[0]) == shown.as_bytes() && !case[1].is_null())
        })
        .collect();
    assert!(
        unexpected.is_empty(),
        "refused but read by PHP: {unexpected:?}"
    );
}

#[test]
fn gmdate_matches_php() {
    for case in data()["gmdate"].as_array().unwrap() {
        let timestamp = case[0].as_i64().unwrap();
        assert_eq!(
            gmdate("D, d M Y H:i:s T", timestamp),
            case[1],
            "{timestamp}"
        );
        assert_eq!(
            gmdate("D, d-M-Y H:i:s T", timestamp),
            case[2],
            "{timestamp}"
        );
        assert_eq!(gmdate("D, d M Y H:i:s", timestamp), case[3], "{timestamp}");
    }
}
