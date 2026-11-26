//! Port of tests/DriverManagerTest.php. The `databaseUrlProvider` rows are
//! upstream's, recorded (tests/data/connection.json).
//!
//! Upstream: doctrine/dbal tests/DriverManagerTest.php
#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    reason = "test helpers; a panic is a test failure"
)]
#![allow(non_snake_case)]

mod connection_support;

use connection_support::{array, data, s};
use doctrine_dbal::tools::DsnParser;
use doctrine_dbal::{Connection, DriverManager};
use php_value::vectors::{decode, phpunit_equals};
use php_value::{Key, Value};

#[test]
fn testCheckParams() {
    let e = DriverManager::get_connection(array::<&str>([]), None).unwrap_err();
    assert!(e.is_instance_of("Doctrine\\DBAL\\Exception"));
}

#[test]
fn testInvalidDriver() {
    let e =
        DriverManager::get_connection(array([("driver", s("invalid_driver"))]), None).unwrap_err();
    assert!(e.is_instance_of("Doctrine\\DBAL\\Exception"));
}

#[test]
fn testDefaultWrapper() {
    let conn = DriverManager::get_connection(
        array([
            ("driver", s("pdo_sqlite")),
            ("memory", Value::Bool(true)),
            ("wrapperClass", s(Connection::CLASS)),
        ]),
        None,
    );
    assert!(conn.is_ok());
}

#[test]
fn testInvalidWrapperClass() {
    let e = DriverManager::get_connection(
        array([
            ("driver", s("pdo_sqlite")),
            ("memory", Value::Bool(true)),
            ("wrapperClass", s("stdClass")),
        ]),
        None,
    )
    .unwrap_err();
    assert!(e.is_instance_of("Doctrine\\DBAL\\Exception"));
}

#[test]
fn testInvalidDriverClass() {
    let e =
        DriverManager::get_connection(array([("driverClass", s("stdClass"))]), None).unwrap_err();
    assert!(e.is_instance_of("Doctrine\\DBAL\\Exception"));
}

#[test]
fn testValidDriverClass() {
    let driver_class = "Doctrine\\DBAL\\Driver\\PDO\\MySQL\\Driver";
    let conn =
        DriverManager::get_connection(array([("driverClass", s(driver_class))]), None).unwrap();
    assert_eq!(conn.get_driver().class(), driver_class);
}

#[test]
fn testDatabaseUrl() {
    let rows = data()["providers"]["DriverManagerTest"]["testDatabaseUrl"]["rows"]
        .as_array()
        .unwrap();
    assert!(!rows.is_empty());
    for row in rows {
        let name = row[0].as_str().unwrap();
        let args = row[1].as_array().unwrap();
        let Value::String(url) = decode(&args[0]).unwrap() else {
            panic!("url");
        };
        let Value::Array(mut params) = decode(&args[1]).unwrap() else {
            panic!("params");
        };
        let expected = decode(&args[2]).unwrap();
        let parser = DsnParser::new([("mysql", "pdo_mysql"), ("sqlite", "pdo_sqlite")]);
        // array_merge($params, $parser->parse($url))
        for (key, value) in parser.parse(url.as_bytes()).unwrap().iter() {
            params.insert(key.clone(), value.clone());
        }
        let conn = DriverManager::get_connection(params, None);
        let Value::Array(expected) = expected else {
            let e = conn.unwrap_err();
            assert!(e.is_instance_of("Doctrine\\DBAL\\Exception"), "{name}: {e}");
            continue;
        };
        let conn = conn.unwrap_or_else(|e| panic!("{name}: {e}"));
        let params = conn.get_params();
        for (key, value) in expected.iter() {
            if *key == Key::from("driver") || *key == Key::from("driverClass") {
                let class = value.as_string().unwrap().to_string_lossy().into_owned();
                assert_eq!(conn.get_driver().class(), class, "{name}");
            } else {
                let actual = params.get(key).cloned().unwrap_or(Value::Null);
                assert!(
                    phpunit_equals(value, &actual),
                    "{name}: {key:?} expected {value:?}, got {actual:?}"
                );
            }
        }
    }
}
