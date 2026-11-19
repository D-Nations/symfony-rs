//! Port of Symfony\Component\Config\Definition\Builder\EnumNodeDefinition.php:
//! the definition of an EnumNode, from a list of values or an enum
//! class (looked up in a [`Registry`], PHP's class table here).
//!
//! Symfony source copyright (c) Fabien Potencier; distributed under MIT.
//!
//! Upstream: symfony/config Definition/Builder/EnumNodeDefinition.php

use php_value::{Value, constants::Registry};

use crate::definition::{
    base_node::Node,
    builder::node_definition::{Def, Definition, class},
    enum_node::{self, EnumNode},
    exception::{Class, Error},
};

/// EnumNodeDefinition.
pub type EnumNodeDefinition<P = ()> = Definition<class::Enum, P>;

impl<P: Clone> Definition<class::Enum, P> {
    /// `values($values)`: at least one.
    pub fn values(&self, values: Vec<Value>) -> Result<Self, Error> {
        if values.is_empty() {
            return Err(Error::new(
                Class::InvalidArgument,
                "->values() must be called with at least one value.",
            ));
        }
        self.def.write(|s| s.enumeration.values = Some(values));
        Ok(self.clone())
    }

    /// `enumFqcn($enumFqcn)`: an enum of `enums`.
    pub fn enum_fqcn(&self, enum_fqcn: &str, enums: &Registry) -> Result<Self, Error> {
        if !enums.enum_exists(enum_fqcn) {
            return Err(Error::new(
                Class::InvalidArgument,
                format!("The enum class \"{enum_fqcn}\" does not exist."),
            ));
        }
        let fqcn = (enum_fqcn.to_owned(), enums.clone());
        self.def.write(|s| s.enumeration.enum_fqcn = Some(fqcn));
        Ok(self.clone())
    }
}

/// `instantiateNode()`: values or an enum class, not both.
pub(crate) fn instantiate_node(def: &Def) -> Result<Node, Error> {
    let (values, enum_fqcn, separator) = def.read(|s| {
        (
            s.enumeration.values.clone(),
            s.enumeration.enum_fqcn.clone(),
            s.path_separator.clone(),
        )
    });
    match (&values, &enum_fqcn) {
        (None, None) => {
            return Err(Error::new(
                Class::Runtime,
                "You must call either ->values() or ->enumFqcn() on enum nodes.",
            ));
        }
        (Some(_), Some(_)) => {
            return Err(Error::new(
                Class::Runtime,
                "You must call either ->values() or ->enumFqcn() on enum nodes but not both.",
            ));
        }
        _ => {}
    }
    let parent = def.parent_node(enum_node::CLASS)?;
    let (fqcn, enums) = match enum_fqcn {
        Some((f, r)) => (Some(f), r),
        None => (None, Registry::empty()),
    };
    EnumNode::with_options(
        def.name_bytes(),
        parent.as_ref(),
        values.unwrap_or_default(),
        &separator,
        fqcn.as_deref(),
        &enums,
    )
}
