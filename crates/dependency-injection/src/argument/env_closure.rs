//! Port of Symfony\Component\DependencyInjection\Argument\EnvClosure.php:
//! the closure a compiled container passes for an `EnvClosureArgument`.
//!
//! `__toString()` returns what the closure gave, coerced to `string` as
//! PHP coerces a return value (the file has no `strict_types`): scalars
//! and `\Stringable` objects; anything else is PHP's `\TypeError`.
//!
//! Upstream: symfony/dependency-injection Argument/EnvClosure.php

use std::sync::Arc;

use php_value::{PhpArray, PhpString, Value, value::PhpObject};

use crate::exception::{Error, ErrorKind, Result};

type Closure = dyn Fn() -> Result<Value> + Send + Sync;

/// `EnvClosure`: reads an env var when invoked, with a default for a
/// missing one.
#[derive(Clone)]
pub struct EnvClosure {
    closure: Arc<Closure>,
    default: Value,
}

impl std::fmt::Debug for EnvClosure {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("EnvClosure")
            .field("default", &self.default)
            .finish_non_exhaustive()
    }
}

impl EnvClosure {
    pub const CLASS: &'static str = "Symfony\\Component\\DependencyInjection\\Argument\\EnvClosure";

    /// `new EnvClosure($closure, $default)`.
    pub fn new(
        closure: impl Fn() -> Result<Value> + Send + Sync + 'static,
        default: Value,
    ) -> Self {
        Self {
            closure: Arc::new(closure),
            default,
        }
    }

    /// `__invoke()`: the closure's value; when it throws an
    /// `EnvNotFoundException`, the default unless that is null.
    pub fn invoke(&self) -> Result<Value> {
        match (self.closure)() {
            Err(e) if e.is(&ErrorKind::EnvNotFound) && !self.default.is_null() => {
                Ok(self.default.clone())
            }
            other => other,
        }
    }

    /// `__toString()`.
    pub fn to_string(&self) -> Result<PhpString> {
        let value = self.invoke()?;
        value
            .to_php_string_or_stringable()
            .filter(|_| !value.is_null())
            .ok_or_else(|| {
                Error::new(
                    ErrorKind::Other("TypeError".to_owned()),
                    format!(
                        "{}::__toString(): Return value must be of type string, {} returned",
                        Self::CLASS,
                        value.value_name()
                    ),
                )
            })
    }
}

impl PhpObject for EnvClosure {
    fn class(&self) -> &str {
        Self::CLASS
    }

    fn properties(&self) -> PhpArray {
        PhpArray::new()
    }

    /// `(string) $closure`; `None` when `__toString()` throws.
    fn to_php_string(&self) -> Option<PhpString> {
        EnvClosure::to_string(self).ok()
    }

    fn is_stringable(&self) -> bool {
        true
    }
}
