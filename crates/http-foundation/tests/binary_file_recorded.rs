//! `BinaryFileResponse` against PHP 8.4.27 running the pinned
//! symfony/http-foundation (v8.1.11), recorded by
//! `oracle/recorders/http_foundation_binary_file.php` (`scripts/record.sh
//! http-foundation-binary-file`): 1,500 generated scripts.
//!
//! Each script recreates a directory of files (contents, names and
//! modification times from pools) and builds a response from a path, a
//! `File`, a `File\Stream`, a temporary file, a `file://` URL or a missing
//! path, trusting `X-Sendfile-Type` or not. It then runs operations
//! (dispositions, chunk sizes, delete after send, automatic `ETag` and
//! `Last-Modified`, `setContent()`, `getContent()`), then `prepare()` with
//! a request (method, `Range`, `If-Range`, `X-Sendfile-Type`,
//! `X-Accel-Mapping`) and `sendContent()`, some with the client gone.
//!
//! Compared: each result; each echo's length and the CRC32 of what was
//! echoed; then the status, the headers, the file's pathname and whether
//! it still exists. The directory's path is `{dir}` on both sides.
//!
//! Upstream: none

use std::{
    sync::Arc,
    time::{Duration, SystemTime},
};

use php_value::{PhpArray, PhpString, Value, clock, url::rawurlencode, vectors};
use serde_json::{Value as Json, json};
use symfony_http_foundation::{
    binary_file_response::{
        self, FileArgument, TempFile, distrust_x_sendfile_type_header, trust_x_sendfile_type_header,
    },
    exception::Error,
    file::{File, FileLike},
    file_stream::Stream,
    header_bag::HeaderValues,
    php::{Globals, OutputCall, set_globals, with_output},
    request::{Create, Request},
    response::Response,
    streamed_response::Throwable,
};

type TestError = Box<dyn std::error::Error + Send + Sync>;

const DATA: &str = include_str!("data/binary_file.json");

fn text(json: &Json) -> Result<&str, TestError> {
    json.as_str().ok_or_else(|| "expected a string".into())
}

fn bytes(json: &Json) -> Result<Vec<u8>, TestError> {
    vectors::base64(text(json)?)
}

fn items(json: &Json) -> Result<&Vec<Json>, TestError> {
    json.as_array().ok_or_else(|| "expected a list".into())
}

/// The recorder's content pool.
fn contents(index: u64) -> Vec<u8> {
    match index {
        0 => Vec::new(),
        1 => b"A".to_vec(),
        2 => b"hello world\n".to_vec(),
        3 => b"0123456789ABCDEFGHIJKLMNOPQRSTUVWXY".to_vec(),
        _ => (0..20_000_u32)
            .map(|i| u8::try_from((i * 7 + 3) % 256).unwrap_or(0))
            .collect(),
    }
}

/// `crc32()` (IEEE).
fn crc32(data: &[u8]) -> u32 {
    let mut crc = !0_u32;
    for &byte in data {
        crc ^= u32::from(byte);
        for _ in 0..8 {
            crc = if crc & 1 == 1 {
                (crc >> 1) ^ 0xEDB8_8320
            } else {
                crc >> 1
            };
        }
    }
    !crc
}

struct Dir {
    /// The directory, as `realpath()` gives it.
    path: String,
}

impl Dir {
    fn new() -> Result<Self, TestError> {
        let dir =
            std::env::temp_dir().join(format!("symfony-rs-bfr-recorded-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir)?;
        let real =
            symfony_finder::fs::realpath(dir.to_string_lossy().as_bytes()).ok_or("no realpath")?;
        Ok(Self {
            path: String::from_utf8(real)?,
        })
    }

    fn file(&self, name: &[u8]) -> Vec<u8> {
        [self.path.as_bytes(), b"/", name].concat()
    }

    /// The script's files, freshly written.
    fn fill(&self, files: &[Json]) -> Result<(), TestError> {
        for entry in std::fs::read_dir(&self.path)? {
            std::fs::remove_file(entry?.path())?;
        }
        for file in files {
            let name = String::from_utf8(bytes(&file[0])?)?;
            let target = format!("{}/{name}", self.path);
            std::fs::write(&target, contents(file[1].as_u64().ok_or("content")?))?;
            let mtime =
                SystemTime::UNIX_EPOCH + Duration::from_secs(file[2].as_u64().ok_or("mtime")?);
            std::fs::File::options()
                .write(true)
                .open(&target)?
                .set_modified(mtime)?;
        }
        Ok(())
    }

    /// The recorder's `$normalize`: the directory is `{dir}`.
    fn normalize(&self, text: &[u8]) -> Vec<u8> {
        let mut out = String::from_utf8_lossy(text).into_owned();
        let encoded = String::from_utf8_lossy(&rawurlencode(self.path.as_bytes())).into_owned();
        out = out.replace(&self.path, "{dir}").replace(&encoded, "{dir}");
        out.into_bytes()
    }
}

impl Drop for Dir {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.path);
    }
}

fn thrown(error: &Throwable) -> (String, Vec<u8>) {
    if let Some(error) = error.downcast_ref::<Error>() {
        return (
            error.class().name().to_owned(),
            error.message().as_bytes().to_vec(),
        );
    }
    ("unknown".to_owned(), error.to_string().into_bytes())
}

fn same_thrown(dir: &Dir, recorded: &Json, class: &str, message: &[u8]) -> Result<bool, TestError> {
    Ok(recorded[0] == "throws"
        && recorded[1] == class
        && bytes(&recorded[2])? == dir.normalize(message))
}

fn headers_value(dir: &Dir, response: &Response) -> Value {
    let mut headers = PhpArray::new();
    for (name, values) in response.headers().all_preserve_case().iter() {
        headers.insert(
            name.clone(),
            Value::Array(PhpArray::list(values.iter().map(|v| {
                v.clone().map_or(Value::Null, |v| {
                    Value::String(PhpString::from(dir.normalize(v.as_bytes())))
                })
            }))),
        );
    }
    Value::Array(headers)
}

fn binary(
    response: &mut Response,
) -> Result<&mut symfony_http_foundation::binary_file_response::BinaryFile, TestError> {
    Ok(response.binary_file_mut().ok_or("not binary")?)
}

fn op_result(
    dir: &Dir,
    response: &mut Response,
    op: &Json,
) -> Result<Result<Json, (String, Vec<u8>)>, TestError> {
    let err = |e: Error| (e.class().name().to_owned(), e.message().as_bytes().to_vec());
    let this = json!(["self"]);
    let _ = dir;
    Ok(match text(&op[0])? {
        "disposition" => response
            .set_content_disposition(text(&op[1])?.as_bytes(), &bytes(&op[2])?, &bytes(&op[3])?)
            .map(|_| this)
            .map_err(err),
        "chunk" => binary(response)?
            .set_chunk_size(op[1].as_i64().ok_or("chunk")?)
            .map(|_| this)
            .map_err(err),
        "delete" => {
            let binary = binary(response)?;
            binary.delete_file_after_send(op[1].as_bool().ok_or("delete")?);
            Ok(json!(["bool", binary.should_delete_file_after_send()]))
        }
        "etag" => response.set_auto_etag().map(|_| this).map_err(err),
        "lm" => response.set_auto_last_modified().map(|_| this).map_err(err),
        "set_content" => response
            .set_content(op[1].as_str().map(PhpString::from))
            .map(|_| this)
            .map_err(err),
        "get_content" => Ok(json!(["bool", false])).and_then(|v| {
            if response.get_content().is_some() {
                Err(("string".to_owned(), Vec::new()))
            } else {
                Ok(v)
            }
        }),
        other => return Err(format!("unknown op {other}").into()),
    })
}

fn same_result(
    dir: &Dir,
    recorded: &Json,
    got: &Result<Json, (String, Vec<u8>)>,
) -> Result<bool, TestError> {
    Ok(match got {
        Err((class, message)) => same_thrown(dir, recorded, class, message)?,
        Ok(value) if *value == json!(["self"]) => recorded == value,
        // shouldDeleteFileAfterSend(); getContent() is false (or a string)
        Ok(value) => match value[1].as_bool() {
            Some(b) if recorded[0] != "throws" => {
                vectors::decode(recorded)?.identical(&Value::Bool(b))
            }
            _ => false,
        },
    })
}

#[test]
#[allow(clippy::too_many_lines, reason = "one replay, in the recorder's order")]
fn binary_file_responses_answer_as_php() -> Result<(), TestError> {
    let data: Json = serde_json::from_str(DATA)?;
    let scripts = items(&data["scripts"])?;
    assert_eq!(1500, scripts.len());
    let _clock = clock::mock(1_700_000_000);
    let dir = Dir::new()?;
    let mut failures = Vec::new();
    let mut answers = 0;

    for (n, script) in scripts.iter().enumerate() {
        set_globals(Globals::default());
        dir.fill(items(&script["files"])?)?;
        if script["trust"] == true {
            trust_x_sendfile_type_header();
        } else {
            distrust_x_sendfile_type_header();
        }
        let aborted = script["aborted"] == true;

        let name = bytes(&script["name"])?;
        let path = dir.file(&name);
        let file = match text(&script["kind"])? {
            "path" => FileArgument::Path(path.clone()),
            "file" => match File::new(path.clone(), true) {
                Ok(file) => FileArgument::File(Arc::new(file)),
                Err(e) => return Err(format!("script {n}: {e}").into()),
            },
            "stream" => {
                let stream: Arc<dyn FileLike> = Arc::new(Stream::new(path.clone(), true)?);
                FileArgument::File(stream)
            }
            "temp" => {
                let temp = TempFile::default();
                temp.write(&contents(script["temp"].as_u64().ok_or("temp")?));
                FileArgument::Temp(temp)
            }
            "url" => FileArgument::Path([&b"file://"[..], &path].concat()),
            _ => FileArgument::Path(dir.file(b"missing.txt")),
        };
        // PHP's array: a key given twice keeps its first position and its
        // last value.
        let mut headers: Vec<(String, String)> = Vec::new();
        for pair in items(&script["headers_in"])? {
            let (key, value) = (text(&pair[0])?.to_owned(), text(&pair[1])?.to_owned());
            match headers.iter_mut().find(|(k, _)| *k == key) {
                Some(entry) => entry.1 = value,
                None => headers.push((key, value)),
            }
        }

        answers += 1;
        let created = binary_file_response::new(
            file,
            script["status"].as_i64().ok_or("status")?,
            headers
                .iter()
                .map(|(k, v)| (k.as_bytes(), HeaderValues::from(v.as_str()))),
            script["public"] == true,
            script["disposition"].as_str().map(str::as_bytes),
            script["auto_etag"] == true,
            script["auto_last_modified"] == true,
        );
        let mut response = match (created, &script["new"]) {
            (Ok(response), Json::Null) => response,
            (Err(e), recorded)
                if same_thrown(&dir, recorded, e.class().name(), e.message().as_bytes())? =>
            {
                continue;
            }
            (got, recorded) => {
                failures.push(format!(
                    "script {n} new: expected {recorded}, got {:?}",
                    got.err()
                ));
                continue;
            }
        };

        for (i, (op, recorded)) in items(&script["ops"])?
            .iter()
            .zip(items(&script["results"])?)
            .enumerate()
        {
            answers += 1;
            let got = op_result(&dir, &mut response, op)?;
            if !same_result(&dir, recorded, &got)? {
                failures.push(format!(
                    "script {n} op {i} {op}: expected {recorded}, got {got:?}"
                ));
            }
        }

        let mut request = Request::create_with(
            "/",
            Create {
                method: Some(PhpString::from(text(&script["method"])?)),
                ..Create::default()
            },
        )?;
        let etag = response.get_etag().unwrap_or_default();
        let last_modified = response
            .headers()
            .get("Last-Modified", None)
            .unwrap_or_default();
        for pair in items(&script["request_headers"])? {
            let value = String::from_utf8(bytes(&pair[1])?)?
                .replace("{etag}", &String::from_utf8_lossy(etag.as_bytes()))
                .replace("{lm}", &String::from_utf8_lossy(last_modified.as_bytes()))
                .replace("{dir}", &dir.path);
            request
                .headers_mut()
                .set(text(&pair[0])?.as_bytes(), value.as_str(), true)?;
        }

        let sends = script["sends"].as_u64().ok_or("sends")?;
        if sends > 0 {
            answers += 1;
            let got = response
                .prepare(&mut request)
                .map(|_| json!(["self"]))
                .map_err(|e| (e.class().name().to_owned(), e.message().as_bytes().to_vec()));
            if !same_result(&dir, &script["prepared"], &got)? {
                failures.push(format!(
                    "script {n} prepare: expected {}, got {got:?}",
                    script["prepared"]
                ));
            }
        }

        for (i, recorded) in items(&script["sent"])?.iter().enumerate() {
            answers += 1;
            with_output(|o| {
                o.calls.clear();
                o.connection_aborted = aborted;
            });
            let got = response
                .send_content()
                .map(|_| json!(["self"]))
                .map_err(|e| thrown(&e));
            let chunks: Vec<Vec<u8>> = with_output(|o| {
                o.calls
                    .iter()
                    .filter_map(|call| match call {
                        OutputCall::Echo(text) if !text.as_bytes().is_empty() => {
                            Some(text.as_bytes().to_vec())
                        }
                        _ => None,
                    })
                    .collect()
            });
            let lengths: Vec<usize> = chunks.iter().map(Vec::len).collect();
            let crc = format!("{:08x}", crc32(&chunks.concat()));
            let expected_lengths: Vec<usize> = items(&recorded[1])?
                .iter()
                .map(|l| {
                    l.as_u64()
                        .and_then(|l| usize::try_from(l).ok())
                        .ok_or("length")
                })
                .collect::<Result<_, _>>()?;
            if !same_result(&dir, &recorded[0], &got)?
                || lengths != expected_lengths
                || crc != text(&recorded[2])?
            {
                failures.push(format!(
                    "script {n} send {i}: expected {} {:?} {}, got {got:?} {lengths:?} {crc}",
                    recorded[0], expected_lengths, recorded[2]
                ));
            }
        }

        answers += 4;
        if Some(response.get_status_code()) != script["status_out"].as_i64() {
            failures.push(format!(
                "script {n} status: expected {}, got {}",
                script["status_out"],
                response.get_status_code()
            ));
        }
        if !vectors::decode(&script["headers"])?.identical(&headers_value(&dir, &response)) {
            failures.push(format!(
                "script {n} headers: got {:?}",
                response.headers().all_preserve_case()
            ));
        }
        let binary = response.binary_file().ok_or("not binary")?;
        let pathname = binary.get_file().get_pathname().to_vec();
        if dir.normalize(&pathname) != bytes(&script["pathname"])? {
            failures.push(format!(
                "script {n} pathname: got {}",
                String::from_utf8_lossy(&pathname)
            ));
        }
        if let Some(exists) = script["exists"].as_bool()
            && exists != symfony_finder::fs::is_file(&pathname)
        {
            failures.push(format!("script {n} exists: expected {exists}"));
        }
    }
    distrust_x_sendfile_type_header();
    let total = failures.len();
    failures.truncate(15);
    assert!(
        failures.is_empty(),
        "{total} of {answers} answers differ:\n{}",
        failures.join("\n")
    );
    eprintln!("{answers} answers");
    Ok(())
}
