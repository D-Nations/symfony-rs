//! Port of Symfony\Component\HttpFoundation\BinaryFileResponse.php: a
//! response that delivers a file. It covers `Content-Length`, byte ranges
//! (`Range`, `If-Range`, 206 and 416), `X-Sendfile`-style headers for the
//! web server, `Content-Disposition` with an ASCII fallback, an automatic
//! `ETag` (`xxh128` of the file) and `Last-Modified`, and deleting the file
//! once it is sent.
//!
//! A [`BinaryFileResponse`] is a [`Response`] carrying a [`BinaryFile`]
//! (the subclass's state); `prepare()`, `sendContent()`, `setContent()`
//! and `getContent()` check for it, as PHP's late binding does. The file
//! is a [`FileLike`]: a `File`, a `File\Stream` (no size), or an object of
//! another subclass. `\SplTempFileObject` is a [`TempFile`], whose name is
//! `php://temp`.
//!
//! `trustXSendfileTypeHeader()` sets a process-wide flag, as PHP's static
//! property lasts as long as the worker process. The file is written in
//! chunks (16 KiB by default), each an echo, stopping once
//! `connection_aborted()`. `ignore_user_abort(true)` has no Rust
//! counterpart: the response goes on being written until the client is
//! seen to be gone, as PHP's does with it.
//!
//! Symfony source copyright (c) Fabien Potencier; distributed under MIT.
//!
//! Upstream: symfony/http-foundation BinaryFileResponse.php

use std::{
    fmt,
    io::Read,
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
};

use php_value::{
    PhpString, Value, clock,
    datetime::{DateTimeImmutable, TimeZone},
    url::rawurlencode,
};

use crate::{
    exception::{Class, Error, Result},
    file::{File, FileLike},
    header_bag::{HeaderValues, ResponseHeaderBag},
    header_utils,
    php::{Stream, with_output},
    request::Request,
    response::Response,
    streamed_response::{Throwable, echo},
};

/// `BinaryFileResponse::class`.
pub const CLASS: &str = "Symfony\\Component\\HttpFoundation\\BinaryFileResponse";

/// A `BinaryFileResponse`, which is a [`Response`].
pub type BinaryFileResponse = Response;

/// `self::$trustXSendfileTypeHeader`.
static TRUST_X_SENDFILE_TYPE_HEADER: AtomicBool = AtomicBool::new(false);

/// `BinaryFileResponse::trustXSendfileTypeHeader()`.
pub fn trust_x_sendfile_type_header() {
    TRUST_X_SENDFILE_TYPE_HEADER.store(true, Ordering::Relaxed);
}

/// Sets `self::$trustXSendfileTypeHeader` back to `false`, as a test does
/// by reflection between cases.
#[doc(hidden)]
pub fn distrust_x_sendfile_type_header() {
    TRUST_X_SENDFILE_TYPE_HEADER.store(false, Ordering::Relaxed);
}

/// `X-Sendfile-Type` values, lowercase, and the header each sends.
const SENDFILE_TYPES: [(&str, &str); 3] = [
    ("x-sendfile", "X-Sendfile"),
    ("x-accel-redirect", "X-Accel-Redirect"),
    ("x-lighttpd-send-file", "X-LIGHTTPD-send-file"),
];

/// An `\SplTempFileObject`: bytes written to `php://temp`.
pub type TempFile = Stream;

/// What `new BinaryFileResponse($file)` and `setFile($file)` take.
#[derive(Debug, Clone)]
pub enum FileArgument {
    /// A path, made a checked `File`.
    Path(Vec<u8>),
    /// A `File`, or an object of a subclass.
    File(Arc<dyn FileLike>),
    /// An `\SplTempFileObject`.
    Temp(TempFile),
}

/// The state `BinaryFileResponse` adds to `Response`.
#[derive(Clone)]
pub struct BinaryFile {
    /// `$this->file`.
    file: Arc<dyn FileLike>,
    /// `$this->tempFileObject`.
    temp_file_object: Option<TempFile>,
    /// `$this->offset`.
    offset: i64,
    /// `$this->maxlen`: -1 for the rest of the file.
    maxlen: i64,
    /// `$this->deleteFileAfterSend`.
    delete_file_after_send: bool,
    /// `$this->chunkSize`.
    chunk_size: i64,
}

impl fmt::Debug for BinaryFile {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("BinaryFile")
            .field("file", &self.file)
            .field("temp", &self.temp_file_object.is_some())
            .field("offset", &self.offset)
            .field("maxlen", &self.maxlen)
            .field("delete_file_after_send", &self.delete_file_after_send)
            .field("chunk_size", &self.chunk_size)
            .finish()
    }
}

/// `==` on the state: the same file object and temporary file, and equal
/// values.
impl PartialEq for BinaryFile {
    fn eq(&self, other: &Self) -> bool {
        let temp = match (&self.temp_file_object, &other.temp_file_object) {
            (None, None) => true,
            (Some(a), Some(b)) => a.same(b),
            _ => false,
        };
        Arc::ptr_eq(&self.file, &other.file)
            && temp
            && self.offset == other.offset
            && self.maxlen == other.maxlen
            && self.delete_file_after_send == other.delete_file_after_send
            && self.chunk_size == other.chunk_size
    }
}

impl BinaryFile {
    /// `getFile()`.
    pub fn get_file(&self) -> &Arc<dyn FileLike> {
        &self.file
    }

    /// `setChunkSize($chunkSize)`.
    pub fn set_chunk_size(&mut self, chunk_size: i64) -> Result<&mut Self> {
        if chunk_size < 1 {
            return Err(Error::new(
                Class::PhpInvalidArgument,
                "The chunk size of a BinaryFileResponse cannot be less than 1.",
            ));
        }
        self.chunk_size = chunk_size;
        Ok(self)
    }

    /// `deleteFileAfterSend($shouldDelete)`.
    pub fn delete_file_after_send(&mut self, should_delete: bool) -> &mut Self {
        self.delete_file_after_send = should_delete;
        self
    }

    /// `shouldDeleteFileAfterSend()`.
    pub fn should_delete_file_after_send(&self) -> bool {
        self.delete_file_after_send
    }
}

/// `new BinaryFileResponse($file, $status, $headers, $public,
/// $contentDisposition, $autoEtag, $autoLastModified)`.
///
/// Fails with `FileNotFoundException` for a path that is not a file,
/// `FileException` for a file that cannot be read, and
/// `\InvalidArgumentException` for a status code or disposition that is
/// not valid.
pub fn new<K: AsRef<[u8]>, V: Into<HeaderValues>>(
    file: FileArgument,
    status: i64,
    headers: impl IntoIterator<Item = (K, V)>,
    public: bool,
    content_disposition: Option<&[u8]>,
    auto_etag: bool,
    auto_last_modified: bool,
) -> Result<Response> {
    let placeholder = BinaryFile {
        file: Arc::new(File::new(Vec::new(), false)?),
        temp_file_object: None,
        offset: 0,
        maxlen: -1,
        delete_file_after_send: false,
        chunk_size: 16 * 1024,
    };
    let mut response =
        Response::with_binary_file(status, ResponseHeaderBag::new(headers)?, placeholder)?;
    response.set_file(file, content_disposition, auto_etag, auto_last_modified)?;
    if public {
        response.set_public()?;
    }
    Ok(response)
}

fn not_binary() -> Error {
    Error::new(Class::NotPorted, "not a BinaryFileResponse")
}

impl Response {
    fn binary(&self) -> Result<&BinaryFile> {
        self.binary_file().ok_or_else(not_binary)
    }

    fn binary_mut(&mut self) -> Result<&mut BinaryFile> {
        self.binary_file_mut().ok_or_else(not_binary)
    }

    /// `setFile($file, $contentDisposition, $autoEtag,
    /// $autoLastModified)`.
    pub fn set_file(
        &mut self,
        file: FileArgument,
        content_disposition: Option<&[u8]>,
        auto_etag: bool,
        auto_last_modified: bool,
    ) -> Result<&mut Self> {
        let (file, temp): (Arc<dyn FileLike>, Option<TempFile>) = match file {
            FileArgument::Path(path) => (Arc::new(File::new(path, true)?), None),
            FileArgument::File(file) => (file, None),
            FileArgument::Temp(temp) => (Arc::new(File::new("php://temp", false)?), Some(temp)),
        };
        let is_temporary_file = temp.is_some();

        if !file.is_readable() && !is_temporary_file {
            return Err(Error::new(Class::File, "File must be readable."));
        }

        let binary = self.binary_mut()?;
        binary.temp_file_object = temp;
        binary.file = file;

        if auto_etag {
            self.set_auto_etag()?;
        }

        if auto_last_modified && !is_temporary_file {
            self.set_auto_last_modified()?;
        }

        if let Some(disposition) = content_disposition
            && Value::String(PhpString::from(disposition)).is_truthy()
        {
            self.set_content_disposition(disposition, b"", b"")?;
        }

        Ok(self)
    }

    /// `setAutoLastModified()`: the file's modification time, or now for a
    /// temporary file.
    pub fn set_auto_last_modified(&mut self) -> Result<&mut Self> {
        let binary = self.binary()?;
        let timestamp = if binary.temp_file_object.is_some() {
            clock::time()
        } else {
            binary.file.get_m_time()?
        };
        let date = DateTimeImmutable::from_timestamp(timestamp, 0, TimeZone::Offset(0));
        self.set_last_modified(Some(&date))
    }

    /// `setAutoEtag()`: `base64_encode(hash_file('xxh128', $path, true))`.
    /// A file that cannot be read hashes as PHP's `false`, which encodes as
    /// `''`.
    pub fn set_auto_etag(&mut self) -> Result<&mut Self> {
        let binary = self.binary()?;
        let etag = hash_file_xxh128(binary.file.get_pathname())
            .map(|hash| base64_encode(&hash))
            .unwrap_or_default();
        self.set_etag(Some(&etag), false)
    }

    /// `setContentDisposition($disposition, $filename,
    /// $filenameFallback)`: an empty filename is the file's; without a
    /// fallback, a filename that is not printable ASCII or has `%` gets one
    /// with `_` for each other character.
    pub fn set_content_disposition(
        &mut self,
        disposition: &[u8],
        filename: &[u8],
        filename_fallback: &[u8],
    ) -> Result<&mut Self> {
        let filename = if filename.is_empty() {
            self.binary()?.file.get_filename().to_vec()
        } else {
            filename.to_vec()
        };

        let mut fallback = filename_fallback.to_vec();
        if fallback.is_empty()
            && (!filename.iter().all(|&b| (0x20..=0x7e).contains(&b)) || filename.contains(&b'%'))
        {
            fallback = ascii_fallback(&filename);
        }

        let header = header_utils::make_disposition(disposition, &filename, &fallback)?;
        self.headers_mut()
            .set(b"Content-Disposition", header, true)?;
        Ok(self)
    }
}

/// The fallback `setContentDisposition()` builds: each character of
/// `mb_detect_encoding($filename, null, true)` (ASCII or UTF-8, else
/// bytes) kept when it is printable ASCII other than `%`, else `_`.
fn ascii_fallback(filename: &[u8]) -> Vec<u8> {
    let keep = |b: u8| b != b'%' && (32..=126).contains(&b);
    match std::str::from_utf8(filename) {
        Ok(text) => text
            .chars()
            .map(|c| match u8::try_from(c) {
                Ok(b) if keep(b) => b,
                _ => b'_',
            })
            .collect(),
        Err(_) => filename
            .iter()
            .map(|&b| if keep(b) { b } else { b'_' })
            .collect(),
    }
}

/// `hash_file('xxh128', $path, true)`; `None` is `false`.
fn hash_file_xxh128(path: &[u8]) -> Option<[u8; 16]> {
    let bytes = if path == b"php://temp" || path.starts_with(b"php://temp/") {
        // A new php://temp stream is empty.
        Vec::new()
    } else {
        symfony_finder::fs::read(path).ok()?
    };
    Some(twox_hash::XxHash3_128::oneshot(&bytes).to_be_bytes())
}

fn base64_encode(bytes: &[u8]) -> Vec<u8> {
    const ALPHABET: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut out = Vec::with_capacity(bytes.len().div_ceil(3) * 4);
    for chunk in bytes.chunks(3) {
        let b = [
            chunk[0],
            chunk.get(1).copied().unwrap_or(0),
            chunk.get(2).copied().unwrap_or(0),
        ];
        let n = (u32::from(b[0]) << 16) | (u32::from(b[1]) << 8) | u32::from(b[2]);
        for (i, shift) in [18, 12, 6, 0].into_iter().enumerate() {
            if i <= chunk.len() {
                out.push(ALPHABET[((n >> shift) & 63) as usize]);
            } else {
                out.push(b'=');
            }
        }
    }
    out
}

/// `(int) $digits` for a string of digits: saturating, as PHP's cast of a
/// numeric string that overflows.
fn int_of_digits(digits: &[u8]) -> i64 {
    digits.iter().fold(0_i64, |n, &d| {
        n.saturating_mul(10).saturating_add(i64::from(d - b'0'))
    })
}

/// `preg_match('/^bytes=(\d*)-(\d*)$/i', $range, $matches)`: the two
/// groups. `$` also matches before a final newline.
fn parse_range(range: &[u8]) -> Option<(&[u8], &[u8])> {
    let range = range.strip_suffix(b"\n").unwrap_or(range);
    let rest = range
        .get(..6)
        .filter(|p| p.eq_ignore_ascii_case(b"bytes="))
        .map(|_| &range[6..])?;
    let dash = rest.iter().position(|&b| b == b'-')?;
    let (start, end) = (&rest[..dash], &rest[dash + 1..]);
    (start.iter().all(u8::is_ascii_digit) && end.iter().all(u8::is_ascii_digit))
        .then_some((start, end))
}

/// `prepare($request)`.
pub(crate) fn prepare(response: &mut Response, request: &mut Request) -> Result<()> {
    if !response.is_successful() || response.is_empty() {
        response.prepare_parent(request)?;
        response.binary_mut()?.maxlen = 0;
        return Ok(());
    }

    if !response.headers().has("Content-Type") {
        let binary = response.binary()?;
        let mime_type = if binary.temp_file_object.is_none() {
            binary.file.get_mime_type()?
        } else {
            None
        };
        let mime_type = mime_type
            .filter(|m| Value::string(m.as_str()).is_truthy())
            .unwrap_or_else(|| "application/octet-stream".to_owned());
        response
            .headers_mut()
            .set(b"Content-Type", mime_type.as_str(), true)?;
    }

    response.prepare_parent(request)?;

    let binary = response.binary_mut()?;
    binary.offset = 0;
    binary.maxlen = -1;

    let file_size = match &binary.temp_file_object {
        Some(temp) => i64::try_from(temp.size()).unwrap_or(i64::MAX),
        None => match binary.file.get_size()? {
            Some(size) => size,
            None => return Ok(()),
        },
    };
    let file = Arc::clone(&binary.file);

    response.headers_mut().remove("Transfer-Encoding");
    response
        .headers_mut()
        .set(b"Content-Length", file_size.to_string().as_str(), true)?;

    if !response.headers().has("Accept-Ranges") {
        // Only accept ranges on safe HTTP methods
        let value = if request.is_method_safe()? {
            "bytes"
        } else {
            "none"
        };
        response.headers_mut().set(b"Accept-Ranges", value, true)?;
    }

    let sendfile_type = if TRUST_X_SENDFILE_TYPE_HEADER.load(Ordering::Relaxed) {
        request.headers().get("X-Sendfile-Type", None)
    } else {
        None
    };
    let lower = sendfile_type
        .map(|t| t.as_bytes().to_ascii_lowercase())
        .unwrap_or_default();
    let kind = SENDFILE_TYPES
        .iter()
        .find(|(name, _)| name.as_bytes() == lower)
        .map(|(_, header)| *header);

    if let Some(kind) = kind {
        sendfile_headers(response, request, file.as_ref(), kind)?;
    } else if request.headers().has("Range") && request.is_method("GET")? {
        // Process the range headers.
        let if_range_valid = !request.headers().has("If-Range")
            || has_valid_if_range_header(response, request.headers().get("If-Range", None))?;
        if if_range_valid {
            let range = request.headers().get("Range", None).unwrap_or_default();
            if let Some((start, end)) = parse_range(range.as_bytes())
                && (!start.is_empty() || !end.is_empty())
            {
                range_headers(response, start, end, file_size)?;
            }
        }
    }

    if request.is_method("HEAD")? {
        response.binary_mut()?.maxlen = 0;
    }

    Ok(())
}

/// The `X-Sendfile`-style header for `kind`, which the web server sends
/// the file for: no content is sent.
fn sendfile_headers(
    response: &mut Response,
    request: &Request,
    file: &dyn FileLike,
    kind: &str,
) -> Result<()> {
    // Use X-Sendfile, do not send any content.
    // Fall back to scheme://path for stream wrapped locations.
    let mut path = file
        .get_real_path()
        .unwrap_or_else(|| file.get_pathname().to_vec());

    if kind == "X-Accel-Redirect" {
        // Do X-Accel-Mapping substitutions.
        if !request.headers().has("X-Accel-Mapping") {
            return Err(Error::new(
                Class::PhpLogic,
                "The \"X-Accel-Mapping\" header must be set when \"X-Sendfile-Type\" is set to \"X-Accel-Redirect\".",
            ));
        }
        let mapping = request
            .headers()
            .get("X-Accel-Mapping", None)
            .unwrap_or_default();
        let parts = header_utils::split(mapping.as_bytes(), b",=")?;
        for part in parts.values() {
            // [$pathPrefix, $location] = $part
            let (prefix, location) = match part {
                Value::Array(pair) => {
                    let mut values = pair.values();
                    (values.next().cloned(), values.next().cloned())
                }
                _ => (None, None),
            };
            let text = |v: Option<Value>| {
                v.and_then(|v| v.to_php_string())
                    .map(|s| s.as_bytes().to_vec())
                    .unwrap_or_default()
            };
            let (prefix, location) = (text(prefix), text(location));
            if path.starts_with(&prefix) {
                path = [&location[..], &path[prefix.len()..]].concat();
                // Only set X-Accel-Redirect header if a valid URI can be produced
                // as nginx does not serve arbitrary file paths.
                response.headers_mut().set(
                    kind.as_bytes(),
                    PhpString::from(rawurlencode(&path)),
                    true,
                )?;
                response.binary_mut()?.maxlen = 0;
                break;
            }
        }
    } else {
        response
            .headers_mut()
            .set(kind.as_bytes(), PhpString::from(path), true)?;
        response.binary_mut()?.maxlen = 0;
    }
    Ok(())
}

/// The `Range` of `bytes=start-end`, once parsed.
fn range_headers(response: &mut Response, start: &[u8], end: &[u8], file_size: i64) -> Result<()> {
    let mut end = if end.is_empty() {
        file_size - 1
    } else {
        int_of_digits(end)
    };
    let start = if start.is_empty() {
        // a suffix longer than the file selects the whole file, see RFC 9110 section 14.1.2
        let start = 0.max(file_size.saturating_sub(end));
        end = file_size - 1;
        start
    } else {
        int_of_digits(start)
    };

    if start <= end {
        end = end.min(file_size - 1);
        if start < 0 || start > end {
            response.set_status_code(416, None)?;
            response.headers_mut().set(
                b"Content-Range",
                format!("bytes */{file_size}").as_str(),
                true,
            )?;
        } else {
            let binary = response.binary_mut()?;
            binary.maxlen = if end < file_size { end - start + 1 } else { -1 };
            binary.offset = start;
            response.set_status_code(206, None)?;
            response.headers_mut().set(
                b"Content-Range",
                format!("bytes {start}-{end}/{file_size}").as_str(),
                true,
            )?;
            response.headers_mut().set(
                b"Content-Length",
                (end - start + 1).to_string().as_str(),
                true,
            )?;
        }
    }
    Ok(())
}

/// `hasValidIfRangeHeader($header)`.
fn has_valid_if_range_header(response: &Response, header: Option<PhpString>) -> Result<bool> {
    if response.get_etag() == header {
        return Ok(true);
    }
    let Some(last_modified) = response.get_last_modified()? else {
        return Ok(false);
    };
    let formatted = format!("{} GMT", last_modified.format("D, d M Y H:i:s"));
    Ok(header.is_some_and(|h| h.as_bytes() == formatted.as_bytes()))
}

/// The file `sendContent()` reads.
enum Source {
    Temp(TempFile),
    File(std::fs::File),
}

impl Source {
    /// `fread($length)`: up to `length` bytes, fewer only at the end.
    fn read(&mut self, length: usize) -> std::io::Result<Vec<u8>> {
        match self {
            Self::Temp(temp) => Ok(temp.read(length)),
            Self::File(file) => {
                let mut data = Vec::with_capacity(length.min(1 << 20));
                file.by_ref()
                    .take(u64::try_from(length).unwrap_or(u64::MAX))
                    .read_to_end(&mut data)?;
                Ok(data)
            }
        }
    }
}

/// `new \SplFileObject($pathname, 'r')`, seeked to `offset`.
fn open(binary: &BinaryFile) -> std::result::Result<Source, Throwable> {
    if let Some(temp) = &binary.temp_file_object {
        temp.rewind();
        if binary.offset != 0 {
            temp.seek(usize::try_from(binary.offset).unwrap_or(usize::MAX));
        }
        return Ok(Source::Temp(temp.clone()));
    }
    let pathname = binary.file.get_pathname();
    let failed = |reason: &str| -> Throwable {
        Arc::new(Error::new(
            Class::PhpRuntime,
            format!(
                "SplFileObject::__construct({}): Failed to open stream: {reason}",
                String::from_utf8_lossy(pathname)
            ),
        ))
    };
    let path = symfony_filesystem::php::local_path(pathname)
        .ok_or_else(|| failed("No such file or directory"))?;
    let mut file = std::fs::File::open(&path).map_err(|e| {
        failed(match e.kind() {
            std::io::ErrorKind::PermissionDenied => "Permission denied",
            _ => "No such file or directory",
        })
    })?;
    if binary.offset != 0 {
        use std::io::Seek;
        file.seek(std::io::SeekFrom::Start(
            u64::try_from(binary.offset).unwrap_or(0),
        ))
        .map_err(|e| -> Throwable { Arc::new(e) })?;
    }
    Ok(Source::File(file))
}

/// `sendContent()`: the range of the file, in chunks, until the client
/// is gone; then the file is deleted when asked, whatever happened.
pub(crate) fn send_content(response: &mut Response) -> std::result::Result<(), Throwable> {
    let successful = response.is_successful();
    let binary = response
        .binary_file()
        .cloned()
        .ok_or_else(|| -> Throwable { Arc::new(not_binary()) })?;
    let sent = write_file(&binary, successful);

    if binary.temp_file_object.is_none()
        && binary.delete_file_after_send
        && symfony_finder::fs::is_file(binary.file.get_pathname())
    {
        symfony_filesystem::php::unlink(binary.file.get_pathname());
    }
    sent
}

fn write_file(binary: &BinaryFile, successful: bool) -> std::result::Result<(), Throwable> {
    if !successful || binary.maxlen == 0 {
        return Ok(());
    }
    let mut file = open(binary)?;
    let chunk_size = usize::try_from(binary.chunk_size).unwrap_or(usize::MAX);
    let mut length = binary.maxlen;
    while length != 0 {
        let read = if length < 0 {
            chunk_size
        } else {
            usize::try_from(length)
                .unwrap_or(usize::MAX)
                .min(chunk_size)
        };
        let data = file.read(read).map_err(|e| -> Throwable { Arc::new(e) })?;
        if data.is_empty() {
            // The end of the file.
            break;
        }
        let written = i64::try_from(data.len()).unwrap_or(i64::MAX);
        echo(data);
        if with_output(|o| o.connection_aborted) {
            break;
        }
        if length > 0 {
            length -= written;
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn base64_encodes_as_php() {
        assert_eq!(base64_encode(b""), b"");
        assert_eq!(base64_encode(b"f"), b"Zg==");
        assert_eq!(base64_encode(b"fo"), b"Zm8=");
        assert_eq!(base64_encode(b"foo"), b"Zm9v");
        assert_eq!(base64_encode(b"foob"), b"Zm9vYg==");
    }

    #[test]
    fn ranges_parse_as_the_pattern() {
        assert_eq!(parse_range(b"bytes=1-4"), Some((&b"1"[..], &b"4"[..])));
        assert_eq!(parse_range(b"Bytes=-5"), Some((&b""[..], &b"5"[..])));
        assert_eq!(parse_range(b"bytes=1-4\n"), Some((&b"1"[..], &b"4"[..])));
        assert_eq!(parse_range(b"bytes=1-2-3"), None);
        assert_eq!(parse_range(b"bytes=+5-10"), None);
        assert_eq!(parse_range(b"unknown=10-20"), None);
    }

    #[test]
    fn fallbacks_replace_each_character() {
        assert_eq!(ascii_fallback("föö.html".as_bytes()), b"f__.html");
        assert_eq!(ascii_fallback(b"f\xf6\xf6.html"), b"f__.html");
        assert_eq!(ascii_fallback(b"a%b\n"), b"a_b_");
    }

    #[test]
    fn digits_saturate() {
        assert_eq!(int_of_digits(b"0042"), 42);
        assert_eq!(int_of_digits(b"99999999999999999999"), i64::MAX);
    }
}
