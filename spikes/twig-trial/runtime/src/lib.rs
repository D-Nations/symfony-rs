//! Trial Twig runtime: what the code emitted by `twig-trial-compiler`
//! calls. Values are `php-value` values; objects that Twig reads with `.`
//! are wrapped in [`Obj`] so attribute lookup can find their methods.
//! Strict variables are off, as in the demo's `prod` environment.
//!
//! Upstream: twig/twig src/Extension/CoreExtension.php src/Runtime/EscaperRuntime.php src/Markup.php src/Template.php

#![forbid(unsafe_code)]

use std::{cmp::Ordering, fmt, sync::Arc};

use php_value::numeric::{float_to_string, numeric_value};
pub use php_value::{Key, PhpArray, PhpObject, PhpString, Value};

/// The render context (`$context`).
pub type Context = PhpArray;

/// A compiled template's entry point.
pub type TemplateFn = fn(&Env, &mut Context, &mut String) -> Result<(), Error>;

/// The run-time environment: the compiled template registry.
pub struct Env {
    pub templates: fn(&str) -> Option<TemplateFn>,
}

/// A Twig `RuntimeError` / `LoaderError`: message, template and line.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Error {
    pub message: String,
    pub template: Option<String>,
    pub line: Option<u32>,
}

impl Error {
    pub fn new(message: impl Into<String>) -> Self {
        Self {
            message: message.into(),
            template: None,
            line: None,
        }
    }

    /// Adds the location, unless an inner call already set one.
    pub fn at(mut self, template: &str, line: u32) -> Self {
        if self.template.is_none() {
            self.template = Some(template.to_owned());
            self.line = Some(line);
        }
        self
    }
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match (&self.template, self.line) {
            (Some(t), Some(l)) => write!(
                f,
                "{} in \"{t}\" at line {l}.",
                self.message.trim_end_matches('.')
            ),
            _ => f.write_str(&self.message),
        }
    }
}

impl std::error::Error for Error {}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CallType {
    /// `a.b`: array key, property, then method.
    Any,
    /// `a.b()`: method only.
    Method,
}

/// What Twig needs from a PHP object besides `php-value`'s view of it:
/// public properties, public methods, and iteration (`\Traversable`).
/// A typed Rust struct implements this by hand in the trial; M4 would
/// generate it with a derive.
pub trait TwigObject: Send + Sync + fmt::Debug + 'static {
    fn class(&self) -> &str;

    /// A public property (`isset($object->$name)` or declared).
    fn property(&self, _name: &str) -> Option<Value> {
        None
    }

    /// Public method names as PHP declares them (`get_class_methods()`).
    fn methods(&self) -> &'static [&'static str] {
        &[]
    }

    /// Calls a method named as in [`TwigObject::methods`].
    fn call(&self, method: &str, _arguments: &[Value]) -> Result<Value, Error> {
        Err(Error::new(format!(
            "Call to undefined method {}::{method}()",
            self.class()
        )))
    }

    /// The items of a `\Traversable`, or `None`.
    fn iterate(&self) -> Option<Vec<(Value, Value)>> {
        None
    }

    fn to_php_string(&self) -> Option<PhpString> {
        None
    }
}

/// A [`TwigObject`] as a `php-value` object.
#[derive(Debug)]
pub struct Obj(pub Arc<dyn TwigObject>);

impl Obj {
    pub fn value(object: impl TwigObject) -> Value {
        Value::Object(Arc::new(Obj(Arc::new(object))))
    }
}

impl PhpObject for Obj {
    fn class(&self) -> &str {
        self.0.class()
    }

    fn properties(&self) -> PhpArray {
        PhpArray::new()
    }

    fn to_php_string(&self) -> Option<PhpString> {
        self.0.to_php_string()
    }
}

/// `Twig\Markup`: a string that is already safe for HTML.
#[derive(Debug)]
pub struct Markup(pub String);

impl Markup {
    pub fn value(html: String) -> Value {
        Value::Object(Arc::new(Markup(html)))
    }
}

impl PhpObject for Markup {
    fn class(&self) -> &str {
        "Twig\\Markup"
    }

    fn properties(&self) -> PhpArray {
        PhpArray::new()
    }

    fn to_php_string(&self) -> Option<PhpString> {
        Some(PhpString::from(self.0.as_str()))
    }
}

pub fn get(context: &Context, name: &str) -> Value {
    context
        .get(&Key::from(name))
        .cloned()
        .unwrap_or(Value::Null)
}

pub fn set(context: &mut Context, name: &str, value: Value) {
    context.insert(name, value);
}

/// ForNode's exit: the loop variables are unset, then
/// `$context = array_intersect_key($context, $_parent) + $_parent`, so a
/// loop variable never leaks and an outer variable of the same name
/// comes back.
pub fn restore_after_loop(context: &mut Context, parent: Context, loop_variables: &[&str]) {
    let mut restored = PhpArray::new();
    for (key, value) in context.iter() {
        if parent.contains_key(key) && !loop_variables.iter().any(|v| *key == Key::from(*v)) {
            restored.insert(key.clone(), value.clone());
        }
    }
    restored.union(&parent);
    *context = restored;
}

/// An ArrayExpression: keys `None` take the next index.
pub fn array(elements: Vec<(Option<Value>, Value)>) -> Value {
    let mut array = PhpArray::new();
    for (key, value) in elements {
        match key.as_ref().and_then(Key::from_value) {
            Some(key) => array.insert(key, value),
            None => {
                let _ = array.push(value);
            }
        }
    }
    Value::Array(array)
}

/// PHP's boolean conversion.
pub fn to_bool(value: &Value) -> bool {
    match value {
        Value::Null => false,
        Value::Bool(b) => *b,
        Value::Int(i) => *i != 0,
        Value::Float(f) => *f != 0.0,
        Value::String(s) => !(s.is_empty() || s.as_bytes() == b"0"),
        Value::Array(a) => !a.is_empty(),
        Value::Object(_) => true,
    }
}

fn number(value: &Value) -> Option<f64> {
    match value {
        Value::Int(i) => Some(*i as f64),
        Value::Float(f) => Some(*f),
        Value::String(s) => numeric_value(s.as_bytes()).map(|n| n.as_f64()),
        _ => None,
    }
}

/// PHP 8's `==` for scalars and null; arrays and objects compare by
/// identity in the trial.
pub fn loose_eq(a: &Value, b: &Value) -> bool {
    use Value::*;
    match (a, b) {
        (Null, Null) => true,
        (Bool(x), other) | (other, Bool(x)) => *x == to_bool(other),
        (Null, String(s)) | (String(s), Null) => s.is_empty(),
        (Null, other) | (other, Null) => !to_bool(other),
        (String(x), String(y)) => {
            match (numeric_value(x.as_bytes()), numeric_value(y.as_bytes())) {
                (Some(p), Some(q)) => p.as_f64() == q.as_f64(),
                _ => x.as_bytes() == y.as_bytes(),
            }
        }
        (Int(_) | Float(_), String(s)) | (String(s), Int(_) | Float(_)) => {
            let n = if let String(_) = a { b } else { a };
            match numeric_value(s.as_bytes()) {
                Some(m) => number(n) == Some(m.as_f64()),
                None => to_string(n).is_ok_and(|t| t.as_bytes() == s.as_bytes()),
            }
        }
        (Int(_) | Float(_), Int(_) | Float(_)) => number(a) == number(b),
        _ => a.identical(b),
    }
}

/// `<=>` for numbers and strings (the trial's subset).
pub fn compare(a: &Value, b: &Value) -> Ordering {
    match (number(a), number(b)) {
        (Some(x), Some(y)) => x.partial_cmp(&y).unwrap_or(Ordering::Equal),
        _ => match (to_string(a), to_string(b)) {
            (Ok(x), Ok(y)) => x.as_bytes().cmp(y.as_bytes()),
            _ => Ordering::Equal,
        },
    }
}

fn arithmetic(
    a: &Value,
    b: &Value,
    int: fn(i64, i64) -> Option<i64>,
    float: fn(f64, f64) -> f64,
) -> Value {
    match (a, b) {
        (Value::Int(x), Value::Int(y)) => {
            int(*x, *y).map_or_else(|| Value::Float(float(*x as f64, *y as f64)), Value::Int)
        }
        _ => Value::Float(float(number(a).unwrap_or(0.0), number(b).unwrap_or(0.0))),
    }
}

pub fn add(a: &Value, b: &Value) -> Value {
    arithmetic(a, b, i64::checked_add, |x, y| x + y)
}

pub fn sub(a: &Value, b: &Value) -> Value {
    arithmetic(a, b, i64::checked_sub, |x, y| x - y)
}

pub fn mul(a: &Value, b: &Value) -> Value {
    arithmetic(a, b, i64::checked_mul, |x, y| x * y)
}

pub fn neg(a: &Value) -> Value {
    mul(a, &Value::Int(-1))
}

pub fn pos(a: &Value) -> Value {
    mul(a, &Value::Int(1))
}

pub fn concat(a: &Value, b: &Value) -> Value {
    let mut bytes = to_string(a)
        .map(|s| s.as_bytes().to_vec())
        .unwrap_or_default();
    bytes.extend_from_slice(
        to_string(b)
            .map(|s| s.as_bytes().to_vec())
            .unwrap_or_default()
            .as_slice(),
    );
    Value::String(PhpString::from(bytes))
}

pub fn null_coalesce(a: Value, b: impl FnOnce() -> Value) -> Value {
    if a.is_null() { b() } else { a }
}

/// PHP's string conversion (`(string) $value`, as `echo` does it).
pub fn to_string(value: &Value) -> Result<PhpString, Error> {
    Ok(match value {
        Value::Null => PhpString::from(""),
        Value::Bool(true) => PhpString::from("1"),
        Value::Bool(false) => PhpString::from(""),
        Value::Int(i) => PhpString::from(i.to_string().as_str()),
        Value::Float(f) => PhpString::from(float_to_string(*f).as_str()),
        Value::String(s) => s.clone(),
        Value::Array(_) => return Err(Error::new("Array to string conversion")),
        Value::Object(o) => o.to_php_string().ok_or_else(|| {
            Error::new(format!(
                "Object of class {} could not be converted to string",
                o.class()
            ))
        })?,
    })
}

/// EscaperRuntime::escape() with the `html` strategy, for autoescaped
/// output: Markup is printed as is.
pub fn write_escaped_html(out: &mut String, value: &Value) -> Result<(), Error> {
    if let Value::Object(o) = value
        && let Some(markup) = o.downcast_ref::<Markup>()
    {
        out.push_str(&markup.0);
        return Ok(());
    }
    let text = to_string(value)?;
    out.push_str(&escape_html(&text.to_string_lossy()));
    Ok(())
}

pub fn write_raw(out: &mut String, value: &Value) -> Result<(), Error> {
    out.push_str(&to_string(value)?.to_string_lossy());
    Ok(())
}

/// `htmlspecialchars($s, ENT_QUOTES | ENT_SUBSTITUTE, 'UTF-8')` for
/// valid UTF-8 input.
pub fn escape_html(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for c in s.chars() {
        match c {
            '&' => out.push_str("&amp;"),
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            '"' => out.push_str("&quot;"),
            '\'' => out.push_str("&#039;"),
            c => out.push(c),
        }
    }
    out
}

/// CoreExtension::ensureTraversable() plus the foreach over it.
pub fn iterate(value: &Value) -> Vec<(Value, Value)> {
    match value {
        Value::Array(a) => a.iter().map(|(k, v)| (k.to_value(), v.clone())).collect(),
        Value::Object(o) => o
            .downcast_ref::<Obj>()
            .and_then(|obj| obj.0.iterate())
            .unwrap_or_default(),
        _ => Vec::new(),
    }
}

/// CoreExtension::getAttribute() without the sandbox, with strict
/// variables off: a missing key, property or method gives null.
pub fn get_attribute(
    _env: &Env,
    object: &Value,
    item: &str,
    arguments: &[Value],
    call: CallType,
) -> Result<Value, Error> {
    if call == CallType::Any
        && let Value::Array(array) = object
    {
        return Ok(array.get(&Key::from(item)).cloned().unwrap_or(Value::Null));
    }
    let Value::Object(o) = object else {
        return Ok(Value::Null);
    };
    let Some(Obj(obj)) = o.downcast_ref::<Obj>() else {
        return Ok(Value::Null);
    };
    if call == CallType::Any
        && let Some(value) = obj.property(item)
    {
        return Ok(value);
    }
    match find_method(obj.methods(), item) {
        Some(method) => obj.call(method, arguments),
        None => Ok(Value::Null),
    }
}

/// The method cache of getAttribute(): exact name, lower-case name, then
/// the `get`/`is`/`has` forms (a `has` form loses to an `is` form).
fn find_method(methods: &'static [&'static str], item: &str) -> Option<&'static str> {
    let mut sorted: Vec<&'static str> = methods.to_vec();
    sorted.sort_unstable();
    let lc_methods: Vec<String> = sorted.iter().map(|m| m.to_ascii_lowercase()).collect();
    let mut cache: Vec<(String, &'static str)> = Vec::new();
    let insert = |cache: &mut Vec<(String, &'static str)>, key: String, method| {
        if !cache.iter().any(|(k, _)| *k == key) {
            cache.push((key, method));
        }
    };
    for (method, lc) in sorted.iter().zip(&lc_methods) {
        cache.retain(|(k, _)| k != method && k != lc);
        cache.push(((*method).to_owned(), *method));
        cache.push((lc.clone(), *method));
        let prefix = if lc.starts_with("get") {
            3
        } else if lc.starts_with("is") {
            2
        } else if let Some(rest) = lc.strip_prefix("has") {
            if lc_methods.iter().any(|m| *m == format!("is{rest}")) {
                continue;
            }
            3
        } else {
            continue;
        };
        let lc_name = &lc[prefix..];
        if lc_name.is_empty() {
            continue;
        }
        let name = format!("{}{}", &lc_name[..1], &method[prefix + 1..]);
        insert(&mut cache, name, *method);
        insert(&mut cache, lc_name.to_owned(), *method);
    }
    let lookup = |key: &str| cache.iter().find(|(k, _)| k == key).map(|(_, m)| *m);
    lookup(item).or_else(|| lookup(&item.to_ascii_lowercase()))
}

/// CoreExtension::include() for a template compiled in the same set.
pub fn include_static(env: &Env, context: &Context, template: TemplateFn) -> Result<Value, Error> {
    let mut variables = context.clone();
    let mut out = String::new();
    template(env, &mut variables, &mut out)?;
    Ok(if out.is_empty() {
        Value::string("")
    } else {
        Markup::value(out)
    })
}

/// CoreExtension::include() through the compiled registry.
pub fn include(env: &Env, context: &Context, name: &Value) -> Result<Value, Error> {
    let name = to_string(name)?;
    let name = name.to_string_lossy();
    let template = (env.templates)(&name)
        .ok_or_else(|| Error::new(format!("Template \"{name}\" is not defined.")))?;
    include_static(env, context, template)
}
