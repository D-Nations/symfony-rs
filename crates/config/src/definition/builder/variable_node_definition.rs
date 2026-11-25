//! Port of Symfony\Component\Config\Definition\Builder\VariableNodeDefinition.php:
//! the definition of a `VariableNode`, and `createNode()` for it and its
//! subclasses (scalar, string, boolean, integer, float and enum), whose
//! `instantiateNode()` is in their own modules.
//!
//! Symfony source copyright (c) Fabien Potencier; distributed under MIT.
//!
//! Upstream: symfony/config Definition/Builder/VariableNodeDefinition.php

use php_value::Value;

use crate::definition::{
    base_node::{BASE_NODE, Node},
    builder::{
        boolean_node_definition, enum_node_definition, float_node_definition,
        integer_node_definition,
        node_definition::{Builtin, Def, Definition, class},
        scalar_node_definition, string_node_definition,
    },
    exception::Error,
    variable_node::VariableNode,
};

/// `VariableNodeDefinition`.
pub type VariableNodeDefinition<P = ()> = Definition<class::Variable, P>;

/// `VariableNodeDefinition`'s `instantiateNode()`.
fn instantiate_node(def: &Def) -> Result<Node, Error> {
    let parent = def.parent_node(BASE_NODE)?;
    VariableNode::with_separator(
        def.name_bytes(),
        parent.as_ref(),
        &def.read(|s| s.path_separator.clone()),
    )
}

/// `createNode()` of `VariableNodeDefinition` and its subclasses.
pub(crate) fn create_node(def: &Def, builtin: Builtin) -> Result<Node, Error> {
    let node = match builtin {
        Builtin::Scalar => scalar_node_definition::instantiate_node(def)?,
        Builtin::String => string_node_definition::instantiate_node(def)?,
        Builtin::Boolean => boolean_node_definition::instantiate_node(def)?,
        Builtin::Integer => integer_node_definition::instantiate_node(def)?,
        Builtin::Float => float_node_definition::instantiate_node(def)?,
        Builtin::Enum => enum_node_definition::instantiate_node(def)?,
        Builtin::Variable | Builtin::Array => instantiate_node(def)?,
    };

    if let Some(closures) = def.normalization_closures() {
        node.set_normalization_closures(closures);
    }
    if let Some(merge) = def.read(|s| s.merge) {
        node.set_allow_overwrite(merge.allow_overwrite);
    }
    let (default_value, allow_empty_value, required) = def.read(|s| {
        (
            s.default.then(|| s.default_value.clone()).flatten(),
            s.allow_empty_value,
            s.required,
        )
    });
    if let Some(value) = default_value {
        node.set_default_value(value)?;
    }
    node.set_allow_empty_value(allow_empty_value)?;
    let [null, truthy, falsy] = def.equivalents();
    node.add_equivalent_value(Value::Null, null);
    node.add_equivalent_value(Value::Bool(true), truthy);
    node.add_equivalent_value(Value::Bool(false), falsy);
    node.set_required(required);
    if let Some([package, version, message]) = def.deprecation() {
        node.set_deprecated(&package, &version, &message);
    }
    if let Some(closures) = def.validation_closures() {
        node.set_final_validation_closures(closures);
    }
    Ok(node)
}
