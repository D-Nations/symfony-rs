//! Port of Symfony\Component\DependencyInjection\Dumper\DumperInterface.php.
//!
//! Upstream returns `string|array`; the array form is `PhpDumper`'s
//! (one file per class, with `as_files`), which is not ported, so a
//! dump here is a string.
//!
//! Upstream: symfony/dependency-injection Dumper/DumperInterface.php

use php_value::{PhpArray, PhpString};

use crate::exception::Result;

/// `DumperInterface`: what a service container dumper implements.
pub trait DumperInterface {
    /// `dump($options)`: dumps the service container.
    fn dump(&mut self, options: &PhpArray) -> Result<PhpString>;
}
