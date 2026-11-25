//! `OptionsResolver` behaviour its upstream tests leave open, checked
//! against `tests/data/edges.json`, recorded from PHP 8.4.27 by
//! `oracle/recorders/options_resolver_edges.php` (`scripts/record.sh
//! options-resolver`). Each case here builds the same resolver as the
//! recorder's case of the same name.
//!
//! Upstream: none

use std::sync::Arc;

use php_value::{
    PhpArray, Value,
    datetime::{DateTimeImmutable, Fields, TimeZone},
    object::{ArrayObject, StdClass},
    php_array,
    vectors::{Outcome, outcome},
};
use symfony_options_resolver::{
    AllowedValues, Class, DefaultValue, Error, NestedClosure, OptionsResolver, php::ClosureValue,
};

type Case = Result<Value, Error>;

fn nested(
    f: impl Fn(&OptionsResolver, &OptionsResolver) -> Result<(), Error> + Send + Sync + 'static,
) -> NestedClosure {
    NestedClosure::new(f)
}

fn resolved(r: &OptionsResolver, options: PhpArray) -> Case {
    r.resolve(&options).map(Value::Array)
}

/// The recorder's `$o['x']` in a lazy closure, returning a caught
/// `NoSuchOptionException`'s message.
fn catching_lazy() -> DefaultValue {
    DefaultValue::lazy(|o| match o.offset_get("x") {
        Ok(v) => Ok(v),
        Err(e) if e.is_a(&Class::NoSuchOption) => Ok(Value::String(e.message().clone())),
        Err(e) => Err(e),
    })
}

fn run(name: &str) -> Option<Case> {
    let r = OptionsResolver::new();
    Some((|| -> Case {
        match name {
            "parents_shift_after_caught_error" => {
                r.set_options(
                    "foo",
                    nested(|n, _| {
                        n.set_defined("x")?;
                        n.set_default("a", catching_lazy())?;
                        n.set_default("b", 1)?;
                        n.set_allowed_types("b", "string")?;
                        Ok(())
                    }),
                )?;
                resolved(&r, PhpArray::new())
            }
            "parents_shift_caught_message" => {
                r.set_options(
                    "foo",
                    nested(|n, _| {
                        n.set_defined("x")?;
                        n.set_default("a", catching_lazy())?;
                        Ok(())
                    }),
                )?;
                resolved(&r, PhpArray::new())
            }
            "two_levels_prefix" => {
                r.set_options(
                    "a",
                    nested(|n, _| {
                        n.set_options(
                            "b",
                            nested(|m, _| {
                                m.set_required(["y", "x"])?;
                                Ok(())
                            }),
                        )?;
                        Ok(())
                    }),
                )?;
                resolved(&r, PhpArray::new())
            }
            "prototype_in_prototype_prefix" => {
                r.set_options(
                    "a",
                    nested(|n, _| {
                        n.set_prototype(true)?;
                        n.set_options(
                            "b",
                            nested(|m, _| {
                                m.set_defined("c")?;
                                m.set_allowed_types("c", "int")?;
                                Ok(())
                            }),
                        )?;
                        Ok(())
                    }),
                )?;
                resolved(
                    &r,
                    php_array!["a" => php_array!["k" => php_array!["b" => php_array!["c" => "no"]]]],
                )
            }
            "undefined_sorted" => {
                r.set_defined(["z", "3", "B", "a"])?;
                let mut options = PhpArray::new();
                for key in ["b", "10", "a1", "2", "1.5", "x1", "1e1", " 3", "-1"] {
                    options.insert(key, 1.into());
                }
                resolved(&r, options)
            }
            "missing_sorted" => {
                r.set_required(["b", "10", "a", "2", "1.5", " 3"])?;
                resolved(&r, PhpArray::new())
            }
            "int_option_names" => {
                r.set_defaults_array(&php_array!["foo", "bar"])?;
                resolved(&r, php_array!["1" => "baz"])
            }
            "allowed_values_merge_string_keys" => {
                r.set_default("o", "none")?;
                r.set_allowed_values("o", php_array!["a" => "x", 0 => "y"])?;
                r.add_allowed_values("o", php_array!["a" => "z", 0 => "w"])?;
                resolved(&r, PhpArray::new())
            }
            "format_values" => {
                r.set_default("o", 0.1 + 0.2)?;
                let mut values = php_array![
                    1.0,
                    -0.0,
                    1e100,
                    1.5e-7,
                    f64::INFINITY,
                    f64::NEG_INFINITY,
                    f64::NAN,
                    7,
                    true
                ];
                let _ = values.push(Value::Null);
                let _ = values.push(Value::Array(PhpArray::new()));
                let _ = values.push(StdClass::object(PhpArray::new()));
                r.set_allowed_values("o", AllowedValues::from(Value::Array(values)))?;
                resolved(&r, PhpArray::new())
            }
            "deprecation_name_twice" => {
                r.set_defined("o")?;
                r.set_deprecated("o", "", "", "%name% and %name%%name")?;
                resolved(&r, php_array!["o" => 1])
            }
            "deprecation_undefined" => {
                r.set_defined(["b", "a"])?;
                r.set_deprecated("c", "p", "1", "x")?;
                Ok(Value::Null)
            }
            "nested_closure_returns_array_prototype_non_array" => {
                r.set_options(
                    "a",
                    nested(|n, _| {
                        n.set_prototype(true)?;
                        Ok(())
                    }),
                )?;
                resolved(
                    &r,
                    php_array!["a" => php_array!["x" => PhpArray::new(), "y" => 3]],
                )
            }
            "ignore_undefined_with_required" => {
                r.set_ignore_undefined(true);
                r.set_required("r")?;
                resolved(&r, php_array!["x" => 1])
            }
            _ => type_case(&r, name).unwrap_or(Err(Error::other("unknown", name))),
        }
    })())
}

fn type_value(label: &str) -> Option<Value> {
    Some(match label {
        "int" => 1.into(),
        "float" => 1.5.into(),
        "numeric_string" => " 1".into(),
        "trailing_space" => "1 ".into(),
        "exponent" => "1e3".into(),
        "hex" => "0x1A".into(),
        "word" => "abc".into(),
        "empty" => "".into(),
        "true" => true.into(),
        "null" => Value::Null,
        "array" => php_array![1].into(),
        "stdclass" => StdClass::object(PhpArray::new()),
        "arrayobject" => Value::Object(Arc::new(ArrayObject::new(PhpArray::new()))),
        "datetime" => Value::Object(Arc::new(DateTimeImmutable::from_fields(
            Fields {
                year: 1970,
                month: 1,
                day: 1,
                ..Fields::default()
            },
            TimeZone::Utc,
        ))),
        "closure" => ClosureValue::value(),
        _ => return None,
    })
}

/// `type <allowed type> <value label>`; an object result is `'ok'`.
fn type_case(r: &OptionsResolver, name: &str) -> Option<Case> {
    let rest = name.strip_prefix("type ")?;
    let (allowed_type, label) = rest.rsplit_once(' ')?;
    let value = type_value(label)?;
    Some((|| {
        r.set_default("o", value)?;
        r.set_allowed_types("o", allowed_type)?;
        let resolved = r.resolve(&PhpArray::new())?;
        Ok(match resolved.get(&"o".into()) {
            Some(Value::Object(_)) => "ok".into(),
            _ => Value::Array(resolved),
        })
    })())
}

#[test]
fn recorded_edges() {
    let json: serde_json::Value =
        serde_json::from_str(include_str!("data/edges.json")).expect("edges JSON");
    let cases = json.as_array().expect("cases");
    assert!(cases.len() > 300);
    for case in cases {
        let name = case["name"].as_str().expect("name");
        let expected = outcome(&case["outcome"]).expect("outcome");
        let expected_deprecations: Vec<String> = case["deprecations"]
            .as_array()
            .expect("deprecations")
            .iter()
            .map(|d| d.as_str().expect("message").to_owned())
            .collect();
        let (actual, deprecations) = symfony_deprecation_contracts::collect(|| run(name));
        let actual = actual.unwrap_or_else(|| panic!("no case {name}"));
        match (expected, actual) {
            (Outcome::Value(e), Ok(a)) => assert_eq!(e, a, "{name}"),
            (Outcome::Throws { class, message }, Err(a)) => {
                assert_eq!(class, a.class().name(), "{name}");
                assert_eq!(message, a.message().to_string_lossy(), "{name}");
            }
            (e, a) => panic!("{name}: expected {e:?}, got {a:?}"),
        }
        let deprecations: Vec<String> = deprecations
            .iter()
            .map(|d| String::from_utf8_lossy(&d.message).into_owned())
            .collect();
        assert_eq!(expected_deprecations, deprecations, "{name}");
    }
}
