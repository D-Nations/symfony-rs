//! What `ResolveDecoratorStackPass` and `ResolveInstanceofConditionalsPass`
//! use of symfony/var-exporter's `DeepCloner`: a copy of a value in which
//! every mutable object of the component (definitions, aliases, argument
//! objects) is a new object, and an object met twice is cloned once.
//! Immutable objects (`Reference`, `Parameter`, `EnvClosureArgument`,
//! ...) are shared, as cloning them changes nothing.
//!
//! An object's clone is built through the component's public API, so a
//! definition's method calls are renumbered when they hold an object
//! (where `DeepCloner` keeps a gap that `removeMethodCall()` left).
//!
//! Upstream: none (symfony/var-exporter DeepCloner.php is not a DI file)

use php_value::{PhpArray, Value};

use crate::alias::Alias;
use crate::argument::{
    AbstractArgument, BoundArgument, IteratorArgument, ServiceClosureArgument,
    ServiceLocatorArgument,
};
use crate::definition::Definition;
use crate::exception::Result;

/// `DeepCloner::deepClone($value)`.
pub(crate) fn deep_clone(value: &Value) -> Result<Value> {
    DeepCloner::default().value(value)
}

/// `(new DeepCloner($definition))->clone()`.
pub(crate) fn deep_clone_definition(definition: &Definition) -> Result<Definition> {
    let cloned = deep_clone(&definition.to_value())?;
    Ok(Definition::from_value(&cloned).unwrap_or_else(|| definition.clone_object()))
}

#[derive(Default)]
struct DeepCloner {
    /// Objects already cloned, with their clones.
    seen: Vec<(Value, Value)>,
}

fn holds_object(array: &PhpArray) -> bool {
    array.values().any(|v| match v {
        Value::Object(_) => true,
        Value::Array(a) => holds_object(a),
        _ => false,
    })
}

impl DeepCloner {
    fn array(&mut self, array: &PhpArray) -> Result<PhpArray> {
        let mut out = PhpArray::new();
        for (k, v) in array.iter() {
            out.insert(k.clone(), self.value(v)?);
        }
        Ok(out)
    }

    fn remember(&mut self, original: &Value, clone: Value) -> Value {
        self.seen.push((original.clone(), clone.clone()));
        clone
    }

    fn value(&mut self, value: &Value) -> Result<Value> {
        match value {
            Value::Array(array) => return Ok(Value::Array(self.array(array)?)),
            Value::Object(_) => {}
            _ => return Ok(value.clone()),
        }
        if let Some((_, clone)) = self.seen.iter().find(|(o, _)| o.identical(value)) {
            return Ok(clone.clone());
        }

        if let Some(definition) = Definition::from_value(value) {
            let clone = definition.clone_object();
            self.remember(value, clone.to_value());
            let changes = clone.changes();
            clone.set_arguments(self.array(&clone.arguments())?);
            clone.set_properties(self.array(&clone.properties())?);
            let calls = clone.method_calls();
            if holds_object(&calls) {
                clone.set_method_calls(&self.array(&calls)?)?;
            }
            if matches!(clone.factory(), Value::Object(_) | Value::Array(_)) {
                clone.set_factory(self.value(&clone.factory())?);
            }
            if matches!(clone.configurator(), Value::Object(_) | Value::Array(_)) {
                clone.set_configurator(self.value(&clone.configurator())?);
            }
            let bindings = clone.bindings();
            if !bindings.is_empty() {
                clone.set_bindings(&self.array(&bindings)?);
            }
            let instanceof = clone.instanceof_conditionals();
            if !instanceof.is_empty() {
                clone.set_instanceof_conditionals(self.array(&instanceof)?);
            }
            let tags = clone.tags();
            if holds_object(&tags) {
                clone.set_tags(self.array(&tags)?);
            }
            clone.set_changes(changes);
            return Ok(clone.to_value());
        }
        if let Some(alias) = Alias::from_value(value) {
            return Ok(self.remember(value, alias.clone_object().to_value()));
        }
        if let Some(argument) = IteratorArgument::from_value(value) {
            let clone = argument.clone_object();
            self.remember(value, clone.to_value());
            clone.set_values(self.array(&clone.values())?);
            return Ok(clone.to_value());
        }
        if let Some(argument) = ServiceClosureArgument::from_value(value) {
            let clone = ServiceClosureArgument::new(Value::Null);
            self.remember(value, clone.to_value());
            clone.set_values(self.array(&argument.values())?)?;
            return Ok(clone.to_value());
        }
        if let Some(argument) = ServiceLocatorArgument::from_value(value) {
            let clone = match argument.tagged_iterator_argument() {
                Some(tagged) => {
                    let tagged = self.value(&tagged.to_value())?;
                    ServiceLocatorArgument::from_tagged(
                        IteratorArgument::from_value(&tagged)
                            .unwrap_or_else(|| IteratorArgument::new(PhpArray::new())),
                    )
                }
                None => ServiceLocatorArgument::new(PhpArray::new()),
            };
            self.remember(value, clone.to_value());
            clone.set_values(self.array(&argument.values())?);
            return Ok(clone.to_value());
        }
        if let Some(argument) = BoundArgument::from_value(value) {
            let clone = BoundArgument::new(Value::Null, false, 0, None);
            self.remember(value, clone.to_value());
            clone.set_values(&self.array(&argument.values())?);
            return Ok(clone.to_value());
        }
        if let Some(argument) = AbstractArgument::from_value(value) {
            let clone = AbstractArgument::new(&argument.text());
            // The context is only ever set by `setContext()`, which
            // appends " is abstract" (and ": " before a text).
            let with_context = argument.text_with_context();
            let context_len = with_context.len().saturating_sub(argument.text().len() + 1);
            let context = &with_context[..context_len];
            let context = context
                .strip_suffix(" is abstract: ")
                .or_else(|| context.strip_suffix(" is abstract"));
            if let Some(context) = context {
                clone.set_context(context);
            }
            return Ok(self.remember(value, clone.to_value()));
        }
        Ok(value.clone())
    }
}
