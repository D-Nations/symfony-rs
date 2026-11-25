//! Port of Symfony\Component\Routing\Matcher\RedirectableUrlMatcher.php,
//! with RedirectableUrlMatcherInterface.php: a [`UrlMatcher`] that, for a
//! GET or HEAD request no route matches, tries the path with its trailing
//! slash added or removed, then the schemes the routes require, and lets
//! [`RedirectableUrlMatcherInterface::redirect`] turn a match into a
//! redirect.
//!
//! Upstream's `goto redirect_scheme` is a loop over the same steps. Each
//! retry is a fresh `match()` (it resets the collected methods and
//! schemes, as PHP's does), on a copy of the context with the scheme
//! changed; the matcher's own context is never modified. Only routing
//! exceptions are caught on a retry, as `catch (ExceptionInterface)` does.
//!
//! `redirect()` receives the matched `_route` as PHP passes a value to a
//! `string` parameter; a `\TypeError` for a value that is not a string
//! names `RedirectableUrlMatcher::redirect()`, where PHP names the
//! concrete subclass.
//!
//! Symfony source copyright (c) Fabien Potencier; distributed under MIT.
//!
//! Upstream: symfony/routing Matcher/RedirectableUrlMatcher.php Matcher/RedirectableUrlMatcherInterface.php

use php_value::{Key, PhpArray, PhpString, Value};

use crate::{
    exception::Error,
    php::string_argument,
    request_context::RequestContext,
    route_collection::RouteCollection,
    url_matcher::{State, UrlMatcher},
};

const EXCEPTION_INTERFACE: &str = "Symfony\\Component\\Routing\\Exception\\ExceptionInterface";
const RESOURCE_NOT_FOUND: &str =
    "Symfony\\Component\\Routing\\Exception\\ResourceNotFoundException";
const REDIRECT: &str = "Symfony\\Component\\Routing\\Matcher\\RedirectableUrlMatcher::redirect";

/// `RedirectableUrlMatcherInterface`: what the matcher returns for a
/// redirect to `path` (still URL-encoded) for `route`, with the scheme to
/// redirect to when it changes. The array is merged with the route's
/// attributes (`redirect(...) + $attributes`).
pub trait RedirectableUrlMatcherInterface {
    fn redirect(
        &self,
        path: &[u8],
        route: &PhpString,
        scheme: Option<&[u8]>,
    ) -> Result<PhpArray, Error>;
}

/// `RedirectableUrlMatcher`, the abstract class, with its `redirect()`.
pub struct RedirectableUrlMatcher<R> {
    matcher: UrlMatcher,
    redirector: R,
}

impl<R> std::fmt::Debug for RedirectableUrlMatcher<R> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("RedirectableUrlMatcher")
            .field("matcher", &self.matcher)
            .finish_non_exhaustive()
    }
}

/// Where `match()` goes after the plain match failed.
enum Step {
    /// `redirect_scheme:` with the path to match.
    Scheme(Vec<u8>),
    /// The path with its trailing slash added or removed.
    Slash,
}

impl<R: RedirectableUrlMatcherInterface> RedirectableUrlMatcher<R> {
    /// `new class($routes, $context) extends RedirectableUrlMatcher`.
    pub fn new(routes: RouteCollection, context: RequestContext, redirector: R) -> Self {
        Self {
            matcher: UrlMatcher::new(routes, context),
            redirector,
        }
    }

    /// The matcher it extends.
    pub fn matcher(&self) -> &UrlMatcher {
        &self.matcher
    }

    /// The matcher it extends, for changing its context.
    pub fn matcher_mut(&mut self) -> &mut UrlMatcher {
        &mut self.matcher
    }

    /// The `redirect()` implementation.
    pub fn redirector(&self) -> &R {
        &self.redirector
    }

    /// `getContext()`.
    pub fn context(&self) -> &RequestContext {
        self.matcher.context()
    }

    /// `match($pathinfo)`.
    pub fn match_path(&self, pathinfo: &[u8]) -> Result<PhpArray, Error> {
        let context = self.matcher.context();
        let mut state = State::default();
        let e = match self.matcher.match_in(pathinfo, context, &mut state, true) {
            Ok(ret) => return Ok(ret),
            Err(e) if e.is_a(RESOURCE_NOT_FOUND) => e,
            Err(e) => return Err(e),
        };
        if !matches!(context.method().as_bytes(), b"HEAD" | b"GET") {
            return Err(e);
        }

        let mut step = if state.allow_schemes.is_empty() {
            if pathinfo.iter().all(|&b| b == b'/') {
                return Err(e);
            }
            Step::Slash
        } else {
            Step::Scheme(pathinfo.to_vec())
        };

        loop {
            match step {
                Step::Scheme(pathinfo) => {
                    let Some(scheme) = state.allow_schemes.first() else {
                        return Err(e);
                    };
                    let mut redirected = context.clone();
                    redirected.set_scheme(scheme.as_bytes());
                    let result = self
                        .matcher
                        .match_in(&pathinfo, &redirected, &mut state, true)
                        .and_then(|ret| {
                            self.redirect(&pathinfo, &ret, Some(redirected.scheme().as_bytes()))
                        });
                    return match result {
                        Ok(ret) => Ok(ret),
                        Err(err) if err.is_a(EXCEPTION_INTERFACE) => Err(e),
                        Err(err) => Err(err),
                    };
                }
                Step::Slash => {
                    let fallback_schemes = state.trailing_slash_schemes.clone();
                    let trimmed = rtrim_slashes(pathinfo);
                    let slashed = if trimmed == pathinfo {
                        [pathinfo, b"/"].concat()
                    } else {
                        trimmed.to_vec()
                    };
                    let result = self
                        .matcher
                        .match_in(&slashed, context, &mut state, true)
                        .and_then(|ret| self.redirect(&slashed, &ret, None));
                    match result {
                        Ok(ret) => return Ok(ret),
                        Err(err) if err.is_a(EXCEPTION_INTERFACE) => {}
                        Err(err) => return Err(err),
                    }
                    if !state.allow_schemes.is_empty() {
                        step = Step::Scheme(slashed);
                    } else if !state.trailing_slash_schemes.is_empty() {
                        state.allow_schemes = state.trailing_slash_schemes.clone();
                        step = Step::Scheme(slashed);
                    } else if !fallback_schemes.is_empty() {
                        state.allow_schemes = fallback_schemes;
                        step = Step::Scheme(pathinfo.to_vec());
                    } else {
                        return Err(e);
                    }
                }
            }
        }
    }

    /// `$this->redirect($pathinfo, $ret['_route'] ?? null, $scheme) + $ret`.
    fn redirect(
        &self,
        pathinfo: &[u8],
        ret: &PhpArray,
        scheme: Option<&[u8]>,
    ) -> Result<PhpArray, Error> {
        let route = ret
            .get(&Key::from_bytes(b"_route"))
            .cloned()
            .unwrap_or(Value::Null);
        let route = string_argument(&route, REDIRECT, 2, "route")?;
        let mut redirect = self.redirector.redirect(pathinfo, &route, scheme)?;
        redirect.union(ret);
        Ok(redirect)
    }
}

/// `rtrim($s, '/')`.
fn rtrim_slashes(s: &[u8]) -> &[u8] {
    let end = s.iter().rposition(|&b| b != b'/').map_or(0, |e| e + 1);
    &s[..end]
}
