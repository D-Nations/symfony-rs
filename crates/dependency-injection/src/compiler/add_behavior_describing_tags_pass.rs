//! Port of Symfony\Component\DependencyInjection\Compiler\AddBehaviorDescribingTagsPass.php:
//! allows bundles to extend the list of behavior-describing tags.
//!
//! Upstream: symfony/dependency-injection Compiler/AddBehaviorDescribingTagsPass.php

use php_value::{PhpArray, Value};

use super::compiler_pass_interface::CompilerPassInterface;
use crate::container_builder::ContainerBuilder;
use crate::exception::Result;

const DEFAULT_TAGS: [&str; 5] = [
    "proxy",
    "container.do_not_inline",
    "container.service_locator",
    "container.service_subscriber",
    "container.service_subscriber.locator",
];

/// `AddBehaviorDescribingTagsPass`.
#[derive(Debug, Clone, Default)]
pub struct AddBehaviorDescribingTagsPass {
    tags: Vec<String>,
}

impl AddBehaviorDescribingTagsPass {
    /// `new AddBehaviorDescribingTagsPass($tags)`.
    pub fn new(tags: Vec<String>) -> Self {
        Self { tags }
    }
}

impl CompilerPassInterface for AddBehaviorDescribingTagsPass {
    fn class(&self) -> &'static str {
        "Symfony\\Component\\DependencyInjection\\Compiler\\AddBehaviorDescribingTagsPass"
    }

    fn process(&mut self, container: &mut ContainerBuilder) -> Result<()> {
        let tags = if container.has_parameter("container.behavior_describing_tags") {
            match container.get_parameter("container.behavior_describing_tags")? {
                Value::Array(a) => a,
                _ => PhpArray::new(),
            }
        } else {
            PhpArray::list(DEFAULT_TAGS.iter().map(|t| Value::from(*t)))
        };

        // array_unique(array_merge($tags, $this->tags)): string keys are
        // kept, int keys renumbered, and the first of equal values wins.
        let mut merged = PhpArray::new();
        for (key, tag) in tags.iter() {
            match key {
                php_value::array::Key::Int(_) => {
                    let _ = merged.push(tag.clone());
                }
                key @ php_value::array::Key::Str(_) => merged.insert(key.clone(), tag.clone()),
            }
        }
        for tag in &self.tags {
            let _ = merged.push(Value::from(tag.as_str()));
        }
        let mut unique = PhpArray::new();
        let mut seen: Vec<Vec<u8>> = Vec::new();
        for (key, tag) in merged.iter() {
            let text = tag
                .to_php_string()
                .map(|s| s.as_bytes().to_vec())
                .unwrap_or_default();
            if seen.contains(&text) {
                continue;
            }
            seen.push(text);
            unique.insert(key.clone(), tag.clone());
        }

        container.set_parameter("container.behavior_describing_tags", Value::Array(unique))
    }
}
