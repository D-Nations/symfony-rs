//! Session/Storage/Handler/IdentityMarshallerTest.php at
//! symfony/http-foundation v8.1.11. Names and assertions are PHPUnit's;
//! each data set of a provider is a case of the same test.
//!
//! Upstream: symfony/http-foundation Tests/Session/Storage/Handler/IdentityMarshallerTest.php
#![allow(non_snake_case)]

use std::sync::Arc;

use php_value::{PhpArray, PhpString, Value, object::StdClass};
use symfony_http_foundation::{
    exception::Error,
    session::storage::handler::{
        identity_marshaller::IdentityMarshaller, marshaller_interface::MarshallerInterface,
    },
};

type TestResult = Result<(), Box<dyn std::error::Error>>;

#[test]
fn testMarshall() -> TestResult {
    let marshaller = IdentityMarshaller;
    let mut values = PhpArray::new();
    values.insert("data", Value::string("string_data"));
    let mut failed = PhpArray::new();
    let result = marshaller
        .marshall(&values, &mut failed)
        .map_err(|e| e.to_string())?;
    assert!(Value::Array(result).identical(&Value::Array(values)));
    Ok(())
}

#[test]
fn testMarshallInvalidData() -> TestResult {
    let mut object = PhpArray::new();
    object.insert("object", Value::Object(Arc::new(StdClass::default())));
    let mut array = PhpArray::new();
    array.insert("foo", Value::Array(PhpArray::list([Value::string("bar")])));
    for values in [object, array] {
        let marshaller = IdentityMarshaller;
        let mut failed = PhpArray::new();
        let error = marshaller
            .marshall(&values, &mut failed)
            .err()
            .ok_or("expected a LogicException")?;
        let error = error.downcast_ref::<Error>().ok_or("not an Error")?;
        assert!(error.is_a("LogicException"));
        assert!(String::from_utf8_lossy(error.message().as_bytes()).contains(
            "Symfony\\Component\\HttpFoundation\\Session\\Storage\\Handler\\IdentityMarshaller::marshall accepts only string as data"
        ));
    }
    Ok(())
}

#[test]
fn testUnmarshall() -> TestResult {
    let marshaller = IdentityMarshaller;
    assert_eq!(
        Value::string("data"),
        marshaller
            .unmarshall(&PhpString::from("data"))
            .map_err(|e| e.to_string())?
    );
    Ok(())
}
