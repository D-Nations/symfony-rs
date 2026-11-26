//! BinaryFileResponseTest.php at symfony/http-foundation v8.1.11, with
//! the test it inherits from ResponseTestCase.php. Names and assertions are
//! PHPUnit's; each data set of a provider is a case of the same test.
//!
//! `__FILE__` is this file, `__DIR__.'/../README.md'` the crate's README,
//! `File/Fixtures` the copy in `tests/fixtures/file`. `FakeFile` is a
//! [`FileLike`] overriding what upstream's does. `clone $response` is
//! `Clone`; what `sendContent()` echoed is read from the recorded output in
//! place of `expectOutputString()`.
//!
//! Upstream: symfony/http-foundation Tests/BinaryFileResponseTest.php
#![allow(non_snake_case)]

#[path = "files/mod.rs"]
mod files;

use std::sync::Arc;

use files::{TestResult, fixture};
use php_value::{PhpString, Value, clock};
use symfony_http_foundation::{
    binary_file_response::{self, FileArgument, TempFile, trust_x_sendfile_type_header},
    file::{File, FileLike},
    file_stream::Stream,
    header_bag::HeaderValues,
    header_utils::{DISPOSITION_ATTACHMENT, DISPOSITION_INLINE},
    php::{OutputCall, with_output},
    request::{Create, Request},
    response::Response,
};

const NO_HEADERS: [(&str, HeaderValues); 0] = [];

fn this_file() -> Vec<u8> {
    format!(
        "{}/tests/binary_file_response_test.rs",
        env!("CARGO_MANIFEST_DIR")
    )
    .into_bytes()
}

fn readme() -> Vec<u8> {
    format!("{}/README.md", env!("CARGO_MANIFEST_DIR")).into_bytes()
}

fn octet_stream() -> [(&'static str, HeaderValues); 1] {
    [(
        "Content-Type",
        HeaderValues::from("application/octet-stream"),
    )]
}

fn path(path: Vec<u8>) -> FileArgument {
    FileArgument::Path(path)
}

/// `new BinaryFileResponse($file, $status, $headers)` with the defaults.
fn response<K: AsRef<[u8]>, V: Into<HeaderValues>>(
    file: FileArgument,
    status: i64,
    headers: impl IntoIterator<Item = (K, V)>,
) -> Result<Response, Box<dyn std::error::Error>> {
    Ok(binary_file_response::new(
        file, status, headers, true, None, false, true,
    )?)
}

fn get(response: &Response, name: &str) -> Option<String> {
    response
        .headers()
        .get(name, None)
        .map(|v| String::from_utf8_lossy(v.as_bytes()).into_owned())
}

fn set_header(request: &mut Request, name: &str, value: &str) -> TestResult {
    request.headers_mut().set(name.as_bytes(), value, true)?;
    Ok(())
}

fn create(uri: &str, method: &str) -> Result<Request, Box<dyn std::error::Error>> {
    Ok(Request::create_with(
        uri,
        Create {
            method: Some(PhpString::from(method)),
            ..Create::default()
        },
    )?)
}

/// `sendContent()`, and what it echoed.
fn send_content(response: &mut Response) -> Result<Vec<u8>, Box<dyn std::error::Error>> {
    with_output(|o| o.calls.clear());
    response
        .send_content()
        .map_err(|e| -> Box<dyn std::error::Error> { e.to_string().into() })?;
    Ok(with_output(|o| {
        o.calls
            .iter()
            .filter_map(|call| match call {
                OutputCall::Echo(text) => Some(text.as_bytes().to_vec()),
                _ => None,
            })
            .collect::<Vec<_>>()
            .concat()
    }))
}

fn gif() -> Result<Vec<u8>, Box<dyn std::error::Error>> {
    Ok(std::fs::read(String::from_utf8(fixture("test.gif"))?)?)
}

#[test]
fn testConstruction() -> TestResult {
    let file = readme();
    let response = binary_file_response::new(
        path(file.clone()),
        404,
        [("X-Header", HeaderValues::from("Foo"))],
        true,
        None,
        true,
        true,
    )?;
    assert_eq!(404, response.get_status_code());
    assert_eq!(Some("Foo".to_owned()), get(&response, "X-Header"));
    assert!(response.headers().has("ETag"));
    assert!(response.headers().has("Last-Modified"));
    assert!(!response.headers().has("Content-Disposition"));

    let response = binary_file_response::new(
        path(file),
        404,
        NO_HEADERS,
        true,
        Some(DISPOSITION_INLINE.as_bytes()),
        false,
        true,
    )?;
    assert_eq!(404, response.get_status_code());
    assert!(!response.headers().has("ETag"));
    assert_eq!(
        Some("inline; filename=README.md".to_owned()),
        get(&response, "Content-Disposition")
    );
    Ok(())
}

#[test]
fn testConstructWithNonAsciiFilename() -> TestResult {
    let dir = std::env::temp_dir().join(format!("symfony-rs-bfr-{}", std::process::id()));
    std::fs::create_dir_all(&dir)?;
    let file = dir.join("fööö.html");
    std::fs::write(&file, b"")?;
    let path_bytes = file.to_string_lossy().replace('\\', "/").into_bytes();
    let response = binary_file_response::new(
        path(path_bytes),
        200,
        NO_HEADERS,
        true,
        Some(b"attachment"),
        false,
        true,
    );
    let _ = std::fs::remove_file(&file);
    let _ = std::fs::remove_dir(&dir);
    let response = response?;

    let binary = response.binary_file().ok_or("not a BinaryFileResponse")?;
    assert_eq!("fööö.html".as_bytes(), binary.get_file().get_filename());
    Ok(())
}

#[test]
fn testSetContent() -> TestResult {
    let mut response = response(path(this_file()), 200, NO_HEADERS)?;
    let error = response
        .set_content(Some(PhpString::from("foo")))
        .err()
        .ok_or("expected a LogicException")?;
    assert!(error.is_a("LogicException"));
    Ok(())
}

#[test]
fn testGetContent() -> TestResult {
    let response = response(path(this_file()), 200, NO_HEADERS)?;
    assert_eq!(None, response.get_content());
    Ok(())
}

#[test]
fn testSetContentDispositionGeneratesSafeFallbackFilename() -> TestResult {
    let mut response = response(path(this_file()), 200, NO_HEADERS)?;
    response.set_content_disposition(
        DISPOSITION_ATTACHMENT.as_bytes(),
        "föö.html".as_bytes(),
        b"",
    )?;

    assert_eq!(
        Some("attachment; filename=f__.html; filename*=utf-8''f%C3%B6%C3%B6.html".to_owned()),
        get(&response, "Content-Disposition")
    );
    Ok(())
}

#[test]
fn testSetContentDispositionGeneratesSafeFallbackFilenameForTrailingNewline() -> TestResult {
    let mut response = response(path(this_file()), 200, NO_HEADERS)?;
    response.set_content_disposition(DISPOSITION_ATTACHMENT.as_bytes(), b"foo\n", b"")?;

    assert_eq!(
        Some("attachment; filename=foo_; filename*=utf-8''foo%0A".to_owned()),
        get(&response, "Content-Disposition")
    );
    Ok(())
}

#[test]
fn testSetContentDispositionRejectsTrailingNewlineInFallback() -> TestResult {
    let mut response = response(path(this_file()), 200, NO_HEADERS)?;
    let error = response
        .set_content_disposition(DISPOSITION_ATTACHMENT.as_bytes(), b"foo", b"foo\n")
        .err()
        .ok_or("expected an InvalidArgumentException")?;
    assert!(error.is_a("InvalidArgumentException"));
    Ok(())
}

#[test]
fn testSetContentDispositionGeneratesSafeFallbackFilenameForWronglyEncodedFilename() -> TestResult {
    let mut response = response(path(this_file()), 200, NO_HEADERS)?;
    // mb_convert_encoding('föö.html', 'ISO-8859-1', 'UTF-8')
    let iso88591_encoded_filename = b"f\xf6\xf6.html";
    response.set_content_disposition(
        DISPOSITION_ATTACHMENT.as_bytes(),
        iso88591_encoded_filename,
        b"",
    )?;

    // the parameter filename* is invalid in this case (rawurldecode('f%F6%F6') does not provide a UTF-8 string but an ISO-8859-1 encoded one)
    assert_eq!(
        Some("attachment; filename=f__.html; filename*=utf-8''f%F6%F6.html".to_owned()),
        get(&response, "Content-Disposition")
    );
    Ok(())
}

fn provide_ranges() -> [(&'static str, usize, usize, &'static str); 11] {
    [
        ("bytes=0-", 0, 35, "bytes 0-34/35"),
        ("bytes=0-34", 0, 35, "bytes 0-34/35"),
        ("bytes=-35", 0, 35, "bytes 0-34/35"),
        ("bytes=-40", 0, 35, "bytes 0-34/35"),
        ("bytes=1-4", 1, 4, "bytes 1-4/35"),
        ("Bytes=1-4", 1, 4, "bytes 1-4/35"),
        ("bytes=-5", 30, 5, "bytes 30-34/35"),
        ("bytes=30-", 30, 5, "bytes 30-34/35"),
        ("bytes=30-30", 30, 1, "bytes 30-30/35"),
        ("bytes=30-34", 30, 5, "bytes 30-34/35"),
        ("bytes=30-40", 30, 5, "bytes 30-34/35"),
    ]
}

#[test]
fn testRequests() -> TestResult {
    for (request_range, offset, length, response_range) in provide_ranges() {
        let mut response = response(path(fixture("test.gif")), 200, octet_stream())?;
        response.set_auto_etag()?;

        // do a request to get the ETag
        let mut request = Request::create("/")?;
        response.prepare(&mut request)?;
        let etag = get(&response, "ETag").ok_or("no ETag")?;

        // prepare a request for a range of the testing file
        let mut request = Request::create("/")?;
        set_header(&mut request, "If-Range", &etag)?;
        set_header(&mut request, "Range", request_range)?;

        let data = gif()?[offset..offset + length].to_vec();

        let mut response = response.clone();
        response.prepare(&mut request)?;
        assert_eq!(data, send_content(&mut response)?, "{request_range}");

        assert_eq!(206, response.get_status_code());
        assert_eq!(
            Some(response_range.to_owned()),
            get(&response, "Content-Range")
        );
        assert_eq!(Some(length.to_string()), get(&response, "Content-Length"));
    }
    Ok(())
}

#[test]
fn testRequestsWithoutEtag() -> TestResult {
    for (request_range, offset, length, response_range) in provide_ranges() {
        let mut response = response(path(fixture("test.gif")), 200, octet_stream())?;

        // do a request to get the LastModified
        let mut request = Request::create("/")?;
        response.prepare(&mut request)?;
        let last_modified = get(&response, "Last-Modified").ok_or("no Last-Modified")?;

        // prepare a request for a range of the testing file
        let mut request = Request::create("/")?;
        set_header(&mut request, "If-Range", &last_modified)?;
        set_header(&mut request, "Range", request_range)?;

        let data = gif()?[offset..offset + length].to_vec();

        let mut response = response.clone();
        response.prepare(&mut request)?;
        assert_eq!(data, send_content(&mut response)?, "{request_range}");

        assert_eq!(206, response.get_status_code());
        assert_eq!(
            Some(response_range.to_owned()),
            get(&response, "Content-Range")
        );
    }
    Ok(())
}

#[test]
fn testRangeRequestsWithoutLastModifiedDate() -> TestResult {
    // prevent auto last modified
    let response = binary_file_response::new(
        path(fixture("test.gif")),
        200,
        octet_stream(),
        true,
        None,
        false,
        false,
    )?;

    // prepare a request for a range of the testing file
    let mut request = Request::create("/")?;
    let now = php_value::datetime::DateTimeImmutable::from_timestamp(
        clock::time(),
        0,
        php_value::datetime::TimeZone::Utc,
    );
    set_header(
        &mut request,
        "If-Range",
        &format!("{} GMT", now.format("D, d M Y H:i:s")),
    )?;
    set_header(&mut request, "Range", "bytes=1-4")?;

    let mut response = response.clone();
    response.prepare(&mut request)?;
    assert_eq!(gif()?, send_content(&mut response)?);

    assert_eq!(200, response.get_status_code());
    assert_eq!(None, get(&response, "Content-Range"));
    Ok(())
}

#[test]
fn testFullFileRequests() -> TestResult {
    for request_range in [
        // Syntactical invalid range-request should also return the full resource
        "bytes=20-10",
        "bytes=50-40",
        "bytes=-",
        "bytes=abc-def",
        "bytes=1-2-3",
        "bytes=-5-10",
        "bytes=+5-10",
        "bytes=0x10-20",
        "bytes=1e1-20",
        // lists of ranges are not supported and must be ignored
        "bytes=0-4,10-14",
        "bytes=0-4, 10-14",
        "bytes=0-4 ,10-14",
        "bytes=-5,0-4",
        "bytes=0-,10-14",
        // range units other than bytes must be ignored
        "unknown=10-20",
    ] {
        let mut response = response(path(fixture("test.gif")), 200, octet_stream())?;
        response.set_auto_etag()?;

        // prepare a request for a range of the testing file
        let mut request = Request::create("/")?;
        set_header(&mut request, "Range", request_range)?;

        let data = gif()?[..35].to_vec();

        let mut response = response.clone();
        response.prepare(&mut request)?;
        assert_eq!(data, send_content(&mut response)?, "{request_range}");

        assert_eq!(200, response.get_status_code(), "{request_range}");
    }
    Ok(())
}

#[test]
fn testRangeOnPostMethod() -> TestResult {
    let mut request = create("/", "POST")?;
    set_header(&mut request, "Range", "bytes=10-20")?;
    let response = response(path(fixture("test.gif")), 200, octet_stream())?;

    let data = gif()?[..35].to_vec();

    let mut response = response.clone();
    response.prepare(&mut request)?;
    assert_eq!(data, send_content(&mut response)?);

    assert_eq!(200, response.get_status_code());
    assert_eq!(Some("35".to_owned()), get(&response, "Content-Length"));
    assert_eq!(None, get(&response, "Content-Range"));
    Ok(())
}

#[test]
fn testUnpreparedResponseSendsFullFile() -> TestResult {
    let response = response(path(fixture("test.gif")), 200, NO_HEADERS)?;

    let data = gif()?;

    let mut response = response.clone();
    assert_eq!(data, send_content(&mut response)?);

    assert_eq!(200, response.get_status_code());
    Ok(())
}

#[test]
fn testInvalidRequests() -> TestResult {
    // provideInvalidRanges()
    #[expect(clippy::single_element_loop, reason = "upstream's data provider")]
    for request_range in ["bytes=40-50"] {
        let mut response = response(path(fixture("test.gif")), 200, octet_stream())?;
        response.set_auto_etag()?;

        // prepare a request for a range of the testing file
        let mut request = Request::create("/")?;
        set_header(&mut request, "Range", request_range)?;

        let mut response = response.clone();
        response.prepare(&mut request)?;
        send_content(&mut response)?;

        assert_eq!(416, response.get_status_code());
        assert_eq!(
            Some("bytes */35".to_owned()),
            get(&response, "Content-Range")
        );
    }
    Ok(())
}

#[test]
fn testXSendfile() -> TestResult {
    let readme = String::from_utf8(readme())?;
    for file in [readme.clone(), format!("file://{readme}")] {
        let mut request = Request::create("/")?;
        set_header(&mut request, "X-Sendfile-Type", "X-Sendfile")?;

        trust_x_sendfile_type_header();
        let mut response = response(path(file.into_bytes()), 200, octet_stream())?;
        response.prepare(&mut request)?;

        assert_eq!(b"", send_content(&mut response)?.as_slice());

        assert!(
            get(&response, "X-Sendfile")
                .ok_or("no X-Sendfile")?
                .contains("README.md")
        );
    }
    Ok(())
}

#[test]
fn testSupportedXSendfileTypes() -> TestResult {
    for (kind, header) in [
        ("X-Sendfile", "X-Sendfile"),
        ("x-sendfile", "X-Sendfile"),
        ("X-LIGHTTPD-send-file", "X-LIGHTTPD-send-file"),
    ] {
        let mut request = Request::create("/")?;
        set_header(&mut request, "X-Sendfile-Type", kind)?;

        trust_x_sendfile_type_header();
        let mut response = response(path(readme()), 200, octet_stream())?;
        response.prepare(&mut request)?;

        assert_eq!(b"", send_content(&mut response)?.as_slice());

        let all = response.headers().all_preserve_case();
        assert!(
            all.iter()
                .any(|(name, _)| *name == php_value::Key::from_bytes(header.as_bytes())),
            "{kind}"
        );
        assert!(
            get(&response, header)
                .ok_or("no header")?
                .contains("README.md")
        );
    }
    Ok(())
}

#[test]
fn testUnsupportedXSendfileTypeIsIgnored() -> TestResult {
    for kind in ["X-Leak-Path", "Location"] {
        let mut request = Request::create("/")?;
        set_header(&mut request, "X-Sendfile-Type", kind)?;

        trust_x_sendfile_type_header();
        let mut response = response(path(fixture("test.gif")), 200, octet_stream())?;
        response.prepare(&mut request)?;

        assert!(!response.headers().has(kind));

        assert_eq!(gif()?, send_content(&mut response)?);
    }
    Ok(())
}

/// Upstream's `Tests\File\FakeFile`.
#[derive(Debug)]
struct FakeFile {
    realpath: Vec<u8>,
    file: File,
}

impl FakeFile {
    fn new(realpath: &str, path: Vec<u8>) -> Result<Self, Box<dyn std::error::Error>> {
        Ok(Self {
            realpath: realpath.as_bytes().to_vec(),
            file: File::new(path, false)?,
        })
    }
}

impl FileLike for FakeFile {
    fn get_pathname(&self) -> &[u8] {
        self.file.get_pathname()
    }

    fn is_readable(&self) -> bool {
        true
    }

    fn get_real_path(&self) -> Option<Vec<u8>> {
        Some(self.realpath.clone())
    }

    fn get_size(&self) -> symfony_http_foundation::exception::Result<Option<i64>> {
        Ok(Some(42))
    }

    fn get_m_time(&self) -> symfony_http_foundation::exception::Result<i64> {
        Ok(clock::time())
    }
}

#[test]
fn testXAccelMapping() -> TestResult {
    for (realpath, mapping, r#virtual) in [
        (
            "/var/www/var/www/files/foo.txt",
            "/var/www/=/files/",
            Some("/files/var/www/files/foo.txt"),
        ),
        (
            "/home/Foo/bar.txt",
            "/var/www/=/files/,/home/Foo/=/baz/",
            Some("/baz/bar.txt"),
        ),
        (
            "/home/Foo/bar.txt",
            "\"/var/www/\"=\"/files/\", \"/home/Foo/\"=\"/baz/\"",
            Some("/baz/bar.txt"),
        ),
        (
            "/tmp/bar.txt",
            "\"/var/www/\"=\"/files/\", \"/home/Foo/\"=\"/baz/\"",
            None,
        ),
        (
            "/var/www/var/www/files/foo%.txt",
            "/var/www/=/files/",
            Some("/files/var/www/files/foo%.txt"),
        ),
    ] {
        let mut request = Request::create("/")?;
        set_header(&mut request, "X-Sendfile-Type", "X-Accel-Redirect")?;
        set_header(&mut request, "X-Accel-Mapping", mapping)?;

        let file: Arc<dyn FileLike> = Arc::new(FakeFile::new(realpath, fixture("test"))?);

        trust_x_sendfile_type_header();
        let mut response = response(FileArgument::File(file), 200, octet_stream())?;
        response.prepare(&mut request)?;

        let header = get(&response, "X-Accel-Redirect");
        let header = match (r#virtual, header) {
            (Some(_), Some(header)) => {
                // Making sure the path doesn't contain characters unsupported by nginx
                let mut rest = header.as_bytes();
                while let Some((&c, tail)) = rest.split_first() {
                    match c {
                        b'?' => panic!("{header} has '?'"),
                        b'%' => {
                            assert!(
                                tail.len() >= 2
                                    && tail[..2]
                                        .iter()
                                        .all(|b| b.is_ascii_digit() || (b'A'..=b'F').contains(b)),
                                "{header}"
                            );
                            rest = &tail[2..];
                        }
                        _ => rest = tail,
                    }
                }
                Some(String::from_utf8(php_value::url::rawurldecode(
                    header.as_bytes(),
                ))?)
            }
            (_, header) => header,
        };
        assert_eq!(r#virtual.map(str::to_owned), header, "{realpath}");
    }
    Ok(())
}

#[test]
fn testDeleteFileAfterSend() -> TestResult {
    let scratch = files::Scratch::new("bfr-delete")?;
    let mut request = Request::create("/")?;

    let path_text = scratch.path("to_delete");
    std::fs::write(&path_text, b"")?;
    let real_path = files::realpath(&path_text).ok_or("no realpath")?;
    assert!(files::exists(&String::from_utf8(real_path.clone())?));

    let mut response = response(path(real_path), 200, octet_stream())?;
    response
        .binary_file_mut()
        .ok_or("not a BinaryFileResponse")?
        .delete_file_after_send(true);

    response.prepare(&mut request)?;
    send_content(&mut response)?;

    assert!(!files::exists(&path_text));
    Ok(())
}

#[test]
fn testShouldDeleteFileAfterSend() -> TestResult {
    let mut response = response(path(fixture("test.gif")), 200, NO_HEADERS)?;
    let binary = response
        .binary_file_mut()
        .ok_or("not a BinaryFileResponse")?;
    assert!(!binary.should_delete_file_after_send());

    binary.delete_file_after_send(true);
    assert!(binary.should_delete_file_after_send());

    binary.delete_file_after_send(false);
    assert!(!binary.should_delete_file_after_send());
    Ok(())
}

#[test]
fn testAcceptRangeOnUnsafeMethods() -> TestResult {
    let mut request = create("/", "POST")?;
    let mut response = response(path(fixture("test.gif")), 200, octet_stream())?;
    response.prepare(&mut request)?;

    assert_eq!(Some("none".to_owned()), get(&response, "Accept-Ranges"));
    Ok(())
}

#[test]
fn testAcceptRangeNotOverridden() -> TestResult {
    let mut request = create("/", "POST")?;
    let mut response = response(path(fixture("test.gif")), 200, octet_stream())?;
    response.headers_mut().set(b"Accept-Ranges", "foo", true)?;
    response.prepare(&mut request)?;

    assert_eq!(Some("foo".to_owned()), get(&response, "Accept-Ranges"));
    Ok(())
}

#[test]
fn testStream() -> TestResult {
    let mut request = Request::create("/")?;
    let stream: Arc<dyn FileLike> = Arc::new(Stream::new(readme(), true)?);
    let mut response = response(
        FileArgument::File(stream),
        200,
        [("Content-Type", HeaderValues::from("text/plain"))],
    )?;
    response.prepare(&mut request)?;

    assert_eq!(None, get(&response, "Content-Length"));
    Ok(())
}

#[test]
fn testPrepareNotAddingContentTypeHeaderIfNoContentResponse() -> TestResult {
    let mut request = Request::create("/")?;
    let now = php_value::datetime::DateTimeImmutable::from_timestamp(
        clock::time(),
        0,
        php_value::datetime::TimeZone::Utc,
    );
    set_header(
        &mut request,
        "If-Modified-Since",
        &format!("{} GMT", now.format("D, d M Y H:i:s")),
    )?;

    let mut response = response(path(fixture("test.gif")), 200, octet_stream())?;
    let yesterday = php_value::datetime::DateTimeImmutable::from_timestamp(
        clock::time() - 86_400,
        0,
        php_value::datetime::TimeZone::Utc,
    );
    response.set_last_modified(Some(&yesterday))?;

    response.is_not_modified(&mut request)?;
    response.prepare(&mut request)?;

    assert_eq!(304, response.get_status_code());
    assert!(!response.headers().has("Content-Type"));
    Ok(())
}

#[test]
fn testPrepareNotAddingContentLengthHeaderIfNotSuccessfulResponse() -> TestResult {
    let mut request = Request::create("/")?;
    set_header(&mut request, "Range", "bytes=0-10")?;
    let mut response = response(path(fixture("test.gif")), 404, NO_HEADERS)?;
    response.prepare(&mut request)?;

    assert_eq!(b"", send_content(&mut response)?.as_slice());

    assert_eq!(404, response.get_status_code());
    assert!(!response.headers().has("Content-Length"));
    assert!(!response.headers().has("Content-Range"));
    Ok(())
}

#[test]
fn testContentTypeIsCorrectlyDetected() -> TestResult {
    let file = File::new(fixture("test.gif"), true)?;
    if file.get_mime_type().is_err() {
        eprintln!("skipped: Guessing the mime type is not possible");
        return Ok(());
    }

    let mut response = response(FileArgument::File(Arc::new(file)), 200, NO_HEADERS)?;

    let mut request = Request::create("/")?;
    response.prepare(&mut request)?;

    assert_eq!(200, response.get_status_code());
    assert_eq!(Some("image/gif".to_owned()), get(&response, "Content-Type"));
    Ok(())
}

#[test]
fn testContentTypeIsNotGuessedWhenTheFileWasNotModified() -> TestResult {
    let mut response = response(path(fixture("test.gif")), 200, NO_HEADERS)?;
    response.set_auto_last_modified()?;

    let mut request = Request::create("/")?;
    let last_modified = response.get_last_modified()?.ok_or("no Last-Modified")?;
    set_header(
        &mut request,
        "If-Modified-Since",
        &format!("{} GMT", last_modified.format("D, d M Y H:i:s")),
    )?;
    let is_not_modified = response.is_not_modified(&mut request)?;
    assert!(is_not_modified);

    response.prepare(&mut request)?;

    assert_eq!(304, response.get_status_code());
    assert!(!response.headers().has("Content-Type"));
    Ok(())
}

/// `provideResponse()`.
fn provide_response() -> Result<Response, Box<dyn std::error::Error>> {
    response(path(readme()), 200, octet_stream())
}

#[test]
fn testNoCacheControlHeaderOnAttachmentUsingHTTPSAndMSIE() -> TestResult {
    let attachment = |request: &mut Request| -> Result<Response, Box<dyn std::error::Error>> {
        let mut response = provide_response()?;
        response.headers_mut().set(
            b"Content-Disposition",
            "attachment; filename=\"fname.ext\"",
            true,
        )?;
        response.prepare(request)?;
        Ok(response)
    };
    let agent = |request: &mut Request, agent: &str| {
        request
            .server_mut()
            .set("HTTP_USER_AGENT", Value::string(agent));
    };
    let ie8 = "Mozilla/4.0 (compatible; MSIE 8.0; Windows NT 6.0; Trident/4.0)";

    // Check for HTTPS and IE 8
    let mut request = Request::default();
    request.server_mut().set("HTTPS", Value::Bool(true));
    agent(&mut request, ie8);
    assert!(!attachment(&mut request)?.headers().has("Cache-Control"));

    // Check for IE 10 and HTTPS
    agent(
        &mut request,
        "Mozilla/5.0 (compatible; MSIE 10.0; Windows NT 6.1; WOW64; Trident/6.0)",
    );
    assert!(attachment(&mut request)?.headers().has("Cache-Control"));

    // Check for IE 9 and HTTPS
    agent(
        &mut request,
        "Mozilla/5.0 (compatible; MSIE 9.0; Windows NT 7.1; Trident/5.0)",
    );
    assert!(attachment(&mut request)?.headers().has("Cache-Control"));

    // Check for IE 9 and HTTP
    request.server_mut().set("HTTPS", Value::Bool(false));
    assert!(attachment(&mut request)?.headers().has("Cache-Control"));

    // Check for IE 8 and HTTP
    agent(&mut request, ie8);
    assert!(attachment(&mut request)?.headers().has("Cache-Control"));

    // Check for non-IE and HTTPS
    request.server_mut().set("HTTPS", Value::Bool(true));
    agent(
        &mut request,
        "Mozilla/5.0 (Windows NT 6.1; WOW64) AppleWebKit/537.17 (KHTML, like Gecko) Chrome/24.0.1312.60 Safari/537.17",
    );
    assert!(attachment(&mut request)?.headers().has("Cache-Control"));

    // Check for non-IE and HTTP
    request.server_mut().set("HTTPS", Value::Bool(false));
    assert!(attachment(&mut request)?.headers().has("Cache-Control"));
    Ok(())
}

#[test]
fn testCreateFromTemporaryFile() -> TestResult {
    let file = TempFile::default();
    file.write(b"foo,bar");

    let mut response = response(
        FileArgument::Temp(file),
        201,
        [("Content-Type", HeaderValues::from("text/csv"))],
    )?;
    response.set_content_disposition(DISPOSITION_ATTACHMENT.as_bytes(), b"", b"")?;

    assert_eq!(201, response.get_status_code());
    assert_eq!(Some("text/csv".to_owned()), get(&response, "Content-Type"));
    assert_eq!(
        Some("attachment; filename=temp".to_owned()),
        get(&response, "Content-Disposition")
    );

    response.set_auto_last_modified()?;
    response.prepare(&mut Request::default())?;
    assert_eq!(Some("7".to_owned()), get(&response, "Content-Length"));
    let string = send_content(&mut response)?;

    assert_eq!(b"foo,bar", string.as_slice());
    Ok(())
}

#[test]
fn testSetChunkSizeTooSmall() -> TestResult {
    let mut response = response(path(fixture("test.gif")), 200, NO_HEADERS)?;
    let error = response
        .binary_file_mut()
        .ok_or("not a BinaryFileResponse")?
        .set_chunk_size(0)
        .err()
        .ok_or("expected an InvalidArgumentException")?;
    assert!(error.is_a("InvalidArgumentException"));
    assert_eq!(
        "The chunk size of a BinaryFileResponse cannot be less than 1.".as_bytes(),
        error.message().as_bytes()
    );
    Ok(())
}

#[test]
fn testCreateFromTemporaryFileWithoutMimeType() -> TestResult {
    let file = TempFile::default();
    file.write(b"foo,bar");

    let mut response = response(FileArgument::Temp(file), 200, NO_HEADERS)?;
    response.prepare(&mut Request::default())?;

    assert_eq!(
        Some("application/octet-stream".to_owned()),
        get(&response, "Content-Type")
    );
    Ok(())
}
