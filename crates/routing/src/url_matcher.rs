//! Port of Symfony\Component\Routing\Matcher\UrlMatcher.php, with
//! UrlMatcherInterface.php: the attributes of the first route of a
//! [`RouteCollection`] that matches a path in a [`RequestContext`].
//!
//! Routes are tried in the collection's order (priorities first). A route
//! matches when its static prefix, path regex (made tolerant of a trailing
//! slash), host regex, condition, trailing slash, scheme and method all
//! agree; the methods and schemes of routes that matched on everything but
//! those are collected for `MethodNotAllowedException` and for the
//! redirecting matcher ([`crate::redirectable_url_matcher`]). The
//! attributes are a PHP array: the route's defaults, overridden by the
//! named matches that are not null, with `_route` (or the
//! `_canonical_route` default) and `_route_mapping`.
//!
//! Each route is compiled once, in [`UrlMatcher::new`]; a route that does
//! not compile fails the match when it is reached, as PHP's lazy
//! `compile()` does. Path and host regexes are compiled once per matcher.
//!
//! Not ported: route conditions, which need ExpressionLanguage. A route
//! with a condition throws upstream's `\LogicException` for a missing
//! ExpressionLanguage component when it is reached, which is what PHP
//! does without that component.
//!
//! `matchRequest()` (RequestMatcherInterface.php) matches the request's
//! path info on a copy of the context filled by
//! [`RequestContext::from_request`]; the matcher's own context is never
//! changed, which is what upstream's `finally` restores. `$this->request`,
//! which only route conditions read, is not kept.
//!
//! Symfony source copyright (c) Fabien Potencier; distributed under MIT.
//!
//! Upstream: symfony/routing Matcher/UrlMatcher.php Matcher/UrlMatcherInterface.php Matcher/RequestMatcherInterface.php

use std::{
    collections::HashMap,
    sync::{Arc, LazyLock, Mutex},
};

use php_pcre::{Pcre, PcreError};
use php_value::{Key, PhpArray, PhpString, Value, url::rawurldecode};
use symfony_http_foundation::request::Request;

use crate::{
    compiled_route::CompiledRoute,
    exception::{Class, Error, concat},
    php::{isset, preg_match, truthy},
    request_context::RequestContext,
    route::Route,
    route_collection::RouteCollection,
};

/// `UrlMatcher::REQUIREMENT_MATCH`, `REQUIREMENT_MISMATCH` and
/// `ROUTE_MATCH`: what `handleRouteRequirements()` reports.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RequirementStatus {
    Match,
    Mismatch,
    RouteMatch,
}

impl RequirementStatus {
    /// The constant's value.
    pub fn value(self) -> i64 {
        match self {
            Self::Match => 0,
            Self::Mismatch => 1,
            Self::RouteMatch => 2,
        }
    }
}

/// The message `getExpressionLanguage()` throws without the
/// ExpressionLanguage component.
const NO_EXPRESSION_LANGUAGE: &str = "Unable to use expressions as the Symfony ExpressionLanguage component is not installed. Try running \"composer require symfony/expression-language\".";

/// `'#\{[\w\x80-\xFF]+\}/?$#'`: a path ending in a variable.
static TRAILING_VARIABLE: LazyLock<Option<Pcre>> =
    LazyLock::new(|| Pcre::new(br"#\{[\w\x80-\xFF]+\}/?$#").ok());

/// A regex as `preg_match()` compiles it: `None` when PHP cannot compile
/// it either (`preg_match()` warns and returns false).
type Compiled = Option<Arc<Pcre>>;

/// What one `match()` collects: `$allow`, `$allowSchemes` and
/// `$trailingSlashSchemes`.
#[derive(Debug, Default, Clone)]
pub(crate) struct State {
    pub(crate) allow: Vec<PhpString>,
    pub(crate) allow_schemes: Vec<PhpString>,
    pub(crate) trailing_slash_schemes: Vec<PhpString>,
}

/// `UrlMatcher`.
pub struct UrlMatcher {
    routes: RouteCollection,
    context: RequestContext,
    patterns: Mutex<HashMap<Vec<u8>, Compiled>>,
}

impl std::fmt::Debug for UrlMatcher {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("UrlMatcher")
            .field("routes", &self.routes)
            .field("context", &self.context)
            .finish_non_exhaustive()
    }
}

impl UrlMatcher {
    /// `new UrlMatcher($routes, $context)`. The routes are compiled here.
    pub fn new(mut routes: RouteCollection, context: RequestContext) -> Self {
        let names: Vec<PhpString> = routes.all().into_iter().map(|(n, _)| n.clone()).collect();
        for name in names {
            if let Ok(Some(route)) = routes.get_mut(name.as_bytes()) {
                // A route that does not compile fails the match that reaches it.
                let _ = route.compile();
            }
        }
        Self {
            routes,
            context,
            patterns: Mutex::new(HashMap::new()),
        }
    }

    /// `getContext()`.
    pub fn context(&self) -> &RequestContext {
        &self.context
    }

    /// `getContext()` for changing the context in place.
    pub fn context_mut(&mut self) -> &mut RequestContext {
        &mut self.context
    }

    /// `setContext($context)`.
    pub fn set_context(&mut self, context: RequestContext) {
        self.context = context;
    }

    /// The routes the matcher was built with.
    pub fn routes(&self) -> &RouteCollection {
        &self.routes
    }

    /// `match($pathinfo)`: the attributes of the matching route, or
    /// `ResourceNotFoundException`, `MethodNotAllowedException` or
    /// `NoConfigurationException`.
    pub fn match_path(&self, pathinfo: &[u8]) -> Result<PhpArray, Error> {
        self.match_in(pathinfo, &self.context, &mut State::default(), false)
    }

    /// `matchRequest($request)`: `match($request->getPathInfo())` in the
    /// context of the request. A Request exception, or PHP's `\TypeError`
    /// for a port that is not an int, propagates.
    pub fn match_request(&self, request: &mut Request) -> Result<PhpArray, Error> {
        let context = self.request_context(request)?;
        let pathinfo = request.get_path_info()?;
        self.match_in(pathinfo.as_bytes(), &context, &mut State::default(), false)
    }

    /// `(clone $this->context)->fromRequest($request)`.
    pub(crate) fn request_context(&self, request: &mut Request) -> Result<RequestContext, Error> {
        let mut context = self.context.clone();
        context.from_request(request)?;
        Ok(context)
    }

    /// `match()` with the context and state it runs on; `redirectable` is
    /// `$this instanceof RedirectableUrlMatcherInterface`.
    pub(crate) fn match_in(
        &self,
        pathinfo: &[u8],
        context: &RequestContext,
        state: &mut State,
        redirectable: bool,
    ) -> Result<PhpArray, Error> {
        *state = State::default();
        let decoded = rawurldecode(pathinfo);
        let pathinfo: &[u8] = if decoded.is_empty() { b"/" } else { &decoded };

        let ret = self.match_collection(pathinfo, context, state, redirectable)?;
        if !ret.is_empty() {
            return Ok(ret);
        }

        if pathinfo == b"/" && state.allow.is_empty() && state.allow_schemes.is_empty() {
            return Err(Error::new(Class::NoConfiguration, ""));
        }

        if state.allow.is_empty() {
            Err(Error::new(
                Class::ResourceNotFound,
                concat(&[b"No routes found for \"", pathinfo, b"\"."]),
            ))
        } else {
            Err(Error::method_not_allowed(unique(&state.allow), ""))
        }
    }

    /// `matchCollection($pathinfo, $routes)`.
    fn match_collection(
        &self,
        pathinfo: &[u8],
        context: &RequestContext,
        state: &mut State,
        redirectable: bool,
    ) -> Result<PhpArray, Error> {
        // HEAD and GET are equivalent as per RFC
        let request_method = context.method().as_bytes();
        let method: &[u8] = if request_method == b"HEAD" {
            b"GET"
        } else {
            request_method
        };
        let supports_trailing_slash = method == b"GET" && redirectable;
        let trimmed = rtrim_slashes(pathinfo);
        let trimmed_pathinfo: &[u8] = if trimmed.is_empty() { b"/" } else { trimmed };

        for (name, route) in self.routes.all() {
            let mut uncompiled;
            let compiled_route: &CompiledRoute = if let Some(compiled) = route.compiled() {
                compiled
            } else {
                uncompiled = route.clone();
                uncompiled.compile()?
            };
            let static_prefix = rtrim_slashes(compiled_route.static_prefix().as_bytes());
            let required_methods = strings(route.methods());

            // check the static prefix of the URL first. Only use the more expensive preg_match when it matches
            if !static_prefix.is_empty() && !trimmed_pathinfo.starts_with(static_prefix) {
                continue;
            }
            let regex = compiled_route.regex().as_bytes();

            let Some(pos) = regex.iter().rposition(|&b| b == b'$') else {
                return Err(Error::new(
                    Class::NotPorted,
                    "a compiled route regex without '$'",
                ));
            };
            let has_trailing_slash = pos > 0 && regex[pos - 1] == b'/';
            let start = pos - usize::from(has_trailing_slash);
            let regex = concat(&[&regex[..start], b"/?$", &regex[pos + 1..]]);
            let Some(regex) = self.pattern(&regex)? else {
                continue;
            };

            let Some(mut matches) = preg_match(&regex, pathinfo) else {
                continue;
            };

            let mut has_trailing_var = trimmed_pathinfo != pathinfo
                && match TRAILING_VARIABLE.as_ref() {
                    Some(trailing) => trailing.is_match(route.path()).unwrap_or(false),
                    None => false,
                };

            if has_trailing_var {
                let index =
                    i64::try_from(compiled_route.path_variables().len()).unwrap_or(i64::MAX);
                let last = match matches.get(&Key::Int(index)) {
                    Some(Value::String(m)) => Some(m.as_bytes().last().copied().unwrap_or(b'/')),
                    Some(Value::Null) | None => None,
                    Some(_) => Some(b'/'),
                };
                if (has_trailing_slash || last.is_none_or(|c| c != b'/'))
                    && let Some(m) = preg_match(&regex, trimmed_pathinfo)
                {
                    if has_trailing_slash {
                        matches = m;
                    } else {
                        has_trailing_var = false;
                    }
                }
            }

            let mut host_matches = PhpArray::new();
            if let Some(host_regex) = compiled_route
                .host_regex()
                .filter(|r| truthy_bytes(r.as_bytes()))
            {
                let Some(host_regex) = self.pattern(host_regex.as_bytes())? else {
                    continue;
                };
                match preg_match(&host_regex, context.host().as_bytes()) {
                    Some(m) => host_matches = m,
                    None => continue,
                }
            }

            for (key, value) in host_matches.iter() {
                matches.insert(key.clone(), value.clone());
            }
            let attributes = get_attributes(route, name, matches);

            let status = handle_route_requirements(route)?;

            if status == RequirementStatus::Mismatch {
                continue;
            }

            if pathinfo != b"/"
                && !has_trailing_var
                && has_trailing_slash == (trimmed_pathinfo == pathinfo)
            {
                if supports_trailing_slash
                    && (required_methods.is_empty()
                        || required_methods.iter().any(|m| m.as_bytes() == b"GET"))
                {
                    state.trailing_slash_schemes = std::mem::take(&mut state.allow_schemes);
                    state.allow.clear();
                    return Ok(PhpArray::new());
                }
                continue;
            }

            let schemes = strings(route.schemes());
            if !schemes.is_empty() && !route.has_scheme(context.scheme().as_bytes()) {
                state.allow_schemes.extend(schemes);
                continue;
            }

            if !required_methods.is_empty()
                && !required_methods.iter().any(|m| m.as_bytes() == method)
                && !required_methods
                    .iter()
                    .any(|m| m.as_bytes() == request_method)
            {
                state.allow.extend(required_methods);
                continue;
            }

            return Ok(attributes);
        }

        Ok(PhpArray::new())
    }

    /// `preg_match($regex, ...)`'s compilation, cached.
    fn pattern(&self, pattern: &[u8]) -> Result<Compiled, Error> {
        let mut cache = match self.patterns.lock() {
            Ok(cache) => cache,
            Err(poisoned) => poisoned.into_inner(),
        };
        if let Some(compiled) = cache.get(pattern) {
            return Ok(compiled.clone());
        }
        let compiled = match Pcre::new(pattern) {
            Ok(regex) => Some(Arc::new(regex)),
            Err(PcreError::Compile(_)) => None,
            Err(e @ PcreError::Unsupported(_)) => {
                return Err(Error::new(Class::NotPorted, e.to_string()));
            }
        };
        cache.insert(pattern.to_vec(), compiled.clone());
        Ok(compiled)
    }
}

/// `rtrim($s, '/')`.
fn rtrim_slashes(s: &[u8]) -> &[u8] {
    let end = s.iter().rposition(|&b| b != b'/').map_or(0, |e| e + 1);
    &s[..end]
}

/// A non-empty string is truthy unless it is `"0"`.
fn truthy_bytes(s: &[u8]) -> bool {
    !s.is_empty() && s != b"0"
}

/// The values of a list of strings (a route's methods or schemes).
fn strings(array: &PhpArray) -> Vec<PhpString> {
    array
        .values()
        .filter_map(|v| match v {
            Value::String(s) => Some(s.clone()),
            _ => None,
        })
        .collect()
}

/// `array_unique($values)` for strings: the first of equal values, in
/// order.
fn unique(values: &[PhpString]) -> Vec<PhpString> {
    let mut seen: Vec<PhpString> = Vec::new();
    for value in values {
        if !seen.contains(value) {
            seen.push(value.clone());
        }
    }
    seen
}

/// `getAttributes($route, $name, $attributes)`.
fn get_attributes(route: &Route, name: &PhpString, mut attributes: PhpArray) -> PhpArray {
    let mut defaults = route.defaults().clone();
    let mut name = Value::String(name.clone());
    if isset(&defaults, b"_canonical_route") {
        let key = Key::from_bytes(b"_canonical_route");
        if let Some(canonical) = defaults.remove(&key) {
            name = canonical;
        }
    }
    attributes.insert(Key::from_bytes(b"_route"), name);

    if let Some(mapping) = route.option(b"mapping").filter(|m| truthy(m)) {
        attributes.insert(Key::from_bytes(b"_route_mapping"), mapping.clone());
    }

    merge_defaults(&attributes, defaults)
}

/// `handleRouteRequirements(...)`: a route with a condition needs
/// ExpressionLanguage.
fn handle_route_requirements(route: &Route) -> Result<RequirementStatus, Error> {
    if !route.condition().is_empty() {
        return Err(Error::new(Class::PhpLogic, NO_EXPRESSION_LANGUAGE));
    }
    Ok(RequirementStatus::Match)
}

/// `mergeDefaults($params, $defaults)`: each parameter with a string key
/// and a value that is not null replaces the default of that name.
fn merge_defaults(params: &PhpArray, mut defaults: PhpArray) -> PhpArray {
    for (key, value) in params.iter() {
        if !matches!(key, Key::Int(_)) && !value.is_null() {
            defaults.insert(key.clone(), value.clone());
        }
    }
    defaults
}
