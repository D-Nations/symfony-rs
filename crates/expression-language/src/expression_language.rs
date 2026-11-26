//! Port of Symfony\Component\ExpressionLanguage\ExpressionLanguage.php.
//!
//! Parsed expressions are cached in a [`CacheItemPool`] (PSR-6's
//! `getItem()`/`save()`); the default, [`ArrayCache`], keeps copies, as
//! symfony/cache's `ArrayAdapter` keeps serialized values: each `parse()`
//! gets a fresh tree. `constant()` and `enum()` read a constants
//! [`Registry`] (php-value), which the application fills.
//!
//! Symfony source copyright (c) Fabien Potencier; distributed under MIT.
//!
//! Upstream: symfony/expression-language ExpressionLanguage.php

use std::{
    collections::HashMap,
    fmt,
    sync::{Arc, Mutex, OnceLock},
};

use php_value::{
    PhpArray, PhpString, Value, constants::Registry, object::EnumCase, url::rawurlencode,
};

use crate::{
    compiler::Compiler,
    expression::ExpressionInput,
    expression_function::{
        ExpressionFunction, ExpressionFunctionProvider, Functions, PhpFunctions, constant,
    },
    lexer::Lexer,
    parsed_expression::ParsedExpression,
    parser::Parser,
    php,
    syntax_error::{Class, Error},
};

/// A PSR-6 cache pool for parsed expressions.
pub trait CacheItemPool: Send + Sync {
    /// `getItem($key)->get()`: the cached value, if any.
    fn get_item(&self, key: &[u8]) -> Option<Arc<ParsedExpression>>;

    /// `$item->set($value)` and `save($item)`.
    fn save(&self, key: &[u8], value: Arc<ParsedExpression>) -> bool;
}

/// An in-memory pool that returns a copy of what was saved.
#[derive(Debug, Default)]
pub struct ArrayCache {
    items: Mutex<HashMap<Vec<u8>, ParsedExpression>>,
}

impl ArrayCache {
    pub fn new() -> Self {
        Self::default()
    }
}

impl CacheItemPool for ArrayCache {
    fn get_item(&self, key: &[u8]) -> Option<Arc<ParsedExpression>> {
        let items = self
            .items
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        items.get(key).map(|p| Arc::new(p.clone()))
    }

    fn save(&self, key: &[u8], value: Arc<ParsedExpression>) -> bool {
        let mut items = self
            .items
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        items.insert(key.to_vec(), (*value).clone());
        true
    }
}

/// Compiles and evaluates expressions.
pub struct ExpressionLanguage {
    cache: Arc<dyn CacheItemPool>,
    functions: Functions,
    constants: Arc<Registry>,
    php_functions: PhpFunctions,
    parser: OnceLock<Parser>,
    compiler_functions: OnceLock<Functions>,
}

impl fmt::Debug for ExpressionLanguage {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("ExpressionLanguage")
            .field("functions", &self.functions)
            .finish_non_exhaustive()
    }
}

impl Default for ExpressionLanguage {
    fn default() -> Self {
        Self::new()
    }
}

/// The source `enum()` compiles to, around its argument.
const ENUM_COMPILED: (&str, &str) = (
    "(static fn ($v) => \\constant($v) instanceof \\UnitEnum ? \\constant($v) : throw new \\TypeError(\\sprintf('The string \"%s\" is not the name of a valid enum case.', $v)))(",
    ")",
);

impl ExpressionLanguage {
    /// `new ExpressionLanguage()`, with PHP's core constants
    /// ([`Registry::with_core`]).
    pub fn new() -> Self {
        Self::with_constants(None, Registry::with_core())
    }

    /// `new ExpressionLanguage($cache, $providers)`.
    pub fn with(
        cache: Option<Arc<dyn CacheItemPool>>,
        providers: &[&dyn ExpressionFunctionProvider],
    ) -> Result<Self, Error> {
        let mut language = Self::with_constants(cache, Registry::with_core());
        for provider in providers {
            language.register_provider(*provider)?;
        }
        Ok(language)
    }

    /// An instance whose `constant()` and `enum()` read `constants`.
    pub fn with_constants(cache: Option<Arc<dyn CacheItemPool>>, constants: Registry) -> Self {
        let constants = Arc::new(constants);
        let mut language = Self {
            cache: cache.unwrap_or_else(|| Arc::new(ArrayCache::new())),
            functions: Functions::new(),
            php_functions: PhpFunctions::builtin(Arc::clone(&constants)),
            constants,
            parser: OnceLock::new(),
            compiler_functions: OnceLock::new(),
        };
        language.register_functions();
        language
    }

    /// The PHP functions `ExpressionFunction::fromPhp()` finds for this
    /// instance (`constant()` reads its constants).
    pub fn php_functions(&self) -> &PhpFunctions {
        &self.php_functions
    }

    /// The registered functions.
    pub fn functions(&self) -> &Functions {
        &self.functions
    }

    /// `compile($expression, $names)`.
    pub fn compile(
        &self,
        expression: impl Into<ExpressionInput>,
        names: &PhpArray,
    ) -> Result<PhpString, Error> {
        let parsed = self.parse(expression, names, 0)?;
        let functions = self
            .compiler_functions
            .get_or_init(|| self.functions.clone())
            .clone();
        let mut compiler = Compiler::new(functions);
        compiler.compile(parsed.nodes())?;
        Ok(compiler.source())
    }

    /// `evaluate($expression, $values)`.
    pub fn evaluate(
        &self,
        expression: impl Into<ExpressionInput>,
        values: &PhpArray,
    ) -> Result<Value, Error> {
        let names = PhpArray::list(values.keys().map(php_value::Key::to_value));
        self.parse(expression, &names, 0)?
            .nodes()
            .evaluate(&self.functions, values)
    }

    /// `parse($expression, $names, $flags)`.
    pub fn parse(
        &self,
        expression: impl Into<ExpressionInput>,
        names: &PhpArray,
        flags: u32,
    ) -> Result<Arc<ParsedExpression>, Error> {
        let source = match expression.into() {
            ExpressionInput::Parsed(parsed) => return Ok(parsed),
            ExpressionInput::Source(source) => source,
        };

        let mut sorted: Vec<(php_value::Key, Value)> =
            names.iter().map(|(k, v)| (k.clone(), v.clone())).collect();
        // asort(): by value, stable
        let mut failure = None;
        sorted.sort_by(|(_, a), (_, b)| match php::compare(a, b) {
            Ok(c) => c.cmp(&0),
            Err(e) => {
                failure.get_or_insert(e);
                std::cmp::Ordering::Equal
            }
        });
        if let Some(e) = failure {
            return Err(e);
        }
        let mut items = Vec::new();
        let mut sorted_names = PhpArray::new();
        for (key, name) in &sorted {
            let name_text = php::to_string(name)?;
            items.push(match key {
                php_value::Key::Int(_) => name_text.as_bytes().to_vec(),
                php_value::Key::Str(k) => [k.as_bytes(), b":", name_text.as_bytes()].concat(),
            });
            sorted_names.insert(key.clone(), name.clone());
        }
        let mut key = source.as_bytes().to_vec();
        key.extend_from_slice(b"//");
        key.extend_from_slice(&items.join(b"|".as_slice()));
        if flags != 0 {
            key.extend_from_slice(format!("//{flags}").as_bytes());
        }
        let key = rawurlencode(&key);

        if let Some(parsed) = self.cache.get_item(&key) {
            return Ok(parsed);
        }
        let stream = Lexer::new().tokenize(source.as_bytes())?;
        let nodes = self.parser().parse(stream, &sorted_names, flags)?;
        let parsed = Arc::new(ParsedExpression::new(source, nodes));
        self.cache.save(&key, Arc::clone(&parsed));
        Ok(parsed)
    }

    /// `lint($expression, $names, $flags)`.
    pub fn lint(
        &self,
        expression: impl Into<ExpressionInput>,
        names: &PhpArray,
        flags: u32,
    ) -> Result<(), Error> {
        let source = match expression.into() {
            ExpressionInput::Parsed(_) => return Ok(()),
            ExpressionInput::Source(source) => source,
        };
        let stream = Lexer::new().tokenize(source.as_bytes())?;
        self.parser().lint(stream, names, flags)
    }

    fn parser(&self) -> &Parser {
        self.parser
            .get_or_init(|| Parser::new(self.functions.clone()))
    }

    /// `register($name, $compiler, $evaluator)`.
    pub fn register(
        &mut self,
        name: impl Into<PhpString>,
        compiler: impl Fn(&[PhpString]) -> Result<PhpString, Error> + Send + Sync + 'static,
        evaluator: impl Fn(&PhpArray, &[Value]) -> Result<Value, Error> + Send + Sync + 'static,
    ) -> Result<(), Error> {
        self.add_function(ExpressionFunction::new(name, compiler, evaluator))
    }

    /// `addFunction($function)`.
    pub fn add_function(&mut self, function: ExpressionFunction) -> Result<(), Error> {
        if self.parser.get().is_some() {
            return Err(Error::new(
                Class::LogicException,
                "Registering functions after calling evaluate(), compile() or parse() is not supported.",
            ));
        }
        self.functions.insert(function);
        Ok(())
    }

    /// `registerProvider($provider)`.
    pub fn register_provider(
        &mut self,
        provider: &dyn ExpressionFunctionProvider,
    ) -> Result<(), Error> {
        for function in provider.functions()? {
            self.add_function(function)?;
        }
        Ok(())
    }

    /// `registerFunctions()`: `constant()`, `min()`, `max()` and `enum()`.
    fn register_functions(&mut self) {
        for name in ["constant", "min", "max"] {
            if let Ok(function) = ExpressionFunction::from_php(name, None, &self.php_functions) {
                self.functions.insert(function);
            }
        }
        let constants = Arc::clone(&self.constants);
        self.functions.insert(ExpressionFunction::new(
            "enum",
            |arguments: &[PhpString]| {
                let Some(str) = arguments.first() else {
                    return Err(Error::new(
                        Class::ArgumentCountError,
                        "Too few arguments to function Symfony\\Component\\ExpressionLanguage\\ExpressionLanguage::{closure:Symfony\\Component\\ExpressionLanguage\\ExpressionLanguage::registerFunctions():149}(), 0 passed and exactly 1 expected",
                    ));
                };
                let mut out = ENUM_COMPILED.0.as_bytes().to_vec();
                out.extend_from_slice(str.as_bytes());
                out.extend_from_slice(ENUM_COMPILED.1.as_bytes());
                Ok(PhpString::from(out))
            },
            move |_values: &PhpArray, arguments: &[Value]| {
                let Some(str) = arguments.first() else {
                    return Err(Error::new(
                        Class::ArgumentCountError,
                        "Too few arguments to function Symfony\\Component\\ExpressionLanguage\\ExpressionLanguage::{closure:Symfony\\Component\\ExpressionLanguage\\ExpressionLanguage::registerFunctions():150}(), 1 passed and exactly 2 expected",
                    ));
                };
                let name = php::string_argument("constant", 1, "name", str)?;
                let value = constant(&constants, name.as_bytes())?;
                if let Value::Object(o) = &value
                    && o.downcast_ref::<EnumCase>().is_some()
                {
                    return Ok(value);
                }
                let mut message = b"The string \"".to_vec();
                message.extend_from_slice(name.as_bytes());
                message.extend_from_slice(b"\" is not the name of a valid enum case.");
                Err(Error::new(Class::TypeError, message))
            },
        ));
    }
}
