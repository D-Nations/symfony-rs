//! Port of Doctrine\DBAL\Tools\DsnParser.php: turns a database URL into
//! connection parameters for `DriverManager::getConnection()`.
//!
//! PHP's `is_a($driver, Driver::class, true)` knows every loaded class;
//! here a scheme maps to a driver class when it names one of the drivers
//! DBAL ships or one given to [`DsnParser::with_driver_class`].
//!
//! Doctrine source copyright (c) Doctrine Project; distributed under MIT.
//!
//! Upstream: doctrine/dbal src/Tools/DsnParser.php

use php_value::url::{Url, parse_url, rawurldecode};
use php_value::vars::parse_str;
use php_value::{Key, PhpArray, PhpString, Value};

use crate::Exception;
use crate::driver_manager::is_dbal_driver_class;

/// A DSN parser (`Doctrine\DBAL\Tools\DsnParser`).
#[derive(Debug, Clone, Default)]
pub struct DsnParser {
    /// The scheme mapping: a scheme (with dashes as underscores) to a
    /// driver name or class.
    scheme_mapping: Vec<(String, String)>,
    driver_classes: Vec<String>,
}

/// `preg_replace('#^((?:pdo-)?sqlite3?):///#', '$1://localhost/', $dsn)`.
fn sqlite_localhost(dsn: &[u8]) -> Vec<u8> {
    let mut scheme = 0;
    if dsn.starts_with(b"pdo-") {
        scheme = 4;
    }
    if !dsn[scheme..].starts_with(b"sqlite") {
        return dsn.to_vec();
    }
    scheme += 6;
    if dsn.get(scheme) == Some(&b'3') {
        scheme += 1;
    }
    if !dsn[scheme..].starts_with(b":///") {
        return dsn.to_vec();
    }
    let mut url = dsn[..scheme].to_vec();
    url.extend_from_slice(b"://localhost/");
    url.extend_from_slice(&dsn[scheme + 4..]);
    url
}

fn decoded(component: Option<&Vec<u8>>) -> Option<PhpString> {
    component.map(|value| PhpString::from(rawurldecode(value)))
}

impl DsnParser {
    pub fn new<S: Into<String>, T: Into<String>>(
        scheme_mapping: impl IntoIterator<Item = (S, T)>,
    ) -> Self {
        Self {
            scheme_mapping: scheme_mapping
                .into_iter()
                .map(|(scheme, driver)| (scheme.into(), driver.into()))
                .collect(),
            driver_classes: Vec::new(),
        }
    }

    /// Declares a class of the program that implements the `Driver`
    /// interface, which a scheme mapping may name.
    #[must_use]
    pub fn with_driver_class(mut self, class: impl Into<String>) -> Self {
        self.driver_classes.push(class.into());
        self
    }

    fn is_driver_class(&self, class: &[u8]) -> bool {
        is_dbal_driver_class(class) || self.driver_classes.iter().any(|c| c.as_bytes() == class)
    }

    /// Parses a database URL into connection parameters.
    pub fn parse(&self, dsn: &[u8]) -> Result<PhpArray, Exception> {
        // (pdo-)?sqlite3?:///... => (pdo-)?sqlite3?://localhost/... or
        // else the URL will be invalid
        let url = sqlite_localhost(dsn);
        let Some(url) = parse_url(&url) else {
            return Err(Exception::malformed_dsn());
        };
        let mut params = PhpArray::new();
        let mut driver: Option<PhpString> = None;
        if let Some(scheme) = decoded(url.scheme.as_ref()) {
            let value = self.parse_database_url_scheme(&scheme);
            params.insert("driver", Value::String(value.clone()));
            driver = Some(value);
        }
        let host = decoded(url.host.as_ref());
        if let Some(host) = &host {
            params.insert("host", Value::String(host.clone()));
        }
        if let Some(port) = url.port {
            params.insert("port", Value::Int(port));
        }
        if let Some(user) = decoded(url.user.as_ref()) {
            params.insert("user", Value::String(user));
        }
        if let Some(pass) = decoded(url.pass.as_ref()) {
            params.insert("password", Value::String(pass));
        }
        if let Some(class) = &driver
            && self.is_driver_class(class.as_bytes())
        {
            params.insert("driverClass", Value::String(class.clone()));
            params.remove(&Key::from("driver"));
            driver = None;
        }
        Self::parse_database_url_path(&url, &mut params, host.is_some(), driver.as_ref());
        Self::parse_database_url_query(&url, &mut params);
        Ok(params)
    }

    fn parse_database_url_path(
        url: &Url,
        params: &mut PhpArray,
        has_host: bool,
        driver: Option<&PhpString>,
    ) {
        let Some(mut path) = decoded(url.path.as_ref()) else {
            return;
        };
        if has_host {
            // Only normalize the path if a host is also available.
            // Otherwise we might trim leading slashes from a pure dbname.
            path = PhpString::from(path.as_bytes().get(1..).unwrap_or_default());
        }
        // If we do not have a known DBAL driver, we do not know any
        // connection URL path semantics to evaluate and therefore treat
        // the path as a regular DBAL connection URL path.
        let sqlite = driver.is_some_and(|driver| {
            driver
                .as_bytes()
                .windows(6)
                .any(|window| window == b"sqlite")
        });
        if !sqlite {
            params.insert("dbname", Value::String(path));
            return;
        }
        if path.as_bytes() == b":memory:" {
            params.insert("memory", Value::Bool(true));
            return;
        }
        // pdo_sqlite driver uses 'path' instead of 'dbname' key
        params.insert("path", Value::String(path));
    }

    fn parse_database_url_query(url: &Url, params: &mut PhpArray) {
        let Some(query) = decoded(url.query.as_ref()) else {
            return;
        };
        // simply ingest query as extra params, e.g. charset or sslmode
        for (key, value) in parse_str(query.as_bytes()).iter() {
            params.insert(key.clone(), value.clone());
        }
    }

    fn parse_database_url_scheme(&self, scheme: &PhpString) -> PhpString {
        // URL schemes must not contain underscores, but dashes are ok
        let driver: Vec<u8> = scheme
            .as_bytes()
            .iter()
            .map(|&byte| if byte == b'-' { b'_' } else { byte })
            .collect();
        // If the driver is an alias (e.g. "postgres"), map it to the
        // actual name ("pdo-pgsql").
        match self
            .scheme_mapping
            .iter()
            .find(|(scheme, _)| scheme.as_bytes() == driver.as_slice())
        {
            Some((_, mapped)) => PhpString::from(mapped.as_str()),
            None => PhpString::from(driver),
        }
    }
}
