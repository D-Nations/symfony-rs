//! Port of Symfony\Component\Yaml\Inline.php: inline YAML (flow
//! sequences and mappings, scalars) to PHP values and back.
//!
//! PHP keeps the parse flags, line number and file name in static
//! properties; here they live in an `Inline` value. As in PHP, `parse()`
//! resets the flags (and the file name) and keeps the line number, and
//! the nested calls PHP makes with default flags keep the static ones.
//!
//! The input is bytes, as a PHP string. Where upstream matches a `/u`
//! pattern through `Parser::preg_match()`, input that is not UTF-8 fails
//! with PCRE's message, as there. PHP constants and enums come from the
//! `Environment`'s registry; `unserialize()` creates only the classes it
//! allows (PLAN.md decision 3).
//!
//! Symfony source copyright (c) Fabien Potencier; distributed under MIT.
//!
//! Upstream: symfony/yaml Inline.php

use std::{collections::HashMap, sync::Arc, sync::LazyLock};

use php_value::{
    Key, PhpArray, PhpObject, PhpString, Value,
    constants::Registry,
    datetime::DateTimeImmutable,
    numeric::{
        Number, float_to_string, hexdec, is_numeric, octdec, string_to_float, string_to_int,
    },
    object::{ArrayObject, EnumCase, StdClass, to_array},
    serialize::{Classes, serialize, unserialize},
};
use regex::bytes::Regex;

use crate::{
    Flags,
    escaper::{
        escape_with_double_quotes, escape_with_single_quotes, requires_double_quoting,
        requires_single_quoting, str_replace,
    },
    exception::{DumpException, ParseException},
    parser_state::ParserState,
    tag::TaggedValue,
    timestamp,
    unescaper::{unescape_double_quoted_string, unescape_single_quoted_string},
};

/// What PHP finds in its global state: constants and enums, and the
/// classes `unserialize()` may create.
#[derive(Debug, Clone, Default)]
pub struct Environment {
    pub constants: Registry,
    pub classes: Classes,
}

impl Environment {
    /// PHP's core constants and the `Symfony\Component\Yaml\Yaml::*`
    /// flag constants (defined whenever the class is loaded); no classes.
    pub fn new() -> Self {
        let mut constants = Registry::with_core();
        for (name, flag) in Flags::NAMED {
            constants.define(
                format!(r"Symfony\Component\Yaml\Yaml::{name}"),
                Value::Int(i64::from(flag.0)),
            );
        }
        Self {
            constants,
            classes: Classes::new(),
        }
    }
}

/// Anchors defined so far (`&name`), keyed as PHP array keys.
pub type References = HashMap<Key, Value>;

/// PCRE's message when a `/u` pattern meets bytes that are not UTF-8.
const MALFORMED_UTF8: &str = "Malformed UTF-8 characters, possibly incorrectly encoded";

fn regex(pattern: &str) -> Regex {
    Regex::new(pattern).unwrap_or_else(|e| unreachable!("constant pattern {pattern}: {e}"))
}

/// `REGEX_QUOTED_STRING`, anchored (`/A`), with `/u`. Possessive
/// quantifiers become greedy: the classes exclude what follows them, so
/// the matches are the same.
static QUOTED: LazyLock<Regex> =
    LazyLock::new(|| regex(r#"^(?:"([^"\\]*(?:\\.[^"\\]*)*)"|'([^']*(?:''[^']*)*)')"#));
static COMMENT: LazyLock<Regex> = LazyLock::new(|| regex(r"(?-u)[ \t]+#"));
static ANCHOR: LazyLock<Regex> =
    LazyLock::new(|| regex(r"(?-u)^&([^ \t,\[\]\{\}\n]+)[ \t\n\x0b\x0c\r]*"));
static OCTAL: LazyLock<Regex> = LazyLock::new(|| regex(r"(?-u)^(?:\+|-)?0o([0-7_]+)\n?\z"));
static INTEGER: LazyLock<Regex> = LazyLock::new(|| regex(r"(?-u)^[+-]?[0-9][0-9_]*\n?\z"));
static HEX: LazyLock<Regex> = LazyLock::new(|| regex(r"(?-u)^0x[0-9a-fA-F_]+\n?\z"));
static FLOAT: LazyLock<Regex> =
    LazyLock::new(|| regex(r"(?-u)^(-|\+)?[0-9][0-9_]*(\.[0-9_]+)?\n?\z"));
static DUMP_NUMBER: LazyLock<Regex> =
    LazyLock::new(|| regex(r"(?-u)^[+-]?[0-9][0-9_]*(\.[0-9_]+)?\n?\z"));
static DUMP_OCTAL: LazyLock<Regex> = LazyLock::new(|| regex(r"(?-u)^[+-]?0o[0-7_]+\n?\z"));
static BASE64: LazyLock<Regex> = LazyLock::new(|| regex(r"(?i-u)^[A-Z0-9+/]+={0,2}\n?\z"));

/// A delimiter of `parseScalar()`, as the regex fragments upstream joins.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Delimiter {
    Byte(u8),
    /// `(?<!:):(?!:)`: a colon that is not part of `::`.
    LoneColon,
}

impl Delimiter {
    fn text(self) -> String {
        match self {
            Self::Byte(b) => char::from(b).to_string(),
            Self::LoneColon => "(?<!:):(?!:)".to_owned(),
        }
    }

    fn matches(self, s: &[u8], at: usize) -> bool {
        match self {
            Self::Byte(b) => s.get(at) == Some(&b),
            Self::LoneColon => {
                s.get(at) == Some(&b':')
                    && (at == 0 || s[at - 1] != b':')
                    && s.get(at + 1) != Some(&b':')
            }
        }
    }
}

const SEQUENCE_DELIMITERS: &[Delimiter] = &[Delimiter::Byte(b','), Delimiter::Byte(b']')];
const MAPPING_DELIMITERS: &[Delimiter] = &[
    Delimiter::Byte(b','),
    Delimiter::Byte(b'}'),
    Delimiter::Byte(b'\n'),
];
const KEY_DELIMITERS: &[Delimiter] = &[Delimiter::Byte(b':'), Delimiter::Byte(b' ')];

/// PHP's `trim()` characters.
fn is_trim(b: u8) -> bool {
    matches!(b, b' ' | b'\t' | b'\n' | b'\r' | 0 | 0x0b)
}

pub(crate) fn trim(s: &[u8]) -> &[u8] {
    let start = s.iter().position(|&b| !is_trim(b)).unwrap_or(s.len());
    let end = s
        .iter()
        .rposition(|&b| !is_trim(b))
        .map_or(start, |p| p + 1);
    &s[start..end]
}

fn ltrim_chars<'a>(s: &'a [u8], chars: &[u8]) -> &'a [u8] {
    let start = s.iter().position(|b| !chars.contains(b)).unwrap_or(s.len());
    &s[start..]
}

/// PHP truthiness of a string: not `""` and not `"0"`.
fn truthy(s: &[u8]) -> bool {
    !s.is_empty() && s != b"0"
}

fn lossy(s: &[u8]) -> std::borrow::Cow<'_, str> {
    String::from_utf8_lossy(s)
}

fn string(s: &[u8]) -> Value {
    Value::String(PhpString::from(s))
}

/// `is_scalar($v ?? '') || $v instanceof \Stringable`, then `(string)`.
fn scalar_string(value: &Value) -> Option<PhpString> {
    match value {
        Value::Array(_) | Value::Object(_) => None,
        other => other.to_php_string(),
    }
}

/// `$output[$key] = $value` with PHP's key conversion.
fn set(output: &mut PhpArray, key: &Value, value: Value) -> Result<(), ParseException> {
    let key =
        Key::from_value(key).ok_or_else(|| ParseException::message_only("Illegal offset type"))?;
    output.insert(key, value);
    Ok(())
}

/// `isset($output[$key])`: present and not null.
fn isset(output: &PhpArray, key: &Value) -> bool {
    Key::from_value(key).is_some_and(|k| output.get(&k).is_some_and(|v| !v.is_null()))
}

/// The inline parser and dumper, with the state PHP keeps in statics.
#[derive(Debug, Clone)]
#[expect(clippy::struct_excessive_bools, reason = "upstream's flags")]
pub struct Inline {
    pub parsed_line_number: i64,
    pub parsed_filename: Option<String>,
    exception_on_invalid_type: bool,
    object_support: bool,
    object_for_map: bool,
    constant_support: bool,
    environment: Arc<Environment>,
}

impl Default for Inline {
    fn default() -> Self {
        Self::new(Arc::new(Environment::new()))
    }
}

impl Inline {
    pub fn new(environment: Arc<Environment>) -> Self {
        Self {
            parsed_line_number: -1,
            parsed_filename: None,
            exception_on_invalid_type: false,
            object_support: false,
            object_for_map: false,
            constant_support: false,
            environment,
        }
    }

    pub fn environment(&self) -> &Arc<Environment> {
        &self.environment
    }

    /// `Inline::initialize()`.
    pub fn initialize(
        &mut self,
        flags: Flags,
        parsed_line_number: Option<i64>,
        parsed_filename: Option<String>,
    ) {
        self.exception_on_invalid_type = flags.contains(Flags::PARSE_EXCEPTION_ON_INVALID_TYPE);
        self.object_support = flags.contains(Flags::PARSE_OBJECT);
        self.object_for_map = flags.contains(Flags::PARSE_OBJECT_FOR_MAP);
        self.constant_support = flags.contains(Flags::PARSE_CONSTANT);
        self.parsed_filename = parsed_filename;
        if let Some(line) = parsed_line_number {
            self.parsed_line_number = line;
        }
    }

    fn error(&self, message: impl Into<String>, snippet: Option<&[u8]>) -> ParseException {
        ParseException::new(
            message,
            self.parsed_line_number + 1,
            snippet.map(Into::into),
            self.parsed_filename.clone(),
        )
    }

    /// `Inline::parse()`: converts an inline YAML string to a PHP value.
    pub fn parse(
        &mut self,
        value: &[u8],
        flags: Flags,
        references: &mut References,
        state: Option<&mut ParserState>,
    ) -> Result<Value, ParseException> {
        self.initialize(flags, None, None);
        let mut own_state = ParserState::default();
        let state = state.unwrap_or(&mut own_state);

        let value = trim(value);
        if value.is_empty() {
            return Ok(string(b""));
        }

        let mut i = 0;
        let tag = self.parse_tag(value, &mut i, flags)?;

        // a tag without value, possibly followed by a comment
        if let Some(tag) = &tag
            && !tag.is_empty()
            && (i >= value.len() || value[i] == b'#')
        {
            if tag.starts_with("php/") {
                return Err(self.error(format!("Missing value for tag \"{tag}\"."), Some(value)));
            }
            return Ok(tagged(tag, string(b"")));
        }

        let result = match value.get(i) {
            Some(b'[') => {
                let r = self.parse_sequence(state, value, flags, &mut i, references)?;
                i += 1;
                r
            }
            Some(b'{') => {
                let r = self.parse_mapping(state, value, flags, &mut i, references)?;
                i += 1;
                r
            }
            _ => {
                let mut is_quoted = false;
                self.parse_scalar(
                    value,
                    flags,
                    None,
                    &mut i,
                    true,
                    references,
                    &mut is_quoted,
                    Some(state),
                )?
            }
        };

        // some comments are allowed at the end
        let rest = value.get(i..).unwrap_or(b"");
        if truthy(strip_trailing_comment(rest)) {
            return Err(self.error(
                format!("Unexpected characters near \"{}\".", lossy(rest)),
                Some(value),
            ));
        }

        Ok(match tag {
            Some(tag) if !tag.is_empty() => tagged(&tag, result),
            _ => result,
        })
    }

    /// `Inline::parseScalar($scalar)`: one scalar, evaluated, with no
    /// delimiters and the static flags.
    pub fn parse_scalar_value(&mut self, scalar: &[u8]) -> Result<Value, ParseException> {
        self.parse_scalar_simple(scalar, Flags::NONE, true)
    }

    /// `parseScalar()` with the defaults PHP code uses when it passes only
    /// the string: no delimiters, evaluate, fresh references and state.
    fn parse_scalar_simple(
        &mut self,
        scalar: &[u8],
        flags: Flags,
        evaluate: bool,
    ) -> Result<Value, ParseException> {
        let mut i = 0;
        let mut references = References::new();
        let mut is_quoted = false;
        self.parse_scalar(
            scalar,
            flags,
            None,
            &mut i,
            evaluate,
            &mut references,
            &mut is_quoted,
            None,
        )
    }

    /// `Inline::parseScalar()`.
    #[allow(clippy::too_many_arguments)]
    fn parse_scalar(
        &mut self,
        scalar: &[u8],
        flags: Flags,
        delimiters: Option<&[Delimiter]>,
        i: &mut usize,
        evaluate: bool,
        references: &mut References,
        is_quoted: &mut bool,
        state: Option<&mut ParserState>,
    ) -> Result<Value, ParseException> {
        if matches!(scalar.get(*i), Some(b'"' | b'\'')) {
            // quoted scalar
            *is_quoted = true;
            let output = self.parse_quoted_scalar(scalar, i)?;

            if let Some(delimiters) = delimiters {
                let tmp = ltrim_chars(scalar.get(*i..).unwrap_or(b""), b" \n");
                if tmp.is_empty() {
                    let expected: String = delimiters.iter().map(|d| d.text()).collect();
                    return Err(self.error(
                        format!("Unexpected end of line, expected one of \"{expected}\"."),
                        Some(scalar),
                    ));
                }
                if !delimiters.iter().any(|d| *d == Delimiter::Byte(tmp[0])) {
                    return Err(self.error(
                        format!("Unexpected characters ({}).", lossy(&scalar[*i..])),
                        Some(scalar),
                    ));
                }
            }
            return Ok(Value::String(output));
        }

        // "normal" string
        *is_quoted = false;
        let output: Vec<u8> = match delimiters {
            None | Some([]) => {
                let mut output = scalar.get(*i..).unwrap_or(b"").to_vec();
                *i += output.len();
                // remove comments
                if let Some(m) = COMMENT.find(&output) {
                    output.truncate(m.start());
                }
                output
            }
            Some(delimiters) => {
                let rest = scalar.get(*i..).unwrap_or(b"");
                match find_delimiter(rest, delimiters) {
                    Some(end) => {
                        *i += end;
                        trim(&rest[..end]).to_vec()
                    }
                    None => {
                        return Err(ParseException::new(
                            format!("Malformed inline YAML string: \"{}\".", lossy(scalar)),
                            self.parsed_line_number + 1,
                            None,
                            self.parsed_filename.clone(),
                        ));
                    }
                }
            }
        };

        // a non-quoted string cannot start with @ or ` (reserved) nor with a scalar indicator (| or >)
        if truthy(&output) && matches!(output[0], b'@' | b'`' | b'|' | b'>' | b'%') {
            return Err(self.error(
                format!(
                    "The reserved indicator \"{}\" cannot start a plain scalar; you need to quote the scalar.",
                    char::from(output[0])
                ),
                Some(&output),
            ));
        }

        if evaluate {
            let mut own_state = ParserState::default();
            let state = state.unwrap_or(&mut own_state);
            return self.evaluate_scalar(state, &output, flags, references, is_quoted);
        }
        Ok(string(&output))
    }

    /// `parseQuotedScalar()`.
    fn parse_quoted_scalar(
        &self,
        scalar: &[u8],
        i: &mut usize,
    ) -> Result<PhpString, ParseException> {
        let subject = scalar.get(*i..).unwrap_or(b"");
        if std::str::from_utf8(subject).is_err() {
            return Err(ParseException::message_only(MALFORMED_UTF8));
        }
        let Some(m) = QUOTED.find(subject) else {
            return Err(self.error(
                format!("Malformed inline YAML string: \"{}\".", lossy(subject)),
                Some(scalar),
            ));
        };
        let matched = m.as_bytes();
        let inner = &matched[1..matched.len() - 1];
        let output = if scalar[*i] == b'"' {
            // `subject` is UTF-8 and the match ends on a quote.
            let text = std::str::from_utf8(inner)
                .map_err(|_| ParseException::message_only(MALFORMED_UTF8))?;
            unescape_double_quoted_string(text)?
        } else {
            unescape_single_quoted_string(inner)
        };
        *i += matched.len();
        Ok(PhpString::from(output))
    }

    /// `parseSequence()`.
    fn parse_sequence(
        &mut self,
        state: &mut ParserState,
        sequence: &[u8],
        flags: Flags,
        i: &mut usize,
        references: &mut References,
    ) -> Result<Value, ParseException> {
        state.enter_nesting_level(
            self.parsed_line_number + 1,
            None,
            self.parsed_filename.as_deref(),
        )?;
        let result = self.parse_sequence_items(state, sequence, flags, i, references);
        state.leave_nesting_level();
        result
    }

    #[expect(clippy::too_many_lines, reason = "one function, as upstream's")]
    fn parse_sequence_items(
        &mut self,
        state: &mut ParserState,
        sequence: &[u8],
        flags: Flags,
        i: &mut usize,
        references: &mut References,
    ) -> Result<Value, ParseException> {
        let mut output = PhpArray::new();
        let len = sequence.len();
        *i += 1;

        // [foo, bar, ...]
        let mut last_token: Option<&str> = None;
        while *i < len {
            if sequence[*i] == b']' {
                return Ok(Value::Array(output));
            }
            if sequence[*i] == b',' || sequence[*i] == b' ' {
                if sequence[*i] == b',' && last_token.is_none_or(|t| t == "separator") {
                    push(&mut output, Value::Null)?;
                } else if sequence[*i] == b',' {
                    last_token = Some("separator");
                }
                *i += 1;
                continue;
            }

            let mut tag = self.parse_tag(sequence, i, flags)?;
            let anchor_ref = Self::parse_anchor(sequence, i);
            if anchor_ref.is_some() && tag.is_none() {
                tag = self.parse_tag(sequence, i, flags)?;
            }

            if let Some(anchor) = &anchor_ref
                && (*i >= len || sequence[*i] == b',' || sequence[*i] == b']')
            {
                let mut value = Value::Null;
                references.insert(anchor.clone(), value.clone());
                if let Some(tag) = &tag
                    && !tag.is_empty()
                {
                    value = tagged(tag, value);
                }
                push(&mut output, value)?;
                last_token = Some("value");
                continue;
            }

            if *i >= len {
                break;
            }

            let value = match sequence[*i] {
                b'[' => self.parse_sequence(state, sequence, flags, i, references)?,
                b'{' => self.parse_mapping(state, sequence, flags, i, references)?,
                _ => {
                    let mut is_quoted = false;
                    let mut value = self.parse_scalar(
                        sequence,
                        flags,
                        Some(SEQUENCE_DELIMITERS),
                        i,
                        tag.is_none(),
                        references,
                        &mut is_quoted,
                        Some(state),
                    )?;

                    // the value can be an array if a reference has been resolved to an array var
                    if let Value::String(s) = &value
                        && !is_quoted
                        && contains(s, b": ")
                    {
                        // embedded mapping?
                        let mut j = *i;
                        let mut mapping_value = s.as_bytes().to_vec();
                        let mut mapping_exception;
                        loop {
                            let mut wrapped = b"{".to_vec();
                            wrapped.extend_from_slice(&mapping_value);
                            wrapped.push(b'}');
                            let mut pos = 0;
                            match self.parse_mapping(state, &wrapped, flags, &mut pos, references) {
                                Ok(mapping) => {
                                    value = mapping;
                                    *i = j;
                                    mapping_exception = None;
                                    break;
                                }
                                Err(exception) => {
                                    mapping_exception = Some(exception);
                                    if j >= len {
                                        break;
                                    }
                                    mapping_value.push(sequence[j]);
                                    j += 1;
                                    if j >= len {
                                        break;
                                    }
                                    let mut quoted = false;
                                    let next = self.parse_scalar(
                                        sequence,
                                        flags,
                                        Some(SEQUENCE_DELIMITERS),
                                        &mut j,
                                        tag.is_none(),
                                        references,
                                        &mut quoted,
                                        None,
                                    )?;
                                    mapping_value.extend_from_slice(&concat_string(&next));
                                }
                            }
                            if j >= len {
                                break;
                            }
                        }
                        if let Some(exception) = mapping_exception {
                            return Err(exception);
                        }
                    }
                    *i -= 1;
                    value
                }
            };

            if let Some(anchor) = &anchor_ref {
                references.insert(anchor.clone(), value.clone());
            }
            let value = match &tag {
                Some(tag) if !tag.is_empty() => tagged(tag, value),
                _ => value,
            };
            push(&mut output, value)?;
            last_token = Some("value");
            *i += 1;
        }

        Err(ParseException::new(
            format!("Malformed inline YAML string: \"{}\".", lossy(sequence)),
            self.parsed_line_number + 1,
            None,
            self.parsed_filename.clone(),
        ))
    }

    /// `parseMapping()`.
    fn parse_mapping(
        &mut self,
        state: &mut ParserState,
        mapping: &[u8],
        flags: Flags,
        i: &mut usize,
        references: &mut References,
    ) -> Result<Value, ParseException> {
        state.enter_nesting_level(
            self.parsed_line_number + 1,
            None,
            self.parsed_filename.as_deref(),
        )?;
        let result = self.parse_mapping_items(state, mapping, flags, i, references);
        state.leave_nesting_level();
        result
    }

    fn mapping_error(&self, message: impl Into<String>, mapping: &[u8]) -> ParseException {
        ParseException::new(
            message,
            self.parsed_line_number + 1,
            Some(mapping.into()),
            None,
        )
    }

    #[expect(clippy::too_many_lines, reason = "one function, as upstream's")]
    fn parse_mapping_items(
        &mut self,
        state: &mut ParserState,
        mapping: &[u8],
        flags: Flags,
        i: &mut usize,
        references: &mut References,
    ) -> Result<Value, ParseException> {
        let mut output = PhpArray::new();
        let len = mapping.len();
        *i += 1;
        let mut allow_overwrite = false;

        // {foo: bar, bar:foo, ...}
        'mapping: while *i < len {
            match mapping[*i] {
                b' ' | b',' | b'\n' => {
                    *i += 1;
                    continue;
                }
                b'}' => {
                    if self.object_for_map {
                        return Ok(StdClass::object(output));
                    }
                    return Ok(Value::Array(output));
                }
                _ => {}
            }

            // key
            let offset_before_key_parsing = *i;
            let is_key_quoted = matches!(mapping[*i], b'"' | b'\'');
            let mut quoted = false;
            let mut key = self.parse_scalar(
                mapping,
                flags,
                Some(KEY_DELIMITERS),
                i,
                false,
                references,
                &mut quoted,
                None,
            )?;

            if offset_before_key_parsing == *i {
                return Err(self.mapping_error("Missing mapping key.", mapping));
            }

            if let Value::String(k) = &key
                && (k.as_bytes() == b"!php/const" || k.as_bytes() == b"!php/enum")
            {
                let mut joined = k.as_bytes().to_vec();
                joined.push(b' ');
                let mut quoted = false;
                let rest = self.parse_scalar(
                    mapping,
                    flags,
                    Some(&[Delimiter::LoneColon]),
                    i,
                    false,
                    references,
                    &mut quoted,
                    None,
                )?;
                joined.extend_from_slice(&concat_string(&rest));
                let mut quoted = false;
                key = self.evaluate_scalar(
                    state,
                    &joined,
                    flags,
                    &mut References::new(),
                    &mut quoted,
                )?;
            }

            if self.object_for_map
                && let Value::String(k) = &key
                && k.first() == Some(&0)
            {
                return Err(self.mapping_error(
                    "Mapping keys starting with a NUL byte are not supported with the \"Yaml::PARSE_OBJECT_FOR_MAP\" flag.",
                    mapping,
                ));
            }

            match mapping[*i..].iter().position(|&b| b == b':') {
                Some(p) => *i += p,
                None => break,
            }

            if !is_key_quoted && let Value::String(k) = &key {
                let mut quoted = false;
                let evaluated_key =
                    self.evaluate_scalar(state, k, flags, references, &mut quoted)?;
                let incompatible = !k.is_empty()
                    && !evaluated_key.identical(&key)
                    && (!matches!(evaluated_key, Value::String(_)) || k[0] == b'!')
                    && !matches!(evaluated_key, Value::Int(_));
                if incompatible {
                    return Err(self.mapping_error(
                        "Implicit casting of incompatible mapping keys to strings is not supported. Quote your evaluable mapping keys instead.",
                        mapping,
                    ));
                }
            }

            if !is_key_quoted
                && !matches!(
                    mapping.get(*i + 1),
                    Some(b' ' | b',' | b'[' | b']' | b'{' | b'}' | b'\n')
                )
            {
                return Err(self.mapping_error(
                    "Colons must be followed by a space or an indication character (i.e. \" \", \",\", \"[\", \"]\", \"{\", \"}\").",
                    mapping,
                ));
            }

            let is_merge_key =
                !is_key_quoted && key.as_string().is_some_and(|k| k.as_bytes() == b"<<");
            if is_merge_key {
                allow_overwrite = true;
            }

            while *i < len {
                if matches!(mapping[*i], b':' | b' ' | b'\n') {
                    *i += 1;
                    continue;
                }

                let mut tag = self.parse_tag(mapping, i, flags)?;
                let anchor_ref = Self::parse_anchor(mapping, i);
                if anchor_ref.is_some() && tag.is_none() {
                    tag = self.parse_tag(mapping, i, flags)?;
                }

                if let Some(anchor) = &anchor_ref
                    && (*i >= len || matches!(mapping[*i], b',' | b'}' | b'\n'))
                {
                    let value = Value::Null;
                    references.insert(anchor.clone(), value.clone());
                    if !is_merge_key {
                        if allow_overwrite || !isset(&output, &key) {
                            let value = match &tag {
                                Some(tag) => tagged(tag, value),
                                None => value,
                            };
                            set(&mut output, &key, value)?;
                        } else {
                            return Err(self.duplicate_key(&key, mapping));
                        }
                    }
                    continue 'mapping;
                }

                if *i >= len {
                    break;
                }

                match mapping[*i] {
                    b'[' => {
                        // nested sequence
                        let value = self.parse_sequence(state, mapping, flags, i, references)?;
                        if let Some(anchor) = &anchor_ref {
                            references.insert(anchor.clone(), value.clone());
                        }
                        // Spec: Keys MUST be unique; first one wins.
                        // Parser cannot abort this mapping earlier, since lines
                        // are processed sequentially.
                        // But overwriting is allowed when a merge node is used in current block.
                        if is_merge_key {
                            if let Value::Array(items) = &value {
                                for parsed_value in items.values() {
                                    let parsed_value =
                                        if self.object_for_map && is_std_class(parsed_value) {
                                            Value::Array(to_array(parsed_value))
                                        } else {
                                            parsed_value.clone()
                                        };
                                    let Value::Array(parsed) = parsed_value else {
                                        return Err(self.mapping_error(
                                            "Merge items must be arrays.",
                                            mapping,
                                        ));
                                    };
                                    output.union(&parsed);
                                }
                            }
                        } else if allow_overwrite || !isset(&output, &key) {
                            let value = match &tag {
                                Some(tag) => tagged(tag, value),
                                None => value,
                            };
                            set(&mut output, &key, value)?;
                        } else {
                            return Err(self.duplicate_key(&key, mapping));
                        }
                    }
                    b'{' => {
                        // nested mapping
                        let value = self.parse_mapping(state, mapping, flags, i, references)?;
                        if let Some(anchor) = &anchor_ref {
                            references.insert(anchor.clone(), value.clone());
                        }
                        if is_merge_key {
                            output.union(&to_array(&value));
                        } else if allow_overwrite || !isset(&output, &key) {
                            let value = match &tag {
                                Some(tag) => tagged(tag, value),
                                None => value,
                            };
                            set(&mut output, &key, value)?;
                        } else {
                            return Err(self.duplicate_key(&key, mapping));
                        }
                    }
                    _ => {
                        let mut is_value_quoted = false;
                        let value = self.parse_scalar(
                            mapping,
                            flags,
                            Some(MAPPING_DELIMITERS),
                            i,
                            tag.is_none(),
                            references,
                            &mut is_value_quoted,
                            Some(state),
                        )?;
                        if let Some(anchor) = &anchor_ref {
                            references.insert(anchor.clone(), value.clone());
                        }
                        if is_merge_key {
                            let value = if self.object_for_map && is_std_class(&value) {
                                Value::Array(to_array(&value))
                            } else {
                                value
                            };
                            let Value::Array(merged) = value else {
                                return Err(self.mapping_error(
                                    "YAML merge keys used with a scalar value instead of an array.",
                                    mapping,
                                ));
                            };
                            output.union(&merged);
                        } else if allow_overwrite || !isset(&output, &key) {
                            let value = match &tag {
                                Some(tag) => tagged(tag, value),
                                None => value,
                            };
                            set(&mut output, &key, value)?;
                        } else {
                            return Err(self.duplicate_key(&key, mapping));
                        }
                        *i -= 1;
                    }
                }
                *i += 1;
                continue 'mapping;
            }
        }

        Err(ParseException::new(
            format!("Malformed inline YAML string: \"{}\".", lossy(mapping)),
            self.parsed_line_number + 1,
            None,
            self.parsed_filename.clone(),
        ))
    }

    fn duplicate_key(&self, key: &Value, mapping: &[u8]) -> ParseException {
        let key = key.to_php_string().unwrap_or_default();
        self.mapping_error(format!("Duplicate key \"{key}\" detected."), mapping)
    }

    /// `evaluateScalar()`: evaluates scalars and replaces magic values.
    #[expect(
        clippy::cast_precision_loss,
        reason = "PHP's int and float conversions, offsets and lengths, as upstream"
    )]
    #[expect(clippy::too_many_lines, reason = "one function, as upstream's")]
    #[expect(clippy::similar_names, reason = "names as in the PHP source")]
    fn evaluate_scalar(
        &mut self,
        state: &mut ParserState,
        scalar: &[u8],
        flags: Flags,
        references: &mut References,
        is_quoted_string: &mut bool,
    ) -> Result<Value, ParseException> {
        *is_quoted_string = false;
        let scalar = trim(scalar);

        if let Some(name) = scalar.strip_prefix(b"*") {
            let mut value = name.to_vec();
            // remove comments
            if let Some(m) = COMMENT.find(&value) {
                value.truncate(m.start());
            }
            // an unquoted *
            if value.is_empty() {
                return Err(self.error(
                    "A reference must contain at least one character.",
                    Some(&value),
                ));
            }
            let Some(referenced) = references.get(&Key::from_bytes(&value)).cloned() else {
                return Err(self.error(
                    format!("Reference \"{}\" does not exist.", lossy(&value)),
                    Some(&value),
                ));
            };
            state.count_alias(
                &referenced,
                self.parsed_line_number + 1,
                None,
                self.parsed_filename.as_deref(),
            )?;
            return Ok(referenced);
        }

        let lower = scalar.to_ascii_lowercase();
        if lower == b"null" || scalar.is_empty() || scalar == b"~" {
            return Ok(Value::Null);
        }
        if lower == b"true" {
            return Ok(Value::Bool(true));
        }
        if lower == b"false" {
            return Ok(Value::Bool(false));
        }

        if scalar[0] == b'!' {
            return self.evaluate_tagged_scalar(scalar, flags, is_quoted_string);
        }

        if let Some(c) = OCTAL.captures(scalar) {
            let digits = str_replace(c.get(1).map_or(&b""[..], |m| m.as_bytes()), b"_", b"");
            let value = number(octdec(&digits));
            return Ok(if scalar[0] == b'-' {
                negate(value)
            } else {
                value
            });
        }

        if matches!(scalar[0], b'+' | b'-' | b'.') || scalar[0].is_ascii_digit() {
            let mut scalar = scalar.to_vec();
            if INTEGER.is_match(&scalar) {
                scalar = str_replace(&scalar, b"_", b"");
                if scalar.first() == Some(&b'+') {
                    scalar.remove(0);
                }
            }

            let ctype_digit = |s: &[u8]| !s.is_empty() && s.iter().all(u8::is_ascii_digit);
            if ctype_digit(&scalar) || (scalar.first() == Some(&b'-') && ctype_digit(&scalar[1..]))
            {
                // Out of range, or not the canonical form of its int.
                let cast = string_to_int(&scalar);
                return Ok(if cast.to_string().as_bytes() == scalar.as_slice() {
                    Value::Int(cast)
                } else {
                    string(&scalar)
                });
            }
            if is_numeric(&scalar) || HEX.is_match(&scalar) {
                let scalar = str_replace(&scalar, b"_", b"");
                return Ok(if scalar.starts_with(b"0x") {
                    number(hexdec(&scalar))
                } else {
                    Value::Float(string_to_float(&scalar))
                });
            }
            let lower = scalar.to_ascii_lowercase();
            if lower == b".inf" {
                return Ok(Value::Float(f64::INFINITY));
            }
            if lower == b".nan" {
                return Ok(Value::Float(f64::NAN));
            }
            if lower == b"-.inf" {
                return Ok(Value::Float(f64::NEG_INFINITY));
            }
            if FLOAT.is_match(&scalar) {
                return Ok(Value::Float(string_to_float(&str_replace(
                    &scalar, b"_", b"",
                ))));
            }
            if timestamp::is_timestamp(&scalar) {
                // When no timezone is provided in the parsed date, YAML spec says we must assume UTC.
                let Some(time) = timestamp::parse(&scalar) else {
                    // Some dates accepted by the regex are not valid dates.
                    return Err(self.error(
                        format!(
                            "The date \"{}\" could not be parsed as it is an invalid date.",
                            lossy(&scalar)
                        ),
                        Some(&scalar),
                    ));
                };
                if flags.contains(Flags::PARSE_DATETIME) {
                    return Ok(Value::Object(Arc::new(time)));
                }
                if time.microsecond() != 0 {
                    return Ok(Value::Float(
                        time.timestamp() as f64 + f64::from(time.microsecond()) / 1e6,
                    ));
                }
                return Ok(Value::Int(time.timestamp()));
            }
            return Ok(string(&scalar));
        }

        Ok(string(scalar))
    }

    /// The `'!' === $scalar[0]` branch of `evaluateScalar()`.
    #[expect(clippy::too_many_lines, reason = "one function, as upstream's")]
    fn evaluate_tagged_scalar(
        &mut self,
        scalar: &[u8],
        _flags: Flags,
        is_quoted_string: &mut bool,
    ) -> Result<Value, ParseException> {
        if scalar == b"!!str" || scalar == b"!!binary" {
            return Ok(string(b""));
        }
        if let Some(s) = scalar.strip_prefix(b"!!str ") {
            if matches!(s.first(), Some(b'"' | b'\'')) {
                *is_quoted_string = true;
                let mut i = 0;
                return Ok(Value::String(self.parse_quoted_scalar(s, &mut i)?));
            }
            return Ok(string(s));
        }
        if let Some(s) = scalar.strip_prefix(b"! ") {
            return Ok(string(s));
        }
        if scalar.starts_with(b"!php/object") {
            if self.object_support {
                if scalar.len() <= 12 {
                    return Err(self.error("Missing value for tag \"!php/object\".", Some(scalar)));
                }
                let serialized = self.parse_scalar_simple(&scalar[12..], Flags::NONE, true)?;
                let Some(serialized) = scalar_string(&serialized) else {
                    return Err(self.error(
                        format!(
                            "The \"!php/object\" tag only supports a string value, got \"{}\".",
                            serialized.debug_type()
                        ),
                        Some(scalar),
                    ));
                };
                let environment = &self.environment;
                return Ok(
                    unserialize(&serialized, &environment.classes, &environment.constants)
                        .unwrap_or(Value::Bool(false)),
                );
            }
            if self.exception_on_invalid_type {
                return Err(self.error(
                    "Object support when parsing a YAML file has been disabled.",
                    Some(scalar),
                ));
            }
            return Ok(Value::Null);
        }
        if scalar.starts_with(b"!php/const") {
            if self.constant_support {
                if scalar.len() <= 11 {
                    return Err(self.error("Missing value for tag \"!php/const\".", Some(scalar)));
                }
                let constant = self.parse_scalar_simple(&scalar[11..], Flags::NONE, false)?;
                let Some(constant) = scalar_string(&constant) else {
                    return Err(self.error(
                        format!(
                            "The \"!php/const\" tag only supports a string value, got \"{}\".",
                            constant.debug_type()
                        ),
                        Some(scalar),
                    ));
                };
                let name = constant.to_string_lossy();
                if let Some(value) = self.environment.constants.constant(&name) {
                    return Ok(value.clone());
                }
                return Err(self.error(
                    format!("The constant \"{name}\" is not defined."),
                    Some(scalar),
                ));
            }
            if self.exception_on_invalid_type {
                return Err(self.error(
                    format!(
                        "The string \"{}\" could not be parsed as a constant. Did you forget to pass the \"Yaml::PARSE_CONSTANT\" flag to the parser?",
                        lossy(scalar)
                    ),
                    Some(scalar),
                ));
            }
            return Ok(Value::Null);
        }
        if scalar.starts_with(b"!php/enum") {
            if self.constant_support {
                if scalar.len() <= 11 {
                    return Err(self.error("Missing value for tag \"!php/enum\".", Some(scalar)));
                }
                let enum_name = self.parse_scalar_simple(&scalar[10..], Flags::NONE, false)?;
                let Some(enum_name) = scalar_string(&enum_name) else {
                    return Err(self.error(
                        format!(
                            "The \"!php/enum\" tag only supports a string value, got \"{}\".",
                            enum_name.debug_type()
                        ),
                        Some(scalar),
                    ));
                };
                let mut enum_name = enum_name.to_string_lossy().into_owned();
                let use_name = enum_name.contains("::");
                let class = match enum_name.split_once("::") {
                    Some((class, _)) => class.to_owned(),
                    None => enum_name.clone(),
                };
                let constants = &self.environment.constants;
                if !constants.enum_exists(&class) {
                    return Err(self.error(
                        format!("The enum \"{class}\" is not defined."),
                        Some(scalar),
                    ));
                }
                if !use_name {
                    let cases = constants.cases(&class).unwrap_or(&[]);
                    return Ok(Value::Array(PhpArray::list(
                        cases.iter().cloned().map(Value::Object),
                    )));
                }
                let use_value = enum_name.ends_with("->value");
                if use_value {
                    enum_name.truncate(enum_name.len() - 7);
                }
                let Some(value) = constants.constant(&enum_name).cloned() else {
                    return Err(self.error(
                        format!("The string \"{enum_name}\" is not the name of a valid enum."),
                        Some(scalar),
                    ));
                };
                if !use_value {
                    return Ok(value);
                }
                let backing = value
                    .as_object()
                    .and_then(|o| o.downcast_ref::<EnumCase>())
                    .and_then(|case| case.value().cloned());
                return match backing {
                    Some(backing) => Ok(backing),
                    None => Err(self.error(
                        format!("The enum \"{enum_name}\" defines no value next to its name."),
                        Some(scalar),
                    )),
                };
            }
            if self.exception_on_invalid_type {
                return Err(self.error(
                    format!(
                        "The string \"{}\" could not be parsed as an enum. Did you forget to pass the \"Yaml::PARSE_CONSTANT\" flag to the parser?",
                        lossy(scalar)
                    ),
                    Some(scalar),
                ));
            }
            return Ok(Value::Null);
        }
        if let Some(s) = scalar.strip_prefix(b"!!float ") {
            return Ok(Value::Float(string_to_float(s)));
        }
        if let Some(s) = scalar.strip_prefix(b"!!binary ") {
            return Ok(Value::String(self.evaluate_binary_scalar(s)?));
        }
        // Upstream reports this one without the "+ 1".
        Err(ParseException::new(
            format!(
                "The string \"{}\" could not be parsed as it uses an unsupported built-in tag.",
                lossy(scalar)
            ),
            self.parsed_line_number,
            Some(scalar.into()),
            self.parsed_filename.clone(),
        ))
    }

    /// `parseAnchor()`.
    fn parse_anchor(value: &[u8], i: &mut usize) -> Option<Key> {
        if value.get(*i) != Some(&b'&') {
            return None;
        }
        let c = ANCHOR.captures(&value[*i..])?;
        let whole = c.get(0)?;
        let name = c.get(1)?.as_bytes().to_vec();
        *i += whole.len();
        Some(Key::from_bytes(&name))
    }

    /// `parseTag()`.
    fn parse_tag(
        &self,
        value: &[u8],
        i: &mut usize,
        flags: Flags,
    ) -> Result<Option<String>, ParseException> {
        if value.get(*i) != Some(&b'!') {
            return Ok(None);
        }
        let start = *i + 1;
        let tag_length = value[start..]
            .iter()
            .take_while(|b| !b" \t\n[]{},".contains(b))
            .count();
        let tag = lossy(&value[start..start + tag_length]).into_owned();

        let mut next_offset = *i + tag_length + 1;
        next_offset += value[next_offset..]
            .iter()
            .take_while(|&&b| b == b' ')
            .count();

        if tag.is_empty() && matches!(value.get(next_offset), None | Some(b']' | b'}' | b',')) {
            return Err(self.error(
                "Using the unquoted scalar value \"!\" is not supported. You must quote it.",
                Some(value),
            ));
        }

        // Is followed by a scalar and is a built-in tag
        if !tag.is_empty()
            && !matches!(value.get(next_offset), Some(b'[' | b'{'))
            && (tag.starts_with('!')
                || matches!(
                    tag.as_str(),
                    "str" | "php/const" | "php/enum" | "php/object"
                ))
        {
            // Manage in {@link self::evaluateScalar()}
            return Ok(None);
        }

        *i = next_offset;

        // Built-in tags
        if tag.starts_with('!') {
            return Err(self.error(
                format!("The built-in tag \"!{tag}\" is not implemented."),
                Some(value),
            ));
        }

        if tag.is_empty() || flags.contains(Flags::PARSE_CUSTOM_TAGS) {
            return Ok(Some(tag));
        }

        Err(self.error(
            format!("Tags support is not enabled. Enable the \"Yaml::PARSE_CUSTOM_TAGS\" flag to use \"!{tag}\"."),
            Some(value),
        ))
    }

    /// `evaluateBinaryScalar()`.
    pub fn evaluate_binary_scalar(&mut self, scalar: &[u8]) -> Result<PhpString, ParseException> {
        let parsed: Vec<u8> = scalar
            .iter()
            .copied()
            .filter(|b| !matches!(b, b' ' | b'\t' | b'\n' | b'\r' | 0x0b | 0x0c))
            .collect();
        if parsed.is_empty() {
            return Ok(PhpString::default());
        }
        let parsed = self.parse_scalar_simple(&parsed, Flags::NONE, true)?;
        if parsed
            .as_string()
            .is_some_and(php_value::PhpString::is_empty)
        {
            return Ok(PhpString::default());
        }
        let Some(parsed) = scalar_string(&parsed) else {
            return Err(self.error(
                format!(
                    "The \"!!binary\" tag only supports a base64 encoded string, got \"{}\".",
                    parsed.debug_type()
                ),
                Some(scalar),
            ));
        };
        if parsed.len() % 4 != 0 {
            return Err(self.error(
                format!(
                    "The normalized base64 encoded data (data without whitespace characters) length must be a multiple of four ({} bytes given).",
                    parsed.len()
                ),
                Some(scalar),
            ));
        }
        if !BASE64.is_match(&parsed) {
            return Err(self.error(
                format!("The base64 encoded data ({parsed}) contains invalid characters."),
                Some(scalar),
            ));
        }
        php_value::base64::decode_strict(&parsed)
            .map(PhpString::from)
            .ok_or_else(|| {
                self.error(
                    "The base64 encoded data could not be decoded.",
                    Some(scalar),
                )
            })
    }

    /// `Inline::dump()`: a PHP value as inline YAML.
    pub fn dump(
        &self,
        value: &Value,
        flags: Flags,
        root_level: bool,
    ) -> Result<PhpString, DumpException> {
        let out: Vec<u8> = match value {
            Value::Object(object) => return self.dump_object(object, flags),
            Value::Array(array) => return self.dump_array(array, flags),
            Value::Null => return Ok(dump_null(flags, root_level)),
            Value::Bool(true) => b"true".to_vec(),
            Value::Bool(false) => b"false".to_vec(),
            Value::Int(i) => i.to_string().into_bytes(),
            Value::Float(f) if f.is_nan() => b".NaN".to_vec(),
            Value::Float(f) => dump_float(*f).into_bytes(),
            Value::String(s) => return Ok(dump_string(s, flags)),
        };
        Ok(PhpString::from(out))
    }

    fn dump_object(
        &self,
        object: &php_value::Object,
        flags: Flags,
    ) -> Result<PhpString, DumpException> {
        if let Some(time) = object.downcast_ref::<DateTimeImmutable>() {
            let fraction = time.format("u");
            let length = fraction.trim_end_matches('0').len();
            let format = match length {
                0 => "c",
                1..=3 => "Y-m-d\\TH:i:s.vP",
                _ => "Y-m-d\\TH:i:s.uP",
            };
            return Ok(PhpString::from(time.format(format)));
        }
        if let Some(case) = object.downcast_ref::<EnumCase>() {
            return Ok(PhpString::from(format!(
                "!php/enum {}::{}",
                case.class(),
                case.name()
            )));
        }
        if let Some(tagged) = object.downcast_ref::<TaggedValue>() {
            let mut out = format!("!{} ", tagged.get_tag()).into_bytes();
            out.extend_from_slice(&self.dump(tagged.get_value(), flags, false)?);
            return Ok(PhpString::from(out));
        }
        if flags.contains(Flags::DUMP_OBJECT) {
            let serialized = serialize(&Value::Object(object.clone()));
            let mut out = b"!php/object ".to_vec();
            out.extend_from_slice(&self.dump(&Value::String(serialized), Flags::NONE, false)?);
            return Ok(PhpString::from(out));
        }
        if flags.contains(Flags::DUMP_OBJECT_AS_MAP)
            && (object.downcast_ref::<StdClass>().is_some()
                || object.downcast_ref::<ArrayObject>().is_some())
        {
            return self.dump_hash_array(&object.properties(), flags);
        }
        if flags.contains(Flags::DUMP_EXCEPTION_ON_INVALID_TYPE) {
            return Err(DumpException::new(
                "Object support when dumping a YAML file has been disabled.",
            ));
        }
        Ok(dump_null(flags, false))
    }

    fn dump_array(&self, value: &PhpArray, flags: Flags) -> Result<PhpString, DumpException> {
        // array
        if (!value.is_empty() || flags.contains(Flags::DUMP_EMPTY_ARRAY_AS_SEQUENCE))
            && value.is_list()
        {
            let mut output = Vec::new();
            for val in value.values() {
                output.push(self.dump(val, flags, false)?);
            }
            return Ok(PhpString::from(format!("[{}]", join(&output, ", "))));
        }
        self.dump_hash_array(value, flags)
    }

    fn dump_hash_array(&self, value: &PhpArray, flags: Flags) -> Result<PhpString, DumpException> {
        let mut output = Vec::new();
        let key_flags = Flags(flags.0 & !Flags::DUMP_FORCE_DOUBLE_QUOTES_ON_VALUES.0);
        for (key, val) in value.iter() {
            let key = match key {
                Key::Int(k) if flags.contains(Flags::DUMP_NUMERIC_KEY_AS_STRING) => {
                    Value::from(k.to_string())
                }
                k => k.to_value(),
            };
            let mut entry = self.dump(&key, key_flags, false)?.as_bytes().to_vec();
            entry.extend_from_slice(b": ");
            entry.extend_from_slice(&self.dump(val, flags, false)?);
            output.push(PhpString::from(entry));
        }
        if output.is_empty() {
            return Ok(PhpString::from("{}"));
        }
        Ok(PhpString::from(format!("{{ {} }}", join(&output, ", "))))
    }
}

/// `Inline::isHash()`: true for `stdClass` and `ArrayObject`, and for arrays
/// that are not a list.
pub fn is_hash(value: &Value) -> bool {
    match value {
        Value::Array(array) => !array.is_list(),
        Value::Object(o) => {
            o.downcast_ref::<StdClass>().is_some() || o.downcast_ref::<ArrayObject>().is_some()
        }
        _ => false,
    }
}

fn join(parts: &[PhpString], separator: &str) -> String {
    parts
        .iter()
        .map(|p| p.to_string_lossy().into_owned())
        .collect::<Vec<_>>()
        .join(separator)
}

fn tagged(tag: &str, value: Value) -> Value {
    Value::Object(Arc::new(TaggedValue::new(tag, value)))
}

fn push(output: &mut PhpArray, value: Value) -> Result<(), ParseException> {
    output
        .push(value)
        .map(|_| ())
        .map_err(|e| ParseException::message_only(e.to_string()))
}

fn contains(haystack: &[u8], needle: &[u8]) -> bool {
    haystack.windows(needle.len()).any(|w| w == needle)
}

fn is_std_class(value: &Value) -> bool {
    value
        .as_object()
        .is_some_and(|o| o.downcast_ref::<StdClass>().is_some())
}

/// `.=` of a parsed value onto a string: `(string)`, arrays as `"Array"`.
fn concat_string(value: &Value) -> Vec<u8> {
    match value {
        Value::Array(_) => b"Array".to_vec(),
        Value::Object(o) => o.class().as_bytes().to_vec(),
        other => other
            .to_php_string()
            .unwrap_or_default()
            .as_bytes()
            .to_vec(),
    }
}

fn number(value: Number) -> Value {
    match value {
        Number::Int(i) => Value::Int(i),
        Number::Float(f) => Value::Float(f),
    }
}

/// `-$value` for an `octdec()` result.
#[expect(
    clippy::cast_precision_loss,
    reason = "PHP's int and float conversions, offsets and lengths, as upstream"
)]
fn negate(value: Value) -> Value {
    match value {
        Value::Int(i) => i
            .checked_neg()
            .map_or(Value::Float(-(i as f64)), Value::Int),
        Value::Float(f) => Value::Float(-f),
        other => other,
    }
}

/// `preg_replace('/\s*#.*$/A', '', $rest)`: removes a comment that runs to
/// the end; anything else is left.
fn strip_trailing_comment(rest: &[u8]) -> &[u8] {
    let spaces = rest
        .iter()
        .take_while(|b| matches!(b, b' ' | b'\t' | b'\n' | b'\r' | 0x0b | 0x0c))
        .count();
    if rest.get(spaces) == Some(&b'#') {
        let comment = &rest[spaces..];
        let end = comment
            .iter()
            .position(|&b| b == b'\n')
            .unwrap_or(comment.len());
        // `.*$`: to the end, or to a final line feed.
        if end == comment.len() || end + 1 == comment.len() {
            return &comment[end..];
        }
    }
    rest
}

/// `/^(.*?)(d1|d2|...)/` without `/s`: the first position where a
/// delimiter matches, with no line feed before it.
fn find_delimiter(s: &[u8], delimiters: &[Delimiter]) -> Option<usize> {
    for at in 0..s.len() {
        if delimiters.iter().any(|d| d.matches(s, at)) {
            return Some(at);
        }
        if s[at] == b'\n' {
            return None;
        }
    }
    None
}

fn dump_null(flags: Flags, root_level: bool) -> PhpString {
    if flags.contains(Flags::DUMP_NULL_AS_TILDE) {
        return PhpString::from("~");
    }
    if flags.contains(Flags::DUMP_NULL_AS_EMPTY) && !root_level {
        return PhpString::default();
    }
    PhpString::from("null")
}

/// The float branch of `dump()`, under the C locale.
#[expect(
    clippy::float_cmp,
    reason = "PHP's int and float conversions, offsets and lengths, as upstream"
)]
fn dump_float(value: f64) -> String {
    let repr = float_to_string(value);
    if value.is_infinite() {
        return repr.replace("INF", ".Inf");
    }
    // floor($value) == $value && $repr == $value
    if value.floor() == value && string_to_float(repr.as_bytes()) == value && !repr.contains('E') {
        // Preserve float data type since storing a whole number will result in integer value.
        return format!("{repr}.0");
    }
    repr
}

fn is_binary_string(value: &[u8]) -> bool {
    std::str::from_utf8(value).is_err()
        || value
            .iter()
            .any(|&b| matches!(b, 0x01..=0x06 | 0x0e..=0x1a | 0x1c..=0x1f))
}

/// The string branches of `dump()`.
fn dump_string(value: &PhpString, flags: Flags) -> PhpString {
    let bytes = value.as_bytes();
    if is_numeric(bytes)
        && !bytes
            .iter()
            .any(|b| matches!(b, 0x0c | b'\n' | b'\r' | b'\t' | 0x0b))
    {
        let mut out = b"'".to_vec();
        out.extend_from_slice(bytes);
        out.push(b'\'');
        return PhpString::from(out);
    }
    if bytes.is_empty() {
        return PhpString::from("''");
    }
    if is_binary_string(bytes) {
        return PhpString::from(format!("!!binary {}", php_value::base64::encode(bytes)));
    }
    if requires_double_quoting(bytes) || flags.contains(Flags::DUMP_FORCE_DOUBLE_QUOTES_ON_VALUES) {
        return PhpString::from(escape_with_double_quotes(bytes));
    }
    if requires_single_quoting(bytes) {
        let single_quoted = escape_with_single_quotes(bytes);
        if !bytes.contains(&b'\'') {
            return PhpString::from(single_quoted);
        }
        // Attempt double-quoting the string instead to see if it's more efficient.
        let double_quoted = escape_with_double_quotes(bytes);
        return PhpString::from(if double_quoted.len() < single_quoted.len() {
            double_quoted
        } else {
            single_quoted
        });
    }
    if DUMP_NUMBER.is_match(bytes)
        || DUMP_OCTAL.is_match(bytes)
        || HEX.is_match(bytes)
        || timestamp::is_timestamp(bytes)
    {
        return PhpString::from(escape_with_single_quotes(bytes));
    }
    value.clone()
}
