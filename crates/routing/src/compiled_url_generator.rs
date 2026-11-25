//! Port of Symfony\Component\Routing\Generator\CompiledUrlGenerator.php:
//! a [`UrlGenerator`] reading routes compiled ahead of time
//! ([`CompiledRoutes`], what
//! [`CompiledUrlGeneratorDumper`](crate::compiled_url_generator_dumper::CompiledUrlGeneratorDumper)
//! gives) instead of a [`RouteCollection`](crate::route_collection::RouteCollection).
//!
//! Upstream reads the compiled routes from the PHP file the dumper wrote
//! (`require`); here they are the dumper's data, kept as values (owner,
//! November 24: the dump becomes a build-time emitter later). An entry is
//! upstream's `[$variables, $defaults, $requirements, $tokens,
//! $hostTokens, $requiredSchemes, $deprecations]`.
//!
//! Symfony source copyright (c) Fabien Potencier; distributed under MIT.
//!
//! Upstream: symfony/routing Generator/CompiledUrlGenerator.php

use std::{collections::HashMap, sync::Arc};

use php_value::{PhpArray, PhpString, Value};
use psr_log::LoggerInterface;
use symfony_deprecation_contracts::trigger_deprecation;

use crate::{
    alias::Deprecation,
    exception::{Class, Error, concat},
    request_context::RequestContext,
    route_collection::RouteCollection,
    url_generator::{
        ConfigurableRequirementsInterface, ReferenceType, UrlGenerator, UrlGeneratorInterface,
        for_each_locale, localize_parameters, requested_locale,
    },
};

/// One compiled route or alias.
#[derive(Debug, Clone, PartialEq)]
pub struct CompiledRoute {
    /// The compiled route's variables.
    pub variables: PhpArray,
    /// The route's defaults (for a localized alias, with its own
    /// `_canonical_route`).
    pub defaults: PhpArray,
    pub requirements: PhpArray,
    /// The compiled path tokens.
    pub tokens: PhpArray,
    /// The compiled host tokens.
    pub host_tokens: PhpArray,
    /// The route's schemes.
    pub schemes: PhpArray,
    /// Deprecations triggered when generating through this name (an
    /// alias and the deprecated aliases it leads through).
    pub deprecations: Vec<Deprecation>,
}

/// `array $compiledRoutes`: compiled routes by name, in order.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct CompiledRoutes {
    entries: Vec<(PhpString, CompiledRoute)>,
    index: HashMap<Vec<u8>, usize>,
}

impl CompiledRoutes {
    pub fn new() -> Self {
        Self::default()
    }

    /// `$compiledRoutes[$name] = $route`: replaces in place, as assigning
    /// an existing PHP array key keeps its position.
    pub fn insert(&mut self, name: impl Into<PhpString>, route: CompiledRoute) {
        let name = name.into();
        if let Some(&i) = self.index.get(name.as_bytes()) {
            self.entries[i].1 = route
        } else {
            self.index
                .insert(name.as_bytes().to_vec(), self.entries.len());
            self.entries.push((name, route));
        }
    }

    /// `$compiledRoutes[$name] ?? null`.
    pub fn get(&self, name: &[u8]) -> Option<&CompiledRoute> {
        self.index.get(name).map(|&i| &self.entries[i].1)
    }

    /// `unset($compiledRoutes[$name])`.
    pub fn remove(&mut self, name: &[u8]) -> Option<CompiledRoute> {
        let i = self.index.remove(name)?;
        let (_, route) = self.entries.remove(i);
        for position in self.index.values_mut() {
            if *position > i {
                *position -= 1;
            }
        }
        Some(route)
    }

    pub fn len(&self) -> usize {
        self.entries.len()
    }

    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    pub fn iter(&self) -> impl Iterator<Item = (&PhpString, &CompiledRoute)> {
        self.entries.iter().map(|(n, r)| (n, r))
    }
}

/// `CompiledUrlGenerator`.
#[derive(Debug)]
pub struct CompiledUrlGenerator {
    compiled_routes: CompiledRoutes,
    default_locale: Option<PhpString>,
    /// The parent class's state: context, logger, strictness.
    generator: UrlGenerator,
}

impl CompiledUrlGenerator {
    /// `new CompiledUrlGenerator($compiledRoutes, $context, $logger,
    /// $defaultLocale)`.
    pub fn new(
        compiled_routes: CompiledRoutes,
        context: RequestContext,
        logger: Option<Arc<dyn LoggerInterface>>,
        default_locale: Option<PhpString>,
    ) -> Self {
        Self {
            compiled_routes,
            default_locale,
            generator: UrlGenerator::new(RouteCollection::new(), context, logger, None),
        }
    }

    /// `getContext()` for changing the context in place.
    pub fn context_mut(&mut self) -> &mut RequestContext {
        self.generator.context_mut()
    }
}

impl UrlGeneratorInterface for CompiledUrlGenerator {
    fn generate(
        &self,
        name: &[u8],
        mut parameters: PhpArray,
        reference_type: ReferenceType,
    ) -> Result<PhpString, Error> {
        let mut name = name.to_vec();
        let locale = requested_locale(
            self.generator.context(),
            &parameters,
            self.default_locale.as_ref(),
        );

        if let Some(locale) = locale {
            let mut localized = None;
            for_each_locale(&locale, |text| {
                let candidate = concat(&[&name, b".", text]);
                let canonical = self
                    .compiled_routes
                    .get(&candidate)
                    .and_then(|r| r.defaults.get(&"_canonical_route".into()));
                if canonical.is_some_and(|c| c.identical(&Value::string(name.as_slice()))) {
                    localized = Some(candidate);
                    return Ok(true);
                }
                Ok(false)
            })?;
            if let Some(localized) = localized {
                name = localized;
            }
        }

        let Some(route) = self.compiled_routes.get(&name) else {
            return Err(Error::new(
                Class::RouteNotFound,
                concat(&[
                    b"Unable to generate a URL for the named route \"",
                    &name,
                    b"\" as such route does not exist.",
                ]),
            ));
        };

        for deprecation in &route.deprecations {
            trigger_deprecation(
                deprecation.package.as_bytes(),
                deprecation.version.as_bytes(),
                deprecation.message.as_bytes(),
            );
        }

        localize_parameters(&route.defaults, &route.variables, &mut parameters);

        self.generator.do_generate(
            &route.variables,
            &route.defaults,
            &route.requirements,
            &route.tokens,
            parameters,
            &name,
            reference_type,
            &route.host_tokens,
            &route.schemes,
        )
    }

    fn set_context(&mut self, context: RequestContext) {
        self.generator.set_context(context);
    }

    fn context(&self) -> &RequestContext {
        self.generator.context()
    }
}

impl ConfigurableRequirementsInterface for CompiledUrlGenerator {
    fn set_strict_requirements(&mut self, enabled: Option<bool>) {
        self.generator.set_strict_requirements(enabled);
    }

    fn is_strict_requirements(&self) -> Option<bool> {
        self.generator.is_strict_requirements()
    }
}
