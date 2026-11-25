//! Port of Symfony\Component\HttpFoundation\Request.php (v8.1.11).
//!
//! What PHP keeps in static properties (trusted proxies and hosts, the
//! formats, the method override settings, the factory) is kept per thread
//! here, as PHP keeps it per request; [`Request::reset_statics`] stands for
//! the end of a PHP request. What `Request` reads from the PHP process (the
//! superglobals, `php://input`) is [`crate::php::Globals`].
//!
//! The bags are behind accessors: `query()` and `query_mut()` read and
//! change the bag, as `$request->query->set()` does; `set_query()` replaces
//! it, which upstream deprecates. Values PHP would coerce to a string (an
//! int `SERVER_PORT`, a `null` header) are coerced as PHP does; an array
//! or object where PHP wants a string is refused (`Class::NotPorted`)
//! rather than approximated.
//!
//! Not ported: the session methods (`getSession()`, `hasSession()`,
//! `hasPreviousSession()`, `setSession()`, `setSessionFactory()`), which
//! need HttpFoundation's Session classes, and `setPhpDefaultLocale()`'s
//! `\Locale::setDefault()` (intl has no port: setting a locale does not
//! change a process-wide ICU default here).
//!
//! The URL methods follow PHP on Linux, where the oracle runs:
//! `DIRECTORY_SEPARATOR` is `/`, so `prepareBasePath()`'s Windows branch is
//! not taken.
//!
//! Symfony source copyright (c) Fabien Potencier; distributed under MIT.
//! The base URL and path methods are derived by upstream from the Zend
//! Framework (1.10dev), new BSD licence, copyright (c) 2005-2010 Zend
//! Technologies USA Inc.
//!
//! Upstream: symfony/http-foundation Request.php

use std::{
    cell::RefCell,
    collections::{HashMap, HashSet},
    rc::Rc,
    sync::{Arc, LazyLock},
};

use php_pcre::Pcre;
use php_value::{
    Key, PhpArray, PhpString, Value,
    compare::loose_equals,
    html::html_entity_decode,
    json::decode_assoc,
    numeric::{float_to_string, string_to_int},
    path::{basename, dirname},
    url::{QueryEncoding, http_build_query, http_build_query_with, parse_url, rawurldecode},
    vars::parse_str,
};

use crate::{
    accept_header::AcceptHeader,
    exception::{Class, Error, Result},
    file_bag::FileBag,
    header_bag::{HeaderBag, HeaderValues},
    header_utils,
    input_bag::InputBag,
    ip_utils,
    parameter_bag::ParameterBag,
    php::{Stream, with_globals},
    server_bag::ServerBag,
};

pub const CLASS: &str = "Symfony\\Component\\HttpFoundation\\Request";

/// The raw body: PHP's `string|resource|null $content`.
#[derive(Debug, Clone, Default)]
pub enum Content {
    /// `null`: read `php://input` when asked.
    #[default]
    Null,
    String(PhpString),
    Resource(Stream),
}

impl From<&str> for Content {
    fn from(s: &str) -> Self {
        Self::String(PhpString::from(s))
    }
}

/// The body as the request holds it: after `getContent(true)` without a
/// body, PHP sets `$content` to `false`.
#[derive(Debug, Clone)]
enum Body {
    Null,
    False,
    String(PhpString),
    Resource(Stream),
}

impl From<Content> for Body {
    fn from(content: Content) -> Self {
        match content {
            Content::Null => Self::Null,
            Content::String(s) => Self::String(s),
            Content::Resource(r) => Self::Resource(r),
        }
    }
}

/// What `getContent()` returns.
#[derive(Debug, Clone)]
pub enum ContentValue {
    String(PhpString),
    Resource(Stream),
}

/// The constructor's and `initialize()`'s arguments, by name.
#[derive(Debug, Clone, Default)]
pub struct Parameters {
    pub query: PhpArray,
    pub request: PhpArray,
    pub attributes: PhpArray,
    pub cookies: PhpArray,
    pub files: PhpArray,
    pub server: PhpArray,
    pub content: Content,
}

/// `create()`'s arguments after the URI, by name; `method` is `GET` when
/// `None`.
#[derive(Debug, Clone, Default)]
pub struct Create {
    pub method: Option<PhpString>,
    pub parameters: PhpArray,
    pub cookies: PhpArray,
    pub files: PhpArray,
    pub server: PhpArray,
    pub content: Content,
}

/// `duplicate()`'s arguments: `None` keeps the request's bag.
#[derive(Debug, Clone, Default)]
pub struct Duplicate {
    pub query: Option<PhpArray>,
    pub request: Option<PhpArray>,
    pub attributes: Option<PhpArray>,
    pub cookies: Option<PhpArray>,
    pub files: Option<PhpArray>,
    pub server: Option<PhpArray>,
}

/// `setFactory()`'s callable. PHP checks that it returns a `Request`; the
/// type does here.
pub type Factory = Rc<dyn Fn(Parameters) -> Result<Request>>;

/// The client IPs `getClientIps()` returns: an entry is `null` where PHP's
/// is.
pub type ClientIps = Vec<Option<PhpString>>;

struct Statics {
    formats: Option<Vec<(PhpString, Vec<PhpString>)>>,
    trusted_proxies: Vec<PhpString>,
    trusted_host_patterns: Vec<PhpString>,
    http_method_parameter_override: bool,
    allowed_http_method_override: Option<Vec<PhpString>>,
    request_factory: Option<Factory>,
    trusted_header_set: i64,
    trusted_hosts_literals: HashSet<Vec<u8>>,
    trusted_hosts_regexps: Vec<(PhpString, Arc<Pcre>)>,
}

impl Statics {
    fn new() -> Self {
        Self {
            formats: None,
            trusted_proxies: Vec::new(),
            trusted_host_patterns: Vec::new(),
            http_method_parameter_override: false,
            allowed_http_method_override: None,
            request_factory: None,
            trusted_header_set: -1,
            trusted_hosts_literals: HashSet::new(),
            trusted_hosts_regexps: Vec::new(),
        }
    }
}

thread_local! {
    static STATICS: RefCell<Statics> = RefCell::new(Statics::new());
}

fn statics<R>(f: impl FnOnce(&mut Statics) -> R) -> R {
    STATICS.with(|s| f(&mut s.borrow_mut()))
}

const STRUCTURED_SUFFIX_FORMATS: [(&str, &str); 10] = [
    ("json", "json"),
    ("xml", "xml"),
    ("xhtml", "html"),
    ("cbor", "cbor"),
    ("zip", "zip"),
    ("ber", "asn1"),
    ("der", "asn1"),
    ("tlv", "tlv"),
    ("wbxml", "xml"),
    ("yaml", "yaml"),
];

const FORMATS: [(&str, &[&str]); 19] = [
    ("html", &["text/html", "application/xhtml+xml"]),
    ("txt", &["text/plain"]),
    (
        "js",
        &[
            "application/javascript",
            "application/x-javascript",
            "text/javascript",
        ],
    ),
    ("css", &["text/css"]),
    ("json", &["application/json", "application/x-json"]),
    ("jsonld", &["application/ld+json"]),
    ("xml", &["text/xml", "application/xml", "application/x-xml"]),
    ("rdf", &["application/rdf+xml"]),
    ("atom", &["application/atom+xml"]),
    ("rss", &["application/rss+xml"]),
    (
        "form",
        &["application/x-www-form-urlencoded", "multipart/form-data"],
    ),
    ("soap", &["application/soap+xml"]),
    ("problem", &["application/problem+json"]),
    ("hal", &["application/hal+json", "application/hal+xml"]),
    ("jsonapi", &["application/vnd.api+json"]),
    ("yaml", &["text/yaml", "application/x-yaml"]),
    ("wbxml", &["application/vnd.wap.wbxml"]),
    ("pdf", &["application/pdf"]),
    ("csv", &["text/csv"]),
];

fn initialize_formats(s: &mut Statics) -> &mut Vec<(PhpString, Vec<PhpString>)> {
    s.formats.get_or_insert_with(|| {
        FORMATS
            .iter()
            .map(|(format, types)| {
                (
                    PhpString::from(*format),
                    types.iter().map(|t| PhpString::from(*t)).collect(),
                )
            })
            .collect()
    })
}

/// The `$_SERVER` defaults `create()` starts from.
fn create_defaults() -> PhpArray {
    let mut server = PhpArray::new();
    for (key, value) in [
        ("SERVER_NAME", Value::string("localhost")),
        ("SERVER_PORT", Value::Int(80)),
        ("HTTP_HOST", Value::string("localhost")),
        ("HTTP_USER_AGENT", Value::string("Symfony")),
        (
            "HTTP_ACCEPT",
            Value::string("text/html,application/xhtml+xml,application/xml;q=0.9,*/*;q=0.8"),
        ),
        ("HTTP_ACCEPT_LANGUAGE", Value::string("en-us,en;q=0.5")),
        (
            "HTTP_ACCEPT_CHARSET",
            Value::string("ISO-8859-1,utf-8;q=0.7,*;q=0.7"),
        ),
        ("REMOTE_ADDR", Value::string("127.0.0.1")),
        ("SCRIPT_NAME", Value::string("")),
        ("SCRIPT_FILENAME", Value::string("")),
        ("SERVER_PROTOCOL", Value::string("HTTP/1.1")),
        ("REQUEST_TIME", Value::Int(php_value::clock::time())),
        (
            "REQUEST_TIME_FLOAT",
            Value::Float(php_value::clock::microtime()),
        ),
    ] {
        server.insert(key, value);
    }
    server
}

/// A value PHP converts to a string where `Request` passes it on.
fn string_of(value: &Value) -> Result<PhpString> {
    value.to_php_string_or_stringable().ok_or_else(|| {
        Error::new(
            Class::NotPorted,
            format!(
                "Request reads a {} where PHP converts the value to a string",
                value.debug_type()
            ),
        )
    })
}

/// A `?string` property or parameter given `value`.
fn nullable_string_of(value: &Value) -> Result<Option<PhpString>> {
    if value.is_null() {
        Ok(None)
    } else {
        string_of(value).map(Some)
    }
}

fn truthy(s: &[u8]) -> bool {
    php_value::string::truthy(s)
}

fn strtolower(s: &[u8]) -> Vec<u8> {
    s.to_ascii_lowercase()
}

fn strtoupper(s: &[u8]) -> Vec<u8> {
    s.to_ascii_uppercase()
}

fn cat(parts: &[&[u8]]) -> Vec<u8> {
    parts.concat()
}

fn concat(parts: &[&[u8]]) -> PhpString {
    PhpString::from(cat(parts))
}

fn find(haystack: &[u8], needle: &[u8]) -> Option<usize> {
    if needle.is_empty() {
        return Some(0);
    }
    haystack.windows(needle.len()).position(|w| w == needle)
}

fn find_ignore_case(haystack: &[u8], needle: &[u8]) -> Option<usize> {
    find(&strtolower(haystack), &strtolower(needle))
}

/// `rtrim($s, $chars)`.
fn rtrim<'a>(s: &'a [u8], chars: &[u8]) -> &'a [u8] {
    let end = s
        .iter()
        .rposition(|c| !chars.contains(c))
        .map_or(0, |i| i + 1);
    &s[..end]
}

/// `trim($s, $chars)`.
fn trim<'a>(s: &'a [u8], chars: &[u8]) -> &'a [u8] {
    let s = rtrim(s, chars);
    let start = s.iter().position(|c| !chars.contains(c)).unwrap_or(s.len());
    &s[start..]
}

/// `substr($s, $start, $length)` for a start and length PHP may give
/// negative.
fn substr(s: &[u8], start: i64, length: Option<i64>) -> &[u8] {
    let len = i64::try_from(s.len()).unwrap_or(i64::MAX);
    let mut from = if start < 0 {
        (len + start).max(0)
    } else {
        start
    };
    if from > len {
        from = len;
    }
    let to = match length {
        None => len,
        Some(l) if l < 0 => (len + l).max(from),
        Some(l) => (from + l).min(len),
    };
    let (from, to) = (
        usize::try_from(from).unwrap_or(0),
        usize::try_from(to).unwrap_or(0),
    );
    &s[from..to.max(from)]
}

/// `filter_var($value, FILTER_VALIDATE_IP)`, with `FILTER_FLAG_IPV4`
/// and `FILTER_FLAG_IPV6` as `v4` and `v6` (neither: both).
fn validate_ip(value: &[u8], v4: bool, v6: bool) -> bool {
    php_value::net::validate_ip(value, v4, v6)
}

fn check_ip(ip: &[u8], proxies: &[PhpString]) -> Result<bool> {
    let proxies: Vec<String> = proxies
        .iter()
        .map(|p| p.to_string_lossy().into_owned())
        .collect();
    let proxies: Vec<&str> = proxies.iter().map(String::as_str).collect();
    ip_utils::check_ip(&String::from_utf8_lossy(ip), &proxies)
}

/// `isHostValid()`: an IPv6 address in brackets; an IPv4 address when the
/// host ends like one (`/\.[0-9]++\.?$/D`); else labels of
/// `[-a-zA-Z0-9_]`, each followed by at most one dot
/// (`preg_replace('/[-a-zA-Z0-9_]++\.?/', '', $host)` leaving nothing).
fn is_host_valid(host: &[u8]) -> Result<bool> {
    if host.first() == Some(&b'[') {
        if host.last() != Some(&b']') || host.len() < 2 {
            return Ok(false);
        }
        return Ok(validate_ip(&host[1..host.len() - 1], false, true));
    }
    let unterminated = host.strip_suffix(b".").unwrap_or(host);
    let digits = unterminated
        .iter()
        .rev()
        .take_while(|c| c.is_ascii_digit())
        .count();
    if digits > 0
        && unterminated.len() > digits
        && unterminated[unterminated.len() - digits - 1] == b'.'
    {
        return Ok(validate_ip(host, true, false));
    }
    let label = |c: &u8| c.is_ascii_alphanumeric() || matches!(c, b'-' | b'_');
    let mut i = 0;
    while i < host.len() {
        if !label(&host[i]) {
            return Ok(false);
        }
        i += host[i..].iter().take_while(|c| label(c)).count();
        if host.get(i) == Some(&b'.') {
            i += 1;
        }
    }
    Ok(true)
}

/// A host pattern that is a constant (`^literal$`, escapes only before
/// non-alphanumerics): its lowercase host, as `setTrustedHosts()` reads it
/// with `{^\^((?:[a-z0-9_:-]|\\[^a-z0-9])++)\$$}Di`.
fn literal_host(pattern: &[u8]) -> Option<Vec<u8>> {
    let body = pattern.strip_prefix(b"^")?;
    let mut host = Vec::new();
    let mut i = 0;
    // possessive: tokens are taken while they match, without giving back
    while i < body.len() {
        let c = body[i];
        if c.is_ascii_alphanumeric() || matches!(c, b'_' | b':' | b'-') {
            host.push(c);
            i += 1;
        } else if c == b'\\' && body.get(i + 1).is_some_and(|n| !n.is_ascii_alphanumeric()) {
            host.push(body[i + 1]);
            i += 2;
        } else {
            break;
        }
    }
    (!host.is_empty() && &body[i..] == b"$").then(|| strtolower(&host))
}

/// `preg_match()` on a trusted host pattern: an error is no match, as
/// `preg_match()`'s `false` is.
fn host_matches(regex: &Pcre, host: &[u8]) -> bool {
    regex.is_match(host).unwrap_or(false)
}

static LANGUAGE: LazyLock<std::result::Result<Pcre, php_pcre::PcreError>> = LazyLock::new(|| {
    Pcre::new(b"/^([a-zA-Z]{2,3}|i-[a-zA-Z]{5,})(?:-([a-zA-Z]{4}))?(?:-([a-zA-Z]{2}))?(?:-(.+))?$/")
});

/// `getLanguageComponents()`: language, script and region.
fn language_components(locale: &[u8]) -> Result<[Option<Vec<u8>>; 3]> {
    let locale: Vec<u8> = strtolower(locale)
        .into_iter()
        .map(|c| if c == b'_' { b'-' } else { c })
        .collect();
    let regex = LANGUAGE
        .as_ref()
        .map_err(|e| Error::new(Class::NotPorted, e.to_string()))?;
    let groups = regex
        .captures_from(&locale, 0)
        .map_err(|e| Error::new(Class::NotPorted, e.0))?;
    let Some(groups) = groups else {
        return Ok([Some(locale), None, None]);
    };
    let group = |n: usize| {
        groups
            .get(n)
            .copied()
            .flatten()
            .map(|(a, b)| locale[a..b].to_vec())
    };
    let mut language = group(1).unwrap_or_default();
    if language.starts_with(b"i-") {
        language.drain(..2);
    }
    // a group that did not take part is '' in $matches when a later one
    // did, and either way array_filter() drops it
    let script = group(2).map(|s| {
        let mut s = strtolower(&s);
        if let Some(first) = s.first_mut() {
            first.make_ascii_uppercase();
        }
        s
    });
    let region = group(3).map(|r| strtoupper(&r));
    Ok([Some(language), script, region])
}

/// `implode('_', array_filter($parts))`.
fn join_truthy(parts: &[&Option<Vec<u8>>]) -> Vec<u8> {
    parts
        .iter()
        .filter_map(|p| p.as_deref().filter(|p| truthy(p)))
        .collect::<Vec<_>>()
        .join(&b'_')
}

fn format_locale(locale: &[u8]) -> Result<Vec<u8>> {
    let [language, script, region] = language_components(locale)?;
    Ok(join_truthy(&[&language, &script, &region]))
}

/// `getLanguageCombinations()`, `array_unique()` keeping the first.
fn language_combinations(locale: &[u8]) -> Result<Vec<Vec<u8>>> {
    let [language, script, region] = language_components(locale)?;
    let mut out: Vec<Vec<u8>> = Vec::new();
    for combination in [
        join_truthy(&[&language, &script, &region]),
        join_truthy(&[&language, &script]),
        join_truthy(&[&language, &region]),
        language.clone().unwrap_or_default(),
    ] {
        if !out.contains(&combination) {
            out.push(combination);
        }
    }
    Ok(out)
}

/// The header values of a `$_SERVER`-style entry, as `new HeaderBag()`
/// takes them.
fn header_values(value: &Value) -> Result<HeaderValues> {
    Ok(match value {
        Value::Array(values) => HeaderValues::Many(
            values
                .values()
                .map(nullable_string_of)
                .collect::<Result<_>>()?,
        ),
        other => HeaderValues::One(nullable_string_of(other)?),
    })
}

fn header_bag(headers: &PhpArray) -> Result<HeaderBag> {
    let mut bag = HeaderBag::new(Vec::<(&[u8], HeaderValues)>::new());
    for (key, value) in headers.iter() {
        bag.set(&key.bytes(), header_values(value)?, true)?;
    }
    Ok(bag)
}

/// `get_debug_type()` of a decoded JSON value.
fn json_type(value: &Value) -> String {
    value.debug_type()
}

macro_rules! bag {
    ($field:ident, $mut:ident, $set:ident, $ty:ty, $extra:literal) => {
        #[doc = concat!("`$request->", stringify!($field), "`.")]
        pub fn $field(&self) -> &$ty {
            &self.$field
        }

        #[doc = concat!("`$request->", stringify!($field), "`, to change the bag.")]
        pub fn $mut(&mut self) -> &mut $ty {
            &mut self.$field
        }

        #[doc = concat!("`$request->", stringify!($field), " = $bag`, deprecated since 8.1.")]
        pub fn $set(&mut self, bag: $ty) {
            symfony_deprecation_contracts::trigger_deprecation(
                b"symfony/http-foundation",
                b"8.1",
                format!(
                    concat!(
                        "Directly setting property \"",
                        stringify!($field),
                        "\" of \"{}\" is deprecated; ",
                        $extra
                    ),
                    CLASS
                )
                .as_bytes(),
            );
            self.$field = bag;
        }
    };
}

/// `Symfony\Component\HttpFoundation\Request`.
#[derive(Debug, Clone)]
pub struct Request {
    attributes: ParameterBag,
    request: InputBag,
    query: InputBag,
    server: ServerBag,
    files: FileBag,
    cookies: InputBag,
    headers: HeaderBag,
    content: Body,
    languages: Option<PhpArray>,
    charsets: Option<Vec<PhpString>>,
    encodings: Option<Vec<PhpString>>,
    acceptable_content_types: Option<Vec<PhpString>>,
    path_info: Option<PhpString>,
    request_uri: Option<PhpString>,
    base_url: Option<PhpString>,
    base_path: Option<PhpString>,
    method: Option<PhpString>,
    format: Option<PhpString>,
    locale: Option<PhpString>,
    default_locale: PhpString,
    preferred_format: Option<PhpString>,
    is_host_valid: bool,
    is_forwarded_valid: bool,
    is_safe_content_preferred: Option<bool>,
    trusted_values_cache: HashMap<Vec<u8>, Vec<Value>>,
    is_iis_rewrite: bool,
}

impl Default for Request {
    /// `new Request()`.
    fn default() -> Self {
        Self {
            attributes: ParameterBag::default(),
            request: InputBag::default(),
            query: InputBag::default(),
            server: ServerBag::new(PhpArray::new()),
            files: FileBag::default(),
            cookies: InputBag::default(),
            headers: HeaderBag::default(),
            content: Body::Null,
            languages: None,
            charsets: None,
            encodings: None,
            acceptable_content_types: None,
            path_info: None,
            request_uri: None,
            base_url: None,
            base_path: None,
            method: None,
            format: None,
            locale: None,
            default_locale: PhpString::from("en"),
            preferred_format: None,
            is_host_valid: true,
            is_forwarded_valid: true,
            is_safe_content_preferred: None,
            trusted_values_cache: HashMap::new(),
            is_iis_rewrite: false,
        }
    }
}

impl Request {
    pub const HEADER_FORWARDED: i64 = 0b00_0001;
    pub const HEADER_X_FORWARDED_FOR: i64 = 0b00_0010;
    pub const HEADER_X_FORWARDED_HOST: i64 = 0b00_0100;
    pub const HEADER_X_FORWARDED_PROTO: i64 = 0b00_1000;
    pub const HEADER_X_FORWARDED_PORT: i64 = 0b01_0000;
    pub const HEADER_X_FORWARDED_PREFIX: i64 = 0b10_0000;
    /// AWS ELB doesn't send X-Forwarded-Host.
    pub const HEADER_X_FORWARDED_AWS_ELB: i64 = 0b001_1010;
    /// All "X-Forwarded-*" headers sent by Traefik reverse proxy.
    pub const HEADER_X_FORWARDED_TRAEFIK: i64 = 0b011_1110;

    pub const METHOD_HEAD: &'static str = "HEAD";
    pub const METHOD_GET: &'static str = "GET";
    pub const METHOD_POST: &'static str = "POST";
    pub const METHOD_PUT: &'static str = "PUT";
    pub const METHOD_PATCH: &'static str = "PATCH";
    pub const METHOD_DELETE: &'static str = "DELETE";
    pub const METHOD_PURGE: &'static str = "PURGE";
    pub const METHOD_OPTIONS: &'static str = "OPTIONS";
    pub const METHOD_TRACE: &'static str = "TRACE";
    pub const METHOD_CONNECT: &'static str = "CONNECT";
    pub const METHOD_QUERY: &'static str = "QUERY";

    /// The class's int constants, as reflection lists them.
    pub const INT_CONSTANTS: [(&'static str, i64); 8] = [
        ("HEADER_FORWARDED", Self::HEADER_FORWARDED),
        ("HEADER_X_FORWARDED_FOR", Self::HEADER_X_FORWARDED_FOR),
        ("HEADER_X_FORWARDED_HOST", Self::HEADER_X_FORWARDED_HOST),
        ("HEADER_X_FORWARDED_PROTO", Self::HEADER_X_FORWARDED_PROTO),
        ("HEADER_X_FORWARDED_PORT", Self::HEADER_X_FORWARDED_PORT),
        ("HEADER_X_FORWARDED_PREFIX", Self::HEADER_X_FORWARDED_PREFIX),
        (
            "HEADER_X_FORWARDED_AWS_ELB",
            Self::HEADER_X_FORWARDED_AWS_ELB,
        ),
        (
            "HEADER_X_FORWARDED_TRAEFIK",
            Self::HEADER_X_FORWARDED_TRAEFIK,
        ),
    ];

    /// `TRUSTED_HEADERS`.
    fn trusted_header(kind: i64) -> &'static str {
        match kind {
            Self::HEADER_FORWARDED => "FORWARDED",
            Self::HEADER_X_FORWARDED_FOR => "X_FORWARDED_FOR",
            Self::HEADER_X_FORWARDED_HOST => "X_FORWARDED_HOST",
            Self::HEADER_X_FORWARDED_PROTO => "X_FORWARDED_PROTO",
            Self::HEADER_X_FORWARDED_PORT => "X_FORWARDED_PORT",
            _ => "X_FORWARDED_PREFIX",
        }
    }

    /// `FORWARDED_PARAMS`.
    fn forwarded_param(kind: i64) -> Option<&'static str> {
        match kind {
            Self::HEADER_X_FORWARDED_FOR => Some("for"),
            Self::HEADER_X_FORWARDED_HOST | Self::HEADER_X_FORWARDED_PORT => Some("host"),
            Self::HEADER_X_FORWARDED_PROTO => Some("proto"),
            _ => None,
        }
    }

    /// `new Request(...)`.
    pub fn new(parameters: Parameters) -> Result<Self> {
        let mut request = Self::default();
        request.initialize(parameters)?;
        Ok(request)
    }

    bag!(
        attributes,
        attributes_mut,
        set_attributes,
        ParameterBag,
        "pass attributes as a constructor argument or call \"initialize()\" instead."
    );
    bag!(
        request,
        request_mut,
        set_request,
        InputBag,
        "pass the POST data as a constructor argument or call \"initialize()\" instead."
    );
    bag!(
        query,
        query_mut,
        set_query,
        InputBag,
        "pass query parameters as a constructor argument or call \"initialize()\" instead."
    );
    bag!(
        server,
        server_mut,
        set_server,
        ServerBag,
        "pass server parameters as a constructor argument or call \"initialize()\" instead."
    );
    bag!(
        files,
        files_mut,
        set_files,
        FileBag,
        "pass files as a constructor argument or call \"initialize()\" instead."
    );
    bag!(
        cookies,
        cookies_mut,
        set_cookies,
        InputBag,
        "pass cookies as a constructor argument or call \"initialize()\" instead."
    );
    bag!(
        headers,
        headers_mut,
        set_headers,
        HeaderBag,
        "pass header parameters as a constructor argument or call \"initialize()\" instead."
    );

    fn reset_caches(&mut self) {
        self.languages = None;
        self.charsets = None;
        self.encodings = None;
        self.acceptable_content_types = None;
        self.path_info = None;
        self.request_uri = None;
        self.base_url = None;
        self.base_path = None;
        self.method = None;
        self.format = None;
    }

    /// `initialize(...)`: new bags, and every cached value forgotten.
    pub fn initialize(&mut self, parameters: Parameters) -> Result<()> {
        self.request = InputBag::new(parameters.request);
        self.query = InputBag::new(parameters.query);
        self.attributes = ParameterBag::new(parameters.attributes);
        self.cookies = InputBag::new(parameters.cookies);
        self.files = FileBag::new(parameters.files)?;
        self.server = ServerBag::new(parameters.server);
        self.headers = header_bag(&self.server.get_headers()?)?;
        self.content = parameters.content.into();
        self.reset_caches();
        Ok(())
    }

    /// `createFromGlobals()`: from this thread's [`crate::php::Globals`];
    /// for PUT, DELETE, PATCH and QUERY the body is what
    /// `request_parse_body()` returned, or `$_POST` and `$_FILES` where it
    /// threw.
    pub fn create_from_globals() -> Result<Self> {
        let globals = with_globals(|g| g.clone());
        let method = globals.server.get(&Key::from("REQUEST_METHOD"));
        let parses_body = matches!(method, Some(Value::String(m))
            if matches!(m.as_bytes(), b"PUT" | b"DELETE" | b"PATCH" | b"QUERY"));
        let (post, files) = match (parses_body, globals.parsed_body) {
            (true, Some(body)) => body,
            _ => (globals.post, globals.files),
        };
        Self::create_request_from_factory(Parameters {
            query: globals.get,
            request: post,
            attributes: PhpArray::new(),
            cookies: globals.cookie,
            files,
            server: globals.server,
            content: Content::Null,
        })
    }

    /// `create($uri)` with the default method and no parameters.
    pub fn create(uri: impl AsRef<[u8]>) -> Result<Self> {
        Self::create_with(uri, Create::default())
    }

    /// `create($uri, $method, $parameters, $cookies, $files, $server,
    /// $content)`.
    pub fn create_with(uri: impl AsRef<[u8]>, args: Create) -> Result<Self> {
        let uri = uri.as_ref();
        let method = strtoupper(
            args.method
                .as_ref()
                .map_or(&b"GET"[..], PhpString::as_bytes),
        );
        let mut server = create_defaults();
        for (key, value) in args.server.iter() {
            server.insert(key.clone(), value.clone());
        }
        server.insert("PATH_INFO", Value::string(""));
        server.insert("REQUEST_METHOD", Value::string(method.clone()));

        let bad = |message: &str| Error::new(Class::BadRequest, message);
        let i = uri
            .iter()
            .position(|c| matches!(c, b':' | b'/' | b'?' | b'#'))
            .unwrap_or(uri.len());
        if i > 0
            && uri.get(i) == Some(&b':')
            && (uri[..i]
                .iter()
                .any(|c| !(c.is_ascii_alphanumeric() || matches!(c, b'+' | b'-' | b'.')))
                || !uri[0].is_ascii_alphabetic())
        {
            return Err(bad("Invalid URI: Scheme is malformed."));
        }
        let has_query_or_fragment = uri.iter().any(|c| matches!(c, b'?' | b'#'));
        let parsed = if has_query_or_fragment {
            parse_url(uri)
        } else {
            parse_url(&[uri, b"#"].concat())
        };
        let Some(mut components) = parsed else {
            return Err(bad("Invalid URI."));
        };

        let user_info = [
            components.user.as_deref().unwrap_or_default(),
            b":",
            components.pass.as_deref().unwrap_or_default(),
        ]
        .concat();
        if user_info != b":" && user_info.iter().any(|c| matches!(c, b'[' | b']')) {
            return Err(bad("Invalid URI: Userinfo is malformed."));
        }
        if let Some(host) = components.host.as_deref()
            && truthy(host)
            && !is_host_valid(host)?
        {
            return Err(bad("Invalid URI: Host is malformed."));
        }
        let before_query = uri
            .iter()
            .position(|c| matches!(c, b'?' | b'#'))
            .unwrap_or(uri.len());
        if uri
            .iter()
            .position(|&c| c == b'\\')
            .is_some_and(|b| b < before_query)
        {
            return Err(bad("Invalid URI: A URI cannot contain a backslash."));
        }
        if uri.iter().any(|c| matches!(c, b'\r' | b'\n' | b'\t')) {
            return Err(bad(
                "Invalid URI: A URI cannot contain CR/LF/TAB characters.",
            ));
        }
        if let (Some(&first), Some(&last)) = (uri.first(), uri.last())
            && (first <= 32 || last <= 32)
        {
            return Err(bad(
                "Invalid URI: A URI must not start nor end with ASCII control characters or spaces.",
            ));
        }

        if let Some(host) = &components.host {
            server.insert("SERVER_NAME", Value::string(host.clone()));
            server.insert("HTTP_HOST", Value::string(host.clone()));
        }
        if let Some(scheme) = &components.scheme {
            if scheme == b"https" {
                server.insert("HTTPS", Value::string("on"));
                server.insert("SERVER_PORT", Value::Int(443));
            } else {
                server.remove(&Key::from("HTTPS"));
                server.insert("SERVER_PORT", Value::Int(80));
            }
        }
        if let Some(port) = components.port {
            server.insert("SERVER_PORT", Value::Int(port));
            let host = server
                .get(&Key::from("HTTP_HOST"))
                .map(string_of)
                .transpose()?
                .unwrap_or_default();
            server.insert(
                "HTTP_HOST",
                Value::String(concat(&[
                    host.as_bytes(),
                    b":",
                    port.to_string().as_bytes(),
                ])),
            );
        }
        if let Some(user) = &components.user {
            server.insert("PHP_AUTH_USER", Value::string(user.clone()));
        }
        if let Some(pass) = &components.pass {
            server.insert("PHP_AUTH_PW", Value::string(pass.clone()));
        }

        let path = components.path.clone().unwrap_or_default();
        if path.is_empty() {
            components.path = Some(b"/".to_vec());
        } else if components.scheme.is_none() && components.host.is_none() && path[0] != b'/' {
            let first = path.split(|&c| c == b'/').next().unwrap_or_default();
            if first.contains(&b':') {
                return Err(bad("Invalid URI: Path is malformed."));
            }
        }

        let (request, mut query) = match method.as_slice() {
            b"POST" | b"PUT" | b"DELETE" | b"QUERY" | b"PATCH" => {
                if method != b"PATCH"
                    && server
                        .get(&Key::from("CONTENT_TYPE"))
                        .is_none_or(Value::is_null)
                {
                    server.insert(
                        "CONTENT_TYPE",
                        Value::string("application/x-www-form-urlencoded"),
                    );
                }
                (args.parameters, PhpArray::new())
            }
            _ => (PhpArray::new(), args.parameters),
        };

        let build = |query: &PhpArray| {
            http_build_query(query, b"&").ok_or_else(|| {
                Error::new(
                    Class::NotPorted,
                    "http_build_query() of an object is not ported",
                )
            })
        };
        let mut query_string = Vec::new();
        if let Some(components_query) = &components.query {
            let qs = parse_str(&html_entity_decode(components_query));
            if query.is_empty() {
                query = qs;
                query_string.clone_from(components_query);
            } else {
                let mut replaced = qs;
                for (key, value) in query.iter() {
                    replaced.insert(key.clone(), value.clone());
                }
                query = replaced;
                query_string = build(&query)?;
            }
        } else if !query.is_empty() {
            query_string = build(&query)?;
        }

        let mut request_uri = components.path.clone().unwrap_or_default();
        if !query_string.is_empty() {
            request_uri.push(b'?');
            request_uri.extend_from_slice(&query_string);
        }
        server.insert("REQUEST_URI", Value::string(request_uri));
        server.insert("QUERY_STRING", Value::string(query_string));

        Self::create_request_from_factory(Parameters {
            query,
            request,
            attributes: PhpArray::new(),
            cookies: args.cookies,
            files: args.files,
            server,
            content: args.content,
        })
    }

    /// `setFactory($callable)`: `None` restores `new static(...)`.
    pub fn set_factory(factory: Option<Factory>) {
        statics(|s| s.request_factory = factory);
    }

    fn create_request_from_factory(parameters: Parameters) -> Result<Self> {
        match statics(|s| s.request_factory.clone()) {
            Some(factory) => factory(parameters),
            None => Self::new(parameters),
        }
    }

    /// `duplicate(...)`: a copy with the given bags replaced.
    pub fn duplicate(&mut self, args: Duplicate) -> Result<Self> {
        let mut dup = self.clone();
        if let Some(query) = args.query {
            dup.query = InputBag::new(query);
        }
        if let Some(request) = args.request {
            dup.request = InputBag::new(request);
        }
        if let Some(attributes) = args.attributes {
            dup.attributes = ParameterBag::new(attributes);
        }
        if let Some(cookies) = args.cookies {
            dup.cookies = InputBag::new(cookies);
        }
        if let Some(files) = args.files {
            dup.files = FileBag::new(files)?;
        }
        if let Some(server) = args.server {
            dup.server = ServerBag::new(server);
            dup.headers = header_bag(&dup.server.get_headers()?)?;
        }
        dup.reset_caches();
        if !dup.attributes.has("_format") && self.attributes.has("_format") {
            dup.attributes
                .set("_format", self.attributes.get("_format", Value::Null));
        }
        if !dup
            .get_request_format(None)?
            .is_some_and(|f| truthy(f.as_bytes()))
        {
            let format = self.get_request_format(None)?;
            dup.set_request_format(format);
        }
        Ok(dup)
    }

    /// `__toString()`.
    pub fn to_php_string(&mut self) -> Result<PhpString> {
        let content = match self.get_content(false)? {
            ContentValue::String(s) => s,
            ContentValue::Resource(r) => PhpString::from(r.contents()),
        };
        let mut cookies: Vec<Vec<u8>> = Vec::new();
        for (key, value) in self.cookies.iter() {
            if let Value::Array(_) = value {
                let mut one = PhpArray::new();
                one.insert(key.clone(), value.clone());
                cookies.push(
                    http_build_query_with(&one, b"; ", QueryEncoding::Rfc3986).ok_or_else(
                        || Error::new(Class::NotPorted, "http_build_query() of an object"),
                    )?,
                );
            } else {
                cookies.push([&key.bytes()[..], b"=", string_of(value)?.as_bytes()].concat());
            }
        }
        let cookie_header = if cookies.is_empty() {
            Vec::new()
        } else {
            [b"Cookie: ", &cookies.join(&b"; "[..])[..], b"\r\n"].concat()
        };
        let method = self.get_method()?;
        let uri = self.get_request_uri()?;
        let protocol = string_of(&self.server.get("SERVER_PROTOCOL", Value::Null))?;
        Ok(concat(&[
            method.as_bytes(),
            b" ",
            uri.as_bytes(),
            b" ",
            protocol.as_bytes(),
            b"\r\n",
            self.headers.to_php_string().as_bytes(),
            &cookie_header,
            b"\r\n",
            content.as_bytes(),
        ]))
    }

    /// `overrideGlobals()`: `$_GET`, `$_POST`, `$_SERVER`, `$_COOKIE` and
    /// `$_REQUEST` from this request, on this thread's
    /// [`crate::php::Globals`]. `$_FILES` is left as it is.
    pub fn override_globals(&mut self) -> Result<()> {
        let query_string = Self::normalize_query_string(Some(
            &http_build_query(self.query.all(), b"&")
                .ok_or_else(|| Error::new(Class::NotPorted, "http_build_query() of an object"))?,
        ))?;
        self.server.set("QUERY_STRING", Value::String(query_string));

        let get = self.query.all().clone();
        let post = self.request.all().clone();
        let cookie = self.cookies.all().clone();
        let mut server = self.server.all().clone();
        for (key, values) in self.headers.all().iter() {
            let key: Vec<u8> = strtoupper(&key.bytes())
                .into_iter()
                .map(|c| if c == b'-' { b'_' } else { c })
                .collect();
            let value: Vec<u8> = values
                .iter()
                .map(|v| {
                    v.as_ref()
                        .map(|v| v.as_bytes().to_vec())
                        .unwrap_or_default()
                })
                .collect::<Vec<_>>()
                .join(&b", "[..]);
            let name = if matches!(
                key.as_slice(),
                b"CONTENT_TYPE" | b"CONTENT_LENGTH" | b"CONTENT_MD5"
            ) {
                key
            } else {
                [b"HTTP_", key.as_slice()].concat()
            };
            server.insert(Key::from_bytes(&name), Value::string(value));
        }

        with_globals(|g| {
            let order = if truthy(g.request_order.as_bytes()) {
                g.request_order.as_bytes()
            } else {
                g.variables_order.as_bytes()
            };
            let mut order: Vec<u8> = strtolower(order)
                .into_iter()
                .filter(|c| matches!(c, b'c' | b'g' | b'p'))
                .collect();
            if order.is_empty() {
                order = b"gp".to_vec();
            }
            let mut request = PhpArray::new();
            for c in order {
                let source = match c {
                    b'g' => &get,
                    b'p' => &post,
                    _ => &cookie,
                };
                // array_merge(): string keys replace, int keys append
                for (key, value) in source.iter() {
                    match key {
                        Key::Int(_) => {
                            let next = request.next_index().unwrap_or(0);
                            request.insert(next, value.clone());
                        }
                        Key::Str(_) => request.insert(key.clone(), value.clone()),
                    }
                }
            }
            g.get = get.clone();
            g.post = post.clone();
            g.server = server;
            g.cookie = cookie.clone();
            g.request = request;
        });
        Ok(())
    }

    /// `setTrustedProxies($proxies, $trustedHeaderSet)`: `REMOTE_ADDR` is
    /// this thread's `$_SERVER['REMOTE_ADDR']` (or dropped), and
    /// `PRIVATE_SUBNETS` or `private_ranges` is `IpUtils::PRIVATE_RANGES`.
    pub fn set_trusted_proxies<P: Into<PhpString>>(
        proxies: impl IntoIterator<Item = P>,
        trusted_header_set: i64,
    ) -> Result<()> {
        let mut proxies: Vec<PhpString> = proxies.into_iter().map(Into::into).collect();
        if let Some(i) = proxies.iter().position(|p| p.as_bytes() == b"REMOTE_ADDR") {
            let remote = with_globals(|g| g.server.get(&Key::from("REMOTE_ADDR")).cloned());
            match remote.filter(|v| !v.is_null()) {
                Some(addr) => proxies[i] = string_of(&addr)?,
                None => {
                    proxies.remove(i);
                }
            }
        }
        let private = proxies
            .iter()
            .position(|p| p.as_bytes() == b"PRIVATE_SUBNETS")
            .or_else(|| {
                proxies
                    .iter()
                    .position(|p| p.as_bytes() == b"private_ranges")
            });
        if let Some(i) = private {
            proxies.remove(i);
            proxies.extend(ip_utils::PRIVATE_RANGES.iter().map(|r| PhpString::from(*r)));
        }
        statics(|s| {
            s.trusted_proxies = proxies;
            s.trusted_header_set = trusted_header_set;
        });
        Ok(())
    }

    /// `getTrustedProxies()`.
    pub fn get_trusted_proxies() -> Vec<PhpString> {
        statics(|s| s.trusted_proxies.clone())
    }

    /// `getTrustedHeaderSet()`.
    pub fn get_trusted_header_set() -> i64 {
        statics(|s| s.trusted_header_set)
    }

    /// `setTrustedHosts($hostPatterns)`. Constant patterns are looked up;
    /// each other pattern is matched on its own as `{(?:pattern)}i`, which
    /// is what upstream's combined `(?|...)` pattern does for it. A
    /// pattern that does not compile alone (as `{pattern}i` and wrapped)
    /// could change its neighbours in the combined pattern, and is refused
    /// (`Class::NotPorted`), as is one php-pcre does not translate.
    pub fn set_trusted_hosts<P: Into<PhpString>>(
        host_patterns: impl IntoIterator<Item = P>,
    ) -> Result<()> {
        let host_patterns: Vec<PhpString> = host_patterns.into_iter().map(Into::into).collect();
        let mut literals = HashSet::new();
        let mut regexps = Vec::new();
        for pattern in &host_patterns {
            if let Some(host) = literal_host(pattern.as_bytes()) {
                literals.insert(host);
                continue;
            }
            let refuse = |why: String| {
                Error::new(
                    Class::NotPorted,
                    format!(
                        "trusted host pattern \"{}\": {why}",
                        pattern.to_string_lossy()
                    ),
                )
            };
            Pcre::new(&[b"{", pattern.as_bytes(), b"}i"].concat())
                .map_err(|e| refuse(e.to_string()))?;
            let regex = Pcre::new(&[b"{(?:", pattern.as_bytes(), b")}i"].concat())
                .map_err(|e| refuse(e.to_string()))?;
            regexps.push((
                PhpString::from([b"{(?:", pattern.as_bytes(), b")}i"].concat()),
                Arc::new(regex),
            ));
        }
        statics(|s| {
            s.trusted_host_patterns = host_patterns
                .iter()
                .map(|p| concat(&[b"{", p.as_bytes(), b"}i"]))
                .collect();
            s.trusted_hosts_literals = literals;
            s.trusted_hosts_regexps = regexps;
        });
        Ok(())
    }

    /// `getTrustedHosts()`: the patterns as `{pattern}i`.
    pub fn get_trusted_hosts() -> Vec<PhpString> {
        statics(|s| s.trusted_host_patterns.clone())
    }

    /// The private `$trustedHostsRegexps`: the patterns matched as regular
    /// expressions (constant ones are looked up instead).
    #[doc(hidden)]
    pub fn trusted_hosts_regexps() -> Vec<PhpString> {
        statics(|s| {
            s.trusted_hosts_regexps
                .iter()
                .map(|(p, _)| p.clone())
                .collect()
        })
    }

    /// `normalizeQueryString($qs)`.
    pub fn normalize_query_string(qs: Option<&[u8]>) -> Result<PhpString> {
        let Some(qs) = qs.filter(|qs| !qs.is_empty()) else {
            return Ok(PhpString::default());
        };
        let mut parsed = header_utils::parse_query(qs, false, b"&")?;
        parsed.ksort();
        http_build_query_with(&parsed, b"&", QueryEncoding::Rfc3986)
            .map(PhpString::from)
            .ok_or_else(|| Error::new(Class::NotPorted, "http_build_query() of an object"))
    }

    /// `enableHttpMethodParameterOverride()`.
    pub fn enable_http_method_parameter_override() {
        statics(|s| s.http_method_parameter_override = true);
    }

    /// `getHttpMethodParameterOverride()`.
    pub fn get_http_method_parameter_override() -> bool {
        statics(|s| s.http_method_parameter_override)
    }

    /// `setAllowedHttpMethodOverride($methods)`.
    pub fn set_allowed_http_method_override<P: Into<PhpString>>(
        methods: Option<impl IntoIterator<Item = P>>,
    ) -> Result<()> {
        let methods: Option<Vec<PhpString>> =
            methods.map(|m| m.into_iter().map(Into::into).collect());
        if methods
            .iter()
            .flatten()
            .any(|m| matches!(m.as_bytes(), b"GET" | b"HEAD" | b"CONNECT" | b"TRACE"))
        {
            return Err(Error::new(
                Class::PhpInvalidArgument,
                "The HTTP methods \"GET\", \"HEAD\", \"CONNECT\", and \"TRACE\" cannot be overridden.",
            ));
        }
        statics(|s| s.allowed_http_method_override = methods);
        Ok(())
    }

    /// `getAllowedHttpMethodOverride()`.
    pub fn get_allowed_http_method_override() -> Option<Vec<PhpString>> {
        statics(|s| s.allowed_http_method_override.clone())
    }

    /// The end of a PHP request: every static property back to its
    /// declared value (no trusted proxies or hosts, the default formats,
    /// no method override, no factory).
    pub fn reset_statics() {
        statics(|s| *s = Statics::new());
    }

    fn remote_addr(&self) -> Result<Option<PhpString>> {
        nullable_string_of(&self.server.get("REMOTE_ADDR", Value::Null))
    }

    /// `getClientIps()`: the most trusted first; trusted proxies removed.
    pub fn get_client_ips(&mut self) -> Result<ClientIps> {
        let ip = self.remote_addr()?;
        if !self.is_from_trusted_proxy()? {
            return Ok(vec![ip]);
        }
        let values = self.get_trusted_values(Self::HEADER_X_FORWARDED_FOR, ip.as_ref())?;
        if values.is_empty() {
            return Ok(vec![ip]);
        }
        values.iter().map(nullable_string_of).collect::<Result<_>>()
    }

    /// `getClientIp()`.
    pub fn get_client_ip(&mut self) -> Result<Option<PhpString>> {
        Ok(self.get_client_ips()?.into_iter().next().flatten())
    }

    /// `getScriptName()`.
    pub fn get_script_name(&self) -> Result<PhpString> {
        let orig = self.server.get("ORIG_SCRIPT_NAME", Value::string(""));
        string_of(&self.server.get("SCRIPT_NAME", orig))
    }

    /// `getPathInfo()`: the raw path relative to the executed script.
    pub fn get_path_info(&mut self) -> Result<PhpString> {
        if let Some(path_info) = &self.path_info {
            return Ok(path_info.clone());
        }
        let path_info = self.prepare_path_info()?;
        self.path_info = Some(path_info.clone());
        Ok(path_info)
    }

    /// `getBasePath()`.
    pub fn get_base_path(&mut self) -> Result<PhpString> {
        if let Some(base_path) = &self.base_path {
            return Ok(base_path.clone());
        }
        let base_path = self.prepare_base_path()?;
        self.base_path = Some(base_path.clone());
        Ok(base_path)
    }

    /// `getBaseUrl()`: with a trusted proxy's prefix.
    pub fn get_base_url(&mut self) -> Result<PhpString> {
        let mut prefix = Vec::new();
        if self.is_from_trusted_proxy()? {
            let values = self.get_trusted_values(Self::HEADER_X_FORWARDED_PREFIX, None)?;
            if let Some(first) = values.first() {
                prefix = rtrim(string_of(first)?.as_bytes(), b"/").to_vec();
            }
        }
        let real = self.get_base_url_real()?;
        Ok(concat(&[&prefix, real.as_bytes()]))
    }

    fn get_base_url_real(&mut self) -> Result<PhpString> {
        if let Some(base_url) = &self.base_url {
            return Ok(base_url.clone());
        }
        let base_url = self.prepare_base_url()?;
        self.base_url = Some(base_url.clone());
        Ok(base_url)
    }

    /// `getScheme()`.
    pub fn get_scheme(&mut self) -> Result<&'static str> {
        Ok(if self.is_secure()? { "https" } else { "http" })
    }

    /// `getPort()`: an int, or `SERVER_PORT` as the server bag holds it,
    /// coerced to the `int|string|null` return type ([`port_return`]).
    pub fn get_port(&mut self) -> Result<Value> {
        let host = if let Some(first) = self.trusted_first(Self::HEADER_X_FORWARDED_PORT)? {
            first
        } else if let Some(first) = self.trusted_first(Self::HEADER_X_FORWARDED_HOST)? {
            first
        } else {
            match self.headers.get("HOST", None) {
                Some(host) if truthy(host.as_bytes()) => host,
                _ => return port_return(self.server.get("SERVER_PORT", Value::Null)),
            }
        };
        let host = host.as_bytes();
        let pos = if host.first() == Some(&b'[') {
            let from = host.iter().rposition(|&c| c == b']').unwrap_or(0);
            host[from..]
                .iter()
                .position(|&c| c == b':')
                .map(|p| p + from)
        } else {
            host.iter().rposition(|&c| c == b':')
        };
        if let Some(pos) = pos {
            let port = &host[pos + 1..];
            if truthy(port) {
                return Ok(Value::Int(string_to_int(port)));
            }
        }
        Ok(Value::Int(if self.get_scheme()? == "https" {
            443
        } else {
            80
        }))
    }

    /// The first trusted value of a kind, when the request comes from a
    /// trusted proxy and there is one.
    fn trusted_first(&mut self, kind: i64) -> Result<Option<PhpString>> {
        if !self.is_from_trusted_proxy()? {
            return Ok(None);
        }
        match self.get_trusted_values(kind, None)?.first() {
            Some(first) => string_of(first).map(Some),
            None => Ok(None),
        }
    }

    /// `getUser()`.
    pub fn get_user(&self) -> Option<PhpString> {
        self.headers.get("PHP_AUTH_USER", None)
    }

    /// `getPassword()`.
    pub fn get_password(&self) -> Option<PhpString> {
        self.headers.get("PHP_AUTH_PW", None)
    }

    /// `getUserInfo()`.
    pub fn get_user_info(&self) -> Option<PhpString> {
        let user = self.get_user();
        match self.get_password() {
            Some(pass) if !pass.is_empty() => Some(concat(&[
                user.as_ref().map(PhpString::as_bytes).unwrap_or_default(),
                b":",
                pass.as_bytes(),
            ])),
            _ => user,
        }
    }

    /// `getHttpHost()`: the host, with the port when it is not the
    /// scheme's.
    pub fn get_http_host(&mut self) -> Result<PhpString> {
        let scheme = self.get_scheme()?;
        let port = self.get_port()?;
        let standard = match scheme {
            "http" => loose_equals(&Value::Int(80), &port),
            _ => loose_equals(&Value::Int(443), &port),
        };
        let host = self.get_host()?;
        if standard {
            return Ok(host);
        }
        Ok(concat(&[
            host.as_bytes(),
            b":",
            string_of(&port)?.as_bytes(),
        ]))
    }

    /// `getRequestUri()`: the raw path and query string.
    pub fn get_request_uri(&mut self) -> Result<PhpString> {
        if let Some(uri) = &self.request_uri {
            return Ok(uri.clone());
        }
        let uri = self.prepare_request_uri()?;
        self.request_uri = Some(uri.clone());
        Ok(uri)
    }

    /// `getSchemeAndHttpHost()`.
    pub fn get_scheme_and_http_host(&mut self) -> Result<PhpString> {
        let scheme = self.get_scheme()?;
        let host = self.get_http_host()?;
        Ok(concat(&[scheme.as_bytes(), b"://", host.as_bytes()]))
    }

    /// `getUri()`: a normalized URI for the request.
    pub fn get_uri(&mut self) -> Result<PhpString> {
        let qs = self
            .get_query_string()?
            .map(|qs| concat(&[b"?", qs.as_bytes()]))
            .unwrap_or_default();
        let start = self.get_scheme_and_http_host()?;
        let base_url = self.get_base_url()?;
        let path_info = self.get_path_info()?;
        Ok(concat(&[
            start.as_bytes(),
            base_url.as_bytes(),
            path_info.as_bytes(),
            qs.as_bytes(),
        ]))
    }

    /// `getUriForPath($path)`.
    pub fn get_uri_for_path(&mut self, path: &[u8]) -> Result<PhpString> {
        let start = self.get_scheme_and_http_host()?;
        let base_url = self.get_base_url()?;
        Ok(concat(&[start.as_bytes(), base_url.as_bytes(), path]))
    }

    /// `getRelativeUriForPath($path)`.
    pub fn get_relative_uri_for_path(&mut self, path: &[u8]) -> Result<PhpString> {
        if path.first() != Some(&b'/') {
            return Ok(PhpString::from(path));
        }
        let base_path = self.get_path_info()?;
        let base_path = base_path.as_bytes();
        if path == base_path {
            return Ok(PhpString::default());
        }
        let source = base_path.strip_prefix(b"/").unwrap_or(base_path);
        let mut source_dirs: Vec<&[u8]> = source.split(|&c| c == b'/').collect();
        let mut target_dirs: Vec<&[u8]> = path[1..].split(|&c| c == b'/').collect();
        source_dirs.pop();
        let target_file = target_dirs.pop().unwrap_or_default();
        let common = source_dirs
            .iter()
            .zip(&target_dirs)
            .take_while(|(a, b)| a == b)
            .count();
        let ups = source_dirs.len() - common;
        let mut rest: Vec<&[u8]> = target_dirs[common..].to_vec();
        rest.push(target_file);
        let mut out = b"../".repeat(ups);
        out.extend_from_slice(&rest.join(&b'/'));
        let colon = out.iter().position(|&c| c == b':');
        let slash = out.iter().position(|&c| c == b'/');
        let needs_dot = out.first().is_none_or(|&c| c == b'/')
            || colon.is_some_and(|c| slash.is_none_or(|s| c < s));
        Ok(PhpString::from(if needs_dot {
            [b"./", out.as_slice()].concat()
        } else {
            out
        }))
    }

    /// `getQueryString()`: normalized, or `None` when empty.
    pub fn get_query_string(&self) -> Result<Option<PhpString>> {
        let qs = nullable_string_of(&self.server.get("QUERY_STRING", Value::Null))?;
        let qs = Self::normalize_query_string(qs.as_ref().map(PhpString::as_bytes))?;
        Ok((!qs.is_empty()).then_some(qs))
    }

    /// `isSecure()`.
    pub fn is_secure(&mut self) -> Result<bool> {
        if let Some(proto) = self.trusted_first(Self::HEADER_X_FORWARDED_PROTO)? {
            return Ok(matches!(
                strtolower(proto.as_bytes()).as_slice(),
                b"https" | b"on" | b"ssl" | b"1"
            ));
        }
        let https = self.server.get("HTTPS", Value::Null);
        Ok(https.is_truthy()
            && match &https {
                Value::String(s) => strtolower(s.as_bytes()) != b"off",
                _ => true,
            })
    }

    /// `getHost()`: lowercase, without the port, checked against the
    /// trusted host patterns.
    pub fn get_host(&mut self) -> Result<PhpString> {
        let host = if let Some(first) = self.trusted_first(Self::HEADER_X_FORWARDED_HOST)? {
            first
        } else {
            match self.headers.get("HOST", None) {
                Some(h) if truthy(h.as_bytes()) => h,
                _ => {
                    let name = self.server.get("SERVER_NAME", Value::Null);
                    if name.is_truthy() {
                        string_of(&name)?
                    } else {
                        string_of(&self.server.get("SERVER_ADDR", Value::string("")))?
                    }
                }
            }
        };
        let host = php_value::string::trim(host.as_bytes());
        // preg_replace('/:\d+$/', '', ...): trim() left no final newline
        let digits = host.iter().rev().take_while(|c| c.is_ascii_digit()).count();
        let host = if digits > 0 && host.len() > digits && host[host.len() - digits - 1] == b':' {
            &host[..host.len() - digits - 1]
        } else {
            host
        };
        let host = strtolower(host);

        if truthy(&host) && !is_host_valid(&host)? {
            if !self.is_host_valid {
                return Ok(PhpString::default());
            }
            self.is_host_valid = false;
            return Err(Error::new(
                Class::SuspiciousOperation,
                concat(&[b"Invalid Host \"", &host, b"\"."]),
            ));
        }

        let trusted = statics(|s| {
            if s.trusted_hosts_literals.is_empty() && s.trusted_hosts_regexps.is_empty() {
                return None;
            }
            Some(
                s.trusted_hosts_literals.contains(&host)
                    || s.trusted_hosts_regexps
                        .iter()
                        .any(|(_, regex)| host_matches(regex, &host)),
            )
        });
        match trusted {
            None | Some(true) => Ok(PhpString::from(host)),
            Some(false) => {
                if !self.is_host_valid {
                    return Ok(PhpString::default());
                }
                self.is_host_valid = false;
                Err(Error::new(
                    Class::SuspiciousOperation,
                    concat(&[b"Untrusted Host \"", &host, b"\"."]),
                ))
            }
        }
    }

    /// `setMethod($method)`.
    pub fn set_method(&mut self, method: impl Into<PhpString>) {
        self.method = None;
        self.server
            .set("REQUEST_METHOD", Value::String(method.into()));
    }

    /// `getMethod()`: the intended method, `X-HTTP-Method-Override` or
    /// `_method` applied to a POST.
    pub fn get_method(&mut self) -> Result<PhpString> {
        if let Some(method) = &self.method {
            return Ok(method.clone());
        }
        let method = PhpString::from(strtoupper(
            string_of(&self.server.get("REQUEST_METHOD", Value::string("GET")))?.as_bytes(),
        ));
        self.method = Some(method.clone());
        let allowed = Self::get_allowed_http_method_override();
        if method.as_bytes() != b"POST" || allowed.as_ref().is_some_and(Vec::is_empty) {
            return Ok(method);
        }
        let mut overridden = self
            .headers
            .get("X-HTTP-METHOD-OVERRIDE", None)
            .map(Value::String)
            .unwrap_or(Value::Null);
        if !overridden.is_truthy() && Self::get_http_method_parameter_override() {
            let default = self.query.get("_method", Value::string("POST"))?;
            overridden = self.request.get("_method", default)?;
        }
        let Value::String(overridden) = overridden else {
            return Ok(method);
        };
        let overridden = strtoupper(overridden.as_bytes());
        if matches!(
            overridden.as_slice(),
            b"GET" | b"HEAD" | b"CONNECT" | b"TRACE"
        ) {
            return Ok(method);
        }
        if let Some(allowed) = &allowed
            && !allowed.iter().any(|m| m.as_bytes() == overridden)
        {
            return Ok(method);
        }
        if !overridden.iter().all(u8::is_ascii_uppercase) {
            return Err(Error::new(
                Class::SuspiciousOperation,
                "Invalid HTTP method override.",
            ));
        }
        let overridden = PhpString::from(overridden);
        self.method = Some(overridden.clone());
        Ok(overridden)
    }

    /// `getRealMethod()`.
    pub fn get_real_method(&self) -> Result<PhpString> {
        Ok(PhpString::from(strtoupper(
            string_of(&self.server.get("REQUEST_METHOD", Value::string("GET")))?.as_bytes(),
        )))
    }

    /// `getMimeType($format)`.
    pub fn get_mime_type(&self, format: &[u8]) -> Option<PhpString> {
        Self::get_mime_types(format).into_iter().next()
    }

    /// `getMimeTypes($format)`.
    pub fn get_mime_types(format: &[u8]) -> Vec<PhpString> {
        statics(|s| {
            initialize_formats(s)
                .iter()
                .find(|(f, _)| f.as_bytes() == format)
                .map(|(_, types)| types.clone())
                .unwrap_or_default()
        })
    }

    /// `getFormat($mimeType, $subtypeFallback)`.
    pub fn get_format(
        &self,
        mime_type: Option<&[u8]>,
        subtype_fallback: bool,
    ) -> Option<PhpString> {
        let mut canonical: Option<Vec<u8>> = None;
        if let Some(mime) = mime_type.filter(|m| truthy(m))
            && let Some(pos) = mime.iter().position(|&c| c == b';')
        {
            canonical = Some(php_value::string::trim(&mime[..pos]).to_vec());
        }
        let (exact, canonical_format) = statics(|s| {
            let mut exact = None;
            let mut canonical_format = None;
            for (format, types) in initialize_formats(s).iter() {
                if mime_type.is_some_and(|m| types.iter().any(|t| t.as_bytes() == m)) {
                    exact = Some(format.clone());
                }
                if let Some(c) = &canonical
                    && types.iter().any(|t| t.as_bytes() == c.as_slice())
                {
                    canonical_format = Some(format.clone());
                }
            }
            (exact, canonical_format)
        });
        if let Some(format) = exact.or(canonical_format)
            && truthy(format.as_bytes())
        {
            return Some(format);
        }
        let canonical = canonical.or_else(|| mime_type.map(<[u8]>::to_vec))?;
        if !truthy(&canonical) {
            return None;
        }
        if canonical.starts_with(b"application/")
            && let Some(plus) = canonical.iter().rposition(|&c| c == b'+')
        {
            let suffix = &canonical[plus + 1..];
            if let Some((_, format)) = STRUCTURED_SUFFIX_FORMATS
                .iter()
                .find(|(s, _)| s.as_bytes() == suffix)
            {
                return Some(PhpString::from(*format));
            }
        }
        if subtype_fallback && let Some(slash) = canonical.iter().position(|&c| c == b'/') {
            let mut subtype = &canonical[slash + 1..];
            if let Some(rest) = subtype.strip_prefix(b"x-") {
                subtype = rest;
            }
            if !subtype.contains(&b'+') {
                return Some(PhpString::from(subtype));
            }
        }
        None
    }

    /// `setFormat($format, $mimeTypes)`.
    pub fn set_format<P: Into<PhpString>>(
        &self,
        format: impl Into<PhpString>,
        mime_types: impl IntoIterator<Item = P>,
    ) {
        let format = format.into();
        let types: Vec<PhpString> = mime_types.into_iter().map(Into::into).collect();
        statics(|s| {
            let formats = initialize_formats(s);
            match formats.iter_mut().find(|(f, _)| *f == format) {
                Some((_, existing)) => *existing = types,
                None => formats.push((format, types)),
            }
        });
    }

    /// `getRequestFormat($default)`: the format set, else the `_format`
    /// attribute, else `default`.
    pub fn get_request_format(&mut self, default: Option<&str>) -> Result<Option<PhpString>> {
        if self.format.is_none() {
            let attribute = self.attributes.get("_format", Value::Null);
            if let Value::Array(_) = attribute {
                return Err(Error::new(
                    Class::PhpTypeError,
                    format!("Cannot assign array to property {CLASS}::$format of type ?string"),
                ));
            }
            self.format = nullable_string_of(&attribute)?;
        }
        Ok(self.format.clone().or_else(|| default.map(PhpString::from)))
    }

    /// `setRequestFormat($format)`.
    pub fn set_request_format(&mut self, format: Option<PhpString>) {
        self.format = format;
    }

    /// `getContentTypeFormat()`.
    pub fn get_content_type_format(&self) -> Option<PhpString> {
        let content_type = self.headers.get("CONTENT_TYPE", Some(""));
        self.get_format(content_type.as_ref().map(PhpString::as_bytes), false)
    }

    /// `setDefaultLocale($locale)`.
    pub fn set_default_locale(&mut self, locale: impl Into<PhpString>) {
        self.default_locale = locale.into();
    }

    /// `getDefaultLocale()`.
    pub fn get_default_locale(&self) -> PhpString {
        self.default_locale.clone()
    }

    /// `setLocale($locale)`.
    pub fn set_locale(&mut self, locale: impl Into<PhpString>) {
        self.locale = Some(locale.into());
    }

    /// `getLocale()`.
    pub fn get_locale(&self) -> PhpString {
        self.locale
            .clone()
            .unwrap_or_else(|| self.default_locale.clone())
    }

    /// `isMethod($method)`.
    pub fn is_method(&mut self, method: &str) -> Result<bool> {
        Ok(self.get_method()?.as_bytes() == strtoupper(method.as_bytes()))
    }

    /// `isMethodSafe()`.
    pub fn is_method_safe(&mut self) -> Result<bool> {
        Ok(matches!(
            self.get_method()?.as_bytes(),
            b"GET" | b"HEAD" | b"OPTIONS" | b"TRACE" | b"QUERY"
        ))
    }

    /// `isMethodIdempotent()`.
    pub fn is_method_idempotent(&mut self) -> Result<bool> {
        Ok(matches!(
            self.get_method()?.as_bytes(),
            b"HEAD" | b"GET" | b"PUT" | b"DELETE" | b"TRACE" | b"OPTIONS" | b"PURGE" | b"QUERY"
        ))
    }

    /// `isMethodCacheable()`.
    pub fn is_method_cacheable(&mut self) -> Result<bool> {
        Ok(matches!(
            self.get_method()?.as_bytes(),
            b"GET" | b"HEAD" | b"QUERY"
        ))
    }

    /// `getProtocolVersion()`: a trusted proxy's `Via` version, else
    /// `SERVER_PROTOCOL`.
    pub fn get_protocol_version(&self) -> Result<Option<PhpString>> {
        if self.is_from_trusted_proxy()? {
            // preg_match('~^(HTTP/)?([1-9]\.[0-9])\b~', $via)
            let via = self.headers.get("Via", None).unwrap_or_default();
            let via = via.as_bytes();
            for start in [via.strip_prefix(b"HTTP/"), Some(via)]
                .into_iter()
                .flatten()
            {
                if let [major @ b'1'..=b'9', b'.', minor @ b'0'..=b'9', rest @ ..] = start
                    && rest
                        .first()
                        .is_none_or(|c| !(c.is_ascii_alphanumeric() || *c == b'_'))
                {
                    return Ok(Some(concat(&[b"HTTP/", &[*major, b'.', *minor]])));
                }
            }
        }
        nullable_string_of(&self.server.get("SERVER_PROTOCOL", Value::Null))
    }

    /// `getContent($asResource)`: the body, read from `php://input` when
    /// none was given.
    pub fn get_content(&mut self, as_resource: bool) -> Result<ContentValue> {
        let input = || with_globals(|g| g.input.clone());
        if as_resource {
            return Ok(ContentValue::Resource(match &self.content {
                Body::Resource(stream) => {
                    stream.rewind();
                    stream.clone()
                }
                Body::String(s) => Stream::new(s.as_bytes()),
                Body::Null | Body::False => {
                    self.content = Body::False;
                    Stream::new(&*input())
                }
            }));
        }
        Ok(ContentValue::String(match &self.content {
            Body::Resource(stream) => {
                stream.rewind();
                PhpString::from(stream.contents())
            }
            Body::String(s) => s.clone(),
            Body::Null | Body::False => {
                let s = PhpString::from(&*input());
                self.content = Body::String(s.clone());
                s
            }
        }))
    }

    fn content_string(&mut self) -> Result<PhpString> {
        match self.get_content(false)? {
            ContentValue::String(s) => Ok(s),
            ContentValue::Resource(r) => Ok(PhpString::from(r.contents())),
        }
    }

    fn decode_json(content: &[u8]) -> Result<PhpArray> {
        let value = decode_assoc(content, 512, true).map_err(|e| {
            Error::with_previous(
                Class::Json,
                "Could not decode request body.",
                e.code(),
                Error::new(Class::PhpJson, e.message()).with_code(e.code()),
            )
        })?;
        match value {
            Value::Array(array) => Ok(array),
            other => Err(Error::new(
                Class::Json,
                format!(
                    "JSON content was expected to decode to an array, \"{}\" returned.",
                    json_type(&other)
                ),
            )),
        }
    }

    /// `getPayload()`: the form fields, else the JSON body.
    pub fn get_payload(&mut self) -> Result<InputBag> {
        if self.request.count() > 0 {
            return Ok(self.request.clone());
        }
        let content = self.content_string()?;
        if content.is_empty() {
            return Ok(InputBag::new(PhpArray::new()));
        }
        Ok(InputBag::new(Self::decode_json(content.as_bytes())?))
    }

    /// `toArray()`: the JSON body.
    pub fn to_array(&mut self) -> Result<PhpArray> {
        let content = self.content_string()?;
        if content.is_empty() {
            return Err(Error::new(Class::Json, "Request body is empty."));
        }
        Self::decode_json(content.as_bytes())
    }

    /// `getETags()`: `If-None-Match` split on commas and the whitespace
    /// around them (`preg_split('/\s*,\s*/', ..., PREG_SPLIT_NO_EMPTY)`).
    pub fn get_etags(&self) -> Vec<PhpString> {
        let header = self
            .headers
            .get("If-None-Match", Some(""))
            .unwrap_or_default();
        let space = |c: &u8| matches!(c, b' ' | b'\t' | b'\n' | 0x0B | 0x0C | b'\r');
        let pieces: Vec<&[u8]> = header.as_bytes().split(|&c| c == b',').collect();
        let last = pieces.len() - 1;
        pieces
            .iter()
            .enumerate()
            .map(|(i, piece)| {
                let mut piece: &[u8] = piece;
                if i > 0 {
                    piece = &piece[piece.iter().take_while(|c| space(c)).count()..];
                }
                if i < last {
                    piece =
                        &piece[..piece.len() - piece.iter().rev().take_while(|c| space(c)).count()];
                }
                piece
            })
            .filter(|p| !p.is_empty())
            .map(PhpString::from)
            .collect()
    }

    /// `isNoCache()`.
    pub fn is_no_cache(&self) -> bool {
        self.headers.has_cache_control_directive("no-cache")
            || self
                .headers
                .get("Pragma", None)
                .is_some_and(|p| p.as_bytes() == b"no-cache")
    }

    /// `getPreferredFormat($default)`: the request format, else the first
    /// acceptable content type with a format.
    pub fn get_preferred_format(&mut self, default: Option<&str>) -> Result<Option<PhpString>> {
        if self.preferred_format.is_none()
            && let Some(format) = self.get_request_format(None)?
        {
            self.preferred_format = Some(format);
        }
        if let Some(format) = &self.preferred_format
            && truthy(format.as_bytes())
        {
            return Ok(Some(format.clone()));
        }
        for mime in self.get_acceptable_content_types()? {
            self.preferred_format = self.get_format(Some(mime.as_bytes()), false);
            if let Some(format) = &self.preferred_format
                && truthy(format.as_bytes())
            {
                return Ok(Some(format.clone()));
            }
        }
        Ok(default.map(PhpString::from))
    }

    /// `getPreferredLanguage($locales)`.
    pub fn get_preferred_language(
        &mut self,
        locales: Option<&[&str]>,
    ) -> Result<Option<PhpString>> {
        let preferred = self.get_languages()?;
        let Some(locales) = locales.filter(|l| !l.is_empty()) else {
            return preferred.first().map(|(_, v)| v).map(string_of).transpose();
        };
        let locales: Vec<Vec<u8>> = locales
            .iter()
            .map(|l| format_locale(l.as_bytes()))
            .collect::<Result<_>>()?;
        if preferred.is_empty() {
            return Ok(Some(PhpString::from(locales[0].clone())));
        }
        let mut combinations = Vec::new();
        for (_, language) in preferred.iter() {
            combinations.extend(language_combinations(string_of(language)?.as_bytes())?);
        }
        for combination in &combinations {
            for locale in &locales {
                if locale == combination
                    || locale.starts_with(&[combination.as_slice(), b"_"].concat())
                {
                    return Ok(Some(PhpString::from(locale.clone())));
                }
            }
        }
        Ok(Some(PhpString::from(locales[0].clone())))
    }

    /// `getLanguages()`: the `Accept-Language` languages, canonical, in
    /// preference order, as `array_unique()` leaves them (a later
    /// duplicate removed with its key).
    pub fn get_languages(&mut self) -> Result<PhpArray> {
        if let Some(languages) = &self.languages {
            return Ok(languages.clone());
        }
        let header = self.headers.get("Accept-Language", None);
        let mut accept = AcceptHeader::from_string(header.as_ref().map(PhpString::as_bytes));
        let mut languages = PhpArray::new();
        let mut seen: Vec<Vec<u8>> = Vec::new();
        for (index, (_, item)) in accept.all().iter().enumerate() {
            let language = format_locale(item.value())?;
            if !seen.contains(&language) {
                seen.push(language.clone());
                languages.insert(
                    i64::try_from(index).unwrap_or(i64::MAX),
                    Value::string(language),
                );
            }
        }
        self.languages = Some(languages.clone());
        Ok(languages)
    }

    fn accept_keys(&self, header: &str) -> Vec<PhpString> {
        let value = self.headers.get(header, None);
        let mut accept = AcceptHeader::from_string(value.as_ref().map(PhpString::as_bytes));
        accept
            .all()
            .iter()
            .map(|(key, _)| PhpString::from(key.bytes().into_owned()))
            .collect()
    }

    /// `getCharsets()`.
    pub fn get_charsets(&mut self) -> Vec<PhpString> {
        if self.charsets.is_none() {
            self.charsets = Some(self.accept_keys("Accept-Charset"));
        }
        self.charsets.clone().unwrap_or_default()
    }

    /// `getEncodings()`.
    pub fn get_encodings(&mut self) -> Vec<PhpString> {
        if self.encodings.is_none() {
            self.encodings = Some(self.accept_keys("Accept-Encoding"));
        }
        self.encodings.clone().unwrap_or_default()
    }

    /// `getAcceptableContentTypes()`.
    pub fn get_acceptable_content_types(&mut self) -> Result<Vec<PhpString>> {
        if self.acceptable_content_types.is_none() {
            self.acceptable_content_types = Some(self.accept_keys("Accept"));
        }
        Ok(self.acceptable_content_types.clone().unwrap_or_default())
    }

    /// `isXmlHttpRequest()`.
    pub fn is_xml_http_request(&self) -> bool {
        self.headers
            .get("X-Requested-With", None)
            .is_some_and(|v| v.as_bytes() == b"XMLHttpRequest")
    }

    /// `preferSafeContent()` (RFC 8674).
    pub fn prefer_safe_content(&mut self) -> Result<bool> {
        if let Some(preferred) = self.is_safe_content_preferred {
            return Ok(preferred);
        }
        let preferred = self.is_secure()? && {
            let prefer = self.headers.get("Prefer", None);
            AcceptHeader::from_string(prefer.as_ref().map(PhpString::as_bytes)).has(b"safe")?
        };
        self.is_safe_content_preferred = Some(preferred);
        Ok(preferred)
    }

    fn server_string(&self, key: &str) -> Result<PhpString> {
        string_of(&self.server.get(key, Value::string("")))
    }

    fn prepare_request_uri(&mut self) -> Result<PhpString> {
        let mut request_uri = Vec::new();
        let unencoded = self.server.get("UNENCODED_URL", Value::Null);
        if self.is_iis_rewrite()? && !loose_equals(&Value::string(""), &unencoded) {
            request_uri = string_of(&unencoded)?.as_bytes().to_vec();
            self.server.remove("UNENCODED_URL");
        } else if self.server.has("REQUEST_URI") {
            request_uri = string_of(&self.server.get("REQUEST_URI", Value::Null))?
                .as_bytes()
                .to_vec();
            if request_uri.first() == Some(&b'/') {
                if let Some(pos) = request_uri.iter().position(|&c| c == b'#') {
                    request_uri.truncate(pos);
                }
            } else if let Some(components) = parse_url(&request_uri.clone()) {
                if let Some(path) = components.path {
                    request_uri = path;
                }
                if let Some(query) = components.query {
                    request_uri.push(b'?');
                    request_uri.extend_from_slice(&query);
                }
            }
        } else if self.server.has("ORIG_PATH_INFO") {
            request_uri = string_of(&self.server.get("ORIG_PATH_INFO", Value::Null))?
                .as_bytes()
                .to_vec();
            let query_string = self.server.get("QUERY_STRING", Value::Null);
            if !loose_equals(&Value::string(""), &query_string) {
                request_uri.push(b'?');
                request_uri.extend_from_slice(string_of(&query_string)?.as_bytes());
            }
            self.server.remove("ORIG_PATH_INFO");
        }
        // the normalized URI eases creating sub-requests from this one
        self.server
            .set("REQUEST_URI", Value::string(request_uri.clone()));
        Ok(PhpString::from(request_uri))
    }

    fn prepare_base_url(&mut self) -> Result<PhpString> {
        let script_filename = self.server_string("SCRIPT_FILENAME")?;
        let filename = basename(script_filename.as_bytes()).to_vec();
        let mut base_url: Vec<u8> = Vec::new();
        let mut found = false;
        for key in ["SCRIPT_NAME", "PHP_SELF", "ORIG_SCRIPT_NAME"] {
            if basename(self.server_string(key)?.as_bytes()) == filename.as_slice() {
                base_url = nullable_string_of(&self.server.get(key, Value::Null))?
                    .map(|s| s.as_bytes().to_vec())
                    .unwrap_or_default();
                found = true;
                break;
            }
        }
        if !found {
            // backtrack up the script filename to the part PHP_SELF has
            let path = self.server_string("PHP_SELF")?;
            let file = script_filename;
            let segments: Vec<&[u8]> = trim(file.as_bytes(), b"/")
                .split(|&c| c == b'/')
                .rev()
                .collect();
            let last = segments.len();
            let mut index = 0;
            loop {
                base_url = [b"/", segments[index], base_url.as_slice()].concat();
                index += 1;
                let more =
                    last > index && find(path.as_bytes(), &base_url).is_some_and(|pos| pos != 0);
                if !more {
                    break;
                }
            }
        }

        let mut request_uri = self.get_request_uri()?.as_bytes().to_vec();
        if !request_uri.is_empty() && request_uri[0] != b'/' {
            request_uri.insert(0, b'/');
        }
        if request_uri.starts_with(b"//") || request_uri.starts_with(b"/\\") {
            // the URI starts with a host that the client chose: URLs built
            // on it would point there
            return Ok(PhpString::default());
        }
        let segment_end = |at: usize| matches!(request_uri.get(at), None | Some(b'/' | b'?'));

        if truthy(&base_url)
            && let Some(prefix) = self.get_urlencoded_prefix(&request_uri, &base_url)?
            && segment_end(prefix.len())
        {
            // the full base URL matches, and is not only the start of a
            // longer path segment
            return Ok(PhpString::from(prefix));
        }

        if truthy(&base_url) {
            let mut dir = rtrim(&dirname(&base_url), b"/").to_vec();
            dir.push(b'/');
            if let Some(prefix) = self.get_urlencoded_prefix(&request_uri, &dir)? {
                // the directory part of the base URL matches
                return Ok(PhpString::from(rtrim(&prefix, b"/")));
            }
        }

        let truncated = match request_uri.iter().position(|&c| c == b'?') {
            Some(pos) => request_uri[..pos].to_vec(),
            None => request_uri.clone(),
        };

        // with mod_rewrite or ISAPI_Rewrite, the script filename can come
        // after a prefix in the path: the base URL then ends with it
        let decoded = rawurldecode(&truncated);
        let pos = if truthy(&base_url) {
            if self.is_iis_rewrite()? {
                find_ignore_case(&decoded, &base_url)
            } else {
                find(&decoded, &base_url)
            }
        } else {
            None
        };
        let Some(pos) = pos.filter(|&p| p != 0) else {
            // the script filename isn't in the path, or only at its start,
            // which is handled above
            return Ok(PhpString::default());
        };
        let base_url = self
            .get_urlencoded_prefix(&truncated, &decoded[..pos + base_url.len()])?
            .unwrap_or_default();
        if !segment_end(base_url.len()) {
            // the script filename is only the start of a longer segment
            return Ok(PhpString::default());
        }
        Ok(PhpString::from(rtrim(&base_url, b"/")))
    }

    fn prepare_base_path(&mut self) -> Result<PhpString> {
        let base_url = self.get_base_url()?;
        if !truthy(base_url.as_bytes()) {
            return Ok(PhpString::default());
        }
        let filename =
            basename(string_of(&self.server.get("SCRIPT_FILENAME", Value::Null))?.as_bytes())
                .to_vec();
        let base_path = if basename(base_url.as_bytes()) == filename.as_slice() {
            dirname(base_url.as_bytes())
        } else {
            base_url.as_bytes().to_vec()
        };
        Ok(PhpString::from(rtrim(&base_path, b"/")))
    }

    fn prepare_path_info(&mut self) -> Result<PhpString> {
        let request_uri = self.get_request_uri()?;
        let mut request_uri = request_uri.as_bytes().to_vec();
        if let Some(pos) = request_uri.iter().position(|&c| c == b'?') {
            request_uri.truncate(pos);
        }
        if !request_uri.is_empty() && request_uri[0] != b'/' {
            request_uri.insert(0, b'/');
        }
        let base_url = self.get_base_url_real()?;
        let path_info = request_uri.get(base_url.len()..).unwrap_or_default();
        if path_info.first() != Some(&b'/') {
            return Ok(concat(&[b"/", path_info]));
        }
        Ok(PhpString::from(path_info))
    }

    /// The private `getUrlencodedPrefix($string, $prefix)`: the start of
    /// `string`, as encoded there, when it decodes to start with `prefix`.
    #[doc(hidden)]
    pub fn get_urlencoded_prefix(
        &mut self,
        string: &[u8],
        prefix: &[u8],
    ) -> Result<Option<Vec<u8>>> {
        let decoded = rawurldecode(string);
        let starts = if self.is_iis_rewrite()? {
            decoded.len() >= prefix.len() && decoded[..prefix.len()].eq_ignore_ascii_case(prefix)
        } else {
            decoded.starts_with(prefix)
        };
        if !starts {
            return Ok(None);
        }
        // each byte of the prefix takes 1 or 3 characters in the string
        // (preg_match_all('#%[[:xdigit:]]{2}|.#s', ...))
        let window = &string[..string.len().min(prefix.len().saturating_mul(3))];
        let mut out = Vec::new();
        let mut i = 0;
        let mut taken = 0;
        while i < window.len() && taken < prefix.len() {
            let len = if window[i] == b'%'
                && window.get(i + 1).is_some_and(u8::is_ascii_hexdigit)
                && window.get(i + 2).is_some_and(u8::is_ascii_hexdigit)
            {
                3
            } else {
                1
            };
            out.extend_from_slice(&window[i..i + len]);
            i += len;
            taken += 1;
        }
        Ok(Some(out))
    }

    /// `isFromTrustedProxy()`.
    pub fn is_from_trusted_proxy(&self) -> Result<bool> {
        let proxies = Self::get_trusted_proxies();
        if proxies.is_empty() {
            return Ok(false);
        }
        let remote = self.server.get("REMOTE_ADDR", Value::string(""));
        let remote = match &remote {
            Value::Null | Value::Array(_) => {
                return Err(Error::new(
                    Class::PhpTypeError,
                    format!(
                        "Symfony\\Component\\HttpFoundation\\IpUtils::checkIp(): Argument #1 ($requestIp) must be of type string, {} given",
                        remote.debug_type()
                    ),
                ));
            }
            other => string_of(other)?,
        };
        check_ip(remote.as_bytes(), &proxies)
    }

    fn get_trusted_values(&mut self, kind: i64, ip: Option<&PhpString>) -> Result<Vec<Value>> {
        let set = Self::get_trusted_header_set();
        let header = Self::trusted_header(kind);
        let own = if set & kind != 0 {
            self.headers.get(header, None).unwrap_or_default()
        } else {
            PhpString::default()
        };
        let forwarded_header = self.headers.get("FORWARDED", None).unwrap_or_default();
        let cache_key = [
            kind.to_string().as_bytes(),
            b"\0",
            own.as_bytes(),
            b"\0",
            ip.map(PhpString::as_bytes).unwrap_or_default(),
            b"\0",
            forwarded_header.as_bytes(),
        ]
        .concat();
        if let Some(values) = self.trusted_values_cache.get(&cache_key) {
            return Ok(values.clone());
        }

        let mut client: Vec<Value> = Vec::new();
        let mut forwarded: Vec<Value> = Vec::new();
        if set & kind != 0 && self.headers.has(header) {
            let value = self.headers.get(header, None).unwrap_or_default();
            for v in value.as_bytes().split(|&c| c == b',') {
                let prefix: &[u8] = if kind == Self::HEADER_X_FORWARDED_PORT {
                    b"0.0.0.0:"
                } else {
                    b""
                };
                client.push(Value::String(concat(&[prefix, php_value::string::trim(v)])));
            }
        }
        if set & Self::HEADER_FORWARDED != 0
            && let Some(param) = Self::forwarded_param(kind)
            && self.headers.has("FORWARDED")
        {
            let parts = header_utils::split(forwarded_header.as_bytes(), b",;=")?;
            for sub_parts in parts.values() {
                let Value::Array(sub_parts) = sub_parts else {
                    continue;
                };
                let combined = header_utils::combine(sub_parts)?;
                let Some(v) = combined.get(&Key::from(param)).filter(|v| !v.is_null()) else {
                    continue;
                };
                let mut v = v.clone();
                if kind == Self::HEADER_X_FORWARDED_PORT {
                    let text = string_of(&v)?;
                    let text = text.as_bytes();
                    let port: Vec<u8> = match text.iter().rposition(|&c| c == b':') {
                        Some(colon) if !text.ends_with(b"]") => text[colon..].to_vec(),
                        _ => {
                            if self.is_secure()? {
                                b":443".to_vec()
                            } else {
                                b":80".to_vec()
                            }
                        }
                    };
                    v = Value::String(concat(&[b"0.0.0.0", &port]));
                }
                forwarded.push(v);
            }
        }

        if let Some(ip) = ip {
            client = Self::normalize_and_filter_client_ips(client, ip)?;
            forwarded = Self::normalize_and_filter_client_ips(forwarded, ip)?;
        }

        let same = forwarded.len() == client.len()
            && forwarded.iter().zip(&client).all(|(a, b)| a.identical(b));
        let values = if same || client.is_empty() {
            forwarded
        } else if forwarded.is_empty() {
            client
        } else if !self.is_forwarded_valid {
            match ip {
                Some(ip) => vec![Value::string("0.0.0.0"), Value::String(ip.clone())],
                None => Vec::new(),
            }
        } else {
            self.is_forwarded_valid = false;
            return Err(Error::new(
                Class::ConflictingHeaders,
                format!(
                    "The request has both a trusted \"FORWARDED\" header and a trusted \"{header}\" header, conflicting with each other. You should either configure your proxy to remove one of them, or configure your project to distrust the offending one."
                ),
            ));
        };
        self.trusted_values_cache.insert(cache_key, values.clone());
        Ok(values)
    }

    fn normalize_and_filter_client_ips(
        client_ips: Vec<Value>,
        ip: &PhpString,
    ) -> Result<Vec<Value>> {
        if client_ips.is_empty() {
            return Ok(Vec::new());
        }
        let proxies = Self::get_trusted_proxies();
        let mut ips = client_ips;
        // complete the chain with the IP the request actually came from
        ips.push(Value::String(ip.clone()));
        let mut kept: Vec<Value> = Vec::new();
        let mut first_trusted: Option<PhpString> = None;
        for value in ips {
            let mut client_ip = string_of(&value)?.as_bytes().to_vec();
            let mut value = value;
            if client_ip.starts_with(b"[") {
                // strip brackets and :port from IPv6 addresses
                let end = client_ip[1..]
                    .iter()
                    .position(|&c| c == b']')
                    .map_or(-1, |p| i64::try_from(p).unwrap_or(i64::MAX));
                client_ip = substr(&client_ip, 1, Some(end)).to_vec();
                value = Value::string(client_ip.clone());
            } else if client_ip
                .iter()
                .position(|&c| c == b'.')
                .is_some_and(|p| p > 0)
                && client_ip.iter().filter(|&&c| c == b':').count() == 1
            {
                // strip :port from IPv4 addresses; an IPv6 address with an
                // embedded IPv4 one has two colons at least and is kept
                let colon = client_ip.iter().position(|&c| c == b':').unwrap_or(0);
                client_ip.truncate(colon);
                value = Value::string(client_ip.clone());
            }
            if !validate_ip(&client_ip, false, false) {
                continue;
            }
            if check_ip(&client_ip, &proxies)? {
                // fall back to this when every client IP is a trusted proxy
                first_trusted.get_or_insert_with(|| PhpString::from(client_ip.clone()));
                continue;
            }
            kept.push(value);
        }
        Ok(if kept.is_empty() {
            vec![first_trusted.map_or(Value::Null, Value::String)]
        } else {
            kept.into_iter().rev().collect()
        })
    }

    /// `isIisRewrite()`: reads and removes `IIS_WasUrlRewritten`, so a
    /// sub-request does not inherit it.
    fn is_iis_rewrite(&mut self) -> Result<bool> {
        if self.server.get_int("IIS_WasUrlRewritten", 0)? == 1 {
            self.is_iis_rewrite = true;
            self.server.remove("IIS_WasUrlRewritten");
        }
        Ok(self.is_iis_rewrite)
    }
}

/// `getPort()`'s `return $value;` under its `int|string|null` return type,
/// in coercive typing mode (recorded from PHP 8.4.27): a bool is 0 or 1, a
/// float that fits an int is truncated (PHP 8.1+ deprecates a fraction, a
/// notice this does not print), any other float is its `precision = 14`
/// string (`"1.0E+30"`, `"NAN"`), a `\Stringable` object its string; an
/// array or other object is a `\TypeError`.
fn port_return(value: Value) -> Result<Value> {
    const LIMIT: f64 = 9_223_372_036_854_775_808.0; // 2^63
    match value {
        Value::Null | Value::Int(_) | Value::String(_) => Ok(value),
        Value::Bool(b) => Ok(Value::Int(i64::from(b))),
        Value::Float(f) if f.is_finite() && (-LIMIT..LIMIT).contains(&f) => {
            Ok(Value::Int(f.trunc() as i64))
        }
        Value::Float(f) => Ok(Value::string(float_to_string(f))),
        Value::Object(ref object) => match object.to_php_string() {
            Some(s) => Ok(Value::String(s)),
            None => Err(port_type_error(&value)),
        },
        Value::Array(_) => Err(port_type_error(&value)),
    }
}

fn port_type_error(value: &Value) -> Error {
    Error::new(
        Class::PhpTypeError,
        format!(
            r"Symfony\Component\HttpFoundation\Request::getPort(): Return value must be of type string|int|null, {} returned",
            value.debug_type()
        ),
    )
}
