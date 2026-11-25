//! CompiledRouteTest.php at symfony/routing
//! f3fefc6cad3748d538be970ca81b71f06d7aa8b3. Names and assertions are
//! PHPUnit's; the data provider's cases run in a loop. The
//! `__toString()` gadget is an object whose `__toString()` records that it
//! ran; `unserialize()` reaches `__unserialize()` as a Route's compiled
//! route does, through [`CompiledRoute::from_serialized`].
//!
//! Upstream: symfony/routing Tests/CompiledRouteTest.php
#![allow(non_snake_case)]
#![allow(
    clippy::unnecessary_wraps,
    reason = "test code ports PHPUnit tests and helpers as they are"
)]

mod common;

use std::sync::{
    Arc,
    atomic::{AtomicBool, Ordering},
};

use common::{TestResult, list, s};
use php_value::{
    PhpArray, PhpObject, PhpString, Value,
    constants::Registry,
    object::GenericObject,
    serialize::{Classes, serialize, unserialize},
};
use symfony_routing::compiled_route::{CLASS, CompiledRoute};

#[test]
fn testAccessors() {
    let compiled = CompiledRoute::new(
        "prefix",
        "regex",
        list(&["tokens"]),
        PhpArray::new(),
        None,
        PhpArray::new(),
        PhpArray::new(),
        list(&["variables"]),
    );
    assert_eq!(
        s("prefix"),
        *compiled.static_prefix(),
        "__construct() takes a static prefix as its second argument"
    );
    assert_eq!(
        s("regex"),
        *compiled.regex(),
        "__construct() takes a regexp as its third argument"
    );
    assert_eq!(
        list(&["tokens"]),
        *compiled.tokens(),
        "__construct() takes an array of tokens as its fourth argument"
    );
    assert_eq!(
        list(&["variables"]),
        *compiled.variables(),
        "__construct() takes an array of variables as its ninth argument"
    );
}

/// `CompiledRouteTestToStringGadget`.
#[derive(Debug)]
struct Gadget(Arc<AtomicBool>);

const GADGET: &str = "Symfony\\Component\\Routing\\Tests\\CompiledRouteTestToStringGadget";

impl PhpObject for Gadget {
    fn class(&self) -> &str {
        GADGET
    }

    fn properties(&self) -> PhpArray {
        PhpArray::new()
    }

    fn to_php_string(&self) -> Option<PhpString> {
        self.0.store(true, Ordering::SeqCst);
        Some(PhpString::default())
    }

    fn is_stringable(&self) -> bool {
        true
    }
}

#[test]
fn testUnserializeRejectsObjectInTypedScalarProperty() -> TestResult {
    // provideTrampolineProperties()
    for property in ["path_prefix", "path_regex", "host_regex"] {
        let empty = || Value::Array(PhpArray::new());
        let mut data: Vec<(&str, Value)> = vec![
            ("vars", empty()),
            ("path_prefix", Value::string("")),
            ("path_regex", Value::string("")),
            ("path_tokens", empty()),
            ("path_vars", empty()),
            ("host_regex", Value::Null),
            ("host_tokens", empty()),
            ("host_vars", empty()),
        ];
        let gadget = Value::Object(Arc::new(GenericObject::new(GADGET, PhpArray::new())));
        for (key, value) in &mut data {
            if *key == property {
                *value = gadget.clone();
            }
        }
        let mut payload = format!("O:{}:\"{CLASS}\":{}:{{", CLASS.len(), data.len()).into_bytes();
        for (key, value) in &data {
            payload.extend_from_slice(&serialize(&Value::string(*key)));
            payload.extend_from_slice(&serialize(value));
        }
        payload.push(b'}');
        let fired = Arc::new(AtomicBool::new(false));

        let mut classes = Classes::new();
        classes.allow_with_defaults(CLASS, PhpArray::new());
        let flag = fired.clone();
        classes.allow_with(GADGET, Arc::new(move |_| Arc::new(Gadget(flag.clone()))));
        let Some(Value::Object(object)) = unserialize(&payload, &classes, &Registry::empty())
        else {
            panic!("the payload does not unserialize");
        };
        match CompiledRoute::from_serialized(object.class(), &object.properties()) {
            Err(e) if e.is_a("BadMethodCallException") => {}
            other => panic!("Expected BadMethodCallException, got {other:?} ({property})"),
        }

        assert!(
            !fired.load(Ordering::SeqCst),
            "__toString gadget must not fire during unserialize"
        );
    }
    Ok(())
}
