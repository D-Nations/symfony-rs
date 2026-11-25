//! Port of Doctrine\DBAL\Types\TypeRegistry.php.
//!
//! The registry is shared (`Type::getTypeRegistry()` is process-wide), so
//! its map sits behind a lock and every method takes `&self`.
//!
//! Doctrine source copyright (c) Doctrine Project; distributed under MIT.
//!
//! Upstream: doctrine/dbal src/Types/TypeRegistry.php

use std::sync::{Arc, PoisonError, RwLock};

use super::{Type, TypesException};

/// The type registry is responsible for holding a map of all known DBAL
/// types.
#[derive(Debug, Default)]
pub struct TypeRegistry {
    /// Type names and their flyweight objects, in registration order.
    instances: RwLock<Vec<(String, Arc<dyn Type>)>>,
}

impl TypeRegistry {
    /// An empty registry.
    pub fn new() -> Self {
        Self::default()
    }

    /// `new TypeRegistry($instances)`.
    pub fn with_instances(
        instances: impl IntoIterator<Item = (String, Arc<dyn Type>)>,
    ) -> Result<Self, TypesException> {
        let registry = Self::new();
        for (name, type_) in instances {
            registry.register(&name, type_)?;
        }
        Ok(registry)
    }

    fn read<R>(&self, f: impl FnOnce(&Vec<(String, Arc<dyn Type>)>) -> R) -> R {
        f(&self
            .instances
            .read()
            .unwrap_or_else(PoisonError::into_inner))
    }

    /// Finds a type by the given name.
    pub fn get(&self, name: &str) -> Result<Arc<dyn Type>, TypesException> {
        self.read(|instances| {
            instances
                .iter()
                .find(|(n, _)| n == name)
                .map(|(_, type_)| Arc::clone(type_))
                .ok_or_else(|| TypesException::UnknownColumnType(name.to_owned()))
        })
    }

    /// Finds a name for the given type.
    pub fn lookup_name(&self, type_: &Arc<dyn Type>) -> Result<String, TypesException> {
        self.read(|instances| find_type_name(instances, type_))
            .ok_or_else(|| TypesException::type_not_registered(type_))
    }

    /// Checks if there is a type of the given name.
    pub fn has(&self, name: &str) -> bool {
        self.read(|instances| instances.iter().any(|(n, _)| n == name))
    }

    /// Registers a custom type to the type map.
    pub fn register(&self, name: &str, type_: Arc<dyn Type>) -> Result<(), TypesException> {
        let mut instances = self
            .instances
            .write()
            .unwrap_or_else(PoisonError::into_inner);
        if instances.iter().any(|(n, _)| n == name) {
            return Err(TypesException::TypesAlreadyExists(name.to_owned()));
        }
        if find_type_name(&instances, &type_).is_some() {
            return Err(TypesException::type_already_registered(&type_));
        }
        instances.push((name.to_owned(), type_));
        Ok(())
    }

    /// `override()`: overrides an already defined type to use a different
    /// implementation.
    pub fn override_type(&self, name: &str, type_: Arc<dyn Type>) -> Result<(), TypesException> {
        let mut instances = self
            .instances
            .write()
            .unwrap_or_else(PoisonError::into_inner);
        let Some(index) = instances.iter().position(|(n, _)| n == name) else {
            return Err(TypesException::TypeNotFound(name.to_owned()));
        };
        if find_type_name(&instances, &type_).is_some_and(|found| found != name) {
            return Err(TypesException::type_already_registered(&type_));
        }
        instances[index].1 = type_;
        Ok(())
    }

    /// Gets the map of all registered types and their corresponding type
    /// instances (`@internal`).
    pub fn get_map(&self) -> Vec<(String, Arc<dyn Type>)> {
        self.read(Clone::clone)
    }
}

fn find_type_name(instances: &[(String, Arc<dyn Type>)], type_: &Arc<dyn Type>) -> Option<String> {
    instances
        .iter()
        .find(|(_, t)| Arc::ptr_eq(t, type_))
        .map(|(name, _)| name.clone())
}
