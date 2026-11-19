//! Port of Symfony\Component\Yaml\Tag\TaggedValue.php.
//!
//! Symfony source copyright (c) Fabien Potencier; distributed under MIT.
//!
//! Upstream: symfony/yaml Tag/TaggedValue.php

use php_value::{PhpArray, PhpObject, Value, php_array};

/// A value with a custom tag (`!my_tag value`, under `PARSE_CUSTOM_TAGS`).
#[derive(Debug, Clone, PartialEq)]
pub struct TaggedValue {
    tag: String,
    value: Value,
}

impl TaggedValue {
    pub const CLASS: &'static str = "Symfony\\Component\\Yaml\\Tag\\TaggedValue";

    pub fn new(tag: impl Into<String>, value: Value) -> Self {
        Self {
            tag: tag.into(),
            value,
        }
    }

    pub fn get_tag(&self) -> &str {
        &self.tag
    }

    pub fn get_value(&self) -> &Value {
        &self.value
    }
}

impl PhpObject for TaggedValue {
    fn class(&self) -> &str {
        Self::CLASS
    }

    fn properties(&self) -> PhpArray {
        php_array!["tag" => self.tag.as_str(), "value" => self.value.clone()]
    }
}
