//! Port of Symfony\Component\DependencyInjection\Argument\IteratorArgument.php
//! and TaggedIteratorArgument.php.
//!
//! A `TaggedIteratorArgument` is an [`IteratorArgument`] with tag data
//! ([`IteratorArgument::tagged`]); its class name is
//! `TaggedIteratorArgument`, and it is still `instanceof IteratorArgument`.
//! Upstream's constructor takes two shapes (the 8.1 one, and the
//! deprecated one with default index and priority methods, told apart by
//! its argument types); they are [`IteratorArgument::tagged`] and
//! [`IteratorArgument::tagged_legacy`].
//!
//! Upstream: symfony/dependency-injection Argument/IteratorArgument.php Argument/TaggedIteratorArgument.php

use php_value::{PhpArray, Value};
use symfony_deprecation_contracts::trigger_deprecation;

use crate::object::{ObjectData, Shared};

const ITERATOR: &str = "Symfony\\Component\\DependencyInjection\\Argument\\IteratorArgument";
const TAGGED_ITERATOR: &str =
    "Symfony\\Component\\DependencyInjection\\Argument\\TaggedIteratorArgument";

/// The fields `TaggedIteratorArgument` adds.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TaggedData {
    pub tag: String,
    pub index_attribute: Option<String>,
    pub default_index_method: Option<String>,
    pub default_priority_method: Option<String>,
    pub needs_indexes: bool,
    pub exclude: Vec<String>,
    pub exclude_self: bool,
}

#[derive(Debug, Clone, Default)]
pub struct IteratorArgumentData {
    values: PhpArray,
    tagged: Option<Box<TaggedData>>,
}

impl ObjectData for IteratorArgumentData {
    fn class(&self) -> &'static str {
        if self.tagged.is_some() {
            TAGGED_ITERATOR
        } else {
            ITERATOR
        }
    }

    fn properties(&self) -> PhpArray {
        let mut properties = PhpArray::new();
        properties.insert("values", Value::Array(self.values.clone()));
        if let Some(t) = &self.tagged {
            let opt = |s: &Option<String>| s.as_deref().map_or(Value::Null, Value::from);
            properties.insert("tag", Value::from(t.tag.as_str()));
            properties.insert("indexAttribute", opt(&t.index_attribute));
            properties.insert("defaultIndexMethod", opt(&t.default_index_method));
            properties.insert("defaultPriorityMethod", opt(&t.default_priority_method));
            properties.insert("needsIndexes", Value::Bool(t.needs_indexes));
            properties.insert(
                "exclude",
                Value::Array(PhpArray::list(
                    t.exclude.iter().map(|e| Value::from(e.as_str())),
                )),
            );
            properties.insert("excludeSelf", Value::Bool(t.exclude_self));
        }
        properties
    }
}

/// PHP truthiness of an optional string.
fn truthy(s: Option<&str>) -> bool {
    matches!(s, Some(s) if !s.is_empty() && s != "0")
}

/// `'getDefault'.str_replace(' ', '', ucwords(preg_replace('/[^a-zA-Z0-9\x7f-\xff]++/', ' ', $attribute))).$suffix`.
fn default_method(attribute: &str, suffix: &str) -> String {
    let mut words = Vec::new();
    let mut in_run = false;
    for &b in attribute.as_bytes() {
        if b.is_ascii_alphanumeric() || b >= 0x7f {
            words.push(b);
            in_run = false;
        } else if !in_run {
            words.push(b' ');
            in_run = true;
        }
    }
    let mut out = b"getDefault".to_vec();
    let mut start = true;
    for b in words {
        if b == b' ' {
            start = true;
            continue;
        }
        out.push(if start { b.to_ascii_uppercase() } else { b });
        start = false;
    }
    out.extend_from_slice(suffix.as_bytes());
    // Every byte comes from a UTF-8 string, with ASCII letters changed.
    String::from_utf8_lossy(&out).into_owned()
}

/// `preg_match('/[^.]++$/', $tag, $m) ? $m[0] : $tag`.
fn last_segment(tag: &str) -> String {
    let segment = tag.rsplit('.').next().unwrap_or(tag);
    if segment.is_empty() {
        tag.to_string()
    } else {
        segment.to_string()
    }
}

/// `IteratorArgument` (and `TaggedIteratorArgument`): a lazy iterator of
/// values.
#[derive(Debug, Clone)]
pub struct IteratorArgument(Shared<IteratorArgumentData>);

impl IteratorArgument {
    pub const CLASS: &'static str = ITERATOR;
    pub const TAGGED_CLASS: &'static str = TAGGED_ITERATOR;

    /// `new IteratorArgument($values)`.
    pub fn new(values: PhpArray) -> Self {
        Self(Shared::new(IteratorArgumentData {
            values,
            tagged: None,
        }))
    }

    /// `new TaggedIteratorArgument($tag, $indexAttribute, $needsIndexes,
    /// $exclude, $excludeSelf)`.
    pub fn tagged(
        tag: &str,
        index_attribute: Option<&str>,
        needs_indexes: bool,
        exclude: Vec<String>,
        exclude_self: bool,
    ) -> Self {
        Self::build(
            tag,
            index_attribute,
            None,
            needs_indexes,
            None,
            exclude,
            exclude_self,
        )
    }

    /// The deprecated constructor shape: `new TaggedIteratorArgument($tag,
    /// $indexAttribute, $defaultIndexMethod, $needsIndexes,
    /// $defaultPriorityMethod, $exclude, $excludeSelf)`, which triggers a
    /// deprecation.
    pub fn tagged_legacy(
        tag: &str,
        index_attribute: Option<&str>,
        default_index_method: Option<&str>,
        needs_indexes: bool,
        default_priority_method: Option<&str>,
        exclude: Vec<String>,
        exclude_self: bool,
    ) -> Self {
        trigger_deprecation(
            b"symfony/dependency-injection",
            b"8.1",
            format!("The $defaultIndexMethod and $defaultPriorityMethod arguments of tagged locators and iterators are deprecated, use the #[AsTaggedItem] attribute instead. (tag: \"{tag}\")").as_bytes(),
        );
        Self::build(
            tag,
            index_attribute,
            default_index_method,
            needs_indexes,
            default_priority_method,
            exclude,
            exclude_self,
        )
    }

    fn build(
        tag: &str,
        index_attribute: Option<&str>,
        default_index_method: Option<&str>,
        needs_indexes: bool,
        default_priority_method: Option<&str>,
        exclude: Vec<String>,
        exclude_self: bool,
    ) -> Self {
        let index_attribute = match index_attribute {
            None if needs_indexes => Some(last_segment(tag)),
            other => other.map(str::to_string),
        };
        let attribute = index_attribute.as_deref();
        let method = |given: Option<&str>, suffix: &str| {
            if truthy(given) {
                given.map(str::to_string)
            } else if truthy(attribute) {
                attribute.map(|a| default_method(a, suffix))
            } else {
                None
            }
        };
        let default_index_method = method(default_index_method, "Name");
        let default_priority_method = method(default_priority_method, "Priority");
        Self(Shared::new(IteratorArgumentData {
            values: PhpArray::new(),
            tagged: Some(Box::new(TaggedData {
                tag: tag.to_string(),
                index_attribute,
                default_index_method,
                default_priority_method,
                needs_indexes,
                exclude,
                exclude_self,
            })),
        }))
    }

    /// `getValues()`.
    pub fn values(&self) -> PhpArray {
        self.0.read(|a| a.values.clone())
    }

    /// `setValues($values)`.
    pub fn set_values(&self, values: PhpArray) {
        self.0.write(|a| a.values = values);
    }

    /// `$argument instanceof TaggedIteratorArgument`.
    pub fn is_tagged(&self) -> bool {
        self.0.read(|a| a.tagged.is_some())
    }

    /// The tag data of a `TaggedIteratorArgument`.
    pub fn tagged_data(&self) -> Option<TaggedData> {
        self.0.read(|a| a.tagged.as_deref().cloned())
    }

    /// `getTag()`.
    pub fn tag(&self) -> Option<String> {
        self.0.read(|a| a.tagged.as_ref().map(|t| t.tag.clone()))
    }

    /// `getIndexAttribute()`.
    pub fn index_attribute(&self) -> Option<String> {
        self.0
            .read(|a| a.tagged.as_ref().and_then(|t| t.index_attribute.clone()))
    }

    /// `getDefaultIndexMethod($triggerDeprecation)`.
    pub fn default_index_method(&self, trigger: bool) -> Option<String> {
        if trigger {
            trigger_deprecation(
                b"symfony/dependency-injection",
                b"8.1",
                b"The \"Symfony\\Component\\DependencyInjection\\Argument\\TaggedIteratorArgument::getDefaultIndexMethod()\" method is deprecated, use the #[AsTaggedItem] attribute instead of default methods.",
            );
        }
        self.0.read(|a| {
            a.tagged
                .as_ref()
                .and_then(|t| t.default_index_method.clone())
        })
    }

    /// `getDefaultPriorityMethod($triggerDeprecation)`.
    pub fn default_priority_method(&self, trigger: bool) -> Option<String> {
        if trigger {
            trigger_deprecation(
                b"symfony/dependency-injection",
                b"8.1",
                b"The \"Symfony\\Component\\DependencyInjection\\Argument\\TaggedIteratorArgument::getDefaultPriorityMethod()\" method is deprecated, use the #[AsTaggedItem] attribute instead of default methods.",
            );
        }
        self.0.read(|a| {
            a.tagged
                .as_ref()
                .and_then(|t| t.default_priority_method.clone())
        })
    }

    /// `needsIndexes()`.
    pub fn needs_indexes(&self) -> bool {
        self.0
            .read(|a| a.tagged.as_ref().is_some_and(|t| t.needs_indexes))
    }

    /// `getExclude()`.
    pub fn exclude(&self) -> Vec<String> {
        self.0
            .read(|a| a.tagged.as_ref().map(|t| t.exclude.clone()))
            .unwrap_or_default()
    }

    /// `excludeSelf()`.
    pub fn exclude_self(&self) -> bool {
        self.0
            .read(|a| a.tagged.as_ref().is_none_or(|t| t.exclude_self))
    }

    pub fn ptr_eq(&self, other: &Self) -> bool {
        self.0.ptr_eq(&other.0)
    }

    /// `clone $argument`.
    #[must_use]
    pub fn clone_object(&self) -> Self {
        Self(self.0.clone_object())
    }

    pub fn to_value(&self) -> Value {
        self.0.to_value()
    }

    pub fn from_value(value: &Value) -> Option<Self> {
        Shared::from_value(value).map(Self)
    }
}

impl From<IteratorArgument> for Value {
    fn from(argument: IteratorArgument) -> Self {
        argument.to_value()
    }
}
