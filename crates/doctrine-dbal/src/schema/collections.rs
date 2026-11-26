//! Port of Doctrine\DBAL\Schema\Collections: ObjectSet.php,
//! UnqualifiedNamedObjectSet.php, OptionallyUnqualifiedNamedObjectSet.php
//! and the Exception\ObjectAlreadyExists.php and ObjectDoesNotExist.php
//! they throw (see [`super::exception`] for the messages).
//!
//! PHP's `ObjectSet` interface is two generic structs here. A failure is
//! a [`CollectionError`], which keeps the object name the exceptions'
//! `getObjectName()` returns.
//!
//! Doctrine source copyright (c) Doctrine Project; distributed under MIT.
//!
//! Upstream: doctrine/dbal src/Schema/Collections/ObjectSet.php
//! Upstream: doctrine/dbal src/Schema/Collections/UnqualifiedNamedObjectSet.php
//! Upstream: doctrine/dbal src/Schema/Collections/OptionallyUnqualifiedNamedObjectSet.php
//! Upstream: doctrine/dbal src/Schema/Collections/Exception.php
//! Upstream: doctrine/dbal src/Schema/Collections/Exception/ObjectAlreadyExists.php
//! Upstream: doctrine/dbal src/Schema/Collections/Exception/ObjectDoesNotExist.php

use crate::exception::Exception;

use super::array_map::ArrayMap;
use super::exception;
use super::name::UnqualifiedName;

/// `NamedObject<UnqualifiedName>`.
pub trait NamedObject {
    fn named_object_name(&self) -> Result<UnqualifiedName, Exception>;
}

/// `OptionallyNamedObject<UnqualifiedName>`.
pub trait OptionallyNamedObject {
    fn optional_object_name(&self) -> Result<Option<UnqualifiedName>, Exception>;
}

/// What a set operation threw.
#[derive(Debug, Clone, PartialEq)]
pub enum CollectionError {
    /// `ObjectAlreadyExists`.
    AlreadyExists(UnqualifiedName),
    /// `ObjectDoesNotExist`.
    DoesNotExist(UnqualifiedName),
    /// Another exception (an element's name could not be read, or the
    /// modification threw).
    Other(Exception),
}

impl CollectionError {
    /// The thrown exception.
    pub fn exception(&self) -> Exception {
        match self {
            Self::AlreadyExists(name) => exception::object_already_exists(name),
            Self::DoesNotExist(name) => exception::object_does_not_exist(name),
            Self::Other(e) => e.clone(),
        }
    }
}

impl From<Exception> for CollectionError {
    fn from(e: Exception) -> Self {
        Self::Other(e)
    }
}

impl From<CollectionError> for Exception {
    fn from(e: CollectionError) -> Self {
        e.exception()
    }
}

fn get_key(name: &UnqualifiedName) -> String {
    name.get_identifier().get_value().to_ascii_lowercase()
}

/// A set of objects where each object is uniquely identified by its
/// unqualified name.
#[derive(Debug, Clone, PartialEq)]
pub struct UnqualifiedNamedObjectSet<E> {
    elements: ArrayMap<E>,
}

impl<E> Default for UnqualifiedNamedObjectSet<E> {
    fn default() -> Self {
        Self {
            elements: ArrayMap::new(),
        }
    }
}

impl<E: NamedObject> UnqualifiedNamedObjectSet<E> {
    pub fn new(elements: impl IntoIterator<Item = E>) -> Result<Self, CollectionError> {
        let mut set = Self::default();
        for element in elements {
            set.add(element)?;
        }
        Ok(set)
    }

    pub fn is_empty(&self) -> bool {
        self.elements.is_empty()
    }

    pub fn get(&self, name: &UnqualifiedName) -> Option<&E> {
        self.elements.get(&get_key(name))
    }

    pub fn add(&mut self, element: E) -> Result<(), CollectionError> {
        let name = element.named_object_name()?;
        let key = get_key(&name);
        if self.elements.contains_key(&key) {
            return Err(CollectionError::AlreadyExists(name));
        }
        self.elements.insert(key, element);
        Ok(())
    }

    pub fn remove(&mut self, name: &UnqualifiedName) -> Result<(), CollectionError> {
        match self.elements.remove(&get_key(name)) {
            Some(_) => Ok(()),
            None => Err(CollectionError::DoesNotExist(name.clone())),
        }
    }

    pub fn modify(
        &mut self,
        name: &UnqualifiedName,
        modification: impl FnOnce(&E) -> Result<E, Exception>,
    ) -> Result<(), CollectionError> {
        let old_key = get_key(name);
        let Some(current) = self.elements.get(&old_key) else {
            return Err(CollectionError::DoesNotExist(name.clone()));
        };
        let element = modification(current)?;
        let element_name = element.named_object_name()?;
        let new_key = get_key(&element_name);
        if new_key == old_key {
            self.elements.insert(old_key, element);
            return Ok(());
        }
        if self.elements.contains_key(&new_key) {
            return Err(CollectionError::AlreadyExists(element_name));
        }
        self.elements.rekey(&old_key, new_key, element);
        Ok(())
    }

    pub fn clear(&mut self) {
        self.elements.clear();
    }

    pub fn to_list(&self) -> Vec<&E> {
        self.elements.values().collect()
    }

    pub fn iter(&self) -> impl Iterator<Item = &E> {
        self.elements.values()
    }
}

/// A set of objects where each object is uniquely identified by its
/// unqualified name, if it has one; unnamed objects are kept in order.
#[derive(Debug, Clone, PartialEq)]
pub struct OptionallyUnqualifiedNamedObjectSet<E> {
    elements: Vec<E>,
    positions_by_key: Vec<(String, usize)>,
}

impl<E> Default for OptionallyUnqualifiedNamedObjectSet<E> {
    fn default() -> Self {
        Self {
            elements: Vec::new(),
            positions_by_key: Vec::new(),
        }
    }
}

impl<E: OptionallyNamedObject> OptionallyUnqualifiedNamedObjectSet<E> {
    pub fn new(elements: impl IntoIterator<Item = E>) -> Result<Self, CollectionError> {
        let mut set = Self::default();
        for element in elements {
            set.add(element)?;
        }
        Ok(set)
    }

    fn position(&self, key: &str) -> Option<usize> {
        self.positions_by_key
            .iter()
            .find(|(k, _)| k == key)
            .map(|(_, p)| *p)
    }

    pub fn is_empty(&self) -> bool {
        self.elements.is_empty()
    }

    pub fn get(&self, name: &UnqualifiedName) -> Option<&E> {
        self.position(&get_key(name)).map(|p| &self.elements[p])
    }

    pub fn add(&mut self, element: E) -> Result<(), CollectionError> {
        if let Some(name) = element.optional_object_name()? {
            let key = get_key(&name);
            if self.position(&key).is_some() {
                return Err(CollectionError::AlreadyExists(name));
            }
            self.positions_by_key.push((key, self.elements.len()));
        }
        self.elements.push(element);
        Ok(())
    }

    pub fn remove(&mut self, name: &UnqualifiedName) -> Result<(), CollectionError> {
        let key = get_key(name);
        let Some(position) = self.position(&key) else {
            return Err(CollectionError::DoesNotExist(name.clone()));
        };
        self.elements.remove(position);
        self.positions_by_key.retain(|(k, _)| *k != key);
        for (_, p) in &mut self.positions_by_key {
            if *p > position {
                *p -= 1;
            }
        }
        Ok(())
    }

    pub fn modify(
        &mut self,
        name: &UnqualifiedName,
        modification: impl FnOnce(&E) -> Result<E, Exception>,
    ) -> Result<(), CollectionError> {
        let old_key = get_key(name);
        let Some(position) = self.position(&old_key) else {
            return Err(CollectionError::DoesNotExist(name.clone()));
        };
        let element = modification(&self.elements[position])?;
        match element.optional_object_name()? {
            Some(element_name) => {
                let new_key = get_key(&element_name);
                if new_key != old_key {
                    if self.position(&new_key).is_some() {
                        return Err(CollectionError::AlreadyExists(element_name));
                    }
                    self.positions_by_key.retain(|(k, _)| *k != old_key);
                    self.positions_by_key.push((new_key, position));
                }
            }
            None => self.positions_by_key.retain(|(k, _)| *k != old_key),
        }
        self.elements[position] = element;
        Ok(())
    }

    pub fn clear(&mut self) {
        self.elements.clear();
        self.positions_by_key.clear();
    }

    pub fn to_list(&self) -> Vec<&E> {
        self.elements.iter().collect()
    }

    pub fn iter(&self) -> impl Iterator<Item = &E> {
        self.elements.iter()
    }
}
