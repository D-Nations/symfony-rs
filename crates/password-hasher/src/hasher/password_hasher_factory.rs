//! Port of Symfony\Component\PasswordHasher\Hasher\PasswordHasherFactory.php.
//!
//! A generic hasher factory implementation. The configuration is PHP's:
//! for each key (a class name or a hasher name), a hasher, or an array of
//! options (`algorithm`, `migrate_from`, `class` and `arguments`, `cost`,
//! ...) as `php-value` values, which may also hold an `instance` hasher
//! ([`HasherConfig`]).
//!
//! `new $config['class'](...$config['arguments'])` knows the five hasher
//! classes of this package (positional and named arguments, PHP's
//! coercion of scalar arguments); another class is `Class "X" not found`.
//! A `\TypeError` message stops before PHP's ", called in <file> on line
//! <n>". The environment is the oracle's: libsodium with Argon2id, so
//! `argon2id` is the sodium hasher, `argon2i` the native one, and `auto`
//! includes sodium. A configuration that would make PHP recurse without
//! end (e.g. `hash_algorithm: bcrypt`) is refused with an `\Error`
//! instead of exhausting the stack (README.md, "Differences").
//!
//! Symfony source copyright (c) Fabien Potencier; distributed under MIT.
//!
//! Upstream: symfony/password-hasher Hasher/PasswordHasherFactory.php

use std::sync::{Arc, Mutex, PoisonError};

use php_value::{Key, PhpArray, PhpString, Value, compare::loose_equals, numeric};

use crate::{
    MessageDigestPasswordHasher, MigratingPasswordHasher, NativePasswordHasher,
    PasswordHasherInterface, Pbkdf2PasswordHasher, PlaintextPasswordHasher, SodiumPasswordHasher,
    exception::{Class, Error},
    hasher::{
        password_hasher_aware_interface::PasswordHasherAwareInterface,
        password_hasher_factory_interface::{HasherSubject, PasswordHasherFactoryInterface},
    },
    php::password::{PASSWORD_ARGON2I, PASSWORD_BCRYPT},
    user::same_class,
};

const NATIVE: &str = "Symfony\\Component\\PasswordHasher\\Hasher\\NativePasswordHasher";
const SODIUM: &str = "Symfony\\Component\\PasswordHasher\\Hasher\\SodiumPasswordHasher";
const DIGEST: &str = "Symfony\\Component\\PasswordHasher\\Hasher\\MessageDigestPasswordHasher";
const PBKDF2: &str = "Symfony\\Component\\PasswordHasher\\Hasher\\Pbkdf2PasswordHasher";
const PLAINTEXT: &str = "Symfony\\Component\\PasswordHasher\\Hasher\\PlaintextPasswordHasher";
const MIGRATING: &str = "Symfony\\Component\\PasswordHasher\\Hasher\\MigratingPasswordHasher";

/// Deeper `createHasher()` calls than this are a configuration that
/// recurses without end.
const MAX_DEPTH: usize = 64;

/// One value of the factory's configuration array.
#[derive(Clone)]
pub enum HasherConfig {
    /// A `PasswordHasherInterface` instance.
    Hasher(Arc<dyn PasswordHasherInterface>),
    /// An array of options; `instance` is its `'instance'` entry.
    Options {
        instance: Option<Arc<dyn PasswordHasherInterface>>,
        options: PhpArray,
    },
}

impl HasherConfig {
    /// An array of options without `instance`.
    pub fn options(options: PhpArray) -> Self {
        Self::Options {
            instance: None,
            options,
        }
    }

    /// `['instance' => $hasher] + $options`.
    pub fn instance(instance: Arc<dyn PasswordHasherInterface>, options: PhpArray) -> Self {
        Self::Options {
            instance: Some(instance),
            options,
        }
    }

    /// `(bool) $config`.
    fn is_truthy(&self) -> bool {
        match self {
            Self::Hasher(_) => true,
            Self::Options { instance, options } => instance.is_some() || !options.is_empty(),
        }
    }
}

/// An options array with its `instance`.
#[derive(Clone)]
struct Config {
    instance: Option<Arc<dyn PasswordHasherInterface>>,
    options: PhpArray,
}

impl Config {
    fn from_options(options: PhpArray) -> Self {
        Self {
            instance: None,
            options,
        }
    }

    /// `$config[$key]` when set (`isset()`).
    fn get(&self, key: &str) -> Option<&Value> {
        get(&self.options, key)
    }

    fn set(&mut self, key: &str, value: Value) {
        self.options.insert(key, value);
    }
}

/// `$array[$key]` when set (`isset()`, `??`).
fn get<'a>(array: &'a PhpArray, key: &str) -> Option<&'a Value> {
    array
        .get(&Key::from_bytes(key.as_bytes()))
        .filter(|v| !v.is_null())
}

/// What `getHasherConfigFromAlgorithm()` and `getMigratingPasswordConfig()`
/// return: `['class' => ..., 'arguments' => ...]`, with the chain of a
/// `MigratingPasswordHasher` kept as hashers.
enum Resolved {
    Migrating(Vec<Arc<dyn PasswordHasherInterface>>),
    Array(PhpArray),
}

fn class_array(class: &str, arguments: PhpArray) -> Resolved {
    let mut array = PhpArray::new();
    array.insert("class", Value::string(class));
    array.insert("arguments", Value::Array(arguments));
    Resolved::Array(array)
}

/// The password hashers by user class or name, created on first use.
pub struct PasswordHasherFactory {
    password_hashers: Mutex<Vec<(Key, HasherConfig)>>,
}

impl PasswordHasherFactory {
    /// `$passwordHashers`: hashers or option arrays, by class or name, in
    /// order.
    pub fn new<K: AsRef<str>>(
        password_hashers: impl IntoIterator<Item = (K, HasherConfig)>,
    ) -> Self {
        Self {
            password_hashers: Mutex::new(
                password_hashers
                    .into_iter()
                    .map(|(k, v)| (Key::from_bytes(k.as_ref().as_bytes()), v))
                    .collect(),
            ),
        }
    }

    fn entry(&self, key: &Key) -> Option<HasherConfig> {
        let hashers = self
            .password_hashers
            .lock()
            .unwrap_or_else(PoisonError::into_inner);
        hashers
            .iter()
            .find(|(k, _)| k == key)
            .map(|(_, v)| v.clone())
    }

    /// Creates the actual hasher instance.
    fn create_hasher(
        &self,
        config: Config,
        is_extra: bool,
        depth: usize,
    ) -> Result<Arc<dyn PasswordHasherInterface>, Error> {
        if depth > MAX_DEPTH {
            return Err(Error::new(
                Class::Error,
                "The password hasher configuration recurses without end.",
            ));
        }
        let mut raw_config = None;
        let resolved = if let Some(instance) = &config.instance {
            if config.get("migrate_from").is_none() {
                return Ok(instance.clone());
            }
            self.get_migrating_password_config(config, depth)?
        } else if config.get("algorithm").is_some() {
            raw_config = Some(config.clone());
            self.get_hasher_config_from_algorithm(config, depth)?
        } else {
            Resolved::Array(config.options)
        };

        let (class, hasher) = match resolved {
            Resolved::Migrating(chain) => {
                let mut chain = chain.into_iter();
                let Some(best) = chain.next() else {
                    return Err(Error::new(
                        Class::ArgumentCountError,
                        format!(
                            "Too few arguments to function {MIGRATING}::__construct(), 0 passed and at least 1 expected"
                        ),
                    ));
                };
                let hasher: Arc<dyn PasswordHasherInterface> =
                    Arc::new(MigratingPasswordHasher::new(best, chain.collect()));
                (MIGRATING, hasher)
            }
            Resolved::Array(array) => {
                let Some(class) = get(&array, "class") else {
                    return Err(Error::invalid_argument(format!(
                        "\"class\" must be set in {}",
                        json_encode(&array)
                    )));
                };
                let Some(arguments) = get(&array, "arguments") else {
                    return Err(Error::invalid_argument(format!(
                        "\"arguments\" must be set in {}",
                        json_encode(&array)
                    )));
                };
                instantiate(class, arguments)?
            }
        };

        if is_extra || (class != NATIVE && class != SODIUM) {
            return Ok(hasher);
        }

        let extra_password_hashers = if let Some(raw_config) = raw_config {
            let hash_algorithm = raw_config
                .get("hash_algorithm")
                .cloned()
                .unwrap_or_else(|| Value::string("sha512"));
            let mut extras = Vec::new();
            for algo in [Value::string("pbkdf2"), hash_algorithm] {
                // array_map(function (string $algo) ...)
                let algo = coerce_string(&algo).ok_or_else(|| {
                    Error::new(
                        Class::TypeError,
                        format!(
                            "{{closure}}(): Argument #1 ($algo) must be of type string, {} given",
                            algo.value_name()
                        ),
                    )
                })?;
                let mut config = raw_config.clone();
                config.set("algorithm", Value::String(algo));
                extras.push(self.create_hasher(config, false, depth + 1)?);
            }
            extras
        } else {
            let pbkdf2: Arc<dyn PasswordHasherInterface> =
                Arc::new(Pbkdf2PasswordHasher::default());
            let digest: Arc<dyn PasswordHasherInterface> =
                Arc::new(MessageDigestPasswordHasher::default());
            vec![pbkdf2, digest]
        };

        Ok(Arc::new(MigratingPasswordHasher::new(
            hasher,
            extra_password_hashers,
        )))
    }

    fn get_hasher_config_from_algorithm(
        &self,
        mut config: Config,
        depth: usize,
    ) -> Result<Resolved, Error> {
        let algorithm = config.get("algorithm").cloned().unwrap_or_default();
        if algorithm.identical(&Value::string("auto")) {
            // "plaintext" is not listed as any leaked hashes could then be
            // used to authenticate directly
            let mut algorithms = vec![
                Value::string("native"),
                Value::string("sodium"),
                Value::string("pbkdf2"),
            ];
            if let Some(hash_algorithm) = config.get("hash_algorithm")
                && hash_algorithm.is_truthy()
            {
                algorithms.push(hash_algorithm.clone());
            }
            let mut hasher_chain = Vec::new();
            for algorithm in algorithms {
                config.set("algorithm", algorithm);
                hasher_chain.push(self.create_hasher(config.clone(), true, depth + 1)?);
            }
            return Ok(Resolved::Migrating(hasher_chain));
        }

        if config.get("migrate_from").is_some_and(Value::is_truthy) {
            return self.get_migrating_password_config(config, depth);
        }

        let is = |name: &str| loose_equals(&algorithm, &Value::string(name));
        let option = |key: &str, default: Value| config.get(key).cloned().unwrap_or(default);

        if is("plaintext") {
            return Ok(class_array(
                PLAINTEXT,
                PhpArray::list([option("ignore_case", Value::Bool(false))]),
            ));
        }
        if is("pbkdf2") {
            return Ok(class_array(
                PBKDF2,
                PhpArray::list([
                    option("hash_algorithm", Value::string("sha512")),
                    option("encode_as_base64", Value::Bool(true)),
                    option("iterations", Value::Int(1000)),
                    option("key_length", Value::Int(40)),
                ]),
            ));
        }
        if is("bcrypt") {
            config.set("algorithm", Value::string("native"));
            config.set("native_algorithm", Value::string(PASSWORD_BCRYPT));
            return self.get_hasher_config_from_algorithm(config, depth);
        }
        if is("native") {
            let mut arguments = PhpArray::list([
                option("time_cost", Value::Null),
                memory_cost(&config)?,
                option("cost", Value::Null),
            ]);
            if let Some(native_algorithm) = config.get("native_algorithm") {
                arguments.insert(3, native_algorithm.clone());
            }
            return Ok(class_array(NATIVE, arguments));
        }
        if is("sodium") {
            return Ok(class_array(
                SODIUM,
                PhpArray::list([option("time_cost", Value::Null), memory_cost(&config)?]),
            ));
        }
        if is("argon2i") {
            // Sodium has Argon2id, so Argon2i comes from password_hash().
            config.set("algorithm", Value::string("native"));
            config.set("native_algorithm", Value::string(PASSWORD_ARGON2I));
            return self.get_hasher_config_from_algorithm(config, depth);
        }
        if is("argon2id") {
            config.set("algorithm", Value::string("sodium"));
            return self.get_hasher_config_from_algorithm(config, depth);
        }

        Ok(class_array(
            DIGEST,
            PhpArray::list([
                algorithm,
                option("encode_as_base64", Value::Bool(true)),
                option("iterations", Value::Int(5000)),
            ]),
        ))
    }

    fn get_migrating_password_config(
        &self,
        mut config: Config,
        depth: usize,
    ) -> Result<Resolved, Error> {
        let from_password_hashers = config
            .options
            .remove(&Key::from_bytes(b"migrate_from"))
            .unwrap_or_default();
        let mut hasher_chain = vec![self.create_hasher(config, true, depth + 1)?];
        // foreach over a value that is not an array iterates nothing.
        if let Value::Array(names) = from_password_hashers {
            for name in names.values() {
                let entry = Key::from_value(name)
                    .and_then(|key| self.entry(&key))
                    .filter(HasherConfig::is_truthy);
                let hasher = match entry {
                    Some(HasherConfig::Hasher(hasher)) => hasher,
                    Some(HasherConfig::Options { instance, options }) => {
                        self.create_hasher(Config { instance, options }, true, depth + 1)?
                    }
                    None => {
                        let mut options = PhpArray::new();
                        options.insert("algorithm", name.clone());
                        self.create_hasher(Config::from_options(options), true, depth + 1)?
                    }
                };
                hasher_chain.push(hasher);
            }
        }
        Ok(Resolved::Migrating(hasher_chain))
    }
}

impl PasswordHasherFactoryInterface for PasswordHasherFactory {
    fn get_password_hasher(
        &self,
        user: HasherSubject<'_>,
    ) -> Result<Arc<dyn PasswordHasherInterface>, Error> {
        let mut hasher_key = None;
        let named = match user {
            HasherSubject::Object(object) => object
                .as_password_hasher_aware()
                .and_then(PasswordHasherAwareInterface::get_password_hasher_name),
            HasherSubject::Class(_) => None,
        };
        let entries: Vec<Key> = {
            let hashers = self
                .password_hashers
                .lock()
                .unwrap_or_else(PoisonError::into_inner);
            hashers.iter().map(|(k, _)| k.clone()).collect()
        };
        if let Some(hasher_name) = named {
            let key = Key::from_bytes(hasher_name.as_bytes());
            if !entries.contains(&key) {
                return Err(Error::runtime(format!(
                    "The password hasher \"{hasher_name}\" was not configured."
                )));
            }
            hasher_key = Some(key);
        } else {
            for class in entries {
                let class_name = String::from_utf8_lossy(&class.bytes()).into_owned();
                let matches = match user {
                    HasherSubject::Object(object) => object.php_class().instance_of(&class_name),
                    HasherSubject::Class(name) => {
                        name.is_subclass_of(&class_name)
                            || loose_equals(&Value::string(name.name), &class.to_value())
                    }
                };
                if matches {
                    hasher_key = Some(class);
                    break;
                }
            }
        }

        let Some(hasher_key) = hasher_key else {
            let account = match user {
                HasherSubject::Object(object) => {
                    php_value::value::debug_class_name(object.php_class().name).to_owned()
                }
                HasherSubject::Class(name) => name.name.to_owned(),
            };
            return Err(Error::runtime(format!(
                "No password hasher has been configured for account \"{account}\"."
            )));
        };

        let config = match self.entry(&hasher_key) {
            Some(HasherConfig::Hasher(hasher)) => return Ok(hasher),
            Some(HasherConfig::Options { instance, options }) => Config { instance, options },
            None => Config::from_options(PhpArray::new()),
        };
        let hasher = self.create_hasher(config, false, 0)?;
        let mut hashers = self
            .password_hashers
            .lock()
            .unwrap_or_else(PoisonError::into_inner);
        if let Some((_, entry)) = hashers.iter_mut().find(|(k, _)| *k == hasher_key) {
            *entry = HasherConfig::Hasher(hasher.clone());
        }
        Ok(hasher)
    }
}

/// `json_encode($config)`, or `''` (`false` as a string) when it fails.
fn json_encode(array: &PhpArray) -> String {
    php_value::json::encode(&Value::Array(array.clone()), 0, 512)
        .ok()
        .and_then(|e| e.json)
        .unwrap_or_default()
}

/// `(($config['memory_cost'] ?? 0) << 10) ?: null`.
fn memory_cost(config: &Config) -> Result<Value, Error> {
    let value = config.get("memory_cost").cloned().unwrap_or(Value::Int(0));
    let unsupported = || {
        Error::new(
            Class::TypeError,
            format!("Unsupported operand types: {} << int", value.debug_type()),
        )
    };
    let int = match &value {
        Value::Null => 0,
        Value::Bool(b) => i64::from(*b),
        Value::Int(i) => *i,
        Value::Float(f) => float_to_int(*f),
        Value::String(s) => match numeric::leading_numeric_value(s.as_bytes()) {
            Some(n) => number_to_int(n),
            None => return Err(unsupported()),
        },
        Value::Array(_) | Value::Object(_) => return Err(unsupported()),
    };
    let shifted = int.wrapping_shl(10);
    Ok(if shifted == 0 {
        Value::Null
    } else {
        Value::Int(shifted)
    })
}

/// PHP's float to int conversion: truncation, 0 for what does not fit.
#[expect(
    clippy::cast_possible_truncation,
    reason = "PHP's float to int conversion truncates"
)]
fn float_to_int(f: f64) -> i64 {
    const LIMIT: f64 = 9_223_372_036_854_775_808.0;
    if f.is_finite() && (-LIMIT..LIMIT).contains(&f) {
        f.trunc() as i64
    } else {
        0
    }
}

fn number_to_int(n: php_value::Number) -> i64 {
    match n {
        php_value::Number::Int(i) => i,
        php_value::Number::Float(f) => float_to_int(f),
    }
}

/// A `string` parameter in coercive mode: scalars convert; `None` for
/// `null`, arrays and objects.
fn coerce_string(value: &Value) -> Option<PhpString> {
    match value {
        Value::Bool(_) | Value::Int(_) | Value::Float(_) | Value::String(_) => {
            value.to_php_string()
        }
        Value::Null | Value::Array(_) | Value::Object(_) => None,
    }
}

/// A scalar parameter type of the hashers' constructors.
#[derive(Clone, Copy)]
enum Type {
    Int,
    String,
    Bool,
}

/// One constructor parameter: its name, type, and whether it takes `null`.
struct Param {
    name: &'static str,
    ty: Type,
    nullable: bool,
}

const fn param(name: &'static str, ty: Type, nullable: bool) -> Param {
    Param { name, ty, nullable }
}

/// An argument after PHP's coercion.
#[derive(Clone)]
enum Arg {
    Int(Option<i64>),
    String(Option<String>),
    Bool(bool),
}

impl Arg {
    fn int(&self) -> Option<i64> {
        match self {
            Self::Int(i) => *i,
            _ => None,
        }
    }

    fn string(&self) -> Option<String> {
        match self {
            Self::String(s) => s.clone(),
            _ => None,
        }
    }

    fn bool(&self) -> bool {
        matches!(self, Self::Bool(true))
    }
}

/// Coerces `value` for `param` (argument `position`, from 1) of
/// `class::__construct()`.
fn coerce(class: &str, position: usize, param: &Param, value: &Value) -> Result<Arg, Error> {
    let type_error = || {
        let ty = match param.ty {
            Type::Int => "int",
            Type::String => "string",
            Type::Bool => "bool",
        };
        Error::new(
            Class::TypeError,
            format!(
                "{class}::__construct(): Argument #{position} (${}) must be of type {}{ty}, {} given",
                param.name,
                if param.nullable { "?" } else { "" },
                value.value_name()
            ),
        )
    };
    if value.is_null() {
        if !param.nullable {
            return Err(type_error());
        }
        return Ok(match param.ty {
            Type::Int => Arg::Int(None),
            Type::String => Arg::String(None),
            Type::Bool => Arg::Bool(false),
        });
    }
    match param.ty {
        Type::Int => {
            let int = match value {
                Value::Int(i) => Some(*i),
                Value::Bool(b) => Some(i64::from(*b)),
                Value::Float(f) if f.is_finite() && f.abs() < 9.223_372_036_854_776e18 => {
                    Some(float_to_int(*f))
                }
                Value::String(s) => numeric::int_argument(s.as_bytes()),
                _ => None,
            };
            int.map(|i| Arg::Int(Some(i))).ok_or_else(type_error)
        }
        Type::String => coerce_string(value)
            .map(|s| Arg::String(Some(String::from_utf8_lossy(s.as_bytes()).into_owned())))
            .ok_or_else(type_error),
        Type::Bool => match value {
            Value::Bool(_) | Value::Int(_) | Value::Float(_) | Value::String(_) => {
                Ok(Arg::Bool(value.is_truthy()))
            }
            _ => Err(type_error()),
        },
    }
}

/// `new $class(...$arguments)` for the hasher classes of this package.
#[expect(
    clippy::too_many_lines,
    reason = "one table of the five constructors, as PHP's classes declare them"
)]
fn instantiate(
    class: &Value,
    arguments: &Value,
) -> Result<(&'static str, Arc<dyn PasswordHasherInterface>), Error> {
    let Value::String(name) = class else {
        return Err(Error::new(
            Class::Error,
            "Class name must be a valid object or a string",
        ));
    };
    let name = String::from_utf8_lossy(name.as_bytes()).into_owned();
    let Value::Array(arguments) = arguments else {
        return Err(Error::new(
            Class::Error,
            "Only arrays and Traversables can be unpacked",
        ));
    };
    let class = [NATIVE, SODIUM, DIGEST, PBKDF2, PLAINTEXT, MIGRATING]
        .into_iter()
        .find(|c| same_class(c, &name))
        .ok_or_else(|| Error::new(Class::Error, format!("Class \"{name}\" not found")))?;

    let (params, defaults): (Vec<Param>, Vec<Arg>) = match class {
        NATIVE => (
            vec![
                param("opsLimit", Type::Int, true),
                param("memLimit", Type::Int, true),
                param("cost", Type::Int, true),
                param("algorithm", Type::String, true),
            ],
            vec![
                Arg::Int(None),
                Arg::Int(None),
                Arg::Int(None),
                Arg::String(None),
            ],
        ),
        SODIUM => (
            vec![
                param("opsLimit", Type::Int, true),
                param("memLimit", Type::Int, true),
            ],
            vec![Arg::Int(None), Arg::Int(None)],
        ),
        DIGEST => (
            vec![
                param("algorithm", Type::String, false),
                param("encodeHashAsBase64", Type::Bool, false),
                param("iterations", Type::Int, false),
            ],
            vec![
                Arg::String(Some("sha512".to_owned())),
                Arg::Bool(true),
                Arg::Int(Some(5000)),
            ],
        ),
        PBKDF2 => (
            vec![
                param("algorithm", Type::String, false),
                param("encodeHashAsBase64", Type::Bool, false),
                param("iterations", Type::Int, false),
                param("length", Type::Int, false),
            ],
            vec![
                Arg::String(Some("sha512".to_owned())),
                Arg::Bool(true),
                Arg::Int(Some(1000)),
                Arg::Int(Some(40)),
            ],
        ),
        PLAINTEXT => (
            vec![param("ignorePasswordCase", Type::Bool, false)],
            vec![Arg::Bool(false)],
        ),
        _ => {
            // MigratingPasswordHasher takes hashers, which an options
            // array cannot hold.
            let given = arguments
                .values()
                .next()
                .map_or_else(|| "none".to_owned(), Value::value_name);
            return Err(Error::new(
                Class::TypeError,
                format!(
                    "{MIGRATING}::__construct(): Argument #1 ($bestHasher) must be of type Symfony\\Component\\PasswordHasher\\PasswordHasherInterface, {given} given"
                ),
            ));
        }
    };

    let mut args = defaults;
    let mut set = vec![false; params.len()];
    let mut positional = 0;
    let mut named = false;
    for (key, value) in arguments.iter() {
        let index = match key {
            Key::Int(_) => {
                if named {
                    return Err(Error::new(
                        Class::Error,
                        "Cannot use positional argument after named argument during unpacking",
                    ));
                }
                positional += 1;
                positional - 1
            }
            Key::Str(s) => {
                named = true;
                let s = String::from_utf8_lossy(s.as_bytes()).into_owned();
                let Some(index) = params.iter().position(|p| p.name == s) else {
                    return Err(Error::new(
                        Class::Error,
                        format!("Unknown named parameter ${s}"),
                    ));
                };
                if set[index] {
                    return Err(Error::new(
                        Class::Error,
                        format!("Named parameter ${s} overwrites previous argument"),
                    ));
                }
                index
            }
        };
        // Extra positional arguments are ignored, as PHP does for a
        // function without variadics.
        if let Some(param) = params.get(index) {
            args[index] = coerce(class, index + 1, param, value)?;
            set[index] = true;
        }
    }

    let hasher: Arc<dyn PasswordHasherInterface> = match class {
        NATIVE => Arc::new(NativePasswordHasher::new(
            args[0].int(),
            args[1].int(),
            args[2].int(),
            args[3].string().as_deref(),
        )?),
        SODIUM => Arc::new(SodiumPasswordHasher::new(args[0].int(), args[1].int())?),
        DIGEST => Arc::new(MessageDigestPasswordHasher::new(
            args[0].string().unwrap_or_default(),
            args[1].bool(),
            args[2].int().unwrap_or_default(),
        )),
        PBKDF2 => Arc::new(Pbkdf2PasswordHasher::new(
            args[0].string().unwrap_or_default(),
            args[1].bool(),
            args[2].int().unwrap_or_default(),
            args[3].int().unwrap_or_default(),
        )?),
        _ => Arc::new(PlaintextPasswordHasher::new(args[0].bool())),
    };
    Ok((class, hasher))
}
