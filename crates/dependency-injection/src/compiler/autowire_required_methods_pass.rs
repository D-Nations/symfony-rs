//! Port of Symfony\Component\DependencyInjection\Compiler\AutowireRequiredMethodsPass.php:
//! looks for definitions with autowiring enabled and registers their
//! corresponding `#[Required]` methods as setters.
//!
//! `getMethods()` and `getPrototype()` read the class metadata: a class's
//! methods come first, then the ones it inherits, then those of its
//! interfaces; a method's prototype is the same method in the closest
//! ancestor class, else in an interface.
//!
//! Upstream: symfony/dependency-injection Compiler/AutowireRequiredMethodsPass.php

use php_value::{PhpArray, Value, array::Key};

use super::abstract_recursive_pass::{
    AbstractRecursivePass, RecursivePassState, recursive_process,
};
use super::compiler_pass_interface::CompilerPassInterface;
use crate::class_metadata::{ClassMetadata, ClassRegistry, MethodMetadata};
use crate::container_builder::ContainerBuilder;
use crate::definition::Definition;
use crate::exception::Result;

/// `Required::class`.
pub const REQUIRED: &str = "Symfony\\Contracts\\Service\\Attribute\\Required";

/// `AutowireRequiredMethodsPass`.
#[derive(Debug, Clone)]
pub struct AutowireRequiredMethodsPass {
    state: RecursivePassState,
}

impl Default for AutowireRequiredMethodsPass {
    fn default() -> Self {
        Self::new()
    }
}

impl AutowireRequiredMethodsPass {
    pub fn new() -> Self {
        let mut state = RecursivePassState::default();
        state.skip_scalars = true;
        Self { state }
    }
}

fn same_class(a: &str, b: &str) -> bool {
    a.trim_start_matches('\\')
        .eq_ignore_ascii_case(b.trim_start_matches('\\'))
}

/// `$r->getMethods()`: each method with its declaring class.
pub fn class_methods(
    registry: &ClassRegistry,
    class: &ClassMetadata,
) -> Vec<(String, MethodMetadata)> {
    let mut methods: Vec<(String, MethodMetadata)> = Vec::new();
    let push =
        |declaring: &str, method: &MethodMetadata, methods: &mut Vec<(String, MethodMetadata)>| {
            if !methods
                .iter()
                .any(|(_, m)| m.name.eq_ignore_ascii_case(&method.name))
            {
                methods.push((declaring.to_string(), method.clone()));
            }
        };
    let mut current = Some(std::sync::Arc::new(class.clone()));
    while let Some(c) = current {
        for method in &c.methods {
            push(&c.name, method, &mut methods);
        }
        current = c.parent.as_deref().and_then(|p| registry.class(p));
    }
    for interface in registry.interface_names(class) {
        if let Some(i) = registry.class(&interface) {
            for method in &i.methods {
                push(&i.name, method, &mut methods);
            }
        }
    }
    methods
}

/// `$r->hasPrototype()` and `$r->getPrototype()`.
fn prototype(
    registry: &ClassRegistry,
    declaring: &str,
    name: &str,
) -> Option<(String, MethodMetadata)> {
    let class = registry.class(declaring)?;
    let mut current = class.parent.as_deref().and_then(|p| registry.class(p));
    while let Some(c) = current {
        if let Some(m) = c.declared_method(name) {
            return Some((c.name.clone(), m.clone()));
        }
        current = c.parent.as_deref().and_then(|p| registry.class(p));
    }
    registry
        .interface_names(&class)
        .into_iter()
        .find_map(|interface| {
            let i = registry.class(&interface)?;
            i.declared_method(name).map(|m| (i.name.clone(), m.clone()))
        })
}

/// `preg_match('#(?:^/\*\*|\n\s*+\*)\s*+@return\s++(static|\$this)[\s\*]#i', $doc)`:
/// the word after the first `@return` tag that starts a doc line.
fn doc_return(doc: &str) -> Option<&'static str> {
    let bytes = doc.as_bytes();
    let is_space = |b: u8| matches!(b, b' ' | b'\t' | b'\n' | b'\r' | 0x0b | 0x0c);
    // Candidate starts: after `^/**` or after `\n\s*\*`.
    let mut starts = Vec::new();
    if bytes.starts_with(b"/**") {
        starts.push(3);
    }
    for (i, &b) in bytes.iter().enumerate() {
        if b == b'\n' {
            let mut j = i + 1;
            while j < bytes.len() && is_space(bytes[j]) {
                j += 1;
            }
            if bytes.get(j) == Some(&b'*') {
                starts.push(j + 1);
            }
        }
    }
    starts.sort_unstable();
    for start in starts {
        let mut j = start;
        while j < bytes.len() && is_space(bytes[j]) {
            j += 1;
        }
        let rest = &bytes[j..];
        if rest.len() < 7 || !rest[..7].eq_ignore_ascii_case(b"@return") {
            continue;
        }
        let mut k = 7;
        let spaces = k;
        while k < rest.len() && is_space(rest[k]) {
            k += 1;
        }
        if k == spaces {
            continue;
        }
        let word = &rest[k..];
        for (candidate, result) in [(&b"static"[..], "static"), (&b"$this"[..], "$this")] {
            if word.len() > candidate.len()
                && word[..candidate.len()].eq_ignore_ascii_case(candidate)
                && (is_space(word[candidate.len()]) || word[candidate.len()] == b'*')
            {
                return Some(result);
            }
        }
    }
    None
}

/// `isWither($reflectionMethod, $doc)`.
fn is_wither(method: &MethodMetadata, doc: &str) -> bool {
    match doc_return(doc) {
        Some("static") => return true,
        Some(_) => return false,
        None => {}
    }
    method.return_type.as_deref() == Some("static")
}

impl CompilerPassInterface for AutowireRequiredMethodsPass {
    fn class(&self) -> &'static str {
        "Symfony\\Component\\DependencyInjection\\Compiler\\AutowireRequiredMethodsPass"
    }

    fn process(&mut self, container: &mut ContainerBuilder) -> Result<()> {
        recursive_process(self, container)
    }
}

impl AbstractRecursivePass for AutowireRequiredMethodsPass {
    fn state(&mut self) -> &mut RecursivePassState {
        &mut self.state
    }

    fn process_value(
        &mut self,
        container: &mut ContainerBuilder,
        value: Value,
        is_root: bool,
    ) -> Result<Value> {
        let value = self.parent_process_value(container, value, is_root)?;

        let Some(definition) = Definition::from_value(&value) else {
            return Ok(value);
        };
        let Some(class) = definition.class().filter(|c| !c.is_empty() && c != "0") else {
            return Ok(value);
        };
        if !definition.is_autowired() || definition.is_abstract() {
            return Ok(value);
        }
        let Some(reflection_class) = container.get_reflection_class(Some(&class), false)? else {
            return Ok(value);
        };
        let registry = container.class_registry();

        let mut already_called_methods: Vec<String> = Vec::new();
        let mut withers: Vec<Value> = Vec::new();

        for call in definition.method_calls().values() {
            if let Some(method) = call
                .as_array()
                .and_then(|c| c.get(&Key::Int(0)))
                .and_then(Value::to_php_string)
            {
                already_called_methods.push(method.to_string_lossy().to_lowercase());
            }
        }

        for (declaring, method) in class_methods(&registry, &reflection_class) {
            if method.name.eq_ignore_ascii_case("__construct")
                || already_called_methods.contains(&method.name.to_lowercase())
            {
                continue;
            }

            let mut r = (declaring, method.clone());
            loop {
                if r.1
                    .attributes
                    .iter()
                    .any(|a| same_class(&a.class, REQUIRED))
                {
                    if is_wither(&r.1, r.1.doc_comment.as_deref().unwrap_or("")) {
                        withers.push(Value::Array(PhpArray::list([
                            Value::from(r.1.name.as_str()),
                            Value::Array(PhpArray::new()),
                            Value::Bool(true),
                        ])));
                    } else {
                        definition.add_method_call(&r.1.name, PhpArray::new(), false)?;
                    }
                    break;
                }
                let Some(p) = prototype(&registry, &r.0, &r.1.name) else {
                    break;
                };
                r = p;
            }
        }

        if !withers.is_empty() {
            // Prepend withers to prevent creating circular loops
            let setters = definition.method_calls();
            definition.set_method_calls(&PhpArray::list(withers))?;
            for call in setters.values() {
                let Some(call) = call.as_array() else {
                    continue;
                };
                let method = call
                    .get(&Key::Int(0))
                    .and_then(Value::to_php_string)
                    .map(|s| s.to_string_lossy().into_owned())
                    .unwrap_or_default();
                let arguments = match call.get(&Key::Int(1)) {
                    Some(Value::Array(a)) => a.clone(),
                    _ => PhpArray::new(),
                };
                let returns_clone = call.get(&Key::Int(2)).is_some_and(Value::is_truthy);
                definition.add_method_call(&method, arguments, returns_clone)?;
            }
        }

        Ok(value)
    }
}
