//! Trial build-time Twig compiler: reads `.twig` files in `build.rs` and
//! emits Rust that renders them with `twig-trial-runtime`. Not the M4
//! port: it supports the language subset that `blog/_post.html.twig` and
//! `blog/_post_tags.html.twig` from symfony/demo use, and refuses
//! everything else with the template name and line (see README.md).
//!
//! Upstream: twig/twig src/Environment.php src/FileExtensionEscapingStrategy.php

#![forbid(unsafe_code)]

pub mod emit;
pub mod lexer;
pub mod parser;
pub mod prelexer;

use std::fmt;

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Number {
    Int(i64),
    Float(f64),
}

impl fmt::Display for Number {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Number::Int(i) => write!(f, "{i}"),
            Number::Float(x) => write!(f, "{x}"),
        }
    }
}

/// A located compile error (Twig's `SyntaxError`: message, template, line).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Error {
    pub message: String,
    pub template: String,
    pub line: usize,
}

impl Error {
    pub fn syntax(message: impl Into<String>, template: &str, line: usize) -> Self {
        Self {
            message: message.into(),
            template: template.to_owned(),
            line,
        }
    }
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        // Twig's SyntaxError::updateRepr() form.
        let message = self.message.trim_end_matches('.');
        write!(
            f,
            "{message} in \"{}\" at line {}.",
            self.template, self.line
        )
    }
}

impl std::error::Error for Error {}

/// A function or filter an extension registers: its Twig name, the Rust
/// path the generated code calls, and the escaping strategies for which
/// its output is already safe (`is_safe`).
#[derive(Debug, Clone)]
pub struct Callable {
    pub name: String,
    pub rust_path: String,
    pub safe_html: bool,
}

impl Callable {
    pub fn new(name: &str, rust_path: &str, safe_html: bool) -> Self {
        Self {
            name: name.to_owned(),
            rust_path: rust_path.to_owned(),
            safe_html,
        }
    }
}

/// The compile-time view of a Twig `Environment`: the extensions'
/// functions and filters, and whether the `<twig:...>` pre-lexer of
/// symfony/ux-twig-component runs (ComponentLexer).
#[derive(Debug, Clone, Default)]
pub struct Environment {
    pub functions: Vec<Callable>,
    pub filters: Vec<Callable>,
    pub component_pre_lexer: bool,
}

impl Environment {
    pub fn function(&self, name: &str) -> Option<&Callable> {
        self.functions.iter().find(|c| c.name == name)
    }

    pub fn filter(&self, name: &str) -> Option<&Callable> {
        self.filters.iter().find(|c| c.name == name)
    }
}

/// FileExtensionEscapingStrategy::guess(), the `autoescape: name`
/// default of TwigBundle; `None` means no escaping (`.txt`).
pub fn escaping_strategy(name: &str) -> Option<&'static str> {
    if name.ends_with('/') || name.ends_with('\\') {
        return Some("html");
    }
    let name = name.strip_suffix(".twig").unwrap_or(name);
    let file = name.rsplit(['/', '\\']).next().unwrap_or(name);
    match file.rsplit_once('.').map(|(_, e)| e) {
        Some("js") => Some("js"),
        Some("css") => Some("css"),
        Some("txt") => None,
        _ => Some("html"),
    }
}

/// Parses one template into its node tree.
pub fn parse(env: &Environment, name: &str, source: &str) -> Result<Vec<parser::Node>, Error> {
    let source = if env.component_pre_lexer {
        prelexer::pre_lex(name, source)?
    } else {
        source.to_owned()
    };
    let tokens = lexer::tokenize(name, &source)?;
    parser::parse(name, tokens)
}

/// Compiles `(name, source)` templates into one Rust module's source.
pub fn compile(env: &Environment, templates: &[(String, String)]) -> Result<String, Error> {
    let mut parsed = Vec::new();
    for (name, source) in templates {
        if escaping_strategy(name) != Some("html") {
            return Err(Error::syntax(
                "The Twig trial compiler supports the \"html\" escaping strategy only.",
                name,
                1,
            ));
        }
        parsed.push((name.clone(), parse(env, name, source)?));
    }
    emit::emit(env, &parsed)
}
