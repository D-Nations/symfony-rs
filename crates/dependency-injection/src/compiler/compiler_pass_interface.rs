//! Port of Symfony\Component\DependencyInjection\Compiler\CompilerPassInterface.php.
//!
//! A pass is a [`CompilerPassInterface`] object behind a shared handle,
//! [`Pass`]: the pass list, the compiler and `ContainerBuilder` hold the
//! same object, as PHP holds the same instance (`assertSame()` on passes
//! is [`Pass::ptr_eq`]). The pass's class name is [`CompilerPassInterface::class`]
//! (`$pass::class`, which `Compiler::log()` writes).
//!
//! Upstream: symfony/dependency-injection Compiler/CompilerPassInterface.php

use std::any::Any;
use std::fmt;
use std::sync::{Arc, Mutex, MutexGuard, PoisonError};

use crate::container_builder::ContainerBuilder;
use crate::exception::Result;

/// `CompilerPassInterface`.
pub trait CompilerPassInterface: Any + Send {
    /// `$pass::class`.
    fn class(&self) -> &str;

    /// `process($container)`.
    fn process(&mut self, container: &mut ContainerBuilder) -> Result<()>;
}

/// A shared handle to a compiler pass.
#[derive(Clone)]
pub struct Pass(Arc<Mutex<dyn CompilerPassInterface>>);

impl fmt::Debug for Pass {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "{}#{:p}",
            self.class(),
            Arc::as_ptr(&self.0).cast::<()>()
        )
    }
}

impl Pass {
    pub fn new(pass: impl CompilerPassInterface) -> Self {
        Self(Arc::new(Mutex::new(pass)))
    }

    fn lock(&self) -> MutexGuard<'_, dyn CompilerPassInterface + 'static> {
        self.0.lock().unwrap_or_else(PoisonError::into_inner)
    }

    /// `$pass::class`.
    pub fn class(&self) -> String {
        self.lock().class().to_string()
    }

    /// `$pass->process($container)`.
    pub fn process(&self, container: &mut ContainerBuilder) -> Result<()> {
        self.lock().process(container)
    }

    /// `$a === $b`.
    pub fn ptr_eq(&self, other: &Self) -> bool {
        std::ptr::addr_eq(Arc::as_ptr(&self.0), Arc::as_ptr(&other.0))
    }

    /// Calls `f` with the pass, when it is a `T` (`instanceof`).
    pub fn with<T: CompilerPassInterface, R>(&self, f: impl FnOnce(&mut T) -> R) -> Option<R> {
        let mut guard = self.lock();
        let any: &mut dyn Any = &mut *guard;
        any.downcast_mut::<T>().map(f)
    }
}
