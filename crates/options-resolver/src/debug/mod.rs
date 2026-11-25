//! Port of Symfony\Component\OptionsResolver\Debug\OptionsResolverIntrospector.php.
//!
//! Upstream binds a closure to the resolver to read its private
//! properties; here the introspector reads the crate-private state.
//!
//! Symfony source copyright (c) Fabien Potencier; distributed under MIT.
//!
//! Upstream: symfony/options-resolver Debug/OptionsResolverIntrospector.php

use php_value::{Key, Value};

use crate::{
    exception::{Class, Error},
    options_resolver::{
        AllowedValues, Deprecation, NestedClosure, OptionClosure, OptionsResolver, State,
    },
    php::key_bytes,
};

/// Reads an option's definition from a resolver.
#[derive(Debug)]
pub struct OptionsResolverIntrospector<'a> {
    resolver: &'a OptionsResolver,
}

impl<'a> OptionsResolverIntrospector<'a> {
    pub fn new(resolver: &'a OptionsResolver) -> Self {
        Self { resolver }
    }

    /// Upstream's bound `$get` closure.
    fn get<T>(
        &self,
        option: &Key,
        message: &str,
        property: impl Fn(&State, &Key) -> Option<T>,
    ) -> Result<T, Error> {
        let state = self.resolver.state();
        if !state.defined.contains(option) {
            let mut text = b"The option \"".to_vec();
            text.extend(key_bytes(option));
            text.extend_from_slice(b"\" does not exist.");
            return Err(Error::new(Class::UndefinedOptions, text));
        }
        property(&state, option).ok_or_else(|| {
            let mut text = message.as_bytes().to_vec();
            text = replace_option(&text, option);
            Error::new(Class::NoConfiguration, text)
        })
    }

    /// `getDefault()`.
    pub fn get_default(&self, option: impl Into<Key>) -> Result<Value, Error> {
        self.get(
            &option.into(),
            "No default value was set for the \"%s\" option.",
            |s, o| s.defaults.get(o).cloned(),
        )
    }

    /// `getLazyClosures()`.
    pub fn get_lazy_closures(&self, option: impl Into<Key>) -> Result<Vec<OptionClosure>, Error> {
        self.get(
            &option.into(),
            "No lazy closures were set for the \"%s\" option.",
            |s, o| s.lazy.get(o).cloned(),
        )
    }

    /// `getAllowedTypes()`.
    pub fn get_allowed_types(&self, option: impl Into<Key>) -> Result<Vec<String>, Error> {
        self.get(
            &option.into(),
            "No allowed types were set for the \"%s\" option.",
            |s, o| s.allowed_types.get(o).cloned(),
        )
    }

    /// `getAllowedValues()`.
    pub fn get_allowed_values(&self, option: impl Into<Key>) -> Result<AllowedValues, Error> {
        self.get(
            &option.into(),
            "No allowed values were set for the \"%s\" option.",
            |s, o| s.allowed_values.get(o).cloned(),
        )
    }

    /// `getNormalizer()`: the first normalizer (`current()`).
    pub fn get_normalizer(&self, option: impl Into<Key>) -> Result<Option<OptionClosure>, Error> {
        Ok(self.get_normalizers(option)?.into_iter().next())
    }

    /// `getNormalizers()`.
    pub fn get_normalizers(&self, option: impl Into<Key>) -> Result<Vec<OptionClosure>, Error> {
        self.get(
            &option.into(),
            "No normalizer was set for the \"%s\" option.",
            |s, o| s.normalizers.get(o).cloned(),
        )
    }

    /// `getDeprecation()`.
    pub fn get_deprecation(&self, option: impl Into<Key>) -> Result<Deprecation, Error> {
        self.get(
            &option.into(),
            "No deprecation was set for the \"%s\" option.",
            |s, o| s.deprecated.get(o).cloned(),
        )
    }

    /// `getNestedOptions()`.
    pub fn get_nested_options(&self, option: impl Into<Key>) -> Result<Vec<NestedClosure>, Error> {
        self.get(
            &option.into(),
            "No nested option was set for the \"%s\" option.",
            |s, o| s.nested.get(o).cloned(),
        )
    }
}

/// `sprintf($format, $option)` for a format with one `%s`.
fn replace_option(format: &[u8], option: &Key) -> Vec<u8> {
    crate::php::replace(format, b"%s", &key_bytes(option))
}
