//! Port of Symfony\Component\DependencyInjection\Loader\IniFileLoader.php:
//! loads parameters from the `[parameters]` (and `[parameters@env]`)
//! sections of an INI file.
//!
//! PHP's `parse_ini_file($path, true[, INI_SCANNER_RAW])` is written here
//! from PHP's documentation, for what the loader reads: sections,
//! `key = value` lines, `key[]` and `key[name]` arrays, `;` comments,
//! quoted values, and the normal scanner's keywords (`true`, `on`,
//! `yes` as `"1"`; `false`, `off`, `no`, `none`, `null` as `""`) and
//! constants. A line it does not understand, a key holding one of
//! `{}|&~![()^"`, or an unterminated quote is a syntax error, as the
//! normal scanner reports one (`false`). Not supported: `${...}`
//! variables, string concatenation and escaped quotes (upstream's own
//! `phpize()` does not support the last two either).
//!
//! `defined()`/`constant()` read the loader's constants
//! ([`IniFileLoader::set_constants`]; PHP's core constants by default).
//!
//! Symfony source copyright (c) Fabien Potencier; distributed under MIT.
//!
//! Upstream: symfony/dependency-injection Loader/IniFileLoader.php

use std::sync::{Arc, PoisonError, RwLock, Weak};

use php_value::constants::Registry;
use php_value::{Key, PhpArray, Value};
use symfony_config::file_locator::FileLocatorInterface;
use symfony_config::loader::{
    FileLoader as BaseFileLoader, LoaderInterface, LoaderResolverInterface,
};
use symfony_config::{Class, Error as ConfigError, Located, XmlUtils};

use super::{FileLoader, SharedContainer, cfg, lock};
use crate::container_builder::TrackContents;
use crate::exception::Error;

pub const CLASS: &str = "Symfony\\Component\\DependencyInjection\\Loader\\IniFileLoader";

/// `IniFileLoader`.
pub struct IniFileLoader {
    file: FileLoader,
    constants: RwLock<Arc<Registry>>,
}

impl IniFileLoader {
    /// `new IniFileLoader($container, $locator, $env, $prepend)`.
    pub fn new(
        container: SharedContainer,
        locator: Arc<dyn FileLocatorInterface>,
        env: Option<&str>,
        prepend: bool,
    ) -> Arc<Self> {
        Arc::new_cyclic(|this: &Weak<Self>| {
            let this: Weak<dyn LoaderInterface> = this.clone();
            Self {
                file: FileLoader::new(container, locator, env, prepend, this),
                constants: RwLock::new(Arc::new(Registry::with_core())),
            }
        })
    }

    /// The constants `defined()` sees.
    pub fn set_constants(&self, constants: Arc<Registry>) {
        *self
            .constants
            .write()
            .unwrap_or_else(PoisonError::into_inner) = constants;
    }

    /// The embedded `FileLoader`.
    pub fn file_loader(&self) -> &FileLoader {
        &self.file
    }

    fn constants(&self) -> Arc<Registry> {
        self.constants
            .read()
            .unwrap_or_else(PoisonError::into_inner)
            .clone()
    }

    /// `phpize($value)`.
    fn phpize(&self, value: &Value) -> Result<Value, ConfigError> {
        let Value::String(value) = value else {
            return Err(ConfigError::new(
                Class::TypeError,
                format!(
                    "{CLASS}::phpize(): Argument #1 ($value) must be of type string, {} given",
                    symfony_config::php::error_type_name(value)
                ),
            ));
        };
        let mut value = value.as_bytes().to_vec();
        // trim on the right as comments removal keep whitespaces
        let v = rtrim(&value).to_vec();
        if v != value {
            value = if v.len() >= 2 && v[0] == b'"' && v[v.len() - 1] == b'"' {
                v[1..v.len() - 1].to_vec()
            } else {
                v
            };
        }
        let lowercase = value.to_ascii_lowercase();

        let constants = self.constants();
        if let Some(constant) = std::str::from_utf8(&value)
            .ok()
            .and_then(|name| constants.constant(name))
        {
            return Ok(constant.clone());
        }
        Ok(match lowercase.as_slice() {
            b"yes" | b"on" => Value::Bool(true),
            b"no" | b"off" | b"none" => Value::Bool(false),
            _ if value.len() > 1
                && ((value[0] == b'\'' && value[value.len() - 1] == b'\'')
                    || (value[0] == b'"' && value[value.len() - 1] == b'"')) =>
            {
                // quoted string
                Value::string(value[1..value.len() - 1].to_vec())
            }
            _ => XmlUtils::phpize(&value),
        })
    }

    fn set_parameter(&self, key: &Key, value: Value) -> Result<(), ConfigError> {
        let name = String::from_utf8_lossy(&symfony_config::php::key_bytes(key)).into_owned();
        lock(self.file.container())
            .set_parameter(&name, value)
            .map_err(cfg)
    }
}

impl LoaderInterface for IniFileLoader {
    fn class(&self) -> &str {
        CLASS
    }

    fn load(&self, resource: &Value, _type: Option<&[u8]>) -> Result<Value, ConfigError> {
        let name = symfony_config::php::string_value(resource)?;
        let path = match self.file.base().get_locator().locate(&name, None, true)? {
            Located::Path(p) => p,
            Located::Paths(mut ps) => ps.remove(0),
        };

        lock(self.file.container())
            .file_exists(&String::from_utf8_lossy(&path), &TrackContents::Yes)
            .map_err(cfg)?;

        let constants = self.constants();
        let content = php_value::path::to_path(&path).and_then(|p| std::fs::read(p).ok());

        // first pass to catch parsing errors
        let result = content
            .as_deref()
            .and_then(|c| parse_ini(c, false, &constants));
        if result.as_ref().is_none_or(PhpArray::is_empty) {
            return Err(cfg(Error::invalid_argument(format!(
                "The \"{}\" file is not valid.",
                String::from_utf8_lossy(&name)
            ))));
        }

        // real raw parsing
        let result = content
            .as_deref()
            .and_then(|c| parse_ini(c, true, &constants))
            .unwrap_or_default();

        if let Some(Value::Array(parameters)) = result.get(&Key::from("parameters")) {
            for (key, value) in parameters.iter() {
                if let Value::Array(values) = value {
                    let mut mapped = PhpArray::new();
                    for (k, v) in values.iter() {
                        mapped.insert(k.clone(), self.phpize(v)?);
                    }
                    self.set_parameter(key, Value::Array(mapped))?;
                } else {
                    self.set_parameter(key, self.phpize(value)?)?;
                }
            }
        }

        if let Some(env) = self.file.env().filter(|e| !e.is_empty() && e != "0")
            && let Some(Value::Array(parameters)) =
                result.get(&Key::from(format!("parameters@{env}").as_str()))
        {
            for (key, value) in parameters.iter() {
                self.set_parameter(key, self.phpize(value)?)?;
            }
        }

        Ok(Value::Null)
    }

    fn supports(&self, resource: &Value, r#type: Option<&[u8]>) -> bool {
        let Value::String(resource) = resource else {
            return false;
        };

        if r#type.is_none() && extension(resource.as_bytes()) == b"ini" {
            return true;
        }

        r#type == Some(b"ini")
    }

    fn get_resolver(&self) -> Result<Arc<dyn LoaderResolverInterface>, ConfigError> {
        self.file.base().loader().get_resolver()
    }

    fn set_resolver(&self, resolver: Arc<dyn LoaderResolverInterface>) {
        self.file.base().loader().set_resolver(resolver);
    }

    fn as_file_loader(&self) -> Option<&BaseFileLoader> {
        Some(self.file.base())
    }
}

/// `pathinfo($path, PATHINFO_EXTENSION)`.
pub(crate) fn extension(path: &[u8]) -> &[u8] {
    let base = php_value::path::basename(path);
    match base.iter().rposition(|&b| b == b'.') {
        Some(i) => &base[i + 1..],
        None => b"",
    }
}

fn rtrim(s: &[u8]) -> &[u8] {
    let end = s
        .iter()
        .rposition(|b| !b" \t\n\r\0\x0B".contains(b))
        .map_or(0, |i| i + 1);
    &s[..end]
}

fn trim(s: &[u8]) -> &[u8] {
    let s = rtrim(s);
    let start = s
        .iter()
        .position(|b| !b" \t\n\r\0\x0B".contains(b))
        .unwrap_or(s.len());
    &s[start..]
}

/// `parse_ini_string($content, true, $raw ? INI_SCANNER_RAW :
/// INI_SCANNER_NORMAL)`; `None` is PHP's `false` (a syntax error).
fn parse_ini(content: &[u8], raw: bool, constants: &Registry) -> Option<PhpArray> {
    let mut result = PhpArray::new();
    let mut section: Option<Key> = None;

    for line in content.split(|&b| b == b'\n') {
        let line = trim(line);
        if line.is_empty() || line[0] == b';' {
            continue;
        }
        if line[0] == b'[' {
            let close = line.iter().position(|&b| b == b']')?;
            let rest = trim(&line[close + 1..]);
            if !rest.is_empty() && rest[0] != b';' {
                return None;
            }
            let name = trim(&line[1..close]);
            let key = Key::from_bytes(name);
            if !matches!(result.get(&key), Some(Value::Array(_))) {
                result.insert(key.clone(), Value::Array(PhpArray::new()));
            }
            section = Some(key);
            continue;
        }

        let eq = line.iter().position(|&b| b == b'=')?;
        let raw_key = trim(&line[..eq]);
        if raw_key.is_empty() || raw_key.iter().any(|b| b"{}|&~!()^\"".contains(b)) {
            return None;
        }
        let value = parse_value(trim_start(&line[eq + 1..]), raw, constants)?;

        // `key[]` and `key[name]`
        let (name, offset) = match raw_key.iter().position(|&b| b == b'[') {
            Some(open) => {
                if raw_key.last() != Some(&b']') {
                    return None;
                }
                let offset = trim(&raw_key[open + 1..raw_key.len() - 1]);
                (trim(&raw_key[..open]), Some(offset))
            }
            None => (raw_key, None),
        };
        if name.is_empty() {
            return None;
        }

        let target = match &section {
            Some(key) => match result.get_mut(key) {
                Some(Value::Array(a)) => a,
                _ => return None,
            },
            None => &mut result,
        };
        let name = Key::from_bytes(name);
        match offset {
            None => target.insert(name, Value::String(value.into())),
            Some(offset) => {
                if !matches!(target.get(&name), Some(Value::Array(_))) {
                    target.insert(name.clone(), Value::Array(PhpArray::new()));
                }
                let Some(Value::Array(array)) = target.get_mut(&name) else {
                    return None;
                };
                if offset.is_empty() {
                    array.push(Value::String(value.into())).ok()?;
                } else {
                    array.insert(Key::from_bytes(offset), Value::String(value.into()));
                }
            }
        }
    }

    Some(result)
}

fn trim_start(s: &[u8]) -> &[u8] {
    let start = s
        .iter()
        .position(|b| !b" \t\r\0\x0B".contains(b))
        .unwrap_or(s.len());
    &s[start..]
}

/// A value after `=`, leading whitespace removed.
fn parse_value(value: &[u8], raw: bool, constants: &Registry) -> Option<Vec<u8>> {
    if raw {
        // the text up to a comment outside double quotes, kept as it is
        // (with the whitespace before the comment); a value that is
        // only a double-quoted string loses its quotes
        let mut in_quotes = false;
        let mut comment = None;
        for (i, &b) in value.iter().enumerate() {
            match b {
                b'"' => in_quotes = !in_quotes,
                b';' if !in_quotes => {
                    comment = Some(i);
                    break;
                }
                _ => {}
            }
        }
        if let Some(i) = comment {
            return Some(value[..i].to_vec());
        }
        let v = rtrim(value);
        return Some(if v.len() >= 2 && v[0] == b'"' && v[v.len() - 1] == b'"' {
            v[1..v.len() - 1].to_vec()
        } else {
            v.to_vec()
        });
    }

    if let Some(&quote @ (b'"' | b'\'')) = value.first() {
        let close = value[1..].iter().position(|&b| b == quote)? + 1;
        let rest = trim(&value[close + 1..]);
        if !rest.is_empty() && rest[0] != b';' {
            return None;
        }
        return Some(value[1..close].to_vec());
    }
    {
        {
            let end = value.iter().position(|&b| b == b';').unwrap_or(value.len());
            let v = trim(&value[..end]);
            if v.iter().any(|b| b"{}|&~![()^\"'".contains(b)) {
                return None;
            }
            Some(match v.to_ascii_lowercase().as_slice() {
                b"true" | b"on" | b"yes" => b"1".to_vec(),
                b"false" | b"off" | b"no" | b"none" | b"null" => Vec::new(),
                _ => match std::str::from_utf8(v)
                    .ok()
                    .and_then(|n| constants.constant(n))
                {
                    Some(constant) => symfony_config::php::to_bytes(constant),
                    None => v.to_vec(),
                },
            })
        }
    }
}
