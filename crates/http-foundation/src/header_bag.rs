//! Port of Symfony\Component\HttpFoundation\HeaderBag.php and
//! ResponseHeaderBag.php: HTTP headers by lower-cased name, the
//! Cache-Control directives, and a response's cookies and computed
//! Cache-Control.
//!
//! In PHP, `ResponseHeaderBag` overrides `set()`, `remove()`, `all()`,
//! `replace()` and the Cache-Control readers, and `HeaderBag`'s own methods
//! (`get()`, `has()`, `keys()`, `__toString()`, the constructor) call
//! those overrides. Here one [`HeaderBag`] holds an optional response
//! part and each method follows the override when it is present;
//! [`ResponseHeaderBag`] is that bag with the response-only methods.
//!
//! Header names are PHP array keys (a name such as `"123"` is an int key);
//! values are lists of strings or nulls. PHP would also keep an int or
//! other scalar inside a list of values as given; here a value is a
//! string or null (callers cast with `(string)` first).
//! `ResponseHeaderBag::makeDisposition()` is not ported: it calls
//! `HeaderUtils::makeDisposition()`, which waits for the v8.1.11 version
//! move (PLAN.md decision 16).
//!
//! Symfony source copyright (c) Fabien Potencier; distributed under MIT.
//!
//! Upstream: symfony/http-foundation HeaderBag.php ResponseHeaderBag.php

use std::{
    fmt,
    ops::{Deref, DerefMut},
};

use php_value::{
    Key, PhpArray, PhpString, Value,
    array::compare_keys,
    clock::time,
    dateparse::create_from_format_rfc2822,
    datetime::{DateTimeImmutable, gmdate},
};

use crate::{
    cookie::{Cookie, Options},
    exception::{Class, Error, Result},
    header_utils,
};

/// The values of one header.
pub type Values = Vec<Option<PhpString>>;

/// `string|string[]|null $values`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum HeaderValues {
    One(Option<PhpString>),
    /// An array: its values, keys ignored.
    Many(Values),
}

impl From<&str> for HeaderValues {
    fn from(value: &str) -> Self {
        Self::One(Some(PhpString::from(value)))
    }
}

impl From<PhpString> for HeaderValues {
    fn from(value: PhpString) -> Self {
        Self::One(Some(value))
    }
}

impl From<Option<&str>> for HeaderValues {
    fn from(value: Option<&str>) -> Self {
        Self::One(value.map(PhpString::from))
    }
}

impl From<Vec<&str>> for HeaderValues {
    fn from(values: Vec<&str>) -> Self {
        Self::Many(
            values
                .into_iter()
                .map(|v| Some(PhpString::from(v)))
                .collect(),
        )
    }
}

impl From<Values> for HeaderValues {
    fn from(values: Values) -> Self {
        Self::Many(values)
    }
}

/// An ordered map with PHP array semantics: assigning an existing key
/// keeps its place.
#[derive(Debug, Clone, PartialEq)]
pub struct Ordered<T>(Vec<(Key, T)>);

impl<T> Default for Ordered<T> {
    fn default() -> Self {
        Self(Vec::new())
    }
}

impl<T> Ordered<T> {
    pub fn get(&self, key: &Key) -> Option<&T> {
        self.0.iter().find(|(k, _)| k == key).map(|(_, v)| v)
    }

    fn get_mut(&mut self, key: &Key) -> Option<&mut T> {
        self.0.iter_mut().find(|(k, _)| k == key).map(|(_, v)| v)
    }

    pub fn contains_key(&self, key: &Key) -> bool {
        self.get(key).is_some()
    }

    fn insert(&mut self, key: Key, value: T) {
        match self.get_mut(&key) {
            Some(slot) => *slot = value,
            None => self.0.push((key, value)),
        }
    }

    fn entry(&mut self, key: Key, default: impl FnOnce() -> T) -> &mut T {
        let at = if let Some(at) = self.0.iter().position(|(k, _)| *k == key) {
            at
        } else {
            self.0.push((key, default()));
            self.0.len() - 1
        };
        &mut self.0[at].1
    }

    fn remove(&mut self, key: &Key) {
        self.0.retain(|(k, _)| k != key);
    }

    pub fn iter(&self) -> impl Iterator<Item = (&Key, &T)> {
        self.0.iter().map(|(k, v)| (k, v))
    }

    pub fn keys(&self) -> impl Iterator<Item = &Key> {
        self.0.iter().map(|(k, _)| k)
    }

    fn values_mut(&mut self) -> impl Iterator<Item = &mut T> {
        self.0.iter_mut().map(|(_, v)| v)
    }

    pub fn len(&self) -> usize {
        self.0.len()
    }

    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }

    /// `ksort()`.
    fn ksort(&mut self) {
        self.0.sort_by(|(a, _), (b, _)| compare_keys(a, b));
    }
}

/// `$this->cookies`: domain, then path, then name.
pub type CookieTree = Ordered<Ordered<Ordered<Cookie>>>;

/// What `ResponseHeaderBag` adds to the bag.
#[derive(Debug, Clone, Default, PartialEq)]
struct Response {
    computed_cache_control: PhpArray,
    cookies: CookieTree,
    header_names: Ordered<PhpString>,
}

/// `HeaderBag`.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct HeaderBag {
    headers: Ordered<Values>,
    cache_control: PhpArray,
    response: Option<Response>,
}

/// `strtr($key, self::UPPER, self::LOWER)`: `_` to `-`, ASCII to lower
/// case.
fn normalize(key: &[u8]) -> Key {
    let lowered: Vec<u8> = key
        .iter()
        .map(|&b| {
            if b == b'_' {
                b'-'
            } else {
                b.to_ascii_lowercase()
            }
        })
        .collect();
    Key::from_bytes(&lowered)
}

fn key_text(key: &Key) -> PhpString {
    key.to_value().to_php_string().unwrap_or_default()
}

fn is(key: &Key, name: &str) -> bool {
    key.as_str()
        .is_some_and(|k| k.as_bytes() == name.as_bytes())
}

/// `ucwords($name, '-')`.
fn ucwords_dash(name: &[u8]) -> Vec<u8> {
    let mut upper_next = true;
    name.iter()
        .map(|&b| {
            let out = if upper_next {
                b.to_ascii_uppercase()
            } else {
                b
            };
            upper_next = b == b'-';
            out
        })
        .collect()
}

impl HeaderBag {
    /// `new HeaderBag($headers)`.
    pub fn new<K: AsRef<[u8]>, V: Into<HeaderValues>>(
        headers: impl IntoIterator<Item = (K, V)>,
    ) -> Self {
        let mut bag = Self::default();
        for (key, values) in headers {
            bag.set_plain(key.as_ref(), values.into(), true);
        }
        bag
    }

    /// `__toString()`: every header as `Name: value` lines, names sorted
    /// and padded to the longest.
    pub fn to_php_string(&self) -> PhpString {
        let mut headers = self.all();
        if headers.is_empty() {
            return PhpString::default();
        }
        headers.ksort();
        let names: Vec<PhpString> = headers.keys().map(key_text).collect();
        let max = names.iter().map(PhpString::len).max().unwrap_or(0) + 1;
        let mut content = Vec::new();
        for (name, (_, values)) in names.iter().zip(headers.iter()) {
            let mut label = ucwords_dash(name.as_bytes());
            label.push(b':');
            for value in values {
                content.extend_from_slice(&label);
                content.extend(std::iter::repeat_n(b' ', max.saturating_sub(label.len())));
                content.push(b' ');
                if let Some(value) = value {
                    content.extend_from_slice(value.as_bytes());
                }
                content.extend_from_slice(b"\r\n");
            }
        }
        PhpString::from(content)
    }

    /// `all()`: every header; a response's cookies last, under
    /// `set-cookie`.
    pub fn all(&self) -> Ordered<Values> {
        let mut headers = self.headers.clone();
        if let Some(response) = &self.response {
            for cookie in flatten(&response.cookies) {
                headers
                    .entry(Key::from_bytes(b"set-cookie"), Vec::new)
                    .push(Some(cookie.to_php_string()));
            }
        }
        headers
    }

    /// `all($key)`: the values of one header, `[]` when absent.
    pub fn all_of(&self, key: &str) -> Values {
        let key = normalize(key.as_bytes());
        if let Some(response) = &self.response
            && is(&key, "set-cookie")
        {
            return flatten(&response.cookies)
                .map(|cookie| Some(cookie.to_php_string()))
                .collect();
        }
        self.headers.get(&key).cloned().unwrap_or_default()
    }

    /// `keys()`.
    pub fn keys(&self) -> Vec<Key> {
        self.all().keys().cloned().collect()
    }

    /// `replace($headers)`.
    pub fn replace<K: AsRef<[u8]>, V: Into<HeaderValues>>(
        &mut self,
        headers: impl IntoIterator<Item = (K, V)>,
    ) -> Result<()> {
        if let Some(response) = &mut self.response {
            response.header_names = Ordered::default();
        }
        self.headers = Ordered::default();
        self.add(headers)?;
        self.response_defaults()
    }

    /// `add($headers)`.
    pub fn add<K: AsRef<[u8]>, V: Into<HeaderValues>>(
        &mut self,
        headers: impl IntoIterator<Item = (K, V)>,
    ) -> Result<()> {
        for (key, values) in headers {
            self.set(key.as_ref(), values.into(), true)?;
        }
        Ok(())
    }

    /// `get($key, $default)`: the first value, `default` when absent.
    pub fn get(&self, key: &str, default: Option<&str>) -> Option<PhpString> {
        match self.all_of(key).into_iter().next() {
            None => default.map(PhpString::from),
            Some(first) => first,
        }
    }

    /// `set($key, $values, $replace)`. A response's `Set-Cookie` values
    /// are read by `Cookie::fromString()`, which may throw.
    pub fn set(
        &mut self,
        key: &[u8],
        values: impl Into<HeaderValues>,
        replace: bool,
    ) -> Result<()> {
        let values = values.into();
        if self.response.is_none() {
            self.set_plain(key, values, replace);
            return Ok(());
        }
        let unique = normalize(key);
        if is(&unique, "set-cookie") {
            let cookies: Vec<Option<PhpString>> = match values {
                HeaderValues::One(None) => Vec::new(),
                HeaderValues::One(Some(value)) => vec![Some(value)],
                HeaderValues::Many(values) => values,
            };
            if replace && let Some(response) = &mut self.response {
                response.cookies = Ordered::default();
            }
            for cookie in cookies {
                let Some(cookie) = cookie else {
                    return Err(Error::new(
                        Class::PhpTypeError,
                        "Symfony\\Component\\HttpFoundation\\Cookie::fromString(): Argument #1 ($cookie) must be of type string, null given",
                    ));
                };
                self.set_cookie_inner(Cookie::from_string(cookie.as_bytes(), false)?);
            }
            if let Some(response) = &mut self.response {
                response.header_names.insert(unique, PhpString::from(key));
            }
            return Ok(());
        }
        if let Some(response) = &mut self.response {
            response
                .header_names
                .insert(unique.clone(), PhpString::from(key));
        }
        self.set_plain(key, values, replace);
        let watched = ["cache-control", "etag", "last-modified", "expires"];
        if watched.iter().any(|name| is(&unique, name)) {
            let computed = self.compute_cache_control_value();
            if !computed.is_empty() {
                let parsed = parse_cache_control(computed.as_bytes());
                self.headers
                    .insert(Key::from_bytes(b"cache-control"), vec![Some(computed)]);
                if let Some(response) = &mut self.response {
                    response.header_names.insert(
                        Key::from_bytes(b"cache-control"),
                        PhpString::from("Cache-Control"),
                    );
                    response.computed_cache_control = parsed;
                }
            }
        }
        Ok(())
    }

    /// `HeaderBag`'s own `set()`.
    fn set_plain(&mut self, key: &[u8], values: HeaderValues, replace: bool) {
        let key = normalize(key);
        let exists = self.headers.contains_key(&key);
        match values {
            HeaderValues::Many(values) => {
                if replace || !exists {
                    self.headers.insert(key.clone(), values);
                } else {
                    self.headers.entry(key.clone(), Vec::new).extend(values);
                }
            }
            HeaderValues::One(value) => {
                if replace || !exists {
                    self.headers.insert(key.clone(), vec![value]);
                } else {
                    self.headers.entry(key.clone(), Vec::new).push(value);
                }
            }
        }
        if is(&key, "cache-control") {
            let joined: Vec<Vec<u8>> = self
                .headers
                .get(&key)
                .map(|values| {
                    values
                        .iter()
                        .map(|v| {
                            v.as_ref()
                                .map(|v| v.as_bytes().to_vec())
                                .unwrap_or_default()
                        })
                        .collect()
                })
                .unwrap_or_default();
            self.cache_control = parse_cache_control(&joined.join(&b", "[..]));
        }
    }

    /// `has($key)`.
    pub fn has(&self, key: &str) -> bool {
        self.all().contains_key(&normalize(key.as_bytes()))
    }

    /// `contains($key, $value)`.
    pub fn contains(&self, key: &str, value: &str) -> bool {
        self.all_of(key)
            .iter()
            .any(|v| v.as_ref().is_some_and(|v| v.as_bytes() == value.as_bytes()))
    }

    /// `remove($key)`.
    pub fn remove(&mut self, key: &str) {
        let unique = normalize(key.as_bytes());
        if let Some(response) = &mut self.response {
            response.header_names.remove(&unique);
            if is(&unique, "set-cookie") {
                response.cookies = Ordered::default();
                return;
            }
        }
        self.headers.remove(&unique);
        if is(&unique, "cache-control") {
            self.cache_control = PhpArray::new();
            if let Some(response) = &mut self.response {
                response.computed_cache_control = PhpArray::new();
            }
        }
        if self.response.is_some() && is(&unique, "date") {
            self.init_date();
        }
    }

    /// `getDate($key, $default)`: the header read as an RFC 2822 date.
    pub fn get_date(
        &self,
        key: &str,
        default: Option<DateTimeImmutable>,
    ) -> Result<Option<DateTimeImmutable>> {
        let Some(value) = self.get(key, None) else {
            return Ok(default);
        };
        match create_from_format_rfc2822(value.as_bytes()) {
            Ok(Some(date)) => Ok(Some(date)),
            Ok(None) => {
                let mut message =
                    format!("The \"{key}\" HTTP header is not parseable (").into_bytes();
                message.extend_from_slice(value.as_bytes());
                message.extend_from_slice(b").");
                Err(Error::new(Class::PhpRuntime, message))
            }
            Err(unsupported) => Err(Error::new(Class::NotPorted, unsupported.to_string())),
        }
    }

    /// `addCacheControlDirective($key, $value)`: `value` is `true` or a
    /// string.
    pub fn add_cache_control_directive(
        &mut self,
        key: &str,
        value: impl Into<Value>,
    ) -> Result<()> {
        self.cache_control
            .insert(Key::from_bytes(key.as_bytes()), value.into());
        let header = self.cache_control_header()?;
        self.set(b"Cache-Control", header, true)
    }

    /// `hasCacheControlDirective($key)`.
    pub fn has_cache_control_directive(&self, key: &str) -> bool {
        self.directives()
            .contains_key(&Key::from_bytes(key.as_bytes()))
    }

    /// `getCacheControlDirective($key)`: `true`, a string, or null.
    pub fn get_cache_control_directive(&self, key: &str) -> Value {
        self.directives()
            .get(&Key::from_bytes(key.as_bytes()))
            .cloned()
            .unwrap_or(Value::Null)
    }

    fn directives(&self) -> &PhpArray {
        match &self.response {
            Some(response) => &response.computed_cache_control,
            None => &self.cache_control,
        }
    }

    /// `removeCacheControlDirective($key)`.
    pub fn remove_cache_control_directive(&mut self, key: &str) -> Result<()> {
        self.cache_control.remove(&Key::from_bytes(key.as_bytes()));
        let header = self.cache_control_header()?;
        self.set(b"Cache-Control", header, true)
    }

    /// `getIterator()`: the headers, without a response's cookies.
    pub fn iter(&self) -> impl Iterator<Item = (&Key, &Values)> {
        self.headers.iter()
    }

    /// `count()`: the headers, without a response's cookies.
    pub fn count(&self) -> usize {
        self.headers.len()
    }

    /// `getCacheControlHeader()`; sorts the directives.
    fn cache_control_header(&mut self) -> Result<PhpString> {
        self.cache_control.ksort();
        header_utils::to_string(&self.cache_control, b",")
    }

    /// `ResponseHeaderBag::computeCacheControlValue()`.
    fn compute_cache_control_value(&mut self) -> PhpString {
        if self.cache_control.is_empty() {
            if self.has("Last-Modified") || self.has("Expires") {
                // allows for heuristic expiration (RFC 7234 Section 4.2.2)
                // in the case of "Last-Modified"
                return PhpString::from("private, must-revalidate");
            }
            // conservative by default
            return PhpString::from("no-cache, private");
        }
        let header = self.cache_control_header().unwrap_or_default();
        let has = |name: &[u8]| self.cache_control.contains_key(&Key::from_bytes(name));
        if has(b"public") || has(b"private") {
            return header;
        }
        // public if s-maxage is defined, private otherwise
        if !has(b"s-maxage") {
            let mut private = header.as_bytes().to_vec();
            private.extend_from_slice(b", private");
            return PhpString::from(private);
        }
        header
    }

    fn set_cookie_inner(&mut self, cookie: Cookie) {
        if let Some(response) = &mut self.response {
            let domain = Key::from_bytes(cookie.domain().map_or(&b""[..], PhpString::as_bytes));
            let path = Key::from_bytes(cookie.path().as_bytes());
            let name = Key::from_bytes(cookie.name().as_bytes());
            response
                .cookies
                .entry(domain, Ordered::default)
                .entry(path, Ordered::default)
                .insert(name, cookie);
            response.header_names.insert(
                Key::from_bytes(b"set-cookie"),
                PhpString::from("Set-Cookie"),
            );
        }
    }

    /// `initDate()`.
    fn init_date(&mut self) {
        let mut date = gmdate("D, d M Y H:i:s", time());
        date.push_str(" GMT");
        // A plain value never fails.
        let _ = self.set(b"Date", date.as_str(), true);
    }

    /// The constructor's and `replace()`'s defaults for a response.
    fn response_defaults(&mut self) -> Result<()> {
        if self.response.is_none() {
            return Ok(());
        }
        if !self
            .headers
            .contains_key(&Key::from_bytes(b"cache-control"))
        {
            self.set(b"Cache-Control", "", true)?;
        }
        if !self.headers.contains_key(&Key::from_bytes(b"date")) {
            self.init_date();
        }
        Ok(())
    }
}

/// `parseCacheControl()`.
fn parse_cache_control(header: &[u8]) -> PhpArray {
    header_utils::split(header, b",=")
        .and_then(|parts| header_utils::combine(&parts))
        .unwrap_or_default()
}

/// The cookies in domain, path, name order.
fn flatten(cookies: &CookieTree) -> impl Iterator<Item = &Cookie> {
    cookies.iter().flat_map(|(_, paths)| {
        paths
            .iter()
            .flat_map(|(_, names)| names.iter().map(|(_, cookie)| cookie))
    })
}

impl fmt::Display for HeaderBag {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.to_php_string().to_string_lossy())
    }
}

/// `ResponseHeaderBag::getCookies()`'s result.
#[derive(Debug, Clone, PartialEq)]
pub enum Cookies {
    /// `COOKIES_FLAT`.
    Flat(Vec<Cookie>),
    /// `COOKIES_ARRAY`: domain, path, name.
    Array(CookieTree),
}

/// `clearCookie()`'s arguments after the name, path and domain, with
/// PHP's defaults.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ClearCookie {
    pub secure: bool,
    pub http_only: bool,
    pub same_site: Option<PhpString>,
    pub partitioned: bool,
}

impl Default for ClearCookie {
    fn default() -> Self {
        Self {
            secure: false,
            http_only: true,
            same_site: None,
            partitioned: false,
        }
    }
}

/// `ResponseHeaderBag`.
#[derive(Debug, Clone, PartialEq)]
pub struct ResponseHeaderBag(HeaderBag);

impl Deref for ResponseHeaderBag {
    type Target = HeaderBag;

    fn deref(&self) -> &HeaderBag {
        &self.0
    }
}

impl DerefMut for ResponseHeaderBag {
    fn deref_mut(&mut self) -> &mut HeaderBag {
        &mut self.0
    }
}

impl ResponseHeaderBag {
    pub const COOKIES_FLAT: &'static str = "flat";
    pub const COOKIES_ARRAY: &'static str = "array";

    /// `new ResponseHeaderBag($headers)`: a Cache-Control and a Date
    /// header are added when missing.
    pub fn new<K: AsRef<[u8]>, V: Into<HeaderValues>>(
        headers: impl IntoIterator<Item = (K, V)>,
    ) -> Result<Self> {
        let mut bag = HeaderBag {
            response: Some(Response::default()),
            ..HeaderBag::default()
        };
        bag.add(headers)?;
        bag.response_defaults()?;
        Ok(Self(bag))
    }

    /// `new ResponseHeaderBag()`.
    pub fn empty() -> Self {
        let mut bag = HeaderBag {
            response: Some(Response::default()),
            ..HeaderBag::default()
        };
        // Setting the defaults on an empty bag sets plain values only.
        let _ = bag.response_defaults();
        Self(bag)
    }

    fn response(&self) -> &Response {
        // Every ResponseHeaderBag is built with a response part.
        static EMPTY: std::sync::OnceLock<Response> = std::sync::OnceLock::new();
        self.0
            .response
            .as_ref()
            .unwrap_or_else(|| EMPTY.get_or_init(Response::default))
    }

    /// `allPreserveCase()`: the headers under the names they were set
    /// with.
    pub fn all_preserve_case(&self) -> Ordered<Values> {
        let names = &self.response().header_names;
        let mut headers = Ordered::default();
        for (name, values) in self.all().iter() {
            let key = match names.get(name) {
                Some(original) => Key::from_bytes(original.as_bytes()),
                None => name.clone(),
            };
            headers.insert(key, values.clone());
        }
        headers
    }

    /// `allPreserveCaseWithoutCookies()`.
    pub fn all_preserve_case_without_cookies(&self) -> Ordered<Values> {
        let mut headers = self.all_preserve_case();
        if let Some(name) = self
            .response()
            .header_names
            .get(&Key::from_bytes(b"set-cookie"))
        {
            headers.remove(&Key::from_bytes(name.as_bytes()));
        }
        headers
    }

    /// `setCookie($cookie)`.
    pub fn set_cookie(&mut self, cookie: Cookie) {
        self.0.set_cookie_inner(cookie);
    }

    /// `removeCookie($name, $path, $domain)`: from the bag only; a null
    /// path is `/`.
    pub fn remove_cookie(&mut self, name: &str, path: Option<&str>, domain: Option<&str>) {
        let Some(response) = &mut self.0.response else {
            return;
        };
        let domain = Key::from_bytes(domain.unwrap_or("").as_bytes());
        let path = Key::from_bytes(path.unwrap_or("/").as_bytes());
        let name = Key::from_bytes(name.as_bytes());
        let mut drop_domain = false;
        if let Some(paths) = response.cookies.get_mut(&domain) {
            let mut drop_path = false;
            if let Some(names) = paths.get_mut(&path) {
                names.remove(&name);
                drop_path = names.is_empty();
            }
            if drop_path || !paths.contains_key(&path) {
                paths.remove(&path);
                drop_domain = paths.is_empty();
            }
        }
        if drop_domain {
            response.cookies.remove(&domain);
        }
        if response.cookies.is_empty() {
            response
                .header_names
                .remove(&Key::from_bytes(b"set-cookie"));
        }
    }

    /// `getCookies($format)`.
    pub fn get_cookies(&self, format: &str) -> Result<Cookies> {
        match format {
            Self::COOKIES_FLAT => Ok(Cookies::Flat(
                flatten(&self.response().cookies).cloned().collect(),
            )),
            Self::COOKIES_ARRAY => Ok(Cookies::Array(self.response().cookies.clone())),
            _ => Err(Error::new(
                Class::PhpInvalidArgument,
                format!("Format \"{format}\" invalid (flat, array)."),
            )),
        }
    }

    /// The bag's own cookies, in `getCookies()`'s order. PHP's
    /// `getCookies()` returns the objects the bag holds, so a change to
    /// one (`Response::prepare()`'s `setSecureDefault()`) is a change to
    /// the bag; here the bag lends them.
    pub fn cookies_mut(&mut self) -> impl Iterator<Item = &mut Cookie> {
        self.0.response.iter_mut().flat_map(|response| {
            response
                .cookies
                .values_mut()
                .flat_map(|paths| paths.values_mut().flat_map(Ordered::values_mut))
        })
    }

    /// `clearCookie($name, $path, $domain, ...)`: a cookie that expired,
    /// for the browser to delete.
    pub fn clear_cookie(
        &mut self,
        name: &str,
        path: Option<&str>,
        domain: Option<&str>,
        options: ClearCookie,
    ) -> Result<()> {
        let cookie = Cookie::new(
            name,
            Options {
                value: None,
                expire: 1.into(),
                path: path.map(PhpString::from),
                domain: domain.map(PhpString::from),
                secure: Some(options.secure),
                http_only: options.http_only,
                raw: false,
                same_site: options.same_site,
                partitioned: options.partitioned,
            },
        )?;
        self.set_cookie(cookie);
        Ok(())
    }
}

impl fmt::Display for ResponseHeaderBag {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.0.fmt(f)
    }
}
