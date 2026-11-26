//! Port of Symfony\Component\DependencyInjection\Compiler\CheckArgumentsValidityPass.php.
//!
//! Checks that the positional arguments of constructors and method calls
//! are a list and come before the named ones.
//!
//! Upstream: symfony/dependency-injection Compiler/CheckArgumentsValidityPass.php

use php_value::{PhpArray, Value, array::Key};

use super::abstract_recursive_pass::{
    AbstractRecursivePass, RecursivePassState, recursive_process,
};
use super::compiler_pass_interface::CompilerPassInterface;
use crate::container_builder::ContainerBuilder;
use crate::definition::{Definition, DefinitionError};
use crate::exception::{Error, Result};

/// `CheckArgumentsValidityPass`.
#[derive(Debug)]
pub struct CheckArgumentsValidityPass {
    state: RecursivePassState,
    throw_exceptions: bool,
}

impl Default for CheckArgumentsValidityPass {
    fn default() -> Self {
        Self::new(true)
    }
}

/// `preg_match('/^[a-zA-Z_\x7f-\xff][a-zA-Z0-9_\x7f-\xff]*$/', $k)`, on
/// the key as a string.
fn is_name(key: &Key) -> bool {
    let bytes = key.bytes();
    let bytes = bytes.strip_suffix(b"\n").unwrap_or(&bytes);
    let Some((first, rest)) = bytes.split_first() else {
        return false;
    };
    let word = |b: u8| b == b'_' || b >= 0x7f || b.is_ascii_alphabetic();
    word(*first) && rest.iter().all(|b| word(*b) || b.is_ascii_digit())
}

/// What a key of an argument list is wrong about.
enum Problem {
    /// `integer expected but found string "%s"`.
    StringKey(String),
    /// `argument %d must be defined before`: the 1-based number and the
    /// expected index.
    Gap(i64, i64),
    /// `cannot use positional argument after named argument`.
    PositionalAfterNamed,
}

/// The checks of one argument list, in upstream's order; `report` is
/// called for each problem and fails when the pass throws.
fn check_arguments(
    arguments: &PhpArray,
    mut report: impl FnMut(Problem) -> Result<()>,
) -> Result<()> {
    let mut i: i64 = 0;
    let mut has_named_args = false;
    for k in arguments.keys() {
        if is_name(k) {
            has_named_args = true;
            continue;
        }

        let expected = i;
        i += 1;
        if k.as_int() != Some(expected) {
            let Some(k) = k.as_int() else {
                report(Problem::StringKey(
                    String::from_utf8_lossy(&k.bytes()).into_owned(),
                ))?;
                break;
            };
            report(Problem::Gap(1 + k, i))?;
        }

        if has_named_args {
            report(Problem::PositionalAfterNamed)?;
            break;
        }
    }
    Ok(())
}

impl CheckArgumentsValidityPass {
    pub const CLASS: &'static str =
        "Symfony\\Component\\DependencyInjection\\Compiler\\CheckArgumentsValidityPass";

    /// `new CheckArgumentsValidityPass($throwExceptions)`.
    pub fn new(throw_exceptions: bool) -> Self {
        let mut state = RecursivePassState::default();
        state.skip_scalars = true;
        Self {
            state,
            throw_exceptions,
        }
    }

    fn report(&self, definition: &Definition, msg: String) -> Result<()> {
        definition.add_error(DefinitionError::Text(msg.clone()));
        if self.throw_exceptions {
            return Err(Error::runtime(msg));
        }
        Ok(())
    }
}

impl CompilerPassInterface for CheckArgumentsValidityPass {
    fn class(&self) -> &str {
        Self::CLASS
    }

    fn process(&mut self, container: &mut ContainerBuilder) -> Result<()> {
        recursive_process(self, container)
    }
}

impl AbstractRecursivePass for CheckArgumentsValidityPass {
    fn state(&mut self) -> &mut RecursivePassState {
        &mut self.state
    }

    /// Returns null for a definition, as upstream does (the root array
    /// it lands in is dropped, and nested definitions are not walked).
    fn process_value(
        &mut self,
        container: &mut ContainerBuilder,
        value: Value,
        is_root: bool,
    ) -> Result<Value> {
        let Some(definition) = Definition::from_value(&value) else {
            return self.parent_process_value(container, value, is_root);
        };
        let current_id = self.state.current_id.clone().unwrap_or_default();

        check_arguments(&definition.arguments(), |problem| {
            let msg = match problem {
                Problem::StringKey(k) => format!(
                    "Invalid constructor argument for service \"{current_id}\": integer expected but found string \"{k}\". Check your service definition."
                ),
                Problem::Gap(number, i) => format!(
                    "Invalid constructor argument {number} for service \"{current_id}\": argument {i} must be defined before. Check your service definition."
                ),
                Problem::PositionalAfterNamed => format!(
                    "Invalid constructor argument for service \"{current_id}\": cannot use positional argument after named argument. Check your service definition."
                ),
            };
            self.report(&definition, msg)
        })?;

        for method_call in definition.method_calls().values() {
            let Some(method_call) = method_call.as_array() else {
                continue;
            };
            let method = method_call
                .get(&Key::Int(0))
                .and_then(Value::to_php_string)
                .map(|s| s.to_string_lossy().into_owned())
                .unwrap_or_default();
            let arguments = method_call
                .get(&Key::Int(1))
                .and_then(Value::as_array)
                .cloned()
                .unwrap_or_default();
            check_arguments(&arguments, |problem| {
                let msg = match problem {
                    Problem::StringKey(k) => format!(
                        "Invalid argument for method call \"{method}\" of service \"{current_id}\": integer expected but found string \"{k}\". Check your service definition."
                    ),
                    Problem::Gap(number, i) => format!(
                        "Invalid argument {number} for method call \"{method}\" of service \"{current_id}\": argument {i} must be defined before. Check your service definition."
                    ),
                    Problem::PositionalAfterNamed => format!(
                        "Invalid argument for method call \"{method}\" of service \"{current_id}\": cannot use positional argument after named argument. Check your service definition."
                    ),
                };
                self.report(&definition, msg)
            })?;
        }

        Ok(Value::Null)
    }
}
