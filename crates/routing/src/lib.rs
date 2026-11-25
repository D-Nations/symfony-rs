//! Ports the demo's subset of Symfony Routing's `UrlMatcher.php`,
//! `RedirectableUrlMatcher.php` and `Generator/UrlGenerator.php`.
//! Routes are registered once as typed segments; there is no request-time
//! route-pattern or regular-expression compilation. See README for omissions.
//!
//! The modules [`route`], [`compiled_route`], [`route_compiler`], [`request_context`],
//! [`route_collection`], [`alias`] and [`url_generator`] port Symfony's own Route,
//! `CompiledRoute`, `RouteCompiler`, `RequestContext`, `RouteCollection`, Alias and
//! `UrlGenerator` classes; the typed matcher and generator here do not use them yet.
//!
//! Upstream: symfony/routing Matcher/UrlMatcher.php Matcher/RedirectableUrlMatcher.php Generator/UrlGenerator.php

#![forbid(unsafe_code)]

pub mod alias;
pub mod compiled_route;
pub mod compiled_url_generator;
pub mod compiled_url_generator_dumper;
pub mod exception;
pub mod generator;
mod ordered;
mod php;
pub mod request_context;
pub mod route;
pub mod route_collection;
pub mod route_compiler;
pub mod url_generator;

use symfony_http_foundation::query::rawurldecode;

/// The closed built-in requirement set, plus an application predicate.
#[derive(Clone, Copy)]
pub enum Requirement {
    NonEmpty,
    AsciiSlug,
    /// Positive decimal fitting the demo controllers' signed 64-bit argument.
    PositiveInt64,
    Custom(fn(&str) -> bool),
}

impl Requirement {
    fn accepts(self, text: &str) -> bool {
        match self {
            Self::NonEmpty => !text.is_empty(),
            Self::AsciiSlug => {
                !text.is_empty()
                    && text.split('-').all(|part| {
                        !part.is_empty() && part.bytes().all(|b| b.is_ascii_alphanumeric())
                    })
            }
            Self::PositiveInt64 => {
                text.as_bytes()
                    .first()
                    .is_some_and(|b| (b'1'..=b'9').contains(b))
                    && text.bytes().all(|b| b.is_ascii_digit())
                    && text.parse::<i64>().is_ok()
            }
            Self::Custom(predicate) => predicate(text),
        }
    }
}

#[derive(Clone, Copy)]
pub enum Segment {
    Literal(&'static str),
    Parameter(&'static str, Requirement),
}

#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct Parameters(Vec<(&'static str, String)>);

impl Parameters {
    pub fn get(&self, name: &str) -> Option<&str> {
        self.0
            .iter()
            .find(|(key, _)| *key == name)
            .map(|(_, value)| value.as_str())
    }
}

/// Applications keep route identity, methods and controller types.
/// Method names are canonical uppercase HTTP tokens, as Symfony Route stores them.
pub trait Target {
    fn methods(&self) -> Option<&'static [&'static str]>;
}

type TargetFactory<T> = dyn Fn(&Parameters) -> Option<T> + Send + Sync;

pub struct Definition<T> {
    segments: Vec<Segment>,
    target: Box<TargetFactory<T>>,
}

impl<T> Definition<T> {
    pub fn new(
        segments: Vec<Segment>,
        target: impl Fn(&Parameters) -> Option<T> + Send + Sync + 'static,
    ) -> Self {
        Self {
            segments,
            target: Box::new(target),
        }
    }

    fn resolve(&self, path: &str) -> Option<T> {
        let parts: Vec<_> = path.strip_prefix('/')?.split('/').collect();
        if parts.len() != self.segments.len() {
            return None;
        }
        let mut parameters = Parameters::default();
        for (segment, text) in self.segments.iter().zip(parts) {
            match segment {
                Segment::Literal(literal) if *literal == text => {}
                Segment::Parameter(name, requirement) if requirement.accepts(text) => {
                    parameters.0.push((name, text.to_owned()));
                }
                _ => return None,
            }
        }
        (self.target)(&parameters)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Match<T> {
    Found(T),
    MethodNotAllowed { allow: Vec<&'static str> },
    TrailingSlashRedirect { path: String, target: T },
    NotFound,
}

pub struct RouteCollection<T> {
    definitions: Vec<Definition<T>>,
}

impl<T: Target> Default for RouteCollection<T> {
    fn default() -> Self {
        Self {
            definitions: Vec::new(),
        }
    }
}

fn accepts(target: &impl Target, method: &str) -> bool {
    target.methods().is_none_or(|methods| {
        methods.contains(&method) || (method == "HEAD" && methods.contains(&"GET"))
    })
}

impl<T: Target> RouteCollection<T> {
    /// Registration order decides precedence, including slash redirects.
    pub fn add(&mut self, definition: Definition<T>) {
        self.definitions.push(definition);
    }

    pub fn match_request(&self, method: &str, raw_path: &str) -> Match<T> {
        let path = rawurldecode(if raw_path.is_empty() { "/" } else { raw_path });
        let alternative = if matches!(method, "GET" | "HEAD") && raw_path != "/" {
            let trimmed = raw_path.trim_end_matches('/');
            if trimmed.is_empty() {
                None
            } else if trimmed.len() < raw_path.len() {
                Some(trimmed.to_owned())
            } else {
                Some(format!("{raw_path}/"))
            }
        } else {
            None
        };
        let decoded_alternative = alternative.as_deref().map(rawurldecode);
        let mut allow = Vec::new();
        for definition in &self.definitions {
            if let Some(target) = definition.resolve(&path) {
                if accepts(&target, method) {
                    return Match::Found(target);
                }
                for allowed in target.methods().into_iter().flatten() {
                    if !allow.contains(allowed) {
                        allow.push(*allowed);
                    }
                }
            } else if let (Some(raw), Some(decoded)) = (&alternative, &decoded_alternative)
                && let Some(target) = definition.resolve(decoded)
                && accepts(&target, method)
            {
                return Match::TrailingSlashRedirect {
                    path: raw.clone(),
                    target,
                };
            }
        }
        if allow.is_empty() {
            Match::NotFound
        } else {
            Match::MethodNotAllowed { allow }
        }
    }
}
