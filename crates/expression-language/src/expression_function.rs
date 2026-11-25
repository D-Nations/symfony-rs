//! Port of Symfony\Component\ExpressionLanguage\ExpressionFunction.php and
//! ExpressionFunctionProviderInterface.php.
//!
//! A function is two closures: the compiler gets the compiled arguments
//! and returns PHP source; the evaluator gets the variables and the
//! argument values. `fromPhp()` wraps a PHP function: PHP looks it up by
//! name at run time; here the name is looked up in a [`PhpFunctions`]
//! table, which holds Rust implementations of PHP functions
//! (`constant()`, `min()`, `max()`, `strtoupper()`, `strtolower()`, and
//! what the application defines).
//!
//! Symfony source copyright (c) Fabien Potencier; distributed under MIT.
//!
//! Upstream: symfony/expression-language ExpressionFunction.php ExpressionFunctionProviderInterface.php

use std::{collections::HashMap, fmt, sync::Arc};

use php_value::{PhpArray, PhpString, Value, constants::Registry};

use crate::{
    php,
    syntax_error::{Class, Error},
};

/// A function's compiler: the compiled arguments, the PHP source.
pub type CompilerFn = Arc<dyn Fn(&[PhpString]) -> Result<PhpString, Error> + Send + Sync>;

/// A function's evaluator: the variables and the arguments, the value.
pub type EvaluatorFn = Arc<dyn Fn(&PhpArray, &[Value]) -> Result<Value, Error> + Send + Sync>;

/// A PHP function: its arguments, its value.
pub type PhpFn = Arc<dyn Fn(&[Value]) -> Result<Value, Error> + Send + Sync>;

/// A function that can be used in an expression.
#[derive(Clone)]
pub struct ExpressionFunction {
    name: PhpString,
    compiler: CompilerFn,
    evaluator: EvaluatorFn,
}

impl fmt::Debug for ExpressionFunction {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("ExpressionFunction")
            .field("name", &self.name)
            .finish_non_exhaustive()
    }
}

impl ExpressionFunction {
    pub fn new(
        name: impl Into<PhpString>,
        compiler: impl Fn(&[PhpString]) -> Result<PhpString, Error> + Send + Sync + 'static,
        evaluator: impl Fn(&PhpArray, &[Value]) -> Result<Value, Error> + Send + Sync + 'static,
    ) -> Self {
        Self {
            name: name.into(),
            compiler: Arc::new(compiler),
            evaluator: Arc::new(evaluator),
        }
    }

    /// `getName()`.
    pub fn name(&self) -> &PhpString {
        &self.name
    }

    /// `getCompiler()`.
    pub fn compiler(&self) -> &CompilerFn {
        &self.compiler
    }

    /// `getEvaluator()`.
    pub fn evaluator(&self) -> &EvaluatorFn {
        &self.evaluator
    }

    /// Calls the compiler.
    pub fn compile(&self, arguments: &[PhpString]) -> Result<PhpString, Error> {
        (self.compiler)(arguments)
    }

    /// Calls the evaluator.
    pub fn evaluate(&self, values: &PhpArray, arguments: &[Value]) -> Result<Value, Error> {
        (self.evaluator)(values, arguments)
    }

    /// `ExpressionFunction::fromPhp($phpFunctionName,
    /// $expressionFunctionName)`, with PHP's functions in `php`.
    pub fn from_php(
        php_function_name: &str,
        expression_function_name: Option<&str>,
        php: &PhpFunctions,
    ) -> Result<Self, Error> {
        let php_function_name = php_function_name.trim_start_matches('\\').to_owned();
        let Some(function) = php.get(&php_function_name) else {
            return Err(Error::new(
                Class::InvalidArgumentException,
                format!("PHP function \"{php_function_name}\" does not exist."),
            ));
        };
        let parts: Vec<&str> = php_function_name.split('\\').collect();
        let expression_function_name =
            expression_function_name.filter(|name| !name.is_empty() && *name != "0");
        if expression_function_name.is_none() && parts.len() > 1 {
            return Err(Error::new(
                Class::InvalidArgumentException,
                format!(
                    "An expression function name must be defined when PHP function \"{php_function_name}\" is namespaced."
                ),
            ));
        }
        let name = expression_function_name
            .or_else(|| parts.last().copied())
            .unwrap_or_default()
            .to_owned();
        let compiled_name = php_function_name.clone();
        Ok(Self {
            name: PhpString::from(name),
            compiler: Arc::new(move |arguments: &[PhpString]| {
                let mut out = format!("\\{compiled_name}(").into_bytes();
                out.extend_from_slice(
                    &arguments
                        .iter()
                        .map(PhpString::as_bytes)
                        .collect::<Vec<_>>()
                        .join(b", ".as_slice()),
                );
                out.push(b')');
                Ok(PhpString::from(out))
            }),
            evaluator: Arc::new(move |_values: &PhpArray, arguments: &[Value]| function(arguments)),
        })
    }
}

/// `ExpressionFunctionProviderInterface`.
pub trait ExpressionFunctionProvider {
    /// `getFunctions()`.
    fn functions(&self) -> Result<Vec<ExpressionFunction>, Error>;
}

/// The registered functions, by name, in registration order (PHP's
/// `$functions` array).
#[derive(Debug, Clone, Default)]
pub struct Functions {
    list: Vec<ExpressionFunction>,
    index: HashMap<Vec<u8>, usize>,
}

impl Functions {
    pub fn new() -> Self {
        Self::default()
    }

    /// `$functions[$name] = $function`: a new name goes last, a known one
    /// keeps its place.
    pub fn insert(&mut self, function: ExpressionFunction) {
        let key = function.name.as_bytes().to_vec();
        if let Some(&i) = self.index.get(&key) {
            self.list[i] = function;
        } else {
            self.index.insert(key, self.list.len());
            self.list.push(function);
        }
    }

    /// `$functions[$name]`.
    pub fn get(&self, name: &[u8]) -> Option<&ExpressionFunction> {
        self.index.get(name).map(|&i| &self.list[i])
    }

    /// `isset($functions[$name])`.
    pub fn contains(&self, name: &[u8]) -> bool {
        self.index.contains_key(name)
    }

    /// `array_keys($functions)`.
    pub fn names(&self) -> Vec<PhpString> {
        self.list.iter().map(|f| f.name.clone()).collect()
    }

    pub fn is_empty(&self) -> bool {
        self.list.is_empty()
    }

    pub fn len(&self) -> usize {
        self.list.len()
    }
}

/// PHP functions by name (case-insensitive, as PHP's), for
/// [`ExpressionFunction::from_php`].
#[derive(Clone, Default)]
pub struct PhpFunctions {
    functions: HashMap<String, PhpFn>,
}

impl fmt::Debug for PhpFunctions {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let mut names: Vec<&String> = self.functions.keys().collect();
        names.sort();
        f.debug_struct("PhpFunctions")
            .field("functions", &names)
            .finish()
    }
}

impl PhpFunctions {
    /// No functions.
    pub fn empty() -> Self {
        Self::default()
    }

    /// The PHP functions this crate implements: `constant()` (reading
    /// `constants`), `min()`, `max()`, `strtoupper()` and `strtolower()`.
    pub fn builtin(constants: Arc<Registry>) -> Self {
        let mut php = Self::default();
        php.define("constant", move |arguments: &[Value]| {
            let [name] = arguments else {
                return Err(Error::new(
                    Class::ArgumentCountError,
                    format!(
                        "constant() expects exactly 1 argument, {} given",
                        arguments.len()
                    ),
                ));
            };
            let name = php::string_argument("constant", 1, "name", name)?;
            constant(&constants, name.as_bytes())
        });
        php.define("min", |arguments: &[Value]| php::min_max("min", arguments));
        php.define("max", |arguments: &[Value]| php::min_max("max", arguments));
        php.define("strtoupper", |arguments: &[Value]| {
            php::change_case("strtoupper", arguments)
        });
        php.define("strtolower", |arguments: &[Value]| {
            php::change_case("strtolower", arguments)
        });
        php
    }

    /// Defines a function (a user function, `function name() {}`).
    pub fn define(
        &mut self,
        name: &str,
        f: impl Fn(&[Value]) -> Result<Value, Error> + Send + Sync + 'static,
    ) {
        self.functions.insert(
            name.trim_start_matches('\\').to_ascii_lowercase(),
            Arc::new(f),
        );
    }

    /// `function_exists($name)`, and the function.
    pub fn get(&self, name: &str) -> Option<PhpFn> {
        self.functions
            .get(&name.trim_start_matches('\\').to_ascii_lowercase())
            .cloned()
    }
}

/// `constant($name)` on `constants`.
pub(crate) fn constant(constants: &Registry, name: &[u8]) -> Result<Value, Error> {
    let name = name.strip_prefix(b"\\").unwrap_or(name);
    let text = String::from_utf8_lossy(name);
    if let Some(value) = constants.constant(&text) {
        return Ok(value.clone());
    }
    if let Some(at) = text.find("::") {
        return Err(Error::new(
            Class::Error,
            format!("Class \"{}\" not found", &text[..at]),
        ));
    }
    Err(Error::new(
        Class::Error,
        format!("Undefined constant \"{text}\""),
    ))
}
