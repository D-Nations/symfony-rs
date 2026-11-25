//! A stand-in for `Composer\InstalledVersions::getVersion()`, which
//! `NodeDefinition::docUrl()` calls: the installed packages and their
//! normalized versions (`8.1.10.0`), registered by the application at
//! start-up (in PHP, `vendor/composer/installed.php`). Process-wide, as
//! Composer's data is.
//!
//! Composer's behaviour: an unknown package throws an
//! `OutOfBoundsException` ('Package "x" is not installed'); a package with
//! no version gives null.
//!
//! Upstream: none

use std::sync::{PoisonError, RwLock};

use crate::definition::exception::{Class, Error};

static INSTALLED: RwLock<Vec<(String, Option<String>)>> = RwLock::new(Vec::new());

/// Registers installed packages (name, normalized version), replacing
/// earlier entries of the same name.
pub fn set_installed_versions<'a>(packages: impl IntoIterator<Item = (&'a str, Option<&'a str>)>) {
    let mut installed = INSTALLED.write().unwrap_or_else(PoisonError::into_inner);
    for (name, version) in packages {
        let version = version.map(str::to_owned);
        match installed.iter_mut().find(|(n, _)| n == name) {
            Some((_, v)) => *v = version,
            None => installed.push((name.to_owned(), version)),
        }
    }
}

/// `InstalledVersions::getVersion($packageName)`.
pub fn get_version(package: &str) -> Result<Option<String>, Error> {
    let installed = INSTALLED.read().unwrap_or_else(PoisonError::into_inner);
    match installed.iter().find(|(n, _)| n == package) {
        Some((_, version)) => Ok(version.clone()),
        None => Err(Error::new(
            Class::Other {
                class: "OutOfBoundsException".to_owned(),
                exception: true,
            },
            format!("Package \"{package}\" is not installed"),
        )),
    }
}
