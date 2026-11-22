//! Upstream's Tests/Fixtures/Configuration: ExampleConfiguration, the tree
//! the reference dumper tests print, and CustomNode / CustomNodeDefinition
//! (a node that implements NodeInterface without extending BaseNode).
//! Upstream's `prototype('array')` is `array_prototype()`, which is the
//! same call with its return type checked.
//!
//! Upstream: symfony/config Tests/Fixtures/Configuration/ExampleConfiguration.php Tests/Fixtures/Configuration/CustomNode.php Tests/Fixtures/Configuration/CustomNodeDefinition.php

use std::sync::Arc;

use php_value::{PhpArray, PhpString, Value, constants::Registry};
use symfony_config::{
    ConfigurationInterface, Error, Node,
    definition::{
        CustomNode as CustomNodeClass,
        builder::{CustomDefinition, NodeClass, NodeDefinition, TreeBuilder},
    },
};

use super::{case, enums, fixture};

pub const CUSTOM_NODE: &str =
    "Symfony\\Component\\Config\\Tests\\Fixtures\\Configuration\\CustomNode";
pub const CUSTOM_NODE_DEFINITION: &str =
    "Symfony\\Component\\Config\\Tests\\Fixtures\\Configuration\\CustomNodeDefinition";

/// `class CustomNode implements NodeInterface`, under the given class
/// name (the oracle's copy lives in another namespace).
pub struct CustomNode(pub &'static str);

impl CustomNodeClass for CustomNode {
    fn class(&self) -> &str {
        self.0
    }

    fn validate_type(&self, _node: &Node, _value: &Value) -> Result<(), Error> {
        Ok(())
    }

    fn normalize_value(&self, _node: &Node, value: Value) -> Result<Value, Error> {
        Ok(value)
    }

    /// `array_merge($leftSide, $rightSide)`.
    fn merge_values(&self, _node: &Node, left: Value, right: Value) -> Result<Value, Error> {
        let (Value::Array(left), Value::Array(right)) = (left, right) else {
            return Err(Error::new(
                symfony_config::Class::TypeError,
                "array_merge(): Argument must be of type array",
            ));
        };
        let mut merged = PhpArray::new();
        for (k, v) in left.iter().chain(right.iter()) {
            match k {
                php_value::Key::Int(_) => {
                    if merged.push(v.clone()).is_err() {
                        return Err(Error::new(
                            symfony_config::Class::Error,
                            "Cannot add element to the array as the next element is already occupied",
                        ));
                    }
                }
                k => {
                    merged.insert(k.clone(), v.clone());
                }
            }
        }
        Ok(Value::Array(merged))
    }

    fn finalize_value(&self, _node: &Node, value: Value) -> Result<Value, Error> {
        Ok(value)
    }

    fn has_default_value(&self, _node: &Node) -> bool {
        true
    }

    fn get_default_value(&self, _node: &Node) -> Result<Value, Error> {
        Ok(Value::Bool(true))
    }

    fn get_path(&self, _node: &Node) -> Option<PhpString> {
        Some(PhpString::from("custom"))
    }

    fn extends_base_node(&self) -> bool {
        false
    }
}

/// `class CustomNodeDefinition extends NodeDefinition`.
pub struct CustomNodeDefinition;

impl CustomDefinition for CustomNodeDefinition {
    fn class(&self) -> &str {
        CUSTOM_NODE_DEFINITION
    }

    /// `new CustomNode()`, whose `getName()` is "custom_node".
    fn create_node(&self, _definition: &NodeDefinition) -> Result<Node, Error> {
        Node::custom(
            b"custom_node",
            None,
            b".",
            Arc::new(CustomNode(CUSTOM_NODE)),
        )
    }
}

/// `class ExampleConfiguration implements ConfigurationInterface`.
pub struct ExampleConfiguration {
    enums: Registry,
}

impl ExampleConfiguration {
    pub fn new() -> Self {
        Self { enums: enums() }
    }
}

impl ConfigurationInterface for ExampleConfiguration {
    fn get_config_tree_builder(&self) -> Result<TreeBuilder, Error> {
        let tree_builder = TreeBuilder::new("acme_root")?;
        let list =
            |items: &[&str]| Value::Array(PhpArray::list(items.iter().map(|s| Value::from(*s))));

        tree_builder
            .get_root_node()
            .fix_xml_config("parameter", None)
            .fix_xml_config("connection", None)
            .fix_xml_config("cms_page", None)
            .children()
            .boolean_node("boolean")?
            .default_true()?
            .end()
            .scalar_node("scalar_empty")?
            .end()
            .scalar_node("scalar_null")?
            .default_null()?
            .end()
            .scalar_node("scalar_true")?
            .default_true()?
            .end()
            .scalar_node("scalar_false")?
            .default_false()?
            .end()
            .scalar_node("scalar_default")?
            .default_value(Value::from("default"))?
            .end()
            .scalar_node("scalar_array_empty")?
            .default_value(Value::Array(PhpArray::new()))?
            .end()
            .scalar_node("scalar_array_defaults")?
            .default_value(list(&["elem1", "elem2"]))?
            .end()
            .scalar_node("scalar_required")?
            .is_required()?
            .end()
            .scalar_node("scalar_deprecated")?
            .set_deprecated(
                "vendor/package",
                "1.1",
                symfony_config::definition::DEFAULT_DEPRECATION_MESSAGE,
            )
            .end()
            .scalar_node("scalar_deprecated_with_message")?
            .set_deprecated(
                "vendor/package",
                "1.1",
                "Deprecation custom message for \"%node%\" at \"%path%\"",
            )
            .end()
            .scalar_node("node_with_a_looong_name")?
            .end()
            .enum_node("enum_with_default")?
            .values(vec![Value::from("this"), Value::from("that")])?
            .default_value(Value::from("this"))?
            .end()
            .enum_node("enum")?
            .values(vec![
                Value::from("this"),
                Value::from("that"),
                case(&self.enums, "TestEnum::Ccc"),
            ])?
            .end()
            .enum_node("enum_with_class")?
            .enum_fqcn(&fixture("StringBackedTestEnum"), &self.enums)?
            .end()
            .enum_node("unit_enum_with_class")?
            .enum_fqcn(&fixture("TestEnum"), &self.enums)?
            .end()
            .array_node("array", None)?
            .info("some info")
            .can_be_unset(true)
            .children()
            .scalar_node("child1")?
            .end()
            .scalar_node("child2")?
            .end()
            .scalar_node("child3")?
            .info(
                "this is a long\n\
                             multi-line info text\n\
                             which should be indented",
            )
            .example(Value::from("example setting"))?
            .end()
            .end()
            .end()
            .array_node("scalar_prototyped", None)?
            .prototype("scalar")?
            .end()
            .end()
            .array_node("string_list_default_null", None)?
            .default_null()?
            .string_prototype()?
            .end()
            .end()
            .variable_node("variable")?
            .example(list(&["foo", "bar"]))?
            .end()
            .array_node("parameters", None)?
            .use_attribute_as_key("name", true)
            .prototype("scalar")?
            .info("Parameter name")
            .end()
            .end()
            .array_node("connections", None)?
            .array_prototype()?
            .children()
            .scalar_node("user")?
            .end()
            .scalar_node("pass")?
            .end()
            .end()
            .end()
            .end()
            .array_node("cms_pages", None)?
            .use_attribute_as_key("page", true)
            .array_prototype()?
            .use_attribute_as_key("locale", true)
            .array_prototype()?
            .children()
            .scalar_node("title")?
            .is_required()?
            .end()
            .scalar_node("path")?
            .is_required()?
            .end()
            .end()
            .end()
            .end()
            .end()
            .array_node("pipou", None)?
            .use_attribute_as_key("name", true)
            .array_prototype()?
            .array_prototype()?
            .children()
            .scalar_node("didou")?
            .end()
            .end()
            .end()
            .end()
            .end()
            .array_node("array_with_array_example_and_no_default_value", None)?
            .example(list(&["foo", "bar"]))?
            .end()
            .append(&NodeDefinition::of_class(
                &NodeClass::Custom(Arc::new(CustomNodeDefinition)),
                "acme",
            )?)
            .end();

        Ok(tree_builder)
    }
}
