//! Port of Symfony\Component\Config\Definition\ArrayNode.php: a node with
//! named children. It normalizes dashed keys to underscores, remaps XML
//! singulars to plurals, rejects unknown keys (with "Did you mean"
//! guesses by Levenshtein distance), merges deeply, and fills defaults
//! and checks required children when finalizing.
//!
//! Children are keyed as PHP array keys, so a child named "0" is the key
//! `0`, as upstream's `$this->children[$name]` makes it.
//!
//! Symfony source copyright (c) Fabien Potencier; distributed under MIT.
//!
//! Upstream: symfony/config Definition/ArrayNode.php

use php_value::{Key, PhpArray, PhpString, Value};

use crate::{
    definition::{
        base_node::{DEFAULT_PATH_SEPARATOR, Kind, Node, undefined_method},
        exception::{Class, Error},
        processor::Processor,
    },
    php::{self, Msg},
};

pub const CLASS: &str = "Symfony\\Component\\Config\\Definition\\ArrayNode";

/// `ArrayNode`'s properties (`PrototypedArrayNode` has them too).
#[derive(Clone)]
pub(crate) struct ArrayData {
    pub(crate) xml_remappings: Vec<(PhpString, PhpString)>,
    pub(crate) children: Vec<(Key, Node)>,
    pub(crate) allow_false: bool,
    pub(crate) allow_new_keys: bool,
    pub(crate) add_if_not_set: bool,
    pub(crate) perform_deep_merging: bool,
    pub(crate) ignore_extra_keys: bool,
    pub(crate) remove_extra_keys: bool,
    pub(crate) normalize_keys: bool,
}

impl Default for ArrayData {
    fn default() -> Self {
        Self {
            xml_remappings: Vec::new(),
            children: Vec::new(),
            allow_false: false,
            allow_new_keys: true,
            add_if_not_set: false,
            perform_deep_merging: true,
            ignore_extra_keys: false,
            remove_extra_keys: true,
            normalize_keys: true,
        }
    }
}

impl ArrayData {
    pub(crate) fn child(&self, key: &Key) -> Option<&Node> {
        self.children.iter().find(|(k, _)| k == key).map(|(_, n)| n)
    }
}

/// `new ArrayNode($name, $parent, $pathSeparator)`.
pub struct ArrayNode;

impl ArrayNode {
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
            Kind::Array(ArrayData::default()),
        )
    }
}

impl Node {
    /// Changes the `ArrayNode` properties of an `ArrayNode` or a
    /// `PrototypedArrayNode`, or fails as calling `method` on another class.
    pub(crate) fn with_array_data<R>(
        &self,
        method: &str,
        f: impl FnOnce(&mut ArrayData) -> R,
    ) -> Result<R, Error> {
        let result = self.write(|n| match &mut n.kind {
            Kind::Array(data) | Kind::Prototyped(data, _) => Some(f(data)),
            _ => None,
        });
        result.ok_or_else(|| undefined_method(&self.class(), method))
    }

    pub fn set_normalize_keys(&self, normalize_keys: bool) -> Result<(), Error> {
        self.with_array_data("setNormalizeKeys", |d| d.normalize_keys = normalize_keys)
    }

    /// `getChildren()`: the children by name, in the order they were added.
    pub fn get_children(&self) -> Result<Vec<(Key, Node)>, Error> {
        self.with_array_data("getChildren", |d| d.children.clone())
    }

    /// `setXmlRemappings()`: `[singular, plural]` pairs.
    pub fn set_xml_remappings(&self, remappings: Vec<(PhpString, PhpString)>) -> Result<(), Error> {
        self.with_array_data("setXmlRemappings", |d| d.xml_remappings = remappings)
    }

    pub fn get_xml_remappings(&self) -> Result<Vec<(PhpString, PhpString)>, Error> {
        self.with_array_data("getXmlRemappings", |d| d.xml_remappings.clone())
    }

    pub fn set_add_if_not_set(&self, add: bool) -> Result<(), Error> {
        self.with_array_data("setAddIfNotSet", |d| d.add_if_not_set = add)
    }

    pub fn set_allow_false(&self, allow: bool) -> Result<(), Error> {
        self.with_array_data("setAllowFalse", |d| d.allow_false = allow)
    }

    pub fn set_allow_new_keys(&self, allow: bool) -> Result<(), Error> {
        self.with_array_data("setAllowNewKeys", |d| d.allow_new_keys = allow)
    }

    pub fn set_perform_deep_merging(&self, deep: bool) -> Result<(), Error> {
        self.with_array_data("setPerformDeepMerging", |d| d.perform_deep_merging = deep)
    }

    pub fn should_perform_deep_merging(&self) -> Result<bool, Error> {
        self.with_array_data("shouldPerformDeepMerging", |d| d.perform_deep_merging)
    }

    /// `setIgnoreExtraKeys($boolean, $remove = true)`.
    pub fn set_ignore_extra_keys(&self, ignore: bool, remove: bool) -> Result<(), Error> {
        self.with_array_data("setIgnoreExtraKeys", |d| {
            d.ignore_extra_keys = ignore;
            d.remove_extra_keys = ignore && remove;
        })
    }

    pub fn should_ignore_extra_keys(&self) -> Result<bool, Error> {
        self.with_array_data("shouldIgnoreExtraKeys", |d| d.ignore_extra_keys)
    }

    /// `addChild()`. A `PrototypedArrayNode` has no concrete children.
    pub fn add_child(&self, node: &Node) -> Result<(), Error> {
        match self.kind() {
            Kind::Array(_) => {}
            Kind::Prototyped(..) => {
                return Err(Error::new(
                    Class::Exception,
                    "A prototyped array node cannot have concrete children.",
                ));
            }
            _ => return Err(undefined_method(&self.class(), "addChild")),
        }
        let name = node.get_name();
        if name.is_empty() {
            return Err(Error::new(
                Class::InvalidArgument,
                "Child nodes must be named.",
            ));
        }
        let key = Key::from(name.clone());
        let node = node.clone();
        let added = self.with_array_data("addChild", |d| {
            if d.child(&key).is_some() {
                return false;
            }
            d.children.push((key, node));
            true
        })?;
        if !added {
            return Err(Error::new(
                Class::InvalidArgument,
                Msg::new("A child node named \"")
                    .b(&name)
                    .s("\" already exists."),
            ));
        }
        Ok(())
    }
}

/// `preNormalize()`: "foo-bar" becomes "`foo_bar`" when the key has no
/// underscore and "`foo_bar`" is not a key already.
pub(crate) fn pre_normalize(data: &ArrayData, value: Value) -> Value {
    let Value::Array(array) = &value else {
        return value;
    };
    if !data.normalize_keys {
        return value;
    }
    let mut normalized = PhpArray::new();
    for (k, v) in array.iter() {
        let text = k.bytes();
        if text.contains(&b'-') && !text.contains(&b'_') {
            let renamed: Vec<u8> = text
                .iter()
                .map(|&b| if b == b'-' { b'_' } else { b })
                .collect();
            let renamed = Key::from_bytes(&renamed);
            if !array.contains_key(&renamed) {
                normalized.insert(renamed, v.clone());
                continue;
            }
        }
        normalized.insert(k.clone(), v.clone());
    }
    Value::Array(normalized)
}

/// `ArrayNode`'s `getDefaultValue()`.
pub(crate) fn get_default_value(node: &Node, data: &ArrayData) -> Result<Value, Error> {
    if !data.add_if_not_set {
        return Err(Error::new(
            Class::Runtime,
            Msg::new("The node at path \"")
                .b(&node.get_path())
                .s("\" has no default value."),
        ));
    }
    let mut defaults = PhpArray::new();
    for (name, child) in &data.children {
        if child.has_default_value() {
            defaults.insert(name.clone(), child.get_default_value()?);
        }
    }
    Ok(Value::Array(defaults))
}

fn unset_key_for_false(node: &Node) -> Error {
    Error::new(
        Class::UnsetKey,
        Msg::new("Unsetting key for path \"")
            .b(&node.get_path())
            .s("\", value: false."),
    )
}

/// The array a validated value is, for the code paths where upstream
/// works on it as an array.
pub(crate) fn expect_array(value: Value, function: &str) -> Result<PhpArray, Error> {
    match value {
        Value::Array(a) => Ok(a),
        other => Err(Error::new(
            Class::TypeError,
            format!(
                "{function}(): Argument #1 ($array) must be of type array, {} given",
                other.debug_type()
            ),
        )),
    }
}

/// `ArrayNode`'s `finalizeValue()`.
pub(crate) fn finalize_value(node: &Node, data: &ArrayData, value: Value) -> Result<Value, Error> {
    if matches!(value, Value::Bool(false)) {
        return Err(unset_key_for_false(node));
    }
    let mut value = expect_array(value, "array_key_exists")?;
    for (name, child) in &data.children {
        let Some(current) = value.get(name).cloned() else {
            if child.is_required() {
                let mut message = Msg::new("The child config \"")
                    .b(&name.bytes())
                    .s("\" under \"")
                    .b(&node.get_path())
                    .s("\" must be configured");
                message = match child.hint() {
                    Some(info) => message.s(": ").b(&info),
                    None => message.s("."),
                };
                return Err(node.config_error(Class::InvalidConfiguration, message, false));
            }
            if child.has_default_value() {
                value.insert(name.clone(), child.get_default_value()?);
            }
            continue;
        };
        if child.is_deprecated() {
            child.trigger_child_deprecation(&php::key_bytes(name), &node.get_path())?;
        }
        match child.finalize(current) {
            Ok(v) => value.insert(name.clone(), v),
            Err(e) if *e.class() == Class::UnsetKey => {
                value.remove(name);
            }
            Err(e) => return Err(e),
        }
    }
    Ok(Value::Array(value))
}

/// `ArrayNode`'s `validateType()`: an array, or `false` when allowed.
pub(crate) fn validate_type(node: &Node, data: &ArrayData, value: &Value) -> Result<(), Error> {
    match value {
        Value::Array(_) => Ok(()),
        Value::Bool(false) if data.allow_false => Ok(()),
        _ => Err(node.config_error(
            Class::InvalidType,
            Msg::new("Invalid type for path \"")
                .b(&node.get_path())
                .s("\". Expected \"array\", but got \"")
                .s(&value.debug_type())
                .s("\""),
            true,
        )),
    }
}

/// `remapXml()`: each singular key present (and not null) is replaced by
/// its plural, holding `Processor::normalizeConfig()`'s list.
pub(crate) fn remap_xml(data: &ArrayData, mut value: PhpArray) -> Result<PhpArray, Error> {
    for (singular, plural) in &data.xml_remappings {
        let key = Key::from(singular.clone());
        if value.get(&key).is_none_or(Value::is_null) {
            continue;
        }
        let list = Processor::normalize_config(&value, singular, Some(plural))?;
        value.insert(Key::from(plural.clone()), Value::Array(list));
        value.remove(&key);
    }
    Ok(value)
}

/// `ArrayNode`'s `normalizeValue()`.
pub(crate) fn normalize_value(node: &Node, data: &ArrayData, value: Value) -> Result<Value, Error> {
    if matches!(value, Value::Bool(false)) {
        return Ok(value);
    }
    let mut value = remap_xml(data, expect_array(value, "foreach")?)?;
    let mut normalized = PhpArray::new();
    for (name, val) in value.clone().iter() {
        if let Some(child) = data.child(name) {
            match child.normalize(val.clone()) {
                Ok(v) => normalized.insert(name.clone(), v),
                Err(e) if *e.class() == Class::UnsetKey => {}
                Err(e) => return Err(e),
            }
            value.remove(name);
        } else if !data.remove_extra_keys {
            normalized.insert(name.clone(), val.clone());
        }
    }
    if !value.is_empty() && !data.ignore_extra_keys {
        return Err(unrecognized_options(node, data, &value));
    }
    Ok(Value::Array(normalized))
}

/// The "Unrecognized option" error, with guesses at distance below 3 or
/// the list of available options.
fn unrecognized_options(node: &Node, data: &ArrayData, extra: &PhpArray) -> Error {
    let mut proposals: Vec<Key> = data.children.iter().map(|(k, _)| k.clone()).collect();
    php::sort_keys(&mut proposals);
    // $guesses[$proposal] = $distance, in insertion order.
    let mut guesses: Vec<(Key, usize)> = Vec::new();
    for subject in extra.keys() {
        let subject = subject.bytes();
        let mut min_score = usize::MAX;
        for proposal in &proposals {
            let distance = php::levenshtein(&subject, &proposal.bytes());
            if distance <= min_score && distance < 3 {
                match guesses.iter_mut().find(|(k, _)| k == proposal) {
                    Some(entry) => entry.1 = distance,
                    None => guesses.push((proposal.clone(), distance)),
                }
                min_score = distance;
            }
        }
    }
    let extra_names = php::implode(b", ", extra.keys().map(php::key_bytes));
    let mut message = Msg::new("Unrecognized option")
        .s(if extra.len() == 1 { "" } else { "s" })
        .s(" \"")
        .b(&extra_names)
        .s("\" under \"")
        .b(&node.get_path())
        .s("\"");
    if guesses.is_empty() {
        let one = proposals.len() == 1;
        message = message
            .s(". Available option")
            .s(if one { "" } else { "s" })
            .s(" ")
            .s(if one { "is" } else { "are" })
            .s(" \"")
            .b(&php::implode(
                b"\", \"",
                proposals.iter().map(php::key_bytes),
            ))
            .s("\".");
    } else {
        // asort(): by distance, stable.
        guesses.sort_by_key(|(_, distance)| *distance);
        message = message
            .s(". Did you mean \"")
            .b(&php::implode(
                b"\", \"",
                guesses.iter().map(|(k, _)| php::key_bytes(k)),
            ))
            .s("\"?");
    }
    node.config_error(Class::InvalidConfiguration, message, false)
}

/// `ArrayNode`'s `mergeValues()`.
pub(crate) fn merge_values(
    node: &Node,
    data: &ArrayData,
    left: Value,
    right: Value,
) -> Result<Value, Error> {
    if matches!(right, Value::Bool(false)) {
        // Still false after the last config: finalization removes the key.
        return Ok(right);
    }
    if matches!(left, Value::Bool(false)) || !data.perform_deep_merging {
        return Ok(right);
    }
    let mut left = expect_array(left, "array_key_exists")?;
    let mut right = expect_array(right, "foreach")?;
    if php::truthy(&node.get_attribute(b"auto_enable", Value::Null))
        && !left.contains_key(&Key::from("enabled"))
        && !right.contains_key(&Key::from("enabled"))
    {
        right.insert("enabled", Value::Bool(true));
    }
    for (k, v) in right.iter() {
        if !left.contains_key(k) {
            if !data.allow_new_keys {
                return Err(node.config_error(
                    Class::InvalidConfiguration,
                    Msg::new("You are not allowed to define new elements for path \"")
                        .b(&node.get_path())
                        .s("\". Please define all elements for this path in one config file. If you are trying to overwrite an element, make sure you redefine it with the same name."),
                    false,
                ));
            }
            match data.child(k) {
                // So that the child's merge() handles auto_enable.
                Some(child) if matches!(v, Value::Array(_)) && child.is_array_node() => {
                    let merged = child.merge(Value::Array(PhpArray::new()), v.clone())?;
                    left.insert(k.clone(), merged);
                }
                _ => left.insert(k.clone(), v.clone()),
            }
            continue;
        }
        let Some(child) = data.child(k) else {
            if !data.ignore_extra_keys || data.remove_extra_keys {
                return Err(Error::new(
                    Class::Runtime,
                    "merge() expects a normalized config array.",
                ));
            }
            left.insert(k.clone(), v.clone());
            continue;
        };
        let current = left.get(k).cloned().unwrap_or_default();
        let merged = child.merge(current, v.clone())?;
        left.insert(k.clone(), merged);
    }
    Ok(Value::Array(left))
}
