//! Port of Symfony\Component\OptionsResolver\OptionConfigurator.php.
//!
//! Symfony source copyright (c) Fabien Potencier; distributed under MIT.
//!
//! Upstream: symfony/options-resolver OptionConfigurator.php

use php_value::Key;

use crate::{
    exception::Error,
    options_resolver::{
        AllowedValue, AllowedValues, DeprecationMessage, NestedClosure, OptionClosure,
        OptionsResolver,
    },
};

/// Configures one option of a resolver (`$resolver->define('name')`).
#[derive(Debug)]
pub struct OptionConfigurator<'a> {
    name: Key,
    resolver: &'a OptionsResolver,
}

impl<'a> OptionConfigurator<'a> {
    /// `new OptionConfigurator($name, $resolver)`: defines the option.
    pub fn new(name: Key, resolver: &'a OptionsResolver) -> Result<Self, Error> {
        resolver.set_defined_key(name.clone())?;
        Ok(Self { name, resolver })
    }

    /// `allowedTypes(string ...$types)`.
    pub fn allowed_types(&self, types: &[&str]) -> Result<&Self, Error> {
        self.resolver.set_allowed_types(self.name.clone(), types)?;
        Ok(self)
    }

    /// `allowedValues(mixed ...$values)`.
    pub fn allowed_values(
        &self,
        values: impl IntoIterator<Item = AllowedValue>,
    ) -> Result<&Self, Error> {
        self.resolver
            .set_allowed_values(self.name.clone(), AllowedValues::list(values))?;
        Ok(self)
    }

    /// `default(mixed $value)`.
    pub fn default(
        &self,
        value: impl Into<crate::options_resolver::DefaultValue>,
    ) -> Result<&Self, Error> {
        self.resolver.set_default(self.name.clone(), value)?;
        Ok(self)
    }

    /// `define(string $option)`.
    pub fn define(&self, option: impl Into<Key>) -> Result<OptionConfigurator<'a>, Error> {
        self.resolver.define(option)
    }

    /// `deprecated(string $package, string $version, string|\Closure
    /// $message = 'The option "%name%" is deprecated.')`.
    pub fn deprecated(
        &self,
        package: &str,
        version: &str,
        message: impl Into<DeprecationMessage>,
    ) -> Result<&Self, Error> {
        self.resolver
            .set_deprecated(self.name.clone(), package, version, message)?;
        Ok(self)
    }

    /// `normalize(\Closure $normalizer)`.
    pub fn normalize(&self, normalizer: OptionClosure) -> Result<&Self, Error> {
        self.resolver
            .set_normalizer(self.name.clone(), normalizer)?;
        Ok(self)
    }

    /// `required()`.
    pub fn required(&self) -> Result<&Self, Error> {
        self.resolver.set_required_key(self.name.clone())?;
        Ok(self)
    }

    /// `info(string $info)`.
    pub fn info(&self, info: &str) -> Result<&Self, Error> {
        self.resolver.set_info(self.name.clone(), info)?;
        Ok(self)
    }

    /// `ignoreUndefined(bool $ignore = true)`.
    pub fn ignore_undefined(&self, ignore: bool) -> &Self {
        self.resolver.set_ignore_undefined(ignore);
        self
    }

    /// `options(\Closure $nested)`.
    pub fn options(&self, nested: NestedClosure) -> Result<&Self, Error> {
        self.resolver.set_options(self.name.clone(), nested)?;
        Ok(self)
    }
}
