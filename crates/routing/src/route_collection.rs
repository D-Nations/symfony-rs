//! Port of Symfony\Component\Routing\RouteCollection.php.
//!
//! A collection owns its routes (PHP shares Route objects between the
//! collections that hold their handles): [`RouteCollection::add_collection`]
//! takes the added collection, and [`RouteCollection::get_mut`] is how a
//! route in it is changed. Cloning is `__clone()`'s deep copy of routes
//! and aliases; resources are shared, as PHP shares them.
//!
//! Names are PHP strings in insertion order. `all()` orders by priority
//! (highest first), then by insertion; upstream sorts its own array in
//! place, which gives the same order every later call observes.
//!
//! Resources (`ResourceInterface` from symfony/config, upstream's
//! require-dev) are available with the `config` feature.
//!
//! Symfony source copyright (c) Fabien Potencier; distributed under MIT.
//!
//! Upstream: symfony/routing RouteCollection.php

#[cfg(feature = "config")]
use std::sync::Arc;

use php_value::{PhpArray, PhpString};
#[cfg(feature = "config")]
use symfony_config::resource::ResourceInterface;
use symfony_deprecation_contracts::trigger_deprecation;

use crate::{
    alias::Alias,
    exception::{Class, Error, concat},
    route::Route,
};

/// `RouteCollection`: a set of named routes. Adding a route under an
/// existing name removes the old one first, so the new one comes last.
#[derive(Debug, Clone, Default)]
pub struct RouteCollection {
    routes: Vec<(PhpString, Route)>,
    aliases: Vec<(PhpString, Alias)>,
    #[cfg(feature = "config")]
    resources: Vec<(Vec<u8>, Arc<dyn ResourceInterface>)>,
    priorities: Vec<(PhpString, i64)>,
}

fn position<T>(list: &[(PhpString, T)], name: &[u8]) -> Option<usize> {
    list.iter().position(|(n, _)| n.as_bytes() == name)
}

fn unset<T>(list: &mut Vec<(PhpString, T)>, name: &[u8]) {
    list.retain(|(n, _)| n.as_bytes() != name);
}

/// Inserts or replaces in place, as assigning an existing PHP array key
/// keeps its position.
fn set<T>(list: &mut Vec<(PhpString, T)>, name: PhpString, value: T) -> usize {
    match position(list, &name) {
        Some(i) => {
            list[i].1 = value;
            i
        }
        None => {
            list.push((name, value));
            list.len() - 1
        }
    }
}

impl RouteCollection {
    pub fn new() -> Self {
        Self::default()
    }

    /// `count()`.
    pub fn count(&self) -> usize {
        self.routes.len()
    }

    /// `add()`.
    pub fn add(&mut self, name: impl Into<PhpString>, route: Route, priority: i64) {
        let name = name.into();
        unset(&mut self.routes, &name);
        unset(&mut self.priorities, &name);
        unset(&mut self.aliases, &name);
        self.routes.push((name.clone(), route));
        if priority != 0 {
            self.priorities.push((name, priority));
        }
    }

    fn priority_of(&self, name: &[u8]) -> i64 {
        position(&self.priorities, name).map_or(0, |i| self.priorities[i].1)
    }

    /// `all()` (and `getIterator()`): every route, by priority, then in
    /// insertion order.
    pub fn all(&self) -> Vec<(&PhpString, &Route)> {
        let mut all: Vec<_> = self.routes.iter().map(|(n, r)| (n, r)).collect();
        if !self.priorities.is_empty() {
            // a stable sort keeps insertion order among equal priorities
            all.sort_by_key(|(n, _)| std::cmp::Reverse(self.priority_of(n)));
        }
        all
    }

    /// `get()`: a route by name, following aliases. A deprecated alias
    /// triggers its deprecation.
    pub fn get(&self, name: &[u8]) -> Result<Option<&Route>, Error> {
        let name = self.resolve(name)?;
        Ok(position(&self.routes, &name).map(|i| &self.routes[i].1))
    }

    /// `get()` for changing the route.
    pub fn get_mut(&mut self, name: &[u8]) -> Result<Option<&mut Route>, Error> {
        let name = self.resolve(name)?;
        Ok(position(&self.routes, &name).map(|i| &mut self.routes[i].1))
    }

    fn resolve(&self, name: &[u8]) -> Result<Vec<u8>, Error> {
        let mut name = name.to_vec();
        let mut visited: Vec<Vec<u8>> = Vec::new();
        while let Some(i) = position(&self.aliases, &name) {
            let alias = &self.aliases[i].1;
            if let Some(search_key) = visited.iter().position(|v| *v == name) {
                visited.push(name.clone());
                let path: Vec<&[u8]> = visited[search_key..].iter().map(Vec::as_slice).collect();
                return Err(Error::circular_reference(&name, &path));
            }
            if let Some(deprecation) = alias.deprecation(&name) {
                trigger_deprecation(
                    deprecation.package.as_bytes(),
                    deprecation.version.as_bytes(),
                    deprecation.message.as_bytes(),
                );
            }
            visited.push(name);
            name = alias.id().to_vec();
        }
        Ok(name)
    }

    /// `remove()` with one name.
    pub fn remove(&mut self, name: &[u8]) {
        self.remove_all(&[name]);
    }

    /// `remove()` with a list of names. Aliases of a removed route go too.
    pub fn remove_all(&mut self, names: &[&[u8]]) {
        let mut routes: Vec<Vec<u8>> = Vec::new();
        for &n in names {
            if position(&self.routes, n).is_some() {
                routes.push(n.to_vec());
            }
            unset(&mut self.routes, n);
            unset(&mut self.priorities, n);
            unset(&mut self.aliases, n);
        }
        if routes.is_empty() {
            return;
        }
        self.aliases
            .retain(|(_, alias)| !routes.iter().any(|r| r.as_slice() == alias.id().as_bytes()));
    }

    /// `addCollection()`: appends the routes of `collection`, replacing
    /// routes of the same names.
    pub fn add_collection(&mut self, collection: RouteCollection) {
        // we need to remove all routes with the same names first because just replacing them
        // would not place the new route at the end of the merged array
        let order: Vec<PhpString> = collection
            .all()
            .into_iter()
            .map(|(n, _)| n.clone())
            .collect();
        let RouteCollection {
            routes,
            aliases,
            #[cfg(feature = "config")]
            resources,
            priorities,
        } = collection;
        let mut routes = routes;
        for name in order {
            let Some(i) = position(&routes, &name) else {
                continue;
            };
            let (name, route) = routes.remove(i);
            unset(&mut self.routes, &name);
            unset(&mut self.priorities, &name);
            unset(&mut self.aliases, &name);
            if let Some(p) = position(&priorities, &name) {
                self.priorities.push((name.clone(), priorities[p].1));
            }
            self.routes.push((name, route));
        }
        for (name, alias) in aliases {
            unset(&mut self.routes, &name);
            unset(&mut self.priorities, &name);
            unset(&mut self.aliases, &name);
            self.aliases.push((name, alias));
        }
        #[cfg(feature = "config")]
        for (_, resource) in resources {
            self.add_resource(resource);
        }
    }

    /// `addPrefix()`: a prefix to the path of all child routes.
    pub fn add_prefix(
        &mut self,
        prefix: &[u8],
        defaults: &PhpArray,
        requirements: &PhpArray,
    ) -> Result<(), Error> {
        let prefix = crate::php::trim_char(crate::php::trim(prefix), b'/', true);
        if prefix.is_empty() {
            return Ok(());
        }
        for (_, route) in &mut self.routes {
            let path = concat(&[b"/", prefix, route.path()]);
            route.set_path(path)?;
            route.add_defaults(defaults)?;
            route.add_requirements(requirements)?;
        }
        Ok(())
    }

    /// `addNamePrefix()`: a prefix to the name of all the routes within
    /// the collection.
    pub fn add_name_prefix(&mut self, prefix: &[u8]) -> Result<(), Error> {
        let prefixed = |name: &[u8]| PhpString::from(concat(&[prefix, name]));
        let mut prefixed_routes = Vec::new();
        let mut prefixed_priorities = Vec::new();
        let mut prefixed_aliases = Vec::new();
        let names: Vec<PhpString> = self.routes.iter().map(|(n, _)| n.clone()).collect();

        for (name, mut route) in std::mem::take(&mut self.routes) {
            if let Some(canonical) = route.default_value(b"_canonical_route").cloned() {
                // `$prefix.$canonicalName`
                let Some(canonical) = canonical.to_php_string_or_stringable() else {
                    return Err(Error::new(
                        Class::NotPorted,
                        format!(
                            "a _canonical_route default of type {} is not ported",
                            canonical.debug_type()
                        ),
                    ));
                };
                route.set_default(
                    b"_canonical_route",
                    php_value::Value::String(prefixed(&canonical)),
                )?;
            }
            if let Some(p) = position(&self.priorities, &name) {
                prefixed_priorities.push((prefixed(&name), self.priorities[p].1));
            }
            set(&mut prefixed_routes, prefixed(&name), route);
        }

        for (name, alias) in &self.aliases {
            let mut target = alias.id().to_vec();
            if names.iter().any(|n| n.as_bytes() == target.as_slice())
                || position(&self.aliases, &target).is_some()
            {
                target = concat(&[prefix, &target]);
            }
            set(&mut prefixed_aliases, prefixed(name), alias.with_id(target));
        }

        self.routes = prefixed_routes;
        self.priorities = prefixed_priorities;
        self.aliases = prefixed_aliases;
        Ok(())
    }

    /// `setHost()` on all routes; a null pattern is `''`.
    pub fn set_host(
        &mut self,
        pattern: &[u8],
        defaults: &PhpArray,
        requirements: &PhpArray,
    ) -> Result<(), Error> {
        for (_, route) in &mut self.routes {
            route.set_host(pattern)?;
            route.add_defaults(defaults)?;
            route.add_requirements(requirements)?;
        }
        Ok(())
    }

    /// `setCondition()` on all routes; existing conditions are
    /// overridden. A null condition is `''`.
    pub fn set_condition(&mut self, condition: &[u8]) {
        for (_, route) in &mut self.routes {
            route.set_condition(condition);
        }
    }

    /// `addDefaults()` to all routes; an existing default value under the
    /// same name in a route is overridden.
    pub fn add_defaults(&mut self, defaults: &PhpArray) -> Result<(), Error> {
        if !defaults.is_empty() {
            for (_, route) in &mut self.routes {
                route.add_defaults(defaults)?;
            }
        }
        Ok(())
    }

    /// `addRequirements()` to all routes; an existing requirement under the
    /// same name in a route is overridden.
    pub fn add_requirements(&mut self, requirements: &PhpArray) -> Result<(), Error> {
        if !requirements.is_empty() {
            for (_, route) in &mut self.routes {
                route.add_requirements(requirements)?;
            }
        }
        Ok(())
    }

    /// `addOptions()` to all routes; an existing option value under the
    /// same name in a route is overridden.
    pub fn add_options(&mut self, options: &PhpArray) {
        if !options.is_empty() {
            for (_, route) in &mut self.routes {
                route.add_options(options);
            }
        }
    }

    /// `setSchemes()` on all child routes.
    pub fn set_schemes(&mut self, schemes: &[&str]) {
        for (_, route) in &mut self.routes {
            route.set_schemes(schemes.iter().copied());
        }
    }

    /// `setMethods()` on all child routes.
    pub fn set_methods(&mut self, methods: &[&str]) {
        for (_, route) in &mut self.routes {
            route.set_methods(methods.iter().copied());
        }
    }

    /// `getResources()`.
    #[cfg(feature = "config")]
    pub fn resources(&self) -> Vec<Arc<dyn ResourceInterface>> {
        self.resources.iter().map(|(_, r)| r.clone()).collect()
    }

    /// `addResource()`: a resource whose string form is already present is
    /// not added.
    #[cfg(feature = "config")]
    pub fn add_resource(&mut self, resource: Arc<dyn ResourceInterface>) {
        let key = resource.to_string();
        if !self.resources.iter().any(|(k, _)| *k == key) {
            self.resources.push((key, resource));
        }
    }

    /// `addAlias($name, $alias)`: `name` becomes another name for the
    /// route `alias`.
    pub fn add_alias(&mut self, name: &[u8], alias: &[u8]) -> Result<&mut Alias, Error> {
        if name == alias {
            return Err(Error::new(
                Class::InvalidArgument,
                concat(&[b"Route alias \"", name, b"\" can not reference itself."]),
            ));
        }
        unset(&mut self.routes, name);
        unset(&mut self.priorities, name);
        let i = set(&mut self.aliases, PhpString::from(name), Alias::new(alias));
        Ok(&mut self.aliases[i].1)
    }

    /// `getAliases()`.
    pub fn aliases(&self) -> &[(PhpString, Alias)] {
        &self.aliases
    }

    /// `getAlias()`.
    pub fn alias(&self, name: &[u8]) -> Option<&Alias> {
        position(&self.aliases, name).map(|i| &self.aliases[i].1)
    }

    /// `getAlias()` for changing the alias (PHP's Alias is a handle the
    /// collection shares, e.g. for `setDeprecated()`).
    pub fn alias_mut(&mut self, name: &[u8]) -> Option<&mut Alias> {
        position(&self.aliases, name).map(|i| &mut self.aliases[i].1)
    }

    /// `getPriority()`.
    pub fn priority(&self, name: &[u8]) -> Option<i64> {
        position(&self.priorities, name).map(|i| self.priorities[i].1)
    }
}
