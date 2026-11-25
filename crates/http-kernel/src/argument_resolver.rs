//! Port of Symfony\Component\HttpKernel\Controller\ArgumentResolver.php,
//! with ValueResolverInterface.php and Attribute/ValueResolver.php:
//! responsible for resolving the arguments passed to an action.
//!
//! Each argument is offered to the value resolvers in order; the first
//! that yields a value wins, and a `NearMissValueResolverException` is
//! kept as a reason for the final error. `#[ValueResolver]` attributes
//! pin one named resolver (with the request attribute and default
//! resolvers after it) or disable one, when the resolver was given named
//! resolvers (PHP's service container; [`NamedResolvers`]).
//!
//! A value resolver's iterable is a `Vec`: PHP's resolvers return arrays
//! (generators are read to the end before the count is checked, as
//! `foreach` does). `get_debug_type($resolver)` and `$resolver::class` are
//! [`ValueResolverInterface::class`].
//!
//! Symfony source copyright (c) Fabien Potencier; distributed under MIT.
//!
//! Upstream: symfony/http-kernel Controller/ArgumentResolver.php Controller/ValueResolverInterface.php Attribute/ValueResolver.php

use std::{collections::HashSet, sync::Arc};

use php_value::{PhpArray, Value};
use symfony_http_foundation::request_stack::RequestHandle;

use crate::{
    controller::{ArgumentResolverInterface, Controller, Parameter},
    controller_metadata::{
        ArgumentMetadata, ArgumentMetadataFactory, ArgumentMetadataFactoryInterface,
    },
    exception::{PhpClass, PhpException, Throwable, instance_of, message_of},
    value_resolver::{
        DefaultValueResolver, RequestAttributeValueResolver, RequestValueResolver,
        SessionValueResolver, VariadicValueResolver,
    },
};

/// `Symfony\Component\HttpKernel\Attribute\ValueResolver`.
pub const VALUE_RESOLVER_CLASS: &str = "Symfony\\Component\\HttpKernel\\Attribute\\ValueResolver";

const NEAR_MISS: &str = "Symfony\\Component\\HttpKernel\\Exception\\NearMissValueResolverException";

/// `ValueResolverInterface`: responsible for resolving the value of an
/// argument based on its metadata.
pub trait ValueResolverInterface: Send + Sync {
    /// The resolver's class (`$resolver::class`, `get_debug_type()`).
    fn class(&self) -> &str;

    /// `resolve($request, $argument)`: the values to pass, none when the
    /// resolver does not apply.
    fn resolve(
        &self,
        request: &RequestHandle,
        argument: &ArgumentMetadata,
    ) -> Result<Vec<Value>, Throwable>;
}

/// `#[ValueResolver($resolver, $disabled)]`: defines which value resolver
/// should be used for a given parameter. `class` is the attribute's own
/// class, `ValueResolver` or a subclass of it (`MapEntity`, ...).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ValueResolver {
    pub class: String,
    /// The class name of the resolver to use.
    pub resolver: String,
    /// Whether this value resolver is disabled.
    pub disabled: bool,
}

impl ValueResolver {
    /// `new ValueResolver($resolver, $disabled)`.
    pub fn new(resolver: impl Into<String>, disabled: bool) -> Self {
        Self {
            class: VALUE_RESOLVER_CLASS.to_owned(),
            resolver: resolver.into(),
            disabled,
        }
    }

    /// As an attribute object.
    pub fn object(self) -> Value {
        Value::Object(Arc::new(self))
    }
}

impl php_value::PhpObject for ValueResolver {
    fn class(&self) -> &str {
        &self.class
    }

    fn properties(&self) -> PhpArray {
        let mut properties = PhpArray::new();
        properties.insert("resolver", Value::string(self.resolver.as_str()));
        properties.insert("disabled", Value::Bool(self.disabled));
        properties
    }
}

/// A resolver of the list, with its key: `None` for an int key (the
/// resolver is then disabled by its class), else the name it is disabled
/// by.
pub type Resolvers = Vec<(Option<String>, Arc<dyn ValueResolverInterface>)>;

/// The named resolvers: PHP's `ContainerInterface $namedResolvers`.
#[derive(Clone, Default)]
pub struct NamedResolvers {
    resolvers: Vec<(String, Arc<dyn ValueResolverInterface>)>,
    /// Whether the container is a `ServiceProviderInterface` (a
    /// `ServiceLocator`), whose names the error lists.
    service_provider: bool,
}

impl NamedResolvers {
    /// A service locator of these resolvers.
    pub fn locator(
        resolvers: impl IntoIterator<Item = (String, Arc<dyn ValueResolverInterface>)>,
    ) -> Self {
        Self {
            resolvers: resolvers.into_iter().collect(),
            service_provider: true,
        }
    }

    /// A container that does not list its services.
    pub fn container(
        resolvers: impl IntoIterator<Item = (String, Arc<dyn ValueResolverInterface>)>,
    ) -> Self {
        Self {
            resolvers: resolvers.into_iter().collect(),
            service_provider: false,
        }
    }

    fn get(&self, name: &str) -> Option<Arc<dyn ValueResolverInterface>> {
        self.resolvers
            .iter()
            .find(|(n, _)| n == name)
            .map(|(_, r)| r.clone())
    }

    fn names(&self) -> Vec<String> {
        if self.service_provider {
            self.resolvers.iter().map(|(n, _)| n.clone()).collect()
        } else {
            Vec::new()
        }
    }
}

/// `ArgumentResolver`.
#[derive(Clone)]
pub struct ArgumentResolver {
    argument_metadata_factory: Arc<dyn ArgumentMetadataFactoryInterface>,
    argument_value_resolvers: Resolvers,
    named_resolvers: Option<NamedResolvers>,
}

impl Default for ArgumentResolver {
    /// `new ArgumentResolver()`.
    fn default() -> Self {
        Self::new(None, Vec::new(), None)
    }
}

impl ArgumentResolver {
    /// `new ArgumentResolver($argumentMetadataFactory,
    /// $argumentValueResolvers, $namedResolvers)`: the default factory and
    /// the default resolvers when none are given.
    pub fn new(
        argument_metadata_factory: Option<Arc<dyn ArgumentMetadataFactoryInterface>>,
        argument_value_resolvers: Resolvers,
        named_resolvers: Option<NamedResolvers>,
    ) -> Self {
        Self {
            argument_metadata_factory: argument_metadata_factory
                .unwrap_or_else(|| Arc::new(ArgumentMetadataFactory)),
            argument_value_resolvers: if argument_value_resolvers.is_empty() {
                Self::get_default_argument_value_resolvers()
                    .into_iter()
                    .map(|r| (None, r))
                    .collect()
            } else {
                argument_value_resolvers
            },
            named_resolvers,
        }
    }

    /// The value resolvers, with their keys.
    pub fn resolvers(&self) -> &Resolvers {
        &self.argument_value_resolvers
    }

    /// `getArguments($request, $controller, $reflector)`.
    pub fn get_arguments(
        &self,
        request: &RequestHandle,
        controller: &Controller,
        reflector: Option<&[Parameter]>,
    ) -> Result<Vec<Value>, Throwable> {
        let mut arguments = Vec::new();

        for metadata in self
            .argument_metadata_factory
            .create_argument_metadata(controller, reflector)
        {
            let (argument_value_resolvers, disabled_resolvers) = self.resolvers_for(&metadata)?;

            let mut value_resolver_exceptions: Vec<Throwable> = Vec::new();
            let mut resolved = false;
            for (name, resolver) in &argument_value_resolvers {
                let key = name.as_deref().unwrap_or_else(|| resolver.class());
                if disabled_resolvers.contains(key) {
                    continue;
                }

                let count = match resolver.resolve(request, &metadata) {
                    Ok(values) => {
                        let count = values.len();
                        arguments.extend(values);
                        count
                    }
                    Err(e) if instance_of(e.as_ref(), NEAR_MISS) => {
                        value_resolver_exceptions.push(e);
                        0
                    }
                    Err(e) => return Err(e),
                };

                if 1 < count && !metadata.is_variadic() {
                    return Err(PhpException::new(
                        PhpClass::InvalidArgumentException,
                        format!(
                            "\"{}::resolve()\" must yield at most one value for non-variadic arguments.",
                            resolver.class()
                        ),
                    )
                    .throwable());
                }

                if count > 0 {
                    // continue to the next controller argument
                    resolved = true;
                    break;
                }
            }
            if resolved {
                continue;
            }

            return Err(unresolved(&metadata, &value_resolver_exceptions));
        }

        Ok(arguments)
    }

    /// The resolvers for one argument, and the names it disables: a
    /// `#[ValueResolver]` pins a named resolver.
    fn resolvers_for(
        &self,
        metadata: &ArgumentMetadata,
    ) -> Result<(Resolvers, HashSet<String>), Throwable> {
        let mut argument_value_resolvers = self.argument_value_resolvers.clone();
        let mut disabled_resolvers = HashSet::new();

        if let Some(named) = &self.named_resolvers {
            let attributes = metadata
                .get_attributes_of_type(VALUE_RESOLVER_CLASS, ArgumentMetadata::IS_INSTANCEOF);
            let mut resolver_name: Option<String> = None;
            for attribute in &attributes {
                let Some(attribute) = value_resolver(attribute) else {
                    continue;
                };
                if attribute.disabled {
                    disabled_resolvers.insert(attribute.resolver.clone());
                } else if resolver_name.as_deref().is_some_and(truthy) {
                    return Err(PhpException::new(
                        PhpClass::LogicException,
                        format!(
                            "You can only pin one resolver per argument, but argument \"${}\" of \"{}()\" has more.",
                            metadata.get_name(),
                            metadata.get_controller_name()
                        ),
                    )
                    .throwable());
                } else {
                    resolver_name = Some(attribute.resolver.clone());
                }
            }

            if let Some(resolver_name) = resolver_name.filter(|n| truthy(n)) {
                let Some(resolver) = named.get(&resolver_name) else {
                    return Err(resolver_not_found(&resolver_name, &named.names()));
                };
                argument_value_resolvers = vec![
                    (None, resolver),
                    (None, Arc::new(RequestAttributeValueResolver)),
                    (None, Arc::new(DefaultValueResolver)),
                ];
            }
        }

        Ok((argument_value_resolvers, disabled_resolvers))
    }

    /// `getDefaultArgumentValueResolvers()`.
    pub fn get_default_argument_value_resolvers() -> Vec<Arc<dyn ValueResolverInterface>> {
        vec![
            Arc::new(RequestAttributeValueResolver),
            Arc::new(RequestValueResolver),
            Arc::new(SessionValueResolver),
            Arc::new(DefaultValueResolver),
            Arc::new(VariadicValueResolver),
        ]
    }
}

impl ArgumentResolverInterface for ArgumentResolver {
    fn get_arguments(
        &self,
        request: &RequestHandle,
        controller: &Controller,
        reflector: &[Parameter],
    ) -> Result<Vec<Value>, Throwable> {
        Self::get_arguments(self, request, controller, Some(reflector))
    }
}

/// The `\RuntimeException` for an argument no resolver could resolve,
/// with the near misses as reasons.
fn unresolved(metadata: &ArgumentMetadata, value_resolver_exceptions: &[Throwable]) -> Throwable {
    let mut reasons: Vec<String> = value_resolver_exceptions
        .iter()
        .map(|e| message_of(e.as_ref()).to_string_lossy().into_owned())
        .collect();
    if reasons.is_empty() {
        reasons.push("Either the argument is nullable and no null value has been provided, no default value has been provided or there is a non-optional argument after this one.".to_owned());
    }

    let mut reason_counter = 1;
    if reasons.len() > 1 {
        for reason in &mut reasons {
            *reason = format!("{reason_counter}) {reason}");
            reason_counter += 1;
        }
    }

    PhpException::new(
        PhpClass::RuntimeException,
        format!(
            "Controller \"{}\" requires the \"${}\" argument that could not be resolved. {}{}",
            metadata.get_controller_name(),
            metadata.get_name(),
            if reason_counter > 1 {
                "Possible reasons: "
            } else {
                ""
            },
            reasons.join(" ")
        ),
    )
    .throwable()
}

/// A string's truthiness (`''` and `'0'` are false).
fn truthy(s: &str) -> bool {
    php_value::string::truthy(s.as_bytes())
}

fn value_resolver(attribute: &Value) -> Option<&ValueResolver> {
    match attribute {
        Value::Object(o) => o.downcast_ref::<ValueResolver>(),
        _ => None,
    }
}

/// `new ResolverNotFoundException($name, $alternatives)`.
fn resolver_not_found(name: &str, alternatives: &[String]) -> Throwable {
    let mut msg = format!("You have requested a non-existent resolver \"{name}\".");
    if !alternatives.is_empty() {
        if alternatives.len() == 1 {
            msg.push_str(" Did you mean this: \"");
        } else {
            msg.push_str(" Did you mean one of these: \"");
        }
        msg.push_str(&alternatives.join("\", \""));
        msg.push_str("\"?");
    }
    PhpException::new(PhpClass::ResolverNotFoundException, msg).throwable()
}
