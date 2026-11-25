//! PHP's `preg_*` patterns (delimiter, PCRE2 body, modifiers), translated
//! into `fancy-regex` syntax for symfony/finder and symfony/string (PLAN.md
//! decision 5 and the Finder stack row). Written from PHP's and PCRE2's
//! documentation and checked against PHP 8.4.27 (tests/data/pcre.json);
//! no PHP or PCRE source is used.
//!
//! The translation keeps PCRE's meaning where the two dialects differ:
//! without `/u` the subject and the pattern are bytes and classes are
//! ASCII (`BytesMode::Ascii`); `$` matches at the end or before a final
//! newline (`/D` and `/m` as in PCRE); `^` under `/m` does not match
//! after a final newline; `/x`, `/U` and `/n` are applied here; a `{`
//! that does not start a quantifier is a literal. What is not translated
//! (recursion, conditionals, verbs, `\K`, `\X`, `\C`, branch reset,
//! callouts, `/J`, `/r`, POSIX classes under `/u`) is refused with
//! [`PcreError::Unsupported`] rather than approximated.
//!
//! Upstream: none

mod nesting;

use std::fmt;
use std::fmt::Write as _;

use fancy_regex::{BytesMode, RegexBuilder};

/// Why a pattern cannot be used.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PcreError {
    /// PHP rejects it too: `preg_match()` warns and returns `false`.
    Compile(String),
    /// PHP accepts it but this port does not translate it.
    Unsupported(String),
}

impl fmt::Display for PcreError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Compile(m) => write!(f, "preg compilation failed: {m}"),
            Self::Unsupported(m) => write!(f, "PCRE feature not supported by this port: {m}"),
        }
    }
}

impl std::error::Error for PcreError {}

/// `preg_match()` returned `false` while matching: a subject that is not
/// UTF-8 under `/u`, or the backtracking limit.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MatchError(pub String);

impl fmt::Display for MatchError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl std::error::Error for MatchError {}

/// A compiled `preg_*` pattern.
#[derive(Debug, Clone)]
pub struct Pcre {
    regex: fancy_regex::Regex,
    utf8: bool,
    translated: String,
    /// The named groups: their number and name, in pattern order.
    names: Vec<(usize, Vec<u8>)>,
}

/// Byte ranges of the groups of a match (index 0 is the whole match).
pub type Groups = Vec<Option<(usize, usize)>>;

impl Pcre {
    /// Compiles a delimited pattern, e.g. `/\.php$/i`.
    pub fn new(pattern: &[u8]) -> Result<Self, PcreError> {
        let (body, modifiers) = split_delimiters(pattern)?;
        let mut flags = Flags::default();
        let mut anchored = false;
        let mut utf8 = false;
        let mut dollar_endonly = false;
        for &m in modifiers {
            match m {
                b'i' => flags.i = true,
                b'm' => flags.m = true,
                b's' => flags.s = true,
                b'x' => flags.x = true,
                b'U' => flags.ungreedy = true,
                b'n' => flags.no_capture = true,
                b'A' => anchored = true,
                b'D' => dollar_endonly = true,
                b'u' => utf8 = true,
                b'S' | b'X' | b' ' | b'\n' | b'\r' => {}
                b'e' => {
                    return Err(PcreError::Compile(
                        "The /e modifier is no longer supported".into(),
                    ));
                }
                b'J' | b'r' => {
                    return Err(PcreError::Unsupported(format!("modifier '{}'", m as char)));
                }
                0 => return Err(PcreError::Compile("NUL is not a valid modifier".into())),
                _ => {
                    return Err(PcreError::Compile(format!(
                        "Unknown modifier '{}'",
                        m as char
                    )));
                }
            }
        }
        if utf8 && std::str::from_utf8(body).is_err() {
            return Err(PcreError::Compile(
                "UTF-8 error: the pattern is not valid UTF-8".into(),
            ));
        }
        // fancy-regex parses at most 64 nested groups, PCRE 250
        let flattened = if flags.x { None } else { nesting::reduce(body) };
        let body = flattened.as_deref().unwrap_or(body);
        let mut t = Translator {
            p: body,
            pos: 0,
            utf8,
            dollar_endonly,
            out: String::new(),
            flags,
            stack: Vec::new(),
            prev: Prev::None,
            captures: 0,
            names: Vec::new(),
        };
        let start = t.flag_prefix();
        t.out.push_str(&start);
        if anchored {
            t.out.push_str(r"\A(?:");
        }
        t.translate()?;
        if anchored {
            t.out.push(')');
        }
        let names = t.names;
        let translated = t.out;
        let mut builder = RegexBuilder::new(&translated);
        builder.bytes_mode(if utf8 {
            BytesMode::Unicode
        } else {
            BytesMode::Ascii
        });
        builder.backtrack_limit(1_000_000);
        let regex = builder
            .build()
            .map_err(|e| PcreError::Compile(e.to_string()))?;
        Ok(Self {
            regex,
            utf8,
            translated,
            names,
        })
    }

    /// The number of groups, the whole match included.
    pub fn group_count(&self) -> usize {
        self.regex.captures_len()
    }

    /// Each group's name, indexed by group number (index 0, the whole
    /// match, has none): `preg_match()` reports a named group under its
    /// name and then under its number.
    pub fn group_names(&self) -> Vec<Option<&[u8]>> {
        let mut names = vec![None; self.group_count()];
        for (number, name) in &self.names {
            if let Some(slot) = names.get_mut(*number) {
                *slot = Some(name.as_slice());
            }
        }
        names
    }

    /// The `fancy-regex` pattern this was translated to.
    pub fn translated(&self) -> &str {
        &self.translated
    }

    /// `preg_match($pattern, $subject)`: `Ok(true)` for 1, `Ok(false)` for
    /// 0, `Err` for `false`.
    pub fn is_match(&self, subject: &[u8]) -> Result<bool, MatchError> {
        Ok(self.captures_from(subject, 0)?.is_some())
    }

    /// The groups of the first match at or after `offset`.
    pub fn captures_from(
        &self,
        subject: &[u8],
        offset: usize,
    ) -> Result<Option<Groups>, MatchError> {
        let runtime = |e: fancy_regex::Error| MatchError(e.to_string());
        if self.utf8 {
            let text = std::str::from_utf8(subject)
                .map_err(|_| MatchError("Malformed UTF-8 characters".into()))?;
            let Some(caps) = self
                .regex
                .captures_from_pos(text, offset)
                .map_err(runtime)?
            else {
                return Ok(None);
            };
            Ok(Some(
                (0..caps.len())
                    .map(|i| caps.get(i).map(|m| (m.start(), m.end())))
                    .collect(),
            ))
        } else {
            let Some(caps) = self
                .regex
                .captures_from_pos(subject, offset)
                .map_err(runtime)?
            else {
                return Ok(None);
            };
            Ok(Some(
                (0..caps.len())
                    .map(|i| caps.get(i).map(|m| (m.start(), m.end())))
                    .collect(),
            ))
        }
    }

    /// `preg_replace_callback($pattern, $callback, $subject)`.
    pub fn replace_callback(
        &self,
        subject: &[u8],
        mut callback: impl FnMut(&[Option<&[u8]>]) -> Vec<u8>,
    ) -> Result<Vec<u8>, MatchError> {
        let mut out = Vec::with_capacity(subject.len());
        let mut last = 0;
        let mut pos = 0;
        while pos <= subject.len() {
            let Some(groups) = self.captures_from(subject, pos)? else {
                break;
            };
            let Some((start, end)) = groups[0] else {
                break;
            };
            let texts: Vec<Option<&[u8]>> = groups
                .iter()
                .map(|g| g.map(|(s, e)| &subject[s..e]))
                .collect();
            out.extend_from_slice(&subject[last..start]);
            out.extend(callback(&texts));
            last = end;
            pos = if end == start { end + 1 } else { end };
        }
        out.extend_from_slice(&subject[last.min(subject.len())..]);
        Ok(out)
    }

    /// `preg_replace($pattern, $replacement, $subject)` with `$n`
    /// references in the replacement.
    pub fn replace(&self, subject: &[u8], replacement: &[u8]) -> Result<Vec<u8>, MatchError> {
        self.replace_callback(subject, |groups| {
            let mut out = Vec::new();
            let mut i = 0;
            while i < replacement.len() {
                let c = replacement[i];
                if (c == b'$' || c == b'\\')
                    && replacement.get(i + 1).is_some_and(u8::is_ascii_digit)
                {
                    let mut j = i + 1;
                    let mut n = 0usize;
                    while j < replacement.len() && j < i + 3 && replacement[j].is_ascii_digit() {
                        n = n * 10 + usize::from(replacement[j] - b'0');
                        j += 1;
                    }
                    if let Some(Some(g)) = groups.get(n) {
                        out.extend_from_slice(g);
                    }
                    i = j;
                } else {
                    out.push(c);
                    i += 1;
                }
            }
            out
        })
    }
}

/// `preg_quote($str, $delimiter)`.
pub fn preg_quote(s: &[u8], delimiter: Option<u8>) -> Vec<u8> {
    let mut out = Vec::with_capacity(s.len());
    for &c in s {
        match c {
            0 => out.extend_from_slice(b"\\000"),
            b'.' | b'\\' | b'+' | b'*' | b'?' | b'[' | b'^' | b']' | b'$' | b'(' | b')' | b'{'
            | b'}' | b'=' | b'!' | b'<' | b'>' | b'|' | b':' | b'-' | b'#' => {
                out.push(b'\\');
                out.push(c);
            }
            _ if Some(c) == delimiter => {
                out.push(b'\\');
                out.push(c);
            }
            _ => out.push(c),
        }
    }
    out
}

fn is_space(c: u8) -> bool {
    matches!(c, b' ' | b'\t' | b'\n' | 0x0b | 0x0c | b'\r')
}

/// The pattern between the delimiters, and the modifiers after them.
fn split_delimiters(pattern: &[u8]) -> Result<(&[u8], &[u8]), PcreError> {
    let start = pattern.iter().take_while(|&&c| is_space(c)).count();
    let p = &pattern[start..];
    let Some(&delimiter) = p.first() else {
        return Err(PcreError::Compile("Empty regular expression".into()));
    };
    if delimiter.is_ascii_alphanumeric() || delimiter == b'\\' || delimiter == 0 {
        return Err(PcreError::Compile(
            "Delimiter must not be alphanumeric, backslash, or NUL byte".into(),
        ));
    }
    let closing = match delimiter {
        b'(' => Some(b')'),
        b'[' => Some(b']'),
        b'{' => Some(b'}'),
        b'<' => Some(b'>'),
        _ => None,
    };
    let mut i = 1;
    match closing {
        None => {
            while i < p.len() && p[i] != delimiter {
                if p[i] == b'\\' && i + 1 < p.len() {
                    i += 1;
                }
                i += 1;
            }
            if i >= p.len() {
                return Err(PcreError::Compile(format!(
                    "No ending delimiter '{}' found",
                    delimiter as char
                )));
            }
        }
        Some(end) => {
            let mut depth = 1;
            while i < p.len() {
                if p[i] == b'\\' && i + 1 < p.len() {
                    i += 2;
                    continue;
                }
                if p[i] == end {
                    depth -= 1;
                    if depth == 0 {
                        break;
                    }
                } else if p[i] == delimiter {
                    depth += 1;
                }
                i += 1;
            }
            if i >= p.len() {
                return Err(PcreError::Compile(format!(
                    "No ending matching delimiter '{}' found",
                    end as char
                )));
            }
        }
    }
    Ok((&p[1..i], &p[i + 1..]))
}

/// The capturing groups of a pattern body (plain and named), for telling
/// a back reference from an octal escape.
fn count_groups(p: &[u8]) -> usize {
    let mut n = 0;
    let mut i = 0;
    let mut in_class = false;
    while i < p.len() {
        match p[i] {
            b'\\' => i += 1,
            b'[' if !in_class => {
                in_class = true;
                if p.get(i + 1) == Some(&b'^') {
                    i += 1;
                }
                if p.get(i + 1) == Some(&b']') {
                    i += 1;
                }
            }
            b']' if in_class => in_class = false,
            b'(' if !in_class => {
                let rest = &p[i + 1..];
                if !rest.starts_with(b"?") && !rest.starts_with(b"*")
                    || (rest.starts_with(b"?<")
                        && !rest.starts_with(b"?<=")
                        && !rest.starts_with(b"?<!"))
                    || rest.starts_with(b"?P<")
                    || rest.starts_with(b"?'")
                {
                    n += 1;
                }
            }
            _ => {}
        }
        i += 1;
    }
    n
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
#[expect(
    clippy::struct_excessive_bools,
    reason = "one field per PCRE modifier letter"
)]
struct Flags {
    i: bool,
    m: bool,
    s: bool,
    x: bool,
    ungreedy: bool,
    no_capture: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Prev {
    /// Start of the pattern, a group or an alternative: nothing to repeat.
    None,
    /// A repeatable item.
    Atom,
    /// A quantified item or an assertion: a quantifier here is an error.
    NotRepeatable,
}

struct Translator<'a> {
    p: &'a [u8],
    pos: usize,
    utf8: bool,
    dollar_endonly: bool,
    out: String,
    flags: Flags,
    /// The flags to restore at each open group's `)`.
    stack: Vec<Flags>,
    prev: Prev,
    captures: usize,
    /// The named groups: their number and name.
    names: Vec<(usize, Vec<u8>)>,
}

/// What a non-ASCII character is in a group name under `/u`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum NameChar {
    /// `\p{L}`.
    Letter,
    /// `\p{Nd}`.
    Digit,
}

/// `[\p{L}]` and `[\p{Nd}]`, from fancy-regex's Unicode tables.
static NAME_CLASSES: std::sync::LazyLock<Option<(fancy_regex::Regex, fancy_regex::Regex)>> =
    std::sync::LazyLock::new(|| {
        Some((
            fancy_regex::Regex::new(r"^\p{L}$").ok()?,
            fancy_regex::Regex::new(r"^\p{Nd}$").ok()?,
        ))
    });

/// The length and class of the UTF-8 character at the start of `bytes`,
/// if it may appear in a group name.
fn name_char(bytes: &[u8]) -> Option<(usize, NameChar)> {
    let len = match bytes.first()? {
        0xc0..=0xdf => 2,
        0xe0..=0xef => 3,
        0xf0..=0xf7 => 4,
        _ => return None,
    };
    let c = std::str::from_utf8(bytes.get(..len)?).ok()?;
    let (letter, digit) = NAME_CLASSES.as_ref()?;
    if letter.is_match(c).unwrap_or(false) {
        Some((len, NameChar::Letter))
    } else if digit.is_match(c).unwrap_or(false) {
        Some((len, NameChar::Digit))
    } else {
        None
    }
}

/// One item of a character class.
enum ClassItem {
    Char(u32),
    /// Emitted as is (`\d`, `[:alpha:]`, an expanded set).
    Raw(String),
}

fn compile_err<T>(m: &str) -> Result<T, PcreError> {
    Err(PcreError::Compile(m.into()))
}

fn unsupported<T>(m: &str) -> Result<T, PcreError> {
    Err(PcreError::Unsupported(m.into()))
}

const SPACE_U: &str =
    r"\t-\r \x{85}\x{A0}\x{1680}\x{180E}\x{2000}-\x{200A}\x{2028}\x{2029}\x{202F}\x{205F}\x{3000}";
const HSPACE_B: &str = r"\t\x20\xA0";
const HSPACE_U: &str = r"\t\x20\x{A0}\x{1680}\x{180E}\x{2000}-\x{200A}\x{202F}\x{205F}\x{3000}";
const VSPACE_B: &str = r"\n\x0B\x0C\r\x85";
const VSPACE_U: &str = r"\n\x0B\x0C\r\x{85}\x{2028}\x{2029}";

impl Translator<'_> {
    fn peek(&self) -> Option<u8> {
        self.p.get(self.pos).copied()
    }

    fn peek_at(&self, n: usize) -> Option<u8> {
        self.p.get(self.pos + n).copied()
    }

    fn flag_prefix(&self) -> String {
        let mut on = String::new();
        if self.flags.i {
            on.push('i');
        }
        if self.flags.s {
            on.push('s');
        }
        if on.is_empty() {
            String::new()
        } else {
            format!("(?{on})")
        }
    }

    /// The next literal character (a byte, or a UTF-8 character with `/u`).
    fn take_char(&mut self) -> u32 {
        if self.utf8 {
            let rest = &self.p[self.pos..];
            let len = match rest[0] {
                0x00..=0x7f => 1,
                0xc0..=0xdf => 2,
                0xe0..=0xef => 3,
                _ => 4,
            }
            .min(rest.len());
            let c = std::str::from_utf8(&rest[..len])
                .ok()
                .and_then(|s| s.chars().next())
                .map_or(0xfffd, u32::from);
            self.pos += len;
            c
        } else {
            let c = self.p[self.pos];
            self.pos += 1;
            u32::from(c)
        }
    }

    fn lit(&self, c: u32) -> String {
        match char::from_u32(c) {
            Some(ch) if ch.is_ascii_alphanumeric() || ch == '_' => ch.to_string(),
            _ if c <= 0xff && !self.utf8 => format!(r"\x{c:02X}"),
            _ => format!(r"\x{{{c:X}}}"),
        }
    }

    fn skip_extended(&mut self) {
        if !self.flags.x {
            return;
        }
        while let Some(c) = self.peek() {
            if is_space(c) {
                self.pos += 1;
            } else if c == b'#' {
                while let Some(c) = self.peek() {
                    self.pos += 1;
                    if c == b'\n' {
                        break;
                    }
                }
            } else {
                break;
            }
        }
    }

    fn translate(&mut self) -> Result<(), PcreError> {
        loop {
            self.skip_extended();
            let Some(c) = self.peek() else { break };
            match c {
                b'\\' => self.escape()?,
                b'[' => self.class()?,
                b'(' => self.group_open()?,
                b')' => {
                    self.pos += 1;
                    let Some(saved) = self.stack.pop() else {
                        return compile_err("unmatched closing parenthesis");
                    };
                    self.flags = saved;
                    self.out.push(')');
                    self.prev = Prev::Atom;
                }
                b'|' => {
                    self.pos += 1;
                    self.out.push('|');
                    self.prev = Prev::None;
                }
                b'^' => {
                    self.pos += 1;
                    self.out.push_str(if self.flags.m {
                        r"(?:\A|(?<=\n)(?!\z))"
                    } else {
                        r"\A"
                    });
                    self.prev = Prev::NotRepeatable;
                }
                b'$' => {
                    self.pos += 1;
                    self.out.push_str(if self.flags.m {
                        r"(?=\n|\z)"
                    } else if self.dollar_endonly {
                        r"\z"
                    } else {
                        r"(?=\n?\z)"
                    });
                    self.prev = Prev::NotRepeatable;
                }
                b'.' => {
                    self.pos += 1;
                    self.out.push('.');
                    self.prev = Prev::Atom;
                }
                b'*' | b'+' | b'?' => {
                    self.pos += 1;
                    let q = (c as char).to_string();
                    self.quantifier(&q)?;
                }
                b'{' => {
                    if let Some(q) = self.brace_quantifier()? {
                        self.quantifier(&q)?;
                    } else {
                        self.pos += 1;
                        self.out.push_str(r"\x7B");
                        self.prev = Prev::Atom;
                    }
                }
                _ => {
                    let ch = self.take_char();
                    let l = self.lit(ch);
                    self.out.push_str(&l);
                    self.prev = Prev::Atom;
                }
            }
        }
        if !self.stack.is_empty() {
            return compile_err("missing closing parenthesis");
        }
        Ok(())
    }

    /// `{n}`, `{n,}`, `{n,m}` or `{,m}` at the cursor (consumed), or None
    /// when the `{` is a literal.
    fn brace_quantifier(&mut self) -> Result<Option<String>, PcreError> {
        let rest = &self.p[self.pos + 1..];
        let Some(close) = rest.iter().position(|&c| c == b'}') else {
            return Ok(None);
        };
        let inner = &rest[..close];
        let parse = |s: &[u8]| -> Option<u32> {
            if s.is_empty() || !s.iter().all(u8::is_ascii_digit) {
                return None;
            }
            std::str::from_utf8(s).ok()?.parse().ok()
        };
        let (min, max) = match inner.iter().position(|&c| c == b',') {
            None => match parse(inner) {
                Some(n) => (n, Some(n)),
                None => return Ok(None),
            },
            Some(comma) => {
                let (a, b) = (&inner[..comma], &inner[comma + 1..]);
                match (a.is_empty(), b.is_empty()) {
                    (true, true) => return Ok(None),
                    (true, false) => match parse(b) {
                        Some(m) => (0, Some(m)),
                        None => return Ok(None),
                    },
                    (false, true) => match parse(a) {
                        Some(n) => (n, None),
                        None => return Ok(None),
                    },
                    (false, false) => match (parse(a), parse(b)) {
                        (Some(n), Some(m)) => (n, Some(m)),
                        _ => return Ok(None),
                    },
                }
            }
        };
        if min > 65535 || max.is_some_and(|m| m > 65535) {
            return compile_err("number too big in {} quantifier");
        }
        if max.is_some_and(|m| m < min) {
            return compile_err("numbers out of order in {} quantifier");
        }
        self.pos += close + 2;
        Ok(Some(match max {
            Some(m) if m == min => format!("{{{min}}}"),
            Some(m) => format!("{{{min},{m}}}"),
            None => format!("{{{min},}}"),
        }))
    }

    fn quantifier(&mut self, q: &str) -> Result<(), PcreError> {
        if self.prev != Prev::Atom {
            return compile_err("quantifier does not follow a repeatable item");
        }
        self.out.push_str(q);
        let mut lazy = self.flags.ungreedy;
        match self.peek() {
            Some(b'?') => {
                self.pos += 1;
                lazy = !lazy;
                if lazy {
                    self.out.push('?');
                }
            }
            Some(b'+') => {
                self.pos += 1;
                self.out.push('+');
                lazy = false;
            }
            _ => {
                if lazy {
                    self.out.push('?');
                }
            }
        }
        let _ = lazy;
        self.prev = Prev::NotRepeatable;
        Ok(())
    }

    /// Reads `\x..`, `\o{..}`, `\0..`, `\c.` and the single-letter escapes
    /// that stand for one character. The cursor is after the backslash.
    fn escaped_char(&mut self, c: u8) -> Result<Option<u32>, PcreError> {
        Ok(Some(match c {
            b'a' => 0x07,
            b'e' => 0x1b,
            b'f' => 0x0c,
            b'n' => 0x0a,
            b'r' => 0x0d,
            b't' => 0x09,
            b'x' => {
                if self.peek() == Some(b'{') {
                    let rest = &self.p[self.pos + 1..];
                    let Some(close) = rest.iter().position(|&c| c == b'}') else {
                        return compile_err("missing terminating } in \\x{}");
                    };
                    let hex = &rest[..close];
                    if hex.is_empty() || !hex.iter().all(u8::is_ascii_hexdigit) {
                        return compile_err("non-hex character in \\x{}");
                    }
                    let v = u32::from_str_radix(std::str::from_utf8(hex).unwrap_or("0"), 16)
                        .unwrap_or(u32::MAX);
                    self.pos += close + 2;
                    self.check_code_point(v)?
                } else {
                    let mut v = 0;
                    for _ in 0..2 {
                        match self.peek() {
                            Some(h) if h.is_ascii_hexdigit() => {
                                v = v * 16 + (h as char).to_digit(16).unwrap_or(0);
                                self.pos += 1;
                            }
                            _ => break,
                        }
                    }
                    v
                }
            }
            b'o' => {
                if self.peek() != Some(b'{') {
                    return compile_err("missing opening brace after \\o");
                }
                let rest = &self.p[self.pos + 1..];
                let Some(close) = rest.iter().position(|&c| c == b'}') else {
                    return compile_err("missing terminating } in \\o{}");
                };
                let oct = &rest[..close];
                if oct.is_empty() || !oct.iter().all(|c| (b'0'..=b'7').contains(c)) {
                    return compile_err("non-octal character in \\o{}");
                }
                let v = u32::from_str_radix(std::str::from_utf8(oct).unwrap_or("0"), 8)
                    .unwrap_or(u32::MAX);
                self.pos += close + 2;
                self.check_code_point(v)?
            }
            b'0' => {
                let mut v = 0;
                for _ in 0..2 {
                    match self.peek() {
                        Some(d @ b'0'..=b'7') => {
                            v = v * 8 + u32::from(d - b'0');
                            self.pos += 1;
                        }
                        _ => break,
                    }
                }
                v
            }
            b'c' => {
                let Some(x) = self.peek() else {
                    return compile_err("\\c at end of pattern");
                };
                if !x.is_ascii() {
                    return compile_err("\\c must be followed by a printable ASCII character");
                }
                self.pos += 1;
                u32::from(x.to_ascii_uppercase() ^ 0x40)
            }
            _ => return Ok(None),
        }))
    }

    fn check_code_point(&self, v: u32) -> Result<u32, PcreError> {
        if (!self.utf8 && v > 0xff)
            || v > 0x0010_ffff
            || (self.utf8 && (0xd800..=0xdfff).contains(&v))
        {
            return compile_err("character code point value in \\x{} or \\o{} is too large");
        }
        Ok(v)
    }

    fn shorthand(&self, c: u8) -> Option<String> {
        Some(match c {
            b's' if self.utf8 => format!("[{SPACE_U}]"),
            b'S' if self.utf8 => format!("[^{SPACE_U}]"),
            b'd' | b'D' | b'w' | b'W' | b's' | b'S' => format!("\\{}", c as char),
            b'h' => format!("[{}]", if self.utf8 { HSPACE_U } else { HSPACE_B }),
            b'H' => format!("[^{}]", if self.utf8 { HSPACE_U } else { HSPACE_B }),
            b'v' => format!("[{}]", if self.utf8 { VSPACE_U } else { VSPACE_B }),
            b'V' => format!("[^{}]", if self.utf8 { VSPACE_U } else { VSPACE_B }),
            _ => return None,
        })
    }

    /// The contents to put in a class for a shorthand (`\d` or `[...]`).
    fn class_shorthand(&self, c: u8) -> Option<String> {
        // a negated set cannot be nested as a range list; it stays a class
        self.shorthand(c)
    }

    #[expect(
        clippy::too_many_lines,
        reason = "one arm per PCRE escape letter, kept as a single table"
    )]
    fn escape(&mut self) -> Result<(), PcreError> {
        self.pos += 1;
        let Some(c) = self.peek() else {
            return compile_err("\\ at end of pattern");
        };
        self.pos += 1;
        if let Some(s) = self.shorthand(c) {
            self.out.push_str(&s);
            self.prev = Prev::Atom;
            return Ok(());
        }
        match c {
            b'b' | b'B' => {
                self.out.push('\\');
                self.out.push(c as char);
                self.prev = Prev::NotRepeatable;
            }
            b'A' | b'G' => {
                self.out.push_str(r"\A");
                self.prev = Prev::NotRepeatable;
            }
            b'z' => {
                self.out.push_str(r"\z");
                self.prev = Prev::NotRepeatable;
            }
            b'Z' => {
                self.out.push_str(r"(?=\n?\z)");
                self.prev = Prev::NotRepeatable;
            }
            b'R' => {
                self.out.push_str(if self.utf8 {
                    r"(?>\r\n|[\n\x0B\x0C\r\x{85}\x{2028}\x{2029}])"
                } else {
                    r"(?>\r\n|[\n\x0B\x0C\r\x85])"
                });
                self.prev = Prev::Atom;
            }
            b'N' => {
                if self.peek() == Some(b'{') {
                    return unsupported("\\N{...}");
                }
                self.out.push_str(r"[^\n]");
                self.prev = Prev::Atom;
            }
            b'Q' => {
                let rest = &self.p[self.pos..];
                let end = rest
                    .windows(2)
                    .position(|w| w == b"\\E")
                    .unwrap_or(rest.len());
                let stop = self.pos + end;
                while self.pos < stop {
                    let ch = self.take_char();
                    let l = self.lit(ch);
                    self.out.push_str(&l);
                    self.prev = Prev::Atom;
                }
                self.pos = (stop + 2).min(self.p.len());
            }
            b'E' => {}
            b'1'..=b'9' => {
                let start = self.pos;
                let mut n = u32::from(c - b'0');
                while let Some(d @ b'0'..=b'9') = self.peek() {
                    n = n * 10 + u32::from(d - b'0');
                    self.pos += 1;
                }
                if n >= 10 && n as usize > count_groups(self.p) {
                    // not a group number: up to three octal digits
                    self.pos = start - 1;
                    if !(b'1'..=b'7').contains(&c) {
                        return compile_err("a numbered reference must not be zero");
                    }
                    let mut v = 0u32;
                    for _ in 0..3 {
                        match self.peek() {
                            Some(d @ b'0'..=b'7') => {
                                v = v * 8 + u32::from(d - b'0');
                                self.pos += 1;
                            }
                            _ => break,
                        }
                    }
                    let v = self.check_code_point(v)?;
                    let l = self.lit(v);
                    self.out.push_str(&l);
                } else {
                    let _ = write!(self.out, "\\{n}");
                }
                self.prev = Prev::Atom;
            }
            b'g' => {
                let n = self.group_reference()?;
                let _ = write!(self.out, "\\{n}");
                self.prev = Prev::Atom;
            }
            b'k' => {
                let close = match self.peek() {
                    Some(b'<') => b'>',
                    Some(b'\'') => b'\'',
                    Some(b'{') => b'}',
                    _ => return compile_err("\\k is not followed by a group name"),
                };
                self.pos += 1;
                let name = self.group_name(close)?;
                let _ = write!(self.out, "\\k<{}>", String::from_utf8_lossy(&name));
                self.prev = Prev::Atom;
            }
            b'p' | b'P' => {
                if !self.utf8 {
                    return unsupported("\\p without /u");
                }
                let start = self.pos;
                if self.peek() == Some(b'{') {
                    let Some(close) = self.p[self.pos..].iter().position(|&c| c == b'}') else {
                        return compile_err("malformed \\P or \\p sequence");
                    };
                    self.pos += close + 1;
                } else {
                    self.pos += 1;
                }
                let prop = String::from_utf8_lossy(&self.p[start..self.pos.min(self.p.len())]);
                let _ = write!(self.out, "\\{}{}", c as char, prop);
                self.prev = Prev::Atom;
            }
            b'K' | b'X' | b'C' => return unsupported(&format!("\\{}", c as char)),
            _ => match self.escaped_char(c)? {
                Some(v) => {
                    let l = self.lit(v);
                    self.out.push_str(&l);
                    self.prev = Prev::Atom;
                }
                None if c.is_ascii_alphanumeric() => {
                    return compile_err("unrecognized character follows \\");
                }
                None => {
                    // a non-alphanumeric character stands for itself
                    self.pos -= 1;
                    let ch = self.take_char();
                    let l = self.lit(ch);
                    self.out.push_str(&l);
                    self.prev = Prev::Atom;
                }
            },
        }
        Ok(())
    }

    /// `\g` references to numbered groups: `\gN`, `\g{N}`, `\g{-N}`.
    fn group_reference(&mut self) -> Result<usize, PcreError> {
        let braced = self.peek() == Some(b'{');
        if braced {
            self.pos += 1;
        }
        let negative = self.peek() == Some(b'-');
        if negative || self.peek() == Some(b'+') {
            self.pos += 1;
        }
        let start = self.pos;
        while self.peek().is_some_and(|c| c.is_ascii_digit()) {
            self.pos += 1;
        }
        if start == self.pos {
            return unsupported("\\g with a group name or \\g<...>");
        }
        let n: usize = std::str::from_utf8(&self.p[start..self.pos])
            .ok()
            .and_then(|s| s.parse().ok())
            .unwrap_or(0);
        if braced {
            if self.peek() != Some(b'}') {
                return compile_err(
                    "\\g is not followed by a braced, angle-bracketed, or quoted name/number or by a plain number",
                );
            }
            self.pos += 1;
        }
        let n = if negative {
            match (self.captures + 1).checked_sub(n) {
                Some(v) if n > 0 && v > 0 => v,
                _ => return compile_err("a relative value of zero is not allowed"),
            }
        } else {
            n
        };
        if n == 0 {
            return compile_err("a relative value of zero is not allowed");
        }
        Ok(n)
    }

    /// A group name: ASCII letters, digits and `_`; under `/u` also
    /// Unicode letters and decimal digits (PCRE2 10.44, as PHP 8.4.27
    /// bundles it). It does not start with a digit.
    fn group_name(&mut self, close: u8) -> Result<Vec<u8>, PcreError> {
        let start = self.pos;
        let mut first_is_digit = None;
        loop {
            let rest = &self.p[self.pos..];
            let (len, word, digit) = match rest.first() {
                Some(&c) if c.is_ascii() => (
                    1,
                    c.is_ascii_alphanumeric() || c == b'_',
                    c.is_ascii_digit(),
                ),
                Some(_) if self.utf8 => match name_char(rest) {
                    Some((len, class)) => (len, true, class == NameChar::Digit),
                    None => (0, false, false),
                },
                _ => (0, false, false),
            };
            if !word {
                break;
            }
            first_is_digit.get_or_insert(digit);
            self.pos += len;
        }
        let name = self.p[start..self.pos].to_vec();
        if name.is_empty() {
            return compile_err("group name expected");
        }
        if first_is_digit == Some(true) {
            return compile_err("subpattern name must start with a non-digit");
        }
        if name.len() > 32 {
            return compile_err("subpattern name is too long (maximum 32 code units)");
        }
        if self.peek() != Some(close) {
            return compile_err("syntax error in subpattern name (missing terminator?)");
        }
        self.pos += 1;
        Ok(name)
    }

    fn open(&mut self, text: &str) {
        self.stack.push(self.flags);
        self.out.push_str(text);
        self.prev = Prev::None;
    }

    fn group_open(&mut self) -> Result<(), PcreError> {
        self.pos += 1;
        match self.peek() {
            Some(b'*') => {
                if self
                    .peek_at(1)
                    .is_some_and(|c| c.is_ascii_uppercase() || c == b':')
                {
                    return unsupported("backtracking control verbs");
                }
                return compile_err("quantifier does not follow a repeatable item");
            }
            Some(b'?') => {}
            _ => {
                if self.flags.no_capture {
                    self.open("(?:");
                } else {
                    self.captures += 1;
                    self.open("(");
                }
                return Ok(());
            }
        }
        self.pos += 1;
        let Some(c) = self.peek() else {
            return compile_err("unrecognized character after (? or (?-");
        };
        match c {
            b'#' => {
                let Some(close) = self.p[self.pos..].iter().position(|&c| c == b')') else {
                    return compile_err("missing ) after (?# comment");
                };
                self.pos += close + 1;
            }
            b':' => {
                self.pos += 1;
                self.open("(?:");
            }
            b'=' | b'!' => {
                self.pos += 1;
                self.open(&format!("(?{}", c as char));
            }
            b'>' => {
                self.pos += 1;
                self.open("(?>");
            }
            b'|' => return unsupported("branch reset groups"),
            b'<' if matches!(self.peek_at(1), Some(b'=' | b'!')) => {
                let kind = self.peek_at(1).unwrap_or(b'=');
                self.pos += 2;
                self.open(&format!("(?<{}", kind as char));
            }
            b'<' | b'\'' => {
                self.pos += 1;
                let name = self.group_name(if c == b'<' { b'>' } else { b'\'' })?;
                self.named(name)?;
            }
            b'P' => match self.peek_at(1) {
                Some(b'<') => {
                    self.pos += 2;
                    let name = self.group_name(b'>')?;
                    self.named(name)?;
                }
                Some(b'=') => {
                    self.pos += 2;
                    let name = self.group_name(b')')?;
                    let _ = write!(self.out, "\\k<{}>", String::from_utf8_lossy(&name));
                    self.prev = Prev::Atom;
                }
                Some(b'>') => return unsupported("recursion"),
                _ => return compile_err("unrecognized character after (?P"),
            },
            b'R' | b'&' | b'+' | b'0'..=b'9' => return unsupported("recursion"),
            b'(' => return unsupported("conditional groups"),
            b'C' => return unsupported("callouts"),
            b'^' => return unsupported("(?^ flag reset"),
            _ => self.inline_flags()?,
        }
        Ok(())
    }

    fn named(&mut self, name: Vec<u8>) -> Result<(), PcreError> {
        if self.names.iter().any(|(_, n)| *n == name) {
            return compile_err(
                "two named subpatterns have the same name (PCRE2_DUPNAMES not set)",
            );
        }
        self.captures += 1;
        let text = format!("(?P<{}>", String::from_utf8_lossy(&name));
        self.names.push((self.captures, name));
        self.open(&text);
        Ok(())
    }

    fn inline_flags(&mut self) -> Result<(), PcreError> {
        let mut flags = self.flags;
        let mut negate = false;
        loop {
            let Some(c) = self.peek() else {
                return compile_err("missing closing parenthesis");
            };
            self.pos += 1;
            match c {
                b'-' if !negate => negate = true,
                b'i' => flags.i = !negate,
                b'm' => flags.m = !negate,
                b's' => flags.s = !negate,
                b'x' => flags.x = !negate,
                b'U' => flags.ungreedy = !negate,
                b'n' => flags.no_capture = !negate,
                b'J' => return unsupported("(?J)"),
                b')' | b':' => {
                    let mut on = String::new();
                    let mut off = String::new();
                    for (was, now, letter) in
                        [(self.flags.i, flags.i, 'i'), (self.flags.s, flags.s, 's')]
                    {
                        if was != now {
                            if now {
                                on.push(letter);
                            } else {
                                off.push(letter);
                            }
                        }
                    }
                    let mut spec = on;
                    if !off.is_empty() {
                        spec.push('-');
                        spec.push_str(&off);
                    }
                    if c == b')' {
                        if !spec.is_empty() {
                            let _ = write!(self.out, "(?{spec})");
                        }
                        self.flags = flags;
                        // an option setting leaves nothing to repeat
                        if self.prev == Prev::Atom {
                            self.prev = Prev::NotRepeatable;
                        }
                    } else {
                        self.open(&format!("(?{spec}:"));
                        self.flags = flags;
                    }
                    return Ok(());
                }
                _ => return compile_err("unrecognized character after (? or (?-"),
            }
        }
    }

    fn class(&mut self) -> Result<(), PcreError> {
        self.pos += 1;
        let negated = self.peek() == Some(b'^');
        if negated {
            self.pos += 1;
        }
        let mut items: Vec<ClassItem> = Vec::new();
        let mut first = true;
        loop {
            let Some(c) = self.peek() else {
                return compile_err("missing terminating ] for character class");
            };
            if c == b']' && !first {
                self.pos += 1;
                break;
            }
            first = false;
            let item = self.class_item()?;
            let Some(item) = item else { continue };
            // a range?
            if let ClassItem::Char(from) = item {
                if self.peek() == Some(b'-') && self.peek_at(1).is_some_and(|c| c != b']') {
                    let save = self.pos;
                    self.pos += 1;
                    match self.class_item()? {
                        Some(ClassItem::Char(to)) => {
                            if to < from {
                                return compile_err("range out of order in character class");
                            }
                            items.push(ClassItem::Raw(format!(
                                "{}-{}",
                                self.class_lit(from),
                                self.class_lit(to)
                            )));
                            continue;
                        }
                        Some(ClassItem::Raw(_)) => {
                            return compile_err("invalid range in character class");
                        }
                        None => {
                            self.pos = save;
                        }
                    }
                }
            } else if self.peek() == Some(b'-') && self.peek_at(1).is_some_and(|c| c != b']') {
                return compile_err("invalid range in character class");
            }
            items.push(item);
        }
        let mut out = String::from("[");
        if negated {
            out.push('^');
        }
        if items.is_empty() {
            return compile_err("missing terminating ] for character class");
        }
        for item in items {
            match item {
                ClassItem::Char(c) => out.push_str(&self.class_lit(c)),
                ClassItem::Raw(s) => out.push_str(&s),
            }
        }
        out.push(']');
        if self.flags.i {
            // fancy-regex folds case inside classes the same way
        }
        self.out.push_str(&out);
        self.prev = Prev::Atom;
        Ok(())
    }

    fn class_lit(&self, c: u32) -> String {
        if c <= 0xff && !self.utf8 {
            format!(r"\x{c:02X}")
        } else {
            format!(r"\x{{{c:X}}}")
        }
    }

    /// One item inside `[...]`, or None for something that adds nothing
    /// (`\E`).
    #[expect(
        clippy::too_many_lines,
        reason = "one arm per class item kind, kept as a single table"
    )]
    fn class_item(&mut self) -> Result<Option<ClassItem>, PcreError> {
        const NAMES: [&[u8]; 14] = [
            b"alpha", b"lower", b"upper", b"alnum", b"ascii", b"blank", b"cntrl", b"digit",
            b"graph", b"print", b"punct", b"space", b"word", b"xdigit",
        ];
        let c = self.p[self.pos];
        if c == b'[' {
            if let Some(next @ (b':' | b'.' | b'=')) = self.peek_at(1) {
                // a POSIX class needs its terminator before the class ends
                let rest = &self.p[self.pos + 2..];
                let end = rest.iter().position(|&c| c == b']');
                let term = rest.windows(2).position(|w| w[0] == next && w[1] == b']');
                if let Some(t) = term.filter(|&t| end.is_none_or(|e| t < e)) {
                    if next != b':' {
                        return compile_err("POSIX collating elements are not supported");
                    }
                    let mut name = &rest[..t];
                    let negate = name.first() == Some(&b'^');
                    if negate {
                        name = &name[1..];
                    }
                    if !NAMES.contains(&name) {
                        return compile_err("unknown POSIX class name");
                    }
                    if self.utf8 {
                        return unsupported("POSIX classes with /u");
                    }
                    self.pos += 2 + t + 2;
                    let name = String::from_utf8_lossy(name);
                    return Ok(Some(ClassItem::Raw(format!(
                        "[:{}{}:]",
                        if negate { "^" } else { "" },
                        name
                    ))));
                }
            }
            self.pos += 1;
            return Ok(Some(ClassItem::Char(u32::from(b'['))));
        }
        if c != b'\\' {
            return Ok(Some(ClassItem::Char(self.take_char())));
        }
        self.pos += 1;
        let Some(e) = self.peek() else {
            return compile_err("\\ at end of pattern");
        };
        self.pos += 1;
        if let Some(s) = self.class_shorthand(e) {
            let s = match s.strip_prefix('[') {
                // a positive set is spliced in; a negated one stays nested
                Some(inner) if !inner.starts_with('^') => {
                    inner.strip_suffix(']').unwrap_or(inner).to_string()
                }
                _ => s,
            };
            return Ok(Some(ClassItem::Raw(s)));
        }
        match e {
            b'b' => Ok(Some(ClassItem::Char(0x08))),
            b'E' => Ok(None),
            b'Q' => unsupported("\\Q inside a character class"),
            b'p' | b'P' => {
                // a Unicode property, passed through as outside a class
                if !self.utf8 {
                    return unsupported("\\p without /u");
                }
                let start = self.pos;
                if self.peek() == Some(b'{') {
                    let Some(close) = self.p[self.pos..].iter().position(|&c| c == b'}') else {
                        return compile_err("malformed \\P or \\p sequence");
                    };
                    self.pos += close + 1;
                } else {
                    self.pos += 1;
                }
                let prop = String::from_utf8_lossy(&self.p[start..self.pos.min(self.p.len())]);
                Ok(Some(ClassItem::Raw(format!("\\{}{}", e as char, prop))))
            }
            b'N' | b'R' | b'X' | b'B' | b'A' | b'z' | b'Z' | b'G' | b'K' | b'C' | b'g' | b'k' => {
                compile_err("escape sequence is invalid in character class")
            }
            b'1'..=b'7' => {
                // octal in a class
                let mut v = u32::from(e - b'0');
                for _ in 0..2 {
                    match self.peek() {
                        Some(d @ b'0'..=b'7') => {
                            v = v * 8 + u32::from(d - b'0');
                            self.pos += 1;
                        }
                        _ => break,
                    }
                }
                Ok(Some(ClassItem::Char(self.check_code_point(v)?)))
            }
            _ => match self.escaped_char(e)? {
                Some(v) => Ok(Some(ClassItem::Char(v))),
                None if e.is_ascii_alphanumeric() => {
                    compile_err("unrecognized character follows \\")
                }
                None => {
                    self.pos -= 1;
                    Ok(Some(ClassItem::Char(self.take_char())))
                }
            },
        }
    }
}

#[cfg(test)]
mod tests {
    use super::Pcre;

    #[test]
    fn group_names_follow_group_numbers() {
        let pcre = Pcre::new(b"#^/(x)?(?P<b>y)(?<c>z)?(?:w)('q')?(?'d'e)$#").unwrap();
        assert_eq!(
            pcre.group_names(),
            [
                None,
                None,
                Some(&b"b"[..]),
                Some(&b"c"[..]),
                None,
                Some(&b"d"[..])
            ]
        );
        let translated: Vec<Option<&str>> = pcre.regex.capture_names().collect();
        let names: Vec<Option<String>> = pcre
            .group_names()
            .into_iter()
            .map(|n| n.map(|n| String::from_utf8_lossy(n).into_owned()))
            .collect();
        assert_eq!(
            names,
            translated
                .iter()
                .map(|n| n.map(str::to_owned))
                .collect::<Vec<_>>()
        );
    }
}

#[cfg(test)]
mod group_name_tests {
    use super::Pcre;

    /// PHP 8.4.27 (PCRE2 10.44) on `{^/(?P<name>x)$}u`: compiles or not.
    #[test]
    fn unicode_group_names_follow_pcre2() {
        for (name, ok) in [
            ("bär", true),
            ("ä1", true),
            ("日本", true),
            ("ab٣", true),
            ("éa", true),
            ("a_é", true),
            ("_1", true),
            ("ǅ", true),
            ("aª", true),
            ("1ä", false),
            ("٣a", false),
            ("a·b", false),
            ("a½", false),
            ("aⅣ", false),
            ("Ⅳa", false),
            ("a\u{327}", false),
            ("a〇", false),
        ] {
            let pattern = format!("{{^/(?P<{name}>x)$}}u");
            assert_eq!(Pcre::new(pattern.as_bytes()).is_ok(), ok, "{name}");
        }
        // Without /u a name is ASCII.
        assert!(Pcre::new("{^/(?P<bär>x)$}".as_bytes()).is_err());
        let pcre = Pcre::new("{^/(?P<bär>x)$}u".as_bytes()).unwrap();
        assert_eq!(pcre.group_names(), [None, Some("bär".as_bytes())]);
    }
}
