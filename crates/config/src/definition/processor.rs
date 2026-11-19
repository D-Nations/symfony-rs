//! Port of Symfony\Component\Config\Definition\Processor.php:
//! normalizes, merges and finalizes a list of configs against a tree, and
//! `normalizeConfig()`, which turns an XML singular into a list.
//!
//! Symfony source copyright (c) Fabien Potencier; distributed under MIT.
//!
//! Upstream: symfony/config Definition/Processor.php

use php_value::{Key, PhpArray, Value};

use crate::{
    definition::{
        base_node::Node,
        configuration_interface::ConfigurationInterface,
        exception::{Class, Error},
    },
    php,
};

pub const CLASS: &str = "Symfony\\Component\\Config\\Definition\\Processor";

/// The processor.
#[derive(Debug, Clone, Copy, Default)]
pub struct Processor;

impl Processor {
    pub fn new() -> Self {
        Self
    }

    /// `process($configTree, $configs)`: each config normalized and merged
    /// into the previous ones, then the result finalized.
    pub fn process(&self, config_tree: &Node, configs: &PhpArray) -> Result<PhpArray, Error> {
        let mut current = Value::Array(PhpArray::new());
        for config in configs.values() {
            let config = config_tree.normalize(config.clone())?;
            current = config_tree.merge(current, config)?;
        }
        match config_tree.finalize(current)? {
            Value::Array(array) => Ok(array),
            other => Err(Error::new(
                Class::TypeError,
                format!(
                    "{CLASS}::process(): Return value must be of type array, {} returned",
                    php::error_type_name(&other)
                ),
            )),
        }
    }

    /// `processConfiguration($configuration, $configs)`: `process()` on
    /// the configuration's tree.
    pub fn process_configuration(
        &self,
        configuration: &dyn ConfigurationInterface,
        configs: &PhpArray,
    ) -> Result<PhpArray, Error> {
        let tree = configuration.get_config_tree_builder()?.build_tree()?;
        self.process(&tree, configs)
    }

    /// `Processor::normalizeConfig($config, $key, $plural)`: `$plural`
    /// (default: the key and "s") when set, else the singular as a list
    /// (a string, or an array whose first key is not an int, is one
    /// element), else `[]`.
    pub fn normalize_config(
        config: &PhpArray,
        key: &[u8],
        plural: Option<&[u8]>,
    ) -> Result<PhpArray, Error> {
        let plural = match plural {
            Some(p) => p.to_vec(),
            None => [key, b"s"].concat(),
        };
        if let Some(value) = config
            .get(&Key::from_bytes(&plural))
            .filter(|v| !v.is_null())
        {
            return match value {
                Value::Array(array) => Ok(array.clone()),
                other => Err(Error::new(
                    Class::TypeError,
                    format!(
                        "{CLASS}::normalizeConfig(): Return value must be of type array, {} returned",
                        php::error_type_name(other)
                    ),
                )),
            };
        }
        let Some(value) = config.get(&Key::from_bytes(key)).filter(|v| !v.is_null()) else {
            return Ok(PhpArray::new());
        };
        match value {
            Value::Array(array) if matches!(array.first(), Some((Key::Int(_), _))) => {
                Ok(array.clone())
            }
            // A string, an array keyed by a string or empty, or an object
            // (`key()` on an object is deprecated but works; its property
            // names are not ints here).
            Value::String(_) | Value::Array(_) | Value::Object(_) => {
                Ok(PhpArray::list([value.clone()]))
            }
            other => Err(Error::new(
                Class::TypeError,
                format!(
                    "key(): Argument #1 ($array) must be of type array, {} given",
                    php::error_type_name(other)
                ),
            )),
        }
    }
}
