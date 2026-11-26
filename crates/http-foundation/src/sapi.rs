//! PHP-FPM, the SAPI a Symfony front controller runs under: what PHP makes
//! of one `FastCGI` request (the superglobals, `php://input`, the uploaded
//! files, `request_parse_body()`'s answer) and what it sends back for the
//! output calls the script made (`header()`, `header_remove()`,
//! `http_response_code()`, `setcookie()`, `echo`, output buffers,
//! `fastcgi_finish_request()`), as the CGI response PHP-FPM writes.
//!
//! Written from the PHP manual and from PHP 8.4.27's answers in the
//! oracle's FPM image, which has no php.ini (`tests/data/sapi.json`,
//! recorded by `scripts/record.sh runtime-fpm` from
//! `oracle/recorders/runtime_fpm.php`); no PHP source code is used (PLAN.md
//! decision 8). [`Ini`] holds the settings that matter, at PHP's defaults.
//!
//! Requests ([`start`]):
//!
//! - `$_SERVER` holds the process environment PHP-FPM passes on, in order,
//!   then the `FastCGI` parameters, last first; a parameter sent twice keeps
//!   the place of its first sending and the later value; a parameter that
//!   names an environment variable replaces it in place. PHP then sets
//!   `FCGI_ROLE`, `PHP_SELF` (`SCRIPT_NAME` with `PATH_INFO`), the
//!   `PHP_AUTH_*` entries of an `Authorization` header (`Basic`, decoded
//!   as `base64_decode()` does without `strict`: the user always, the
//!   password when not empty; `Digest`: the rest), `REQUEST_TIME_FLOAT`,
//!   `REQUEST_TIME`, and `argv` / `argc` (`QUERY_STRING` split at `+`;
//!   `register_argc_argv` is on without a php.ini).
//! - `$_GET` from `QUERY_STRING`, `$_COOKIE` from `HTTP_COOKIE`, read as
//!   `query` and `cookies` read them.
//! - `$_POST` and `$_FILES` only for `REQUEST_METHOD` `POST` exactly, a
//!   form or multipart body, and a `CONTENT_LENGTH` within `post_max_size`
//!   (beyond it both are empty and PHP warns). `php://input` is the body,
//!   except for a multipart POST, which leaves it empty.
//! - For PUT, DELETE, PATCH and QUERY, `request_parse_body()` reads a form
//!   or multipart body, answers `[[], []]` with a warning beyond
//!   `post_max_size`, and throws for any other content type and for a
//!   multipart body PHP warned about (which [`Globals::parsed_body`] holds
//!   as `None`). Reading a multipart body empties `php://input`, unless
//!   the boundary is missing; the parse is done at startup, as Symfony's
//!   `createFromGlobals()` always calls `request_parse_body()` for these
//!   methods.
//! - `$_FILES`: each file part gives `name` (the filename after its last
//!   `/` or `\`), `full_path`, `type` (the part's `Content-Type` up to a
//!   `;`), `tmp_name`, `error` and `size`, stored as PHP stores input
//!   variables under `name[name]`, `name[full_path]`, ... (keys inside
//!   brackets lose their leading spaces). An empty filename is
//!   `UPLOAD_ERR_NO_FILE`; a file over `upload_max_filesize` is
//!   `UPLOAD_ERR_INI_SIZE`, one over the last `MAX_FILE_SIZE` field before
//!   it (`intval()`, when not 0) `UPLOAD_ERR_FORM_SIZE`, one the body cuts
//!   off `UPLOAD_ERR_PARTIAL`; these have an empty `type`. A part without
//!   `name` is stored under the next of its own counter (0, 1, ...). A
//!   name with a bracket that does not form `base[...][...]` stops the
//!   reading of files, the fields still being read. A file is written to
//!   `upload_tmp_dir` as `php` and 19 letters or digits, marked as
//!   uploaded (`crate::php::register_uploaded_file`), and removed by
//!   [`finish`] unless it was moved.
//! - `$_REQUEST` merges `$_GET`, `$_POST` and `$_COOKIE` in
//!   `variables_order` order (`request_order` is empty): a key keeps its
//!   first place, arrays merge key by key, anything else replaces.
//!
//! Responses ([`Sapi`]):
//!
//! - `header()` ignores an empty line and drops trailing spaces, tabs, CR
//!   and LF (a line of only those is listed, empty, and not sent); refuses (with PHP's warning) a line with CR or LF inside
//!   or a NUL byte, and any call once the headers are sent. A line
//!   starting `HTTP/` (any case) is the status line when it has a space:
//!   the status is `intval()` of what follows, which PHP-FPM sends as
//!   `Status:`; the `$response_code` argument is then unused. Other lines
//!   are kept as given, `Name: value` or not; `$replace` removes the
//!   earlier lines with the same name (any case) first; a
//!   `$response_code` other than 0 and the current status sets the status
//!   and forgets the status line (`http_response_code()` keeps it).
//!   `Location` sets 302
//!   unless the status is 201 or 3xx, `WWW-Authenticate` sets 401; both
//!   forget the status line. A `Content-Type` starting `text/` without
//!   `charset=` becomes `Content-type: <type>;charset=<default_charset>`,
//!   and any `Content-Type` stops the default one.
//! - `header_remove()` matches names in any case without their trailing
//!   spaces; a name with `:` warns, one with CR or LF does nothing.
//! - Output reaches PHP-FPM when no buffer holds it; the first output, a
//!   `flush()`, `fastcgi_finish_request()` or the end of the script sends
//!   the headers, `Content-type: <default_mimetype>` (with `;
//!   charset=<default_charset>` for a `text/` type) being listed last
//!   unless a `Content-Type` was set or `default_mimetype` is empty. PHP-FPM
//!   writes a status other than 200 first, as `Status: ` and the status
//!   line or the code and its reason, unless (without a status line) a
//!   `Status:` header with a value gives it; it writes only the first such
//!   header, and none when it wrote its own. A 304 is written without a
//!   content type. A HEAD request gets no body. After
//!   `fastcgi_finish_request()` output is dropped, and `flush()` finds the
//!   client gone, which ends the script. An uncaught error makes a 200 a
//!   500.
//! - `setcookie()` / `setrawcookie()` (`expires` 0 only) add a
//!   `Set-Cookie` line without replacing, the value `rawurlencode()`d by
//!   `setcookie()`, an empty value as PHP's deletion cookie; once headers
//!   are sent they answer `false`.
//! - `ini_set('default_charset')` warns twice for an encoding PHP does not
//!   know, and sets it.
//!
//! What PHP would display or log (warnings) is returned to the caller in
//! [`Sapi::warnings`] and [`Startup::warnings`], never written to the
//! body: `display_errors` is off, as in production. A warning's "output
//! started at" file and line are not known here.
//!
//! Upstream: none

use std::{
    collections::hash_map::RandomState,
    hash::{BuildHasher, Hasher},
    io::Write as _,
    path::{Path, PathBuf},
    sync::{
        Arc,
        atomic::{AtomicU64, Ordering},
    },
};

use php_value::{
    Key, PhpArray, PhpString, Value, base64,
    path::from_path,
    url::rawurlencode,
    vars::{self, Duplicate},
};

use crate::{
    BodyFormat,
    cookies::Cookies,
    multipart::{self, FilePart},
    php::{
        FinishRequest, Globals, Output, OutputCall, UPLOAD_ERR_FORM_SIZE, UPLOAD_ERR_INI_SIZE,
        UPLOAD_ERR_NO_FILE, UPLOAD_ERR_OK, UPLOAD_ERR_PARTIAL, register_uploaded_file,
    },
    query::Query,
    response::STATUS_TEXTS,
    trim_ows,
};

/// The php.ini settings PHP-FPM's behaviour here depends on.
#[derive(Debug, Clone)]
pub struct Ini {
    /// `post_max_size`, in bytes (8M).
    pub post_max_size: u64,
    /// `upload_max_filesize`, in bytes (2M).
    pub upload_max_filesize: u64,
    /// `upload_tmp_dir` (the system's temporary directory).
    pub upload_tmp_dir: PathBuf,
    /// `default_mimetype` (`text/html`).
    pub default_mimetype: PhpString,
    /// `default_charset` (`UTF-8`).
    pub default_charset: PhpString,
    /// `expose_php`: PHP's default is on, which adds `X-Powered-By:
    /// PHP/<version>`; `None` here, as the adapter runs no PHP.
    pub x_powered_by: Option<PhpString>,
    /// `variables_order` (`EGPCS`).
    pub variables_order: PhpString,
    /// `request_order` (empty).
    pub request_order: PhpString,
}

impl Default for Ini {
    fn default() -> Self {
        Self {
            post_max_size: 8 * 1024 * 1024,
            upload_max_filesize: 2 * 1024 * 1024,
            upload_tmp_dir: std::env::temp_dir(),
            default_mimetype: PhpString::from("text/html"),
            default_charset: PhpString::from("UTF-8"),
            x_powered_by: None,
            variables_order: PhpString::from("EGPCS"),
            request_order: PhpString::default(),
        }
    }
}

/// One `FastCGI` request as PHP-FPM receives it.
#[derive(Debug, Clone, Default)]
pub struct CgiRequest {
    /// The environment PHP-FPM passes on (`clear_env = no`), in order.
    pub environment: Vec<(PhpString, PhpString)>,
    /// The `FastCGI` parameters, in the order they were sent.
    pub params: Vec<(PhpString, PhpString)>,
    /// The body (`FastCGI` stdin).
    pub stdin: Vec<u8>,
    /// When the request started (`microtime(true)`).
    pub time: f64,
}

/// A started request: its superglobals, and what PHP warned about.
#[derive(Debug)]
pub struct Startup {
    pub globals: Globals,
    pub warnings: Vec<String>,
    /// The uploaded files written for the request, for [`finish`].
    pub uploaded: Vec<PathBuf>,
}

/// Builds what PHP-FPM gives the script for `request`, writing uploaded
/// files to `ini.upload_tmp_dir`.
#[expect(clippy::too_many_lines, reason = "PHP-FPM's startup steps in order")]
pub fn start(request: &CgiRequest, ini: &Ini) -> std::io::Result<Startup> {
    let mut warnings = Vec::new();
    let mut uploaded = Vec::new();
    let server = server(request);
    let param = |name: &str| -> PhpString {
        match server.get(&Key::from(name)) {
            Some(Value::String(s)) => s.clone(),
            _ => PhpString::default(),
        }
    };
    let query_string = param("QUERY_STRING");
    let method = param("REQUEST_METHOD");
    let content_type = param("CONTENT_TYPE");
    let content_length = leading_int(param("CONTENT_LENGTH").as_bytes());
    let too_large = u64::try_from(content_length).is_ok_and(|n| n > ini.post_max_size);
    let size_warning = |caller: &str| {
        format!(
            "{caller}: POST Content-Length of {content_length} bytes exceeds the limit of {} bytes",
            ini.post_max_size
        )
    };

    let get = Query::parse_bytes(query_string.as_bytes()).vars().clone();
    let cookie = Cookies::vars(param("HTTP_COOKIE").as_bytes());
    let format = BodyFormat::of(trim_ows(content_type.as_bytes()));

    let mut post = PhpArray::new();
    let mut files = PhpArray::new();
    let mut input: Arc<[u8]> = Arc::from(request.stdin.as_slice());
    if method.as_bytes() == b"POST" {
        if too_large {
            warnings.push(size_warning("PHP Request Startup"));
        } else {
            match format {
                Some(BodyFormat::UrlEncoded) => {
                    post = Query::parse_body(&request.stdin).vars().clone();
                }
                Some(BodyFormat::Multipart) => {
                    let parsed =
                        multipart::parse(trim_ows(content_type.as_bytes()), &request.stdin);
                    warnings.extend(
                        parsed
                            .warnings
                            .iter()
                            .map(|w| format!("PHP Request Startup: {}", w.text())),
                    );
                    post = Query::from_fields(&parsed.fields).vars().clone();
                    files = uploaded_files(&parsed.files, ini, &mut uploaded)?;
                    input = Arc::from(&b""[..]);
                }
                None => {}
            }
        }
    }

    let parsed_body = if matches!(method.as_bytes(), b"PUT" | b"DELETE" | b"PATCH" | b"QUERY") {
        if too_large {
            warnings.push(size_warning("request_parse_body()"));
            Some((PhpArray::new(), PhpArray::new()))
        } else {
            match format {
                Some(BodyFormat::UrlEncoded) => Some((
                    Query::parse_body(&request.stdin).vars().clone(),
                    PhpArray::new(),
                )),
                Some(BodyFormat::Multipart) => {
                    let parsed =
                        multipart::parse(trim_ows(content_type.as_bytes()), &request.stdin);
                    // Reading the parts consumes php://input; a missing
                    // boundary stops PHP before it reads.
                    if !parsed
                        .warnings
                        .contains(&multipart::Warning::MissingBoundary)
                    {
                        input = Arc::from(&b""[..]);
                    }
                    if parsed.failed {
                        None
                    } else {
                        let files = uploaded_files(&parsed.files, ini, &mut uploaded)?;
                        Some((Query::from_fields(&parsed.fields).vars().clone(), files))
                    }
                }
                None => None,
            }
        }
    } else {
        None
    };

    let mut superglobals = PhpArray::new();
    for letter in ini.variables_order.as_bytes() {
        let source = match letter.to_ascii_uppercase() {
            b'G' => &get,
            b'P' => &post,
            b'C' => &cookie,
            _ => continue,
        };
        merge(&mut superglobals, source);
    }

    let globals = Globals {
        get,
        post,
        cookie,
        files,
        server,
        request: superglobals,
        input,
        parsed_body,
        request_order: ini.request_order.clone(),
        variables_order: ini.variables_order.clone(),
        output: Output {
            sapi: PhpString::from("fpm-fcgi"),
            finish_request: FinishRequest::FastCgi,
            default_mimetype: ini.default_mimetype.clone(),
            ..Output::default()
        },
        session: crate::php_session::SessionState::default(),
    };
    Ok(Startup {
        globals,
        warnings,
        uploaded,
    })
}

/// Ends the request: the uploaded files that were not moved are removed,
/// and none counts as uploaded any more.
pub fn finish(uploaded: &[PathBuf]) {
    for path in uploaded {
        if crate::php::is_uploaded_file(&from_path(path)) {
            // A file the script deleted itself is already gone.
            let _ = std::fs::remove_file(path);
        }
    }
    crate::php::forget_uploaded_files();
}

/// `$_SERVER`.
fn server(request: &CgiRequest) -> PhpArray {
    let mut server = PhpArray::new();
    for (name, value) in &request.environment {
        server.insert(
            Key::from_bytes(name.as_bytes()),
            Value::String(value.clone()),
        );
    }
    // PHP-FPM keeps one entry per name, in the order first sent, with the
    // last value; PHP registers them last first.
    let mut params: Vec<(&PhpString, &PhpString)> = Vec::new();
    for (name, value) in &request.params {
        match params.iter_mut().find(|(n, _)| *n == name) {
            Some(entry) => entry.1 = value,
            None => params.push((name, value)),
        }
    }
    for (name, value) in params.iter().rev() {
        server.insert(
            Key::from_bytes(name.as_bytes()),
            Value::String((*value).clone()),
        );
    }
    let text = |server: &PhpArray, name: &str| match server.get(&Key::from(name)) {
        Some(Value::String(s)) => s.as_bytes().to_vec(),
        _ => Vec::new(),
    };
    server.insert("FCGI_ROLE", Value::string("RESPONDER"));
    let mut php_self = text(&server, "SCRIPT_NAME");
    php_self.extend_from_slice(&text(&server, "PATH_INFO"));
    server.insert("PHP_SELF", Value::String(PhpString::from(php_self)));
    let authorization = text(&server, "HTTP_AUTHORIZATION");
    if authorization.len() > 6 && authorization[..6].eq_ignore_ascii_case(b"basic ") {
        let decoded = base64::decode(&authorization[6..]);
        if let Some(colon) = decoded.iter().position(|&b| b == b':') {
            server.insert(
                "PHP_AUTH_USER",
                Value::String(PhpString::from(&decoded[..colon])),
            );
            if colon + 1 < decoded.len() {
                server.insert(
                    "PHP_AUTH_PW",
                    Value::String(PhpString::from(&decoded[colon + 1..])),
                );
            }
        }
    } else if authorization.len() > 7 && authorization[..7].eq_ignore_ascii_case(b"digest ") {
        server.insert(
            "PHP_AUTH_DIGEST",
            Value::String(PhpString::from(&authorization[7..])),
        );
    }
    server.insert("REQUEST_TIME_FLOAT", Value::Float(request.time));
    #[expect(
        clippy::cast_possible_truncation,
        reason = "PHP's (int) of the request time, which fits"
    )]
    server.insert("REQUEST_TIME", Value::Int(request.time.floor() as i64));
    let query = text(&server, "QUERY_STRING");
    let argv: Vec<Value> = if query.is_empty() {
        Vec::new()
    } else {
        query
            .split(|&b| b == b'+')
            .map(|part| Value::String(PhpString::from(part)))
            .collect()
    };
    let arg_count = i64::try_from(argv.len()).unwrap_or(i64::MAX);
    server.insert("argv", Value::Array(PhpArray::list(argv)));
    server.insert("argc", Value::Int(arg_count));
    server
}

/// `intval()` of a numeric prefix: leading spaces, a sign and digits,
/// saturating; 0 without digits.
fn leading_int(text: &[u8]) -> i64 {
    let text = &text[text
        .iter()
        .position(|b| !matches!(b, b' ' | b'\t' | b'\n' | b'\r' | 0x0b | 0x0c))
        .unwrap_or(text.len())..];
    let (negative, digits) = match text.first() {
        Some(b'-') => (true, &text[1..]),
        Some(b'+') => (false, &text[1..]),
        _ => (false, text),
    };
    let mut value: i64 = 0;
    for &b in digits.iter().take_while(|b| b.is_ascii_digit()) {
        let digit = i64::from(b - b'0');
        value = match value.checked_mul(10).and_then(|v| {
            if negative {
                v.checked_sub(digit)
            } else {
                v.checked_add(digit)
            }
        }) {
            Some(v) => v,
            None => return if negative { i64::MIN } else { i64::MAX },
        };
    }
    value
}

/// `$_REQUEST`'s merge of one superglobal.
fn merge(into: &mut PhpArray, from: &PhpArray) {
    for (key, value) in from.iter() {
        match (into.get_mut(key), value) {
            (Some(Value::Array(existing)), Value::Array(more)) => merge(existing, more),
            _ => into.insert(key.clone(), value.clone()),
        }
    }
}

/// Whether a file field name with brackets fails to be `base[..][..]`.
fn malformed(name: &[u8]) -> bool {
    let Some(open) = name.iter().position(|&b| b == b'[') else {
        return name.contains(&b']');
    };
    if name[..open].contains(&b']') {
        return true;
    }
    let mut rest = &name[open..];
    while let Some(inside) = rest.strip_prefix(b"[") {
        let Some(close) = inside.iter().position(|&b| b == b']') else {
            return true;
        };
        if inside[..close].contains(&b'[') {
            return true;
        }
        rest = &inside[close + 1..];
    }
    !rest.is_empty()
}

/// The registration name of one `$_FILES` entry: `base[entry][...]`, the
/// keys inside brackets without their leading whitespace.
fn entry_name(name: &[u8], entry: &str) -> Vec<u8> {
    let open = name.iter().position(|&b| b == b'[').unwrap_or(name.len());
    let mut out = name[..open].to_vec();
    out.push(b'[');
    out.extend_from_slice(entry.as_bytes());
    out.push(b']');
    let mut rest = &name[open..];
    while let Some(inside) = rest.strip_prefix(b"[") {
        let Some(close) = inside.iter().position(|&b| b == b']') else {
            break;
        };
        let key = &inside[..close];
        let start = key
            .iter()
            .position(|b| !b.is_ascii_whitespace())
            .unwrap_or(key.len());
        out.push(b'[');
        out.extend_from_slice(&key[start..]);
        out.push(b']');
        rest = &inside[close + 1..];
    }
    out
}

/// `$_FILES` for the file parts of a body.
fn uploaded_files(
    parts: &[FilePart],
    ini: &Ini,
    uploaded: &mut Vec<PathBuf>,
) -> std::io::Result<PhpArray> {
    let mut files = PhpArray::new();
    let mut nameless = 0_i64;
    for part in parts {
        let name = match &part.name {
            Some(name) if malformed(name) => break,
            Some(name) => name.clone(),
            None => {
                let name = nameless.to_string().into_bytes();
                nameless += 1;
                name
            }
        };
        let size = part.data.len() as u64;
        let limit = part.max_file_size.as_deref().map_or(0, leading_int);
        let error = if part.filename.is_empty() {
            UPLOAD_ERR_NO_FILE
        } else if size > ini.upload_max_filesize {
            UPLOAD_ERR_INI_SIZE
        } else if limit != 0 && i64::try_from(size).map_or(true, |size| size > limit) {
            UPLOAD_ERR_FORM_SIZE
        } else if !part.complete {
            UPLOAD_ERR_PARTIAL
        } else {
            UPLOAD_ERR_OK
        };
        let (base, full_path) = if part.filename.is_empty() {
            (Vec::new(), Vec::new())
        } else {
            let at = part
                .filename
                .iter()
                .rposition(|&b| b == b'/' || b == b'\\')
                .map_or(0, |at| at + 1);
            (part.filename[at..].to_vec(), part.filename.clone())
        };
        let (kind, tmp_name, size) = if error == UPLOAD_ERR_OK {
            let path = write_upload(&ini.upload_tmp_dir, &part.data)?;
            let bytes = from_path(&path);
            register_uploaded_file(bytes.clone());
            uploaded.push(path);
            let kind = part.content_type.as_deref().map_or(&b""[..], |t| {
                &t[..t.iter().position(|&b| b == b';').unwrap_or(t.len())]
            });
            (
                kind.to_vec(),
                bytes,
                i64::try_from(size).unwrap_or(i64::MAX),
            )
        } else {
            (Vec::new(), Vec::new(), 0)
        };
        for (entry, value) in [
            ("name", Value::String(PhpString::from(base.clone()))),
            (
                "full_path",
                Value::String(PhpString::from(full_path.clone())),
            ),
            ("type", Value::String(PhpString::from(kind))),
            ("tmp_name", Value::String(PhpString::from(tmp_name))),
            ("error", Value::Int(error)),
            ("size", Value::Int(size)),
        ] {
            vars::register_value(
                &mut files,
                &entry_name(&name, entry),
                value,
                Duplicate::Replace,
            );
        }
    }
    Ok(files)
}

static UPLOADS: AtomicU64 = AtomicU64::new(0);

/// Writes an uploaded file as `php` and 19 letters or digits in `dir`,
/// never over an existing file.
fn write_upload(dir: &Path, data: &[u8]) -> std::io::Result<PathBuf> {
    const ALPHABET: &[u8] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789";
    let state = RandomState::new();
    for _ in 0..16 {
        let mut name = String::from("php");
        let mut hasher = state.build_hasher();
        hasher.write_u64(UPLOADS.fetch_add(1, Ordering::Relaxed));
        let mut bits = u128::from(hasher.finish()) << 64;
        hasher.write_u8(1);
        bits |= u128::from(hasher.finish());
        for _ in 0..19 {
            let index = usize::try_from(bits % 62).unwrap_or(0);
            name.push(char::from(ALPHABET[index]));
            bits /= 62;
        }
        let path = dir.join(name);
        match std::fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&path)
        {
            Ok(mut file) => {
                file.write_all(data)?;
                return Ok(path);
            }
            Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => {}
            Err(e) => return Err(e),
        }
    }
    Err(std::io::Error::new(
        std::io::ErrorKind::AlreadyExists,
        "no free name for an uploaded file",
    ))
}

/// What PHP-FPM sends back: the status (from its `Status:` line, 200
/// without one), the header lines in order (its `Status:` line first when
/// it sends one), and the body.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct CgiResponse {
    pub headers: Vec<PhpString>,
    pub body: Vec<u8>,
}

impl CgiResponse {
    /// The CGI response as PHP-FPM writes it to the `FastCGI` stdout stream.
    pub fn to_bytes(&self) -> Vec<u8> {
        let mut out = Vec::new();
        for line in &self.headers {
            out.extend_from_slice(line.as_bytes());
            out.extend_from_slice(b"\r\n");
        }
        out.extend_from_slice(b"\r\n");
        out.extend_from_slice(&self.body);
        out
    }
}

/// `setcookie()`'s and `setrawcookie()`'s arguments.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct SetCookie {
    pub name: PhpString,
    pub value: PhpString,
    pub expires: i64,
    pub path: PhpString,
    pub domain: PhpString,
    pub secure: bool,
    pub httponly: bool,
    /// The `samesite` option (`''` for none).
    pub samesite: PhpString,
}

/// The `\ValueError` or refusal a cookie function raises.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CookieError(pub String);

impl std::fmt::Display for CookieError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}

impl std::error::Error for CookieError {}

/// The SAPI's side of one request's output.
#[expect(clippy::struct_excessive_bools, reason = "the SAPI's own flags")]
#[derive(Debug, Clone)]
pub struct Sapi {
    head: bool,
    mimetype: PhpString,
    charset: PhpString,
    lines: Vec<PhpString>,
    code: i64,
    status_line: Option<PhpString>,
    content_type_set: bool,
    sent: Option<Vec<PhpString>>,
    finished: bool,
    aborted: bool,
    buffers: Vec<Vec<u8>>,
    body: Vec<u8>,
    warnings: Vec<String>,
}

/// A line's header name: up to its `:`, the whole line without one.
fn line_name(line: &[u8]) -> &[u8] {
    &line[..line.iter().position(|&b| b == b':').unwrap_or(line.len())]
}

/// PHP's reason phrases for the `Status:` line it writes from a code
/// alone: Symfony's table without 418 (recorded: `Status: 418`); codes
/// the recordings do not show follow Symfony's table.
fn reason(code: i64) -> Option<&'static str> {
    if code == 418 {
        return None;
    }
    STATUS_TEXTS
        .iter()
        .find(|(c, _)| *c == code)
        .map(|(_, reason)| *reason)
}

fn trim_end(bytes: &[u8]) -> &[u8] {
    let end = bytes
        .iter()
        .rposition(|b| !matches!(b, b' ' | b'\t'))
        .map_or(0, |end| end + 1);
    &bytes[..end]
}

fn concat(parts: &[&[u8]]) -> PhpString {
    PhpString::from(parts.concat())
}

const SENT: &str = "Cannot modify header information - headers already sent";

/// Whether `default_charset` names an encoding PHP knows. Only the
/// encodings the recordings set are pinned (`UTF-8`, `ISO-8859-1`; a MIME
/// type is unknown); the others are the common names from the manual's
/// list of supported character sets.
fn known_encoding(name: &[u8]) -> bool {
    const KNOWN: [&[u8]; 17] = [
        b"UTF-8",
        b"ISO-8859-1",
        b"ISO8859-1",
        b"ISO-8859-5",
        b"ISO-8859-15",
        b"ISO8859-15",
        b"cp866",
        b"cp1251",
        b"cp1252",
        b"Windows-1251",
        b"Windows-1252",
        b"KOI8-R",
        b"BIG5",
        b"GB2312",
        b"BIG5-HKSCS",
        b"Shift_JIS",
        b"EUC-JP",
    ];
    KNOWN.iter().any(|known| known.eq_ignore_ascii_case(name))
}

impl Sapi {
    /// The output side of a request: no header but `X-Powered-By` (with
    /// `expose_php`), status 200; `head` for a HEAD request.
    pub fn new(ini: &Ini, head: bool) -> Self {
        Self {
            head,
            mimetype: ini.default_mimetype.clone(),
            charset: ini.default_charset.clone(),
            lines: ini
                .x_powered_by
                .iter()
                .map(|v| concat(&[b"X-Powered-By: ", v.as_bytes()]))
                .collect(),
            code: 200,
            status_line: None,
            content_type_set: false,
            sent: None,
            finished: false,
            aborted: false,
            buffers: Vec::new(),
            body: Vec::new(),
            warnings: Vec::new(),
        }
    }

    /// The warnings PHP raised, in order.
    pub fn warnings(&self) -> &[String] {
        &self.warnings
    }

    /// `headers_sent()`.
    pub fn headers_sent(&self) -> bool {
        self.sent.is_some()
    }

    /// `headers_list()`.
    pub fn headers_list(&self) -> Vec<PhpString> {
        self.sent.clone().unwrap_or_else(|| self.lines.clone())
    }

    /// `http_response_code()`: the status, `None` for PHP's `false` (0).
    pub fn response_code(&self) -> Option<i64> {
        (self.code != 0).then_some(self.code)
    }

    /// `http_response_code($code)`: the previous status (`true` where it
    /// was 0).
    pub fn set_response_code(&mut self, code: i64) -> Value {
        if self.sent.is_some() {
            self.warnings.push(
                "http_response_code(): Cannot set response code - headers already sent".to_owned(),
            );
            return Value::Bool(false);
        }
        let previous = std::mem::replace(&mut self.code, code);
        if previous == 0 {
            Value::Bool(true)
        } else {
            Value::Int(previous)
        }
    }

    /// `ini_set('default_mimetype', ...)`: the previous value.
    pub fn set_default_mimetype(&mut self, mimetype: PhpString) -> PhpString {
        std::mem::replace(&mut self.mimetype, mimetype)
    }

    /// `ini_set('default_charset', ...)`: the previous value.
    pub fn set_default_charset(&mut self, charset: PhpString) -> PhpString {
        if !charset.as_bytes().is_empty() && !known_encoding(charset.as_bytes()) {
            let name = crate::php::html_warning(&charset.to_string_lossy());
            self.warnings.push(format!(
                "ini_set(): Unknown encoding {q}{name}{q} in ini setting",
                q = "&quot;"
            ));
            self.warnings.push(format!(
                "ini_set(): INI setting contains invalid encoding {q}{name}{q}",
                q = "&quot;"
            ));
        }
        std::mem::replace(&mut self.charset, charset)
    }

    /// `header($line, $replace, $responseCode)`.
    pub fn header(&mut self, line: &[u8], replace: bool, response_code: i64) {
        if self.sent.is_some() {
            self.warnings.push(SENT.to_owned());
            return;
        }
        if line.is_empty() {
            return;
        }
        let end = line
            .iter()
            .rposition(|b| !matches!(b, b' ' | b'\t' | b'\r' | b'\n'))
            .map_or(0, |end| end + 1);
        let line = &line[..end];
        if line.iter().any(|&b| b == b'\r' || b == b'\n') {
            self.warnings.push(
                "Header may not contain more than a single header, new line detected".to_owned(),
            );
            return;
        }
        if line.contains(&0) {
            self.warnings
                .push("Header may not contain NUL bytes".to_owned());
            return;
        }
        if line.len() >= 5 && line[..5].eq_ignore_ascii_case(b"HTTP/") {
            if let Some(space) = line.iter().position(|&b| b == b' ') {
                let text = &line[space + 1..];
                self.code = leading_int(text);
                self.status_line = Some(PhpString::from(text));
            }
            return;
        }
        let name = line_name(line);
        let mut stored = PhpString::from(line);
        if name.eq_ignore_ascii_case(b"location") {
            if self.code != 201 && !(300..400).contains(&self.code) {
                self.code = 302;
                self.status_line = None;
            }
        } else if name.eq_ignore_ascii_case(b"www-authenticate") {
            self.code = 401;
            self.status_line = None;
        } else if name.eq_ignore_ascii_case(b"content-type") {
            self.content_type_set = true;
            let value = &line[line
                .iter()
                .position(|&b| b == b':')
                .map_or(line.len(), |c| c + 1)..];
            let value = &value[value
                .iter()
                .position(|b| !matches!(b, b' ' | b'\t'))
                .unwrap_or(value.len())..];
            if value.starts_with(b"text/")
                && !value.windows(8).any(|w| w == b"charset=")
                && !self.charset.as_bytes().is_empty()
            {
                stored = concat(&[
                    b"Content-type: ",
                    value,
                    b";charset=",
                    self.charset.as_bytes(),
                ]);
            }
        }
        if replace {
            let name = line_name(stored.as_bytes()).to_vec();
            self.lines
                .retain(|l| !line_name(l.as_bytes()).eq_ignore_ascii_case(&name));
        }
        self.lines.push(stored);
        if response_code != 0 && response_code != self.code {
            self.code = response_code;
            self.status_line = None;
        }
    }

    /// `header_remove($name)`; `None` removes every header.
    pub fn header_remove(&mut self, name: Option<&[u8]>) {
        if self.sent.is_some() {
            self.warnings.push(SENT.to_owned());
            return;
        }
        let Some(name) = name else {
            self.lines.clear();
            return;
        };
        if name.contains(&b':') {
            self.warnings
                .push("Header to delete may not contain colon.".to_owned());
            return;
        }
        let name = trim_end(name);
        if name.is_empty() || name.iter().any(|&b| b == b'\r' || b == b'\n') {
            return;
        }
        self.lines
            .retain(|l| !line_name(l.as_bytes()).eq_ignore_ascii_case(name));
    }

    /// The session extension's cookie: earlier `Set-Cookie` headers for
    /// `name` removed in place, then `line` added.
    pub fn session_cookie(&mut self, name: &[u8], line: &[u8]) {
        if self.sent.is_some() {
            self.warnings.push(SENT.to_owned());
            return;
        }
        let prefix = [&b"Set-Cookie: "[..], name, b"="].concat();
        self.lines.retain(|l| !l.as_bytes().starts_with(&prefix));
        self.header(line, false, 0);
    }

    /// `setcookie()` (`raw` false) or `setrawcookie()`; `expires` other
    /// than 0 is not ported.
    pub fn setcookie(&mut self, cookie: &SetCookie, raw: bool) -> Result<bool, CookieError> {
        let function = if raw { "setrawcookie" } else { "setcookie" };
        if cookie.name.as_bytes().is_empty() {
            return Err(CookieError(format!(
                "{function}(): Argument #1 ($name) cannot be empty"
            )));
        }
        if cookie
            .name
            .as_bytes()
            .iter()
            .any(|b| b"=,; \t\r\n\x0b\x0c".contains(b))
        {
            return Err(CookieError(format!(
                "{function}(): Argument #1 ($name) cannot contain \"=\", \",\", \";\", \" \", \"\\t\", \"\\r\", \"\\n\", \"\\013\", or \"\\014\""
            )));
        }
        if raw
            && cookie
                .value
                .as_bytes()
                .iter()
                .any(|b| b",; \t\r\n\x0b\x0c".contains(b))
        {
            return Err(CookieError(format!(
                "{function}(): Argument #2 ($value) cannot contain \",\", \";\", \" \", \"\\t\", \"\\r\", \"\\n\", \"\\013\", or \"\\014\""
            )));
        }
        if cookie.expires != 0 {
            return Err(CookieError(format!(
                "{function}() with an expiry time is not ported"
            )));
        }
        let mut line = concat(&[b"Set-Cookie: ", cookie.name.as_bytes(), b"="])
            .as_bytes()
            .to_vec();
        if cookie.value.as_bytes().is_empty() {
            line.extend_from_slice(b"deleted; expires=Thu, 01 Jan 1970 00:00:01 GMT; Max-Age=0");
        } else if raw {
            line.extend_from_slice(cookie.value.as_bytes());
        } else {
            line.extend_from_slice(&rawurlencode(cookie.value.as_bytes()));
        }
        for (attribute, value) in [
            (&b"; path="[..], &cookie.path),
            (b"; domain=", &cookie.domain),
        ] {
            if !value.as_bytes().is_empty() {
                line.extend_from_slice(attribute);
                line.extend_from_slice(value.as_bytes());
            }
        }
        if cookie.secure {
            line.extend_from_slice(b"; secure");
        }
        if cookie.httponly {
            line.extend_from_slice(b"; HttpOnly");
        }
        if !cookie.samesite.as_bytes().is_empty() {
            line.extend_from_slice(b"; SameSite=");
            line.extend_from_slice(cookie.samesite.as_bytes());
        }
        if self.sent.is_some() {
            self.warnings.push(SENT.to_owned());
            return Ok(false);
        }
        self.header(&line, false, 0);
        Ok(true)
    }

    /// `ob_start()`.
    pub fn ob_start(&mut self) {
        self.buffers.push(Vec::new());
    }

    /// `ob_end_flush()`: false without a buffer.
    pub fn ob_end_flush(&mut self) -> bool {
        let Some(buffer) = self.buffers.pop() else {
            self.warnings.push(
                "ob_end_flush(): Failed to delete and flush buffer. No buffer to delete or flush"
                    .to_owned(),
            );
            return false;
        };
        self.write(&buffer);
        true
    }

    /// `ob_end_clean()`: false without a buffer.
    pub fn ob_end_clean(&mut self) -> bool {
        if self.buffers.pop().is_some() {
            return true;
        }
        self.warnings
            .push("ob_end_clean(): Failed to delete buffer. No buffer to delete".to_owned());
        false
    }

    /// `@ob_flush()`: the innermost buffer's content goes to the next
    /// level and the buffer stays; without one, nothing.
    pub fn ob_flush(&mut self) {
        if let Some(buffer) = self.buffers.pop() {
            self.write(&buffer);
            self.buffers.push(Vec::new());
        }
    }

    /// `echo`.
    pub fn echo(&mut self, bytes: &[u8]) {
        self.write(bytes);
    }

    fn write(&mut self, bytes: &[u8]) {
        if bytes.is_empty() {
            return;
        }
        if let Some(buffer) = self.buffers.last_mut() {
            buffer.extend_from_slice(bytes);
            return;
        }
        self.send_headers();
        if !self.finished {
            self.body.extend_from_slice(bytes);
        }
    }

    /// `flush()`. After `fastcgi_finish_request()` it finds the client
    /// gone and PHP ends the script ([`Self::is_aborted`]).
    pub fn flush(&mut self) {
        if self.finished {
            self.aborted = true;
            return;
        }
        self.send_headers();
    }

    /// Whether PHP ended the script because the client was gone.
    pub fn is_aborted(&self) -> bool {
        self.aborted
    }

    /// `fastcgi_finish_request()`: the buffers are flushed, the response
    /// ends; false when it already ended.
    pub fn fastcgi_finish_request(&mut self) -> bool {
        if self.finished {
            return false;
        }
        self.flush_buffers();
        self.send_headers();
        self.finished = true;
        true
    }

    fn flush_buffers(&mut self) {
        while let Some(buffer) = self.buffers.pop() {
            self.write(&buffer);
        }
    }

    /// An uncaught error ends the script: a 500 while headers can still be
    /// sent.
    pub fn fatal_error(&mut self) {
        if self.sent.is_none() && self.code == 200 {
            self.code = 500;
            self.status_line = None;
        }
    }

    /// Applies a call `Response` recorded (`php::Output::calls`).
    pub fn apply(&mut self, call: &OutputCall) {
        if self.aborted {
            return;
        }
        match call {
            OutputCall::Header {
                line,
                replace,
                response_code,
            } => self.header(line.as_bytes(), *replace, *response_code),
            OutputCall::HeaderRemove(name) => self.header_remove(Some(name.as_bytes())),
            // FrankenPHP's and LiteSpeed's functions do not exist here, so
            // `Response` does not call them.
            OutputCall::HeadersSend(_) | OutputCall::LitespeedFinishRequest => {}
            OutputCall::Echo(bytes) => self.echo(bytes.as_bytes()),
            OutputCall::FastcgiFinishRequest => {
                self.fastcgi_finish_request();
            }
            OutputCall::ObEndFlush => {
                self.ob_end_flush();
            }
            OutputCall::ObEndClean => {
                self.ob_end_clean();
            }
            OutputCall::Flush => self.flush(),
            OutputCall::ObFlush => self.ob_flush(),
            OutputCall::SessionCookie { name, line } => {
                self.session_cookie(name.as_bytes(), line.as_bytes());
            }
            OutputCall::SetCookie(cookie) => {
                // the refusals are PHP's warnings: the cookie is not sent
                let _ = self.setcookie(cookie, false);
            }
        }
    }

    /// The body written since the last call, for a web server passing it
    /// on as it comes.
    pub fn take_body(&mut self) -> Vec<u8> {
        std::mem::take(&mut self.body)
    }

    /// Whether the response has ended (`fastcgi_finish_request()`).
    pub fn is_finished(&self) -> bool {
        self.finished
    }

    fn send_headers(&mut self) {
        if self.sent.is_some() {
            return;
        }
        if !self.content_type_set && !self.mimetype.as_bytes().is_empty() {
            let mut line = concat(&[b"Content-type: ", self.mimetype.as_bytes()])
                .as_bytes()
                .to_vec();
            if self.mimetype.as_bytes().starts_with(b"text/") && !self.charset.as_bytes().is_empty()
            {
                line.extend_from_slice(b"; charset=");
                line.extend_from_slice(self.charset.as_bytes());
            }
            self.lines.push(PhpString::from(line));
        }
        self.sent = Some(self.lines.clone());
    }

    /// The response PHP-FPM has sent so far (all of it once the script
    /// ended or [`Self::fastcgi_finish_request`] was called), headers
    /// sent if they were not.
    pub fn response(&mut self) -> CgiResponse {
        self.send_headers();
        // A `Status:` header with a value.
        let is_status = |line: &PhpString| {
            let line = line.as_bytes();
            let name = line_name(line);
            name.eq_ignore_ascii_case(b"status")
                && line[name.len()..]
                    .iter()
                    .any(|b| !matches!(b, b':' | b' ' | b'\t'))
        };
        let lines = self.sent.clone().unwrap_or_default();
        let mut headers = Vec::new();
        // A status other than 200 is written first, from the status line
        // `header()` was given or from its code, unless (without a status
        // line) a `Status:` header gives it; the first `Status:` header is
        // written when PHP writes none.
        #[expect(clippy::match_same_arms, reason = "PHP's cases, in order")]
        let own = match &self.status_line {
            Some(line) if self.code != 200 => Some(line.as_bytes().to_vec()),
            Some(_) => None,
            None if lines.iter().any(is_status) => None,
            None if self.code != 200 => {
                let mut text = self.code.to_string().into_bytes();
                if let Some(reason) = reason(self.code) {
                    text.push(b' ');
                    text.extend_from_slice(reason.as_bytes());
                }
                Some(text)
            }
            None => None,
        };
        let mut status_written = own.is_some();
        if let Some(text) = own {
            headers.push(concat(&[b"Status: ", &text]));
        }
        for line in lines {
            // An empty line is listed but not written; a 304 is sent
            // without a content type.
            if line.as_bytes().is_empty()
                || (self.code == 304
                    && line_name(line.as_bytes()).eq_ignore_ascii_case(b"content-type")
                    && line.as_bytes().len() > b"content-type:".len())
            {
                continue;
            }
            if is_status(&line) {
                if status_written {
                    continue;
                }
                status_written = true;
            }
            headers.push(line);
        }
        CgiResponse {
            headers,
            body: if self.head {
                Vec::new()
            } else {
                self.body.clone()
            },
        }
    }

    /// The end of the script: buffers flushed, headers sent.
    pub fn end(&mut self) -> CgiResponse {
        self.flush_buffers();
        self.response()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_integers_as_intval() {
        assert_eq!(leading_int(b" 404 x"), 404);
        assert_eq!(leading_int(b"+5"), 5);
        assert_eq!(leading_int(b"-1k"), -1);
        assert_eq!(leading_int(b"abc"), 0);
        assert_eq!(leading_int(b"99999999999999999999"), i64::MAX);
    }

    #[test]
    fn tells_malformed_file_names() {
        for name in [
            &b"o["[..],
            b"c]",
            b"arr[x]y",
            b"a[b][c",
            b"m[]x[y]",
            b"n[a]]",
            b"p[]]",
            b"q[x",
        ] {
            assert!(malformed(name), "{}", String::from_utf8_lossy(name));
        }
        for name in [
            &b"plain"[..],
            b"d[]",
            b"e[][]",
            b"f[ g ]",
            b"[x]",
            b"[]",
            b"r[a][]",
        ] {
            assert!(!malformed(name), "{}", String::from_utf8_lossy(name));
        }
        assert_eq!(entry_name(b"f[ g ][x]", "type"), b"f[type][g ][x]");
    }
}
