//! Port of Symfony\Component\DependencyInjection\EnvVarProcessor.php: the
//! processor behind every `%env(prefix:NAME)%` prefix that
//! `getProvidedTypes()` lists.
//!
//! PHP's process state is passed in, not read from globals:
//!
//! - `$_ENV`, `$_SERVER` and `getenv()` are a symfony-dotenv
//!   [`Globals`], shared behind a lock (the CLI's own superglobals by
//!   default, [`Globals::from_process`]). The lookup order is upstream's:
//!   `$_ENV`, then `$_SERVER` (not for `HTTP_*` names), then `getenv()`,
//!   then the loaders, then the container's `env(NAME)` parameter.
//! - The constants and enums that `const:` and `enum:` read are a php-value
//!   [`Registry`] ([`Registry::with_core`] by default).
//! - The loaders (`\Traversable<EnvVarLoaderInterface>`) are an
//!   [`EnvVarLoaders`]: a list ([`Vec`], PHP's `\ArrayIterator`), or any
//!   iterable that may fail while iterating, as a `RewindableGenerator`'s
//!   generator may.
//!
//! What cannot be kept:
//!
//! - `require:` runs a PHP file: it checks the file as upstream does, then
//!   refuses with a `RuntimeException`.
//! - `shuffle:` draws from PHP's Mt19937 generator, which `srand()` seeds;
//!   the order here is random but not PHP's for a seed.
//! - `reset()` cannot call `Container::resetEnvCache()`: the
//!   [`ContainerInterface`] trait has no such method yet.
//! - Parameter names inside a `resolve:` value are UTF-8 (this crate's
//!   identifiers); other bytes are replaced with U+FFFD.
//!
//! Upstream: symfony/dependency-injection EnvVarProcessor.php

mod builtins;

use std::sync::{Arc, Mutex, PoisonError, RwLock};

use php_value::{
    Key, PhpArray, PhpString, Value, base64,
    constants::Registry,
    filter::{FILTER_VALIDATE_BOOL, FILTER_VALIDATE_INT, filter_var},
    json,
    object::{EnumFromError, backed_enum_from},
    url, vars,
};
use symfony_dotenv::Globals;

use crate::container_interface::ContainerInterface;
use crate::env_var_loader_interface::EnvVarLoaderInterface;
use crate::env_var_processor_interface::EnvVarProcessorInterface;
use crate::exception::{Error, ErrorKind, Result};

/// The loaders an [`EnvVarProcessor`] iterates: PHP's
/// `\Traversable<EnvVarLoaderInterface>`.
pub trait EnvVarLoaders: Send + Sync {
    /// `foreach ($loaders as $loader)`: each loader, or what the iteration
    /// threw (a generator may throw before it yields).
    fn loaders(&self) -> Box<dyn Iterator<Item = Result<Arc<dyn EnvVarLoaderInterface>>> + '_>;

    /// `$loaders->count()` when the loaders are `\Countable`.
    fn count(&self) -> Option<usize>;
}

/// `new \ArrayIterator($loaders)`.
impl EnvVarLoaders for Vec<Arc<dyn EnvVarLoaderInterface>> {
    fn loaders(&self) -> Box<dyn Iterator<Item = Result<Arc<dyn EnvVarLoaderInterface>>> + '_> {
        Box::new(self.iter().cloned().map(Ok))
    }

    fn count(&self) -> Option<usize> {
        Some(self.len())
    }
}

/// `EnvVarProcessor::getProvidedTypes()`.
pub const PROVIDED_TYPES: &[(&str, &str)] = &[
    ("base64", "string"),
    ("bool", "bool"),
    ("not", "bool"),
    ("const", "bool|int|float|string|array"),
    ("csv", "array"),
    ("file", "string"),
    ("float", "float"),
    ("int", "int"),
    ("json", "array"),
    ("key", "bool|int|float|string|array"),
    ("url", "array"),
    ("query_string", "array"),
    ("resolve", "string"),
    ("default", "bool|int|float|string|array"),
    ("string", "string"),
    ("trim", "string"),
    ("require", "bool|int|float|string|array"),
    ("enum", "BackedEnum"),
    ("shuffle", "array"),
    ("defined", "bool"),
    ("urlencode", "string"),
];

/// `EnvVarProcessor`.
pub struct EnvVarProcessor {
    container: Arc<dyn ContainerInterface>,
    /// `$this->loaders`; `None` is an empty `\ArrayIterator`.
    loaders: Mutex<Option<Arc<dyn EnvVarLoaders>>>,
    original_loaders: Option<Arc<dyn EnvVarLoaders>>,
    loaded_vars: Mutex<Vec<PhpArray>>,
    globals: Arc<RwLock<Globals>>,
    constants: Registry,
}

impl std::fmt::Debug for EnvVarProcessor {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("EnvVarProcessor")
            .field("loaded_vars", &self.loaded_vars)
            .finish_non_exhaustive()
    }
}

fn lock<T>(mutex: &Mutex<T>) -> std::sync::MutexGuard<'_, T> {
    mutex.lock().unwrap_or_else(PoisonError::into_inner)
}

/// A string value of `bytes`.
fn bytes(text: impl Into<PhpString>) -> Value {
    Value::String(text.into())
}

fn runtime(message: String) -> Error {
    Error::runtime(message)
}

/// `false === $value`.
fn is_false(value: &Value) -> bool {
    matches!(value, Value::Bool(false))
}

/// `'' === $value`.
fn is_empty_string(value: &Value) -> bool {
    matches!(value, Value::String(s) if s.is_empty())
}

/// `'' !== ($value ?? '')`.
fn is_filled(value: &Value) -> bool {
    !value.is_null() && !is_empty_string(value)
}

fn is_scalar(value: &Value) -> bool {
    matches!(
        value,
        Value::Bool(_) | Value::Int(_) | Value::Float(_) | Value::String(_)
    )
}

/// `$array[$key] ?? null`, as set.
fn get_set(array: &PhpArray, key: &Key) -> Option<Value> {
    array.get(key).filter(|v| !v.is_null()).cloned()
}

/// `(string) $value` of a scalar or null.
fn string_of(value: &Value) -> PhpString {
    value.to_php_string().unwrap_or_default()
}

/// A value's string form in a message (`%s`).
fn text(value: &Value) -> String {
    String::from_utf8_lossy(string_of(value).as_bytes()).into_owned()
}

/// `$stringable` cast to a string when it is a `\Stringable` object.
fn cast_stringable(value: &Value) -> Option<Value> {
    match value {
        Value::Object(object) if object.is_stringable() => {
            Some(Value::String(object.to_php_string().unwrap_or_default()))
        }
        _ => None,
    }
}

fn filter(value: &Value, filter: i64) -> Result<Value> {
    filter_var(value, filter, &Value::Null).map_err(|e| runtime(e.to_string()))
}

/// The C-locale whitespace PCRE's `\s` matches without the `u` modifier.
const fn is_pcre_space(byte: u8) -> bool {
    matches!(byte, b' ' | b'\t' | b'\n' | b'\x0B' | b'\x0C' | b'\r')
}

/// `strcspn($subject, $chars)`.
fn strcspn(subject: &[u8], chars: &[u8]) -> usize {
    subject
        .iter()
        .position(|b| chars.contains(b))
        .unwrap_or(subject.len())
}

impl EnvVarProcessor {
    /// `new EnvVarProcessor($container, $loaders)`.
    pub fn new(
        container: Arc<dyn ContainerInterface>,
        loaders: Option<Arc<dyn EnvVarLoaders>>,
    ) -> Self {
        Self {
            container,
            loaders: Mutex::new(loaders.clone()),
            original_loaders: loaders,
            loaded_vars: Mutex::new(Vec::new()),
            globals: Arc::new(RwLock::new(Globals::from_process(true))),
            constants: Registry::with_core(),
        }
    }

    /// Reads `$_ENV`, `$_SERVER` and `getenv()` from `globals`.
    #[must_use]
    pub fn with_globals(mut self, globals: Arc<RwLock<Globals>>) -> Self {
        self.globals = globals;
        self
    }

    /// Reads the constants and enums of `const:` and `enum:` from
    /// `constants`.
    #[must_use]
    pub fn with_constants(mut self, constants: Registry) -> Self {
        self.constants = constants;
        self
    }

    /// `reset()`: forgets the loaded variables and rewinds the loaders.
    pub fn reset(&self) {
        lock(&self.loaded_vars).clear();
        lock(&self.loaders).clone_from(&self.original_loaders);
    }

    /// The `string` lookup of an env var without a nested prefix:
    /// `$_ENV`, `$_SERVER`, `getenv()`, the loaded and loader variables,
    /// then the `env(NAME)` parameter.
    fn lookup(&self, name: &str) -> Result<Value> {
        let key = Key::from(name);
        let mut env = {
            let globals = self.globals.read().unwrap_or_else(PoisonError::into_inner);
            get_set(&globals.env, &key)
                .or_else(|| {
                    if name.starts_with("HTTP_") {
                        None
                    } else {
                        get_set(&globals.server, &key)
                    }
                })
                .unwrap_or(Value::Null)
        };
        let missing = if is_empty_string(&env) {
            true
        } else if is_false(&env) {
            false
        } else {
            if env.is_null() {
                let globals = self.globals.read().unwrap_or_else(PoisonError::into_inner);
                env = globals
                    .getenv(name.as_bytes())
                    .map_or(Value::Bool(false), bytes);
            }
            is_false(&env)
        };
        if !missing {
            return Ok(env);
        }

        let loaded = lock(&self.loaded_vars).clone();
        for (index, vars) in loaded.iter().enumerate() {
            env = get_set(vars, &key).unwrap_or(env);
            if is_false(&env) {
                continue;
            }
            if let Some(string) = cast_stringable(&env) {
                env = string;
                if let Some(vars) = lock(&self.loaded_vars).get_mut(index) {
                    vars.insert(key.clone(), env.clone());
                }
            }
            if is_filled(&env) {
                break;
            }
        }

        if is_false(&env) || is_empty_string(&env) {
            let loaders = lock(&self.loaders).take();
            let mut after = loaders.clone();
            let outcome = self.load(loaders.as_deref(), &key, &mut env, &mut after);
            *lock(&self.loaders) = match &outcome {
                Ok(()) => after,
                Err(_) => loaders,
            };
            match outcome {
                Err(e) if e.is(&ErrorKind::ParameterCircularReference) => {
                    // skip loaders that need an env var that is not defined
                }
                Err(e) => return Err(e),
                Ok(()) => {}
            }
        }

        if is_false(&env) {
            let parameter = format!("env({name})");
            if !self.container.has_parameter(&parameter) {
                return Err(Error::env_not_found(format!(
                    "Environment variable not found: \"{name}\"."
                )));
            }
            env = self.container.get_parameter(&parameter)?;
        }
        Ok(env)
    }

    /// The loop over the loaders that have not been loaded yet.
    fn load(
        &self,
        loaders: Option<&dyn EnvVarLoaders>,
        key: &Key,
        env: &mut Value,
        after: &mut Option<Arc<dyn EnvVarLoaders>>,
    ) -> Result<()> {
        let mut i = 0;
        let mut ended = true;
        let count = loaders.and_then(EnvVarLoaders::count).unwrap_or(0);
        if let Some(loaders) = loaders {
            for loader in loaders.loaders() {
                let loader = loader?;
                let skip = lock(&self.loaded_vars).len() > i;
                i += 1;
                if skip {
                    continue;
                }
                let vars = loader.load_env_vars()?;
                lock(&self.loaded_vars).push(vars.clone());
                *env = get_set(&vars, key).unwrap_or_else(|| env.clone());
                if is_false(env) {
                    continue;
                }
                if let Some(string) = cast_stringable(env) {
                    *env = string;
                    if let Some(vars) = lock(&self.loaded_vars).last_mut() {
                        vars.insert(key.clone(), env.clone());
                    }
                }
                if is_filled(env) {
                    ended = false;
                    break;
                }
            }
        }
        if ended || count == i {
            after.clone_from(&lock(&self.loaders));
        }
        Ok(())
    }

    fn key(name: &str, i: Option<usize>, get_env: &dyn Fn(&str) -> Result<Value>) -> Result<Value> {
        let Some(i) = i else {
            return Err(runtime(format!(
                "Invalid env \"key:{name}\": a key specifier should be provided."
            )));
        };
        let (key, next) = (&name[..i], &name[i + 1..]);
        let Value::Array(array) = get_env(next)? else {
            return Err(runtime(format!(
                "Resolved value of \"{next}\" did not result in an array value."
            )));
        };
        if let Some(value) = array.get(&Key::from(key)) {
            return Ok(value.clone());
        }
        let json = json::encode(&Value::Array(array.clone()), 0, 512)
            .ok()
            .and_then(|encoded| encoded.json)
            .unwrap_or_default();
        Err(Error::env_not_found(format!(
            "Key \"{key}\" not found in {json} (resolved from \"{next}\")."
        )))
    }

    fn enumeration(
        &self,
        name: &str,
        i: Option<usize>,
        get_env: &dyn Fn(&str) -> Result<Value>,
    ) -> Result<Value> {
        let Some(i) = i else {
            return Err(runtime(format!(
                "Invalid env \"enum:{name}\": a \"BackedEnum\" class-string should be provided."
            )));
        };
        let (class, next) = (&name[..i], &name[i + 1..]);
        let value = get_env(next)?;
        if !matches!(value, Value::String(_) | Value::Int(_)) {
            return Err(runtime(format!(
                "Resolved value of \"{next}\" did not result in a string or int value."
            )));
        }
        let cases = self.constants.cases(class).unwrap_or_default();
        let not_backed = || runtime(format!("\"{class}\" is not a \"BackedEnum\"."));
        match backed_enum_from(class, cases, &value).ok_or_else(not_backed)? {
            Ok(case) => Ok(Value::Object(case)),
            Err(EnumFromError::ValueError(_)) => Err(runtime(format!(
                "Enum value \"{}\" is not backed by \"{class}\".",
                text(&value)
            ))),
            Err(EnumFromError::TypeError(message)) => Err(Error::new(
                ErrorKind::Other("TypeError".to_owned()),
                message.replacen("::from(", "::tryFrom(", 1),
            )),
        }
    }

    fn default(
        &self,
        name: &str,
        i: Option<usize>,
        get_env: &dyn Fn(&str) -> Result<Value>,
    ) -> Result<Value> {
        let Some(i) = i else {
            return Err(runtime(format!(
                "Invalid env \"default:{name}\": a fallback parameter should be provided."
            )));
        };
        let (default, next) = (&name[..i], &name[i + 1..]);
        if !default.is_empty() && !self.container.has_parameter(default) {
            return Err(runtime(format!(
                "Invalid env fallback in \"default:{name}\": parameter \"{default}\" not found."
            )));
        }
        match get_env(next) {
            Ok(env) if is_filled(&env) => return Ok(env),
            Ok(_) => {}
            Err(e) if e.is(&ErrorKind::EnvNotFound) => {}
            Err(e) => return Err(e),
        }
        if default.is_empty() {
            Ok(Value::Null)
        } else {
            self.container.get_parameter(default)
        }
    }

    fn file(prefix: &str, name: &str, get_env: &dyn Fn(&str) -> Result<Value>) -> Result<Value> {
        let file = get_env(name)?;
        if !is_scalar(&file) {
            return Err(runtime(format!(
                "Invalid file name: env var \"{name}\" is non-scalar."
            )));
        }
        let path = string_of(&file);
        let found = !path.is_empty()
            && php_value::path::to_path(path.as_bytes()).is_some_and(|p| p.is_file());
        if !found {
            return Err(Error::env_not_found(format!(
                "File \"{}\" not found (resolved from \"{name}\").",
                text(&file)
            )));
        }
        if prefix == "file" {
            // file_get_contents() is false (with a warning) when the read fails.
            return Ok(php_value::path::to_path(path.as_bytes())
                .and_then(|p| std::fs::read(p).ok())
                .map_or(Value::Bool(false), bytes));
        }
        Err(runtime(format!(
            "Cannot process env var \"require:{name}\": running a PHP file is not supported."
        )))
    }

    fn resolve(
        &self,
        name: &str,
        env: &[u8],
        get_env: &dyn Fn(&str) -> Result<Value>,
    ) -> Result<Value> {
        let mut out = Vec::with_capacity(env.len());
        let mut p = 0;
        while p < env.len() {
            if env[p] == b'%' {
                if env.get(p + 1) == Some(&b'%') {
                    out.push(b'%');
                    p += 2;
                    continue;
                }
                let run = env[p + 1..]
                    .iter()
                    .take_while(|&&b| b != b'%' && !is_pcre_space(b))
                    .count();
                if run > 0 && env.get(p + 1 + run) == Some(&b'%') {
                    let parameter = String::from_utf8_lossy(&env[p + 1..p + 1 + run]).into_owned();
                    let value = match parameter
                        .strip_prefix("env(")
                        .and_then(|r| r.strip_suffix(')'))
                    {
                        Some(inner) if parameter != "env()" => get_env(inner)?,
                        _ => self.container.get_parameter(&parameter)?,
                    };
                    if !is_scalar(&value) {
                        return Err(runtime(format!(
                            "Parameter \"{parameter}\" found when resolving env var \"{name}\" must be scalar, \"{}\" given.",
                            value.debug_type()
                        )));
                    }
                    out.extend_from_slice(string_of(&value).as_bytes());
                    p += run + 2;
                    continue;
                }
            }
            out.push(env[p]);
            p += 1;
        }
        Ok(bytes(out))
    }

    fn url(name: &str, env: &[u8]) -> Result<Value> {
        let Some(parsed) = url::parse_url(env) else {
            return Err(runtime(format!("Invalid URL in env var \"{name}\".")));
        };
        let Some(scheme) = &parsed.scheme else {
            return Err(runtime(format!(
                "Invalid URL in env var \"{name}\": scheme and host expected."
            )));
        };
        if parsed.host.is_none() {
            return Err(runtime(format!(
                "Invalid URL in env var \"{name}\": scheme and host expected."
            )));
        }
        if (!cfg!(windows) || scheme.as_slice() != b"file")
            && let Some(i) = env.iter().position(|&b| b == b'\\')
            && i < strcspn(env, b"?#")
        {
            return Err(runtime(format!(
                "Invalid URL in env var \"{name}\": backslashes are not allowed."
            )));
        }
        let first = env.first().copied().unwrap_or(0);
        let last = env.last().copied().unwrap_or(0);
        if first <= 32 || last <= 32 || env.len() != strcspn(env, b"\r\n\t") {
            return Err(runtime(format!(
                "Invalid URL in env var \"{name}\": leading/trailing ASCII control characters or whitespaces are not allowed."
            )));
        }
        let mut params = parsed.to_array();
        for key in ["port", "user", "pass", "path", "query", "fragment"] {
            if !params.contains_key(&Key::from(key)) {
                params.insert(key, Value::Null);
            }
        }
        for key in ["user", "pass"] {
            if let Some(Value::String(s)) = params.get(&Key::from(key)) {
                let decoded = url::rawurldecode(s.as_bytes());
                params.insert(key, bytes(decoded));
            }
        }
        // remove the '/' separator
        let path = match params.get(&Key::from("path")) {
            Some(Value::String(path)) if path.as_bytes() != b"/" => {
                bytes(path.as_bytes().get(1..).unwrap_or_default())
            }
            _ => Value::from(""),
        };
        params.insert("path", path);
        Ok(Value::Array(params))
    }

    fn query_string(env: &[u8]) -> Value {
        let parsed = url::parse_url(env);
        let query = parsed.as_ref().and_then(|u| u.query.clone());
        let query = match query {
            Some(query) if php_value::string::truthy(&query) => query,
            _ => {
                let scheme = parsed.and_then(|u| u.scheme);
                if scheme.is_some_and(|s| php_value::string::truthy(&s)) {
                    Vec::new()
                } else {
                    env.to_vec()
                }
            }
        };
        Value::Array(vars::parse_str(&query))
    }

    #[expect(clippy::too_many_lines, reason = "one method upstream")]
    fn process(
        &self,
        prefix: &str,
        name: &str,
        get_env: &dyn Fn(&str) -> Result<Value>,
    ) -> Result<Value> {
        let i = name.find(':');

        match prefix {
            "key" => return Self::key(name, i, get_env),
            "enum" => return self.enumeration(name, i, get_env),
            "defined" => {
                return match get_env(name) {
                    Ok(value) => Ok(Value::Bool(is_filled(&value))),
                    Err(e) if e.is(&ErrorKind::EnvNotFound) => Ok(Value::Bool(false)),
                    Err(e) => Err(e),
                };
            }
            "default" => return self.default(name, i, get_env),
            "file" | "require" => return Self::file(prefix, name, get_env),
            _ => {}
        }

        let mut prefix = prefix;
        let mut return_null = false;
        if prefix.is_empty() {
            if name.is_empty() {
                return Ok(Value::Null);
            }
            return_null = true;
            prefix = "string";
        }

        let env = if i.is_some() || prefix != "string" {
            get_env(name)?
        } else {
            self.lookup(name)?
        };

        if env.is_null() {
            if return_null {
                return Ok(Value::Null);
            }
            if !PROVIDED_TYPES.iter().any(|(p, _)| *p == prefix) {
                return Err(runtime(format!("Unsupported env var prefix \"{prefix}\".")));
            }
            if !["string", "bool", "not", "int", "float"].contains(&prefix) {
                return Ok(Value::Null);
            }
        }

        if prefix == "shuffle" {
            return match &env {
                Value::Array(array) => Ok(Value::Array(builtins::shuffle(array))),
                other => Err(runtime(format!(
                    "Env var \"{name}\" cannot be shuffled, expected array, got \"{}\".",
                    other.debug_type()
                ))),
            };
        }

        if !env.is_null() && !is_scalar(&env) {
            return Err(runtime(format!(
                "Non-scalar env var \"{name}\" cannot be cast to \"{prefix}\"."
            )));
        }

        match prefix {
            "string" => return Ok(Value::String(string_of(&env))),
            "bool" | "not" => {
                let truthy = filter(&env, FILTER_VALIDATE_BOOL)?.is_truthy()
                    || filter(&env, FILTER_VALIDATE_INT)?.is_truthy()
                    || builtins::validate_float(&env).is_some_and(|f| f != 0.0);
                return Ok(Value::Bool((prefix == "not") ^ truthy));
            }
            "int" => {
                if env.is_null() {
                    return Ok(Value::Int(0));
                }
                let int = filter(&env, FILTER_VALIDATE_INT)?;
                if let Value::Int(int) = int
                    && int != 0
                {
                    return Ok(Value::Int(int));
                }
                return match builtins::validate_float(&env) {
                    Some(float) => Ok(Value::Int(builtins::float_to_int(float))),
                    None => Err(runtime(format!(
                        "Non-numeric env var \"{name}\" cannot be cast to int."
                    ))),
                };
            }
            "float" => {
                if env.is_null() {
                    return Ok(Value::Float(0.0));
                }
                return match builtins::validate_float(&env) {
                    Some(float) => Ok(Value::Float(float)),
                    None => Err(runtime(format!(
                        "Non-numeric env var \"{name}\" cannot be cast to float."
                    ))),
                };
            }
            _ => {}
        }

        let string = string_of(&env);
        let subject = string.as_bytes();
        match prefix {
            "const" => {
                let constant = std::str::from_utf8(subject)
                    .ok()
                    .and_then(|name| self.constants.constant(name));
                match constant {
                    Some(value) => Ok(value.clone()),
                    None => Err(runtime(format!(
                        "Env var \"{name}\" maps to undefined constant \"{}\".",
                        text(&env)
                    ))),
                }
            }
            "base64" => {
                let translated: Vec<u8> = subject
                    .iter()
                    .map(|&b| match b {
                        b'-' => b'+',
                        b'_' => b'/',
                        other => other,
                    })
                    .collect();
                Ok(bytes(base64::decode(&translated)))
            }
            "json" => {
                let decoded = json::decode_assoc(subject, 512, false).map_err(|e| {
                    runtime(format!(
                        "Invalid JSON in env var \"{name}\": {}",
                        e.message()
                    ))
                })?;
                if !decoded.is_null() && !matches!(decoded, Value::Array(_)) {
                    return Err(runtime(format!(
                        "Invalid JSON env var \"{name}\": array or null expected, \"{}\" given.",
                        decoded.debug_type()
                    )));
                }
                Ok(decoded)
            }
            "url" => Self::url(name, subject),
            "query_string" => Ok(Self::query_string(subject)),
            "resolve" => self.resolve(name, subject, get_env),
            "csv" => Ok(Value::Array(if is_empty_string(&env) {
                PhpArray::new()
            } else {
                builtins::str_getcsv(subject)
            })),
            "trim" => Ok(Value::from(PhpString::from(php_value::string::trim(
                subject,
            )))),
            "urlencode" => Ok(Value::from(PhpString::from(url::rawurlencode(subject)))),
            _ => Err(runtime(format!(
                "Unsupported env var prefix \"{prefix}\" for env name \"{name}\"."
            ))),
        }
    }
}

impl EnvVarProcessorInterface for EnvVarProcessor {
    fn get_env(
        &self,
        prefix: &str,
        name: &str,
        get_env: &dyn Fn(&str) -> Result<Value>,
    ) -> Result<Value> {
        self.process(prefix, name, get_env)
    }

    fn get_provided_types() -> Vec<(String, String)> {
        PROVIDED_TYPES
            .iter()
            .map(|(prefix, types)| ((*prefix).to_owned(), (*types).to_owned()))
            .collect()
    }
}
