//! Port of Symfony\Component\Routing\Route.php.
//!
//! A route is a value: PHP shares a Route object between every holder of
//! its handle, while here a [`RouteCollection`](crate::route_collection)
//! owns its routes and hands out references. Defaults, requirements and
//! options are PHP arrays, so their keys follow PHP's rules (`"0"` is the
//! integer key 0) and defaults hold any PHP value, closures included.
//! Schemes and methods are PHP arrays too, as `__unserialize()` restores
//! them; the setters build lists, as `(array)` of a string or a list does.
//!
//! The setters return `Result`, because upstream's throw: an empty
//! requirement, a `_locale` default that `preg_quote()` cannot take, and
//! inline settings in a pattern (`{name<requirement>?default}`), which
//! are read with upstream's own regular expression through php-pcre.
//!
//! `compile()` resolves the `compiler_class` option. PHP looks the class
//! up at run time; here [`Route::compile`] knows `RouteCompiler` and
//! [`Route::compile_with`] names another compiler type, so the class is
//! chosen when the program is built (AGENTS.md). Any other class is PHP's
//! `Class "%s" not found`.
//!
//! Symfony source copyright (c) Fabien Potencier; distributed under MIT.
//!
//! Upstream: symfony/routing Route.php

use std::sync::{Arc, LazyLock};

use php_pcre::Pcre;
use php_value::{
    Key, PhpArray, PhpString, Value,
    constants::Registry,
    object::GenericObject,
    serialize::{Classes, serialize, unserialize},
};

use crate::{
    compiled_route::{self, CompiledRoute},
    exception::{Class, Error, concat},
    php,
    route_compiler::{RouteCompiler, RouteCompilerInterface},
};

pub const CLASS: &str = "Symfony\\Component\\Routing\\Route";

/// The arguments of `new Route()` after the path, with upstream's
/// defaults: `$defaults`, `$requirements`, `$options`, `$host`, `$schemes`,
/// `$methods` and `$condition` (a null host or condition is `''`).
#[derive(Debug, Clone, Default)]
pub struct Arguments {
    pub defaults: PhpArray,
    pub requirements: PhpArray,
    pub options: PhpArray,
    pub host: PhpString,
    pub schemes: Vec<PhpString>,
    pub methods: Vec<PhpString>,
    pub condition: PhpString,
}

/// `Route`: a path pattern with its defaults, requirements, options,
/// host pattern, schemes, methods and condition.
#[derive(Debug, Clone, PartialEq)]
pub struct Route {
    path: PhpString,
    host: PhpString,
    schemes: PhpArray,
    methods: PhpArray,
    defaults: PhpArray,
    requirements: PhpArray,
    options: PhpArray,
    condition: PhpString,
    compiled: Option<CompiledRoute>,
}

impl Default for Route {
    /// The properties' initial values, before the constructor runs.
    fn default() -> Self {
        Self {
            path: PhpString::from("/"),
            host: PhpString::default(),
            schemes: PhpArray::new(),
            methods: PhpArray::new(),
            defaults: PhpArray::new(),
            requirements: PhpArray::new(),
            options: PhpArray::new(),
            condition: PhpString::default(),
            compiled: None,
        }
    }
}

fn lower(s: &[u8]) -> PhpString {
    PhpString::from(s.to_ascii_lowercase())
}

fn upper(s: &[u8]) -> PhpString {
    PhpString::from(s.to_ascii_uppercase())
}

fn list(items: impl IntoIterator<Item = PhpString>) -> PhpArray {
    PhpArray::list(items.into_iter().map(Value::String))
}

/// Upstream's pattern for inline settings, as written in Route.php.
static INLINE: LazyLock<Result<Pcre, php_pcre::PcreError>> = LazyLock::new(|| {
    Pcre::new(
        br#"/
            \{
                (!?)
                ([\w\x80-\xFF]++)      # The variable name (e.g. "id" or "slug")
                (
                :                      # Match the colon for mapping
                ([\w\x80-\xFF]++)      # The controller argument mapped to
                (\.[\w\x80-\xFF]++)?   # Optional property path
            )?
            (<.*?>)?                   # The requirement
            (\?[^\}]*+)?               # The default value
            \}
            /x"#,
    )
});

pub(crate) fn pcre(
    pattern: &'static LazyLock<Result<Pcre, php_pcre::PcreError>>,
) -> Result<&'static Pcre, Error> {
    pattern
        .as_ref()
        .map_err(|e| Error::new(Class::NotPorted, e.to_string()))
}

#[expect(clippy::needless_pass_by_value, reason = "a map_err() callback")]
pub(crate) fn match_error(e: php_pcre::MatchError) -> Error {
    Error::new(Class::NotPorted, format!("preg_* returned false: {e}"))
}

impl Route {
    /// `new Route($path)`.
    pub fn new(path: impl Into<PhpString>) -> Result<Self, Error> {
        Self::with(path, Arguments::default())
    }

    /// `new Route($path, $defaults, $requirements, $options, $host,
    /// $schemes, $methods, $condition)`.
    pub fn with(path: impl Into<PhpString>, arguments: Arguments) -> Result<Self, Error> {
        let mut route = Self::default();
        route.set_path(path)?;
        route.add_defaults(&arguments.defaults)?;
        route.add_requirements(&arguments.requirements)?;
        route.set_options(&arguments.options);
        route.set_host(arguments.host)?;
        route.set_schemes(arguments.schemes);
        route.set_methods(arguments.methods);
        route.set_condition(arguments.condition);
        Ok(route)
    }

    /// `__serialize()`.
    pub fn serialize_data(&self) -> PhpArray {
        let string = |s: &PhpString| Value::String(s.clone());
        let array = |a: &PhpArray| Value::Array(a.clone());
        [
            ("path", string(&self.path)),
            ("host", string(&self.host)),
            ("defaults", array(&self.defaults)),
            ("requirements", array(&self.requirements)),
            ("options", array(&self.options)),
            ("schemes", array(&self.schemes)),
            ("methods", array(&self.methods)),
            ("condition", string(&self.condition)),
            (
                "compiled",
                self.compiled
                    .as_ref()
                    .map_or(Value::Null, CompiledRoute::to_value),
            ),
        ]
        .into_iter()
        .collect()
    }

    /// The object `serialize()` writes.
    pub fn to_value(&self) -> Value {
        Value::Object(Arc::new(GenericObject::new(CLASS, self.serialize_data())))
    }

    /// `serialize($route)`.
    pub fn serialize(&self) -> Result<PhpString, Error> {
        let value = self.to_value();
        php::check_serializable(&Value::Array(self.serialize_data()))?;
        Ok(serialize(&value))
    }

    /// `__unserialize($data)`. A compiled route is a `CompiledRoute` object
    /// or one of `compiled_subclasses`.
    pub fn from_serialized(data: &PhpArray, compiled_subclasses: &[&str]) -> Result<Self, Error> {
        if php::is_stringable(data, "path")
            || php::is_stringable(data, "host")
            || php::is_stringable(data, "condition")
        {
            return Err(Error::new(
                Class::PhpBadMethodCall,
                format!("Cannot unserialize {CLASS}"),
            ));
        }
        let string = |key| php::assign_string(data, key, CLASS, key, false);
        let array = |key| php::assign_array(data, key, CLASS, key);
        let mut route = Self {
            path: string("path")?.unwrap_or_default(),
            host: string("host")?.unwrap_or_default(),
            defaults: array("defaults")?,
            requirements: array("requirements")?,
            options: array("options")?,
            schemes: array("schemes")?,
            methods: array("methods")?,
            ..Self::default()
        };
        if php::isset(data, b"condition") {
            route.condition = string("condition")?.unwrap_or_default();
        }
        if let Some(value) = php::get(data, b"compiled") {
            route.compiled = Some(
                CompiledRoute::from_value(value, compiled_subclasses).unwrap_or_else(|| {
                    Err(php::object_type_error(
                        value,
                        CLASS,
                        "compiled",
                        "?Symfony\\Component\\Routing\\CompiledRoute",
                    ))
                })?,
            );
        }
        Ok(route)
    }

    /// The classes `unserialize()` must allow for a route: Route and
    /// `CompiledRoute`. Add a compiled route's subclass, or the class of an
    /// object a default holds, before unserializing.
    pub fn classes() -> Classes {
        let mut classes = Classes::new();
        classes
            .allow_with_defaults(CLASS, PhpArray::new())
            .allow_with_defaults(compiled_route::CLASS, PhpArray::new());
        classes
    }

    /// `unserialize($data)` of a serialized route: `Ok(None)` when PHP's
    /// `unserialize()` returns `false` or something other than a Route.
    pub fn unserialize(
        data: &[u8],
        classes: &Classes,
        compiled_subclasses: &[&str],
    ) -> Result<Option<Self>, Error> {
        let Some(Value::Object(object)) = unserialize(data, classes, &Registry::empty()) else {
            return Ok(None);
        };
        if !object.class().eq_ignore_ascii_case(CLASS) {
            return Ok(None);
        }
        Self::from_serialized(&object.properties(), compiled_subclasses).map(Some)
    }

    /// `getPath()`.
    pub fn path(&self) -> &PhpString {
        &self.path
    }

    /// `setPath()`.
    pub fn set_path(&mut self, pattern: impl Into<PhpString>) -> Result<&mut Self, Error> {
        let pattern = self.extract_inline_defaults_and_requirements(pattern.into())?;
        // A pattern must start with a slash and must not have multiple slashes at the beginning because the
        // generated path for this route would be confused with a network path, e.g. '//domain.com/path'.
        self.path = PhpString::from(concat(&[
            b"/",
            php::trim_char(php::trim(&pattern), b'/', false),
        ]));
        self.compiled = None;
        Ok(self)
    }

    /// `getHost()`.
    pub fn host(&self) -> &PhpString {
        &self.host
    }

    /// `setHost()`; a null pattern is `''`.
    pub fn set_host(&mut self, pattern: impl Into<PhpString>) -> Result<&mut Self, Error> {
        self.host = self.extract_inline_defaults_and_requirements(pattern.into())?;
        self.compiled = None;
        Ok(self)
    }

    /// `getSchemes()`: the lowercased schemes this route is restricted to.
    /// So an empty array means that any scheme is allowed.
    pub fn schemes(&self) -> &PhpArray {
        &self.schemes
    }

    /// `setSchemes()` with a scheme or a list of schemes.
    pub fn set_schemes<S: Into<PhpString>>(
        &mut self,
        schemes: impl IntoIterator<Item = S>,
    ) -> &mut Self {
        self.schemes = list(schemes.into_iter().map(|s| lower(&s.into())));
        self.compiled = None;
        self
    }

    /// `hasScheme()`.
    pub fn has_scheme(&self, scheme: &[u8]) -> bool {
        let scheme = Value::String(lower(scheme));
        self.schemes.values().any(|s| *s == scheme)
    }

    /// `getMethods()`: the uppercased HTTP methods this route is restricted
    /// to. So an empty array means that any method is allowed.
    pub fn methods(&self) -> &PhpArray {
        &self.methods
    }

    /// `setMethods()` with a method or a list of methods.
    pub fn set_methods<S: Into<PhpString>>(
        &mut self,
        methods: impl IntoIterator<Item = S>,
    ) -> &mut Self {
        self.methods = list(methods.into_iter().map(|s| upper(&s.into())));
        self.compiled = None;
        self
    }

    /// `getOptions()`.
    pub fn options(&self) -> &PhpArray {
        &self.options
    }

    /// `setOptions()`.
    pub fn set_options(&mut self, options: &PhpArray) -> &mut Self {
        self.options = PhpArray::new();
        self.options
            .insert("compiler_class", Value::string(RouteCompiler::CLASS));
        self.add_options(options)
    }

    /// `addOptions()`.
    pub fn add_options(&mut self, options: &PhpArray) -> &mut Self {
        for (name, option) in options.iter() {
            self.options.insert(name.clone(), option.clone());
        }
        self.compiled = None;
        self
    }

    /// `setOption()`.
    pub fn set_option(&mut self, name: &[u8], value: Value) -> &mut Self {
        self.options.insert(Key::from_bytes(name), value);
        self.compiled = None;
        self
    }

    /// `getOption()`: the option value, or `None` when not found (or null).
    pub fn option(&self, name: &[u8]) -> Option<&Value> {
        php::get(&self.options, name)
    }

    /// `hasOption()`.
    pub fn has_option(&self, name: &[u8]) -> bool {
        self.options.contains_key(&Key::from_bytes(name))
    }

    /// `getDefaults()`.
    pub fn defaults(&self) -> &PhpArray {
        &self.defaults
    }

    /// `setDefaults()`.
    pub fn set_defaults(&mut self, defaults: &PhpArray) -> Result<&mut Self, Error> {
        self.defaults = PhpArray::new();
        self.add_defaults(defaults)
    }

    /// `addDefaults()`.
    pub fn add_defaults(&mut self, defaults: &PhpArray) -> Result<&mut Self, Error> {
        let mut defaults = defaults.clone();
        if php::isset(&defaults, b"_locale") && self.is_localized()? {
            defaults.remove(&Key::from("_locale"));
        }
        for (name, default) in defaults.iter() {
            self.defaults.insert(name.clone(), default.clone());
        }
        self.compiled = None;
        Ok(self)
    }

    /// `getDefault()`: the default value, or `None` when not found (or
    /// null).
    pub fn default_value(&self, name: &[u8]) -> Option<&Value> {
        php::get(&self.defaults, name)
    }

    /// `hasDefault()`.
    pub fn has_default(&self, name: &[u8]) -> bool {
        self.defaults.contains_key(&Key::from_bytes(name))
    }

    /// `setDefault()`.
    pub fn set_default(&mut self, name: &[u8], default: Value) -> Result<&mut Self, Error> {
        if name == b"_locale" && self.is_localized()? {
            return Ok(self);
        }
        self.defaults.insert(Key::from_bytes(name), default);
        self.compiled = None;
        Ok(self)
    }

    /// `getRequirements()`.
    pub fn requirements(&self) -> &PhpArray {
        &self.requirements
    }

    /// `setRequirements()`.
    pub fn set_requirements(&mut self, requirements: &PhpArray) -> Result<&mut Self, Error> {
        self.requirements = PhpArray::new();
        self.add_requirements(requirements)
    }

    /// `addRequirements()` with `string|\Stringable` values.
    pub fn add_requirements(&mut self, requirements: &PhpArray) -> Result<&mut Self, Error> {
        let mut requirements = requirements.clone();
        if php::isset(&requirements, b"_locale") && self.is_localized()? {
            requirements.remove(&Key::from("_locale"));
        }
        for (key, regex) in requirements.iter() {
            let regex = php::string_argument(
                regex,
                "Symfony\\Component\\Routing\\Route::sanitizeRequirement",
                2,
                "regex",
            )?;
            let sanitized = sanitize_requirement(&key.bytes(), &regex)?;
            self.requirements
                .insert(key.clone(), Value::String(sanitized));
        }
        self.compiled = None;
        Ok(self)
    }

    /// `getRequirement()`.
    pub fn requirement(&self, key: &[u8]) -> Option<&PhpString> {
        php::get(&self.requirements, key).and_then(Value::as_string)
    }

    /// `hasRequirement()`.
    pub fn has_requirement(&self, key: &[u8]) -> bool {
        self.requirements.contains_key(&Key::from_bytes(key))
    }

    /// `setRequirement()`.
    pub fn set_requirement(&mut self, key: &[u8], regex: &[u8]) -> Result<&mut Self, Error> {
        if key == b"_locale" && self.is_localized()? {
            return Ok(self);
        }
        let sanitized = sanitize_requirement(key, regex)?;
        self.requirements
            .insert(Key::from_bytes(key), Value::String(sanitized));
        self.compiled = None;
        Ok(self)
    }

    /// `getCondition()`.
    pub fn condition(&self) -> &PhpString {
        &self.condition
    }

    /// `setCondition()`; a null condition is `''`.
    pub fn set_condition(&mut self, condition: impl Into<PhpString>) -> &mut Self {
        self.condition = condition.into();
        self.compiled = None;
        self
    }

    /// The compiled route, if compiled since the last change.
    pub fn compiled(&self) -> Option<&CompiledRoute> {
        self.compiled.as_ref()
    }

    /// `compile()` when `compiler_class` is `RouteCompiler`.
    pub fn compile(&mut self) -> Result<&CompiledRoute, Error> {
        self.compile_with::<RouteCompiler>()
    }

    /// `compile()` when `compiler_class` names `C` (or `RouteCompiler`).
    pub fn compile_with<C: RouteCompilerInterface>(&mut self) -> Result<&CompiledRoute, Error> {
        if self.compiled.is_none() {
            let class = self.option(b"compiler_class").cloned().unwrap_or_default();
            let named = |name: &str| matches!(&class, Value::String(s) if s.as_bytes().eq_ignore_ascii_case(name.as_bytes()));
            let compiled = if named(C::CLASS) {
                C::compile(self)?
            } else if named(RouteCompiler::CLASS) {
                RouteCompiler::compile(self)?
            } else {
                return Err(match class {
                    Value::String(s) => Error::new(
                        Class::PhpError,
                        concat(&[b"Class \"", s.as_bytes(), b"\" not found"]),
                    ),
                    _ => Error::new(
                        Class::PhpError,
                        "Class name must be a valid object or a string",
                    ),
                });
            };
            self.compiled = Some(compiled);
        }
        self.compiled
            .as_ref()
            .ok_or_else(|| Error::new(Class::NotPorted, "route not compiled"))
    }

    fn extract_inline_defaults_and_requirements(
        &mut self,
        pattern: PhpString,
    ) -> Result<PhpString, Error> {
        if !pattern.iter().any(|c| matches!(c, b'?' | b'<' | b':')) {
            return Ok(pattern);
        }
        let mut mapping = match self.default_value(b"_route_mapping") {
            None => PhpArray::new(),
            Some(Value::Array(a)) => a.clone(),
            Some(other) => {
                return Err(Error::new(
                    Class::NotPorted,
                    format!(
                        "a _route_mapping default of type {} is not ported",
                        other.debug_type()
                    ),
                ));
            }
        };
        let mut failure = None;
        let replaced = pcre(&INLINE)?
            .replace_callback(&pattern, |m| {
                let group = |i: usize| m.get(i).copied().flatten().filter(|g| !g.is_empty());
                if failure.is_some() {
                    return m[0].unwrap_or_default().to_vec();
                }
                let name = m.get(2).copied().flatten().unwrap_or_default();
                let mut step = || -> Result<(), Error> {
                    if let Some(default) = group(7) {
                        let value = if default == b"?" {
                            Value::Null
                        } else {
                            Value::String(PhpString::from(&default[1..]))
                        };
                        self.set_default(name, value)?;
                    }
                    if let Some(requirement) = group(6) {
                        self.set_requirement(name, &requirement[1..requirement.len() - 1])?;
                    }
                    if let Some(argument) = group(4) {
                        let target = match group(5) {
                            Some(path) => Value::Array(PhpArray::list([
                                Value::String(PhpString::from(argument)),
                                Value::String(PhpString::from(&path[1..])),
                            ])),
                            None => Value::String(PhpString::from(argument)),
                        };
                        mapping.insert(Key::from_bytes(name), target);
                    }
                    Ok(())
                };
                if let Err(e) = step() {
                    failure = Some(e);
                }
                concat(&[
                    b"{",
                    m.get(1).copied().flatten().unwrap_or_default(),
                    name,
                    b"}",
                ])
            })
            .map_err(match_error)?;
        if let Some(e) = failure {
            return Err(e);
        }
        if !mapping.is_empty() {
            self.set_default(b"_route_mapping", Value::Array(mapping))?;
        }
        Ok(PhpString::from(replaced))
    }

    /// `isLocalized()`.
    fn is_localized(&self) -> Result<bool, Error> {
        let (Some(locale), true) = (
            php::get(&self.defaults, b"_locale"),
            php::isset(&self.defaults, b"_canonical_route"),
        ) else {
            return Ok(false);
        };
        let locale = php::string_argument(locale, "preg_quote", 1, "str")?;
        let quoted = php_pcre::preg_quote(&locale, None);
        Ok(self
            .requirement(b"_locale")
            .is_some_and(|r| r.as_bytes() == quoted.as_slice()))
    }
}

/// `sanitizeRequirement()`.
fn sanitize_requirement(key: &[u8], regex: &[u8]) -> Result<PhpString, Error> {
    let mut regex = regex;
    if let Some(rest) = regex.strip_prefix(b"^") {
        regex = rest;
    } else if let Some(rest) = regex.strip_prefix(br"\A") {
        regex = rest;
    }
    if let Some(rest) = regex.strip_suffix(b"$") {
        regex = rest;
    } else if regex.len() >= 2
        && regex.windows(2).position(|w| w == br"\z") == Some(regex.len() - 2)
    {
        // `strlen($regex) - 2 === strpos($regex, '\\z')`: the first `\z`
        // must be the last two bytes
        regex = &regex[..regex.len() - 2];
    }
    if regex.is_empty() {
        return Err(Error::new(
            Class::PhpInvalidArgument,
            concat(&[b"Routing requirement for \"", key, b"\" cannot be empty."]),
        ));
    }
    Ok(PhpString::from(regex))
}
