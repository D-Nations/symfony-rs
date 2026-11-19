//! The fields of a `multipart/form-data` body, as PHP fills `$_POST`.
//!
//! Ports `main/rfc1867.c` of PHP 8.4.27 (`rfc1867_post_handler()` and the
//! buffer, line, header and word functions it uses), the parser behind the
//! original's form fields. The browser's live search script sends its data
//! this way. Only text fields are kept; file parts are read and dropped as
//! PHP reads them, because how far PHP reads a file part decides where it
//! looks for the next boundary.
//!
//! PHP reads the body through a buffer of 5 KiB (`FILLUNIT`); lines longer
//! than the buffer, and boundaries split across two fills, are handled the
//! way that buffer handles them, so the buffer is ported too, over the
//! already-read body. Settings are PHP's defaults, as in the original:
//! `max_input_vars` 1000, `max_file_uploads` 20, `upload_max_filesize`
//! 2M, `max_multipart_body_parts` -1 (their sum), `file_uploads` on, no
//! encoding translation, the `unsafe_raw` input filter.

/// `FILLUNIT`: the size of PHP's read buffer.
const FILLUNIT: usize = 5 * 1024;
/// `max_input_vars`.
const MAX_INPUT_VARS: usize = 1000;
/// `max_file_uploads`.
const MAX_FILE_UPLOADS: i64 = 20;
/// `upload_max_filesize` (2M).
const UPLOAD_MAX_FILESIZE: i64 = 2 * 1024 * 1024;

/// What the parser found.
#[derive(Debug, Default, PartialEq, Eq)]
pub struct Parsed {
    /// Text fields in order, name and value as sent. Names are registered
    /// as PHP registers them (`Query::from_fields`).
    pub fields: Vec<(Vec<u8>, Vec<u8>)>,
    /// PHP raised one of its warnings (`EMIT_WARNING_OR_ERROR`). For POST
    /// they are only warnings; `request_parse_body()`, which Symfony uses
    /// for PUT, DELETE, PATCH and QUERY, turns them into an exception,
    /// and Symfony then uses no fields at all.
    pub failed: bool,
}

/// `isspace()` in the C locale.
const fn is_space(byte: u8) -> bool {
    matches!(byte, b' ' | b'\t' | b'\n' | 0x0B | 0x0C | b'\r')
}

/// A C string: the bytes before the first NUL.
fn c_str(bytes: &[u8]) -> &[u8] {
    bytes
        .iter()
        .position(|&b| b == 0)
        .map_or(bytes, |end| &bytes[..end])
}

/// `multipart_buffer`, reading from the body instead of the socket.
struct Buffer<'a> {
    input: &'a [u8],
    /// Bytes of `input` handed to the buffer so far.
    read: usize,
    buffer: Vec<u8>,
    bufsize: usize,
    begin: usize,
    in_buffer: usize,
    boundary: Vec<u8>,
    boundary_next: Vec<u8>,
}

impl<'a> Buffer<'a> {
    /// `multipart_buffer_new()`.
    fn new(input: &'a [u8], boundary: &[u8]) -> Self {
        let bufsize = (boundary.len() + 6).max(FILLUNIT);
        let mut dashed = b"--".to_vec();
        dashed.extend_from_slice(boundary);
        let mut next = b"\n--".to_vec();
        next.extend_from_slice(boundary);
        Self {
            input,
            read: 0,
            buffer: vec![0; bufsize + 1],
            bufsize,
            begin: 0,
            in_buffer: 0,
            boundary: dashed,
            boundary_next: next,
        }
    }

    /// `fill_buffer()`: moves what is left to the front and reads as much
    /// as fits. Returns the number of bytes read.
    fn fill(&mut self) -> usize {
        if self.in_buffer > 0 && self.begin != 0 {
            self.buffer
                .copy_within(self.begin..self.begin + self.in_buffer, 0);
        }
        self.begin = 0;
        let wanted = self.bufsize - self.in_buffer;
        let available = self.input.len() - self.read;
        let count = wanted.min(available);
        let start = self.in_buffer;
        self.buffer[start..start + count]
            .copy_from_slice(&self.input[self.read..self.read + count]);
        self.in_buffer += count;
        self.read += count;
        count
    }

    /// `multipart_buffer_eof()`.
    fn eof(&mut self) -> bool {
        self.in_buffer == 0 && self.fill() < 1
    }

    fn data(&self) -> &[u8] {
        &self.buffer[self.begin..self.begin + self.in_buffer]
    }

    /// `next_line()`: the next LF-terminated line without its CR LF, or
    /// the whole buffer when it is full and holds no LF; `None` when the
    /// buffer is not full and holds no LF.
    fn next_line(&mut self) -> Option<Vec<u8>> {
        match self.data().iter().position(|&b| b == b'\n') {
            Some(lf) => {
                let data = self.data();
                let end = if lf > 0 && data[lf - 1] == b'\r' {
                    lf - 1
                } else {
                    lf
                };
                let line = c_str(&data[..end]).to_vec();
                self.begin += lf + 1;
                self.in_buffer -= lf + 1;
                Some(line)
            }
            None if self.in_buffer < self.bufsize => None,
            None => {
                let line = c_str(&self.buffer[self.begin..self.begin + self.bufsize]).to_vec();
                self.in_buffer = 0;
                Some(line)
            }
        }
    }

    /// `get_line()`.
    fn get_line(&mut self) -> Option<Vec<u8>> {
        if let Some(line) = self.next_line() {
            return Some(line);
        }
        self.fill();
        self.next_line()
    }

    /// `find_boundary()`: skips lines up to one that is exactly the
    /// boundary.
    fn find_boundary(&mut self) -> bool {
        while let Some(line) = self.get_line() {
            if line == self.boundary {
                return true;
            }
        }
        false
    }

    /// `multipart_buffer_headers()`: the part's `Content-Disposition` and
    /// `Content-Type` headers (the first of each), or `None` when no
    /// boundary follows.
    fn headers(&mut self) -> Option<Vec<(Vec<u8>, Vec<u8>)>> {
        if !self.find_boundary() {
            return None;
        }
        let mut headers: Vec<(Vec<u8>, Vec<u8>)> = Vec::new();
        let mut key: Option<Vec<u8>> = None;
        let mut value: Option<Vec<u8>> = None;
        while let Some(line) = self.get_line() {
            if line.is_empty() {
                break;
            }
            // A line starting with a space continues the previous header.
            let colon = if is_space(line[0]) {
                None
            } else {
                line.iter().position(|&b| b == b':')
            };
            if let Some(colon) = colon {
                if let (Some(v), Some(k)) = (&value, &key) {
                    headers.push((k.clone(), v.clone()));
                    value = None;
                }
                let name = &line[..colon];
                let saved = name.eq_ignore_ascii_case(b"Content-Disposition")
                    || name.eq_ignore_ascii_case(b"Content-Type");
                if !saved || header_value(&headers, name).is_some() {
                    key = None;
                } else {
                    key = Some(name.to_vec());
                    // `do { value++; } while (isspace(*value))`
                    let rest = &line[colon + 1..];
                    let start = rest
                        .iter()
                        .position(|&b| !is_space(b))
                        .unwrap_or(rest.len());
                    value
                        .get_or_insert_with(Vec::new)
                        .extend_from_slice(&rest[start..]);
                }
            } else if let Some(v) = &mut value {
                v.extend_from_slice(&line);
            }
        }
        if let (Some(v), Some(k)) = (value, key) {
            headers.push((k, v));
        }
        Some(headers)
    }

    /// `multipart_buffer_read()`: up to `bytes - 1` bytes before the next
    /// `"\n--boundary"` (or a prefix of it at the end of the buffer). A CR
    /// just before the boundary is left out but stays in the buffer.
    /// `end` is set when the full boundary is in sight.
    fn read(&mut self, bytes: usize, end: &mut bool) -> Vec<u8> {
        if bytes > self.in_buffer {
            self.fill();
        }
        let bound = memstr(self.data(), &self.boundary_next, true);
        let max = match bound {
            Some(position) => {
                if memstr(self.data(), &self.boundary_next, false).is_some() {
                    *end = true;
                }
                position
            }
            None => self.in_buffer,
        };
        let mut len = max.min(bytes - 1);
        if len == 0 {
            return Vec::new();
        }
        let mut out = self.data()[..len].to_vec();
        if bound.is_some() && out.last() == Some(&b'\r') {
            out.pop();
            len -= 1;
        }
        self.in_buffer -= len;
        self.begin += len;
        out
    }

    /// `multipart_buffer_read_body()`.
    fn read_body(&mut self) -> Vec<u8> {
        let mut out = Vec::new();
        let mut end = false;
        loop {
            let chunk = self.read(FILLUNIT, &mut end);
            if chunk.is_empty() {
                return out;
            }
            out.extend_from_slice(&chunk);
        }
    }
}

/// `php_ap_memstr()`: the first place the needle starts, where a match
/// cut short by the end of the haystack counts if `partial`.
fn memstr(haystack: &[u8], needle: &[u8], partial: bool) -> Option<usize> {
    let first = *needle.first()?;
    let mut from = 0;
    while let Some(offset) = haystack[from..].iter().position(|&b| b == first) {
        let at = from + offset;
        let rest = haystack.len() - at;
        let compared = needle.len().min(rest);
        if haystack[at..at + compared] == needle[..compared] && (partial || rest >= needle.len()) {
            return Some(at);
        }
        from = at + 1;
    }
    None
}

/// `php_mime_get_hdr_value()`.
fn header_value<'h>(headers: &'h [(Vec<u8>, Vec<u8>)], name: &[u8]) -> Option<&'h [u8]> {
    headers
        .iter()
        .find(|(key, _)| key.eq_ignore_ascii_case(name))
        .map(|(_, value)| value.as_slice())
}

/// `php_ap_getword()`: the text up to `stop` (outside quotes); the line
/// moves past it and any repeated `stop`.
fn getword(line: &mut &[u8], stop: u8) -> Vec<u8> {
    let text = *line;
    let mut pos = 0;
    while pos < text.len() && text[pos] != stop {
        let quote = text[pos];
        if quote == b'"' || quote == b'\'' {
            pos += 1;
            while pos < text.len() && text[pos] != quote {
                if text[pos] == b'\\' && text.get(pos + 1) == Some(&quote) {
                    pos += 2;
                } else {
                    pos += 1;
                }
            }
            if pos < text.len() {
                pos += 1;
            }
        } else {
            pos += 1;
        }
    }
    if pos >= text.len() {
        *line = &text[text.len()..];
        return text.to_vec();
    }
    let word = text[..pos].to_vec();
    while pos < text.len() && text[pos] == stop {
        pos += 1;
    }
    *line = &text[pos..];
    word
}

/// `substring_conf()`: up to `len` bytes, stopping at `quote` (a NUL
/// `quote` never matches), with `\\` and `\<quote>` unescaped. Like the C
/// code, an escape may look one byte past `len`.
fn substring_conf(text: &[u8], len: usize, quote: Option<u8>) -> Vec<u8> {
    let mut out = Vec::with_capacity(len);
    let mut i = 0;
    while i < len && i < text.len() && Some(text[i]) != quote {
        let next = text.get(i + 1).copied();
        if text[i] == b'\\' && (next == Some(b'\\') || (quote.is_some() && next == quote)) {
            i += 1;
            out.push(text[i]);
        } else {
            out.push(text[i]);
        }
        i += 1;
    }
    out
}

/// `php_ap_getword_conf()`: a parameter value, quoted or up to the next
/// space.
fn getword_conf(text: &[u8]) -> Vec<u8> {
    let start = text
        .iter()
        .position(|&b| !is_space(b))
        .unwrap_or(text.len());
    let text = &text[start..];
    match text.first() {
        None => Vec::new(),
        Some(&quote) if quote == b'"' || quote == b'\'' => {
            substring_conf(&text[1..], text.len() - 1, Some(quote))
        }
        Some(_) => {
            let end = text.iter().position(|&b| is_space(b)).unwrap_or(text.len());
            substring_conf(text, end, None)
        }
    }
}

/// The boundary of a `Content-Type` header value, as the handler finds
/// it: after the first `boundary` (any case) and its `=`, quoted or up to
/// `,` or `;`. `Err(())` when PHP warns.
fn boundary(content_type: &[u8]) -> Result<Option<Vec<u8>>, ()> {
    let content_type = c_str(content_type);
    let lower = content_type.to_ascii_lowercase();
    let Some(found) =
        memstr(content_type, b"boundary", false).or_else(|| memstr(&lower, b"boundary", false))
    else {
        return Err(());
    };
    let Some(equals) = content_type[found..].iter().position(|&b| b == b'=') else {
        return Err(());
    };
    let value = &content_type[found + equals + 1..];
    let boundary = if value.first() == Some(&b'"') {
        let quoted = &value[1..];
        let Some(close) = quoted.iter().position(|&b| b == b'"') else {
            return Err(());
        };
        &quoted[..close]
    } else {
        let end = value
            .iter()
            .position(|&b| b == b',' || b == b';')
            .unwrap_or(value.len());
        &value[..end]
    };
    // Too large: PHP stops with a warning that is not an exception.
    if boundary.len() > FILLUNIT - 4 {
        return Ok(None);
    }
    Ok(Some(boundary.to_vec()))
}

/// `rfc1867_post_handler()`: the text fields of `body`, sent with the
/// `Content-Type` value `content_type`.
pub fn parse(content_type: &[u8], body: &[u8]) -> Parsed {
    let mut parsed = Parsed::default();
    let boundary = match boundary(content_type) {
        Ok(Some(boundary)) => boundary,
        Ok(None) => return parsed,
        Err(()) => {
            parsed.failed = true;
            return parsed;
        }
    };
    let mut buffer = Buffer::new(body, &boundary);
    let mut uploads_left = MAX_FILE_UPLOADS;
    let mut parts_left = i64::try_from(MAX_INPUT_VARS).unwrap_or(i64::MAX) + MAX_FILE_UPLOADS;
    let mut count = 0usize;
    let mut anonymous_index = 0u32;
    let mut max_file_size: i64 = 0;

    while !buffer.eof() {
        let Some(headers) = buffer.headers() else {
            break;
        };
        let Some(disposition) = header_value(&headers, b"Content-Disposition") else {
            continue;
        };
        parts_left -= 1;
        if parts_left < 0 {
            parsed.failed = true;
            break;
        }
        let start = disposition
            .iter()
            .position(|&b| !is_space(b))
            .unwrap_or(disposition.len());
        let mut cd = &disposition[start..];
        let mut param: Option<Vec<u8>> = None;
        let mut filename: Option<Vec<u8>> = None;
        while !cd.is_empty() {
            let pair = getword(&mut cd, b';');
            let skip = cd.iter().position(|&b| !is_space(b)).unwrap_or(cd.len());
            cd = &cd[skip..];
            if pair.contains(&b'=') {
                let mut rest = pair.as_slice();
                let key = getword(&mut rest, b'=');
                if key.eq_ignore_ascii_case(b"name") {
                    param = Some(getword_conf(rest));
                } else if key.eq_ignore_ascii_case(b"filename") {
                    filename = Some(getword_conf(rest));
                }
            }
        }

        // A normal form field.
        if let (None, Some(name)) = (&filename, &param) {
            let value = buffer.read_body();
            count += 1;
            if count <= MAX_INPUT_VARS {
                parsed.fields.push((name.clone(), value.clone()));
            } else if count == MAX_INPUT_VARS + 1 {
                parsed.failed = true;
            }
            if name.eq_ignore_ascii_case(b"MAX_FILE_SIZE") {
                max_file_size = strtoll(&value);
            }
            continue;
        }

        // A file part.
        let mut skip_upload = false;
        if uploads_left <= 0 {
            skip_upload = true;
            if uploads_left == 0 {
                uploads_left -= 1;
                parsed.failed = true;
            }
        }
        if param.is_none() && filename.is_none() {
            parsed.failed = true;
            break;
        }
        let param = param.unwrap_or_else(|| {
            let name = anonymous_index.to_string().into_bytes();
            anonymous_index += 1;
            name
        });
        if !skip_upload && !brackets_are_closed(&param) {
            skip_upload = true;
        }
        if skip_upload {
            continue;
        }
        let filename = filename.unwrap_or_default();
        // Read the file's data as PHP does, then drop it. A file over a
        // limit is cancelled, after one more read.
        if !filename.is_empty() {
            let mut end = false;
            let mut total: i64 = 0;
            let mut chunk = buffer.read(FILLUNIT, &mut end);
            uploads_left -= 1;
            let mut cancelled = false;
            while !cancelled && !chunk.is_empty() {
                let next_total =
                    total.saturating_add(i64::try_from(chunk.len()).unwrap_or(i64::MAX));
                if next_total > UPLOAD_MAX_FILESIZE
                    || (max_file_size != 0 && next_total > max_file_size)
                {
                    cancelled = true;
                } else {
                    total = next_total;
                }
                chunk = buffer.read(FILLUNIT, &mut end);
            }
        }
    }
    parsed
}

/// "Never repair potential malicious user input": every `]` closes a
/// `[` and is followed by `[` or the end.
fn brackets_are_closed(name: &[u8]) -> bool {
    let mut depth: i64 = 0;
    for (i, &byte) in name.iter().enumerate() {
        if byte == b'[' {
            depth += 1;
        } else if byte == b']' {
            depth -= 1;
            if matches!(name.get(i + 1), Some(&next) if next != b'[') {
                return false;
            }
        }
        if depth < 0 {
            return false;
        }
    }
    depth == 0
}

/// `strtoll(value, NULL, 10)`: leading white space and a sign, then
/// digits; anything else ends the number; out of range gives the nearest
/// limit, as C does.
fn strtoll(value: &[u8]) -> i64 {
    let value = c_str(value);
    let start = value
        .iter()
        .position(|&b| !is_space(b))
        .unwrap_or(value.len());
    let mut digits = &value[start..];
    let negative = digits.first() == Some(&b'-');
    if matches!(digits.first(), Some(b'+' | b'-')) {
        digits = &digits[1..];
    }
    let mut number: i64 = 0;
    for &byte in digits.iter().take_while(|b| b.is_ascii_digit()) {
        let digit = i64::from(byte - b'0');
        number = if negative {
            number.saturating_mul(10).saturating_sub(digit)
        } else {
            number.saturating_mul(10).saturating_add(digit)
        };
    }
    number
}

#[cfg(test)]
mod tests {
    use super::*;

    fn field(name: &str, value: &str) -> (Vec<u8>, Vec<u8>) {
        (name.as_bytes().to_vec(), value.as_bytes().to_vec())
    }

    const TYPE: &[u8] = b"multipart/form-data; boundary=XyZ";

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
            body.push_str(&format!(
                "--XyZ\r\nContent-Disposition: form-data; name=v{i}\r\n\r\n{i}\r\n"
            ));
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

    /// Every body of `data/request/multipart_vectors.json`, recorded from
    /// PHP 8.4.27 itself by `compare/multipart_vectors.py`: `$_POST` for
    /// POST, `request_parse_body()` for PUT (empty when it throws).
    #[test]
    fn reads_every_recorded_body_as_php_does() -> Result<(), Box<dyn std::error::Error>> {
        use base64::Engine as _;
        let vectors: Vec<serde_json::Value> =
            serde_json::from_str(include_str!("../tests/data/multipart_vectors.json"))?;
        assert!(vectors.len() > 100);
        let mut failures = Vec::new();
        for vector in &vectors {
            let name = vector["name"].as_str().unwrap_or("?");
            let method = vector["method"].as_str().unwrap_or("?");
            let content_type = vector["content_type"].as_str().unwrap_or_default();
            let body = base64::engine::general_purpose::STANDARD
                .decode(vector["body"].as_str().unwrap_or_default())?;
            // The kernel reads multipart bodies only (`BodyFormat::of`).
            let is_multipart = crate::BodyFormat::of(content_type.as_bytes())
                == Some(crate::BodyFormat::Multipart);
            let parsed = if is_multipart {
                parse(content_type.as_bytes(), &body)
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
