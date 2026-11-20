//! The part of PHP's date/time string parsing (`new DateTimeImmutable($s)`,
//! `strtotime($s, $now)`) that DateComparator needs, written from PHP's
//! "Supported Date and Time Formats" documentation and checked against
//! PHP 8.4.27 (tests/data/php.json). Times are UTC (PHP's default
//! `date.timezone`). Accepted, case-insensitively and separated by
//! whitespace:
//!
//! - `@<seconds>` alone;
//! - `YYYY-MM-DD` or `YYYY/MM/DD`, optionally followed (after a space or
//!   `T`) by `HH:MM` or `HH:MM:SS`; or a time alone (today);
//! - `now`, `today`, `midnight`, `noon`, `yesterday`, `tomorrow`;
//! - relative amounts: `[+|-]N unit`, `last|previous|next|this unit`,
//!   with `ago` negating the amounts before it; units are sec(ond)(s),
//!   min(ute)(s), hour(s), day(s), week(s), fortnight(s), month(s),
//!   year(s).
//!
//! Anything else (weekday names, month names, time zones, ordinals...) is
//! refused rather than guessed.
//!
//! Upstream: none

/// Days from 1970-01-01 to the given proleptic Gregorian date.
fn days_from_civil(y: i64, m: i64, d: i64) -> i64 {
    let y = if m <= 2 { y - 1 } else { y };
    let era = y.div_euclid(400);
    let yoe = y - era * 400;
    let mp = (m + 9) % 12;
    let doy = (153 * mp + 2) / 5 + d - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    era * 146_097 + doe - 719_468
}

fn civil_from_days(z: i64) -> (i64, i64, i64) {
    let z = z + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z - era * 146_097;
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let y = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    (if m <= 2 { y + 1 } else { y }, m, d)
}

#[derive(Default)]
struct Relative {
    y: i64,
    m: i64,
    d: i64,
    h: i64,
    i: i64,
    s: i64,
}

fn unit(word: &str) -> Option<(usize, i64)> {
    // (field index, multiplier): 0 y, 1 m, 2 d, 3 h, 4 i, 5 s
    Some(match word {
        "sec" | "secs" | "second" | "seconds" => (5, 1),
        "min" | "mins" | "minute" | "minutes" => (4, 1),
        "hour" | "hours" => (3, 1),
        "day" | "days" => (2, 1),
        "week" | "weeks" => (2, 7),
        "fortnight" | "fortnights" => (2, 14),
        "month" | "months" => (1, 1),
        "year" | "years" => (0, 1),
        _ => return None,
    })
}

fn number(s: &str) -> Option<i64> {
    if s.is_empty() || !s.bytes().all(|c| c.is_ascii_digit()) {
        return None;
    }
    s.parse().ok()
}

fn parse_time(s: &str) -> Option<(i64, i64, i64)> {
    let parts: Vec<&str> = s.split(':').collect();
    if !(2..=3).contains(&parts.len()) {
        return None;
    }
    let h = number(parts[0]).filter(|&h| h < 24 && parts[0].len() <= 2)?;
    let i = number(parts[1]).filter(|&i| i < 60 && parts[1].len() == 2)?;
    let s = match parts.get(2) {
        Some(p) => number(p).filter(|&s| s < 60 && p.len() == 2)?,
        None => 0,
    };
    Some((h, i, s))
}

fn parse_date(s: &str) -> Option<(i64, i64, i64)> {
    let sep = if s.contains('-') { '-' } else { '/' };
    let parts: Vec<&str> = s.split(sep).collect();
    if parts.len() != 3 || parts[0].len() != 4 {
        return None;
    }
    let y = number(parts[0])?;
    let m = number(parts[1]).filter(|&m| (1..=12).contains(&m) && parts[1].len() <= 2)?;
    let d = number(parts[2]).filter(|&d| (1..=31).contains(&d) && parts[2].len() <= 2)?;
    Some((y, m, d))
}

/// The Unix timestamp `$s` stands for at `now`, or None when the string
/// is not in the supported subset.
pub fn timestamp(s: &[u8], now: i64) -> Option<i64> {
    let s = std::str::from_utf8(s).ok()?.trim().to_ascii_lowercase();
    if let Some(ts) = s.strip_prefix('@') {
        let (neg, digits) = match ts.strip_prefix('-') {
            Some(d) => (true, d),
            None => (false, ts),
        };
        let v = number(digits)?;
        return Some(if neg { -v } else { v });
    }
    let (mut y, mut m, mut d) = civil_from_days(now.div_euclid(86_400));
    let secs = now.rem_euclid(86_400);
    let (mut h, mut i, mut sec) = (secs / 3600, secs % 3600 / 60, secs % 60);
    let mut rel = Relative::default();
    // tokens: a sign may be separated from its number by spaces
    let mut words: Vec<String> = Vec::new();
    for w in s.split_whitespace() {
        // split a "T" between date and time
        if let Some((date, time)) = w
            .split_once('t')
            .filter(|(a, b)| parse_date(a).is_some() && parse_time(b).is_some())
        {
            words.push(date.to_string());
            words.push(time.to_string());
        } else {
            words.push(w.to_string());
        }
    }
    let mut k = 0;
    let mut seen_any = false;
    while k < words.len() {
        let w = words[k].as_str();
        k += 1;
        seen_any = true;
        if let Some((yy, mm, dd)) = parse_date(w) {
            (y, m, d) = (yy, mm, dd);
            (h, i, sec) = (0, 0, 0);
            continue;
        }
        if let Some(t) = parse_time(w) {
            (h, i, sec) = t;
            continue;
        }
        match w {
            "now" => continue,
            "today" | "midnight" => {
                (h, i, sec) = (0, 0, 0);
                continue;
            }
            "noon" => {
                (h, i, sec) = (12, 0, 0);
                continue;
            }
            "yesterday" | "tomorrow" => {
                rel.d += if w == "yesterday" { -1 } else { 1 };
                (h, i, sec) = (0, 0, 0);
                continue;
            }
            "ago" => {
                rel = Relative {
                    y: -rel.y,
                    m: -rel.m,
                    d: -rel.d,
                    h: -rel.h,
                    i: -rel.i,
                    s: -rel.s,
                };
                continue;
            }
            _ => {}
        }
        // an amount and a unit
        let (amount, unit_word) = match w {
            "last" | "previous" | "next" | "this" => {
                let n = match w {
                    "next" => 1,
                    "this" => 0,
                    _ => -1,
                };
                let u = words.get(k)?.clone();
                k += 1;
                (n, u)
            }
            _ => {
                let (sign, rest) = match w.as_bytes().first() {
                    Some(b'+') => (1, &w[1..]),
                    Some(b'-') => (-1, &w[1..]),
                    _ => (1, w),
                };
                let rest = if rest.is_empty() {
                    let next = words.get(k)?.clone();
                    k += 1;
                    next
                } else {
                    rest.to_string()
                };
                // the number may be glued to its unit ("2days")
                let split = rest
                    .find(|c: char| !c.is_ascii_digit())
                    .unwrap_or(rest.len());
                let n = number(&rest[..split])?;
                let u = if split < rest.len() {
                    rest[split..].to_string()
                } else {
                    let next = words.get(k)?.clone();
                    k += 1;
                    next
                };
                (sign * n, u)
            }
        };
        let (field, factor) = unit(&unit_word)?;
        let v = amount * factor;
        match field {
            0 => rel.y += v,
            1 => rel.m += v,
            2 => rel.d += v,
            3 => rel.h += v,
            4 => rel.i += v,
            _ => rel.s += v,
        }
    }
    if !seen_any {
        return None;
    }
    let months = y * 12 + (m - 1) + rel.y * 12 + rel.m;
    let (y, m) = (months.div_euclid(12), months.rem_euclid(12) + 1);
    let days = days_from_civil(y, m, 1) + (d - 1) + rel.d;
    Some(days * 86_400 + (h + rel.h) * 3600 + (i + rel.i) * 60 + sec + rel.s)
}
