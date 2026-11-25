//! The fields of a `multipart/form-data` body, as PHP fills `$_POST` (and
//! `request_parse_body()`, which Symfony uses for PUT, DELETE, PATCH and
//! QUERY). The browser's live search script sends its data this way.
//!
//! Written from PHP 8.4.27's recorded answers to 137 bodies, each sent as
//! POST and as PUT (`tests/data/multipart_vectors.json`, recorded by
//! `scripts/record.sh multipart`), and the PHP manual's ini defaults; no
//! PHP source code is used (PLAN.md decision 8). Text fields and file parts
//! are kept; settings are PHP's defaults:
//! `max_input_vars` 1000, `max_file_uploads` 20, `file_uploads` on.
//!
//! What the recordings show, and this parser does:
//!
//! - The boundary is what follows the first `boundary` (any case) in the
//!   content type, after one more character (normally `=`): a quoted
//!   string, or the text up to `,` or `;`. No `boundary`, or an unclosed
//!   quote, fails with no fields.
//! - Before the first part, and whenever a delimiter line has more text
//!   after the boundary, lines are skipped until one is exactly
//!   `--boundary` (a final CR aside). So `--boundary--` does not end the
//!   body, and `--boundary ` does not start a part.
//! - Header lines end at LF (a final CR aside) and the headers at an empty
//!   line; a line starting with a space or tab continues the previous
//!   header, and a line without `:` is ignored. Only the first
//!   `Content-Disposition` counts; a part without one is skipped.
//! - A value ends at the first LF followed by `--boundary`, without that
//!   LF and one CR before it; at the end of the body, without one final LF
//!   (and one CR before it). The value may start with `--boundary`.
//! - A disposition with neither `name` nor `filename` fails and stops
//!   parsing; so does the 1,001st text field. A part with `filename` is a
//!   file and is not a field; a file part after 20 non-empty filenames
//!   fails but parsing continues.
//!
//! File parts are kept in [`Parsed::files`], in order, with what PHP's
//! `$_FILES` is built from (`crate::sapi`): the `name` and `filename`
//! parameters, the first `Content-Type` header, the data, whether the part
//! ended at a delimiter, and the `MAX_FILE_SIZE` field sent before it.
//! A file part read once `max_file_uploads` non-empty filenames were read
//! makes PHP warn and read no more files (recorded: `tests/data/sapi.json`,
//! `post-file-20-then-empty` and `post-file-19-empty-2`).
//!
//! PHP reads the body through a fixed-size buffer. None of the recorded
//! bodies placed around its size (`pad-*`, `long-*`) shows an effect of
//! it, so this parser reads the whole body at once.
//!
//! Upstream: none

use crate::query::MAX_INPUT_VARS;

/// `max_file_uploads`.
const MAX_FILE_UPLOADS: usize = 20;

/// What the parser found.
#[derive(Debug, Default, PartialEq, Eq)]
pub struct Parsed {
    /// Text fields in order, name and value as sent. Names are registered
    /// as PHP registers them (`Query::from_fields`).
    pub fields: Vec<(Vec<u8>, Vec<u8>)>,
    /// File parts in order, up to the one `max_file_uploads` stopped.
    pub files: Vec<FilePart>,
    /// PHP raised one of its warnings. For POST they are only warnings;
    /// `request_parse_body()` turns them into an exception, and Symfony
    /// then uses no fields at all.
    pub failed: bool,
    /// The warnings, in order.
    pub warnings: Vec<Warning>,
}

/// A part with a `filename` parameter.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct FilePart {
    /// The `name` parameter; `None` when the part has none.
    pub name: Option<Vec<u8>>,
    pub filename: Vec<u8>,
    /// The first `Content-Type` header's value, spaces and tabs before it
    /// dropped; `None` without one.
    pub content_type: Option<Vec<u8>>,
    pub data: Vec<u8>,
    /// The data ended at a delimiter rather than at the end of the body.
    pub complete: bool,
    /// The last `MAX_FILE_SIZE` field read before this part.
    pub max_file_size: Option<Vec<u8>>,
}

/// One of PHP's warnings while reading a multipart body; [`Warning::text`]
/// is what PHP says.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Warning {
    /// The content type names no boundary, or an unclosed quoted one.
    MissingBoundary,
    /// A part's disposition has neither `name` nor `filename`.
    Garbled,
    /// The `max_input_vars + 1`st field.
    InputVars,
    /// A file part after `max_file_uploads` non-empty filenames.
    MaxFileUploads,
}

impl Warning {
    pub fn text(self) -> &'static str {
        match self {
            Self::MissingBoundary => "Missing boundary in multipart/form-data POST data",
            Self::Garbled => "File Upload Mime headers garbled",
            Self::InputVars => {
                "Input variables exceeded 1000. To increase the limit change max_input_vars in php.ini."
            }
            Self::MaxFileUploads => "Maximum number of allowable file uploads has been exceeded",
        }
    }
}

impl Parsed {
    fn fail(&mut self, warning: Warning) {
        self.failed = true;
        self.warnings.push(warning);
    }
}

pub fn parse(content_type: &[u8], body: &[u8]) -> Parsed {
    let mut parsed = Parsed::default();
    let Some(boundary) = boundary(content_type) else {
        parsed.fail(Warning::MissingBoundary);
        return parsed;
    };
    let delimiter = [b"--", boundary].concat();
    let mut body = Body { bytes: body, at: 0 };
    if !body.seek(&delimiter) {
        return parsed;
    }
    let mut files = 0;
    let mut files_stopped = false;
    let mut max_file_size = None;
    loop {
        let (disposition, part_type) = body.headers();
        let (value, next) = body.value(&delimiter);
        if let Some(disposition) = disposition {
            let params = Params::of(&disposition);
            match (params.name, params.filename) {
                (None, None) => {
                    parsed.fail(Warning::Garbled);
                    return parsed;
                }
                (name, Some(filename)) if !files_stopped => {
                    if files == MAX_FILE_UPLOADS {
                        files_stopped = true;
                        parsed.fail(Warning::MaxFileUploads);
                    } else {
                        if !filename.is_empty() {
                            files += 1;
                        }
                        parsed.files.push(FilePart {
                            name,
                            filename,
                            content_type: part_type,
                            data: value.to_vec(),
                            complete: !matches!(next, Next::End),
                            max_file_size: max_file_size.clone(),
                        });
                    }
                }
                (_, Some(_)) => {}
                (Some(name), None) => {
                    if parsed.fields.len() == MAX_INPUT_VARS {
                        parsed.fail(Warning::InputVars);
                        return parsed;
                    }
                    if name == b"MAX_FILE_SIZE" {
                        max_file_size = Some(value.to_vec());
                    }
                    parsed.fields.push((name, value.to_vec()));
                }
            }
        }
        match next {
            Next::Headers => {}
            Next::Seek if body.seek(&delimiter) => {}
            Next::Seek | Next::End => return parsed,
        }
    }
}

/// The boundary named by the content type, `None` when there is none.
fn boundary(content_type: &[u8]) -> Option<&[u8]> {
    const KEY: &[u8] = b"boundary";
    let at = content_type
        .windows(KEY.len())
        .position(|window| window.eq_ignore_ascii_case(KEY))?;
    let value = content_type.get(at + KEY.len() + 1..).unwrap_or_default();
    match value.strip_prefix(b"\"") {
        Some(quoted) => quoted
            .iter()
            .position(|&b| b == b'"')
            .map(|end| &quoted[..end]),
        None => Some(until(value, |b| b == b',' || b == b';')),
    }
}

fn until(bytes: &[u8], stop: impl Fn(u8) -> bool) -> &[u8] {
    &bytes[..bytes.iter().position(|&b| stop(b)).unwrap_or(bytes.len())]
}

/// Where parsing goes after a value.
enum Next {
    /// The delimiter line held only the boundary: a part's headers follow.
    Headers,
    /// The delimiter line had more text: look for the next delimiter line.
    Seek,
    /// The body ended.
    End,
}

/// The body and the position reached in it.
struct Body<'a> {
    bytes: &'a [u8],
    at: usize,
}

impl<'a> Body<'a> {
    /// The next line without its LF and one CR before it; `None` at the
    /// end of the body.
    fn line(&mut self) -> Option<&'a [u8]> {
        let rest = self.bytes.get(self.at..).filter(|rest| !rest.is_empty())?;
        let (line, used) = match rest.iter().position(|&b| b == b'\n') {
            Some(lf) => (&rest[..lf], lf + 1),
            None => (rest, rest.len()),
        };
        self.at += used;
        Some(line.strip_suffix(b"\r").unwrap_or(line))
    }

    /// Skips lines up to and including one that is exactly `delimiter`.
    fn seek(&mut self, delimiter: &[u8]) -> bool {
        while let Some(line) = self.line() {
            if line == delimiter {
                return true;
            }
        }
        false
    }

    /// Reads a part's headers and returns its first `Content-Disposition`
    /// and its first `Content-Type`.
    fn headers(&mut self) -> (Option<Vec<u8>>, Option<Vec<u8>>) {
        let mut headers: Vec<(&[u8], Vec<u8>)> = Vec::new();
        while let Some(line) = self.line() {
            if line.is_empty() {
                break;
            }
            if matches!(line[0], b' ' | b'\t') {
                if let Some((_, value)) = headers.last_mut() {
                    value.extend_from_slice(line);
                }
            } else if let Some(colon) = line.iter().position(|&b| b == b':') {
                let value = &line[colon + 1..];
                let start = value.iter().position(|&b| b != b' ' && b != b'\t');
                headers.push((
                    &line[..colon],
                    start.map_or(Vec::new(), |s| value[s..].to_vec()),
                ));
            }
        }
        let first = |wanted: &[u8]| {
            headers
                .iter()
                .find(|(name, _)| name.eq_ignore_ascii_case(wanted))
                .map(|(_, value)| value.clone())
        };
        (first(b"content-disposition"), first(b"content-type"))
    }

    /// Reads a part's value and finds what follows it.
    fn value(&mut self, delimiter: &[u8]) -> (&'a [u8], Next) {
        let rest = self.bytes.get(self.at..).unwrap_or_default();
        let found = rest
            .windows(delimiter.len() + 1)
            .position(|window| window[0] == b'\n' && &window[1..] == delimiter);
        let Some(lf) = found else {
            self.at = self.bytes.len();
            let value = rest
                .strip_suffix(b"\n")
                .map_or(rest, |value| value.strip_suffix(b"\r").unwrap_or(value));
            return (value, Next::End);
        };
        let value = &rest[..lf];
        self.at += lf + 1 + delimiter.len();
        let after = self.line().unwrap_or_default();
        let next = if after.is_empty() {
            Next::Headers
        } else {
            Next::Seek
        };
        (value.strip_suffix(b"\r").unwrap_or(value), next)
    }
}

/// The `name` and `filename` parameters of a `Content-Disposition` value.
/// The last of each counts. Parameters are separated by `;`; spaces and
/// tabs before a parameter are skipped; its key runs to `=` (keys compare
/// in any case, and `name *` or `name ` is another key); any `=` after the
/// first are skipped. A value quoted with `"` or `'` runs to the same
/// quote, may be unclosed, and is followed by text ignored up to the next
/// `;`; an unquoted value ends at a space, tab or `;`. In both, a
/// backslash before the quote character or a backslash stands for that
/// character; other backslashes stay.
#[derive(Default)]
struct Params {
    name: Option<Vec<u8>>,
    filename: Option<Vec<u8>>,
}

impl Params {
    fn of(disposition: &[u8]) -> Self {
        let mut params = Self::default();
        let mut rest = disposition;
        loop {
            let start = rest.iter().position(|&b| b != b' ' && b != b'\t');
            let Some(start) = start else {
                return params;
            };
            rest = &rest[start..];
            let key_end = rest.iter().position(|&b| b == b'=' || b == b';');
            let Some(key_end) = key_end.filter(|&end| rest[end] == b'=') else {
                match key_end {
                    Some(semicolon) => {
                        rest = &rest[semicolon + 1..];
                        continue;
                    }
                    None => return params,
                }
            };
            let key = &rest[..key_end];
            let after_equals = rest[key_end..].iter().position(|&b| b != b'=');
            let value_start = after_equals.map_or(rest.len(), |p| key_end + p);
            let (value, used) = value_of(&rest[value_start..]);
            if key.eq_ignore_ascii_case(b"name") {
                params.name = Some(value);
            } else if key.eq_ignore_ascii_case(b"filename") {
                params.filename = Some(value);
            }
            let rest_of_param = &rest[value_start + used..];
            match rest_of_param.iter().position(|&b| b == b';') {
                Some(semicolon) => rest = &rest_of_param[semicolon + 1..],
                None => return params,
            }
        }
    }
}

/// A parameter value and the number of bytes it used.
fn value_of(text: &[u8]) -> (Vec<u8>, usize) {
    let quote = match text.first() {
        Some(&q @ (b'"' | b'\'')) => Some(q),
        _ => None,
    };
    let start = usize::from(quote.is_some());
    let escapable = quote.unwrap_or(b'"');
    let mut value = Vec::new();
    let mut at = start;
    while let Some(&byte) = text.get(at) {
        match quote {
            Some(q) if byte == q => return (value, at + 1),
            None if matches!(byte, b' ' | b'\t' | b';') => return (value, at),
            _ => {}
        }
        match (byte, text.get(at + 1)) {
            (b'\\', Some(&next)) if next == escapable || next == b'\\' => {
                value.push(next);
                at += 2;
            }
            _ => {
                value.push(byte);
                at += 1;
            }
        }
    }
    (value, at)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fmt::Write as _;

    fn field(name: &str, value: &str) -> (Vec<u8>, Vec<u8>) {
        (name.as_bytes().to_vec(), value.as_bytes().to_vec())
    }

    const TYPE: &[u8] = b"multipart/form-data; boundary=XyZ";

    /// The size of PHP's read buffer that the earlier implementation
    /// modelled; kept so the long-value test still crosses it.
    const FILLUNIT: usize = 5 * 1024;

    #[test]
    fn reads_fields_as_browsers_send_them() {
        let body = b"--XyZ\r\nContent-Disposition: form-data; name=\"a\"\r\n\r\none\r\n\
                     --XyZ\r\nContent-Disposition: form-data; name=\"b[]\"\r\n\r\ntwo\r\nlines\r\n\
                     --XyZ--\r\n";
        let parsed = parse(TYPE, body);
        assert_eq!(
            parsed.fields,
            [field("a", "one"), field("b[]", "two\r\nlines")]
        );
        assert!(!parsed.failed);
    }

    #[test]
    fn accepts_lf_only_lines_a_preamble_and_no_closing_boundary() {
        let body = b"preamble\n--XyZ\nContent-Disposition: form-data; name=a\n\nvalue";
        assert_eq!(parse(TYPE, body).fields, [field("a", "value")]);
    }

    #[test]
    fn finds_the_boundary_as_php_does() {
        let body = b"--b\r\nContent-Disposition: form-data; name=a\r\n\r\nv\r\n--b--";
        for content_type in [
            &b"multipart/form-data; boundary=b"[..],
            b"multipart/form-data; BOUNDARY=b",
            b"multipart/form-data; boundary=\"b\"; charset=x",
            b"multipart/form-data; boundary=b, x",
        ] {
            assert_eq!(parse(content_type, body).fields, [field("a", "v")]);
        }
        let missing = parse(b"multipart/form-data", body);
        assert!(missing.fields.is_empty() && missing.failed);
        let unclosed = parse(b"multipart/form-data; boundary=\"b", body);
        assert!(unclosed.fields.is_empty() && unclosed.failed);
    }

    #[test]
    fn skips_file_parts_but_not_text_fields_of_the_same_name() {
        let body = b"--XyZ\r\nContent-Disposition: form-data; name=\"f\"; filename=\"x.txt\"\r\n\
                     Content-Type: text/plain\r\n\r\ndata\r\n\
                     --XyZ\r\nContent-Disposition: form-data; name=\"f\"\r\n\r\nshadow\r\n\
                     --XyZ\r\nContent-Disposition: form-data; name=\"g\"\r\n\r\nkept\r\n--XyZ--";
        assert_eq!(
            parse(TYPE, body).fields,
            [field("f", "shadow"), field("g", "kept")]
        );
    }

    #[test]
    fn unescapes_quoted_names_and_continues_headers() {
        let body =
            b"--XyZ\r\nContent-Disposition: form-data;\r\n name=\"a\\\"b\"\r\n\r\nv\r\n--XyZ--";
        assert_eq!(parse(TYPE, body).fields, [field("a\"b", "v")]);
    }

    #[test]
    fn a_part_without_name_or_filename_stops_parsing() {
        let body = b"--XyZ\r\nContent-Disposition: form-data; name=a\r\n\r\n1\r\n\
                     --XyZ\r\nContent-Disposition: form-data\r\n\r\nx\r\n\
                     --XyZ\r\nContent-Disposition: form-data; name=b\r\n\r\n2\r\n--XyZ--";
        let parsed = parse(TYPE, body);
        assert_eq!(parsed.fields, [field("a", "1")]);
        assert!(parsed.failed);
    }

    #[test]
    fn keeps_long_values_across_buffer_fills() {
        let value = "x".repeat(3 * FILLUNIT + 7);
        let body =
            format!("--XyZ\r\nContent-Disposition: form-data; name=a\r\n\r\n{value}\r\n--XyZ--");
        assert_eq!(parse(TYPE, body.as_bytes()).fields, [field("a", &value)]);
    }

    #[test]
    fn stops_counting_at_max_input_vars() {
        let mut body = String::new();
        for i in 0..=MAX_INPUT_VARS {
            let _ = write!(
                body,
                "--XyZ\r\nContent-Disposition: form-data; name=v{i}\r\n\r\n{i}\r\n"
            );
        }
        let parsed = parse(TYPE, body.as_bytes());
        assert_eq!(parsed.fields.len(), MAX_INPUT_VARS);
        assert!(parsed.failed);
    }

    /// A parsed value: text, or an array of `(key, value)` in order.
    #[derive(Debug, PartialEq)]
    enum Tree {
        Text(String),
        Array(Vec<(String, Tree)>),
    }

    fn from_php(value: &serde_json::Value) -> Result<Tree, String> {
        use base64::Engine as _;
        match value {
            serde_json::Value::String(text) => {
                let bytes = base64::engine::general_purpose::STANDARD
                    .decode(text)
                    .map_err(|error| error.to_string())?;
                Ok(Tree::Text(String::from_utf8_lossy(&bytes).into_owned()))
            }
            serde_json::Value::Array(pairs) => pairs
                .iter()
                .map(|pair| match pair.as_array().map(Vec::as_slice) {
                    Some([serde_json::Value::String(key), value]) => {
                        Ok((key.clone(), from_php(value)?))
                    }
                    _ => Err(format!("not a [key, value] pair: {pair}")),
                })
                .collect::<Result<_, _>>()
                .map(Tree::Array),
            other => Err(format!("unexpected value {other}")),
        }
    }

    fn from_query(entries: &[(String, crate::query::QueryValue)]) -> Tree {
        use crate::query::QueryValue;
        Tree::Array(
            entries
                .iter()
                .map(|(key, value)| {
                    let value = match value {
                        QueryValue::Str(text) => Tree::Text(text.clone()),
                        QueryValue::Map(children) => from_query(children),
                    };
                    (key.clone(), value)
                })
                .collect(),
        )
    }

    /// Every body of `tests/data/multipart_vectors.json`, recorded from
    /// PHP 8.4.27 itself by `scripts/record.sh multipart`: `$_POST` for
    /// POST, `request_parse_body()` for PUT (empty when it throws). The
    /// content type is trimmed of spaces and tabs first, as the HTTP
    /// server in front of PHP trims header values.
    #[test]
    fn reads_every_recorded_body_as_php_does() -> Result<(), Box<dyn std::error::Error>> {
        use base64::Engine as _;
        let vectors: Vec<serde_json::Value> =
            serde_json::from_str(include_str!("../tests/data/multipart_vectors.json"))?;
        assert!(vectors.len() > 250);
        let mut failures = Vec::new();
        for vector in &vectors {
            let name = vector["name"].as_str().unwrap_or("?");
            let method = vector["method"].as_str().unwrap_or("?");
            let content_type = crate::trim_ows(
                vector["content_type"]
                    .as_str()
                    .unwrap_or_default()
                    .as_bytes(),
            );
            let body = base64::engine::general_purpose::STANDARD
                .decode(vector["body"].as_str().unwrap_or_default())?;
            // The kernel reads multipart bodies only (`BodyFormat::of`).
            let is_multipart =
                crate::BodyFormat::of(content_type) == Some(crate::BodyFormat::Multipart);
            let parsed = if is_multipart {
                parse(content_type, &body)
            } else {
                Parsed::default()
            };
            let fields = if method == "PUT" && parsed.failed {
                Vec::new()
            } else {
                parsed.fields
            };
            let got = from_query(crate::query::Query::from_fields(&fields).all());
            let expected = from_php(&vector["fields"])?;
            if is_multipart && method == "PUT" && vector["failed"].as_bool() != Some(parsed.failed)
            {
                failures.push(format!("{name} {method}: failed is {}", parsed.failed));
            }
            if got != expected {
                failures.push(format!(
                    "{name} {method}:\n  php  {expected:?}\n  rust {got:?}"
                ));
            }
        }
        assert!(
            failures.is_empty(),
            "{} of {} differ:\n{}",
            failures.len(),
            vectors.len(),
            failures.join("\n")
        );
        Ok(())
    }
}
