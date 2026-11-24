//! Port of Symfony\Component\Routing\RouteCollection.php.
//!
//! A collection owns its routes (PHP shares Route objects between the
//! collections that hold their handles): [`RouteCollection::add_collection`]
//! takes the added collection, and [`RouteCollection::get_mut`] is how a
//! route in it is changed. Cloning is `__clone()`'s deep copy of routes
//! and aliases; resources are shared, as PHP shares them.
//!
//! Names are PHP strings in insertion order, kept as PHP keeps its
//! arrays ([`Ordered`]): looking up, adding and removing a name take
//! constant time. `all()` orders by priority (highest first), then by
//! insertion; upstream sorts its own array in place, which gives the same
//! order every later call observes.
//!
//! Resources (`ResourceInterface` from symfony/config, upstream's
//! require-dev) are available with the `config` feature.
//!
//! Symfony source copyright (c) Fabien Potencier; distributed under MIT.
//!
//! Upstream: symfony/routing RouteCollection.php

use std::collections::HashSet;
#[cfg(feature = "config")]
use std::sync::Arc;

use php_value::{PhpArray, PhpString, Value, compare::loose_equals};
#[cfg(feature = "config")]
use symfony_config::resource::ResourceInterface;
use symfony_deprecation_contracts::trigger_deprecation;

use crate::{
    alias::Alias,
    exception::{Class, Error, concat},
    ordered::Ordered,
    route::Route,
};

/// `RouteCollection`: a set of named routes. Adding a route under an
/// existing name removes the old one first, so the new one comes last.
#[derive(Debug, Clone, Default)]
pub struct RouteCollection {
    routes: Ordered<Route>,
    aliases: Ordered<Alias>,
    /// By the resource's string form.
    #[cfg(feature = "config")]
    resources: Ordered<Arc<dyn ResourceInterface>>,
    priorities: Ordered<i64>,
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
        self.unset(&name);
        self.routes.set(name.clone(), route);
        if priority != 0 {
            self.priorities.set(name, priority);
        }
    }

    /// `unset($this->routes[$name], $this->priorities[$name],
    /// $this->aliases[$name])`.
    fn unset(&mut self, name: &[u8]) {
        self.routes.remove(name);
        self.priorities.remove(name);
        self.aliases.remove(name);
    }

    fn priority_of(&self, name: &[u8]) -> i64 {
        self.priorities.get(name).copied().unwrap_or(0)
    }

    /// `all()` (and `getIterator()`): every route, by priority, then in
    /// insertion order.
    pub fn all(&self) -> Vec<(&PhpString, &Route)> {
        let mut all: Vec<_> = self.routes.iter().collect();
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
        Ok(self.routes.get(&name))
    }

    /// `get()` for changing the route.
    pub fn get_mut(&mut self, name: &[u8]) -> Result<Option<&mut Route>, Error> {
        let name = self.resolve(name)?;
        Ok(self.routes.get_mut(&name))
    }

    fn resolve(&self, name: &[u8]) -> Result<Vec<u8>, Error> {
        let mut name = name.to_vec();
        let mut visited: Vec<Vec<u8>> = Vec::new();
        while let Some(alias) = self.aliases.get(&name) {
            // array_search() compares loosely: "010" is found as "10"
            let current = Value::string(name.as_slice());
            let search_key = visited
                .iter()
                .position(|v| loose_equals(&Value::string(v.as_slice()), &current));
            if let Some(search_key) = search_key {
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
        let mut routes: HashSet<Vec<u8>> = HashSet::new();
        for &n in names {
            if self.routes.contains(n) {
                routes.insert(n.to_vec());
            }
            self.unset(n);
        }
        if routes.is_empty() {
            return;
        }
        self.aliases
            .retain(|_, alias| !routes.contains(alias.id().as_bytes()));
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
            mut routes,
            mut aliases,
            #[cfg(feature = "config")]
            mut resources,
            priorities,
        } = collection;
        for name in order {
            let Some((name, route)) = routes.remove(&name) else {
                continue;
            };
            self.unset(&name);
            if let Some(&p) = priorities.get(&name) {
                self.priorities.set(name.clone(), p);
            }
            self.routes.set(name, route);
        }
        for (name, alias) in aliases.take() {
            self.unset(&name);
            self.aliases.set(name, alias);
        }
        #[cfg(feature = "config")]
        for (_, resource) in resources.take() {
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
        for route in self.routes.values_mut() {
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
        let mut prefixed_routes = Ordered::default();
        let mut prefixed_priorities = Ordered::default();
        let mut prefixed_aliases = Ordered::default();
        let names: HashSet<Vec<u8>> = self.routes.iter().map(|(n, _)| n.to_vec()).collect();

        for (name, mut route) in self.routes.take() {
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
            if let Some(&p) = self.priorities.get(&name) {
                prefixed_priorities.set(prefixed(&name), p);
            }
            prefixed_routes.set(prefixed(&name), route);
        }

        for (name, alias) in self.aliases.iter() {
            let mut target = alias.id().to_vec();
            if names.contains(&target) || self.aliases.contains(&target) {
                target = concat(&[prefix, &target]);
            }
            prefixed_aliases.set(prefixed(name), alias.with_id(target));
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
        for route in self.routes.values_mut() {
            route.set_host(pattern)?;
            route.add_defaults(defaults)?;
            route.add_requirements(requirements)?;
        }
        Ok(())
    }

    /// `setCondition()` on all routes; existing conditions are
    /// overridden. A null condition is `''`.
    pub fn set_condition(&mut self, condition: &[u8]) {
        for route in self.routes.values_mut() {
            route.set_condition(condition);
        }
    }

    /// `addDefaults()` to all routes; an existing default value under the
    /// same name in a route is overridden.
    pub fn add_defaults(&mut self, defaults: &PhpArray) -> Result<(), Error> {
        if !defaults.is_empty() {
            for route in self.routes.values_mut() {
                route.add_defaults(defaults)?;
            }
        }
        Ok(())
    }

    /// `addRequirements()` to all routes; an existing requirement under the
    /// same name in a route is overridden.
    pub fn add_requirements(&mut self, requirements: &PhpArray) -> Result<(), Error> {
        if !requirements.is_empty() {
            for route in self.routes.values_mut() {
                route.add_requirements(requirements)?;
            }
        }
        Ok(())
    }

    /// `addOptions()` to all routes; an existing option value under the
    /// same name in a route is overridden.
    pub fn add_options(&mut self, options: &PhpArray) {
        if !options.is_empty() {
            for route in self.routes.values_mut() {
                route.add_options(options);
            }
        }
    }

    /// `setSchemes()` on all child routes.
    pub fn set_schemes(&mut self, schemes: &[&str]) {
        for route in self.routes.values_mut() {
            route.set_schemes(schemes.iter().copied());
        }
    }

    /// `setMethods()` on all child routes.
    pub fn set_methods(&mut self, methods: &[&str]) {
        for route in self.routes.values_mut() {
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
        if !self.resources.contains(&key) {
            self.resources.set(PhpString::from(key), resource);
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
        self.routes.remove(name);
        self.priorities.remove(name);
        self.aliases.set(PhpString::from(name), Alias::new(alias));
        self.aliases
            .get_mut(name)
            .ok_or_else(|| Error::new(Class::NotPorted, "alias not stored"))
    }

    /// `getAliases()`, in order.
    pub fn aliases(&self) -> Vec<(&PhpString, &Alias)> {
        self.aliases.iter().collect()
    }

    /// `getAlias()`.
    pub fn alias(&self, name: &[u8]) -> Option<&Alias> {
        self.aliases.get(name)
    }

    /// `getAlias()` for changing the alias (PHP's Alias is a handle the
    /// collection shares, e.g. for `setDeprecated()`).
    pub fn alias_mut(&mut self, name: &[u8]) -> Option<&mut Alias> {
        self.aliases.get_mut(name)
    }

    /// `getPriority()`.
    pub fn priority(&self, name: &[u8]) -> Option<i64> {
        self.priorities.get(name).copied()
    }
}
