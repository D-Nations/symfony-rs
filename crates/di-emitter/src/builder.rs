//! Reads a compiled `ContainerBuilder` of the Rust DI port directly: its
//! definitions, aliases, parameters and argument objects.
//!
//! The reading is `XmlDumper`'s (`Dumper/XmlDumper.php`) followed by the
//! XML reader (`crate::xml`), without the text in between: what the dump
//! writes as a type the XML reader refuses is refused here with the same
//! message, and the graph equals the one read from the container's dump
//! (tests/equivalence.rs). Two things differ, both on purpose:
//!
//! - a tagged iterator is read as an iterator of the services it
//!   resolved to, which is what `oracle/recorders/di_container.php`
//!   writes for the XML reader (`XmlDumper` writes only the tag);
//! - scalars keep their type where the XML text loses it (a float
//!   without a fractional part, a string `"NULL"` or `"True"`, which
//!   `XmlUtils::phpize()` reads as an int, null or a bool).
//!
//! A refusal is located by service id (or parameter name): the builder
//! has no lines.
//!
//! Upstream: none

use php_value::object::EnumCase;
use php_value::{Key, PhpArray, PhpString, Value};
use symfony_dependency_injection::argument::{
    AbstractArgument, EnvClosureArgument, IteratorArgument, ServiceClosureArgument,
    ServiceLocatorArgument,
};
use symfony_dependency_injection::dumper::XmlDumper;
use symfony_dependency_injection::object::downcast;
use symfony_dependency_injection::{
    ContainerBuilder, Definition as DiDefinition, InvalidBehavior, Parameter, Reference,
};

use crate::model::{
    Alias, ArgDef, CallDef, Definition, DumperParameters, FactoryDef, Graph, OnInvalid, Origin,
    Parameters, escape, string_arg,
};
use crate::{Error, Location};

/// `Symfony\Component\ExpressionLanguage\Expression`.
const EXPRESSION: &str = "Symfony\\Component\\ExpressionLanguage\\Expression";

struct Reader<'a> {
    cb: &'a ContainerBuilder,
    graph: Graph,
    parameters: Parameters,
    inline_count: usize,
}

pub fn read(cb: &ContainerBuilder, dumper_parameters: &DumperParameters) -> Result<Graph, Error> {
    if !cb.is_compiled() {
        return Err(Error::new(
            Location::Unknown,
            "the container is not compiled: the emitter reads the graph after the compiler passes",
        ));
    }
    let mut r = Reader {
        cb,
        graph: Graph::default(),
        parameters: dumper_parameters.0.iter().cloned().collect(),
        inline_count: 0,
    };
    r.graph
        .parameters
        .extend(dumper_parameters.0.iter().cloned());
    for (name, value) in cb.get_parameter_bag().all().iter() {
        let name = String::from_utf8_lossy(&name.bytes()).into_owned();
        let at = Location::Parameter(name.clone());
        // XmlDumper escapes `%` first and writes env names afterwards.
        let escaped = r.parameter_value(&escape(value), &at)?;
        let value = r.parameter_value(value, &at)?;
        r.parameters.insert(name.clone(), escaped);
        r.graph.parameters.push((name, value));
    }
    for (id, definition) in cb.get_definitions().iter() {
        let id = r.text(id);
        r.service(definition, Some(id.clone()), &id)?;
    }
    let aliases = cb.get_aliases();
    for (id, alias) in aliases.iter() {
        // XmlDumper writes the last alias of a chain, followed once round.
        let mut alias = alias;
        let mut steps = 0;
        while let Some(next) = aliases.get(&alias.id()) {
            if steps > aliases.len() {
                break;
            }
            steps += 1;
            alias = next;
        }
        let id = r.text(id);
        r.graph.aliases.push(Alias {
            target: r.text(&alias.id()),
            public: alias.is_public(),
            origin: Origin(Location::Service(id.clone())),
            id,
        });
    }
    Ok(r.graph)
}

fn on_invalid(behavior: InvalidBehavior) -> OnInvalid {
    match behavior {
        InvalidBehavior::Null => OnInvalid::Null,
        InvalidBehavior::Ignore => OnInvalid::Ignore,
        InvalidBehavior::IgnoreUninitialized => OnInvalid::IgnoreUninitialized,
        // XmlDumper writes no on-invalid for these.
        InvalidBehavior::Exception | InvalidBehavior::RuntimeException => OnInvalid::Exception,
    }
}

/// `preg_match('/^[^\x00-\x08\x0B\x0C\x0E-\x1F\x7F]*+$/u', $value)`, the
/// strings `XmlDumper` writes as text rather than `type="binary"`.
fn is_xml_text(bytes: &[u8]) -> bool {
    std::str::from_utf8(bytes).is_ok()
        && !bytes
            .iter()
            .any(|&b| matches!(b, 0x00..=0x08 | 0x0B | 0x0C | 0x0E..=0x1F | 0x7F))
}

/// The `type` `XmlDumper` writes for a value the XML reader refuses
/// wherever it is (parameter or argument), or `None`.
fn refused_type(value: &Value) -> Option<&'static str> {
    if EnvClosureArgument::from_value(value).is_some() {
        return Some("env_closure");
    }
    if let Some(locator) = ServiceLocatorArgument::from_value(value) {
        return Some(if locator.tagged_iterator_argument().is_some() {
            "tagged_locator"
        } else {
            "service_locator"
        });
    }
    if value.as_object().is_some_and(|o| o.class() == EXPRESSION) {
        return Some("expression");
    }
    if let Value::String(s) = value
        && !is_xml_text(s.as_bytes())
    {
        return Some("binary");
    }
    if AbstractArgument::from_value(value).is_some() {
        return Some("abstract");
    }
    None
}

/// What `phpToXml()` refuses: an object that is none of the component's.
fn object_error(value: &Value) -> String {
    format!(
        "Unable to dump a service container if a parameter is an object or a resource, got \"{}\".",
        value.debug_type()
    )
}

/// PHP truthiness of a class name.
fn truthy(s: &str) -> bool {
    !s.is_empty() && s != "0"
}

/// `$callable[0]` and `$callable[1]` of a factory or configurator array.
fn callable_parts(callable: &PhpArray) -> (Value, String) {
    let first = callable.get(&Key::Int(0)).cloned().unwrap_or_default();
    let method = callable.get(&Key::Int(1)).map(to_text).unwrap_or_default();
    (first, method)
}

/// `(string) $value` as the dumpers write it: a `Reference` as its id.
fn to_text(value: &Value) -> String {
    if let Some(reference) = Reference::from_value(value) {
        return reference.id().to_owned();
    }
    let s = match value {
        Value::Array(_) => PhpString::from("Array"),
        other => other.to_php_string_or_stringable().unwrap_or_default(),
    };
    String::from_utf8_lossy(s.as_bytes()).into_owned()
}

/// `['Closure', 'fromCallable'] === $callable`.
fn is_closure_from_callable(callable: &PhpArray) -> bool {
    callable.len() == 2
        && matches!(callable.get(&Key::Int(0)), Some(Value::String(s)) if s.as_bytes() == b"Closure")
        && matches!(callable.get(&Key::Int(1)), Some(Value::String(s)) if s.as_bytes() == b"fromCallable")
        && callable.keys().zip([0, 1]).all(|(k, i)| *k == Key::Int(i))
}

impl Reader<'_> {
    /// A string as the dump shows it: env placeholders as `%env(NAME)%`.
    fn text(&self, s: &str) -> String {
        let named = self.cb.env_placeholders_to_names(&PhpString::from(s));
        String::from_utf8_lossy(named.as_bytes()).into_owned()
    }

    fn key(&self, key: &Key) -> Key {
        match key {
            Key::Int(_) => key.clone(),
            Key::Str(s) => Key::from_bytes(self.cb.env_placeholders_to_names(s).as_bytes()),
        }
    }

    /// A parameter, refused where the XML reader refuses its type.
    fn parameter_value(&self, value: &Value, at: &Location) -> Result<Value, Error> {
        if let Some(type_) = refused_type(value) {
            return Err(Error::new(
                at.clone(),
                format!("unsupported parameter type \"{type_}\""),
            ));
        }
        Ok(match value {
            Value::Array(array) => {
                let mut out = PhpArray::new();
                for (k, v) in array.iter() {
                    out.insert(self.key(k), self.parameter_value(v, at)?);
                }
                Value::Array(out)
            }
            Value::String(s) => Value::String(self.cb.env_placeholders_to_names(s)),
            Value::Object(_) => {
                if let Some(parameter) = Parameter::from_value(value) {
                    Value::string(format!("%{}%", parameter.id()))
                } else {
                    let type_ = if downcast::<EnumCase>(value).is_some() {
                        "constant"
                    } else if IteratorArgument::from_value(value).is_some() {
                        "iterator"
                    } else if ServiceClosureArgument::from_value(value).is_some() {
                        "service_closure"
                    } else if Reference::from_value(value).is_some()
                        || DiDefinition::from_value(value).is_some()
                    {
                        "service"
                    } else {
                        return Err(Error::new(at.clone(), object_error(value)));
                    };
                    return Err(Error::new(
                        at.clone(),
                        format!("unsupported parameter type \"{type_}\""),
                    ));
                }
            }
            other => other.clone(),
        })
    }

    fn keyed(
        &mut self,
        values: &PhpArray,
        owner: &str,
    ) -> Result<Vec<(Option<Key>, ArgDef)>, Error> {
        let with_keys = !values.is_list();
        let mut items = Vec::new();
        for (k, v) in values.iter() {
            let key = with_keys.then(|| self.key(k));
            items.push((key, self.argument(v, owner)?));
        }
        Ok(items)
    }

    fn refuse(owner: &str, message: impl Into<String>) -> Error {
        Error::new(Location::Service(owner.to_owned()), message)
    }

    /// One argument, property or collection item of service `owner`.
    fn argument(&mut self, value: &Value, owner: &str) -> Result<ArgDef, Error> {
        if let Some(iterator) = IteratorArgument::from_value(value) {
            // A tagged iterator holds the services it resolved to.
            return Ok(ArgDef::Iterator(self.keyed(&iterator.values(), owner)?));
        }
        if let Some(type_) = refused_type(value) {
            return Err(Self::refuse(
                owner,
                format!("unsupported argument type=\"{type_}\""),
            ));
        }
        if let Value::Array(array) = value {
            return Ok(ArgDef::Collection(self.keyed(array, owner)?));
        }
        if let Some(closure) = ServiceClosureArgument::from_value(value) {
            let Some(reference) = Reference::from_value(&closure.value()).cloned() else {
                return Err(Self::refuse(
                    owner,
                    "unsupported argument type=\"service_closure\" of a value that is not a reference",
                ));
            };
            return Ok(ArgDef::Closure {
                id: self.text(reference.id()),
                on_invalid: on_invalid(reference.invalid_behavior()),
                origin: Origin(Location::Service(owner.to_owned())),
            });
        }
        if let Some(reference) = Reference::from_value(value) {
            return Ok(ArgDef::Ref {
                id: self.text(reference.id()),
                on_invalid: on_invalid(reference.invalid_behavior()),
                origin: Origin(Location::Service(owner.to_owned())),
            });
        }
        if let Some(definition) = DiDefinition::from_value(value) {
            return Ok(ArgDef::Inline(self.service(&definition, None, owner)?));
        }
        if downcast::<EnumCase>(value).is_some() {
            let text =
                XmlDumper::php_to_xml(value).map_err(|e| Self::refuse(owner, e.message()))?;
            return Ok(ArgDef::Constant(
                String::from_utf8_lossy(text.as_bytes()).into_owned(),
            ));
        }
        let text = match value {
            Value::String(s) => self.cb.env_placeholders_to_names(s),
            Value::Object(_) => match Parameter::from_value(value) {
                Some(parameter) => PhpString::from(format!("%{}%", parameter.id())),
                None => return Err(Self::refuse(owner, object_error(value))),
            },
            scalar => return Ok(ArgDef::Value(scalar.clone())),
        };
        string_arg(&String::from_utf8_lossy(text.as_bytes()), &self.parameters)
            .map_err(|m| Self::refuse(owner, m))
    }

    fn callable_service(
        &mut self,
        first: &Value,
        method: String,
        owner: &str,
    ) -> Result<Option<FactoryDef>, Error> {
        if let Some(inner) = DiDefinition::from_value(first) {
            let index = self.service(&inner, None, owner)?;
            return Ok(Some(FactoryDef::Service {
                service: ArgDef::Inline(index),
                method,
            }));
        }
        if Reference::from_value(first).is_some() {
            return Ok(Some(FactoryDef::Service {
                service: ArgDef::Ref {
                    id: self.text(&to_text(first)),
                    on_invalid: OnInvalid::Exception,
                    origin: Origin(Location::Service(owner.to_owned())),
                },
                method,
            }));
        }
        Ok(None)
    }

    /// `<factory>`, or the `constructor` attribute, as `XmlDumper` writes
    /// them and the XML reader reads them back.
    fn factory(
        &mut self,
        d: &DiDefinition,
        class: &str,
        owner: &str,
    ) -> Result<Option<FactoryDef>, Error> {
        let factory = d.factory();
        if !factory.is_truthy() {
            return Ok(None);
        }
        let Value::Array(callable) = &factory else {
            return Ok(Some(FactoryDef::Function(self.text(&to_text(&factory)))));
        };
        let (first, method) = callable_parts(callable);
        let method = self.text(&method);
        if let Some(f) = self.callable_service(&first, method.clone(), owner)? {
            return Ok(Some(f));
        }
        // The constructor attribute (`$definition->getClass() ===
        // $callable[0]`) and `<factory method>` without a class are the
        // definition's own class.
        let class = if first.is_null() || is_constructor(d, callable) {
            class.to_owned()
        } else {
            self.text(&to_text(&first))
        };
        Ok(Some(FactoryDef::Static { class, method }))
    }

    fn configurator(&mut self, d: &DiDefinition, owner: &str) -> Result<Option<FactoryDef>, Error> {
        let configurator = d.configurator();
        if !configurator.is_truthy() {
            return Ok(None);
        }
        let Value::Array(callable) = &configurator else {
            return Ok(Some(FactoryDef::Function(
                self.text(&to_text(&configurator)),
            )));
        };
        let (first, method) = callable_parts(callable);
        let method = self.text(&method);
        if let Some(f) = self.callable_service(&first, method.clone(), owner)? {
            return Ok(Some(f));
        }
        Ok(Some(FactoryDef::Static {
            class: self.text(&to_text(&first)),
            method,
        }))
    }

    /// The tag names, once per tag, with `container.error` for each error
    /// (as `XmlDumper` writes them).
    fn tags(&self, d: &DiDefinition) -> Vec<String> {
        let mut tags = d.tags();
        let errors = PhpArray::list(d.errors().into_iter().map(|e| {
            let mut error = PhpArray::new();
            error.insert("message", Value::from(e));
            Value::Array(error)
        }));
        tags.insert("container.error", Value::Array(errors));
        let mut names = Vec::new();
        for (name, list) in tags.iter() {
            let Value::Array(list) = list else { continue };
            let name = self.text(&String::from_utf8_lossy(&name.bytes()));
            for attributes in list.values() {
                if matches!(attributes, Value::Array(_)) {
                    names.push(name.clone());
                }
            }
        }
        names
    }

    /// The method calls, as `XmlDumper` writes them (`[method,
    /// arguments, returnsClone]`; anything else is skipped).
    fn calls(&mut self, d: &DiDefinition, owner: &str) -> Result<Vec<CallDef>, Error> {
        let mut calls = Vec::new();
        for call in d.method_calls().values() {
            let Value::Array(call) = call else { continue };
            let method = self.text(&call.get(&Key::Int(0)).map(to_text).unwrap_or_default());
            let returns_clone = call.get(&Key::Int(2)).is_some_and(Value::is_truthy);
            let mut arguments = Vec::new();
            if let Some(Value::Array(values)) = call.get(&Key::Int(1)) {
                for v in values.values() {
                    arguments.push(self.argument(v, owner)?);
                }
            }
            calls.push(CallDef {
                method,
                returns_clone,
                arguments,
            });
        }
        Ok(calls)
    }

    /// Reads one definition; returns its index in `Graph::services`.
    /// `owner` is the named service it is part of (itself, if named).
    fn service(
        &mut self,
        d: &DiDefinition,
        id: Option<String>,
        owner: &str,
    ) -> Result<usize, Error> {
        if let Some(decorated) = d.decorated_service() {
            return Err(Self::refuse(
                owner,
                format!(
                    "unsupported attribute decorates=\"{}\" on <service>",
                    self.text(&decorated.id)
                ),
            ));
        }
        if d.is_abstract() {
            return Err(Self::refuse(
                owner,
                "unsupported attribute abstract=\"true\" on <service>",
            ));
        }
        let inline = id.is_none();
        let id = id.unwrap_or_else(|| {
            self.inline_count += 1;
            format!(".inline.{}", self.inline_count)
        });
        let class = d
            .class()
            .filter(|c| truthy(c))
            .map(|c| self.text(c.strip_prefix('\\').unwrap_or(&c)));
        let factory_value = d.factory();
        let class = if let Some(class) = class {
            class
        } else {
            // XmlDumper writes `<factory>` (not `constructor`) only when
            // the callable's class is not the definition's.
            let has_factory = factory_value.is_truthy()
                && match &factory_value {
                    Value::Array(callable) => !is_constructor(d, callable),
                    _ => true,
                };
            if !has_factory {
                return Err(Self::refuse(
                    owner,
                    format!("service \"{id}\" has no class"),
                ));
            }
            "mixed".to_owned()
        };
        if d.file().is_some_and(|f| truthy(&f)) {
            return Err(Self::refuse(
                owner,
                "unsupported element <file> in a service",
            ));
        }
        let index = self.graph.services.len();
        self.graph.services.push(Definition {
            id: id.clone(),
            inline,
            class: class.clone(),
            origin: Origin(Location::Service(owner.to_owned())),
            public: d.is_public(),
            shared: d.is_shared(),
            synthetic: d.is_synthetic(),
            lazy: d.is_lazy(),
            factory: None,
            arguments: Vec::new(),
            calls: Vec::new(),
            properties: Vec::new(),
            configurator: None,
            tags: self.tags(d),
        });
        // XmlDumper's order: arguments, properties, calls, factory,
        // configurator (inline definitions are numbered in it).
        let mut arguments = Vec::new();
        for v in d.arguments().values() {
            arguments.push(self.argument(v, owner)?);
        }
        let mut properties = Vec::new();
        let all = d.properties();
        let with_names = !all.is_list();
        for (name, v) in all.iter() {
            let name = if with_names {
                self.text(&String::from_utf8_lossy(&name.bytes()))
            } else {
                String::new()
            };
            properties.push((name, self.argument(v, owner)?));
        }
        let calls = self.calls(d, owner)?;
        let factory = self.factory(d, &class, owner)?;
        let configurator = self.configurator(d, owner)?;
        let def = &mut self.graph.services[index];
        def.arguments = arguments;
        def.properties = properties;
        def.calls = calls;
        def.factory = factory;
        def.configurator = configurator;
        Ok(index)
    }
}

/// `$definition->getClass() === $callable[0]` for a callable that is not
/// `Closure::fromCallable`: `XmlDumper` writes the `constructor` attribute.
fn is_constructor(d: &DiDefinition, callable: &PhpArray) -> bool {
    if is_closure_from_callable(callable) {
        return false;
    }
    match (d.class(), callable.get(&Key::Int(0))) {
        (None, None | Some(Value::Null)) => true,
        (Some(class), Some(Value::String(s))) => class.as_bytes() == s.as_bytes(),
        _ => false,
    }
}
