//! `serialize()` and `unserialize()`, written from PHP 8.4.27's recorded
//! behaviour (tests/data/php.json).
//!
//! Unserializing is limited to an allow-list, exactly as PHP's
//! `['allowed_classes' => [...]]` option limits it: an object of a class
//! that is not listed becomes `__PHP_Incomplete_Class` (its class name in
//! `__PHP_Incomplete_Class_Name`, then its properties). Enums are not
//! subject to the list: a case resolves if the enum is defined (in the
//! constants registry), else the call fails. Class names compare
//! case-insensitively. Object property names serialize as strings, even
//! numeric ones (`s:1:"0"`). Failure is `None` (PHP's `false`); data after the
//! first complete value is ignored, as PHP ignores it (with a warning).
//!
//! Upstream: none

use std::{collections::HashMap, sync::Arc};

use crate::{
    array::{Key, PhpArray},
    constants::Registry,
    numeric::serialize_float,
    object::{EnumCase, GenericObject, StdClass},
    string::PhpString,
    value::{Object, PhpObject, Value},
};

/// Builds an object of an allowed class from its unserialized properties.
pub type Constructor = Arc<dyn Fn(PhpArray) -> Object + Send + Sync>;

/// The classes `unserialize()` may create, and the enums it may resolve.
#[derive(Clone, Default)]
pub struct Classes {
    allowed: HashMap<String, (String, Build)>,
}

#[derive(Clone)]
enum Build {
    /// Declared properties with their defaults, which the unserialized
    /// ones override (declared first, then any others in stream order).
    Defaults(PhpArray),
    Constructor(Constructor),
}

impl Classes {
    pub fn new() -> Self {
        Self::default()
    }

    /// Allows a class without declared properties (`stdClass` becomes a
    /// `StdClass`).
    pub fn allow(&mut self, class: &str) -> &mut Self {
        self.allow_with_defaults(class, PhpArray::new())
    }

    /// Allows a class whose declared properties have these defaults, as
    /// PHP fills them before applying the unserialized ones.
    pub fn allow_with_defaults(&mut self, class: &str, defaults: PhpArray) -> &mut Self {
        self.allowed.insert(
            class.to_ascii_lowercase(),
            (class.to_owned(), Build::Defaults(defaults)),
        );
        self
    }

    /// Allows a class with its own constructor.
    pub fn allow_with(&mut self, class: &str, constructor: Constructor) -> &mut Self {
        self.allowed.insert(
            class.to_ascii_lowercase(),
            (class.to_owned(), Build::Constructor(constructor)),
        );
        self
    }

    fn get(&self, class: &[u8]) -> Option<&(String, Build)> {
        let lower = String::from_utf8_lossy(class).to_ascii_lowercase();
        self.allowed.get(&lower)
    }
}

impl std::fmt::Debug for Classes {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_set()
            .entries(self.allowed.values().map(|(c, _)| c))
            .finish()
    }
}

fn string(out: &mut Vec<u8>, bytes: &[u8]) {
    out.extend_from_slice(format!("s:{}:\"", bytes.len()).as_bytes());
    out.extend_from_slice(bytes);
    out.extend_from_slice(b"\";");
}

fn key(out: &mut Vec<u8>, key: &Key) {
    match key {
        Key::Int(i) => out.extend_from_slice(format!("i:{i};").as_bytes()),
        Key::Str(s) => string(out, s),
    }
}

fn pairs(out: &mut Vec<u8>, array: &PhpArray, property_names: bool) {
    out.extend_from_slice(format!("{}:{{", array.len()).as_bytes());
    for (k, v) in array.iter() {
        match k {
            // Property tables are keyed by strings.
            Key::Int(i) if property_names => string(out, i.to_string().as_bytes()),
            k => key(out, k),
        }
        write(out, v);
    }
    out.push(b'}');
}

fn write(out: &mut Vec<u8>, value: &Value) {
    match value {
        Value::Null => out.extend_from_slice(b"N;"),
        Value::Bool(b) => out.extend_from_slice(if *b { b"b:1;" } else { b"b:0;" }),
        Value::Int(i) => out.extend_from_slice(format!("i:{i};").as_bytes()),
        Value::Float(f) => out.extend_from_slice(format!("d:{};", serialize_float(*f)).as_bytes()),
        Value::String(s) => string(out, s),
        Value::Array(a) => {
            out.extend_from_slice(b"a:");
            pairs(out, a, false);
        }
        Value::Object(o) => {
            if let Some(case) = o.downcast_ref::<EnumCase>() {
                let name = format!("{}:{}", case.class(), case.name());
                out.extend_from_slice(format!("E:{}:\"{name}\";", name.len()).as_bytes());
                return;
            }
            let class = o.class();
            out.extend_from_slice(format!("O:{}:\"{class}\":", class.len()).as_bytes());
            pairs(out, &o.properties(), true);
        }
    }
}

/// `serialize($value)`.
pub fn serialize(value: &Value) -> PhpString {
    let mut out = Vec::new();
    write(&mut out, value);
    PhpString::from(out)
}

struct Reader<'a> {
    data: &'a [u8],
    pos: usize,
    classes: &'a Classes,
    constants: &'a Registry,
    /// Every value parsed so far, by `r:`/`R:` number (from 1).
    slots: Vec<Value>,
}

impl Reader<'_> {
    fn expect(&mut self, bytes: &[u8]) -> Option<()> {
        self.data[self.pos..]
            .starts_with(bytes)
            .then(|| self.pos += bytes.len())
    }

    /// Digits with an optional sign, up to `end`; the text and its int
    /// value (saturated).
    fn int_until(&mut self, end: u8) -> Option<i64> {
        let start = self.pos;
        let mut i = start;
        if matches!(self.data.get(i), Some(b'+' | b'-')) {
            i += 1;
        }
        let digits = self.data[i..]
            .iter()
            .take_while(|b| b.is_ascii_digit())
            .count();
        if digits == 0 || self.data.get(i + digits) != Some(&end) {
            return None;
        }
        self.pos = i + digits + 1;
        Some(crate::numeric::string_to_int(&self.data[start..i + digits]))
    }

    fn length(&mut self, end: u8) -> Option<usize> {
        usize::try_from(self.int_until(end)?).ok()
    }

    fn quoted(&mut self, len: usize) -> Option<&[u8]> {
        self.expect(b"\"")?;
        let bytes = self.data.get(self.pos..self.pos + len)?;
        self.pos += len;
        self.expect(b"\"")?;
        Some(bytes)
    }

    fn escaped(&mut self, len: usize) -> Option<Vec<u8>> {
        self.expect(b"\"")?;
        let mut out = Vec::with_capacity(len);
        while out.len() < len {
            let b = *self.data.get(self.pos)?;
            if b == b'\\' {
                let hex = self.data.get(self.pos + 1..self.pos + 3)?;
                let text = std::str::from_utf8(hex).ok()?;
                out.push(u8::from_str_radix(text, 16).ok()?);
                self.pos += 3;
            } else {
                out.push(b);
                self.pos += 1;
            }
        }
        self.expect(b"\"")?;
        Some(out)
    }

    fn key(&mut self) -> Option<Key> {
        let tag = *self.data.get(self.pos)?;
        self.pos += 1;
        self.expect(b":")?;
        match tag {
            b'i' => Some(Key::Int(self.int_until(b';')?)),
            b's' => {
                let len = self.length(b':')?;
                let bytes = self.quoted(len)?.to_vec();
                self.expect(b";")?;
                Some(Key::from_bytes(&bytes))
            }
            b'S' => {
                let len = self.length(b':')?;
                let bytes = self.escaped(len)?;
                self.expect(b";")?;
                Some(Key::from_bytes(&bytes))
            }
            _ => None,
        }
    }

    fn members(&mut self) -> Option<PhpArray> {
        let count = self.length(b':')?;
        self.expect(b"{")?;
        let mut array = PhpArray::new();
        for _ in 0..count {
            let key = self.key()?;
            let value = self.value()?;
            array.insert(key, value);
        }
        self.expect(b"}")?;
        Some(array)
    }

    fn value(&mut self) -> Option<Value> {
        let slot = self.slots.len();
        self.slots.push(Value::Null);
        let tag = *self.data.get(self.pos)?;
        self.pos += 1;
        let value = if tag == b'N' {
            self.expect(b";")?;
            Value::Null
        } else {
            self.expect(b":")?;
            match tag {
                b'b' => {
                    let v = match self.data.get(self.pos)? {
                        b'0' => false,
                        b'1' => true,
                        _ => return None,
                    };
                    self.pos += 1;
                    self.expect(b";")?;
                    Value::Bool(v)
                }
                b'i' => Value::Int(self.int_until(b';')?),
                b'd' => {
                    let end = self.data[self.pos..].iter().position(|&b| b == b';')?;
                    let text = &self.data[self.pos..self.pos + end];
                    let f = match text {
                        b"INF" => f64::INFINITY,
                        b"-INF" => f64::NEG_INFINITY,
                        b"NAN" => f64::NAN,
                        _ if crate::numeric::is_numeric(text)
                            && !text.first().is_some_and(u8::is_ascii_whitespace)
                            && !text.last().is_some_and(u8::is_ascii_whitespace) =>
                        {
                            crate::numeric::string_to_float(text)
                        }
                        _ => return None,
                    };
                    self.pos += end + 1;
                    Value::Float(f)
                }
                b's' => {
                    let len = self.length(b':')?;
                    let bytes = self.quoted(len)?.to_vec();
                    self.expect(b";")?;
                    Value::String(PhpString::from(bytes))
                }
                b'S' => {
                    let len = self.length(b':')?;
                    let bytes = self.escaped(len)?;
                    self.expect(b";")?;
                    Value::String(PhpString::from(bytes))
                }
                b'a' => Value::Array(self.members()?),
                b'O' | b'C' => {
                    let len = self.length(b':')?;
                    let class = self.quoted(len)?.to_vec();
                    self.expect(b":")?;
                    let properties = if tag == b'O' {
                        self.members()?
                    } else {
                        // C: custom serialization; only incomplete objects.
                        let len = self.length(b':')?;
                        self.expect(b"{")?;
                        self.data.get(self.pos..self.pos + len)?;
                        self.pos += len;
                        self.expect(b"}")?;
                        if self.classes.get(&class).is_some() {
                            return None;
                        }
                        PhpArray::new()
                    };
                    self.object(&class, properties)
                }
                b'E' => {
                    let len = self.length(b':')?;
                    let name = self.quoted(len)?.to_vec();
                    self.expect(b";")?;
                    let text = std::str::from_utf8(&name).ok()?;
                    let (class, case) = text.split_once(':')?;
                    if !self.constants.enum_exists(class) {
                        return None;
                    }
                    match self.constants.constant(&format!("{class}::{case}"))? {
                        Value::Object(o) if o.downcast_ref::<EnumCase>().is_some() => {
                            Value::Object(o.clone())
                        }
                        _ => return None,
                    }
                }
                b'r' | b'R' => {
                    let n = usize::try_from(self.int_until(b';')?).ok()?;
                    // The reference itself takes no number.
                    self.slots.pop();
                    let target = self.slots.get(n.checked_sub(1)?)?.clone();
                    if tag == b'r' && !matches!(target, Value::Object(_)) {
                        return None;
                    }
                    return Some(target);
                }
                _ => return None,
            }
        };
        self.slots[slot] = value.clone();
        Some(value)
    }

    fn object(&self, class: &[u8], properties: PhpArray) -> Value {
        match self.classes.get(class) {
            Some((_, Build::Constructor(constructor))) => Value::Object(constructor(properties)),
            Some((canonical, _)) if canonical.eq_ignore_ascii_case(StdClass::CLASS) => {
                StdClass::object(properties)
            }
            Some((canonical, Build::Defaults(defaults))) => {
                let mut object = defaults.clone();
                for (k, v) in properties.iter() {
                    object.insert(k.clone(), v.clone());
                }
                Value::Object(Arc::new(GenericObject::new(canonical.clone(), object)))
            }
            None => {
                let mut incomplete = PhpArray::new();
                incomplete.insert(
                    "__PHP_Incomplete_Class_Name",
                    Value::String(PhpString::from(class)),
                );
                incomplete.union(&properties);
                Value::Object(Arc::new(GenericObject::new(
                    "__PHP_Incomplete_Class",
                    incomplete,
                )))
            }
        }
    }
}

/// `unserialize($data, ['allowed_classes' => $classes])`; `None` where
/// PHP returns `false`. Allowed enums are looked up in `constants`.
pub fn unserialize(data: &[u8], classes: &Classes, constants: &Registry) -> Option<Value> {
    let mut reader = Reader {
        data,
        pos: 0,
        classes,
        constants,
        slots: Vec::new(),
    };
    reader.value()
}
