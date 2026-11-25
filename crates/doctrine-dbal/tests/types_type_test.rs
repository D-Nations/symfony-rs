//! Port of tests/Types/TypeTest.php (and its fixture TypeWithConstructor.php) at doctrine/dbal
//! c435cd7ea33e17d2a89288caf994cf3d4549b879. Names and assertions are
//! PHPUnit's; `createStub()`/`createMock()` platforms are
//! `types_support::StubPlatform`.
//!
//! Upstream: doctrine/dbal tests/Types/TypeTest.php tests/Types/TypeWithConstructor.php

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    reason = "test helpers; a panic is a test failure"
)]
#![allow(non_snake_case)]

mod common;

use std::sync::Arc;

use common::{bytes, provider};
use doctrine_dbal::exception::Exception;
use doctrine_dbal::platforms::AbstractPlatform;
use doctrine_dbal::types::{
    Type, TypeArgument, TypeClass, TypesException, add_type, get_type, has_type, lookup_name,
};
use php_value::{PhpArray, PhpString, Thrown};

/// `TypeWithConstructor`: a type whose constructor requires an argument.
#[derive(Debug)]
struct TypeWithConstructor {
    #[allow(dead_code, reason = "the fixture's public property")]
    requirement: bool,
}

impl TypeWithConstructor {
    const CLASS: TypeClass = TypeClass {
        name: "Doctrine\\DBAL\\Tests\\Types\\TypeWithConstructor",
        // `new $class()`: PHP's ArgumentCountError.
        construct: || {
            Err(Thrown::new(
                "ArgumentCountError",
                "Too few arguments to function Doctrine\\DBAL\\Tests\\Types\\TypeWithConstructor::__construct(), 0 passed and exactly 1 expected",
                0,
            ))
        },
    };
}

impl Type for TypeWithConstructor {
    fn class(&self) -> &'static str {
        "Doctrine\\DBAL\\Tests\\Types\\TypeWithConstructor"
    }

    fn get_sql_declaration(
        &self,
        _column: &PhpArray,
        _platform: &dyn AbstractPlatform,
    ) -> Result<PhpString, Exception> {
        Ok(PhpString::default())
    }
}

fn default_types() -> Vec<String> {
    let rows = provider("Types/TypeTest", "testDefaultTypesAreRegistered");
    rows.into_iter()
        .map(|(_, args)| String::from_utf8(bytes(&args[0])).unwrap())
        .collect()
}

#[test]
fn testDefaultTypesAreRegistered() {
    let names = default_types();
    assert_eq!(names.len(), 31);
    assert_eq!(names, doctrine_dbal::types::Types::ALL);
    for name in names {
        assert!(has_type(&name), "{name}");
    }
}

#[test]
fn testDefaultTypesReverseLookup() {
    let rows = provider("Types/TypeTest", "testDefaultTypesReverseLookup");
    assert_eq!(rows.len(), 31);
    for (_, args) in rows {
        let name = String::from_utf8(bytes(&args[0])).unwrap();
        let type_ = get_type(&name).unwrap();
        assert_eq!(lookup_name(&type_).unwrap(), name);
    }
}

#[test]
fn testAddTypeWhenTypeRequiresArguments() {
    let error = add_type("some_type", TypeArgument::Class(TypeWithConstructor::CLASS)).unwrap_err();

    assert!(matches!(
        error,
        TypesException::TypeArgumentCountError { .. }
    ));
    assert_eq!(
        error.message(),
        "To register \"some_type\" pass an instance to `Type::addType` instead."
    );
}

#[test]
fn testAddTypeInstance() {
    assert!(!has_type("some_type"));
    add_type(
        "some_type",
        TypeArgument::Instance(Arc::new(TypeWithConstructor { requirement: true })),
    )
    .unwrap();
    assert!(has_type("some_type"));
}
