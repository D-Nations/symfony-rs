//! Port of Symfony\Component\DependencyInjection\Argument\AbstractArgument.php
//! (with ArgumentTrait.php's `__serialize()` left out: the port does not
//! serialize definitions).
//!
//! Upstream: symfony/dependency-injection Argument/AbstractArgument.php Argument/ArgumentTrait.php

use php_value::{PhpArray, Value};

use crate::object::{ObjectData, Shared};

#[derive(Debug, Clone, Default)]
pub struct AbstractArgumentData {
    text: String,
    context: String,
}

impl ObjectData for AbstractArgumentData {
    fn class(&self) -> &'static str {
        AbstractArgument::CLASS
    }

    fn properties(&self) -> PhpArray {
        let mut properties = PhpArray::new();
        properties.insert("text", Value::from(self.text.as_str()));
        properties.insert("context", Value::from(self.context.as_str()));
        properties
    }
}

/// `AbstractArgument`: an argument that a compiler pass must replace.
#[derive(Debug, Clone)]
pub struct AbstractArgument(Shared<AbstractArgumentData>);

impl AbstractArgument {
    pub const CLASS: &'static str =
        "Symfony\\Component\\DependencyInjection\\Argument\\AbstractArgument";

    /// `new AbstractArgument($text)`: the text without leading and
    /// trailing dots and spaces.
    pub fn new(text: &str) -> Self {
        Self(Shared::new(AbstractArgumentData {
            text: text.trim_matches(['.', ' ']).to_string(),
            context: String::new(),
        }))
    }

    /// `setContext($context)`.
    pub fn set_context(&self, context: &str) {
        self.0.write(|a| {
            a.context = format!(
                "{context} is abstract{}",
                if a.text.is_empty() { "" } else { ": " }
            );
        });
    }

    /// `getText()`.
    pub fn text(&self) -> String {
        self.0.read(|a| a.text.clone())
    }

    /// `getTextWithContext()`.
    pub fn text_with_context(&self) -> String {
        self.0.read(|a| format!("{}{}.", a.context, a.text))
    }

    pub fn ptr_eq(&self, other: &Self) -> bool {
        self.0.ptr_eq(&other.0)
    }

    pub fn to_value(&self) -> Value {
        self.0.to_value()
    }

    pub fn from_value(value: &Value) -> Option<Self> {
        Shared::from_value(value).map(Self)
    }
}

impl From<AbstractArgument> for Value {
    fn from(argument: AbstractArgument) -> Self {
        argument.to_value()
    }
}
