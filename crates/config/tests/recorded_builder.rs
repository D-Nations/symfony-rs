//! Replays tests/data/builder.json, recorded from the PHP oracle by
//! `oracle/recorders/config_builder.php` (`scripts/record.sh config`): trees
//! built by `ArrayNodeDefinition` from lists of steps (normalized types,
//! defaults, and processing of configs, which covers `canBeEnabled()` /
//! `canBeDisabled()`, `acceptAndWrap()`, default children, key attributes,
//! XML remapping and the checks of concrete and prototype nodes);
//! `thenInvalid()`'s `sprintf()` and `json_encode()`; `find()`; `min()` / `max()`;
//! `docUrl()`; and the `TypeErrors` of a definition still parented by its
//! builder and of the methods' return types. Each result must be
//! identical, and each exception must have the same class and message,
//! except that PHP's ", called in <file> on line <n>" is not reproduced.
//!
//! Upstream: none
#![allow(
    clippy::unnecessary_wraps,
    reason = "test code ports PHPUnit tests and helpers as they are"
)]

use php_value::{
    PhpArray, Value, base64,
    object::EnumCase,
    vectors::{Outcome, decode, outcome, phpunit_equals},
};
use serde_json::Value as Json;
use symfony_config::{
    Error, Processor, composer,
    definition::builder::{
        ArrayNodeDefinition, Builtin, ExprType, FloatNodeDefinition, IntegerNodeDefinition,
        NodeBuilder, NodeClass, class,
    },
};

type TestResult = Result<(), Box<dyn std::error::Error>>;

fn text(v: &Value) -> String {
    v.as_string()
        .map(|s| s.to_string_lossy().into_owned())
        .unwrap_or_default()
}

fn list(v: &Value) -> Vec<Value> {
    v.as_array()
        .map(|a| a.values().cloned().collect())
        .unwrap_or_default()
}

fn truthy(v: &Value) -> bool {
    matches!(v, Value::Bool(true))
}

fn expr_type(v: &Value) -> ExprType {
    match text(v).as_str() {
        "any" => ExprType::Any,
        "string" => ExprType::String,
        "null" => ExprType::Null,
        "array" => ExprType::Array,
        "bool" => ExprType::Bool,
        "int" => ExprType::Int,
        "backed-enum" => ExprType::BackedEnum,
        other => panic!("unknown type {other}"),
    }
}

fn wrapped(v: Value) -> Result<Value, Error> {
    let mut a = PhpArray::new();
    a.insert("wrapped", v);
    Ok(Value::Array(a))
}

/// The recorder's `apply()`.
#[expect(clippy::too_many_lines, reason = "one function, as upstream's")]
fn apply(steps: &Value) -> Result<ArrayNodeDefinition, Error> {
    let d = ArrayNodeDefinition::new("root");
    for step in list(steps) {
        let s = list(&step);
        match text(&s[0]).as_str() {
            "can_be_enabled" => {
                d.can_be_enabled(None)?;
            }
            "can_be_disabled" => {
                d.can_be_disabled(None)?;
            }
            "if" => {
                let e = d.before_normalization();
                match text(&s[1]).as_str() {
                    "string" => e.if_string(),
                    "null" => e.if_null(),
                    "array" => e.if_array(),
                    "true" => e.if_true(),
                    "false" => e.if_false(),
                    "always" => e.always(),
                    "empty" => e.if_empty(),
                    other => panic!("unknown if {other}"),
                };
                e.then(wrapped).end()?;
            }
            "accept" => {
                let types: Vec<ExprType> = list(&s[1]).iter().map(expr_type).collect();
                let key = s[2].as_string().map(|k| k.to_vec());
                d.accept_and_wrap(&types, key.as_deref())?;
            }
            "true_like" => {
                d.treat_true_like(s[1].clone());
            }
            "false_like" => {
                d.treat_false_like(s[1].clone());
            }
            "null_like" => {
                d.treat_null_like(s[1].clone());
            }
            "scalar_child" => {
                d.children()
                    .scalar_node(&text(&s[1]))?
                    .default_value(s[2].clone())?;
            }
            "list_child" => {
                d.children()
                    .array_node(&text(&s[1]), None)?
                    .scalar_prototype()?;
            }
            "prototype" => {
                d.prototype(&text(&s[1]))?;
            }
            "proto_array" => {
                d.array_prototype()?
                    .children()
                    .scalar_node("id")?
                    .end()
                    .scalar_node("v")?
                    .end();
            }
            "key" => {
                d.use_attribute_as_key(text(&s[1]), truthy(&s[2]));
            }
            "default_children" => {
                d.add_default_children_if_none_set(s[1].clone())?;
            }
            "default" => {
                d.default_value(s[1].clone())?;
            }
            "required" => {
                d.is_required()?;
            }
            "cannot_be_empty" => {
                d.cannot_be_empty()?;
            }
            "at_least_one" => {
                d.requires_at_least_one_element();
            }
            "defaults_if_not_set" => {
                d.add_defaults_if_not_set();
            }
            "xml" => {
                let plural = s[2].as_string().map(|p| p.to_vec());
                d.fix_xml_config(text(&s[1]), plural.as_deref());
            }
            "no_deep_merging" => {
                d.perform_no_deep_merging();
            }
            "ignore_extra" => {
                d.ignore_extra_keys(truthy(&s[1]));
            }
            "unset" => {
                d.can_be_unset(truthy(&s[1]));
            }
            "deny_overwrite" => {
                d.cannot_be_overwritten(true);
            }
            "no_new_keys" => {
                d.disallow_new_keys_in_subsequent_configs();
            }
            "normalize_keys" => {
                d.normalize_keys(truthy(&s[1]));
            }
            "deprecated" => {
                d.set_deprecated("vendor/pkg", "1.2", text(&s[1]));
            }
            other => panic!("unknown step {other}"),
        }
    }
    Ok(d)
}

fn find_tree(separator: &str) -> Result<ArrayNodeDefinition, Error> {
    let root = ArrayNodeDefinition::new("root");
    root.set_path_separator(separator)
        .children()
        .array_node("a", None)?
        .children()
        .boolean_node("b")?
        .end()
        .end()
        .end()
        .array_node("x.y", None)?
        .children()
        .scalar_node("z")?
        .end()
        .end()
        .end()
        .scalar_node("0")?
        .end()
        .array_node("q", None)?
        .end()
        .end();
    Ok(root)
}

fn swapped_builder(type_name: &str) -> NodeBuilder {
    let class = if type_name == "array" {
        Builtin::Scalar
    } else {
        Builtin::Array
    };
    NodeBuilder::new().set_node_class(type_name, NodeClass::Builtin(class))
}

/// Runs one recorded operation.
#[expect(clippy::too_many_lines, reason = "one function, as upstream's")]
fn run(op: &str, args: &[Value]) -> Result<Value, Error> {
    match op {
        "types" => Ok(Value::Array(PhpArray::list(
            apply(&args[0])?
                .get_node(false)?
                .get_normalized_types()
                .into_iter()
                .map(Value::String),
        ))),
        "default" => apply(&args[0])?.get_node(false)?.get_default_value(),
        "process" => {
            let node = apply(&args[0])?.get_node(false)?;
            let configs = args[1].as_array().cloned().unwrap_or_default();
            Processor::new().process(&node, &configs).map(Value::Array)
        }
        "then_invalid" => {
            let d = ArrayNodeDefinition::new("root");
            d.before_normalization()
                .always()
                .then_invalid(php_value_bytes(&args[0]))
                .end()?;
            d.get_node(false)?.normalize(args[1].clone())
        }
        "find" => {
            let found = find_tree(&text(&args[0]))?.find(php_value_bytes(&args[1]))?;
            Ok(Value::Array(PhpArray::list([
                Value::from(found.class()),
                found.name().map_or(Value::Null, Value::String),
            ])))
        }
        "bounds" => {
            let (a, b) = (number(&args[2]), number(&args[3]));
            let min_max = text(&args[1]) == "min_max";
            if text(&args[0]) == "integer" {
                let d = IntegerNodeDefinition::new("n");
                if min_max {
                    d.min(a)?.max(b)?;
                } else {
                    d.max(a)?.min(b)?;
                }
                d.get_node(false)?.finalize(Value::Int(4))
            } else {
                let d = FloatNodeDefinition::new("n");
                if min_max {
                    d.min(a)?.max(b)?;
                } else {
                    d.max(a)?.min(b)?;
                }
                d.get_node(false)?.finalize(Value::Float(4.0))
            }
        }
        "version" => Ok(composer::get_version(&text(&args[0]))?.map_or(Value::Null, Value::from)),
        "doc_url" => {
            let d = ArrayNodeDefinition::new("r");
            let package = args[1]
                .as_string()
                .map(|p| p.to_string_lossy().into_owned());
            d.doc_url(php_value_bytes(&args[0]), package.as_deref())?;
            d.property("attributes")
        }
        "parent_builder" => {
            let d = ArrayNodeDefinition::new("r")
                .children()
                .node("x", &text(&args[0]))?;
            if let Some(e) = d.downcast::<class::Enum>() {
                e.values(vec![Value::from("a")])?;
            }
            Ok(Value::from(d.get_node(false)?.class()))
        }
        "return_type" => {
            let type_name = text(&args[1]);
            let b = swapped_builder(&type_name);
            let class = match text(&args[0]).as_str() {
                "arrayNode" => b.array_node("a", None)?.class(),
                "scalarNode" => b.scalar_node("a")?.class(),
                "booleanNode" => b.boolean_node("a")?.class(),
                "integerNode" => b.integer_node("a")?.class(),
                "floatNode" => b.float_node("a")?.class(),
                "enumNode" => b.enum_node("a")?.class(),
                "variableNode" => b.variable_node("a")?.class(),
                "stringNode" => b.string_node("a")?.class(),
                other => panic!("unknown method {other}"),
            };
            Ok(Value::from(class))
        }
        "prototype_return_type" => {
            let d = ArrayNodeDefinition::new("r");
            for b in Builtin::ALL {
                let class = if b == Builtin::Array {
                    Builtin::Scalar
                } else {
                    Builtin::Array
                };
                d.children()
                    .set_node_class(b.type_name(), NodeClass::Builtin(class));
            }
            let class = match text(&args[0]).as_str() {
                "variablePrototype" => d.variable_prototype()?.class(),
                "scalarPrototype" => d.scalar_prototype()?.class(),
                "stringPrototype" => d.string_prototype()?.class(),
                "booleanPrototype" => d.boolean_prototype()?.class(),
                "integerPrototype" => d.integer_prototype()?.class(),
                "floatPrototype" => d.float_prototype()?.class(),
                "arrayPrototype" => d.array_prototype()?.class(),
                "enumPrototype" => d.enum_prototype()?.class(),
                other => panic!("unknown method {other}"),
            };
            Ok(Value::from(class))
        }
        other => panic!("unknown op {other}"),
    }
}

fn php_value_bytes(v: &Value) -> Vec<u8> {
    v.as_string().map(|s| s.to_vec()).unwrap_or_default()
}

fn number(v: &Value) -> php_value::Number {
    match v {
        Value::Int(i) => php_value::Number::Int(*i),
        Value::Float(f) => php_value::Number::Float(*f),
        other => panic!("not a number: {other:?}"),
    }
}

/// PHP's `TypeError` message without ", called in <file> on line <n>".
fn without_called_in(message: &str) -> &str {
    message
        .find(", called in ")
        .map_or(message, |i| &message[..i])
}

#[test]
fn recorded_builder_cases() -> TestResult {
    let json: Vec<Json> = serde_json::from_str(include_str!("data/builder.json"))?;

    // Composer's versions, as the oracle reported them.
    for case in json.iter().filter(|c| c[0] == "version") {
        let package = decode(&case[1][0]).map_err(|e| e.to_string())?;
        let version = decode(&case[2]).map_err(|e| e.to_string())?;
        let version = version
            .as_string()
            .map(|v| v.to_string_lossy().into_owned());
        composer::set_installed_versions([(text(&package).as_str(), version.as_deref())]);
    }

    let mut failures = Vec::new();
    for case in &json {
        let op = case[0].as_str().ok_or("op")?;
        let args = case[1]
            .as_array()
            .ok_or("args")?
            .iter()
            .map(decode)
            .collect::<Result<Vec<_>, _>>()
            .map_err(|e| e.to_string())?;
        let expected = if case[2][0] == "throws_base64" {
            Outcome::Throws {
                class: case[2][1].as_str().ok_or("class")?.to_owned(),
                message: String::new(),
            }
        } else {
            outcome(&case[2]).map_err(|e| e.to_string())?
        };
        let expected_bytes: Option<Vec<u8>> = if case[2][0] == "throws_base64" {
            Some(
                base64::decode_strict(case[2][2].as_str().ok_or("message")?.as_bytes())
                    .ok_or("base64")?,
            )
        } else {
            None
        };
        let actual = run(op, &args);
        let same = match (&expected, &actual) {
            (Outcome::Value(e @ Value::Object(o)), Ok(a))
                if o.downcast_ref::<EnumCase>().is_none() =>
            {
                phpunit_equals(e, a)
            }
            (Outcome::Value(e), Ok(a)) => e == a,
            (Outcome::Throws { class, message }, Err(e)) => {
                e.class().name() == class
                    && match &expected_bytes {
                        Some(bytes) => e.get_message() == bytes.as_slice(),
                        None => e.get_message() == without_called_in(message).as_bytes(),
                    }
            }
            _ => false,
        };
        if !same {
            let actual = match &actual {
                Ok(v) => format!("{v:?}"),
                Err(e) => e.to_string(),
            };
            failures.push(format!(
                "{op} {args:?}\n  PHP:  {expected:?}\n  Rust: {actual}"
            ));
        }
    }
    assert!(
        failures.is_empty(),
        "{} of {} recorded cases differ:\n{}",
        failures.len(),
        json.len(),
        failures.join("\n")
    );
    Ok(())
}
