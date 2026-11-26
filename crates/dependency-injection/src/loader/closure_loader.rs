//! Port of Symfony\Component\DependencyInjection\Loader\ClosureLoader.php:
//! loads a closure that configures the container.
//!
//! A PHP `\Closure` resource is a [`LoaderClosure`] here: a Rust closure
//! in a PHP object of class `Closure`, called with the container and the
//! environment. The container is locked while the closure runs, so the
//! closure must not import another resource into it.
//!
//! Symfony source copyright (c) Fabien Potencier; distributed under MIT.
//!
//! Upstream: symfony/dependency-injection Loader/ClosureLoader.php

use std::fmt;
use std::sync::{Arc, Weak};

use php_value::{PhpArray, Value, value::PhpObject};
use symfony_config::Error as ConfigError;
use symfony_config::loader::{Loader, LoaderInterface, LoaderResolverInterface};

use super::{SharedContainer, cfg, lock};
use crate::container_builder::ContainerBuilder;
use crate::exception::Result;
use crate::object::{downcast, object_value};

pub const CLASS: &str = "Symfony\\Component\\DependencyInjection\\Loader\\ClosureLoader";

type Callback = dyn Fn(&mut ContainerBuilder, Option<&str>) -> Result<Value> + Send + Sync;

/// A `\Closure` resource: `function (ContainerBuilder $container, ?string
/// $env)`.
#[derive(Clone)]
pub struct LoaderClosure(Arc<Callback>);

impl LoaderClosure {
    pub fn new(
        callback: impl Fn(&mut ContainerBuilder, Option<&str>) -> Result<Value> + Send + Sync + 'static,
    ) -> Self {
        Self(Arc::new(callback))
    }

    /// The closure as a PHP value.
    pub fn to_value(&self) -> Value {
        object_value(self.clone())
    }
}

impl fmt::Debug for LoaderClosure {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("Closure")
    }
}

impl PhpObject for LoaderClosure {
    #[expect(clippy::unnecessary_literal_bound, reason = "the trait's signature")]
    fn class(&self) -> &str {
        "Closure"
    }

    fn properties(&self) -> PhpArray {
        PhpArray::new()
    }
}

/// `ClosureLoader`.
pub struct ClosureLoader {
    loader: Loader,
    container: SharedContainer,
}

impl ClosureLoader {
    /// `new ClosureLoader($container, $env)`.
    pub fn new(container: SharedContainer, env: Option<&str>) -> Arc<Self> {
        Arc::new_cyclic(|this: &Weak<Self>| {
            let this: Weak<dyn LoaderInterface> = this.clone();
            Self {
                loader: Loader::new(env.map(str::as_bytes), this),
                container,
            }
        })
    }
}

impl LoaderInterface for ClosureLoader {
    fn class(&self) -> &str {
        CLASS
    }

    fn load(
        &self,
        resource: &Value,
        _type: Option<&[u8]>,
    ) -> std::result::Result<Value, ConfigError> {
        let Some(closure) = downcast::<LoaderClosure>(resource) else {
            return Err(ConfigError::new(
                symfony_config::Class::Error,
                format!("Value of type {} is not callable", resource.debug_type()),
            ));
        };
        let env = self
            .loader
            .env()
            .map(|e| String::from_utf8_lossy(e).into_owned());
        let mut container = lock(&self.container);
        (closure.0)(&mut container, env.as_deref()).map_err(cfg)
    }

    fn supports(&self, resource: &Value, _type: Option<&[u8]>) -> bool {
        downcast::<LoaderClosure>(resource).is_some()
    }

    fn get_resolver(&self) -> std::result::Result<Arc<dyn LoaderResolverInterface>, ConfigError> {
        self.loader.get_resolver()
    }

    fn set_resolver(&self, resolver: Arc<dyn LoaderResolverInterface>) {
        self.loader.set_resolver(resolver);
    }
}
