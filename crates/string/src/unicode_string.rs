//! Port of Symfony\Component\String\UnicodeString.php, with the methods it
//! inherits from AbstractUnicodeString.php and AbstractString.php that
//! symfony/console uses: construction (NFC), `length()`, `slice()`,
//! `chunk()`, `split()` without flags, `wordwrap()` and `width()`.
//!
//! Graphemes follow ICU's rules (`grapheme_strlen()`, `grapheme_substr()`)
//! through `unicode-segmentation`, and NFC comes from
//! `unicode-normalization`; both match the oracle's ICU 72.1 on every
//! recorded case (`tests/unicode.rs`). Differences, all pinned by tests:
//! `chunk()` uses PCRE2's `\X` in PHP, which splits 218 of the 4,105
//! recorded sequences differently (a lone regional indicator before an
//! extending mark, Indic conjuncts, Prepend before an emoji); and code
//! points assigned after Unicode 15.0 are classified by the crates' newer
//! data (README, "Differences").
//!
//! Symfony source copyright (c) Fabien Potencier; distributed under MIT.
//!
//! Upstream: symfony/string UnicodeString.php AbstractUnicodeString.php AbstractString.php

use unicode_normalization::{UnicodeNormalization, is_nfc};
use unicode_segmentation::UnicodeSegmentation;

use crate::{InvalidArgumentException, php, tables};

#[derive(Debug, Clone, Default, PartialEq, Eq, Hash)]
pub struct UnicodeString {
    string: String,
}

impl UnicodeString {
    /// `new UnicodeString($string)`: normalized to NFC.
    pub fn new(string: &str) -> Self {
        if string.is_empty() || is_nfc(string) {
            return Self {
                string: string.to_owned(),
            };
        }
        Self {
            string: string.nfc().collect(),
        }
    }

    /// `new UnicodeString($bytes)`: invalid UTF-8 throws.
    pub fn from_bytes(bytes: &[u8]) -> Result<Self, InvalidArgumentException> {
        std::str::from_utf8(bytes)
            .map(Self::new)
            .map_err(|_| InvalidArgumentException("Invalid UTF-8 string.".to_owned()))
    }

    pub fn as_str(&self) -> &str {
        &self.string
    }

    pub fn into_string(self) -> String {
        self.string
    }

    fn with(&self, string: String) -> Self {
        // A clone of $this with another string: no new normalization.
        Self { string }
    }

    /// `length()`: `grapheme_strlen()`.
    pub fn length(&self) -> usize {
        self.string.graphemes(true).count()
    }

    /// `slice($start, $length)`: `grapheme_substr()`. A negative start
    /// counts from the end; a negative length leaves that many graphemes
    /// off the end; out of range gives `''`.
    pub fn slice(&self, start: i64, length: Option<i64>) -> Self {
        let boundaries: Vec<usize> = self
            .string
            .grapheme_indices(true)
            .map(|(at, _)| at)
            .chain(std::iter::once(self.string.len()))
            .collect();
        let count = (boundaries.len() - 1) as i64;
        let from = if start < 0 {
            (count + start).max(0)
        } else {
            start
        };
        if from > count {
            return self.with(String::new());
        }
        let to = match length {
            None => count,
            Some(length) if length < 0 => count + length,
            Some(length) => from.saturating_add(length).min(count),
        };
        if to <= from {
            return self.with(String::new());
        }
        let (from, to) = (boundaries[from as usize], boundaries[to as usize]);
        self.with(self.string[from..to].to_owned())
    }

    /// `chunk($length)`: runs of `$length` graphemes, the last one shorter.
    pub fn chunk(&self, length: usize) -> Result<Vec<Self>, InvalidArgumentException> {
        if length < 1 {
            return Err(InvalidArgumentException(
                "The chunk length must be greater than zero.".to_owned(),
            ));
        }
        if self.string.is_empty() {
            return Ok(Vec::new());
        }
        if self.string.len() <= length {
            return Ok(vec![self.clone()]);
        }
        let graphemes = pcre_graphemes(&self.string);
        Ok(graphemes
            .chunks(length)
            .map(|chunk| self.with(chunk.concat()))
            .collect())
    }

    /// `split($delimiter)` (no limit, no flags): pieces between matches of
    /// the NFC delimiter that start and end on grapheme boundaries, as
    /// `grapheme_strpos()` finds them.
    pub fn split(&self, delimiter: &str) -> Result<Vec<Self>, InvalidArgumentException> {
        if delimiter.is_empty() {
            return Err(InvalidArgumentException(
                "Split delimiter is empty.".to_owned(),
            ));
        }
        let delimiter: String = delimiter.nfc().collect();
        let mut chunks = Vec::new();
        let mut tail = self.string.as_str();
        while let Some(at) = grapheme_find(tail, &delimiter) {
            chunks.push(self.with(tail[..at].to_owned()));
            tail = &tail[at + delimiter.len()..];
        }
        chunks.push(self.with(tail.to_owned()));
        Ok(chunks)
    }

    /// `wordwrap($width, $break, $cut)`: PHP's `wordwrap()` run on a mask of
    /// the graphemes, so a grapheme is never split.
    pub fn wordwrap(
        &self,
        width: usize,
        brk: &str,
        cut: bool,
    ) -> Result<Self, InvalidArgumentException> {
        let lines = if brk.is_empty() {
            vec![self.clone()]
        } else {
            self.split(brk)?
        };
        if lines.len() == 1 && lines[0].string.is_empty() {
            return Ok(lines[0].clone());
        }
        let mut chars: Vec<String> = Vec::new();
        let mut mask = Vec::new();
        for (index, line) in lines.iter().enumerate() {
            if index > 0 {
                chars.push(brk.to_owned());
                mask.push(b'#');
            }
            for grapheme in pcre_graphemes(&line.string) {
                chars.push(grapheme.to_owned());
                mask.push(if grapheme == " " { b' ' } else { b'?' });
            }
        }
        // wordwrap() does not recognize a break at the very end of its
        // input and counts it as part of the last line.
        let kept = mask.len() - mask.iter().rev().take_while(|&&b| b == b'#').count();
        let mut wrapped = php::wordwrap(&mask[..kept], width, b'#', cut);
        wrapped.extend_from_slice(&mask[kept..]);

        let mut string = String::new();
        let mut j = 0;
        let mut i: isize = -1;
        for (b, _) in wrapped.iter().enumerate().filter(|(_, c)| **c == b'#') {
            i += 1;
            while (i as usize) < b {
                if let Some(c) = chars.get(j) {
                    string.push_str(c);
                }
                j += 1;
                i += 1;
            }
            if chars.get(j).is_some_and(|c| c == brk || c == " ") {
                j += 1;
            }
            string.push_str(brk);
        }
        for c in chars.iter().skip(j) {
            string.push_str(c);
        }
        Ok(self.with(string))
    }

    /// `width($ignoreAnsiDecoration)`: the widest line, in terminal columns.
    pub fn width(&self, ignore_ansi_decoration: bool) -> usize {
        let mut s = self.string.clone();
        if ignore_ansi_decoration && s.contains('\x07') {
            s = remove_bel_osc(&s);
        }
        s.retain(|c| !matches!(c, '\x00' | '\x05' | '\x07'));
        if s.contains('\r') {
            s = s.replace("\r\n", "\n").replace('\r', "\n");
        }
        if !ignore_ansi_decoration {
            s.retain(|c| !is_cc(c));
        }
        let mut width = 0;
        for line in s.split('\n') {
            let line = if ignore_ansi_decoration {
                remove_ansi(line)
            } else {
                line.to_owned()
            };
            let line_width = wcswidth(&line);
            if line_width > width as i64 {
                width = line_width as usize;
            }
        }
        width
    }
}

impl std::fmt::Display for UnicodeString {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.string)
    }
}

/// The first match of `needle` in `haystack` that starts and ends on a
/// grapheme boundary of `haystack`.
fn grapheme_find(haystack: &str, needle: &str) -> Option<usize> {
    let boundaries: Vec<usize> = haystack
        .grapheme_indices(true)
        .map(|(at, _)| at)
        .chain(std::iter::once(haystack.len()))
        .collect();
    boundaries.iter().copied().find(|&at| {
        haystack[at..].starts_with(needle) && boundaries.binary_search(&(at + needle.len())).is_ok()
    })
}

/// Extended grapheme clusters as PCRE2 10.44's `\X` finds them, which
/// `chunk()` (and through it `wordwrap()`) uses: ICU's clusters, split where
/// the oracle shows PCRE2 breaks and ICU does not (`tests/unicode.rs`):
/// before a character joined only by the Indic conjunct rule (GB9c, which
/// PCRE2 10.44 lacks), after a regional indicator before anything but
/// another regional indicator, and between a Prepend character and an
/// Extended_Pictographic one.
pub fn pcre_graphemes(string: &str) -> Vec<&str> {
    let mut out = Vec::new();
    for cluster in string.graphemes(true) {
        let mut start = 0;
        let mut previous: Option<char> = None;
        for (at, c) in cluster.char_indices() {
            if let Some(p) = previous
                && pcre_breaks_between(p, c)
            {
                out.push(&cluster[start..at]);
                start = at;
            }
            previous = Some(c);
        }
        out.push(&cluster[start..]);
    }
    out
}

fn is_regional_indicator(c: char) -> bool {
    ('\u{1F1E6}'..='\u{1F1FF}').contains(&c)
}

/// Whether `c` extends any base (GB9, GB9a): "a" followed by it is one cluster.
fn extends(c: char) -> bool {
    let mut text = String::from("a");
    text.push(c);
    text.graphemes(true).count() == 1
}

/// Whether `c` is Prepend (GB9b): it followed by "a" is one cluster.
fn is_prepend(c: char) -> bool {
    let mut text = String::from(c);
    text.push('a');
    text.graphemes(true).count() == 1
}

/// Inside one ICU cluster, whether PCRE2's `\X` breaks between `p` and `c`.
fn pcre_breaks_between(p: char, c: char) -> bool {
    if is_regional_indicator(p) {
        return !is_regional_indicator(c);
    }
    let pictographic = in_table(tables::EXTPICT, c as u32);
    if is_prepend(p) && pictographic {
        return true;
    }
    // After an extending character (a virama, ZWJ or other mark), a joined
    // character that does not extend, is not an emoji after ZWJ (GB11) and
    // not a regional indicator was joined by GB9c alone.
    extends(p) && !extends(c) && !(p == '\u{200D}' && pictographic) && !is_regional_indicator(c)
}

/// `\p{Cc}` or DEL: C0 and C1 controls.
fn is_cc(c: char) -> bool {
    matches!(c, '\u{0}'..='\u{1F}' | '\u{7F}'..='\u{9F}')
}

/// `preg_replace('/\x1B\][^\x07\x1B\n\r]*+\x07/', '', $s)`.
fn remove_bel_osc(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let mut rest = s;
    while let Some(at) = rest.find("\x1B]") {
        let body = &rest[at + 2..];
        let end = body
            .find(['\x07', '\x1B', '\n', '\r'])
            .filter(|&e| body[e..].starts_with('\x07'));
        match end {
            Some(e) => {
                out.push_str(&rest[..at]);
                rest = &body[e + 1..];
            }
            None => {
                out.push_str(&rest[..at + 2]);
                rest = body;
            }
        }
    }
    out.push_str(rest);
    out
}

/// The per-line `preg_replace()` of `width()`: escape sequences (CSI; DCS,
/// OSC, SOS, PM and APC up to ST; any other ESC with its intermediate and
/// final bytes) and runs of control characters.
fn remove_ansi(line: &str) -> String {
    let bytes = line.as_bytes();
    let mut out = String::with_capacity(line.len());
    let mut at = 0;
    let in_range = |b: u8, lo: u8, hi: u8| (lo..=hi).contains(&b);
    while at < bytes.len() {
        if bytes[at] == 0x1B {
            let next = at + 1;
            // \[ [\x30-\x3F]*+ [\x20-\x2F]*+ [\x40-\x7E]
            if bytes.get(next) == Some(&b'[') {
                let mut k = next + 1;
                while bytes.get(k).is_some_and(|&b| in_range(b, 0x30, 0x3F)) {
                    k += 1;
                }
                while bytes.get(k).is_some_and(|&b| in_range(b, 0x20, 0x2F)) {
                    k += 1;
                }
                if bytes.get(k).is_some_and(|&b| in_range(b, 0x40, 0x7E)) {
                    at = k + 1;
                    continue;
                }
            }
            // [P\]X^_] .*? \x1B\\
            if bytes.get(next).is_some_and(|b| b"P]X^_".contains(b))
                && let Some(end) = line[next + 1..].find("\x1B\\")
            {
                at = next + 1 + end + 2;
                continue;
            }
            // [\x20-\x2F]*+ [\x30-\x7E]?
            let mut k = next;
            while bytes.get(k).is_some_and(|&b| in_range(b, 0x20, 0x2F)) {
                k += 1;
            }
            if bytes.get(k).is_some_and(|&b| in_range(b, 0x30, 0x7E)) {
                k += 1;
            }
            at = k;
            continue;
        }
        let c = line[at..].chars().next().unwrap_or('\0');
        if !is_cc(c) {
            out.push(c);
        }
        at += c.len_utf8();
    }
    out
}

/// `EMOJI_REGEX` (PCRE2 10.40 and later): `[\p{ExtPict}\x{1F1E6}-\x{1F1FF}#*0-9]`.
fn is_emoji(c: char) -> bool {
    let cp = c as u32;
    matches!(c, '#' | '*' | '0'..='9')
        || (0x1F1E6..=0x1F1FF).contains(&cp)
        || in_table(tables::EXTPICT, cp)
}

fn in_table(table: &[(u32, u32)], cp: u32) -> bool {
    table
        .binary_search_by(|&(lo, hi)| {
            if hi < cp {
                std::cmp::Ordering::Less
            } else if lo > cp {
                std::cmp::Ordering::Greater
            } else {
                std::cmp::Ordering::Equal
            }
        })
        .is_ok()
}

/// `wcswidth()`: -1 if a control character remains.
fn wcswidth(string: &str) -> i64 {
    let mut width = 0i64;
    let mut last_char: Option<char> = None;
    let mut last_width = 0;
    let mut joined = false;
    // A character after which ZWJ, VS16 or a skin tone joins: non-ASCII
    // (`1 < strlen($lastChar)`) and an emoji.
    let joinable = |c: Option<char>| c.is_some_and(|c| !c.is_ascii() && is_emoji(c));
    for c in string.chars() {
        if joined {
            joined = false;
            if !c.is_ascii() && is_emoji(c) {
                continue;
            }
        }
        let cp = c as u32;
        if (0x20..0x7F).contains(&cp) {
            width += 1;
            last_char = Some(c);
            last_width = 1;
            continue;
        }
        if cp == 0
            || cp == 0x034F
            || cp == 0x061C
            || (0x1160..=0x11FF).contains(&cp)
            || cp == 0x180E
            || (0x200B..=0x200F).contains(&cp)
            || cp == 0x2028
            || cp == 0x2029
            || (0x202A..=0x202E).contains(&cp)
            || (0x2060..=0x206F).contains(&cp)
            || (0xD7B0..=0xD7FF).contains(&cp)
            || cp == 0xFEFF
            || (0xFFF0..=0xFFFB).contains(&cp)
        {
            joined = cp == 0x200D && joinable(last_char);
            continue;
        }
        if cp < 0xA0 {
            return -1;
        }
        if cp == 0xFE0F {
            if last_char.is_some_and(is_emoji) && last_width == 1 {
                width += 1;
                last_width = 2;
            }
            continue;
        }
        if cp > 0xFFFF {
            if (0x13430..=0x1343F).contains(&cp)
                || (0x1BCA0..=0x1BCA3).contains(&cp)
                || (0x1D173..=0x1D17A).contains(&cp)
                || (0xE0000..=0xE0FFF).contains(&cp)
            {
                continue;
            }
            if (0x1F3FB..=0x1F3FF).contains(&cp) && joinable(last_char) {
                if last_width == 1 {
                    width += 1;
                    last_width = 2;
                }
                continue;
            }
        }
        if in_table(tables::ZERO, cp) {
            continue;
        }
        if in_table(tables::WIDE, cp) {
            width += 2;
            last_char = Some(c);
            last_width = 2;
            continue;
        }
        width += 1;
        last_char = Some(c);
        last_width = 1;
    }
    width
}
