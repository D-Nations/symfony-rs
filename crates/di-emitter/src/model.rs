//! The service graph the emitter compiles: definitions, aliases and
//! parameters of a compiled container, with arguments already split into
//! what the emitted code builds (references, closures, iterators, inline
//! definitions, scalars, `%env()%` strings).
//!
//! Two readers fill it: [`Graph::from_container_builder`] (the Rust
//! `ContainerBuilder`, `crate::builder`) and [`Graph::from_xml`] (a dump of
//! Symfony's `XmlDumper`, `crate::xml`, for containers recorded from PHP).
//! Both read strings by the rules of this file, so a container and its
//! XML dump give equal graphs.
//!
//! Upstream: none

use std::collections::HashMap;

use php_value::{Key, PhpArray, PhpString, Value};

use crate::Location;

/// Where the input defined something, for refusals. Two origins always
/// compare equal: a graph read from a `ContainerBuilder` (located by
/// service id) equals the one read from its XML dump (located by line).
#[derive(Clone, Debug)]
pub struct Origin(pub Location);

impl PartialEq for Origin {
    fn eq(&self, _other: &Self) -> bool {
        true
    }
}

impl Origin {
    pub fn location(&self) -> Location {
        self.0.clone()
    }
}

/// `on-invalid`: `ContainerInterface::*_REFERENCE`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum OnInvalid {
    Exception,
    Null,
    Ignore,
    IgnoreUninitialized,
}

/// One part of a string that may hold `%env(...)%` placeholders.
#[derive(Clone, Debug, PartialEq)]
pub enum StrPart {
    Text(String),
    Env(String),
}

#[derive(Clone, Debug, PartialEq)]
pub enum ArgDef {
    /// A scalar.
    Value(Value),
    /// A string holding at least one `%env(...)%`.
    Env(Vec<StrPart>),
    Collection(Vec<(Option<Key>, ArgDef)>),
    Ref {
        id: String,
        on_invalid: OnInvalid,
        origin: Origin,
    },
    Closure {
        id: String,
        on_invalid: OnInvalid,
        origin: Origin,
    },
    Iterator(Vec<(Option<Key>, ArgDef)>),
    /// An inline (anonymous) definition: index into `Graph::services`.
    Inline(usize),
    Constant(String),
}

#[derive(Clone, Debug, PartialEq)]
pub enum FactoryDef {
    /// A PHP function (`function="current"`).
    Function(String),
    Static {
        class: String,
        method: String,
    },
    Service {
        service: ArgDef,
        method: String,
    },
}

#[derive(Clone, Debug, PartialEq)]
pub struct CallDef {
    pub method: String,
    /// `returns-clone="true"`: a wither, `$service = $service->withX()`.
    pub returns_clone: bool,
    pub arguments: Vec<ArgDef>,
}

#[derive(Clone, Debug, PartialEq)]
#[expect(clippy::struct_excessive_bools, reason = "upstream's flags")]
pub struct Definition {
    /// The service id; for an inline definition, a generated
    /// `.inline.<n>` name (never visible to PHP), numbered in the order
    /// `XmlDumper` writes them.
    pub id: String,
    pub inline: bool,
    pub class: String,
    pub origin: Origin,
    pub public: bool,
    pub shared: bool,
    pub synthetic: bool,
    pub lazy: bool,
    pub factory: Option<FactoryDef>,
    pub arguments: Vec<ArgDef>,
    pub calls: Vec<CallDef>,
    pub properties: Vec<(String, ArgDef)>,
    pub configurator: Option<FactoryDef>,
    /// Tag names, once per tag (attributes are not read: the emitter
    /// uses names only).
    pub tags: Vec<String>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Alias {
    pub id: String,
    pub target: String,
    pub public: bool,
    pub origin: Origin,
}

#[derive(Debug, Default, PartialEq)]
pub struct Graph {
    /// The compiled parameters (unescaped: `%%` is `%`; env placeholders
    /// are written `%env(NAME)%`), after the dumper parameters.
    pub parameters: Vec<(String, Value)>,
    pub services: Vec<Definition>,
    pub aliases: Vec<Alias>,
}

/// The parameters `PhpDumper` adds when it writes the PHP container
/// (`container.build_hash`, `container.build_id`); they are not in the
/// `ContainerBuilder`, but its arguments may name them. Symfony derives
/// them from a hash of the dumped code; here, from a seed the caller
/// chooses (the demo: its container file).
#[derive(Clone, Debug, PartialEq)]
pub struct DumperParameters(pub Vec<(String, Value)>);

impl DumperParameters {
    pub fn from_seed(seed: &[u8]) -> Self {
        let hash = fnv1a(seed);
        #[expect(
            clippy::cast_possible_truncation,
            reason = "the low 32 bits name the build"
        )]
        let build_id = format!("{:08x}", hash as u32);
        Self(vec![
            (
                "container.build_hash".to_owned(),
                Value::string(format!("{hash:016x}")),
            ),
            ("container.build_id".to_owned(), Value::string(build_id)),
        ])
    }
}

fn fnv1a(bytes: &[u8]) -> u64 {
    bytes.iter().fold(0xcbf2_9ce4_8422_2325, |h, &b| {
        (h ^ u64::from(b)).wrapping_mul(0x0100_0000_01b3)
    })
}

/// The compiled parameters as the arguments' `%name%` see them: escaped
/// (`%%`), as `XmlDumper` writes them and as `PhpDumper::export()` reads
/// definitions.
pub type Parameters = HashMap<String, Value>;

/// A string value as `PhpDumper`'s `export()` reads it, with
/// `ParameterBag::resolveString()`'s pattern `/%%|%([^%\s]+)%/`: `%%` is
/// a literal `%`, `%env(...)%` is an env placeholder, `%name%` is a
/// parameter, and any other `%` is literal text (`"%d days"`).
pub fn split(s: &str, parameters: &Parameters, depth: u32) -> Result<Vec<StrPart>, String> {
    if depth > 32 {
        return Err(format!(
            "parameters refer to each other without end in \"{s}\""
        ));
    }
    let mut parts = Vec::new();
    let mut text = String::new();
    let mut rest = s;
    while let Some(i) = rest.find('%') {
        text.push_str(&rest[..i]);
        rest = &rest[i..];
        if let Some(r) = rest.strip_prefix("%%") {
            text.push('%');
            rest = r;
            continue;
        }
        let after = &rest[1..];
        let name = after
            .find('%')
            .map(|end| &after[..end])
            .filter(|n| !n.is_empty() && !n.chars().any(char::is_whitespace));
        let Some(name) = name else {
            text.push('%');
            rest = after;
            continue;
        };
        rest = &after[name.len() + 1..];
        if let Some(env) = name.strip_prefix("env(").and_then(|e| e.strip_suffix(')')) {
            if !text.is_empty() {
                parts.push(StrPart::Text(std::mem::take(&mut text)));
            }
            parts.push(StrPart::Env(env.to_owned()));
            continue;
        }
        match parameters.get(name) {
            Some(Value::String(v)) => {
                for p in split(
                    &String::from_utf8_lossy(v.as_bytes()),
                    parameters,
                    depth + 1,
                )? {
                    match p {
                        StrPart::Text(t) => text.push_str(&t),
                        env @ StrPart::Env(_) => {
                            if !text.is_empty() {
                                parts.push(StrPart::Text(std::mem::take(&mut text)));
                            }
                            parts.push(env);
                        }
                    }
                }
            }
            Some(Value::Int(n)) => text.push_str(&n.to_string()),
            Some(Value::Null) => {}
            Some(other) => {
                return Err(format!(
                    "A string value must be composed of strings and/or numbers, but found parameter \"{name}\" of type \"{}\" inside string value \"{s}\".",
                    other.debug_type()
                ));
            }
            None => {
                return Err(format!(
                    "You have requested a non-existent parameter \"{name}\"."
                ));
            }
        }
    }
    text.push_str(rest);
    if !text.is_empty() || parts.is_empty() {
        parts.push(StrPart::Text(text));
    }
    Ok(parts)
}

/// A string argument: `%name%` alone is the compiled parameter itself
/// (Symfony keeps whole-parameter references, so that `PhpDumper` can
/// share the parameter's value); otherwise text with `%%`, `%env()%` and
/// `%name%` parts.
pub fn string_arg(s: &str, parameters: &Parameters) -> Result<ArgDef, String> {
    if let Some(name) = s
        .strip_prefix('%')
        .and_then(|r| r.strip_suffix('%'))
        .filter(|n| {
            !n.is_empty()
                && !n.contains('%')
                && !n.chars().any(char::is_whitespace)
                && !n.starts_with("env(")
        })
    {
        let value = parameters
            .get(name)
            .ok_or_else(|| format!("You have requested a non-existent parameter \"{name}\"."))?;
        return value_arg(value, parameters);
    }
    let parts = split(s, parameters, 0)?;
    Ok(match parts.as_slice() {
        [StrPart::Text(t)] => ArgDef::Value(Value::string(t.as_str())),
        _ => ArgDef::Env(parts),
    })
}

/// A parameter's value as an argument. Parameter strings are escaped as
/// argument strings are (`%%`, `%env()%`).
fn value_arg(value: &Value, parameters: &Parameters) -> Result<ArgDef, String> {
    Ok(match value {
        Value::String(s) => string_arg(&String::from_utf8_lossy(s.as_bytes()), parameters)?,
        Value::Array(a) => ArgDef::Collection(
            a.iter()
                .map(|(k, v)| Ok((Some(k.clone()), value_arg(v, parameters)?)))
                .collect::<Result<_, String>>()?,
        ),
        v => ArgDef::Value(v.clone()),
    })
}

/// `ParameterBag::unescapeValue()` for a string `XmlDumper` escaped
/// after compilation, where `%env(NAME)%` was written afterwards: `%%` is
/// `%`, a `%name%` match stays as it is.
pub fn unescape(value: &Value) -> Value {
    match value {
        Value::String(s) => {
            let s = s.as_bytes();
            let mut out = Vec::with_capacity(s.len());
            let mut i = 0;
            while i < s.len() {
                if s[i] != b'%' {
                    out.push(s[i]);
                    i += 1;
                } else if s.get(i + 1) == Some(&b'%') {
                    out.push(b'%');
                    i += 2;
                } else {
                    // `%([^%\s]+)%` is kept whole, so that its closing
                    // `%` does not start a `%%`.
                    let name = s[i + 1..]
                        .iter()
                        .position(|&b| b == b'%' || b.is_ascii_whitespace())
                        .filter(|&n| n > 0 && s[i + 1 + n] == b'%');
                    let end = name.map_or(i + 1, |n| i + n + 2);
                    out.extend_from_slice(&s[i..end]);
                    i = end;
                }
            }
            Value::String(PhpString::from(out))
        }
        Value::Array(a) => {
            let mut out = PhpArray::new();
            for (k, v) in a.iter() {
                out.insert(k.clone(), unescape(v));
            }
            Value::Array(out)
        }
        other => other.clone(),
    }
}

/// `str_replace('%', '%%', ...)` on the strings of a parameter value, as
/// `XmlDumper` writes a compiled container's parameters.
pub fn escape(value: &Value) -> Value {
    match value {
        Value::String(s) => {
            let mut out = Vec::with_capacity(s.len());
            for &b in s.as_bytes() {
                out.push(b);
                if b == b'%' {
                    out.push(b'%');
                }
            }
            Value::String(PhpString::from(out))
        }
        Value::Array(a) => {
            let mut out = PhpArray::new();
            for (k, v) in a.iter() {
                out.insert(k.clone(), escape(v));
            }
            Value::Array(out)
        }
        other => other.clone(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn env_placeholders_and_escaped_percent() {
        let mut parameters = HashMap::new();
        parameters.insert("p".to_owned(), Value::string("v%%w"));
        assert_eq!(
            split("a%%b%env(X)%c%p%%d days", &parameters, 0),
            Ok(vec![
                StrPart::Text("a%b".into()),
                StrPart::Env("X".into()),
                StrPart::Text("cv%w%d days".into())
            ])
        );
        assert!(split("%missing%", &parameters, 0).is_err());
    }

    #[test]
    fn unescape_undoes_escape_around_env_names() {
        for raw in ["100%", "a%b%%c", "%env(X)%%env(Y)%", "50%d days", "%", ""] {
            let value = Value::string(raw);
            assert_eq!(unescape(&escape(&value)), value, "{raw}");
        }
        // The dumper writes env names after escaping.
        assert_eq!(
            unescape(&Value::string("a%%b%env(X)%%env(Y)%")),
            Value::string("a%b%env(X)%%env(Y)%")
        );
    }
}
