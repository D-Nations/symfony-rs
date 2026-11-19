//! Port of Symfony\Component\Config\Definition\PrototypedArrayNode.php:
//! an ArrayNode whose elements all follow one prototype node, keyed by
//! position or by a key attribute, with default children and a minimum
//! number of elements.
//!
//! The prototype is renamed for each element (`setName($key)`), as
//! upstream does, so error paths name the element. When an element keyed
//! by attribute is left with only "value", a clone of the prototype's
//! "value" child stands in for it; as upstream, later elements reuse the
//! first clone (`current()`), and each reuse prepends the prototype's
//! normalization closures again.
//!
//! Symfony source copyright (c) Fabien Potencier; distributed under MIT.
//!
//! Upstream: symfony/config Definition/PrototypedArrayNode.php

use php_value::{Key, PhpArray, PhpString, Value};

use crate::{
    definition::{
        array_node::{ArrayData, expect_array, remap_xml},
        base_node::{DEFAULT_PATH_SEPARATOR, Kind, Node, undefined_method},
        exception::{Class, Error},
    },
    php::{self, Msg},
};

pub const CLASS: &str = "Symfony\\Component\\Config\\Definition\\PrototypedArrayNode";

/// PrototypedArrayNode's own properties.
#[derive(Clone, Default)]
pub(crate) struct PrototypedData {
    pub(crate) prototype: Option<Node>,
    pub(crate) key_attribute: Option<PhpString>,
    pub(crate) remove_key_attribute: bool,
    pub(crate) min_number_of_elements: i64,
    pub(crate) default_value: PhpArray,
    pub(crate) default_children: Option<Vec<Value>>,
    pub(crate) value_prototypes: Vec<(Key, Node)>,
    pub(crate) default_to_null: bool,
}

/// `new PrototypedArrayNode($name, $parent, $pathSeparator)`.
pub struct PrototypedArrayNode;

impl PrototypedArrayNode {
    pub fn new(name: impl AsRef<[u8]>, parent: Option<&Node>) -> Result<Node, Error> {
        Self::with_separator(name, parent, DEFAULT_PATH_SEPARATOR.as_bytes())
    }

    pub fn with_separator(
        name: impl AsRef<[u8]>,
        parent: Option<&Node>,
        separator: &[u8],
    ) -> Result<Node, Error> {
        Node::construct(
            name.as_ref(),
            parent,
            separator,
            Kind::Prototyped(ArrayData::default(), PrototypedData::default()),
        )
    }
}

fn uninitialized_prototype() -> Error {
    Error::new(
        Class::Error,
        format!("Typed property {CLASS}::$prototype must not be accessed before initialization"),
    )
}

impl Node {
    fn with_prototyped<R>(
        &self,
        method: &str,
        f: impl FnOnce(&mut PrototypedData) -> R,
    ) -> Result<R, Error> {
        let result = self.write(|n| match &mut n.kind {
            Kind::Prototyped(_, data) => Some(f(data)),
            _ => None,
        });
        result.ok_or_else(|| undefined_method(&self.class(), method))
    }

    pub fn set_min_number_of_elements(&self, number: i64) -> Result<(), Error> {
        self.with_prototyped("setMinNumberOfElements", |d| {
            d.min_number_of_elements = number
        })
    }

    /// `setKeyAttribute($attribute, $remove = true)`.
    pub fn set_key_attribute(&self, attribute: &[u8], remove: bool) -> Result<(), Error> {
        self.with_prototyped("setKeyAttribute", |d| {
            d.key_attribute = Some(PhpString::from(attribute));
            d.remove_key_attribute = remove;
        })
    }

    pub fn get_key_attribute(&self) -> Result<Option<PhpString>, Error> {
        self.with_prototyped("getKeyAttribute", |d| d.key_attribute.clone())
    }

    pub(crate) fn set_prototyped_default_value(&self, value: PhpArray) {
        let _ = self.with_prototyped("setDefaultValue", |d| {
            d.default_value = value;
            d.default_to_null = false;
        });
    }

    pub fn set_null_as_default(&self) -> Result<(), Error> {
        self.with_prototyped("setNullAsDefault", |d| {
            d.default_value = PhpArray::new();
            d.default_to_null = true;
        })
    }

    /// `setAddChildrenIfNoneSet(int|string|array|null $children)`:
    /// upstream's default argument is `['defaults']`, which `null` also
    /// gives; a positive int `n` names the children 1 to n.
    pub fn set_add_children_if_none_set(&self, children: Value) -> Result<(), Error> {
        let children = match children {
            Value::Null => vec![Value::from("defaults")],
            // int|string|array|null takes a bool as an int, an integral
            // float as an int and another float as a string.
            Value::Bool(b) => int_children(i64::from(b)),
            Value::Int(n) => int_children(n),
            Value::Float(f) if f.fract() == 0.0 && f.abs() < 9.2e18 => int_children(f as i64),
            Value::Float(f) => vec![Value::from(php::float_string(f))],
            Value::String(s) => vec![Value::String(s)],
            Value::Array(a) => a.values().cloned().collect(),
            Value::Object(o) => {
                return Err(Error::new(
                    Class::TypeError,
                    format!(
                        "{CLASS}::setAddChildrenIfNoneSet(): Argument #1 ($children) must be of type array|string|int|null, {} given",
                        o.class()
                    ),
                ));
            }
        };
        self.with_prototyped("setAddChildrenIfNoneSet", |d| {
            d.default_children = Some(children);
            d.default_to_null = false;
        })
    }

    /// `setPrototype(PrototypeNodeInterface $node)`.
    pub fn set_prototype(&self, node: &Node) -> Result<(), Error> {
        if !self.is_prototyped_array_node() {
            return Err(undefined_method(&self.class(), "setPrototype"));
        }
        if !node.is_prototype_node() {
            return Err(Error::new(
                Class::TypeError,
                format!(
                    "{CLASS}::setPrototype(): Argument #1 ($node) must be of type Symfony\\Component\\Config\\Definition\\PrototypeNodeInterface, {} given",
                    node.class()
                ),
            ));
        }
        let node = node.clone();
        self.with_prototyped("setPrototype", |d| d.prototype = Some(node))
    }

    pub fn get_prototype(&self) -> Result<Node, Error> {
        self.with_prototyped("getPrototype", |d| d.prototype.clone())?
            .ok_or_else(uninitialized_prototype)
    }
}

fn int_children(n: i64) -> Vec<Value> {
    if n > 0 {
        (1..=n).map(Value::Int).collect()
    } else {
        vec![Value::Int(n)]
    }
}

/// PrototypedArrayNode's `getDefaultValue()`.
pub(crate) fn get_default_value(data: &PrototypedData) -> Result<Value, Error> {
    if data.default_to_null {
        return Ok(Value::Null);
    }
    let Some(children) = &data.default_children else {
        return Ok(Value::Array(data.default_value.clone()));
    };
    let prototype = data
        .prototype
        .as_ref()
        .ok_or_else(uninitialized_prototype)?;
    let default = if prototype.has_default_value() {
        prototype.get_default_value()?
    } else {
        Value::Array(PhpArray::new())
    };
    let mut defaults = PhpArray::new();
    for (i, name) in children.iter().enumerate() {
        let key = match data.key_attribute {
            None => Key::Int(i as i64),
            Some(_) => offset(name)?,
        };
        defaults.insert(key, default.clone());
    }
    Ok(Value::Array(defaults))
}

/// The array key a value gives, or PHP's error for an illegal offset.
fn offset(value: &Value) -> Result<Key, Error> {
    Key::from_value(value).ok_or_else(|| {
        Error::new(
            Class::TypeError,
            format!(
                "Cannot access offset of type {} on array",
                value.debug_type()
            ),
        )
    })
}

/// `getPrototypeForChild(string $key)`: the value prototype for the key,
/// or the prototype, renamed to the key.
fn get_prototype_for_child(node: &Node, key: &[u8]) -> Result<Node, Error> {
    let lookup = Key::from_bytes(key);
    let prototype = node.read(|n| match &n.kind {
        Kind::Prototyped(_, d) => d
            .value_prototypes
            .iter()
            .find(|(k, _)| *k == lookup)
            .map(|(_, p)| p.clone())
            .or_else(|| d.prototype.clone()),
        _ => None,
    });
    let prototype = prototype.ok_or_else(uninitialized_prototype)?;
    prototype.set_name(key)?;
    Ok(prototype)
}

/// PrototypedArrayNode's `finalizeValue()`.
pub(crate) fn finalize_value(
    node: &Node,
    data: &PrototypedData,
    value: Value,
) -> Result<Value, Error> {
    if matches!(value, Value::Bool(false)) {
        return Err(Error::new(
            Class::UnsetKey,
            Msg::new("Unsetting key for path \"")
                .b(&node.get_path())
                .s("\", value: false."),
        ));
    }
    let mut value = expect_array(value, "foreach")?;
    for (k, v) in value.clone().iter() {
        let prototype = get_prototype_for_child(node, &php::key_bytes(k))?;
        match prototype.finalize(v.clone()) {
            Ok(finalized) => value.insert(k.clone(), finalized),
            Err(e) if *e.class() == Class::UnsetKey => {
                value.remove(k);
            }
            Err(e) => return Err(e),
        }
    }
    if (value.len() as i64) < data.min_number_of_elements {
        return Err(node.config_error(
            Class::InvalidConfiguration,
            Msg::new("The path \"").b(&node.get_path()).s(&format!(
                "\" should have at least {} element(s) defined.",
                data.min_number_of_elements
            )),
            false,
        ));
    }
    Ok(Value::Array(value))
}

/// PrototypedArrayNode's `normalizeValue()`.
pub(crate) fn normalize_value(
    node: &Node,
    array: &ArrayData,
    data: &PrototypedData,
    value: Value,
) -> Result<Value, Error> {
    if matches!(value, Value::Bool(false)) {
        return Ok(value);
    }
    let value = remap_xml(array, expect_array(value, "foreach")?)?;
    let is_list = value.is_list();
    let mut normalized = PhpArray::new();
    for (key, element) in value.iter() {
        let mut k = key.to_value();
        let mut v = element.clone();
        if let (Some(attribute), Value::Array(fields)) = (&data.key_attribute, element) {
            let attribute_key = Key::from(attribute.clone());
            match fields.get(&attribute_key).filter(|a| !a.is_null()) {
                None if matches!(key, Key::Int(_)) && is_list => {
                    return Err(node.config_error(
                        Class::InvalidConfiguration,
                        Msg::new("The attribute \"")
                            .b(attribute)
                            .s("\" must be set for path \"")
                            .b(&node.get_path())
                            .s("\"."),
                        false,
                    ));
                }
                None => {}
                Some(named) => {
                    k = match named {
                        Value::Float(f) => Value::from(php::var_export_float(*f)),
                        other => other.clone(),
                    };
                    let mut fields = fields.clone();
                    if data.remove_key_attribute {
                        fields.remove(&attribute_key);
                    }
                    let value_key = Key::from("value");
                    if fields.len() == 1 && fields.contains_key(&value_key) {
                        v = fields.get(&value_key).cloned().unwrap_or_default();
                        use_value_prototype(node, &k)?;
                    } else {
                        v = Value::Array(fields);
                    }
                }
            }
            if normalized.contains_key(&array_key_exists_offset(&k)?) {
                return Err(node.config_error(
                    Class::DuplicateKey,
                    Msg::new("Duplicate key \"")
                        .b(&php::to_bytes(&k))
                        .s("\" for path \"")
                        .b(&node.get_path())
                        .s("\"."),
                    false,
                ));
            }
        }
        let prototype = get_prototype_for_child(node, &php::to_bytes(&k))?;
        let normalized_value = prototype.normalize(v)?;
        if data.key_attribute.is_some() || !is_list {
            normalized.insert(offset(&k)?, normalized_value);
        } else {
            normalized
                .push(normalized_value)
                .map_err(|e| Error::new(Class::Error, e.to_string()))?;
        }
    }
    Ok(Value::Array(normalized))
}

/// `array_key_exists($k, ...)`'s key, or its TypeError.
fn array_key_exists_offset(k: &Value) -> Result<Key, Error> {
    Key::from_value(k).ok_or_else(|| {
        Error::new(
            Class::TypeError,
            "array_key_exists(): Argument #1 ($key) must be a valid array offset type",
        )
    })
}

/// Registers the value prototype for an element keyed `k` that only has
/// "value" left, when the prototype is an ArrayNode with a "value" child.
fn use_value_prototype(node: &Node, k: &Value) -> Result<(), Error> {
    let prototype = node.get_prototype()?;
    if !prototype.is_array_node() {
        return Ok(());
    }
    let children = prototype.get_children()?;
    let Some((_, value_child)) = children
        .iter()
        .find(|(name, _)| *name == Key::from("value"))
    else {
        return Ok(());
    };
    let first = node.read(|n| match &n.kind {
        Kind::Prototyped(_, d) => d.value_prototypes.first().map(|(_, p)| p.clone()),
        _ => None,
    });
    let value_prototype = first.unwrap_or_else(|| value_child.clone_node());
    value_prototype.set_parent(Some(node.clone()));
    let mut closures = prototype.get_normalization_closures();
    closures.extend(value_prototype.get_normalization_closures());
    value_prototype.set_normalization_closures(closures);
    let key = offset(k)?;
    node.with_prototyped("normalize", |d| {
        match d
            .value_prototypes
            .iter_mut()
            .find(|(existing, _)| *existing == key)
        {
            Some(entry) => entry.1 = value_prototype,
            None => d.value_prototypes.push((key, value_prototype)),
        }
    })
}

/// PrototypedArrayNode's `mergeValues()`.
pub(crate) fn merge_values(
    node: &Node,
    array: &ArrayData,
    data: &PrototypedData,
    left: Value,
    right: Value,
) -> Result<Value, Error> {
    if matches!(right, Value::Bool(false)) {
        return Ok(right);
    }
    if matches!(left, Value::Bool(false)) || !array.perform_deep_merging {
        return Ok(right);
    }
    let mut left = expect_array(left, "array_key_exists")?;
    let right = expect_array(right, "array_is_list")?;
    // allowNewKeys only applies once a first config is in.
    let initial_population = left.is_empty();
    let is_list = right.is_list();
    for (k, v) in right.iter() {
        if data.key_attribute.is_none() && is_list {
            left.push(v.clone())
                .map_err(|e| Error::new(Class::Error, e.to_string()))?;
            continue;
        }
        if !left.contains_key(k) {
            if !array.allow_new_keys && !initial_population {
                return Err(node.config_error(
                    Class::InvalidConfiguration,
                    Msg::new("You are not allowed to define new elements for path \"")
                        .b(&node.get_path())
                        .s("\". Please define all elements for this path in one config file."),
                    false,
                ));
            }
            if matches!(v, Value::Array(_)) {
                let prototype = get_prototype_for_child(node, &php::key_bytes(k))?;
                if prototype.is_array_node() {
                    // So that the prototype's merge() handles auto_enable.
                    let merged = prototype.merge(Value::Array(PhpArray::new()), v.clone())?;
                    left.insert(k.clone(), merged);
                    continue;
                }
            }
            left.insert(k.clone(), v.clone());
            continue;
        }
        let prototype = get_prototype_for_child(node, &php::key_bytes(k))?;
        let current = left.get(k).cloned().unwrap_or_default();
        let merged = prototype.merge(current, v.clone())?;
        left.insert(k.clone(), merged);
    }
    Ok(Value::Array(left))
}
