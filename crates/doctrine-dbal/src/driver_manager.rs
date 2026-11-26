//! Port of Doctrine\DBAL\DriverManager.php: builds a `Connection` from
//! connection parameters.
//!
//! PHP instantiates the class names it is given. Here a driver class is a
//! [`DriverClass`]: the name and how to build it. `pdo_sqlite` (and its
//! class, `Doctrine\DBAL\Driver\PDO\SQLite\Driver`) builds the sqlx `SQLite`
//! driver; the other drivers of the map are known by name but not ported:
//! their connection can be built, as PHP builds it without connecting, and
//! refuses to connect. A program's own driver class is given to
//! [`DriverManager::get_connection_with`]. A wrapper class other than
//! `Doctrine\DBAL\Connection` is refused, as Rust has no subclasses
//! (README.md, Differences). Middlewares are not ported.
//!
//! Doctrine source copyright (c) Doctrine Project; distributed under MIT.
//!
//! Upstream: doctrine/dbal src/DriverManager.php

use std::sync::Arc;

use php_value::{Key, PhpArray, PhpString, Value};

use crate::driver::api::ExceptionConverter;
use crate::driver::{BoxFuture, Connection as DriverConnection, Driver, PlatformBox, pdo_sqlite};
use crate::{Configuration, Connection, Exception};
use crate::{Query, ServerVersionProvider};

/// A driver class name and how `new $driverClass()` builds it.
#[derive(Clone, Copy)]
pub struct DriverClass {
    pub name: &'static str,
    pub construct: fn() -> Arc<dyn Driver>,
}

impl std::fmt::Debug for DriverClass {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_tuple("DriverClass").field(&self.name).finish()
    }
}

/// The class of the `SQLite` driver.
pub const PDO_SQLITE_DRIVER: &str = "Doctrine\\DBAL\\Driver\\PDO\\SQLite\\Driver";

/// List of supported drivers and their mappings to the driver classes.
const DRIVER_MAP: [(&str, &str); 11] = [
    ("pdo_mysql", "Doctrine\\DBAL\\Driver\\PDO\\MySQL\\Driver"),
    ("pdo_sqlite", PDO_SQLITE_DRIVER),
    ("pdo_pgsql", "Doctrine\\DBAL\\Driver\\PDO\\PgSQL\\Driver"),
    ("pdo_oci", "Doctrine\\DBAL\\Driver\\PDO\\OCI\\Driver"),
    ("oci8", "Doctrine\\DBAL\\Driver\\OCI8\\Driver"),
    ("ibm_db2", "Doctrine\\DBAL\\Driver\\IBMDB2\\Driver"),
    ("pdo_sqlsrv", "Doctrine\\DBAL\\Driver\\PDO\\SQLSrv\\Driver"),
    ("mysqli", "Doctrine\\DBAL\\Driver\\Mysqli\\Driver"),
    ("pgsql", "Doctrine\\DBAL\\Driver\\PgSQL\\Driver"),
    ("sqlsrv", "Doctrine\\DBAL\\Driver\\SQLSrv\\Driver"),
    ("sqlite3", "Doctrine\\DBAL\\Driver\\SQLite3\\Driver"),
];

fn pdo_sqlite_driver() -> Arc<dyn Driver> {
    Arc::new(pdo_sqlite::Driver::new())
}

/// Whether the class is one of the drivers DBAL ships.
pub(crate) fn is_dbal_driver_class(class: &[u8]) -> bool {
    DRIVER_MAP
        .iter()
        .any(|(_, driver_class)| driver_class.as_bytes() == class)
}

/// `$params[$key] ?? null` as a `?string` argument: a value of another
/// type is PHP's `TypeError` under `strict_types`.
fn optional_string(
    params: &PhpArray,
    key: &str,
    position: usize,
    parameter: &str,
) -> Result<Option<PhpString>, Exception> {
    match params.get(&Key::from(key)) {
        None | Some(Value::Null) => Ok(None),
        Some(Value::String(value)) => Ok(Some(value.clone())),
        Some(other) => Err(Exception::new(
            Exception::TYPE_ERROR,
            format!(
                "Doctrine\\DBAL\\DriverManager::createDriver(): Argument #{position} (${parameter}) \
                 must be of type ?string, {} given",
                other.debug_type()
            ),
        )),
    }
}

/// Factory for creating `Doctrine\DBAL\Connection` instances.
#[derive(Debug)]
pub struct DriverManager;

impl DriverManager {
    /// Creates a connection object based on the specified parameters: a
    /// `driver` name of the map, or a `driverClass`.
    pub fn get_connection(
        params: PhpArray,
        config: Option<Configuration>,
    ) -> Result<Connection, Exception> {
        Self::get_connection_with(params, config, &[])
    }

    /// `getConnection()` where the program defines more driver classes
    /// that `driverClass` may name.
    pub fn get_connection_with(
        params: PhpArray,
        config: Option<Configuration>,
        driver_classes: &[DriverClass],
    ) -> Result<Connection, Exception> {
        let config = config.unwrap_or_default();
        let driver = Self::create_driver(
            optional_string(&params, "driver", 1, "driver")?,
            optional_string(&params, "driverClass", 2, "driverClass")?,
            driver_classes,
        )?;
        if let Some(wrapper_class) = params.get(&Key::from("wrapperClass")) {
            let is_connection = wrapper_class
                .as_string()
                .is_some_and(|class| class.as_bytes() == Connection::CLASS.as_bytes());
            if !is_connection {
                let name = wrapper_class.to_php_string().unwrap_or_default();
                return Err(Exception::invalid_wrapper_class(name.as_bytes()));
            }
        }
        Ok(Connection::new(params, driver, Some(config)))
    }

    /// Returns the list of supported drivers.
    pub fn get_available_drivers() -> Vec<&'static str> {
        DRIVER_MAP.iter().map(|(name, _)| *name).collect()
    }

    fn create_driver(
        driver: Option<PhpString>,
        driver_class: Option<PhpString>,
        driver_classes: &[DriverClass],
    ) -> Result<Arc<dyn Driver>, Exception> {
        let driver_class = match driver_class {
            None => {
                let Some(driver) = driver else {
                    return Err(Exception::driver_required(None));
                };
                match DRIVER_MAP
                    .iter()
                    .find(|(name, _)| name.as_bytes() == driver.as_bytes())
                {
                    Some((_, class)) => PhpString::from(*class),
                    None => {
                        return Err(Exception::unknown_driver(
                            driver.as_bytes(),
                            &Self::get_available_drivers(),
                        ));
                    }
                }
            }
            Some(driver_class) => driver_class,
        };
        if driver_class.as_bytes() == PDO_SQLITE_DRIVER.as_bytes() {
            return Ok(pdo_sqlite_driver());
        }
        if let Some(class) = driver_classes
            .iter()
            .find(|class| class.name.as_bytes() == driver_class.as_bytes())
        {
            return Ok((class.construct)());
        }
        if let Some((_, class)) = DRIVER_MAP
            .iter()
            .find(|(_, class)| class.as_bytes() == driver_class.as_bytes())
        {
            return Ok(Arc::new(NotPortedDriver { class }));
        }
        Err(Exception::invalid_driver_class(driver_class.as_bytes()))
    }
}

/// A driver of the map that is not ported: it has its class, and refuses
/// to connect or to give a platform.
struct NotPortedDriver {
    class: &'static str,
}

impl NotPortedDriver {
    fn refusal(&self) -> Exception {
        Exception::new(
            Exception::UNSUPPORTED,
            format!("the driver {} is not ported", self.class),
        )
    }
}

/// The converter of a driver that is not ported: it never has an
/// exception to convert, and keeps one as it is.
struct NoConversion;

impl ExceptionConverter for NoConversion {
    fn convert(&self, exception: Exception, _query: Option<Query>) -> Exception {
        exception
    }
}

impl Driver for NotPortedDriver {
    fn class(&self) -> &str {
        self.class
    }

    fn connect<'a>(
        &'a self,
        _params: &'a PhpArray,
    ) -> BoxFuture<'a, Result<Arc<dyn DriverConnection>, Exception>> {
        Box::pin(async move { Err(self.refusal()) })
    }

    fn get_database_platform<'a>(
        &'a self,
        _version_provider: &'a dyn ServerVersionProvider,
    ) -> BoxFuture<'a, Result<PlatformBox, Exception>> {
        Box::pin(async move { Err(self.refusal()) })
    }

    fn get_exception_converter(&self) -> Arc<dyn ExceptionConverter> {
        Arc::new(NoConversion)
    }
}
