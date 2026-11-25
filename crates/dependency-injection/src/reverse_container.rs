//! Port of Symfony\Component\DependencyInjection\ReverseContainer.php.
//!
//! The closure upstream binds into the container's scope to search its
//! `$services` and `$privates` is a crate-private lookup of [`Container`].
//! The reversible locator is any PSR-11 container; an error it returns
//! that is not this crate's [`Error`] keeps its message as an
//! `ErrorKind::Other("Throwable")`.
//!
//! Upstream: symfony/dependency-injection ReverseContainer.php

use std::sync::Arc;

use php_value::Value;

use crate::container::{Container, SERVICE_CONTAINER};
use crate::exception::{Error, ErrorKind, Result};

/// `ReverseContainer`: the id of a service object.
pub struct ReverseContainer {
    service_container: Arc<Container>,
    reversible_locator: Arc<dyn psr_container::ContainerInterface>,
    tag_name: String,
}

fn from_throwable(e: &psr_container::Throwable) -> Error {
    let error: &(dyn std::error::Error + 'static) = &**e;
    match error.downcast_ref::<Error>() {
        Some(error) => error.clone(),
        None => Error::new(ErrorKind::Other("Throwable".to_owned()), e.to_string()),
    }
}

impl ReverseContainer {
    /// `new ReverseContainer($serviceContainer, $reversibleLocator,
    /// $tagName)`; the tag defaults to `container.reversible`.
    pub fn new(
        service_container: Arc<Container>,
        reversible_locator: Arc<dyn psr_container::ContainerInterface>,
        tag_name: Option<&str>,
    ) -> Self {
        Self {
            service_container,
            reversible_locator,
            tag_name: tag_name.unwrap_or("container.reversible").to_owned(),
        }
    }

    /// `getId($service)`: the id of the object when it exists as a
    /// service. To be reversible, services need to be either public or be
    /// tagged with "container.reversible".
    pub fn get_id(&self, service: &Value) -> Option<String> {
        if self.service_container.is_self(service) {
            return Some(SERVICE_CONTAINER.to_owned());
        }
        let id = self.service_container.find_service_id(service)?;
        if self.service_container.has(&id) || self.reversible_locator.has(id.as_bytes()) {
            return Some(id);
        }
        None
    }

    /// `getService($id)`.
    pub fn get_service(&self, id: &str) -> Result<Value> {
        if self.reversible_locator.has(id.as_bytes()) {
            return self
                .reversible_locator
                .get(id.as_bytes())
                .map_err(|e| from_throwable(&e));
        }
        if self.service_container.is_removed_id(id) {
            return Err(Error::service_not_found(
                id,
                None,
                Vec::new(),
                Some(format!(
                    "The \"{id}\" service is private and cannot be accessed by reference. You should either make it public, or tag it as \"{}\".",
                    self.tag_name
                )),
            ));
        }
        self.service_container.get_service_value(id)
    }
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used)]

    use super::*;
    use crate::container::ContainerClass;
    use crate::service_locator::{LocatorEntry, ServiceLocator};
    use crate::service_object::ServiceObject;

    fn std_class() -> Value {
        ServiceObject::new("stdClass").into_value()
    }

    #[test]
    fn get_id_and_get_service() {
        let container = Container::from_class(
            None,
            ContainerClass {
                removed_ids: vec!["private".to_owned()],
                ..ContainerClass::default()
            },
        );
        let public = std_class();
        container.set("public", Some(public.clone())).unwrap();
        let tagged = container.store_private("tagged", std_class());
        let hidden = container.store_private("hidden", std_class());

        let locator = {
            let tagged = tagged.clone();
            ServiceLocator::new(vec![(
                "tagged".to_owned(),
                LocatorEntry::new(move |_| Ok(tagged.clone())),
            )])
        };
        let reverse = ReverseContainer::new(container.clone(), Arc::new(locator), None);

        assert_eq!(
            Some("service_container"),
            reverse.get_id(&container.to_value()).as_deref()
        );
        assert_eq!(Some("public"), reverse.get_id(&public).as_deref());
        assert_eq!(Some("tagged"), reverse.get_id(&tagged).as_deref());
        assert_eq!(None, reverse.get_id(&hidden));
        assert_eq!(None, reverse.get_id(&std_class()));

        assert!(reverse.get_service("tagged").unwrap().identical(&tagged));
        assert!(reverse.get_service("public").unwrap().identical(&public));
        let e = reverse.get_service("private").unwrap_err();
        assert!(e.is(&ErrorKind::ServiceNotFound));
        assert_eq!(
            "The \"private\" service is private and cannot be accessed by reference. You should either make it public, or tag it as \"container.reversible\".",
            e.message()
        );
    }
}
