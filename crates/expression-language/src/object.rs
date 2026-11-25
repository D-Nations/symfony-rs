//! Objects with methods, for `a.b()` in evaluated expressions.
//!
//! PHP calls any public method of an object; php-value's objects have
//! properties only. A [`MethodObject`] is an object whose class,
//! properties and methods the application gives: a method is a Rust
//! function of its arguments. Method names are case-insensitive, as in
//! PHP.
//!
//! Upstream: none

use std::{collections::HashMap, fmt, sync::Arc};

use php_value::{PhpArray, PhpObject, PhpString, Value};

use crate::syntax_error::Error;

/// A method: its arguments, its return value or what it throws.
pub type Method = Arc<dyn Fn(&[Value]) -> Result<Value, Error> + Send + Sync>;

/// An object with a class, public properties and methods.
#[derive(Clone)]
pub struct MethodObject {
    class: String,
    properties: PhpArray,
    methods: HashMap<Vec<u8>, Method>,
    to_string: Option<PhpString>,
}

impl MethodObject {
    pub fn new(class: impl Into<String>) -> Self {
        Self {
            class: class.into(),
            properties: PhpArray::new(),
            methods: HashMap::new(),
            to_string: None,
        }
    }

    /// Adds a public property.
    #[must_use]
    pub fn property(mut self, name: &str, value: impl Into<Value>) -> Self {
        self.properties.insert(name, value.into());
        self
    }

    /// Adds a public method.
    #[must_use]
    pub fn method(
        mut self,
        name: &str,
        f: impl Fn(&[Value]) -> Result<Value, Error> + Send + Sync + 'static,
    ) -> Self {
        self.methods
            .insert(name.as_bytes().to_ascii_lowercase(), Arc::new(f));
        self
    }

    /// Makes the object `\Stringable`.
    #[must_use]
    pub fn stringable(mut self, text: impl Into<PhpString>) -> Self {
        self.to_string = Some(text.into());
        self
    }

    /// The object as a value.
    pub fn into_value(self) -> Value {
        Value::Object(Arc::new(self))
    }

    /// `is_callable([$object, $name])`.
    pub fn has_method(&self, name: &[u8]) -> bool {
        self.methods.contains_key(&name.to_ascii_lowercase())
    }

    /// `$object->$name(...$arguments)`, `None` when there is no such
    /// method.
    pub fn call(&self, name: &[u8], arguments: &[Value]) -> Option<Result<Value, Error>> {
        self.methods
            .get(&name.to_ascii_lowercase())
            .map(|method| method(arguments))
    }
}

impl fmt::Debug for MethodObject {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("MethodObject")
            .field("class", &self.class)
            .field("properties", &self.properties)
            .finish_non_exhaustive()
    }
}

impl PhpObject for MethodObject {
    fn class(&self) -> &str {
        &self.class
    }

    fn properties(&self) -> PhpArray {
        self.properties.clone()
    }

    fn to_php_string(&self) -> Option<PhpString> {
        self.to_string.clone()
    }
}
