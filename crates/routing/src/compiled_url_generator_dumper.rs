//! Port of Symfony\Component\Routing\Generator\Dumper\CompiledUrlGeneratorDumper.php,
//! with GeneratorDumper.php: the routes and aliases of a
//! [`RouteCollection`] compiled for [`CompiledUrlGenerator`].
//!
//! `getCompiledRoutes()` and `getCompiledAliases()` are ported as data.
//! `dump()`, which writes them as a PHP file for `require`, is not
//! (owner, November 24): it becomes a build-time emitter of Rust when the
//! route loaders are ported. [`CompiledUrlGeneratorDumper::compiled`] is
//! what that file returns. Values `CompiledUrlMatcherDumper::export()`
//! refuses (objects without a PHP export form) are kept here as they are.
//!
//! Symfony source copyright (c) Fabien Potencier; distributed under MIT.
//!
//! Upstream: symfony/routing Generator/Dumper/CompiledUrlGeneratorDumper.php Generator/Dumper/GeneratorDumper.php
//!
//! [`CompiledUrlGenerator`]: crate::compiled_url_generator::CompiledUrlGenerator

use php_value::{Key, PhpArray, PhpString, Value, compare::loose_equals};
use symfony_deprecation_contracts::trigger_deprecation;

use crate::{
    compiled_url_generator::{CompiledRoute, CompiledRoutes},
    exception::{Class, Error, concat},
    php::{isset, string_argument},
    route::Route,
    route_collection::RouteCollection,
    url_generator::cast,
};

/// `CompiledUrlGeneratorDumper`.
#[derive(Debug, Clone)]
pub struct CompiledUrlGeneratorDumper {
    routes: RouteCollection,
}

/// `$route->compile()` and the entry upstream builds from it.
fn compile(route: &Route, defaults: PhpArray) -> Result<CompiledRoute, Error> {
    let mut owned;
    let route = if route.compiled().is_some() {
        route
    } else {
        owned = route.clone();
        owned.compile()?;
        &owned
    };
    let compiled = route
        .compiled()
        .ok_or_else(|| Error::new(Class::NotPorted, "route not compiled"))?;
    Ok(CompiledRoute {
        variables: compiled.variables().clone(),
        defaults,
        requirements: route.requirements().clone(),
        tokens: compiled.tokens().clone(),
        host_tokens: compiled.host_tokens().clone(),
        schemes: route.schemes().clone(),
        deprecations: Vec::new(),
    })
}

impl CompiledUrlGeneratorDumper {
    /// `new CompiledUrlGeneratorDumper($routes)`.
    pub fn new(routes: RouteCollection) -> Self {
        Self { routes }
    }

    /// `getRoutes()`.
    pub fn routes(&self) -> &RouteCollection {
        &self.routes
    }

    /// `getRoutes()` for changing the collection.
    pub fn routes_mut(&mut self) -> &mut RouteCollection {
        &mut self.routes
    }

    /// `getCompiledRoutes()`: every route, compiled, in `all()` order.
    pub fn compiled_routes(&self) -> Result<CompiledRoutes, Error> {
        let mut compiled_routes = CompiledRoutes::new();
        for (name, route) in self.routes.all() {
            compiled_routes.insert(name.clone(), compile(route, route.defaults().clone())?);
        }
        Ok(compiled_routes)
    }

    /// `getCompiledAliases()`: each alias as its target route, with the
    /// deprecations of the aliases it goes through. The deprecations of
    /// aliases past the first are triggered now as well.
    pub fn compiled_aliases(&self) -> Result<CompiledRoutes, Error> {
        let routes = &self.routes;
        let mut compiled_aliases = CompiledRoutes::new();
        for (name, alias) in routes.aliases() {
            let mut deprecations: Vec<_> = alias.deprecation(name.as_bytes()).into_iter().collect();
            let mut current_id = alias.id().clone();
            let mut visited: Vec<PhpString> = Vec::new();
            while let Some(alias) = routes.alias(current_id.as_bytes()) {
                // array_search() compares loosely
                let current = Value::String(current_id.clone());
                let search_key = visited
                    .iter()
                    .position(|v| loose_equals(&Value::String(v.clone()), &current));
                if let Some(search_key) = search_key {
                    visited.push(current_id.clone());
                    let path: Vec<&[u8]> =
                        visited[search_key..].iter().map(|v| v.as_bytes()).collect();
                    return Err(Error::circular_reference(current_id.as_bytes(), &path));
                }

                if let Some(deprecation) = alias.deprecation(current_id.as_bytes()) {
                    trigger_deprecation(
                        deprecation.package.as_bytes(),
                        deprecation.version.as_bytes(),
                        deprecation.message.as_bytes(),
                    );
                    deprecations.push(deprecation);
                }

                visited.push(current_id.clone());
                current_id = alias.id().clone();
            }

            let Some(target) = routes.get(current_id.as_bytes())? else {
                return Err(Error::new(
                    Class::RouteNotFound,
                    concat(&[
                        b"Target route \"",
                        current_id.as_bytes(),
                        b"\" for alias \"",
                        name.as_bytes(),
                        b"\" does not exist.",
                    ]),
                ));
            };

            let mut defaults = target.defaults().clone();
            let locale = defaults.get(&Key::from("_locale")).cloned();
            if let Some(locale) = locale.filter(|_| isset(&defaults, b"_locale")) {
                let suffix = concat(&[b".", cast(&locale)?.as_bytes()]);
                if name.as_bytes().ends_with(&suffix) {
                    // strlen() takes a string
                    let length = string_argument(&locale, "strlen", 1, "string")?.len();
                    let canonical = &name.as_bytes()[..name.len() - length - 1];
                    defaults.insert("_canonical_route", Value::string(canonical));
                }
            }

            let mut compiled = compile(target, defaults)?;
            compiled.deprecations = deprecations;
            compiled_aliases.insert(name.clone(), compiled);
        }
        Ok(compiled_aliases)
    }

    /// What `require` returns for the file `dump()` writes: the compiled
    /// routes, then the compiled aliases.
    pub fn compiled(&self) -> Result<CompiledRoutes, Error> {
        let mut compiled = self.compiled_routes()?;
        for (name, route) in self.compiled_aliases()?.iter() {
            compiled.insert(name.clone(), route.clone());
        }
        Ok(compiled)
    }
}
