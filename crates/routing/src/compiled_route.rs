//! Port of Symfony\Component\Routing\CompiledRoute.php.
//!
//! Tokens and variables are PHP arrays, as upstream stores them: the
//! class accepts any array (its own test passes `['tokens']`), and the
//! compiled matcher and generator read them by position. RouteCompiler
//! writes `['text', $text]` and `['variable', $separator, $regex, $name]`
//! with the optional `utf8` and `important` flags at indexes 4 and 5.
//!
//! A subclass (`CustomCompiledRoute` in upstream's fixtures) is the same
//! data under another class name ([`CompiledRoute::with_class`]).
//!
//! Symfony source copyright (c) Fabien Potencier; distributed under MIT.
//!
//! Upstream: symfony/routing CompiledRoute.php

use php_value::{PhpArray, PhpString, Value, object::GenericObject};

use crate::{
    exception::{Class, Error},
    php,
};

pub const CLASS: &str = "Symfony\\Component\\Routing\\CompiledRoute";

/// `CompiledRoute`: what RouteCompiler makes of a Route.
#[derive(Debug, Clone, PartialEq)]
pub struct CompiledRoute {
    class: String,
    static_prefix: PhpString,
    regex: PhpString,
    tokens: PhpArray,
    path_variables: PhpArray,
    host_regex: Option<PhpString>,
    host_tokens: PhpArray,
    host_variables: PhpArray,
    variables: PhpArray,
}

impl CompiledRoute {
    /// `new CompiledRoute($staticPrefix, $regex, $tokens, $pathVariables,
    /// $hostRegex, $hostTokens, $hostVariables, $variables)`.
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        static_prefix: impl Into<PhpString>,
        regex: impl Into<PhpString>,
        tokens: PhpArray,
        path_variables: PhpArray,
        host_regex: Option<PhpString>,
        host_tokens: PhpArray,
        host_variables: PhpArray,
        variables: PhpArray,
    ) -> Self {
        Self {
            class: CLASS.to_owned(),
            static_prefix: static_prefix.into(),
            regex: regex.into(),
            tokens,
            path_variables,
            host_regex,
            host_tokens,
            host_variables,
            variables,
        }
    }

    /// The same data as an instance of a subclass.
    pub fn with_class(mut self, class: impl Into<String>) -> Self {
        self.class = class.into();
        self
    }

    /// `$compiled::class`.
    pub fn class(&self) -> &str {
        &self.class
    }

    /// `__serialize()`.
    pub fn serialize_data(&self) -> PhpArray {
        let list = |a: &PhpArray| Value::Array(a.clone());
        [
            ("vars", list(&self.variables)),
            ("path_prefix", Value::String(self.static_prefix.clone())),
            ("path_regex", Value::String(self.regex.clone())),
            ("path_tokens", list(&self.tokens)),
            ("path_vars", list(&self.path_variables)),
            (
                "host_regex",
                self.host_regex.clone().map_or(Value::Null, Value::String),
            ),
            ("host_tokens", list(&self.host_tokens)),
            ("host_vars", list(&self.host_variables)),
        ]
        .into_iter()
        .collect()
    }

    /// The object `serialize()` writes.
    pub fn to_value(&self) -> Value {
        Value::Object(std::sync::Arc::new(GenericObject::new(
            self.class.clone(),
            self.serialize_data(),
        )))
    }

    /// `__unserialize($data)` on an instance of `class`.
    pub fn from_serialized(class: &str, data: &PhpArray) -> Result<Self, Error> {
        if php::is_stringable(data, "path_prefix")
            || php::is_stringable(data, "path_regex")
            || php::is_stringable(data, "host_regex")
        {
            return Err(Error::new(
                Class::PhpBadMethodCall,
                format!("Cannot unserialize {CLASS}"),
            ));
        }
        let string = |key, property| php::assign_string(data, key, CLASS, property, false);
        let array = |key, property| php::assign_array(data, key, CLASS, property);
        // in upstream's assignment order
        let variables = array("vars", "variables")?;
        let static_prefix = string("path_prefix", "staticPrefix")?.unwrap_or_default();
        let regex = string("path_regex", "regex")?.unwrap_or_default();
        let tokens = array("path_tokens", "tokens")?;
        let path_variables = array("path_vars", "pathVariables")?;
        let host_regex = php::assign_string(data, "host_regex", CLASS, "hostRegex", true)?;
        let host_tokens = array("host_tokens", "hostTokens")?;
        let host_variables = array("host_vars", "hostVariables")?;
        Ok(Self {
            class: class.to_owned(),
            static_prefix,
            regex,
            tokens,
            path_variables,
            host_regex,
            host_tokens,
            host_variables,
            variables,
        })
    }

    /// The compiled route an unserialized object holds: a
    /// `CompiledRoute` (or a class in `classes`, its subclasses).
    pub(crate) fn from_value(value: &Value, subclasses: &[&str]) -> Option<Result<Self, Error>> {
        let object = value.as_object()?;
        let class = object.class();
        let known = class.eq_ignore_ascii_case(CLASS)
            || subclasses.iter().any(|c| c.eq_ignore_ascii_case(class));
        known.then(|| Self::from_serialized(class, &object.properties()))
    }

    /// `getStaticPrefix()`.
    pub fn static_prefix(&self) -> &PhpString {
        &self.static_prefix
    }

    /// `getRegex()`.
    pub fn regex(&self) -> &PhpString {
        &self.regex
    }

    /// `getHostRegex()`.
    pub fn host_regex(&self) -> Option<&PhpString> {
        self.host_regex.as_ref()
    }

    /// `getTokens()`.
    pub fn tokens(&self) -> &PhpArray {
        &self.tokens
    }

    /// `getHostTokens()`.
    pub fn host_tokens(&self) -> &PhpArray {
        &self.host_tokens
    }

    /// `getVariables()`.
    pub fn variables(&self) -> &PhpArray {
        &self.variables
    }

    /// `getPathVariables()`.
    pub fn path_variables(&self) -> &PhpArray {
        &self.path_variables
    }

    /// `getHostVariables()`.
    pub fn host_variables(&self) -> &PhpArray {
        &self.host_variables
    }
}
