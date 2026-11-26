//! Port of Symfony\Component\DependencyInjection\Compiler\ResolveBindingsPass.php.
//!
//! Fills the arguments of constructors and method calls from the
//! definition's bindings (`$name`, `Type $name` and `Type` keys), and
//! fails on a binding that no argument used. Parameters, their types and
//! their `#[Target]`/`#[Autowire]` attributes come from the class
//! metadata. `Target::parseName()` is ported here, as this pass calls it.
//!
//! Upstream: symfony/dependency-injection Compiler/ResolveBindingsPass.php

use std::fmt::Write as _;

use php_value::{PhpArray, Value, array::Key};

use super::abstract_recursive_pass::{
    AbstractRecursivePass, RecursivePassState, recursive_process,
};
use super::compiler_pass_interface::CompilerPassInterface;
use super::export_type::{DeclaringClass, export_type};
use crate::argument::{
    AbstractArgument, BoundArgument, DEFAULTS_BINDING, INSTANCEOF_BINDING, IteratorArgument,
    ServiceLocatorArgument,
};
use crate::class_metadata::{ClassKind, FunctionReflector, ParameterMetadata};
use crate::container_builder::ContainerBuilder;
use crate::definition::{Definition, DefinitionError};
use crate::exception::{Error, ErrorKind, Result};
use crate::id_map::IdMap;
use crate::reference::Reference;

const TARGET: &str = "Symfony\\Component\\DependencyInjection\\Attribute\\Target";
const AUTOWIRE: &str = "Symfony\\Component\\DependencyInjection\\Attribute\\Autowire";

/// An entry of `$unusedBindings`: `[$key, $serviceId, $bindingType, $file]`.
#[derive(Debug, Clone)]
struct Unused {
    key: String,
    service_id: String,
    binding_type: i64,
    file: Option<String>,
}

/// `ResolveBindingsPass`.
#[derive(Debug, Clone)]
pub struct ResolveBindingsPass {
    state: RecursivePassState,
    /// `$usedBindings`, keyed by `$bindingId ?? ''`.
    used_bindings: Vec<Key>,
    unused_bindings: Vec<(Key, Unused)>,
    error_messages: Vec<String>,
}

impl Default for ResolveBindingsPass {
    fn default() -> Self {
        Self::new()
    }
}

fn key_string(key: &Key) -> String {
    String::from_utf8_lossy(&key.bytes()).into_owned()
}

/// `$bindingId ?? ''` as an array key.
fn binding_id_key(id: &Value) -> Key {
    match id {
        Value::Int(i) => Key::Int(*i),
        Value::Null => Key::from(""),
        other => Key::from_value(other).unwrap_or_else(|| Key::from("")),
    }
}

/// `preg_match('/^(?:(?:array|bool|float|int|string|iterable|([^ $]++)) )\$/', $key, $m)`:
/// `(strlen($m[0]), $m[1] ?? null)`.
fn match_typed_key(key: &str) -> Option<(usize, Option<&str>)> {
    let space = key.find(' ')?;
    let token = &key[..space];
    if token.is_empty() || token.contains('$') || !key[space + 1..].starts_with('$') {
        return None;
    }
    let builtin = ["array", "bool", "float", "int", "string", "iterable"].contains(&token);
    Some((space + 2, (!builtin).then_some(token)))
}

/// `preg_replace('/(^|[(|&])\\\\/', '\1', $typeHint)`.
fn strip_leading_backslashes(type_hint: &str) -> String {
    let mut out = String::with_capacity(type_hint.len());
    let mut previous: Option<char> = None;
    for c in type_hint.chars() {
        if c == '\\' && matches!(previous, None | Some('(' | '|' | '&')) {
            previous = Some(c);
            continue;
        }
        out.push(c);
        previous = Some(c);
    }
    out
}

/// `Target::getParsedName()`: `lcfirst(str_replace(' ', '',
/// ucwords(preg_replace('/[^a-zA-Z0-9\x7f-\xff]++/', ' ', $name))))`.
fn parsed_name(name: &str) -> String {
    let mut out = Vec::with_capacity(name.len());
    let mut upper_next = true;
    for &b in name.as_bytes() {
        if b.is_ascii_alphanumeric() || b >= 0x7f {
            out.push(if upper_next {
                b.to_ascii_uppercase()
            } else {
                b
            });
            upper_next = false;
        } else {
            upper_next = true;
        }
    }
    if let Some(first) = out.first_mut() {
        first.make_ascii_lowercase();
    }
    String::from_utf8_lossy(&out).into_owned()
}

/// `Target::parseName($parameter, parsedName: $parsedName)`: the name and
/// the parsed name.
fn parse_target_name(parameter: &ParameterMetadata, function: &str) -> Result<(String, String)> {
    let Some(target) = parameter.attributes.iter().find(|a| {
        a.class
            .trim_start_matches('\\')
            .eq_ignore_ascii_case(TARGET)
    }) else {
        return Ok((parameter.name.clone(), parsed_name(&parameter.name)));
    };

    let name = target
        .arguments
        .get(&Key::from("name"))
        .or_else(|| target.arguments.get(&Key::Int(0)))
        .and_then(Value::to_php_string)
        .map_or_else(
            || parameter.name.clone(),
            |s| s.to_string_lossy().into_owned(),
        );
    let parsed = parsed_name(&name);

    if !parsed
        .as_bytes()
        .first()
        .is_some_and(|&b| b.is_ascii_alphabetic() || b == b'_' || b >= 0x7f)
    {
        return Err(Error::invalid_argument(format!(
            "Invalid #[Target] name \"{name}\" on parameter \"${}\" of \"{function}()\": the first character must be a letter.",
            parameter.name
        )));
    }

    let simple = !name.is_empty()
        && name
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b == b'_' || b >= 0x7f);
    Ok((if simple { name } else { parsed.clone() }, parsed))
}

/// `$arguments[$k]` is set, not `''` and not an `AbstractArgument`.
fn is_filled(arguments: &PhpArray, key: &Key) -> bool {
    arguments.get(key).is_some_and(|v| {
        !matches!(v, Value::String(s) if s.is_empty()) && AbstractArgument::from_value(v).is_none()
    })
}

/// `ksort($arguments, \SORT_NATURAL)`.
fn ksort_natural(arguments: &PhpArray) -> PhpArray {
    let mut entries: Vec<(Key, Value)> = arguments
        .iter()
        .map(|(k, v)| (k.clone(), v.clone()))
        .collect();
    entries.sort_by(|(a, _), (b, _)| php_value::string::strnatcmp(&a.bytes(), &b.bytes()));
    let mut out = PhpArray::new();
    for (k, v) in entries {
        out.insert(k, v);
    }
    out
}

/// What a call of the list names: the constructor's reflector, or a
/// method name.
enum CallMethod {
    Reflector(FunctionReflector),
    Name(String),
}

impl ResolveBindingsPass {
    pub const CLASS: &'static str =
        "Symfony\\Component\\DependencyInjection\\Compiler\\ResolveBindingsPass";

    pub fn new() -> Self {
        let mut state = RecursivePassState::default();
        state.skip_scalars = true;
        Self {
            state,
            used_bindings: Vec::new(),
            unused_bindings: Vec::new(),
            error_messages: Vec::new(),
        }
    }

    fn current_id(&self) -> String {
        self.state.current_id.clone().unwrap_or_default()
    }

    fn mark_used(&mut self, id: Key) {
        self.unused_bindings.retain(|(k, _)| *k != id);
        if !self.used_bindings.contains(&id) {
            self.used_bindings.push(id);
        }
    }

    /// `getBindingValue($binding)`.
    fn get_binding_value(&mut self, binding: &Value) -> Value {
        let Some(binding) = BoundArgument::from_value(binding) else {
            return Value::Null;
        };
        let values = binding.values();
        let id = values.get(&Key::Int(1)).cloned().unwrap_or_default();
        self.mark_used(binding_id_key(&id));
        values.get(&Key::Int(0)).cloned().unwrap_or_default()
    }

    fn walk(&mut self, container: &mut ContainerBuilder) -> Result<()> {
        recursive_process(self, container)?;

        if let Some((_, unused)) = self.unused_bindings.first() {
            let key = unused.key.as_str();
            let (argument_type, argument_name) = if let Some((t, n)) = key.split_once(' ') {
                (Some(t), Some(n))
            } else if key.starts_with('$') {
                (None, Some(key))
            } else {
                (Some(key), None)
            };
            let mut message = String::new();

            if let Some(t) = argument_type.filter(|t| !t.is_empty() && *t != "0") {
                let _ = write!(message, "of type \"{t}\" ");
            }

            if let Some(n) = argument_name.filter(|n| !n.is_empty() && *n != "0") {
                let _ = write!(message, "named \"{n}\" ");
            }

            if unused.binding_type == DEFAULTS_BINDING {
                message.push_str("under \"_defaults\"");
            } else if unused.binding_type == INSTANCEOF_BINDING {
                message.push_str("under \"_instanceof\"");
            } else {
                let _ = write!(message, "for service \"{}\"", unused.service_id);
            }

            if let Some(file) = unused
                .file
                .as_deref()
                .filter(|f| !f.is_empty() && *f != "0")
            {
                let _ = write!(message, " in file \"{file}\"");
            }

            let mut message = format!(
                "A binding is configured for an argument {message}, but no corresponding argument has been found. It may be unused and should be removed, or it may have a typo."
            );

            if !self.error_messages.is_empty() {
                let _ = write!(
                    message,
                    "\nCould be related to{}:",
                    if self.error_messages.len() > 1 {
                        " one of"
                    } else {
                        ""
                    }
                );
            }
            for m in &self.error_messages {
                message.push_str("\n - ");
                message.push_str(m);
            }
            return Err(Error::invalid_argument(message));
        }
        Ok(())
    }

    /// The body of `foreach ($calls as $i => $call)`: the call's arguments
    /// with bindings applied, or `None` to skip the call.
    #[expect(clippy::too_many_lines, reason = "one upstream loop")]
    fn bind_call(
        &mut self,
        container: &mut ContainerBuilder,
        value: &Definition,
        bindings: &PhpArray,
        binding_names: &IdMap<Value>,
        method: CallMethod,
        mut arguments: PhpArray,
    ) -> Result<Option<PhpArray>> {
        let reflection_method = match method {
            CallMethod::Reflector(r) => r,
            CallMethod::Name(name) => match self.get_reflection_method(container, value, &name) {
                Ok(r) => r,
                Err(e) if e.is(&ErrorKind::Runtime) => {
                    if value.factory().is_truthy() {
                        return Ok(None);
                    }
                    return Err(e);
                }
                Err(e) => return Err(e),
            },
        };
        let method_class = reflection_method.class().unwrap_or_default().to_string();
        let function = if reflection_method.is_method() {
            format!("{method_class}::{}", reflection_method.name())
        } else {
            reflection_method.name().to_string()
        };
        let classes = container.class_registry();
        let declaring_parent = classes.class(&method_class).and_then(|c| c.parent.clone());
        let declaring = DeclaringClass {
            name: reflection_method.class(),
            parent: declaring_parent.as_deref(),
        };

        let mut names: Vec<String> = Vec::new();

        for (index, parameter) in reflection_method.parameters().iter().enumerate() {
            let key = Key::Int(i64::try_from(index).unwrap_or(i64::MAX));
            names.push(parameter.name.clone());

            if is_filled(&arguments, &key) {
                continue;
            }
            if is_filled(&arguments, &Key::from(parameter.name.as_str())) {
                continue;
            }
            if value.is_autowired()
                && !value.has_tag("container.ignore_attributes")
                && parameter.attributes.iter().any(|a| {
                    a.class
                        .trim_start_matches('\\')
                        .eq_ignore_ascii_case(AUTOWIRE)
                        || classes.is_a(&a.class, AUTOWIRE)
                })
            {
                continue;
            }

            let type_hint = export_type(parameter, false, declaring).unwrap_or_default();
            let type_hint = type_hint.trim_start_matches('?');

            let (name, parsed) = parse_target_name(parameter, &function)?;

            if !type_hint.is_empty() {
                let stripped = strip_leading_backslashes(type_hint);
                let mut candidates = vec![
                    format!("{stripped} ${name}"),
                    format!("{stripped} ${parsed}"),
                ];
                if name != parameter.name {
                    candidates.push(format!("{stripped} ${}", parameter.name));
                }
                if let Some(k) = candidates
                    .iter()
                    .find(|k| bindings.contains_key(&Key::from(k.as_str())))
                {
                    let binding = bindings
                        .get(&Key::from(k.as_str()))
                        .cloned()
                        .unwrap_or_default();
                    arguments.insert(key, self.get_binding_value(&binding));
                    continue;
                }
            }

            let mut candidates = vec![format!("${name}"), format!("${parsed}")];
            if name != parameter.name {
                candidates.push(format!("${}", parameter.name));
            }
            if let Some(k) = candidates
                .iter()
                .find(|k| bindings.contains_key(&Key::from(k.as_str())))
            {
                let binding = bindings
                    .get(&Key::from(k.as_str()))
                    .cloned()
                    .unwrap_or_default();
                arguments.insert(key, self.get_binding_value(&binding));
                continue;
            }

            if let Some(unqualified) = type_hint.strip_prefix('\\')
                && let Some(binding) = bindings
                    .get(&Key::from(unqualified))
                    .filter(|b| !b.is_null())
                    .cloned()
            {
                arguments.insert(key, self.get_binding_value(&binding));
                continue;
            }

            let binding = binding_names
                .get(&name)
                .or_else(|| binding_names.get(&parsed))
                .or_else(|| binding_names.get(&parameter.name))
                .filter(|b| !b.is_null());
            if let Some(binding) = binding {
                let binding_key = bindings
                    .iter()
                    .find(|(_, b)| b.identical(binding))
                    .map(|(k, _)| key_string(k))
                    .unwrap_or_default();
                let argument_type = binding_key.split(' ').next().unwrap_or_default();
                self.error_messages.push(format!(
                    "Did you forget to add the type \"{argument_type}\" to argument \"${}\" of method \"{method_class}::{}()\"?",
                    parameter.name,
                    reflection_method.name()
                ));
            }
        }

        for (index, name) in names.iter().enumerate() {
            let name_key = Key::from(name.as_str());
            let key = i64::try_from(index).unwrap_or(i64::MAX);
            if arguments.contains_key(&name_key)
                && (key == 0 || arguments.contains_key(&Key::Int(key - 1)))
            {
                if !arguments.contains_key(&Key::Int(key)) {
                    let by_name = arguments.get(&name_key).cloned().unwrap_or_default();
                    arguments.insert(Key::Int(key), by_name);
                }
                arguments.remove(&name_key);
            }
        }

        Ok(Some(arguments))
    }
}

impl CompilerPassInterface for ResolveBindingsPass {
    fn class(&self) -> &str {
        Self::CLASS
    }

    fn process(&mut self, container: &mut ContainerBuilder) -> Result<()> {
        self.used_bindings = container
            .get_removed_binding_ids()
            .iter()
            .map(|id| Key::Int(*id))
            .collect();

        let result = self.walk(container);
        self.used_bindings.clear();
        self.unused_bindings.clear();
        self.error_messages.clear();
        result
    }
}

impl AbstractRecursivePass for ResolveBindingsPass {
    fn state(&mut self) -> &mut RecursivePassState {
        &mut self.state
    }

    #[expect(clippy::too_many_lines, reason = "one upstream method")]
    fn process_value(
        &mut self,
        container: &mut ContainerBuilder,
        value: Value,
        is_root: bool,
    ) -> Result<Value> {
        if let Some(reference) = Reference::from_value(&value)
            && reference.type_() == Some(reference.id())
        {
            // Already checked
            let bindings = container.get_definition(&self.current_id())?.bindings();

            if let Some(name) = reference.name() {
                let key = Key::from(format!("{} ${name}", reference.id()));
                if let Some(binding) = bindings.get(&key).filter(|b| !b.is_null()) {
                    return Ok(self.get_binding_value(&binding.clone()));
                }
            }

            if let Some(binding) = bindings
                .get(&Key::from(reference.id()))
                .filter(|b| !b.is_null())
            {
                return Ok(self.get_binding_value(&binding.clone()));
            }

            return self.parent_process_value(container, value, is_root);
        }

        let Some(definition) = Definition::from_value(&value) else {
            return self.parent_process_value(container, value, is_root);
        };
        let bindings = definition.bindings();
        if bindings.is_empty() {
            return self.parent_process_value(container, value, is_root);
        }

        let mut binding_names: IdMap<Value> = IdMap::new();

        for (key, binding) in bindings.iter() {
            let key_name = key_string(key);
            let values = BoundArgument::from_value(binding)
                .map(|b| b.values())
                .unwrap_or_default();
            let at = |i: i64| values.get(&Key::Int(i)).cloned().unwrap_or_default();
            let (binding_value, binding_id, used, binding_type, file) =
                (at(0), at(1), at(2), at(3), at(4));
            let binding_id = binding_id_key(&binding_id);
            if used.is_truthy() {
                self.mark_used(binding_id);
            } else if !self.used_bindings.contains(&binding_id) {
                let unused = Unused {
                    key: key_name.clone(),
                    service_id: self.current_id(),
                    binding_type: match binding_type {
                        Value::Int(i) => i,
                        _ => 0,
                    },
                    file: file
                        .to_php_string()
                        .filter(|_| !file.is_null())
                        .map(|s| s.to_string_lossy().into_owned()),
                };
                if let Some(entry) = self
                    .unused_bindings
                    .iter_mut()
                    .find(|(k, _)| *k == binding_id)
                {
                    entry.1 = unused;
                } else {
                    self.unused_bindings.push((binding_id, unused));
                }
            }

            let Some((matched, class)) = match_typed_key(&key_name) else {
                continue;
            };
            binding_names.insert(&key_name[matched..], binding.clone());

            let Some(class) = class else {
                continue;
            };

            if container
                .class_registry()
                .reflect(class)
                .ok()
                .flatten()
                .is_some_and(|c| c.kind == ClassKind::Enum)
            {
                binding_names.insert(&key_name[matched..], binding.clone());
                continue;
            }

            if !binding_value.is_null()
                && Reference::from_value(&binding_value).is_none()
                && Definition::from_value(&binding_value).is_none()
                && !IteratorArgument::from_value(&binding_value).is_some_and(|a| a.is_tagged())
                && ServiceLocatorArgument::from_value(&binding_value).is_none()
            {
                return Err(Error::invalid_argument(format!(
                    "Invalid value for binding key \"{key_name}\" for service \"{}\": expected \"{}\", \"{}\", \"{}\", \"{}\" or null, \"{}\" given.",
                    self.current_id(),
                    Reference::CLASS,
                    Definition::CLASS,
                    IteratorArgument::TAGGED_CLASS,
                    ServiceLocatorArgument::CLASS,
                    binding_value.debug_type()
                )));
            }
        }

        if definition.is_abstract() {
            return self.parent_process_value(container, value, is_root);
        }

        let original_calls = definition.method_calls();
        let mut calls: Vec<(Key, PhpArray, CallMethod)> = Vec::new();
        for (k, call) in original_calls.iter() {
            let call = call.as_array().cloned().unwrap_or_default();
            let method = call
                .get(&Key::Int(0))
                .and_then(Value::to_php_string)
                .map(|s| s.to_string_lossy().into_owned())
                .unwrap_or_default();
            calls.push((k.clone(), call, CallMethod::Name(method)));
        }

        let constructor = match self.get_constructor(container, &definition, false) {
            Ok(constructor) => constructor,
            Err(e) if e.is(&ErrorKind::Runtime) => {
                self.error_messages.push(e.message());
                container
                    .get_definition(&self.current_id())?
                    .add_error(DefinitionError::Text(e.message()));

                return self.parent_process_value(container, value, is_root);
            }
            Err(e) => return Err(e),
        };
        let has_constructor = constructor.is_some();
        if let Some(constructor) = constructor {
            let call = PhpArray::list([Value::Null, Value::Array(definition.arguments())]);
            calls.push((Key::Int(-1), call, CallMethod::Reflector(constructor)));
        }

        let mut new_calls: Vec<(Key, PhpArray)> = Vec::new();
        for (k, call, method) in calls {
            let arguments = call
                .get(&Key::Int(1))
                .and_then(Value::as_array)
                .cloned()
                .unwrap_or_default();
            let Some(bound) = self.bind_call(
                container,
                &definition,
                &bindings,
                &binding_names,
                method,
                arguments.clone(),
            )?
            else {
                new_calls.push((k, call));
                continue;
            };
            let mut call = call;
            if bound != arguments {
                call.insert(Key::Int(1), Value::Array(ksort_natural(&bound)));
            }
            new_calls.push((k, call));
        }

        if has_constructor && let Some((_, call)) = new_calls.pop() {
            let arguments = call
                .get(&Key::Int(1))
                .and_then(Value::as_array)
                .cloned()
                .unwrap_or_default();
            if arguments != definition.arguments() {
                definition.set_arguments(arguments);
            }
        }

        let mut calls = PhpArray::new();
        for (k, call) in new_calls {
            calls.insert(k, Value::Array(call));
        }
        if calls != original_calls {
            definition.set_method_calls(&calls)?;
        }

        self.parent_process_value(container, value, is_root)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn typed_keys() {
        assert_eq!(Some((5, Some("Foo"))), match_typed_key("Foo $bar"));
        assert_eq!(Some((8, None)), match_typed_key("string $bar"));
        assert_eq!(Some((8, Some("arrayX"))), match_typed_key("arrayX $bar"));
        assert_eq!(None, match_typed_key("$bar"));
        assert_eq!(None, match_typed_key("Foo"));
    }

    #[test]
    fn target_names() {
        assert_eq!("imageStorage", parsed_name("image.storage"));
        assert_eq!("apiKey", parsed_name("apiKey"));
        assert_eq!("fooBarBaz", parsed_name("foo_bar-baz"));
    }

    #[test]
    fn backslashes() {
        assert_eq!(
            "Foo\\Bar|Baz",
            strip_leading_backslashes("\\Foo\\Bar|\\Baz")
        );
    }
}
