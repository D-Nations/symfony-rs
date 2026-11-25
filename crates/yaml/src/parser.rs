//! Port of Symfony\Component\Yaml\Parser.php: YAML documents (block
//! mappings and sequences, block scalars, documents markers, anchors,
//! merge keys) to PHP values. Flow collections and scalars go to
//! [`Inline`].
//!
//! PHP's nested block parsers share the anchors (by reference), the
//! `ParserState` and Inline's static state with their parent; here those
//! are one [`Shared`] borrowed by every [`Block`] of a parse.
//!
//! Upstream's patterns are kept as `regex` patterns where the `regex`
//! crate gives the same match (PCRE's `$` becomes `\z` on the lines,
//! which never hold a newline). The few whose possessive quantifiers or
//! lookarounds change what they match (the mapping-key line, the
//! document markers and directives) are written as matchers with PCRE's
//! semantics, each next to the pattern it implements. Patterns with `/u`
//! see Unicode `\s` as PHP's PCRE does (`php_value::pcre`), and fail with
//! PCRE's message on bytes that are not UTF-8.
//!
//! Where PHP throws a `TypeError` (merging a scalar under `<<: &anchor`,
//! or a merge item that is an object), the port returns a
//! `ParseException` with the `TypeError`'s message, without its
//! "called in" suffix.
//!
//! Symfony source copyright (c) Fabien Potencier; distributed under MIT.
//!
//! Upstream: symfony/yaml Parser.php

use std::sync::{Arc, LazyLock};

use php_value::{
    Key, PhpArray, PhpString, Value,
    numeric::{string_to_float, string_to_int},
    object::{StdClass, to_array},
};
use regex::bytes::Regex;

use crate::{
    Environment, Flags,
    exception::{InvalidArgumentException, ParseException},
    inline::{Inline, References},
    parser_state::ParserState,
    tag::TaggedValue,
};

pub use crate::parser_state::{DEFAULT_MAX_ALIASES_FOR_COLLECTIONS, DEFAULT_MAX_NESTING_LEVEL};

/// `Parser::TAG_PATTERN`, as upstream writes it.
pub const TAG_PATTERN: &str = r"(?P<tag>![\w!.\/:-]+)";
/// `Parser::BLOCK_SCALAR_HEADER_PATTERN`, as upstream writes it.
pub const BLOCK_SCALAR_HEADER_PATTERN: &str =
    r"(?P<separator>\||>)(?P<modifiers>\+|\-|\d+|\+\d+|\-\d+|\d+\+|\d+\-)?(?P<comments>[ \t]+#.*)?";
/// `Parser::REFERENCE_PATTERN`, as upstream writes it.
pub const REFERENCE_PATTERN: &str = r"#^&(?P<ref>[^ ]++) *+(?P<value>.*)#u";

/// PCRE's message when a `/u` pattern meets bytes that are not UTF-8.
const MALFORMED_UTF8: &str = "Malformed UTF-8 characters, possibly incorrectly encoded";

fn regex(pattern: &str) -> Regex {
    Regex::new(pattern).unwrap_or_else(|e| unreachable!("constant pattern {pattern}: {e}"))
}

/// `\s` under `/u` (`php_value::pcre::is_space_u`), as a class body.
const SPACE_U: &str =
    r"\t-\r \x{85}\x{A0}\x{1680}\x{180E}\x{2000}-\x{200A}\x{2028}\x{2029}\x{202F}\x{205F}\x{3000}";
/// `TAG_PATTERN` without `/u` (`\w` is ASCII there).
const TAG: &str = r"(?P<tag>![\w!./:-]+)";
/// `BLOCK_SCALAR_HEADER_PATTERN` without `/u`.
const HEADER: &str =
    r"(?P<separator>\||>)(?P<modifiers>\+|-|\d+|\+\d+|-\d+|\d+\+|\d+-)?(?P<comments>[ \t]+#.*)?";

/// `#^\-((?P<leadspaces>\s+)(?P<value>.+))?$#u`
static SEQUENCE_ITEM: LazyLock<Regex> = LazyLock::new(|| {
    regex(&format!(
        r"^-(?:(?P<leadspaces>[{SPACE_U}]+)(?P<value>.+))?\z"
    ))
});
/// `#^(?P<key>REGEX_QUOTED_STRING|[^ '"\{\[].*?) *\:(\s+(?P<value>.+?))?\s*$#u`
static SEQUENCE_MAPPING: LazyLock<Regex> = LazyLock::new(|| {
    regex(&format!(
        r#"^(?:"[^"\\]*(?:\\.[^"\\]*)*"|'[^']*(?:''[^']*)*'|[^ '"\{{\[].*?) *:(?:[{SPACE_U}]+.+?)?[{SPACE_U}]*\z"#
    ))
});
/// `/^TAG_PATTERN[ \t]+BLOCK_SCALAR_HEADER_PATTERN$/`
static TAGGED_BLOCK_SCALAR: LazyLock<Regex> =
    LazyLock::new(|| regex(&format!(r"(?-u)^{TAG}[ \t]+{HEADER}\n?\z")));
/// `/^(?:TAG_PATTERN[ \t]+)?BLOCK_SCALAR_HEADER_PATTERN$/`
static BLOCK_SCALAR: LazyLock<Regex> =
    LazyLock::new(|| regex(&format!(r"(?-u)^(?:{TAG}[ \t]+)?{HEADER}\n?\z")));
/// `/:[ \t]+(?:TAG_PATTERN[ \t]+)?BLOCK_SCALAR_HEADER_PATTERN$/`
static ENDS_WITH_BLOCK_SCALAR: LazyLock<Regex> =
    LazyLock::new(|| regex(&format!(r"(?-u):[ \t]+(?:{TAG}[ \t]+)?{HEADER}\n?\z")));
/// `/^TAG_PATTERN *( +#.*)?$/`
static LINE_TAG: LazyLock<Regex> =
    LazyLock::new(|| regex(&format!(r"(?-u)^{TAG} *(?: +#.*)?\n?\z")));
/// `#^TAG_PATTERN ++(&[^ ]++)#u`: the spaces and the anchor are maximal
/// either way, as what follows each is not a space.
static TAG_THEN_ANCHOR: LazyLock<Regex> =
    LazyLock::new(|| regex(r"^(?P<tag>![\w!./:-]+) +(?P<anchor>&[^ ]+)"));
/// `REFERENCE_PATTERN`: `[^ ]++ *+` keeps the same split when greedy.
static REFERENCE: LazyLock<Regex> = LazyLock::new(|| regex(r"^&(?P<ref>[^ ]+) *(?P<value>.*)"));
/// `#^&(?P<ref>[^ ]+)#u`
static MERGE_REFERENCE: LazyLock<Regex> = LazyLock::new(|| regex(r"^&(?P<ref>[^ ]+)"));
/// `/[ \t]+#/`
static COMMENT: LazyLock<Regex> = LazyLock::new(|| regex(r"(?-u)[ \t]+#"));
/// `REGEX_QUOTED_STRING`, anchored, with `/u`.
static QUOTED: LazyLock<Regex> =
    LazyLock::new(|| regex(r#"^(?:"[^"\\]*(?:\\.[^"\\]*)*"|'[^']*(?:''[^']*)*')"#));

/// `self::preg_match()` with a `/u` pattern: PCRE's error on bytes that
/// are not UTF-8.
fn utf8(subject: &[u8]) -> Result<&str, ParseException> {
    std::str::from_utf8(subject).map_err(|_| ParseException::message_only(MALFORMED_UTF8))
}

/// PHP's default `trim()` characters.
fn is_trim(b: u8) -> bool {
    matches!(b, b' ' | b'\t' | b'\n' | b'\r' | 0 | 0x0b)
}

fn trim(s: &[u8]) -> &[u8] {
    ltrim(rtrim(s))
}

fn ltrim(s: &[u8]) -> &[u8] {
    let start = s.iter().position(|&b| !is_trim(b)).unwrap_or(s.len());
    &s[start..]
}

fn rtrim(s: &[u8]) -> &[u8] {
    let end = s.iter().rposition(|&b| !is_trim(b)).map_or(0, |p| p + 1);
    &s[..end]
}

fn trim_chars<'a>(s: &'a [u8], chars: &[u8]) -> &'a [u8] {
    ltrim_chars(rtrim_chars(s, chars), chars)
}

fn ltrim_chars<'a>(s: &'a [u8], chars: &[u8]) -> &'a [u8] {
    let start = s.iter().position(|b| !chars.contains(b)).unwrap_or(s.len());
    &s[start..]
}

fn rtrim_chars<'a>(s: &'a [u8], chars: &[u8]) -> &'a [u8] {
    let end = s
        .iter()
        .rposition(|b| !chars.contains(b))
        .map_or(0, |p| p + 1);
    &s[..end]
}

/// `strspn($s, $chars, $offset)`.
fn strspn(s: &[u8], chars: &[u8], offset: usize) -> usize {
    s.get(offset..)
        .unwrap_or(b"")
        .iter()
        .take_while(|b| chars.contains(b))
        .count()
}

/// `substr($s, $offset)` for a non-negative offset.
fn substr(s: &[u8], offset: usize) -> &[u8] {
    s.get(offset..).unwrap_or(b"")
}

fn contains(haystack: &[u8], needle: &[u8]) -> bool {
    haystack.windows(needle.len()).any(|w| w == needle)
}

fn lossy(s: &[u8]) -> std::borrow::Cow<'_, str> {
    String::from_utf8_lossy(s)
}

/// PHP truthiness of a string.
fn truthy(s: &[u8]) -> bool {
    !s.is_empty() && s != b"0"
}

fn string(s: &[u8]) -> Value {
    Value::String(PhpString::from(s))
}

fn tagged(tag: &str, value: Value) -> Value {
    Value::Object(Arc::new(TaggedValue::new(tag, value)))
}

fn is_std_class(value: &Value) -> bool {
    value
        .as_object()
        .is_some_and(|o| o.downcast_ref::<StdClass>().is_some())
}

/// `(array) $value` when `PARSE_OBJECT_FOR_MAP` made it a stdClass.
fn unless_object(value: Value, flags: Flags) -> Value {
    if flags.contains(Flags::PARSE_OBJECT_FOR_MAP) && is_std_class(&value) {
        Value::Array(to_array(&value))
    } else {
        value
    }
}

/// `$data += $value`: PHP's `TypeError` unless `$value` is an array.
fn union(data: &mut PhpArray, value: &Value) -> Result<(), ParseException> {
    match value {
        Value::Array(array) => {
            data.union(array);
            Ok(())
        }
        other => Err(ParseException::message_only(format!(
            "Unsupported operand types: array + {}",
            other.debug_type()
        ))),
    }
}

/// `isset($data[$key])`.
fn isset(data: &PhpArray, key: &Key) -> bool {
    data.get(key).is_some_and(|v| !v.is_null())
}

/// `$data[] = $value`.
fn push(data: &mut PhpArray, value: Value) -> Result<(), ParseException> {
    data.push(value)
        .map(|_| ())
        .map_err(|e| ParseException::message_only(e.to_string()))
}

/// `(?:[ \t]*+(?:\#[^\n]*+)?+\n)` at `i`: the end of a blank or comment
/// line, after its newline.
fn blank_line(s: &[u8], i: usize) -> Option<usize> {
    let mut j = i + strspn(s, b" \t", i);
    if s.get(j) == Some(&b'#') {
        j += s[j..].iter().take_while(|&&b| b != b'\n').count();
    }
    (s.get(j) == Some(&b'\n')).then_some(j + 1)
}

/// `(?:[ \t]*+(?:\#[^\n]*+)?+\n)*+` from `i`.
fn blank_lines(s: &[u8], mut i: usize) -> usize {
    while let Some(j) = blank_line(s, i) {
        i = j;
    }
    i
}

/// `#^(?:[ \t]*+(?:\#[^\n]*+)?+\n)*+(?:%YAML[: ][\d.]++[^\n]*+\n(?:[ \t]*+(?:\#[^\n]*+)?+\n)*+)?+#`:
/// the end of the leading blank and comment lines and the `%YAML` line.
fn leading_lines_and_directive(s: &[u8]) -> usize {
    let i = blank_lines(s, 0);
    let rest = &s[i..];
    if rest.starts_with(b"%YAML") && matches!(rest.get(5), Some(b':' | b' ')) {
        let digits = rest[6..]
            .iter()
            .take_while(|b| b.is_ascii_digit() || **b == b'.')
            .count();
        if digits > 0
            && let Some(eol) = rest[6 + digits..].iter().position(|&b| b == b'\n')
        {
            return blank_lines(s, i + 6 + digits + eol + 1);
        }
    }
    i
}

/// `(?=(?:\#[^\n]*+)?+(?:\n|\z))` at `i`.
fn followed_by_comment_or_eol(s: &[u8], i: usize) -> bool {
    match s.get(i) {
        None | Some(b'\n' | b'#') => true,
        Some(_) => false,
    }
}

/// `#^---(?:[ \t]++(?:%[^\n]*+|(?:(?:[!&][^ \t\n]*+|[|>][-+0-9]*+)[ \t]*+)++(?=(?:\#[^\n]*+)?+(?:\n|\z)))?+|(?=\n)|\z)#`:
/// the end of the document start marker with what upstream removes
/// after it, if the document starts with one.
fn start_marker(s: &[u8]) -> Option<usize> {
    if !s.starts_with(b"---") {
        return None;
    }
    let blanks = strspn(s, b" \t", 3);
    if blanks == 0 {
        return matches!(s.get(3), None | Some(b'\n')).then_some(3);
    }
    let i = 3 + blanks;
    if s.get(i) == Some(&b'%') {
        return Some(i + s[i..].iter().take_while(|&&b| b != b'\n').count());
    }
    // node properties and block scalar headers, each with its blanks
    let mut j = i;
    let mut properties = 0;
    loop {
        match s.get(j) {
            Some(b'!' | b'&') => {
                j += 1 + s[j + 1..]
                    .iter()
                    .take_while(|b| !matches!(b, b' ' | b'\t' | b'\n'))
                    .count();
            }
            Some(b'|' | b'>') => {
                j += 1 + s[j + 1..]
                    .iter()
                    .take_while(|b| matches!(b, b'-' | b'+' | b'0'..=b'9'))
                    .count();
            }
            _ => break,
        }
        j += strspn(s, b" \t", j);
        properties += 1;
    }
    Some(if properties > 0 && followed_by_comment_or_eol(s, j) {
        j
    } else {
        i
    })
}

/// `#^((?:![^! \t\n][^ \t\n]*+[ \t]++)?+)&[^ \t\n]++(?:[ \t]++|(?=\n)|\z)#`:
/// the end of the root node's tag (kept) and of its anchor (removed).
fn root_anchor(s: &[u8]) -> Option<(usize, usize)> {
    let not_blank = |b: &u8| !matches!(b, b' ' | b'\t' | b'\n');
    let mut tag_end = 0;
    if s.first() == Some(&b'!') && s.get(1).is_some_and(|b| *b != b'!' && not_blank(b)) {
        let j = 2 + s[2..].iter().take_while(|b| not_blank(b)).count();
        let blanks = strspn(s, b" \t", j);
        if blanks > 0 {
            tag_end = j + blanks;
        }
    }
    if s.get(tag_end) != Some(&b'&') {
        return None;
    }
    let name = s[tag_end + 1..].iter().take_while(|b| not_blank(b)).count();
    if name == 0 {
        return None;
    }
    let end = tag_end + 1 + name;
    Some((tag_end, end + strspn(s, b" \t", end)))
}

/// Line starts of `s` (where `^` matches under `/m`).
fn line_starts(s: &[u8]) -> impl Iterator<Item = usize> + '_ {
    std::iter::once(0).chain(
        s.iter()
            .enumerate()
            .filter(|(_, b)| **b == b'\n')
            .map(|(i, _)| i + 1),
    )
}

/// `#^\.\.\.(?:[ \t]++(?:\#[^\n]*+)?+)?+(?:\n[ \t]*+(?:\#[^\n]*+)?+)*+\z#m`:
/// where the document end marker starts, when only comments follow it.
fn end_marker(s: &[u8]) -> Option<usize> {
    let to_eol = |j: usize| j + s[j..].iter().take_while(|&&b| b != b'\n').count();
    line_starts(s).find(|&p| {
        if !s[p..].starts_with(b"...") {
            return false;
        }
        let mut j = p + 3;
        let blanks = strspn(s, b" \t", j);
        if blanks > 0 {
            j += blanks;
            if s.get(j) == Some(&b'#') {
                j = to_eol(j);
            }
        }
        loop {
            match s.get(j) {
                None => return true,
                Some(b'\n') => {
                    j += 1;
                    j += strspn(s, b" \t", j);
                    if s.get(j) == Some(&b'#') {
                        j = to_eol(j);
                    }
                }
                Some(_) => return false,
            }
        }
    })
}

/// `#^(?:---|\.\.\.)(?:[ \t][^\n]*+)?+$#m`: the first marker line.
fn other_document(s: &[u8]) -> Option<(usize, usize)> {
    line_starts(s).find_map(|p| {
        let rest = &s[p..];
        if !(rest.starts_with(b"---") || rest.starts_with(b"...")) {
            return None;
        }
        let mut j = p + 3;
        if matches!(s.get(j), Some(b' ' | b'\t')) {
            j += s[j..].iter().take_while(|&&b| b != b'\n').count();
        }
        matches!(s.get(j), None | Some(b'\n')).then_some((p, j))
    })
}

/// `#^(?P<key>(?:![^\s]++\s++)?(?:REGEX_QUOTED_STRING|[^ '"\[\{!].*?)|![^\s]++(?= +\:)) *\:(( |\t)++(?P<value>.+))?$#u`
/// on a right-trimmed line: the key and the value, as PCRE splits them.
fn mapping_line(s: &str) -> Option<(&str, Option<&str>)> {
    let space = php_value::pcre::is_space_u;
    // ` *\:(( |\t)++(?P<value>.+))?$` from `e`: where the value starts.
    let tail = |e: usize| -> Option<Option<usize>> {
        let b = s.as_bytes();
        let j = e + strspn(b, b" ", e);
        if b.get(j) != Some(&b':') {
            return None;
        }
        let j = j + 1;
        if j == b.len() {
            return Some(None);
        }
        let v = j + strspn(b, b" \t", j);
        (v > j && v < b.len()).then_some(Some(v))
    };
    let split = |e: usize, value: Option<usize>| (&s[..e], value.map(|v| &s[v..]));
    // `![^\s]++`: the end of the tag at the start.
    let tag_end = s.strip_prefix('!').map(|rest| {
        1 + rest
            .char_indices()
            .find(|(_, c)| space(*c))
            .map_or(rest.len(), |(i, _)| i)
    });

    let mut starts = Vec::new();
    if let Some(t) = tag_end.filter(|&t| t > 1) {
        // `\s++` after the tag
        let blanks: usize = s[t..]
            .chars()
            .take_while(|c| space(*c))
            .map(char::len_utf8)
            .sum();
        if blanks > 0 {
            starts.push(t + blanks);
        }
    }
    starts.push(0);
    for k in starts {
        let rest = &s[k..];
        if let Some(m) = QUOTED.find(rest.as_bytes()) {
            if let Some(value) = tail(k + m.end()) {
                return Some(split(k + m.end(), value));
            }
            continue;
        }
        let Some(first) = rest.chars().next() else {
            continue;
        };
        if matches!(first, ' ' | '\'' | '"' | '[' | '{' | '!') {
            continue;
        }
        // `.*?`: the shortest key that the rest of the line fits.
        let mut ends = rest[first.len_utf8()..]
            .char_indices()
            .map(|(i, _)| k + first.len_utf8() + i)
            .chain(std::iter::once(s.len()));
        if let Some((e, value)) = ends.find_map(|e| tail(e).map(|v| (e, v))) {
            return Some(split(e, value));
        }
    }
    // `![^\s]++(?= +\:)`
    let t = tag_end.filter(|&t| t > 1)?;
    let spaces = strspn(s.as_bytes(), b" ", t);
    if spaces == 0 || s.as_bytes().get(t + spaces) != Some(&b':') {
        return None;
    }
    tail(t).map(|value| split(t, value))
}

/// The context of the block being parsed.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Context {
    Sequence,
    Mapping,
}

/// What every block parser of one parse shares, as PHP's nested parsers
/// share it with their parent: the anchors, the state, and Inline's
/// static state.
struct Shared<'a> {
    inline: &'a mut Inline,
    state: &'a mut ParserState,
    refs: &'a mut References,
}

/// The YAML parser.
#[derive(Debug, Clone)]
pub struct Parser {
    inline: Inline,
    state: ParserState,
    filename: Option<String>,
}

impl Default for Parser {
    fn default() -> Self {
        Self::new()
    }
}

impl Parser {
    /// `new Parser()`, with PHP's core constants and no classes for
    /// `!php/object` ([`Environment::new`]).
    pub fn new() -> Self {
        Self::with_environment(Arc::new(Environment::new()))
    }

    pub fn with_environment(environment: Arc<Environment>) -> Self {
        Self {
            inline: Inline::new(environment),
            state: ParserState::default(),
            filename: None,
        }
    }

    /// `new Parser($maxNestingLevel, $maxAliasesForCollections)`.
    pub fn with_limits(
        mut self,
        max_nesting_level: i64,
        max_aliases_for_collections: i64,
    ) -> Result<Self, InvalidArgumentException> {
        if max_nesting_level < 1 {
            return Err(InvalidArgumentException {
                message: "The maximum nesting depth must be greater than 0.".to_owned(),
            });
        }
        if max_aliases_for_collections < 0 {
            return Err(InvalidArgumentException {
                message:
                    "The maximum number of collection aliases must be greater than or equal to 0."
                        .to_owned(),
            });
        }
        self.state.max_nesting_level = max_nesting_level;
        self.state.max_aliases_for_collections = max_aliases_for_collections;
        Ok(self)
    }

    /// Inline's static state, as the last parse left it.
    pub fn inline(&self) -> &Inline {
        &self.inline
    }

    /// `parseFile()`: parses a YAML file.
    pub fn parse_file(&mut self, filename: &str, flags: Flags) -> Result<Value, ParseException> {
        let path = std::path::Path::new(filename);
        if !path.is_file() {
            return Err(ParseException::message_only(format!(
                "File \"{filename}\" does not exist."
            )));
        }
        let Ok(contents) = std::fs::read(path) else {
            return Err(ParseException::message_only(format!(
                "File \"{filename}\" cannot be read."
            )));
        };
        self.filename = Some(filename.to_owned());
        let result = self.parse(&contents, flags);
        self.filename = None;
        result
    }

    /// `parse()`: parses a YAML string.
    pub fn parse(&mut self, value: &[u8], flags: Flags) -> Result<Value, ParseException> {
        if std::str::from_utf8(value).is_err() {
            return Err(ParseException::new(
                "The YAML value does not appear to be valid UTF-8.",
                -1,
                None,
                self.filename.clone(),
            ));
        }
        let mut refs = References::new();
        self.state.reset();
        self.state.aliases_enabled = !flags.contains(Flags::PARSE_EXCEPTION_ON_ALIAS);
        let mut shared = Shared {
            inline: &mut self.inline,
            state: &mut self.state,
            refs: &mut refs,
        };
        let mut block = Block::new(self.filename.clone(), 0);
        let result = block.do_parse(&mut shared, value, flags);
        self.state.reset();
        result
    }
}

/// One `Parser` instance of PHP's: the root, or one per nested block.
#[derive(Debug, Default)]
struct Block {
    filename: Option<String>,
    offset: i64,
    number_of_parsed_lines: i64,
    total_number_of_lines: Option<i64>,
    lines: Vec<Vec<u8>>,
    current_line_nb: i64,
    current_line: Vec<u8>,
    skipped_line_numbers: Vec<i64>,
    locally_skipped_line_numbers: Vec<i64>,
    refs_being_parsed: Vec<Vec<u8>>,
}

/// What `doParse()` keeps across lines.
struct Document {
    data: PhpArray,
    context: Option<Context>,
    allow_overwrite: bool,
    /// doParse's `$value`: the document, then the last value assigned.
    value: Value,
    has_content_on_start_marker_line: bool,
}

impl Block {
    fn new(filename: Option<String>, offset: i64) -> Self {
        Self {
            filename,
            offset,
            current_line_nb: -1,
            ..Self::default()
        }
    }

    fn error(
        &self,
        message: impl Into<String>,
        line: i64,
        snippet: &[u8],
        with_filename: bool,
    ) -> ParseException {
        ParseException::new(
            message,
            line,
            Some(PhpString::from(snippet)),
            if with_filename {
                self.filename.clone()
            } else {
                None
            },
        )
    }

    /// An error at the current line, with the file name.
    fn here(&self, message: impl Into<String>) -> ParseException {
        self.error(
            message,
            self.get_real_current_line_nb() + 1,
            &self.current_line,
            true,
        )
    }

    /// `$e->setParsedLine(...); $e->setSnippet($this->currentLine)`.
    fn relocate(&self, mut e: ParseException) -> ParseException {
        e.set_parsed_line(self.get_real_current_line_nb() + 1);
        e.set_snippet(PhpString::from(self.current_line.as_slice()));
        e
    }

    #[expect(
        clippy::cast_possible_wrap,
        reason = "PHP's int and float conversions, offsets and lengths, as upstream"
    )]
    fn do_parse(
        &mut self,
        sh: &mut Shared<'_>,
        value: &[u8],
        flags: Flags,
    ) -> Result<Value, ParseException> {
        self.current_line_nb = -1;
        self.current_line.clear();
        let (value, has_content_on_start_marker_line) = self.cleanup(value)?;
        self.lines = value.split(|&b| b == b'\n').map(<[u8]>::to_vec).collect();
        self.number_of_parsed_lines = self.lines.len() as i64;
        self.locally_skipped_line_numbers.clear();
        if self.total_number_of_lines.is_none() {
            self.total_number_of_lines = Some(self.number_of_parsed_lines);
        }

        if !self.move_to_next_line() {
            return Ok(Value::Null);
        }

        let mut doc = Document {
            data: PhpArray::new(),
            context: None,
            allow_overwrite: false,
            value: Value::String(PhpString::from(value.as_slice())),
            has_content_on_start_marker_line,
        };

        while self.is_current_line_empty() {
            if !self.move_to_next_line() {
                return Ok(Value::Null);
            }
        }

        // Resolves the tag and returns if end of the document
        let line = self.current_line.clone();
        let tag = self.get_line_tag(&line, flags, false)?;
        if let Some(tag) = &tag {
            loop {
                if !self.move_to_next_line() {
                    return Ok(tagged(tag, string(b"")));
                }
                if !self.is_current_line_empty() {
                    break;
                }
            }
        }

        loop {
            if let Some(result) = self.parse_line(sh, &mut doc, flags)? {
                return Ok(result);
            }
            if !self.move_to_next_line() {
                break;
            }
        }

        let mut data = Value::Array(doc.data);
        if let Some(tag) = &tag {
            data = tagged(tag, data);
        }
        if flags.contains(Flags::PARSE_OBJECT_FOR_MAP)
            && doc.context == Some(Context::Mapping)
            && let Value::Array(array) = &data
        {
            data = StdClass::object(array.clone());
        }
        Ok(match &data {
            Value::Array(array) if array.is_empty() => Value::Null,
            _ => data,
        })
    }

    /// One iteration of doParse's loop; `Some` when it returns.
    fn parse_line(
        &mut self,
        sh: &mut Shared<'_>,
        doc: &mut Document,
        flags: Flags,
    ) -> Result<Option<Value>, ParseException> {
        if self.is_current_line_empty() {
            return Ok(None);
        }
        let line = self.current_line.clone();

        // tab?
        if line[0] == b'\t' {
            return Err(self.here("A YAML file cannot contain tabs as indentation."));
        }

        sh.inline.initialize(
            flags,
            Some(self.get_real_current_line_nb()),
            self.filename.clone(),
        );

        if line[0] == b'-' {
            let subject = rtrim(&line);
            utf8(subject)?;
            if let Some(captures) = SEQUENCE_ITEM.captures(subject) {
                let leadspaces = captures.name("leadspaces").map(|m| m.as_bytes().to_vec());
                let value = captures.name("value").map(|m| m.as_bytes().to_vec());
                self.sequence_item(sh, doc, flags, leadspaces, value)?;
                return Ok(None);
            }
        }

        let subject = rtrim(&line);
        if let Some((key, value)) = mapping_line(utf8(subject)?)
            && (!key.contains(" #") || matches!(key.as_bytes()[0], b'"' | b'\''))
        {
            let key = key.as_bytes().to_vec();
            let value = value.map(|v| v.as_bytes().to_vec());
            self.mapping_item(sh, doc, flags, &key, value)?;
            return Ok(None);
        }

        match line[0] {
            b'"' | b'\'' => {
                if doc.context.is_some() {
                    return Err(self.here("Unable to parse."));
                }
                let result = (|| {
                    let lexed = self.lex_inline_quoted_string(&mut 0)?;
                    sh.inline
                        .parse(&lexed, flags, sh.refs, Some(&mut *sh.state))
                })();
                result.map(Some).map_err(|e| self.relocate(e))
            }
            b'{' | b'[' => {
                if doc.context.is_some() {
                    return Err(self.here("Unable to parse."));
                }
                let closing = if line[0] == b'{' { b'}' } else { b']' };
                let result = (|| {
                    let lexed = self.lex_inline_structure(sh, &mut 0, closing, true)?;
                    let parsed = sh
                        .inline
                        .parse(&lexed, flags, sh.refs, Some(&mut *sh.state))?;
                    while self.move_to_next_line() {
                        if !self.is_current_line_empty() {
                            return Err(self.here("Unable to parse."));
                        }
                    }
                    Ok(parsed)
                })();
                result.map(Some).map_err(|e| self.relocate(e))
            }
            _ => self.other_line(sh, doc, flags).map(Some),
        }
    }

    #[expect(
        clippy::cast_possible_wrap,
        reason = "PHP's int and float conversions, offsets and lengths, as upstream"
    )]
    #[expect(clippy::needless_pass_by_value, reason = "upstream's signature")]
    fn sequence_item(
        &mut self,
        sh: &mut Shared<'_>,
        doc: &mut Document,
        flags: Flags,
        leadspaces: Option<Vec<u8>>,
        mut value: Option<Vec<u8>>,
    ) -> Result<(), ParseException> {
        if doc.context == Some(Context::Mapping) {
            return Err(self.here("You cannot define a sequence item when in a mapping."));
        }
        doc.context = Some(Context::Sequence);

        if doc.has_content_on_start_marker_line && self.current_line_nb == 0 {
            return Err(self.here(
                "A block collection cannot start on the line of the document start marker (---).",
            ));
        }

        if let Some(v) = &value
            && v.first() == Some(&b'!')
        {
            value = Some(move_anchor_before_tag(v));
        }

        let mut is_ref = None;
        if let Some((name, rest)) = reference(value.as_deref())? {
            self.refs_being_parsed.push(name.clone());
            is_ref = Some(name);
            value = Some(rest);
        }

        if let Some(v) = &value
            && v.len() > 1
            && v[0] == b'?'
            && v[1] == b' '
        {
            return Err(self.error(
                "Complex mappings are not supported.",
                self.get_real_current_line_nb() + 1,
                &self.current_line,
                false,
            ));
        }

        let item = match value.as_deref() {
            // array
            Some(v) if ltrim_chars(v, b" ").starts_with(b"-") => {
                // Inline first child
                let current_line_number = self.get_real_current_line_nb();
                let sequence_indentation = leadspaces.as_ref().map_or(0, Vec::len) + 1;
                let mut sequence_yaml = substr(&self.current_line, sequence_indentation).to_vec();
                let embed_block =
                    self.get_next_embed_block(Some(sequence_indentation as i64), true)?;
                // keep trailing empty lines, they can be part of a block scalar
                if current_line_number != self.get_real_current_line_nb() {
                    sequence_yaml.push(b'\n');
                    sequence_yaml.extend_from_slice(&embed_block);
                }
                self.parse_block(sh, current_line_number, &sequence_yaml, flags)?
            }
            None => self.nested_item(sh, flags)?,
            Some(v) if trim_chars(v, b" ").is_empty() || ltrim_chars(v, b" ").starts_with(b"#") => {
                self.nested_item(sh, flags)?
            }
            Some(v) => {
                if let Some(sub_tag) = self.get_line_tag(ltrim_chars(v, b" "), flags, true)? {
                    let offset = self.get_real_current_line_nb() + 1;
                    let block = self.get_next_embed_block(None, true)?;
                    tagged(&sub_tag, self.parse_block(sh, offset, &block, flags)?)
                } else if TAGGED_BLOCK_SCALAR.is_match(v) {
                    self.parse_value(sh, v, flags, Context::Sequence)?
                } else if leadspaces.is_some()
                    && (v[0] == b'!' || SEQUENCE_MAPPING.is_match(utf8(trim_tag(v))?.as_bytes()))
                {
                    let current_line_number = self.get_real_current_line_nb();
                    let mut block = v.to_vec();
                    if self.is_next_line_indented() || self.is_next_line_in_block_scalar(&block) {
                        let indentation = self.get_current_line_indentation()
                            + leadspaces.as_ref().map_or(0, Vec::len) as i64
                            + 1;
                        block.push(b'\n');
                        block.extend_from_slice(
                            &self.get_next_embed_block(Some(indentation), false)?,
                        );
                    }
                    self.parse_block(sh, current_line_number, &block, flags)?
                } else {
                    self.parse_value(sh, v, flags, Context::Sequence)?
                }
            }
        };
        push(&mut doc.data, item)?;

        if let Some(name) = is_ref
            && truthy(&name)
        {
            let last = doc
                .data
                .last()
                .map_or(Value::Bool(false), |(_, v)| v.clone());
            sh.refs.insert(Key::from_bytes(&name), last);
            self.refs_being_parsed.pop();
        }
        Ok(())
    }

    /// `$this->parseBlock($this->getRealCurrentLineNb() + 1, $this->getNextEmbedBlock(null, true) ?? '', $flags)`
    fn nested_item(&mut self, sh: &mut Shared<'_>, flags: Flags) -> Result<Value, ParseException> {
        let offset = self.get_real_current_line_nb() + 1;
        let block = self.get_next_embed_block(None, true)?;
        self.parse_block(sh, offset, &block, flags)
    }

    /// A reference that does not exist, or one being parsed (circular).
    fn missing_reference(&self, name: &[u8], line: i64) -> ParseException {
        let name_text = lossy(name);
        if let Some(pos) = self.refs_being_parsed.iter().position(|r| r == name) {
            let chain: Vec<String> = self.refs_being_parsed[pos..]
                .iter()
                .map(|r| lossy(r).into_owned())
                .chain(std::iter::once(name_text.clone().into_owned()))
                .collect();
            return self.error(
                format!(
                    "Circular reference [{}] detected for reference \"{name_text}\".",
                    chain.join(", ")
                ),
                self.current_line_nb + 1,
                &self.current_line,
                true,
            );
        }
        self.error(
            format!("Reference \"{name_text}\" does not exist."),
            line,
            &self.current_line,
            true,
        )
    }

    #[expect(clippy::too_many_lines, reason = "one function, as upstream's")]
    fn mapping_item(
        &mut self,
        sh: &mut Shared<'_>,
        doc: &mut Document,
        flags: Flags,
        raw_key: &[u8],
        mut value: Option<Vec<u8>>,
    ) -> Result<(), ParseException> {
        if doc.context == Some(Context::Sequence) {
            return Err(self.error(
                "You cannot define a mapping item when in a sequence.",
                self.current_line_nb + 1,
                &self.current_line,
                true,
            ));
        }
        doc.context = Some(Context::Mapping);

        if doc.has_content_on_start_marker_line && self.current_line_nb == 0 {
            return Err(self.here(
                "A block collection cannot start on the line of the document start marker (---).",
            ));
        }

        if let Some(v) = &value
            && v.first() == Some(&b'!')
        {
            value = Some(move_anchor_before_tag(v));
        }

        let key = sh
            .inline
            .parse_scalar_value(raw_key)
            .map_err(|e| self.relocate(e))?;

        if !matches!(key, Value::String(_) | Value::Int(_)) {
            let kind = if matches!(key, Value::Float(_)) {
                "Numeric"
            } else {
                "Non-string"
            };
            return Err(self.error(
                format!(
                    "{kind} keys are not supported. Quote your evaluable mapping keys instead."
                ),
                self.get_real_current_line_nb() + 1,
                &self.current_line,
                false,
            ));
        }

        if flags.contains(Flags::PARSE_OBJECT_FOR_MAP)
            && let Value::String(k) = &key
            && k.first() == Some(&0)
        {
            return Err(self.here(
                "Mapping keys starting with a NUL byte are not supported with the \"Yaml::PARSE_OBJECT_FOR_MAP\" flag.",
            ));
        }

        let key_text = key.to_php_string().unwrap_or_default();
        let key = Key::from_value(&key).unwrap_or_else(|| Key::from(key_text.clone()));
        let duplicate = |block: &Self, line: i64| {
            block.error(
                format!("Duplicate key \"{}\" detected.", lossy(&key_text)),
                line,
                &block.current_line,
                false,
            )
        };

        // Only a plain (unquoted) "<<" key is a merge key
        let is_merge_key = rtrim(raw_key) == b"<<";

        let mut merge_node = false;
        let mut is_ref = None;
        let mut merge_ref = None;
        if is_merge_key {
            merge_ref = match &value {
                Some(v) if v.first() == Some(&b'&') => MERGE_REFERENCE
                    .captures(utf8(v)?.as_bytes())
                    .map(|c| c["ref"].to_vec()),
                _ => None,
            };
        }
        if is_merge_key && merge_ref.is_none() {
            merge_node = true;
            doc.allow_overwrite = true;
            match &value {
                Some(v) if v.first() == Some(&b'*') => {
                    let mut ref_name = substr(rtrim(v), 1).to_vec();
                    // remove comments
                    if let Some(m) = COMMENT.find(&ref_name) {
                        ref_name.truncate(m.start());
                    }
                    let line = self.get_real_current_line_nb() + 1;
                    let Some(ref_value) = sh.refs.get(&Key::from_bytes(&ref_name)).cloned() else {
                        return Err(self.missing_reference(&ref_name, line));
                    };
                    sh.state.count_alias(
                        &ref_value,
                        line,
                        Some(&self.current_line),
                        self.filename.as_deref(),
                    )?;
                    let ref_value = unless_object(ref_value, flags);
                    let Value::Array(ref_value) = ref_value else {
                        return Err(self.here(
                            "YAML merge keys used with a scalar value instead of an array.",
                        ));
                    };
                    doc.data.union(&ref_value);
                }
                _ => {
                    let current_line_number = self.get_real_current_line_nb();
                    let block = match &value {
                        Some(v) if !v.is_empty() => v.clone(),
                        _ => self.get_next_embed_block(None, false)?,
                    };
                    doc.value = Value::String(PhpString::from(block.as_slice()));
                    let parsed = self.parse_block(sh, current_line_number + 1, &block, flags)?;
                    let parsed = unless_object(parsed, flags);
                    let Value::Array(parsed) = parsed else {
                        return Err(self.here(
                            "YAML merge keys used with a scalar value instead of an array.",
                        ));
                    };
                    if isset(&parsed, &Key::Int(0)) {
                        // If the value associated with the merge key is a sequence, then this sequence is expected to contain mapping nodes
                        // and each of these nodes is merged in turn according to its order in the sequence. Keys in mapping nodes earlier
                        // in the sequence override keys specified in later mapping nodes.
                        for item in parsed.values() {
                            let item = unless_object(item.clone(), flags);
                            let Value::Array(item) = item else {
                                return Err(self.merge_item_error(&item));
                            };
                            doc.data.union(&item);
                        }
                    } else {
                        // If the value associated with the key is a single mapping node, each of its key/value pairs is inserted into the
                        // current mapping, unless the key already exists in it.
                        doc.data.union(&parsed);
                    }
                }
            }
        } else if !is_merge_key && let Some((name, rest)) = reference(value.as_deref())? {
            self.refs_being_parsed.push(name.clone());
            is_ref = Some(name);
            value = Some(rest);
        }

        let mut sub_tag = None;
        let hash = !merge_node
            && match value.as_deref() {
                None | Some(b"") => true,
                Some(v) if v.starts_with(b"#") => true,
                Some(v) => {
                    sub_tag = self.get_line_tag(v, flags, true)?;
                    sub_tag.is_some() || is_merge_key
                }
            };
        if merge_node {
            // Merge keys
        } else if hash {
            // hash
            // if next line is less indented or equal, then it means that the current value is null
            if !self.is_next_line_indented() && !self.is_next_line_unindented_collection() {
                // Spec: Keys MUST be unique; first one wins.
                // But overwriting is allowed when a merge node is used in current block.
                if doc.allow_overwrite || !isset(&doc.data, &key) {
                    if !doc.allow_overwrite && doc.data.contains_key(&key) {
                        return Err(duplicate(self, self.get_real_current_line_nb() + 1));
                    }
                    let item = match &sub_tag {
                        Some(tag) => tagged(tag, string(b"")),
                        None => Value::Null,
                    };
                    doc.data.insert(key.clone(), item);
                } else {
                    return Err(duplicate(self, self.get_real_current_line_nb() + 1));
                }
            } else {
                // remember the parsed line number here in case we need it to provide some contexts in error messages below
                let real_current_line_nb_key = self.get_real_current_line_nb();
                let offset = self.get_real_current_line_nb() + 1;
                let block = self.get_next_embed_block(None, false)?;
                let parsed = self.parse_block(sh, offset, &block, flags)?;
                doc.value = parsed.clone();
                if is_merge_key {
                    let name = merge_ref.unwrap_or_default();
                    sh.refs.insert(Key::from_bytes(&name), parsed.clone());
                    union(&mut doc.data, &unless_object(parsed, flags))?;
                } else if doc.allow_overwrite || !isset(&doc.data, &key) {
                    if !doc.allow_overwrite && doc.data.contains_key(&key) {
                        return Err(duplicate(self, self.get_real_current_line_nb() + 1));
                    }
                    // Spec: Keys MUST be unique; first one wins.
                    // But overwriting is allowed when a merge node is used in current block.
                    let item = match &sub_tag {
                        Some(tag) => tagged(tag, parsed),
                        None => parsed,
                    };
                    doc.data.insert(key.clone(), item);
                } else {
                    return Err(duplicate(self, real_current_line_nb_key + 1));
                }
            }
        } else {
            let v = value.as_deref().unwrap_or(b"");
            let parsed = self.parse_value(sh, rtrim(v), flags, Context::Mapping)?;
            doc.value = parsed.clone();
            // Spec: Keys MUST be unique; first one wins.
            // But overwriting is allowed when a merge node is used in current block.
            if doc.allow_overwrite || !isset(&doc.data, &key) {
                if !doc.allow_overwrite && doc.data.contains_key(&key) {
                    return Err(duplicate(self, self.get_real_current_line_nb() + 1));
                }
                doc.data.insert(key.clone(), parsed);
            } else {
                return Err(duplicate(self, self.get_real_current_line_nb() + 1));
            }
        }
        if let Some(name) = is_ref
            && truthy(&name)
        {
            let item = doc.data.get(&key).cloned().unwrap_or(Value::Null);
            sh.refs.insert(Key::from_bytes(&name), item);
            self.refs_being_parsed.pop();
        }
        Ok(())
    }

    /// "Merge items must be arrays.", with the item as the snippet:
    /// PHP's `TypeError` when the item is not a scalar or null.
    fn merge_item_error(&self, item: &Value) -> ParseException {
        let line = self.get_real_current_line_nb() + 1;
        match item {
            Value::Object(_) | Value::Array(_) => ParseException::message_only(format!(
                "Symfony\\Component\\Yaml\\Exception\\ParseException::__construct(): Argument #3 ($snippet) must be of type ?string, {} given",
                item.debug_type()
            )),
            Value::Null => ParseException::new(
                "Merge items must be arrays.",
                line,
                None,
                self.filename.clone(),
            ),
            scalar => ParseException::new(
                "Merge items must be arrays.",
                line,
                scalar.to_php_string(),
                self.filename.clone(),
            ),
        }
    }

    /// doParse's last branch: a line that is no collection item.
    fn other_line(
        &mut self,
        sh: &mut Shared<'_>,
        doc: &mut Document,
        flags: Flags,
    ) -> Result<Value, ParseException> {
        let line = self.current_line.clone();
        if line.len() > 1 && line[0] == b'?' && line[1] == b' ' {
            return Err(self.error(
                "Complex mappings are not supported.",
                self.get_real_current_line_nb() + 1,
                &self.current_line,
                false,
            ));
        }

        // 1-liner optionally followed by newline(s)
        if let Value::String(value) = &doc.value
            && self.lines[0].as_slice() == trim(value)
        {
            let first = self.lines[0].clone();
            return sh
                .inline
                .parse(&first, flags, sh.refs, Some(&mut *sh.state))
                .map_err(|e| self.relocate(e));
        }

        // try to parse the value as a multi-line string as a last resort
        if self.current_line_nb == 0 {
            let mut previous_line_was_newline = false;
            let mut previous_line_was_terminated_with_backslash = false;
            let mut value = Vec::new();

            for line in &self.lines {
                let trimmed_line = trim(line);
                if trimmed_line.first() == Some(&b'#') {
                    continue;
                }
                // If the indentation is not consistent at offset 0, it is to be considered as a ParseError
                if self.offset == 0 && line.first() == Some(&b' ') {
                    return Err(self.here("Unable to parse."));
                }

                if contains(line, b": ") {
                    return Err(self.here("Mapping values are not allowed in multi-line blocks."));
                }

                if trimmed_line.is_empty() {
                    value.push(b'\n');
                } else if !previous_line_was_newline && !previous_line_was_terminated_with_backslash
                {
                    value.push(b' ');
                }

                if !trimmed_line.is_empty() && line.ends_with(b"\\") {
                    value.extend_from_slice(ltrim(&line[..line.len() - 1]));
                } else if !trimmed_line.is_empty() {
                    value.extend_from_slice(trimmed_line);
                }

                if trimmed_line.is_empty() {
                    previous_line_was_newline = true;
                    previous_line_was_terminated_with_backslash = false;
                } else if line.ends_with(b"\\") {
                    previous_line_was_newline = false;
                    previous_line_was_terminated_with_backslash = true;
                } else {
                    previous_line_was_newline = false;
                    previous_line_was_terminated_with_backslash = false;
                }
            }

            doc.value = Value::String(PhpString::from(value.as_slice()));
            // fall-through to the ParseException thrown below
            if let Ok(parsed) =
                sh.inline
                    .parse(trim(&value), Flags::NONE, sh.refs, Some(&mut *sh.state))
            {
                return Ok(parsed);
            }
        }

        Err(self.here("Unable to parse."))
    }

    fn parse_block(
        &mut self,
        sh: &mut Shared<'_>,
        offset: i64,
        yaml: &[u8],
        flags: Flags,
    ) -> Result<Value, ParseException> {
        let mut skipped_line_numbers = self.skipped_line_numbers.clone();
        skipped_line_numbers.extend(
            self.locally_skipped_line_numbers
                .iter()
                .filter(|&&n| n >= offset),
        );

        let mut parser = Block::new(None, offset);
        parser.total_number_of_lines = self.total_number_of_lines;
        parser.skipped_line_numbers = skipped_line_numbers;
        parser.refs_being_parsed.clone_from(&self.refs_being_parsed);

        sh.state.enter_nesting_level(
            offset + 1,
            Some(&self.current_line),
            self.filename.as_deref(),
        )?;
        let result = parser.do_parse(sh, yaml, flags);
        sh.state.leave_nesting_level();
        result
    }

    /// `getRealCurrentLineNb()`: the current line number, with the offset.
    fn get_real_current_line_nb(&self) -> i64 {
        let mut real_current_line_number = self.current_line_nb + self.offset;
        for &skipped_line_number in &self.skipped_line_numbers {
            if skipped_line_number > real_current_line_number {
                break;
            }
            real_current_line_number += 1;
        }
        real_current_line_number
    }

    #[expect(
        clippy::cast_possible_wrap,
        reason = "PHP's int and float conversions, offsets and lengths, as upstream"
    )]
    fn get_current_line_indentation(&self) -> i64 {
        if self.current_line.first() != Some(&b' ') {
            return 0;
        }
        (self.current_line.len() - ltrim_chars(&self.current_line, b" ").len()) as i64
    }

    /// `getNextEmbedBlock()`. A `None` indentation compares as PHP's
    /// `null`: every indentation is `>=` it and none is `===` it.
    #[expect(
        clippy::cast_possible_truncation,
        clippy::cast_sign_loss,
        reason = "PHP's int and float conversions, offsets and lengths, as upstream"
    )]
    fn get_next_embed_block(
        &mut self,
        indentation: Option<i64>,
        in_sequence: bool,
    ) -> Result<Vec<u8>, ParseException> {
        let old_line_indentation = self.get_current_line_indentation();

        if !self.move_to_next_line() {
            return Ok(Vec::new());
        }

        let new_indent = match indentation {
            None => {
                let mut new_indent = None;
                let mut movements = 0;
                loop {
                    let mut eof = false;
                    // empty and comment-like lines do not influence the indentation depth
                    if self.is_current_line_empty() || self.is_current_line_comment() {
                        eof = !self.move_to_next_line();
                        if !eof {
                            movements += 1;
                        }
                    } else {
                        new_indent = Some(self.get_current_line_indentation());
                    }
                    if eof || new_indent.is_some() {
                        break;
                    }
                }
                for _ in 0..movements {
                    self.move_to_previous_line();
                }

                let unindented_embed_block = self.is_string_unindented_collection_item();

                if !self.is_current_line_empty() && new_indent == Some(0) && !unindented_embed_block
                {
                    return Err(self.here("Indentation problem."));
                }
                new_indent
            }
            Some(indentation) => Some(indentation),
        };
        let ge = |indent: i64| new_indent.is_none_or(|n| indent >= n);
        let cut = |line: &[u8]| substr(line, new_indent.unwrap_or(0) as usize).to_vec();

        let mut data: Vec<Vec<u8>> = Vec::new();

        if ge(self.get_current_line_indentation()) {
            data.push(cut(&self.current_line));
        } else if self.is_current_line_empty() || self.is_current_line_comment() {
            data.push(self.current_line.clone());
        } else {
            self.move_to_previous_line();
            return Ok(Vec::new());
        }

        if in_sequence && Some(old_line_indentation) == new_indent && data[0].first() == Some(&b'-')
        {
            // the previous line contained a dash but no item content, this line is a sequence item with the same indentation
            // and therefore no nested list or mapping
            self.move_to_previous_line();
            return Ok(Vec::new());
        }

        let mut is_it_unindented_collection = self.is_string_unindented_collection_item();
        let mut is_it_comment = self.is_current_line_comment();

        while self.move_to_next_line() {
            if is_it_comment && !is_it_unindented_collection {
                is_it_unindented_collection = self.is_string_unindented_collection_item();
                is_it_comment = self.is_current_line_comment();
            }

            let indent = self.get_current_line_indentation();

            if is_it_unindented_collection
                && !self.is_current_line_empty()
                && !self.is_string_unindented_collection_item()
                && new_indent == Some(indent)
            {
                self.move_to_previous_line();
                break;
            }

            if self.is_current_line_blank() {
                data.push(cut(&self.current_line));
                continue;
            }

            if ge(indent) {
                data.push(cut(&self.current_line));
            } else if self.is_current_line_comment() {
                data.push(self.current_line.clone());
            } else if indent == 0 {
                self.move_to_previous_line();
                break;
            } else {
                return Err(self.here("Indentation problem."));
            }
        }

        Ok(data.join(&b'\n'))
    }

    #[expect(
        clippy::cast_possible_wrap,
        reason = "PHP's int and float conversions, offsets and lengths, as upstream"
    )]
    fn has_more_lines(&self) -> bool {
        (self.lines.len() as i64 - 1) > self.current_line_nb
    }

    /// Moves the parser to the next line.
    #[expect(
        clippy::cast_possible_truncation,
        clippy::cast_sign_loss,
        reason = "PHP's int and float conversions, offsets and lengths, as upstream"
    )]
    fn move_to_next_line(&mut self) -> bool {
        if self.current_line_nb >= self.number_of_parsed_lines - 1 {
            return false;
        }
        self.current_line_nb += 1;
        self.current_line = self.lines[self.current_line_nb as usize].clone();
        true
    }

    /// Moves the parser to the previous line.
    #[expect(
        clippy::cast_possible_truncation,
        clippy::cast_sign_loss,
        reason = "PHP's int and float conversions, offsets and lengths, as upstream"
    )]
    fn move_to_previous_line(&mut self) -> bool {
        if self.current_line_nb < 1 {
            return false;
        }
        self.current_line_nb -= 1;
        self.current_line = self.lines[self.current_line_nb as usize].clone();
        true
    }

    /// `parseValue()`: a YAML value after `key:` or `- `.
    fn parse_value(
        &mut self,
        sh: &mut Shared<'_>,
        value: &[u8],
        flags: Flags,
        context: Context,
    ) -> Result<Value, ParseException> {
        if let Some(rest) = value.strip_prefix(b"*") {
            let mut name = rest.to_vec();
            // remove comments
            if let Some(m) = COMMENT.find(&name) {
                name.truncate(m.start());
            }
            let Some(ref_value) = sh.refs.get(&Key::from_bytes(&name)).cloned() else {
                return Err(self.missing_reference(&name, self.current_line_nb + 1));
            };
            sh.state.count_alias(
                &ref_value,
                self.get_real_current_line_nb() + 1,
                Some(&self.current_line),
                self.filename.as_deref(),
            )?;
            return Ok(ref_value);
        }

        if matches!(value.first(), Some(b'!' | b'|' | b'>'))
            && let Some(captures) = BLOCK_SCALAR.captures(value)
        {
            let modifiers = captures
                .name("modifiers")
                .map_or(&b""[..], |m| m.as_bytes());
            let chomping: Vec<u8> = modifiers
                .iter()
                .copied()
                .filter(|b| !b.is_ascii_digit())
                .collect();
            let indentation = string_to_int(modifiers).unsigned_abs();
            let separator = captures["separator"][0];
            let data = self.parse_block_scalar(separator, &chomping, indentation)?;

            let tag = captures.name("tag").map_or(&b""[..], |m| m.as_bytes());
            if !tag.is_empty() && tag != b"!" {
                return Ok(match tag {
                    b"!!binary" => Value::String(sh.inline.evaluate_binary_scalar(&data)?),
                    b"!!str" => string(&data),
                    b"!!float" => Value::Float(string_to_float(&data)),
                    _ => tagged(&lossy(&tag[1..]), string(&data)),
                });
            }

            return Ok(string(&data));
        }

        let result = self.parse_plain_value(sh, value, flags, context);
        result.map_err(|e| self.relocate(e))
    }

    /// The `try` part of `parseValue()`.
    fn parse_plain_value(
        &mut self,
        sh: &mut Shared<'_>,
        value: &[u8],
        flags: Flags,
        context: Context,
    ) -> Result<Value, ParseException> {
        let start = |block: &Self| {
            rtrim(&block.current_line)
                .len()
                .saturating_sub(rtrim(value).len())
        };
        match value.first() {
            Some(b'{' | b'[') => {
                let closing = if value[0] == b'{' { b'}' } else { b']' };
                let mut cursor = start(self);
                let lexed = self.lex_inline_structure(sh, &mut cursor, closing, true)?;
                sh.inline
                    .parse(&lexed, flags, sh.refs, Some(&mut *sh.state))
            }
            Some(b'"' | b'\'') => {
                let mut cursor = start(self);
                let lexed = self.lex_inline_quoted_string(&mut cursor)?;
                let parsed_value = sh
                    .inline
                    .parse(&lexed, flags, sh.refs, Some(&mut *sh.state))?;

                if cursor < self.current_line.len()
                    && truthy(&strip_trailing_blanks_and_comment(substr(
                        &self.current_line,
                        cursor,
                    )))
                {
                    return Err(ParseException::message_only(format!(
                        "Unexpected characters near \"{}\".",
                        lossy(substr(&self.current_line, cursor))
                    )));
                }

                Ok(parsed_value)
            }
            _ => {
                let mut lines: Vec<Vec<u8>> = Vec::new();

                while self.move_to_next_line() {
                    // unquoted strings end before the first unindented line
                    if self.get_current_line_indentation() == 0 {
                        self.move_to_previous_line();
                        break;
                    }

                    if self.is_current_line_comment() {
                        break;
                    }

                    if context == Context::Mapping
                        && contains(&self.current_line, b": ")
                        && !self.is_current_line_comment()
                    {
                        return Err(
                            self.here("A colon cannot be used in an unquoted mapping value.")
                        );
                    }

                    lines.push(trim(&self.current_line).to_vec());
                }

                let mut value = value.to_vec();
                let mut previous_line_blank = false;
                for line in &lines {
                    if line.is_empty() {
                        value.push(b'\n');
                        previous_line_blank = true;
                    } else if previous_line_blank {
                        value.extend_from_slice(line);
                        previous_line_blank = false;
                    } else {
                        value.push(b' ');
                        value.extend_from_slice(line);
                        previous_line_blank = false;
                    }
                }

                sh.inline.parsed_line_number = self.get_real_current_line_nb();

                let parsed_value = sh
                    .inline
                    .parse(&value, flags, sh.refs, Some(&mut *sh.state))?;

                if context == Context::Mapping
                    && let Value::String(parsed) = &parsed_value
                    && !matches!(value.first(), Some(b'"' | b'\'' | b'[' | b'{' | b'!'))
                    && contains(parsed, b": ")
                {
                    return Err(self.error(
                        "A colon cannot be used in an unquoted mapping value.",
                        self.get_real_current_line_nb() + 1,
                        &value,
                        true,
                    ));
                }

                Ok(parsed_value)
            }
        }
    }

    /// `parseBlockScalar()`: the lines of a `|` or `>` block scalar.
    #[expect(
        clippy::cast_possible_truncation,
        reason = "PHP's int and float conversions, offsets and lengths, as upstream"
    )]
    #[expect(clippy::too_many_lines, reason = "one function, as upstream's")]
    fn parse_block_scalar(
        &mut self,
        style: u8,
        chomping: &[u8],
        mut indentation: u64,
    ) -> Result<Vec<u8>, ParseException> {
        let mut not_eof = self.move_to_next_line();
        if !not_eof {
            return Ok(Vec::new());
        }

        let mut is_current_line_blank = self.is_current_line_blank();
        let mut block_lines: Vec<Vec<u8>> = Vec::new();

        // leading blank lines are consumed before determining indentation
        while not_eof
            && is_current_line_blank
            && (indentation == 0 || self.current_line.len() as u64 <= indentation)
        {
            // newline only if not EOF
            not_eof = self.move_to_next_line();
            if not_eof {
                block_lines.push(Vec::new());
                is_current_line_blank = self.is_current_line_blank();
            }
        }

        // determine indentation if not specified
        if indentation == 0 {
            indentation = strspn(&self.current_line, b" ", 0) as u64;
        }

        if !not_eof
            || indentation == 0
            || (strspn(&self.current_line, b" ", 0) as u64) < indentation
        {
            if not_eof {
                self.move_to_previous_line();
            } else if !self.is_current_line_last_line_in_document() {
                block_lines.push(Vec::new());
            }

            // without content, the line breaks of the empty lines are only kept with the "+" chomping indicator
            return Ok(if chomping == b"+" {
                vec![b'\n'; block_lines.len()]
            } else {
                Vec::new()
            });
        }

        // `/^ {N}(.*)$/`: PCRE refuses to compile a quantifier over 65535.
        let indented = |line: &[u8]| -> Result<Option<Vec<u8>>, ParseException> {
            if indentation > 65535 {
                return Err(ParseException::message_only("Internal error"));
            }
            let n = indentation as usize;
            Ok((line.len() >= n && line[..n].iter().all(|&b| b == b' '))
                .then(|| line[n..].to_vec()))
        };

        while not_eof {
            let content = if is_current_line_blank {
                None
            } else {
                match indented(&self.current_line)? {
                    Some(content) => Some(content),
                    None => break,
                }
            };
            if is_current_line_blank && self.current_line.len() as u64 > indentation {
                block_lines.push(substr(&self.current_line, indentation as usize).to_vec());
            } else if is_current_line_blank {
                block_lines.push(Vec::new());
            } else {
                block_lines.push(content.unwrap_or_default());
            }

            // newline only if not EOF
            not_eof = self.move_to_next_line();
            if not_eof {
                is_current_line_blank = self.is_current_line_blank();
            }
        }

        if not_eof {
            block_lines.push(Vec::new());
            self.move_to_previous_line();
        } else if !self.is_current_line_last_line_in_document() {
            block_lines.push(Vec::new());
        }

        // folded style
        let mut text = if style == b'>' {
            let mut text = Vec::new();
            let mut previous_line_indented: Option<bool> = None;
            let mut previous_line_blank = false;
            for (i, line) in block_lines.iter().enumerate() {
                if line.is_empty() {
                    text.push(b'\n');
                    previous_line_blank = true;
                } else if line[0] == b' ' || line[0] == b'\t' {
                    if previous_line_indented.is_some() {
                        text.push(b'\n');
                    }
                    text.extend_from_slice(line);
                    previous_line_indented = Some(true);
                    previous_line_blank = false;
                } else if previous_line_indented == Some(true) {
                    text.push(b'\n');
                    text.extend_from_slice(line);
                    previous_line_indented = Some(false);
                    previous_line_blank = false;
                } else if previous_line_blank || i == 0 {
                    text.extend_from_slice(line);
                    previous_line_indented = Some(false);
                    previous_line_blank = false;
                } else {
                    text.push(b' ');
                    text.extend_from_slice(line);
                    previous_line_indented = Some(false);
                    previous_line_blank = false;
                }
            }
            text
        } else {
            block_lines.join(&b'\n')
        };

        // deal with trailing newlines: `/\n+$/`
        let trailing = text.iter().rev().take_while(|&&b| b == b'\n').count();
        if trailing > 0 {
            if chomping.is_empty() {
                text.truncate(text.len() - trailing + 1);
            } else if chomping == b"-" {
                text.truncate(text.len() - trailing);
            }
        }

        Ok(text)
    }

    /// Returns true if the next line is indented.
    fn is_next_line_indented(&mut self) -> bool {
        let current_indentation = self.get_current_line_indentation();
        let mut movements = 0;
        let mut eof;
        loop {
            eof = !self.move_to_next_line();
            if !eof {
                movements += 1;
            }
            if eof || !(self.is_current_line_empty() || self.is_current_line_comment()) {
                break;
            }
        }

        let ret = !eof && self.get_current_line_indentation() > current_indentation;

        for _ in 0..movements {
            self.move_to_previous_line();
        }

        ret
    }

    fn is_current_line_empty(&self) -> bool {
        self.is_current_line_blank() || self.is_current_line_comment()
    }

    fn is_current_line_blank(&self) -> bool {
        trim_chars(&self.current_line, b" ").is_empty()
    }

    fn is_current_line_comment(&self) -> bool {
        // checking explicitly the first char of the trim is faster than loops or strpos
        ltrim_chars(&self.current_line, b" ").first() == Some(&b'#')
    }

    fn is_current_line_last_line_in_document(&self) -> bool {
        (self.offset + self.current_line_nb) >= (self.total_number_of_lines.unwrap_or(0) - 1)
    }

    /// `cleanup()`: the document without its header, markers and the
    /// root node's properties; and whether content followed `---`.
    #[expect(
        clippy::cast_possible_wrap,
        reason = "PHP's int and float conversions, offsets and lengths, as upstream"
    )]
    #[expect(
        clippy::naive_bytecount,
        reason = "short slices; no bytecount dependency"
    )]
    fn cleanup(&mut self, value: &[u8]) -> Result<(Vec<u8>, bool), ParseException> {
        let mut value = normalize_line_breaks(value);

        // remove the YAML header, and the comments and empty lines before the content
        let end = leading_lines_and_directive(&value);
        value = self.remove_leading_lines(value, end);

        // document markers and the properties of the root node are only found at the top level, not in nested blocks
        if self.total_number_of_lines.is_some() {
            return Ok((value, false));
        }

        // remove start of the document marker (---), with the YAML 1.0 directives, or the node properties and block scalar header that can follow it alone, as they were always ignored
        let count = match start_marker(&value) {
            Some(end) => {
                value.drain(..end);
                1
            }
            None => 0,
        };

        // remove the anchor of the root node, as nothing can refer to it
        if let Some((tag_end, end)) = root_anchor(&value) {
            value.drain(tag_end..end);
        }

        // remove the lines left without content
        let offset = self.offset;
        let end = blank_lines(&value, 0);
        value = self.remove_leading_lines(value, end);
        let has_content_on_start_marker_line = count == 1 && offset == self.offset;

        // remove end of the document marker (...), which can only be followed by comments
        if let Some(start) = end_marker(&value) {
            value.truncate(start);
        }

        if let Some((start, end)) = other_document(&value) {
            let newlines = value[..start].iter().filter(|&&b| b == b'\n').count() as i64;
            return Err(ParseException::new(
                "Multiple documents are not supported.",
                self.offset + newlines + 1,
                Some(PhpString::from(&value[start..end])),
                self.filename.clone(),
            ));
        }

        Ok((value, has_content_on_start_marker_line))
    }

    /// Tells whether the next line belongs to a block scalar whose header ends the given value.
    ///
    /// Unlike `isNextLineIndented()`, empty and comment-like lines are not skipped, as they can be part of the block scalar.
    #[expect(
        clippy::cast_possible_truncation,
        clippy::cast_possible_wrap,
        clippy::cast_sign_loss,
        reason = "PHP's int and float conversions, offsets and lengths, as upstream"
    )]
    fn is_next_line_in_block_scalar(&self, value: &[u8]) -> bool {
        let Some(next_line) = self.lines.get((self.current_line_nb + 1) as usize) else {
            return false;
        };
        if !ENDS_WITH_BLOCK_SCALAR.is_match(value) {
            return false;
        }
        trim_chars(next_line, b" ").is_empty()
            || strspn(next_line, b" ", 0) as i64 > self.get_current_line_indentation()
    }

    /// `removeLeadingLines()`: drops `value[..end]` and counts its lines
    /// into the offset.
    #[expect(
        clippy::cast_possible_wrap,
        reason = "PHP's int and float conversions, offsets and lengths, as upstream"
    )]
    fn remove_leading_lines(&mut self, mut value: Vec<u8>, end: usize) -> Vec<u8> {
        let removed = value.drain(..end);
        // items have been removed, update the offset
        self.offset += removed.filter(|&b| b == b'\n').count() as i64;
        value
    }

    fn is_next_line_unindented_collection(&mut self) -> bool {
        let current_indentation = self.get_current_line_indentation();
        let mut movements = 0;
        let mut eof;
        loop {
            eof = !self.move_to_next_line();
            if !eof {
                movements += 1;
            }
            if eof || !(self.is_current_line_empty() || self.is_current_line_comment()) {
                break;
            }
        }

        let ret = !eof
            && self.get_current_line_indentation() == current_indentation
            && self.is_string_unindented_collection_item();

        for _ in 0..movements {
            self.move_to_previous_line();
        }

        ret
    }

    fn is_string_unindented_collection_item(&self) -> bool {
        rtrim(&self.current_line) == b"-" || self.current_line.starts_with(b"- ")
    }

    fn get_line_tag(
        &mut self,
        value: &[u8],
        flags: Flags,
        next_line_check: bool,
    ) -> Result<Option<String>, ParseException> {
        if value.first() != Some(&b'!') {
            return Ok(None);
        }
        let Some(captures) = LINE_TAG.captures(value) else {
            return Ok(None);
        };
        let full_tag = lossy(&captures["tag"]).into_owned();

        // unlike a sequence item, the value of a mapping key can also be a sequence that is not indented
        if next_line_check
            && !self.is_next_line_indented()
            && (self.is_string_unindented_collection_item()
                || !self.is_next_line_unindented_collection())
        {
            return Ok(None);
        }

        let tag = &full_tag[1..];

        // Built-in tags
        if truthy(tag.as_bytes()) && tag.starts_with('!') {
            // "!!str" and "!!binary" can be resolved as inline scalars when they don't apply to a nested block
            if !next_line_check
                && matches!(tag, "!str" | "!binary")
                && !self.is_next_line_indented()
            {
                return Ok(None);
            }

            return Err(self.error(
                format!("The built-in tag \"!{tag}\" is not implemented."),
                self.get_real_current_line_nb() + 1,
                value,
                true,
            ));
        }

        if flags.contains(Flags::PARSE_CUSTOM_TAGS) {
            return Ok(Some(tag.to_owned()));
        }

        Err(self.error(
            format!(
                "Tags support is not enabled. You must use the flag \"Yaml::PARSE_CUSTOM_TAGS\" to use \"{full_tag}\"."
            ),
            self.get_real_current_line_nb() + 1,
            value,
            true,
        ))
    }

    fn lex_inline_quoted_string(&mut self, cursor: &mut usize) -> Result<Vec<u8>, ParseException> {
        let quotation = self.current_line[*cursor];
        let mut value = vec![quotation];
        *cursor += 1;

        let mut previous_line_was_newline = true;
        let mut previous_line_was_terminated_with_backslash = false;
        let mut line_number = 0;

        loop {
            line_number += 1;
            if line_number > 1 {
                *cursor += strspn(&self.current_line, b" \t", *cursor);
            }

            let is_blank_line = trim_chars(&self.current_line, b" \t").is_empty();
            if is_blank_line {
                value.push(b'\n');
            } else if !previous_line_was_newline && !previous_line_was_terminated_with_backslash {
                value.push(b' ');
            }

            let mut line: Vec<u8> = Vec::new();
            let mut escaped_length = 0;

            while self.current_line.len() > *cursor {
                match self.current_line[*cursor] {
                    b'\\' => {
                        if quotation == b'\'' {
                            line.push(b'\\');
                        } else {
                            *cursor += 1;
                            if let Some(&c) = self.current_line.get(*cursor) {
                                line.push(b'\\');
                                line.push(c);
                                escaped_length = line.len();
                            }
                        }
                    }
                    c if c == quotation => {
                        *cursor += 1;

                        if quotation == b'\'' && self.current_line.get(*cursor) == Some(&b'\'') {
                            line.extend_from_slice(b"''");
                        } else {
                            value.extend_from_slice(&line);
                            value.push(quotation);
                            return Ok(value);
                        }
                    }
                    c => line.push(c),
                }
                *cursor += 1;
            }

            if is_blank_line {
                previous_line_was_newline = true;
                previous_line_was_terminated_with_backslash = false;
            } else if quotation == b'"'
                && (self.current_line.len() - rtrim_chars(&self.current_line, b"\\").len()) % 2 == 1
            {
                previous_line_was_newline = false;
                previous_line_was_terminated_with_backslash = true;
            } else {
                previous_line_was_newline = false;
                previous_line_was_terminated_with_backslash = false;

                // trailing white space is removed when the line is folded, unless it is escaped
                let kept = escaped_length.min(line.len());
                let rest = rtrim_chars(&line[kept..], b" \t").len();
                line.truncate(kept + rest);
            }

            value.extend_from_slice(&line);

            if self.has_more_lines() {
                *cursor = 0;
            }
            if !self.move_to_next_line() {
                break;
            }
        }

        Err(ParseException::message_only(
            "Malformed inline YAML string.",
        ))
    }

    fn lex_unquoted_string(&self, cursor: &mut usize) -> Result<Vec<u8>, ParseException> {
        let offset = *cursor;

        while *cursor < self.current_line.len() {
            if matches!(
                self.current_line[*cursor],
                b'[' | b']' | b'{' | b'}' | b',' | b':'
            ) {
                break;
            }

            if matches!(self.current_line[*cursor], b' ' | b'\t')
                && self.current_line.get(*cursor + 1) == Some(&b'#')
            {
                break;
            }

            *cursor += 1;
        }

        if *cursor == offset {
            return Err(ParseException::message_only(
                "Malformed unquoted YAML string.",
            ));
        }

        Ok(self.current_line[offset..*cursor].to_vec())
    }

    fn lex_inline_anchor_or_alias(&self, cursor: &mut usize) -> Vec<u8> {
        let offset = *cursor;
        *cursor += 1;

        while *cursor < self.current_line.len()
            && !matches!(
                self.current_line[*cursor],
                b' ' | b'\t' | b',' | b'[' | b']' | b'{' | b'}'
            )
        {
            *cursor += 1;
        }

        substr(&self.current_line, offset)[..*cursor - offset].to_vec()
    }

    fn lex_inline_structure(
        &mut self,
        sh: &mut Shared<'_>,
        cursor: &mut usize,
        closing_tag: u8,
        consume_until_eol: bool,
    ) -> Result<Vec<u8>, ParseException> {
        sh.state.enter_nesting_level(
            self.get_real_current_line_nb() + 1,
            Some(&self.current_line),
            self.filename.as_deref(),
        )?;
        let result = self.do_lex_inline_structure(sh, cursor, closing_tag, consume_until_eol);
        sh.state.leave_nesting_level();
        result
    }

    fn do_lex_inline_structure(
        &mut self,
        sh: &mut Shared<'_>,
        cursor: &mut usize,
        closing_tag: u8,
        consume_until_eol: bool,
    ) -> Result<Vec<u8>, ParseException> {
        let mut value = vec![self.current_line[*cursor]];
        *cursor += 1;

        loop {
            self.consume_whitespaces(cursor);

            while let Some(&c) = self.current_line.get(*cursor) {
                match c {
                    b'"' | b'\'' => value.extend(self.lex_inline_quoted_string(cursor)?),
                    b':' | b',' => {
                        value.push(c);
                        *cursor += 1;
                    }
                    b'{' => value.extend(self.lex_inline_structure(sh, cursor, b'}', false)?),
                    b'[' => value.extend(self.lex_inline_structure(sh, cursor, b']', false)?),
                    b'&' | b'*' => value.extend(self.lex_inline_anchor_or_alias(cursor)),
                    c if c == closing_tag => {
                        value.push(c);
                        *cursor += 1;

                        if consume_until_eol && *cursor < self.current_line.len() {
                            let whitespaces = strspn(&self.current_line, b" ", *cursor) + *cursor;
                            if whitespaces < self.current_line.len()
                                && self.current_line[whitespaces] != b'#'
                            {
                                return Err(ParseException::message_only(format!(
                                    "Unexpected token \"{}\".",
                                    lossy(trim(substr(&self.current_line, *cursor)))
                                )));
                            }
                        }

                        return Ok(value);
                    }
                    b'#' => break,
                    _ => value.extend(self.lex_unquoted_string(cursor)?),
                }

                if self.consume_whitespaces(cursor) {
                    value.push(b' ');
                }
            }

            if self.has_more_lines() {
                *cursor = 0;
            }
            if !self.move_to_next_line() {
                break;
            }
        }

        Err(ParseException::message_only(
            "Malformed inline YAML string.",
        ))
    }

    fn consume_whitespaces(&mut self, cursor: &mut usize) -> bool {
        let mut whitespaces_consumed = 0;

        loop {
            let whitespace_only_token_length = strspn(&self.current_line, b" \t", *cursor);
            whitespaces_consumed += whitespace_only_token_length;
            *cursor += whitespace_only_token_length;

            if *cursor < self.current_line.len() {
                return whitespaces_consumed > 0;
            }

            if self.has_more_lines() {
                *cursor = 0;
            }
            if !self.move_to_next_line() {
                break;
            }
        }

        whitespaces_consumed > 0
    }
}

/// `str_replace(["\r\n", "\r"], "\n", $value)`.
fn normalize_line_breaks(value: &[u8]) -> Vec<u8> {
    let mut out = Vec::with_capacity(value.len());
    let mut i = 0;
    while i < value.len() {
        if value[i] == b'\r' {
            out.push(b'\n');
            i += if value.get(i + 1) == Some(&b'\n') {
                2
            } else {
                1
            };
        } else {
            out.push(value[i]);
            i += 1;
        }
    }
    out
}

/// `preg_replace('/\s*(#.*)?$/A', '', $rest)`: what is left of the rest
/// of the line after a quoted value, when it is not only blanks and a
/// comment.
fn strip_trailing_blanks_and_comment(rest: &[u8]) -> Vec<u8> {
    let blanks = rest
        .iter()
        .take_while(|b| matches!(b, b' ' | b'\t' | b'\n' | 0x0b | 0x0c | b'\r'))
        .count();
    let after = &rest[blanks..];
    let comment_end = if after.first() == Some(&b'#') {
        after.iter().take_while(|&&b| b != b'\n').count()
    } else {
        0
    };
    // `$`: the end, or before a final newline
    let tail = &after[comment_end..];
    if tail.is_empty() || tail == b"\n" {
        Vec::new()
    } else {
        rest.to_vec()
    }
}

/// An anchor's name and the value after it.
type Anchored = (Vec<u8>, Vec<u8>);

/// `'&' === $value[0] && self::preg_match(self::REFERENCE_PATTERN, $value, $matches)`:
/// the anchor's name and the value after it.
fn reference(value: Option<&[u8]>) -> Result<Option<Anchored>, ParseException> {
    let Some(value) = value.filter(|v| v.first() == Some(&b'&')) else {
        return Ok(None);
    };
    Ok(REFERENCE
        .captures(utf8(value)?.as_bytes())
        .map(|c| (c["ref"].to_vec(), c["value"].to_vec())))
}

/// Turns "!tag &anchor value" into "&anchor !tag value", as both orders are allowed.
///
/// Built-in tags are left alone: "!!str &foo" is the "&foo" string, as in flow collections.
fn move_anchor_before_tag(value: &[u8]) -> Vec<u8> {
    if value.starts_with(b"!!") {
        return value.to_vec();
    }
    match TAG_THEN_ANCHOR.captures(value) {
        Some(captures) => {
            let end = captures.get(0).map_or(0, |m| m.end());
            let mut out = captures["anchor"].to_vec();
            out.push(b' ');
            out.extend_from_slice(&captures["tag"]);
            out.extend_from_slice(&value[end..]);
            out
        }
        None => value.to_vec(),
    }
}

/// Trim the tag on top of the value.
///
/// Prevent values such as "!foo {quz: bar}" to be considered as
/// a mapping block.
fn trim_tag(value: &[u8]) -> &[u8] {
    if value.first() == Some(&b'!') {
        let len = value[1..]
            .iter()
            .take_while(|b| !matches!(b, b' ' | b'\r' | b'\n'))
            .count();
        return ltrim_chars(&value[1..=len], b" ");
    }
    value
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn mapping_line_splits_as_pcre() {
        assert_eq!(mapping_line("a: b"), Some(("a", Some("b"))));
        assert_eq!(mapping_line("a b:  c"), Some(("a b", Some("c"))));
        assert_eq!(mapping_line("a:b"), None);
        assert_eq!(mapping_line("a:"), Some(("a", None)));
        assert_eq!(mapping_line("'a': b"), Some(("'a'", Some("b"))));
        assert_eq!(mapping_line("!foo : v"), Some(("!foo", Some("v"))));
        assert_eq!(mapping_line("!!str : v"), Some(("!!str", Some("v"))));
        assert_eq!(mapping_line("!t key: v"), Some(("!t key", Some("v"))));
        assert_eq!(mapping_line("a::b: c"), Some(("a::b", Some("c"))));
        assert_eq!(mapping_line("- a"), None);
        assert_eq!(mapping_line("{a: b}"), None);
    }

    #[test]
    fn document_markers() {
        assert_eq!(start_marker(b"---"), Some(3));
        assert_eq!(start_marker(b"---\nfoo"), Some(3));
        assert_eq!(start_marker(b"---foo"), None);
        assert_eq!(start_marker(b"--- foo"), Some(4));
        assert_eq!(start_marker(b"--- %YAML:1.0\nfoo"), Some(13));
        assert_eq!(start_marker(b"--- !tag &a |\nfoo"), Some(13));
        assert_eq!(start_marker(b"--- |2x"), Some(4));
        assert_eq!(root_anchor(b"&a foo"), Some((0, 3)));
        assert_eq!(root_anchor(b"!t &a foo"), Some((3, 6)));
        assert_eq!(root_anchor(b"!t foo"), None);
        assert_eq!(end_marker(b"a\n...\n# c\n"), Some(2));
        assert_eq!(end_marker(b"a\n... x\n"), None);
        assert_eq!(other_document(b"a\n--- b\n"), Some((2, 7)));
        assert_eq!(other_document(b"a\n---b\n"), None);
    }
}
