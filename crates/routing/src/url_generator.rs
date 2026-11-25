//! Port of Symfony\Component\Routing\Generator\UrlGenerator.php, with
//! UrlGeneratorInterface.php and ConfigurableRequirementsInterface.php:
//! URLs and paths for the routes of a [`RouteCollection`], from the
//! parameters given, the [`RequestContext`] and the route defaults.
//!
//! Parameters are PHP values ([`PhpArray`]), as upstream takes them: they
//! are compared with defaults by `==` ([`loose_equals`]), cast to strings
//! by PHP's rules, and objects among the query parameters are turned into
//! arrays of their public properties or their `__toString()`. The
//! requirement check anchors the whole requirement
//! (`#^(?:requirement)$#i`), the fix for GHSA-72xp-p242-47p9, after
//! removing look-around groups.
//!
//! A route is compiled for each call unless it was compiled in the
//! collection beforehand (`RouteCollection::get_mut(..)?.compile()`);
//! requirement patterns are compiled once per generator. The logger
//! receives context values as strings. `$decodedChars` is upstream's
//! fixed table; a subclass changing it is not ported.
//!
//! Symfony source copyright (c) Fabien Potencier; distributed under MIT.
//!
//! Upstream: symfony/routing Generator/UrlGenerator.php Generator/UrlGeneratorInterface.php Generator/ConfigurableRequirementsInterface.php

use std::{
    collections::HashMap,
    sync::{Arc, Mutex},
};

use php_pcre::{Pcre, PcreError};
use php_value::{
    Key, PhpArray, PhpString, Value,
    compare::loose_equals,
    numeric::float_to_string,
    url::{QueryEncoding, http_build_query_with, rawurlencode},
};
use psr_log::{ContextValue, LoggerInterface};

use crate::{
    exception::{Class, Error, concat},
    php::{get, isset, key_bytes, string_argument, truthy},
    request_context::RequestContext,
    route::Route,
    route_collection::RouteCollection,
};

/// `UrlGeneratorInterface`'s reference types.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ReferenceType {
    /// `ABSOLUTE_URL` (0): `http://example.com/dir/file`.
    AbsoluteUrl,
    /// `ABSOLUTE_PATH` (1, the default): `/dir/file`.
    #[default]
    AbsolutePath,
    /// `RELATIVE_PATH` (2): `../parent-file`, from the context's path.
    RelativePath,
    /// `NETWORK_PATH` (3): `//example.com/dir/file`.
    NetworkPath,
}

impl ReferenceType {
    /// The interface constant's value.
    pub const fn value(self) -> i64 {
        match self {
            Self::AbsoluteUrl => 0,
            Self::AbsolutePath => 1,
            Self::RelativePath => 2,
            Self::NetworkPath => 3,
        }
    }
}

/// `UrlGeneratorInterface`.
pub trait UrlGeneratorInterface {
    /// `generate($name, $parameters, $referenceType)`. Parameters that
    /// are not route variables become the query string; `_fragment` is the
    /// fragment and `_query` an array of query parameters.
    fn generate(
        &self,
        name: &[u8],
        parameters: PhpArray,
        reference_type: ReferenceType,
    ) -> Result<PhpString, Error>;

    /// `setContext()`.
    fn set_context(&mut self, context: RequestContext);

    /// `getContext()`.
    fn context(&self) -> &RequestContext;
}

/// `ConfigurableRequirementsInterface`: `Some(true)` throws for a
/// parameter that does not match its requirement, `Some(false)` logs it
/// and generates `''`, `None` does not check requirements.
pub trait ConfigurableRequirementsInterface {
    /// `setStrictRequirements()`.
    fn set_strict_requirements(&mut self, enabled: Option<bool>);

    /// `isStrictRequirements()`.
    fn is_strict_requirements(&self) -> Option<bool>;
}

/// `UrlGenerator::QUERY_FRAGMENT_DECODED`.
const QUERY_FRAGMENT_DECODED: [(&[u8], &[u8]); 9] = [
    // RFC 3986 explicitly allows those in the query/fragment to reference other URIs unencoded
    (b"%2F", b"/"),
    (b"%252F", b"%2F"),
    (b"%3F", b"?"),
    // reserved chars that have no special meaning for HTTP URIs in a query or fragment
    // this excludes esp. "&", "=" and also "+" because PHP would treat it as a space (form-encoded)
    (b"%40", b"@"),
    (b"%3A", b":"),
    (b"%21", b"!"),
    (b"%3B", b";"),
    (b"%2C", b","),
    (b"%2A", b"*"),
];

/// `UrlGenerator::$decodedChars`: the characters (besides alphanumeric
/// ones) left unencoded in the path.
const DECODED_CHARS: [(&[u8], &[u8]); 11] = [
    // the slash can be used to designate a hierarchical structure
    (b"%2F", b"/"),
    (b"%252F", b"%2F"),
    // general delimiters with special meaning in the authority component only
    (b"%40", b"@"),
    (b"%3A", b":"),
    // sub-delimiters without predefined meaning
    (b"%3B", b";"),
    (b"%2C", b","),
    (b"%3D", b"="),
    (b"%2B", b"+"),
    (b"%21", b"!"),
    (b"%2A", b"*"),
    (b"%7C", b"|"),
];

const MESSAGE: &[u8] = b"Parameter \"{parameter}\" for route \"{route}\" must match \"{expected}\" (\"{given}\" given) to generate a corresponding URL.";

type Compiled = Result<Arc<Pcre>, PcreError>;

/// `UrlGenerator`.
pub struct UrlGenerator {
    routes: RouteCollection,
    context: RequestContext,
    logger: Option<Arc<dyn LoggerInterface>>,
    default_locale: Option<PhpString>,
    strict_requirements: Option<bool>,
    patterns: Mutex<HashMap<Vec<u8>, Compiled>>,
}

impl std::fmt::Debug for UrlGenerator {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("UrlGenerator")
            .field("routes", &self.routes)
            .field("context", &self.context)
            .field("default_locale", &self.default_locale)
            .field("strict_requirements", &self.strict_requirements)
            .finish_non_exhaustive()
    }
}

/// A token's `$token[$i]`.
fn item(token: &Value, i: i64) -> Option<&Value> {
    token.as_array().and_then(|t| t.get(&Key::Int(i)))
}

/// `$token[$i]` as a string, `''` for null (as concatenation reads it).
fn item_bytes(token: &Value, i: i64) -> Result<PhpString, Error> {
    cast(item(token, i).unwrap_or(&Value::Null))
}

/// `(string) $value`: an array is `"Array"` (PHP warns), an object
/// without `__toString()` an `\Error`.
pub(crate) fn cast(value: &Value) -> Result<PhpString, Error> {
    match value {
        Value::Array(_) => Ok(PhpString::from("Array")),
        Value::Object(o) => o.to_php_string().ok_or_else(|| {
            Error::new(
                Class::PhpError,
                format!(
                    "Object of class {} could not be converted to string",
                    o.class()
                ),
            )
        }),
        Value::Float(f) => Ok(PhpString::from(float_to_string(*f))),
        other => Ok(other.to_php_string().unwrap_or_default()),
    }
}

/// A `string` argument of a PHP function: null is `''` (PHP deprecates
/// it), arrays and objects without `__toString()` a `\TypeError`.
fn argument(
    value: &Value,
    function: &str,
    position: usize,
    name: &str,
) -> Result<PhpString, Error> {
    match value {
        Value::Null => Ok(PhpString::default()),
        other => string_argument(other, function, position, name),
    }
}

/// `strtr($subject, $pairs)`: at each position the longest key that
/// matches is replaced, and replacements are not scanned again.
fn strtr(subject: &[u8], pairs: &[(&[u8], &[u8])]) -> Vec<u8> {
    let mut out = Vec::with_capacity(subject.len());
    let mut i = 0;
    while i < subject.len() {
        let found = pairs
            .iter()
            .filter(|(from, _)| subject[i..].starts_with(from))
            .max_by_key(|(from, _)| from.len());
        if let Some((from, to)) = found {
            out.extend_from_slice(to);
            i += from.len();
        } else {
            out.push(subject[i]);
            i += 1;
        }
    }
    out
}

/// The end of a look-around group's body starting at `i`:
/// `(?:[^()\\]+|\\.|\((?1)\))*`, whose alternatives start with different
/// bytes, so it reads without backtracking.
fn group_body(s: &[u8], mut i: usize) -> usize {
    loop {
        match s.get(i) {
            Some(b'(') => match nested_group(s, i) {
                Some(end) => i = end,
                None => return i,
            },
            Some(b')') | None => return i,
            Some(b'\\') => match s.get(i + 1) {
                Some(&c) if c != b'\n' => i += 2,
                _ => return i,
            },
            Some(_) => i += 1,
        }
    }
}

/// `\((?1)\)` at `i`: the end after its `)`.
fn nested_group(s: &[u8], i: usize) -> Option<usize> {
    let end = group_body(s, i + 1);
    (s.get(end) == Some(&b')')).then_some(end + 1)
}

/// `preg_replace('/\(\?(?:=|<=|!|<!)((?:[^()\\\\]+|\\\\.|\((?1)\))*)\)/', '',
/// $requirement)`: removes look-ahead and look-behind groups.
fn remove_look_arounds(requirement: &[u8]) -> Vec<u8> {
    let mut out = Vec::with_capacity(requirement.len());
    let mut i = 0;
    while i < requirement.len() {
        let rest = &requirement[i..];
        let opener = [&b"(?="[..], b"(?<=", b"(?!", b"(?<!"]
            .into_iter()
            .find(|o| rest.starts_with(o));
        if let Some(opener) = opener {
            let end = group_body(requirement, i + opener.len());
            if requirement.get(end) == Some(&b')') {
                i = end + 1;
                continue;
            }
        }
        out.push(requirement[i]);
        i += 1;
    }
    out
}

/// `strstr($locale, '_', true)`.
fn before_underscore(locale: &[u8]) -> Option<&[u8]> {
    locale.iter().position(|&c| c == b'_').map(|i| &locale[..i])
}

/// `$prefix.$value.$suffix` for a URL being built from the end.
fn prepend(url: &mut Vec<u8>, parts: &[&[u8]]) {
    let mut start = parts.concat();
    start.append(url);
    *url = start;
}

impl UrlGenerator {
    /// `new UrlGenerator($routes, $context, $logger, $defaultLocale)`.
    pub fn new(
        routes: RouteCollection,
        context: RequestContext,
        logger: Option<Arc<dyn LoggerInterface>>,
        default_locale: Option<PhpString>,
    ) -> Self {
        Self {
            routes,
            context,
            logger,
            default_locale,
            strict_requirements: Some(true),
            patterns: Mutex::new(HashMap::new()),
        }
    }

    /// The routes (upstream's protected `$routes`).
    pub fn routes(&self) -> &RouteCollection {
        &self.routes
    }

    /// The routes, to change or compile them.
    pub fn routes_mut(&mut self) -> &mut RouteCollection {
        &mut self.routes
    }

    /// `getContext()` for changing the context in place.
    pub fn context_mut(&mut self) -> &mut RequestContext {
        &mut self.context
    }

    fn pattern(&self, pattern: &[u8]) -> Compiled {
        let mut cache = match self.patterns.lock() {
            Ok(cache) => cache,
            Err(poisoned) => poisoned.into_inner(),
        };
        cache
            .entry(pattern.to_vec())
            .or_insert_with(|| Pcre::new(pattern).map(Arc::new))
            .clone()
    }

    /// `preg_match('#^(?:'.$stripped.')$#i'.($utf8 ? 'u' : ''), $subject)`
    /// is 1. A pattern PHP cannot compile, or a failed match (`false`),
    /// is not a match, as `!preg_match(...)` reads it.
    fn requirement_matches(&self, token: &Value, subject: &[u8]) -> Result<bool, Error> {
        let requirement = item_bytes(token, 2)?;
        let utf8 = item(token, 4).is_some_and(truthy);
        let pattern = concat(&[
            b"#^(?:",
            &remove_look_arounds(requirement.as_bytes()),
            b")$#i",
            if utf8 { b"u" } else { b"" },
        ]);
        match self.pattern(&pattern) {
            Ok(regex) => Ok(regex.is_match(subject).unwrap_or(false)),
            Err(PcreError::Compile(_)) => Ok(false),
            Err(e @ PcreError::Unsupported(_)) => Err(Error::new(Class::NotPorted, e.to_string())),
        }
    }

    /// What a parameter failing its requirement does: throws when strict,
    /// else logs and makes `generate()` return `''`.
    fn invalid(&self, name: &[u8], token: &Value, given: &Value) -> Result<(), Error> {
        let parameter = item_bytes(token, 3)?;
        let expected = item_bytes(token, 2)?;
        let given = cast(given)?;
        if self.strict_requirements == Some(true) {
            let message = strtr(
                MESSAGE,
                &[
                    (b"{parameter}", parameter.as_bytes()),
                    (b"{route}", name),
                    (b"{expected}", expected.as_bytes()),
                    (b"{given}", given.as_bytes()),
                ],
            );
            return Err(Error::new(Class::InvalidParameter, message));
        }
        if let Some(logger) = &self.logger {
            let text = |b: &[u8]| ContextValue::String(String::from_utf8_lossy(b).into_owned());
            logger.error(
                &String::from_utf8_lossy(MESSAGE),
                vec![
                    ("parameter".to_owned(), text(parameter.as_bytes())),
                    ("route".to_owned(), text(name)),
                    ("expected".to_owned(), text(expected.as_bytes())),
                    ("given".to_owned(), text(given.as_bytes())),
                ],
            );
        }
        Ok(())
    }

    /// `doGenerate()`: `Ok('')` when a parameter fails its requirement and
    /// requirements are not strict.
    #[allow(clippy::too_many_arguments)]
    pub fn do_generate(
        &self,
        variables: &PhpArray,
        defaults: &PhpArray,
        _requirements: &PhpArray,
        tokens: &PhpArray,
        mut parameters: PhpArray,
        name: &[u8],
        mut reference_type: ReferenceType,
        host_tokens: &PhpArray,
        required_schemes: &PhpArray,
    ) -> Result<PhpString, Error> {
        let mut query_parameters = PhpArray::new();
        if isset(&parameters, b"_query") {
            match parameters.remove(&Key::from("_query")) {
                Some(Value::Array(query)) => query_parameters = query,
                _ => {
                    return Err(Error::new(
                        Class::InvalidParameter,
                        "Parameter \"_query\" must be an array of query parameters.",
                    ));
                }
            }
        }

        // array_flip($variables)
        let mut flipped = PhpArray::new();
        for (index, variable) in variables.iter() {
            if let Some(key) = Key::from_value(variable) {
                flipped.insert(key, index.to_value());
            }
        }
        let mut merged = defaults.clone();
        for source in [self.context.parameters(), &parameters] {
            for (key, value) in source.iter() {
                merged.insert(key.clone(), value.clone());
            }
        }

        // all params must be given
        let missing: Vec<Vec<u8>> = flipped
            .keys()
            .filter(|key| !merged.contains_key(key))
            .map(key_bytes)
            .collect();
        if !missing.is_empty() {
            return Err(Error::missing_mandatory_parameters(name, missing));
        }

        let parameter = |token: &Value| -> Result<(Key, Value), Error> {
            let key = Key::from_bytes(item_bytes(token, 3)?.as_bytes());
            let value = merged.get(&key).cloned().unwrap_or_default();
            Ok((key, value))
        };

        let mut url = Vec::new();
        let mut optional = true;
        for token in tokens.values() {
            let kind = item(token, 0).and_then(Value::as_string);
            if kind.is_some_and(|k| k.as_bytes() == b"variable") {
                let (key, value) = parameter(token)?;
                // variable is not important by default
                let important = item(token, 5).is_some_and(truthy);
                let differs = || -> Result<bool, Error> {
                    let Some(default) = defaults.get(&key) else {
                        return Ok(true);
                    };
                    Ok(!value.is_null() && cast(&value)? != cast(default)?)
                };
                if !optional || important || differs()? {
                    // check requirement (while ignoring look-around patterns)
                    if self.strict_requirements.is_some() {
                        let subject = argument(&value, "preg_match", 2, "subject")?;
                        if !self.requirement_matches(token, subject.as_bytes())? {
                            self.invalid(name, token, &value)?;
                            return Ok(PhpString::default());
                        }
                    }
                    prepend(
                        &mut url,
                        &[item_bytes(token, 1)?.as_bytes(), cast(&value)?.as_bytes()],
                    );
                    optional = false;
                }
            } else {
                // static text
                prepend(&mut url, &[item_bytes(token, 1)?.as_bytes()]);
                optional = false;
            }
        }

        if url.is_empty() {
            url = b"/".to_vec();
        }

        // the contexts base URL is already encoded (see Symfony\Component\HttpFoundation\Request)
        let mut url = strtr(&rawurlencode(&url), &DECODED_CHARS);

        // the path segments "." and ".." are interpreted as relative reference when resolving a URI
        // so they are encoded, as they are not used for this purpose here
        if url.windows(2).any(|w| w == b"/.") {
            url = url
                .split(|&c| c == b'/')
                .map(|segment| match segment {
                    b"." => &b"%2E"[..],
                    b".." => b"%2E%2E",
                    other => other,
                })
                .collect::<Vec<_>>()
                .join(&b'/');
        }

        let mut scheme_authority = Vec::new();
        let mut host = self.context.host().as_bytes().to_vec();
        let mut scheme = self.context.scheme().as_bytes().to_vec();

        if let Some((_, first)) = required_schemes.first() {
            let current = Value::string(scheme.as_slice());
            if !required_schemes.values().any(|s| s.identical(&current)) {
                reference_type = ReferenceType::AbsoluteUrl;
                scheme = cast(first)?.as_bytes().to_vec();
            }
        }

        if !host_tokens.is_empty() {
            let mut route_host = Vec::new();
            for token in host_tokens.values() {
                if item(token, 0)
                    .and_then(Value::as_string)
                    .is_some_and(|k| k.as_bytes() == b"variable")
                {
                    let (_, value) = parameter(token)?;
                    // check requirement (while ignoring look-around patterns)
                    if self.strict_requirements.is_some() {
                        let subject = argument(&value, "preg_match", 2, "subject")?;
                        if !self.requirement_matches(token, subject.as_bytes())? {
                            self.invalid(name, token, &value)?;
                            return Ok(PhpString::default());
                        }
                    }
                    prepend(
                        &mut route_host,
                        &[item_bytes(token, 1)?.as_bytes(), cast(&value)?.as_bytes()],
                    );
                } else {
                    prepend(&mut route_host, &[item_bytes(token, 1)?.as_bytes()]);
                }
            }

            if route_host != host {
                host = route_host;
                if reference_type != ReferenceType::AbsoluteUrl {
                    reference_type = ReferenceType::NetworkPath;
                }
            }
        }

        if matches!(
            reference_type,
            ReferenceType::AbsoluteUrl | ReferenceType::NetworkPath
        ) && (!host.is_empty() || !matches!(scheme.as_slice(), b"" | b"http" | b"https"))
        {
            let mut port = Vec::new();
            if scheme == b"http" && self.context.http_port() != 80 {
                port = format!(":{}", self.context.http_port()).into_bytes();
            } else if scheme == b"https" && self.context.https_port() != 443 {
                port = format!(":{}", self.context.https_port()).into_bytes();
            }

            scheme_authority = if reference_type == ReferenceType::NetworkPath || scheme.is_empty()
            {
                b"//".to_vec()
            } else {
                concat(&[&scheme, b"://"])
            };
            scheme_authority.extend_from_slice(&host);
            scheme_authority.extend_from_slice(&port);
        }

        let mut url = if reference_type == ReferenceType::RelativePath {
            Self::get_relative_path(self.context.path_info().as_bytes(), &url)
        } else {
            concat(&[&scheme_authority, self.context.base_url().as_bytes(), &url])
        };

        // add a query string if needed
        let mut extra = PhpArray::new();
        for (key, value) in parameters.iter() {
            if flipped.contains_key(key) {
                continue;
            }
            if defaults
                .get(key)
                .is_some_and(|default| loose_equals(value, default))
            {
                continue;
            }
            extra.insert(key.clone(), value.clone());
        }
        for (key, value) in query_parameters.iter() {
            extra.insert(key.clone(), value.clone());
        }
        let mut seen = Vec::new();
        let mut extra = cast_objects(&extra, &mut seen, name)?;

        // extract fragment
        let mut fragment = defaults
            .get(&Key::from("_fragment"))
            .filter(|v| !v.is_null())
            .cloned()
            .unwrap_or_else(|| Value::string(""));
        if let Some(value) = get(&extra, b"_fragment") {
            fragment = value.clone();
            extra.remove(&Key::from("_fragment"));
        }

        if !extra.is_empty() {
            let query = http_build_query_with(&extra, b"&", QueryEncoding::Rfc3986)
                .ok_or_else(|| Error::new(Class::NotPorted, "an object in the query"))?;
            if !query.is_empty() {
                url.push(b'?');
                url.extend(strtr(&query, &QUERY_FRAGMENT_DECODED));
            }
        }

        if !fragment.identical(&Value::string("")) {
            let fragment = argument(&fragment, "rawurlencode", 1, "string")?;
            url.push(b'#');
            url.extend(strtr(
                &rawurlencode(fragment.as_bytes()),
                &QUERY_FRAGMENT_DECODED,
            ));
        }

        Ok(PhpString::from(url))
    }

    /// `getRelativePath($basePath, $targetPath)`: the target path as a
    /// reference relative to the base path. Only the paths (starting with
    /// a slash, without relative parts) are given.
    ///
    /// Given a base path of `/a/b/c/d`: `/a/b/c/d` is `''`, `/a/b/c/` is
    /// `./`, `/a/b/` is `../`, `/a/b/c/other` is `other` and `/a/x/y` is
    /// `../../x/y`.
    pub fn get_relative_path(base_path: &[u8], target_path: &[u8]) -> Vec<u8> {
        if base_path == target_path {
            return Vec::new();
        }

        let strip = |p: &'_ [u8]| -> Vec<u8> { p.strip_prefix(b"/").unwrap_or(p).to_vec() };
        let split = |p: Vec<u8>| -> Vec<Option<Vec<u8>>> {
            p.split(|&c| c == b'/').map(|s| Some(s.to_vec())).collect()
        };
        let mut source_dirs = split(strip(base_path));
        let mut target_dirs = split(strip(target_path));
        source_dirs.pop();
        let target_file = target_dirs.pop().flatten().unwrap_or_default();

        for i in 0..source_dirs.len() {
            match (&source_dirs[i], target_dirs.get(i)) {
                (Some(dir), Some(Some(target))) if dir == target => {
                    source_dirs[i] = None;
                    target_dirs[i] = None;
                }
                _ => break,
            }
        }

        let mut targets: Vec<Vec<u8>> = target_dirs.into_iter().flatten().collect();
        targets.push(target_file);
        let ups = source_dirs.iter().flatten().count();
        let mut path = b"../".repeat(ups);
        path.extend(targets.join(&b'/'));

        // A reference to the same base directory or an empty subdirectory must be prefixed with "./".
        // This also applies to a segment with a colon character (e.g., "file:colon") that cannot be used
        // as the first segment of a relative-path reference, as it would be mistaken for a scheme name
        // (see http://tools.ietf.org/html/rfc3986#section-4.2).
        let colon = path.iter().position(|&c| c == b':');
        let slash = path.iter().position(|&c| c == b'/');
        let colon_first = match (colon, slash) {
            (Some(c), Some(s)) => c < s,
            (Some(_), None) => true,
            (None, _) => false,
        };
        if path.is_empty() || path[0] == b'/' || colon_first {
            concat(&[b"./", &path])
        } else {
            path
        }
    }

    /// The route `generate()` uses for `$name` and `$locale`.
    fn find(&self, name: &[u8], parameters: &PhpArray) -> Result<Option<&Route>, Error> {
        let locale = requested_locale(&self.context, parameters, self.default_locale.as_ref());
        let mut route = None;
        if let Some(locale) = locale {
            for_each_locale(&locale, |text| {
                let localized = concat(&[name, b".", text]);
                route = self.routes.get(&localized)?;
                Ok(route.is_some_and(|found| {
                    let canonical = found.default_value(b"_canonical_route");
                    canonical.is_some_and(|c| c.identical(&Value::string(name)))
                        || self.routes.alias(&localized).is_some()
                }))
            })?;
        }

        match route {
            Some(route) => Ok(Some(route)),
            None => self.routes.get(name),
        }
    }
}

/// `$parameters['_locale'] ?? $this->context->getParameter('_locale') ?:
/// $this->defaultLocale`.
pub(crate) fn requested_locale(
    context: &RequestContext,
    parameters: &PhpArray,
    default_locale: Option<&PhpString>,
) -> Option<Value> {
    get(parameters, b"_locale")
        .or_else(|| context.parameter(b"_locale").filter(|v| !v.is_null()))
        .filter(|v| truthy(v))
        .cloned()
        .or_else(|| default_locale.cloned().map(Value::String))
}

/// `do { if ($found($locale)) break; } while (false !== $locale =
/// strstr($locale, '_', true));`: the locale as a string, then each part
/// before its first `_`, until `found` says so. `strstr()` takes a string
/// argument, so an array locale is a `TypeError` after the first try.
pub(crate) fn for_each_locale(
    locale: &Value,
    mut found: impl FnMut(&[u8]) -> Result<bool, Error>,
) -> Result<(), Error> {
    let mut text = cast(locale)?.as_bytes().to_vec();
    let mut first = true;
    loop {
        if found(&text)? {
            return Ok(());
        }
        if first {
            first = false;
            argument(locale, "strstr", 1, "haystack")?;
        }
        match before_underscore(&text) {
            Some(prefix) => text = prefix.to_vec(),
            None => return Ok(()),
        }
    }
}

/// A localized route's `_locale`: dropped from the parameters when the
/// path has no `{_locale}`, else the route's own unless one is given.
pub(crate) fn localize_parameters(
    defaults: &PhpArray,
    variables: &PhpArray,
    parameters: &mut PhpArray,
) {
    if isset(defaults, b"_canonical_route") && isset(defaults, b"_locale") {
        let locale = Value::string("_locale");
        if !variables.values().any(|v| v.identical(&locale)) {
            parameters.remove(&Key::from("_locale"));
        } else if !isset(parameters, b"_locale") {
            parameters.insert(
                "_locale",
                defaults
                    .get(&Key::from("_locale"))
                    .cloned()
                    .unwrap_or_default(),
            );
        }
    }
}

/// `array_walk_recursive()` with upstream's caster: an object becomes the
/// array of its public properties (cast the same way), else its
/// `__toString()`, else `[]`; an object inside itself is refused.
fn cast_objects(
    array: &PhpArray,
    seen: &mut Vec<*const ()>,
    name: &[u8],
) -> Result<PhpArray, Error> {
    let mut out = PhpArray::new();
    for (key, value) in array.iter() {
        let value = match value {
            Value::Array(nested) => Value::Array(cast_objects(nested, seen, name)?),
            Value::Object(object) => {
                let id = Arc::as_ptr(object).cast::<()>();
                if seen.contains(&id) {
                    return Err(Error::new(
                        Class::InvalidParameter,
                        concat(&[
                            b"Parameters for route \"",
                            name,
                            b"\" cannot contain a circular reference (in object of class \"",
                            value.debug_type().as_bytes(),
                            b"\").",
                        ]),
                    ));
                }
                let vars = object.properties();
                if !vars.is_empty() {
                    seen.push(id);
                    let cast = cast_objects(&vars, seen, name);
                    seen.pop();
                    Value::Array(cast?)
                } else if object.is_stringable() {
                    object
                        .to_php_string()
                        .map_or_else(|| Value::Array(PhpArray::new()), Value::String)
                } else {
                    Value::Array(PhpArray::new())
                }
            }
            other => other.clone(),
        };
        out.insert(key.clone(), value);
    }
    Ok(out)
}

impl UrlGeneratorInterface for UrlGenerator {
    fn generate(
        &self,
        name: &[u8],
        mut parameters: PhpArray,
        reference_type: ReferenceType,
    ) -> Result<PhpString, Error> {
        let Some(route) = self.find(name, &parameters)? else {
            return Err(Error::new(
                Class::RouteNotFound,
                concat(&[
                    b"Unable to generate a URL for the named route \"",
                    name,
                    b"\" as such route does not exist.",
                ]),
            ));
        };

        // the Route has a cache of its own and is not recompiled as long as it does not get modified
        let mut owned;
        let route = if route.compiled().is_some() {
            route
        } else {
            owned = route.clone();
            owned.compile()?;
            &owned
        };
        let compiled = route
            .compiled()
            .ok_or_else(|| Error::new(Class::NotPorted, "route not compiled"))?;

        let defaults = route.defaults();
        let variables = compiled.variables();

        localize_parameters(defaults, variables, &mut parameters);

        self.do_generate(
            variables,
            defaults,
            route.requirements(),
            compiled.tokens(),
            parameters,
            name,
            reference_type,
            compiled.host_tokens(),
            route.schemes(),
        )
    }

    fn set_context(&mut self, context: RequestContext) {
        self.context = context;
    }

    fn context(&self) -> &RequestContext {
        &self.context
    }
}

impl ConfigurableRequirementsInterface for UrlGenerator {
    fn set_strict_requirements(&mut self, enabled: Option<bool>) {
        self.strict_requirements = enabled;
    }

    fn is_strict_requirements(&self) -> Option<bool> {
        self.strict_requirements
    }
}
