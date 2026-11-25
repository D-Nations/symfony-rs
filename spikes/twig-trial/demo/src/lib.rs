//! Twig trial: symfony/demo's `blog/_post.html.twig` and
//! `blog/_post_tags.html.twig`, unchanged, compiled to Rust by build.rs.
//!
//! `model` holds typed stand-ins for the demo's entities, with the same
//! public method lists, so Twig's attribute lookup resolves as in PHP.
//! `ext` holds the stubbed bridges, written exactly as the PHP recorder
//! (`oracle/recorders/twig_trial.php`) writes them.
//!
//! Upstream: none

#![forbid(unsafe_code)]

/// Generated code: a template that never reads the context or calls an
/// extension leaves those parameters unused.
#[allow(unused_variables)]
pub mod templates {
    include!(concat!(env!("OUT_DIR"), "/templates.rs"));
}

use twig_trial_runtime::{Env, Error, Value};

pub fn env() -> Env {
    Env {
        templates: templates::template,
    }
}

/// Renders one template by name.
pub fn render(name: &str, context: &mut twig_trial_runtime::Context) -> Result<String, Error> {
    let env = env();
    let template = templates::template(name)
        .ok_or_else(|| Error::new(format!("Unable to find template \"{name}\".")))?;
    let mut out = String::new();
    template(&env, context, &mut out)?;
    Ok(out)
}

pub mod ext {
    //! Stubs for bridges whose providers do not exist yet: `path()`
    //! (Routing, M2-A), `component()` (UX `TwigComponent` + Icons, M5) and
    //! `format_datetime` (twig/intl-extra, M5).

    use php_value::{Key, datetime::DateTimeImmutable};
    use std::fmt::Write as _;
    use twig_trial_runtime::{Env, Error, Value, escape_html, to_string};

    fn arg(arguments: &[Value], index: usize) -> Value {
        arguments.get(index).cloned().unwrap_or(Value::Null)
    }

    fn text(value: &Value) -> Result<String, Error> {
        Ok(to_string(value)?.to_string_lossy().into_owned())
    }

    fn key_text(key: &Key) -> String {
        match key.to_value() {
            Value::Int(i) => i.to_string(),
            other => to_string(&other)
                .map(|s| s.to_string_lossy().into_owned())
                .unwrap_or_default(),
        }
    }

    pub fn path(_env: &Env, arguments: &[Value]) -> Result<Value, Error> {
        let name = text(&arg(arguments, 0))?;
        let mut pairs = Vec::new();
        if let Value::Array(parameters) = arg(arguments, 1) {
            for (key, value) in parameters.iter() {
                let value = if value.is_null() {
                    "~".to_owned()
                } else {
                    text(value)?
                };
                pairs.push(format!("{}={value}", key_text(key)));
            }
        }
        Ok(Value::string(format!("/{name}?{}", pairs.join("&"))))
    }

    pub fn component(_env: &Env, arguments: &[Value]) -> Result<Value, Error> {
        let mut html = format!(
            "<stub-component name=\"{}\"",
            escape_html(&text(&arg(arguments, 0))?)
        );
        if let Value::Array(props) = arg(arguments, 1) {
            for (key, value) in props.iter() {
                let _ = write!(
                    html,
                    " {}=\"{}\"",
                    key_text(key),
                    escape_html(&text(value)?)
                );
            }
        }
        html.push_str("/>");
        Ok(Value::string(html))
    }

    #[expect(
        clippy::needless_pass_by_value,
        reason = "one signature for every Twig filter"
    )]
    pub fn format_datetime(_env: &Env, date: Value, arguments: &[Value]) -> Result<Value, Error> {
        let Some(date) = date
            .as_object()
            .and_then(|o| o.downcast_ref::<DateTimeImmutable>())
        else {
            return Err(Error::new(
                "format_datetime(): the trial stub takes a DateTimeImmutable.",
            ));
        };
        let or = |i: usize, default: &str| -> Result<String, Error> {
            match arguments.get(i) {
                None => Ok(default.to_owned()),
                Some(v) => text(v),
            }
        };
        Ok(Value::string(format!(
            "[{} {}/{}/{}/{}]",
            date.format("Y-m-d H:i:s"),
            or(0, "medium")?,
            or(1, "medium")?,
            or(2, "")?,
            or(3, "")?
        )))
    }
}

pub mod model {
    //! Typed stand-ins for App\Entity\Post, Tag and User, Doctrine's
    //! `ArrayCollection`, and the HTTP objects that `app.request.query`
    //! reaches. M4 would generate the `TwigObject` impls with a derive.

    use std::sync::Arc;

    use php_value::datetime::DateTimeImmutable;
    use twig_trial_runtime::{Error, Obj, PhpString, TwigObject, Value};

    #[derive(Debug)]
    pub struct Post {
        pub title: String,
        pub slug: String,
        pub summary: String,
        pub published_at: DateTimeImmutable,
        pub author: Value,
        pub tags: Value,
    }

    impl TwigObject for Post {
        fn class(&self) -> &'static str {
            "App\\Entity\\Post"
        }

        fn methods(&self) -> &'static [&'static str] {
            &[
                "__construct",
                "getId",
                "getTitle",
                "setTitle",
                "getSlug",
                "setSlug",
                "getContent",
                "setContent",
                "getPublishedAt",
                "setPublishedAt",
                "getAuthor",
                "setAuthor",
                "getComments",
                "addComment",
                "removeComment",
                "getSummary",
                "setSummary",
                "addTag",
                "removeTag",
                "getTags",
            ]
        }

        fn call(&self, method: &str, _arguments: &[Value]) -> Result<Value, Error> {
            Ok(match method {
                "getTitle" => Value::string(self.title.as_str()),
                "getSlug" => Value::string(self.slug.as_str()),
                "getSummary" => Value::string(self.summary.as_str()),
                "getPublishedAt" => Value::Object(Arc::new(self.published_at)),
                "getAuthor" => self.author.clone(),
                "getTags" => self.tags.clone(),
                "getComments" => Obj::value(Collection(Vec::new())),
                _ => Value::Null,
            })
        }
    }

    #[derive(Debug)]
    pub struct Tag(pub String);

    impl TwigObject for Tag {
        fn class(&self) -> &'static str {
            "App\\Entity\\Tag"
        }

        fn methods(&self) -> &'static [&'static str] {
            &[
                "__construct",
                "getId",
                "getName",
                "jsonSerialize",
                "__toString",
            ]
        }

        fn call(&self, method: &str, _arguments: &[Value]) -> Result<Value, Error> {
            Ok(match method {
                "getName" | "jsonSerialize" | "__toString" => Value::string(self.0.as_str()),
                _ => Value::Null,
            })
        }

        fn to_php_string(&self) -> Option<PhpString> {
            Some(PhpString::from(self.0.as_str()))
        }
    }

    #[derive(Debug)]
    pub struct User(pub String);

    impl TwigObject for User {
        fn class(&self) -> &'static str {
            "App\\Entity\\User"
        }

        fn methods(&self) -> &'static [&'static str] {
            &["getId", "setFullName", "getFullName"]
        }

        fn call(&self, method: &str, _arguments: &[Value]) -> Result<Value, Error> {
            Ok(if method == "getFullName" {
                Value::string(self.0.as_str())
            } else {
                Value::Null
            })
        }
    }

    /// The part of Doctrine's `ArrayCollection` the templates use.
    #[derive(Debug)]
    pub struct Collection(pub Vec<Value>);

    impl TwigObject for Collection {
        fn class(&self) -> &'static str {
            "Doctrine\\Common\\Collections\\ArrayCollection"
        }

        fn methods(&self) -> &'static [&'static str] {
            &["__construct", "isEmpty", "count", "toArray", "getIterator"]
        }

        fn call(&self, method: &str, _arguments: &[Value]) -> Result<Value, Error> {
            Ok(match method {
                "isEmpty" => Value::Bool(self.0.is_empty()),
                "count" => Value::Int(i64::try_from(self.0.len()).unwrap_or(i64::MAX)),
                _ => Value::Null,
            })
        }

        fn iterate(&self) -> Option<Vec<(Value, Value)>> {
            Some(
                self.0
                    .iter()
                    .enumerate()
                    .map(|(i, v)| (Value::Int(i64::try_from(i).unwrap_or(i64::MAX)), v.clone()))
                    .collect(),
            )
        }
    }

    /// Stub of Symfony's `AppVariable` (the `app` global).
    #[derive(Debug)]
    pub struct App {
        pub request: Value,
    }

    impl TwigObject for App {
        fn class(&self) -> &'static str {
            "Symfony\\Bridge\\Twig\\AppVariable"
        }

        fn methods(&self) -> &'static [&'static str] {
            &["getRequest"]
        }

        fn call(&self, method: &str, _arguments: &[Value]) -> Result<Value, Error> {
            Ok(if method == "getRequest" {
                self.request.clone()
            } else {
                Value::Null
            })
        }
    }

    /// Stub of Request: only its public `$query`.
    #[derive(Debug)]
    pub struct Request {
        pub query: Value,
    }

    impl TwigObject for Request {
        fn class(&self) -> &'static str {
            "Symfony\\Component\\HttpFoundation\\Request"
        }

        fn property(&self, name: &str) -> Option<Value> {
            (name == "query").then(|| self.query.clone())
        }
    }

    /// Stub of `InputBag`: `get($key, $default = null)` and `all()`.
    #[derive(Debug)]
    pub struct InputBag(pub php_value::PhpArray);

    impl TwigObject for InputBag {
        fn class(&self) -> &'static str {
            "Symfony\\Component\\HttpFoundation\\InputBag"
        }

        fn methods(&self) -> &'static [&'static str] {
            &["get", "all"]
        }

        fn call(&self, method: &str, arguments: &[Value]) -> Result<Value, Error> {
            Ok(match method {
                "get" => {
                    let key = arguments
                        .first()
                        .map(twig_trial_runtime::to_string)
                        .transpose()?;
                    let default = arguments.get(1).cloned().unwrap_or(Value::Null);
                    key.and_then(|k| self.0.get(&php_value::Key::from(k)).cloned())
                        .filter(|v| !v.is_null())
                        .unwrap_or(default)
                }
                "all" => Value::Array(self.0.clone()),
                _ => Value::Null,
            })
        }
    }
}

/// Reports a value as Twig would print it, for tests.
pub fn print(value: &Value) -> String {
    twig_trial_runtime::to_string(value)
        .map(|s| s.to_string_lossy().into_owned())
        .unwrap_or_default()
}
