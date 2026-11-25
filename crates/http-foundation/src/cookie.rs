//! Port of Symfony\Component\HttpFoundation\Cookie.php: a cookie, its
//! `Set-Cookie` text and `fromString()`.
//!
//! PHP's `int|string|\DateTimeInterface $expire` is [`Expire`]; a text is
//! read by `php_value::dateparse::strtotime()`, which refuses
//! ([`Class::NotPorted`]) a date it does not read as PHP would, never
//! guessing. The clock is `php_value::clock::time()`, which tests mock as
//! `ClockMock` does. PHP's string truthiness is kept: a path or domain of
//! `"0"` is not written, and a path of `"0"` becomes `/`.
//!
//! Symfony source copyright (c) Fabien Potencier; distributed under MIT.
//!
//! Upstream: symfony/http-foundation Cookie.php

use std::fmt;

use php_value::{
    PhpString,
    clock::time,
    dateparse::strtotime,
    datetime::{DateTimeImmutable, gmdate},
    numeric::{Number, numeric_value, string_to_int},
    url::{rawurlencode, urldecode},
};

use crate::exception::{Class, Error, Result};

/// `Cookie::SAMESITE_*`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SameSite {
    None,
    Lax,
    Strict,
}

impl SameSite {
    /// The constant's value, as the `samesite` attribute writes it.
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::None => "none",
            Self::Lax => "lax",
            Self::Strict => "strict",
        }
    }

    /// A lower-case attribute value.
    fn from_lower(value: &[u8]) -> Option<Self> {
        [Self::None, Self::Lax, Self::Strict]
            .into_iter()
            .find(|same_site| same_site.as_str().as_bytes() == value)
    }
}

/// `int|string|\DateTimeInterface $expire`.
#[derive(Debug, Clone, PartialEq)]
pub enum Expire {
    Timestamp(i64),
    /// A numeric string is a timestamp; any other is read by `strtotime()`.
    Text(PhpString),
    DateTime(DateTimeImmutable),
}

impl Default for Expire {
    fn default() -> Self {
        Self::Timestamp(0)
    }
}

impl From<i64> for Expire {
    fn from(timestamp: i64) -> Self {
        Self::Timestamp(timestamp)
    }
}

impl From<&str> for Expire {
    fn from(text: &str) -> Self {
        Self::Text(PhpString::from(text))
    }
}

impl From<DateTimeImmutable> for Expire {
    fn from(date: DateTimeImmutable) -> Self {
        Self::DateTime(date)
    }
}

/// The constructor's arguments after the name, with PHP's defaults.
#[derive(Debug, Clone, PartialEq)]
pub struct Options {
    pub value: Option<PhpString>,
    pub expire: Expire,
    pub path: Option<PhpString>,
    pub domain: Option<PhpString>,
    pub secure: Option<bool>,
    pub http_only: bool,
    pub raw: bool,
    /// Any case; `""` is null.
    pub same_site: Option<PhpString>,
    pub partitioned: bool,
}

impl Default for Options {
    fn default() -> Self {
        Self {
            value: None,
            expire: Expire::default(),
            path: Some(PhpString::from("/")),
            domain: None,
            secure: None,
            http_only: true,
            raw: false,
            same_site: Some(PhpString::from(SameSite::Lax.as_str())),
            partitioned: false,
        }
    }
}

/// A cookie.
#[expect(clippy::struct_excessive_bools, reason = "upstream's fields")]
#[derive(Debug, Clone, PartialEq)]
pub struct Cookie {
    name: PhpString,
    value: Option<PhpString>,
    expire: i64,
    path: PhpString,
    domain: Option<PhpString>,
    secure: Option<bool>,
    http_only: bool,
    raw: bool,
    same_site: Option<SameSite>,
    secure_default: bool,
    partitioned: bool,
}

const RESERVED_CHARS_LIST: &[u8] = b"=,; \t\r\n\x0B\x0C";
/// `RESERVED_CHARS_LIST` minus `=`, which PHP allows in the path and
/// domain attributes.
const RESERVED_ATTR_CHARS_LIST: &[u8] = b",; \t\r\n\x0B\x0C";

/// PHP's `trim()` characters.
const PHP_TRIM: &[u8] = b" \t\n\r\0\x0B";

fn trim(text: &[u8]) -> &[u8] {
    let start = text
        .iter()
        .position(|b| !PHP_TRIM.contains(b))
        .unwrap_or(text.len());
    let end = text
        .iter()
        .rposition(|b| !PHP_TRIM.contains(b))
        .map_or(start, |i| i + 1);
    &text[start..end]
}

/// `false !== strpbrk($text, $list)`.
fn contains_any(text: &[u8], list: &[u8]) -> bool {
    text.iter().any(|b| list.contains(b))
}

/// A PHP string's truthiness: not `""` and not `"0"`.
fn truthy(text: &[u8]) -> bool {
    !text.is_empty() && text != b"0"
}

fn invalid(message: impl Into<PhpString>) -> Error {
    Error::new(Class::PhpInvalidArgument, message)
}

fn quoted_message(prefix: &str, value: &[u8], suffix: &str) -> PhpString {
    let mut message = prefix.as_bytes().to_vec();
    message.extend_from_slice(value);
    message.extend_from_slice(suffix.as_bytes());
    PhpString::from(message)
}

/// `expiresTimestamp()`: a Unix timestamp, or 0 for one not after 1970.
fn expires_timestamp(expire: &Expire) -> Result<i64> {
    let positive = |value: i64| value.max(0);
    match expire {
        Expire::Timestamp(timestamp) => Ok(positive(*timestamp)),
        Expire::DateTime(date) => Ok(positive(date.timestamp())),
        Expire::Text(text) => match numeric_value(text.as_bytes()) {
            Some(Number::Int(value)) => Ok(positive(value)),
            Some(Number::Float(value)) => Ok(if 0.0 < value {
                string_to_int(text.as_bytes())
            } else {
                0
            }),
            None => match strtotime(text.as_bytes(), time()) {
                Ok(Some(timestamp)) => Ok(positive(timestamp)),
                Ok(None) => Err(invalid("The cookie expiration time is not valid.")),
                Err(unsupported) => Err(Error::new(Class::NotPorted, unsupported.to_string())),
            },
        },
    }
}

/// `validateAttribute()`: rejects what PHP's `setcookie()` rejects.
fn validate_attribute(attribute: &str, value: Option<&[u8]>) -> Result<()> {
    match value {
        Some(value) if contains_any(value, RESERVED_ATTR_CHARS_LIST) => {
            Err(invalid(quoted_message(
                &format!("The cookie {attribute} \""),
                value,
                "\" contains invalid characters.",
            )))
        }
        _ => Ok(()),
    }
}

fn check_raw_name(name: &[u8]) -> Result<()> {
    if contains_any(name, RESERVED_CHARS_LIST) {
        return Err(invalid(quoted_message(
            "The cookie name \"",
            name,
            "\" contains invalid characters.",
        )));
    }
    Ok(())
}

/// `withSameSite()`'s check: `""` and null are null, any case is read.
fn same_site(value: Option<&[u8]>) -> Result<Option<SameSite>> {
    match value {
        None | Some(b"") => Ok(None),
        Some(value) => SameSite::from_lower(&value.to_ascii_lowercase())
            .map(Some)
            .ok_or_else(|| invalid("The \"sameSite\" parameter value is not valid.")),
    }
}

impl Cookie {
    /// `new Cookie($name, ...)`.
    pub fn new(name: impl Into<PhpString>, options: Options) -> Result<Self> {
        let name = name.into();
        if options.raw {
            check_raw_name(name.as_bytes())?;
        }
        if name.is_empty() {
            return Err(invalid("The cookie name cannot be empty."));
        }
        validate_attribute("path", options.path.as_ref().map(PhpString::as_bytes))?;
        validate_attribute("domain", options.domain.as_ref().map(PhpString::as_bytes))?;
        let expire = expires_timestamp(&options.expire)?;
        let path = match options.path {
            Some(path) if truthy(path.as_bytes()) => path,
            _ => PhpString::from("/"),
        };
        Ok(Self {
            name,
            value: options.value,
            expire,
            path,
            domain: options.domain,
            secure: options.secure,
            http_only: options.http_only,
            raw: options.raw,
            same_site: same_site(options.same_site.as_ref().map(PhpString::as_bytes))?,
            secure_default: false,
            partitioned: options.partitioned,
        })
    }

    /// `Cookie::create($name, ...)`: the constructor, with the same
    /// defaults.
    pub fn create(name: impl Into<PhpString>, options: Options) -> Result<Self> {
        Self::new(name, options)
    }

    /// `Cookie::fromString($cookie, $decode)`: a `Set-Cookie` value.
    pub fn from_string(cookie: &[u8], decode: bool) -> Result<Self> {
        let (pair, attributes) = match cookie.iter().position(|&b| b == b';') {
            Some(at) => (&cookie[..at], &cookie[at + 1..]),
            None => (cookie, &b""[..]),
        };
        let (name, value) = match pair.iter().position(|&b| b == b'=') {
            Some(at) => (trim(&pair[..at]), Some(trim(&pair[at + 1..]))),
            None => (trim(pair), None),
        };
        let (name, value) = if decode {
            (urldecode(name), value.map(urldecode))
        } else {
            (name.to_vec(), value.map(<[u8]>::to_vec))
        };

        let mut expires = 0i64;
        let mut path: Vec<u8> = b"/".to_vec();
        let mut domain: Option<Vec<u8>> = None;
        let mut secure = false;
        let mut http_only = false;
        let mut same_site_value: Option<SameSite> = None;
        let mut partitioned = false;
        let mut max_age: Option<Vec<u8>> = None;

        for attribute in attributes.split(|&b| b == b';') {
            let (attribute_name, attribute_value) = match attribute.iter().position(|&b| b == b'=')
            {
                Some(at) => (&attribute[..at], trim(&attribute[at + 1..])),
                None => (attribute, &b""[..]),
            };
            let attribute_name = trim(attribute_name).to_ascii_lowercase();
            match attribute_name.as_slice() {
                b"secure" => secure = true,
                b"httponly" => http_only = true,
                b"partitioned" => partitioned = true,
                b"expires" => {
                    // double quotes are delimiters in cookie dates, see RFC
                    // 6265, section 5.1.1
                    let unquoted: Vec<u8> = attribute_value
                        .iter()
                        .copied()
                        .filter(|&b| b != b'"')
                        .collect();
                    if numeric_value(&unquoted).is_some() {
                        expires = expires_timestamp(&Expire::Text(PhpString::from(unquoted)))?;
                    } else {
                        match strtotime(&unquoted, time()) {
                            Ok(Some(timestamp)) => expires = timestamp.max(1),
                            Ok(None) => {}
                            Err(unsupported) => {
                                return Err(Error::new(Class::NotPorted, unsupported.to_string()));
                            }
                        }
                    }
                }
                b"max-age" => {
                    let digits = attribute_value
                        .strip_prefix(b"-")
                        .unwrap_or(attribute_value);
                    if digits.is_empty() || !digits.iter().all(u8::is_ascii_digit) {
                        continue;
                    }
                    max_age = Some(attribute_value.to_vec());
                }
                b"samesite" => {
                    if let Some(found) = SameSite::from_lower(&attribute_value.to_ascii_lowercase())
                    {
                        same_site_value = Some(found);
                    }
                }
                b"path" => path = attribute_value.to_vec(),
                b"domain" => domain = Some(attribute_value.to_vec()),
                // 'raw' and unknown attributes are ignored
                _ => {}
            }
        }

        if let Some(max_age) = max_age {
            let max_age = string_to_int(&max_age);
            let now = time();
            if max_age > 0 {
                expires = now + max_age.min(i64::MAX - now);
            } else if expires > now {
                expires = now;
            } else if expires == 0 {
                expires = 1;
            }
        }

        Self::new(
            PhpString::from(name),
            Options {
                value: value.map(PhpString::from),
                expire: Expire::Timestamp(expires),
                path: Some(PhpString::from(path)),
                domain: domain.map(PhpString::from),
                secure: Some(secure),
                http_only,
                raw: !decode,
                same_site: same_site_value.map(|found| PhpString::from(found.as_str())),
                partitioned,
            },
        )
    }

    /// `withValue()`.
    #[must_use]
    pub fn with_value(&self, value: Option<PhpString>) -> Self {
        Self {
            value,
            ..self.clone()
        }
    }

    /// `withDomain()`.
    pub fn with_domain(&self, domain: Option<PhpString>) -> Result<Self> {
        validate_attribute("domain", domain.as_ref().map(PhpString::as_bytes))?;
        Ok(Self {
            domain,
            ..self.clone()
        })
    }

    /// `withExpires()`.
    pub fn with_expires(&self, expire: impl Into<Expire>) -> Result<Self> {
        Ok(Self {
            expire: expires_timestamp(&expire.into())?,
            ..self.clone()
        })
    }

    /// `withPath()`: `""` is `/`.
    pub fn with_path(&self, path: impl Into<PhpString>) -> Result<Self> {
        let path = path.into();
        validate_attribute("path", Some(path.as_bytes()))?;
        Ok(Self {
            path: if path.is_empty() {
                PhpString::from("/")
            } else {
                path
            },
            ..self.clone()
        })
    }

    /// `withSecure()`.
    #[must_use]
    pub fn with_secure(&self, secure: bool) -> Self {
        Self {
            secure: Some(secure),
            ..self.clone()
        }
    }

    /// `withHttpOnly()`.
    #[must_use]
    pub fn with_http_only(&self, http_only: bool) -> Self {
        Self {
            http_only,
            ..self.clone()
        }
    }

    /// `withRaw()`.
    pub fn with_raw(&self, raw: bool) -> Result<Self> {
        if raw {
            check_raw_name(self.name.as_bytes())?;
        }
        Ok(Self {
            raw,
            ..self.clone()
        })
    }

    /// `withSameSite()`: any case; `""` and null are null.
    pub fn with_same_site(&self, same_site_value: Option<&str>) -> Result<Self> {
        Ok(Self {
            same_site: same_site(same_site_value.map(str::as_bytes))?,
            ..self.clone()
        })
    }

    /// `withPartitioned()`.
    #[must_use]
    pub fn with_partitioned(&self, partitioned: bool) -> Self {
        Self {
            partitioned,
            ..self.clone()
        }
    }

    /// `(string) $cookie`: the `Set-Cookie` header value.
    pub fn to_php_string(&self) -> PhpString {
        let mut out = if self.raw {
            self.name.as_bytes().to_vec()
        } else {
            let mut encoded = Vec::with_capacity(self.name.len());
            for &byte in self.name.as_bytes() {
                match byte {
                    b'=' => encoded.extend_from_slice(b"%3D"),
                    b',' => encoded.extend_from_slice(b"%2C"),
                    b';' => encoded.extend_from_slice(b"%3B"),
                    b' ' => encoded.extend_from_slice(b"%20"),
                    b'\t' => encoded.extend_from_slice(b"%09"),
                    b'\r' => encoded.extend_from_slice(b"%0D"),
                    b'\n' => encoded.extend_from_slice(b"%0A"),
                    0x0B => encoded.extend_from_slice(b"%0B"),
                    0x0C => encoded.extend_from_slice(b"%0C"),
                    other => encoded.push(other),
                }
            }
            encoded
        };
        out.push(b'=');
        match self.value.as_ref().filter(|value| !value.is_empty()) {
            None => {
                out.extend_from_slice(b"deleted; expires=");
                out.extend_from_slice(gmdate("D, d M Y H:i:s T", time() - 31_536_001).as_bytes());
                out.extend_from_slice(b"; Max-Age=0");
            }
            Some(value) => {
                if self.raw {
                    out.extend_from_slice(value.as_bytes());
                } else {
                    out.extend_from_slice(&rawurlencode(value.as_bytes()));
                }
                if self.expire != 0 {
                    out.extend_from_slice(b"; expires=");
                    out.extend_from_slice(gmdate("D, d M Y H:i:s T", self.expire).as_bytes());
                    out.extend_from_slice(format!("; Max-Age={}", self.max_age()).as_bytes());
                }
            }
        }
        if truthy(self.path.as_bytes()) {
            out.extend_from_slice(b"; path=");
            out.extend_from_slice(self.path.as_bytes());
        }
        if let Some(domain) = self
            .domain
            .as_ref()
            .filter(|domain| truthy(domain.as_bytes()))
        {
            out.extend_from_slice(b"; domain=");
            out.extend_from_slice(domain.as_bytes());
        }
        if self.is_secure() {
            out.extend_from_slice(b"; secure");
        }
        if self.http_only {
            out.extend_from_slice(b"; httponly");
        }
        if let Some(same_site) = self.same_site {
            out.extend_from_slice(b"; samesite=");
            out.extend_from_slice(same_site.as_str().as_bytes());
        }
        if self.partitioned {
            out.extend_from_slice(b"; partitioned");
        }
        PhpString::from(out)
    }

    /// `getName()`.
    pub fn name(&self) -> &PhpString {
        &self.name
    }

    /// `getValue()`.
    pub fn value(&self) -> Option<&PhpString> {
        self.value.as_ref()
    }

    /// `getDomain()`.
    pub fn domain(&self) -> Option<&PhpString> {
        self.domain.as_ref()
    }

    /// `getExpiresTime()`.
    pub fn expires_time(&self) -> i64 {
        self.expire
    }

    /// `getMaxAge()`.
    pub fn max_age(&self) -> i64 {
        self.expire.saturating_sub(time()).max(0)
    }

    /// `getPath()`.
    pub fn path(&self) -> &PhpString {
        &self.path
    }

    /// `isSecure()`: the flag, or the secure default when it is null.
    pub fn is_secure(&self) -> bool {
        self.secure.unwrap_or(self.secure_default)
    }

    /// `isHttpOnly()`.
    pub fn is_http_only(&self) -> bool {
        self.http_only
    }

    /// `isCleared()`: whether the cookie is about to be cleared.
    pub fn is_cleared(&self) -> bool {
        self.expire != 0 && self.expire < time()
    }

    /// `isRaw()`.
    pub fn is_raw(&self) -> bool {
        self.raw
    }

    /// `isPartitioned()`.
    pub fn is_partitioned(&self) -> bool {
        self.partitioned
    }

    /// `getSameSite()`.
    pub fn same_site(&self) -> Option<SameSite> {
        self.same_site
    }

    /// `setSecureDefault()`.
    pub fn set_secure_default(&mut self, default: bool) {
        self.secure_default = default;
    }
}

impl fmt::Display for Cookie {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.to_php_string().to_string_lossy())
    }
}
