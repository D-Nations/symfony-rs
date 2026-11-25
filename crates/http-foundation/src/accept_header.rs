//! Port of Symfony\Component\HttpFoundation\AcceptHeader.php and
//! AcceptHeaderItem.php: an `Accept-*` header as items sorted by quality,
//! and the lookup of a value by RFC 9110's media-range rules.
//!
//! Items are kept under their canonical key (value, then the attributes
//! other than `q`, lower-cased and sorted) as PHP array keys, so two items
//! that differ only in `q` are one item, the later one, in the earlier's
//! place. Attribute values are strings: `setAttribute()` takes a string,
//! so `HeaderUtils::combine()`'s `true` for a bare attribute becomes `"1"`,
//! as PHP's coercion makes it. `filter()` runs its pattern on php-pcre.
//!
//! Symfony source copyright (c) Fabien Potencier; distributed under MIT.
//!
//! Upstream: symfony/http-foundation AcceptHeader.php AcceptHeaderItem.php

use std::cmp::Ordering;

use php_value::numeric::{float_to_string, string_to_float};
use php_value::{Key, PhpArray, PhpString, Value};

use crate::exception::{Class, Error, Result};
use crate::header_utils::{combine, split, to_string};

/// PHP's `<=>` on two floats: -1, 0 or 1, and 1 when either is NAN.
fn spaceship(a: f64, b: f64) -> Ordering {
    a.partial_cmp(&b).unwrap_or(Ordering::Greater)
}

fn key_bytes(key: &Key) -> Vec<u8> {
    match key {
        Key::Int(i) => i.to_string().into_bytes(),
        Key::Str(s) => s.as_bytes().to_vec(),
    }
}

/// `AcceptHeaderItem`: one value of the header, its quality, its position
/// in the header and its other attributes.
#[derive(Debug, Clone, PartialEq)]
pub struct AcceptHeaderItem {
    value: Vec<u8>,
    quality: f64,
    index: i64,
    attributes: PhpArray,
}

impl AcceptHeaderItem {
    /// `new AcceptHeaderItem($value, $attributes)`: each attribute is set
    /// with `setAttribute()`, its value coerced to a string as PHP's
    /// coercive mode does (`true` is `"1"`); a value that cannot be a
    /// string is PHP's `TypeError` (without PHP's "called in" suffix).
    pub fn new(value: impl Into<Vec<u8>>, attributes: &PhpArray) -> Result<Self> {
        let mut item = Self {
            value: value.into(),
            quality: 1.0,
            index: 0,
            attributes: PhpArray::new(),
        };
        for (name, value) in attributes.iter() {
            let text = match value {
                Value::Null | Value::Array(_) | Value::Object(_) => {
                    return Err(Error::new(
                        Class::PhpTypeError,
                        format!(
                            "Symfony\\Component\\HttpFoundation\\AcceptHeaderItem::setAttribute(): Argument #2 ($value) must be of type string, {} given",
                            value.debug_type()
                        ),
                    ));
                }
                other => other.to_php_string().unwrap_or_default(),
            };
            item.set_attribute(&key_bytes(name), text.as_bytes());
        }
        Ok(item)
    }

    /// `AcceptHeaderItem::fromString($itemValue)`: an empty item has no
    /// value, which is PHP's `TypeError` for a null value.
    pub fn from_string(item_value: Option<&[u8]>) -> Result<Self> {
        let parts = split(item_value.unwrap_or_default(), b";=")?;
        let mut parts = parts.values();
        let Some(Value::Array(part)) = parts.next() else {
            return Err(Error::new(
                Class::PhpTypeError,
                "Symfony\\Component\\HttpFoundation\\AcceptHeaderItem::__construct(): Argument #1 ($value) must be of type string, null given",
            ));
        };
        let value = first_string(part);
        let rest = PhpArray::list(parts.cloned());
        Self::new(value, &combine(&rest)?)
    }

    /// `(string) $item`: the value, `;q=` when the quality is below 1, and
    /// `; ` with the attributes (`HeaderUtils::toString($attributes, ';')`).
    pub fn to_php_string(&self) -> Result<PhpString> {
        let mut out = self.value.clone();
        if self.quality < 1.0 {
            out.extend_from_slice(b";q=");
            out.extend(float_to_string(self.quality).into_bytes());
        }
        if !self.attributes.is_empty() {
            out.extend_from_slice(b"; ");
            out.extend_from_slice(to_string(&self.attributes, b";")?.as_bytes());
        }
        Ok(PhpString::from(out))
    }

    pub fn set_value(&mut self, value: impl Into<Vec<u8>>) -> &mut Self {
        self.value = value.into();
        self
    }

    pub fn value(&self) -> &[u8] {
        &self.value
    }

    pub fn set_quality(&mut self, quality: f64) -> &mut Self {
        self.quality = quality;
        self
    }

    pub fn quality(&self) -> f64 {
        self.quality
    }

    pub fn set_index(&mut self, index: i64) -> &mut Self {
        self.index = index;
        self
    }

    pub fn index(&self) -> i64 {
        self.index
    }

    /// `hasAttribute($name)`.
    pub fn has_attribute(&self, name: &[u8]) -> bool {
        self.attributes.contains_key(&Key::from_bytes(name))
    }

    /// `getAttribute($name)`; PHP's `$default` is the caller's.
    pub fn attribute(&self, name: &[u8]) -> Option<&Value> {
        self.attributes.get(&Key::from_bytes(name))
    }

    /// `getAttributes()`.
    pub fn attributes(&self) -> &PhpArray {
        &self.attributes
    }

    /// `setAttribute($name, $value)`: `q` sets the quality (`(float)`
    /// cast) and is not kept as an attribute.
    pub fn set_attribute(&mut self, name: &[u8], value: &[u8]) -> &mut Self {
        if name == b"q" {
            self.quality = string_to_float(value);
        } else {
            self.attributes
                .insert(Key::from_bytes(name), Value::String(PhpString::from(value)));
        }
        self
    }
}

/// `$part[0]` of a `split()` group: its first string.
fn first_string(part: &PhpArray) -> Vec<u8> {
    match part.first() {
        Some((_, value)) => value
            .to_php_string()
            .map(|s| s.as_bytes().to_vec())
            .unwrap_or_default(),
        None => Vec::new(),
    }
}

/// `AcceptHeader`: the items of an `Accept-*` header.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct AcceptHeader {
    items: Vec<(Key, AcceptHeaderItem)>,
    sorted: bool,
}

impl AcceptHeader {
    /// `new AcceptHeader($items)`.
    pub fn new(items: impl IntoIterator<Item = AcceptHeaderItem>) -> Self {
        let mut header = Self {
            items: Vec::new(),
            sorted: true,
        };
        for item in items {
            header.add(item);
        }
        header
    }

    /// `AcceptHeader::fromString($headerValue)`: each comma-separated item,
    /// its index its position among the non-empty items.
    pub fn from_string(header_value: Option<&[u8]>) -> Self {
        let mut items = Vec::new();
        // split() and combine() fail only for an empty separator list or a
        // part that is not an array, neither of which `,;=` produces.
        if let Ok(groups) = split(header_value.unwrap_or_default(), b",;=") {
            for (i, parts) in groups.values().enumerate() {
                let Value::Array(parts) = parts else {
                    continue;
                };
                let mut parts = parts.values();
                let value = match parts.next() {
                    Some(Value::Array(part)) => first_string(part),
                    _ => continue,
                };
                let Ok(attributes) = combine(&PhpArray::list(parts.cloned())) else {
                    continue;
                };
                let Ok(mut item) = AcceptHeaderItem::new(value, &attributes) else {
                    continue;
                };
                item.set_index(i64::try_from(i).unwrap_or(i64::MAX));
                items.push(item);
            }
        }
        Self::new(items)
    }

    /// `(string) $header`: the items in their current order, joined by `,`.
    pub fn to_php_string(&self) -> Result<PhpString> {
        let mut parts = Vec::new();
        for (_, item) in &self.items {
            parts.push(item.to_php_string()?.as_bytes().to_vec());
        }
        Ok(PhpString::from(parts.join(&b',')))
    }

    /// `has($value)`: whether an item has `value`'s canonical key.
    pub fn has(&self, value: &[u8]) -> Result<bool> {
        let key = canonical_key(&AcceptHeaderItem::from_string(Some(value))?);
        Ok(self.items.iter().any(|(k, _)| *k == key))
    }

    /// `get($value)`: the item with `value`'s canonical key, else the
    /// matching range that is most specific, then of highest quality, then
    /// first in the header.
    pub fn get(&self, value: &[u8]) -> Result<Option<&AcceptHeaderItem>> {
        let mut query = value.to_vec();
        query.extend_from_slice(b";q=1");
        let query = AcceptHeaderItem::from_string(Some(&query))?;
        let key = canonical_key(&query);
        if let Some((_, item)) = self.items.iter().find(|(k, _)| *k == key) {
            return Ok(Some(item));
        }
        let mut candidates: Vec<&AcceptHeaderItem> = self
            .items
            .iter()
            .map(|(_, item)| item)
            .filter(|item| matches(item, &query))
            .collect();
        candidates.sort_by(|a, b| {
            specificity(b, &query)
                .cmp(&specificity(a, &query))
                .then_with(|| spaceship(b.quality, a.quality))
                .then_with(|| a.index.cmp(&b.index))
        });
        Ok(candidates.first().copied())
    }

    /// `add($item)`: stored under its canonical key, replacing an item
    /// with the same key in its place.
    pub fn add(&mut self, item: AcceptHeaderItem) -> &mut Self {
        let key = canonical_key(&item);
        match self.items.iter_mut().find(|(k, _)| *k == key) {
            Some((_, existing)) => *existing = item,
            None => self.items.push((key, item)),
        }
        self.sorted = false;
        self
    }

    /// `all()`: the items under their keys, best first (sorting them).
    pub fn all(&mut self) -> &[(Key, AcceptHeaderItem)] {
        self.sort();
        &self.items
    }

    /// `filter($pattern)`: a new header with the items whose value matches
    /// `pattern` (`preg_match()`). A pattern PHP rejects matches nothing,
    /// as `preg_match()`'s `false` does (its warning is not reproduced); a
    /// pattern php-pcre does not translate is refused.
    pub fn filter(&self, pattern: &[u8]) -> Result<Self> {
        let regex = match php_pcre::Pcre::new(pattern) {
            Ok(regex) => Some(regex),
            Err(php_pcre::PcreError::Compile(_)) => None,
            Err(e @ php_pcre::PcreError::Unsupported(_)) => {
                return Err(Error::new(Class::NotPorted, e.to_string()));
            }
        };
        let items = self.items.iter().filter_map(|(_, item)| {
            let matched = regex
                .as_ref()
                .is_some_and(|regex| regex.is_match(&item.value).unwrap_or(false));
            matched.then(|| item.clone())
        });
        Ok(Self::new(items))
    }

    /// `first()`: the best item.
    pub fn first(&mut self) -> Option<&AcceptHeaderItem> {
        self.sort();
        self.items.first().map(|(_, item)| item)
    }

    /// Sorts by descending quality, then ascending index (`uasort()` is
    /// stable).
    fn sort(&mut self) {
        if !self.sorted {
            self.items.sort_by(|(_, a), (_, b)| {
                spaceship(b.quality, a.quality).then_with(|| a.index.cmp(&b.index))
            });
            self.sorted = true;
        }
    }
}

/// `getMediaParams($item)`: the attributes with lower-cased names
/// (`array_change_key_case()`), without `q`.
fn media_params(item: &AcceptHeaderItem) -> PhpArray {
    let mut params = PhpArray::new();
    for (name, value) in item.attributes.iter() {
        let name = match name {
            Key::Str(s) => Key::Str(PhpString::from(s.as_bytes().to_ascii_lowercase())),
            int @ Key::Int(_) => int.clone(),
        };
        params.insert(name, value.clone());
    }
    params.remove(&Key::from_bytes(b"q"));
    params
}

/// `getCanonicalKey($item)`: the value, then `;name=value` for each
/// attribute in key order, a value holding whitespace, `;`, `,` or `=`
/// quoted with `"` and `\` escaped.
fn canonical_key(item: &AcceptHeaderItem) -> Key {
    let mut params = media_params(item);
    params.ksort();
    let mut key = item.value.clone();
    for (name, value) in params.iter() {
        key.push(b';');
        key.extend(key_bytes(name));
        key.push(b'=');
        let value = value
            .to_php_string()
            .map(|s| s.as_bytes().to_vec())
            .unwrap_or_default();
        if value.iter().any(|b| {
            matches!(
                b,
                b' ' | b'\t' | b'\n' | 0x0b | 0x0c | b'\r' | b';' | b',' | b'='
            )
        }) {
            key.push(b'"');
            for byte in value {
                if matches!(byte, b'"' | b'\\') {
                    key.push(b'\\');
                }
                key.push(byte);
            }
            key.push(b'"');
        } else {
            key.extend(value);
        }
    }
    Key::from_bytes(&key)
}

/// `explode('/', $value, 2) + [1 => '*']`.
fn type_and_subtype(value: &[u8]) -> (&[u8], &[u8]) {
    match value.iter().position(|&b| b == b'/') {
        Some(at) => (&value[..at], &value[at + 1..]),
        None => (value, b"*"),
    }
}

/// `matches($rangeItem, $queryItem)`.
fn matches(range: &AcceptHeaderItem, query: &AcceptHeaderItem) -> bool {
    let range_value = range.value.to_ascii_lowercase();
    let query_value = query.value.to_ascii_lowercase();
    if range_value == b"*" || range_value == b"*/*" {
        return range_parameters_match(range, query);
    }
    if query_value == b"*" {
        return false;
    }
    let is_query_media = query_value.contains(&b'/');
    let is_range_media = range_value.contains(&b'/');
    if is_query_media != is_range_media {
        return false;
    }
    if !is_query_media {
        return range_value == query_value && range_parameters_match(range, query);
    }
    let (query_type, query_subtype) = type_and_subtype(&query_value);
    let (range_type, range_subtype) = type_and_subtype(&range_value);
    if range_type != b"*" && range_type != query_type {
        return false;
    }
    if range_subtype != b"*" && range_subtype != query_subtype {
        return false;
    }
    range_parameters_match(range, query)
}

/// `rangeParametersMatch($rangeItem, $queryItem)`: every parameter of the
/// range is in the query with the same value, ignoring case.
fn range_parameters_match(range: &AcceptHeaderItem, query: &AcceptHeaderItem) -> bool {
    let query_params = media_params(query);
    for (name, range_value) in media_params(range).iter() {
        let Some(query_value) = query_params.get(name) else {
            return false;
        };
        let lower = |value: &Value| {
            value
                .to_php_string()
                .map(|s| s.as_bytes().to_ascii_lowercase())
        };
        if lower(query_value) != lower(range_value) {
            return false;
        }
    }
    true
}

/// `getSpecificity($item, $queryItem)`: media precision plus the number of
/// parameters.
fn specificity(item: &AcceptHeaderItem, query: &AcceptHeaderItem) -> i64 {
    let range_value = item.value.to_ascii_lowercase();
    let query_value = query.value.to_ascii_lowercase();
    let param_count = i64::try_from(media_params(item).len()).unwrap_or(i64::MAX);
    if !query_value.contains(&b'/') && !range_value.contains(&b'/') {
        return if range_value == b"*" { 1000 } else { 2000 } + param_count;
    }
    let (range_type, range_subtype) = type_and_subtype(&range_value);
    let base = if range_subtype != b"*" {
        3000
    } else if range_type != b"*" {
        2000
    } else {
        1000
    };
    base + param_count
}
