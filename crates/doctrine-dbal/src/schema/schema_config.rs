//! Port of Doctrine\DBAL\Schema\SchemaConfig.php and
//! TableConfiguration.php.
//!
//! Doctrine source copyright (c) Doctrine Project; distributed under MIT.
//!
//! Upstream: doctrine/dbal src/Schema/SchemaConfig.php src/Schema/TableConfiguration.php

use php_value::PhpArray;

/// Configuration for a Schema.
#[derive(Debug, Clone, PartialEq)]
pub struct SchemaConfig {
    max_identifier_length: i64,
    name: Option<String>,
    default_table_options: PhpArray,
}

impl Default for SchemaConfig {
    fn default() -> Self {
        Self {
            max_identifier_length: 63,
            name: None,
            default_table_options: PhpArray::new(),
        }
    }
}

impl SchemaConfig {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn set_max_identifier_length(&mut self, length: i64) {
        self.max_identifier_length = length;
    }

    pub fn get_max_identifier_length(&self) -> i64 {
        self.max_identifier_length
    }

    /// The default namespace of schema objects.
    pub fn get_name(&self) -> Option<&str> {
        self.name.as_deref()
    }

    pub fn set_name(&mut self, name: Option<&str>) {
        self.name = name.map(str::to_owned);
    }

    /// The default options passed to tables during creation.
    pub fn get_default_table_options(&self) -> &PhpArray {
        &self.default_table_options
    }

    pub fn set_default_table_options(&mut self, default_table_options: PhpArray) {
        self.default_table_options = default_table_options;
    }

    pub fn to_table_configuration(&self) -> TableConfiguration {
        TableConfiguration::new(self.max_identifier_length)
    }
}

/// Contains platform-specific parameters used for creating and managing
/// objects scoped to a table.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TableConfiguration {
    max_identifier_length: i64,
}

impl TableConfiguration {
    pub fn new(max_identifier_length: i64) -> Self {
        Self {
            max_identifier_length,
        }
    }

    pub fn get_max_identifier_length(&self) -> i64 {
        self.max_identifier_length
    }
}
