//! Replays tests/data/definition.json, recorded from the PHP oracle by
//! oracle/recorders/config_definition.php (`scripts/record.sh config`):
//! the Definition nodes on generated inputs (type checks, empty values,
//! minimum and maximum, enums and their casts, unrecognized options,
//! dashed keys, XML singulars, attribute keys, prototyped merges) and the
//! PHP built-ins written from documentation (`levenshtein()`, `<=>`,
//! `sort()`). Each result must be identical, and each exception must have
//! the same class and message.
//!
//! Upstream: none

use php_value::{
    Key, Number, PhpArray, Value,
    constants::Registry,
    object::EnumCase,
    vectors::{Outcome, decode, outcome, phpunit_equals},
};
use serde_json::Value as Json;
use symfony_config::{
    Error, Node, Processor,
    definition::{
        ArrayNode, BooleanNode, EnumNode, FloatNode, IntegerNode, NumericNode, PrototypedArrayNode,
        ScalarNode, StringNode, VariableNode,
    },
    php,
};

type TestResult = Result<(), Box<dyn std::error::Error>>;

const NS: &str = "Oracle\\Config";

fn enums() -> Registry {
    let mut registry = Registry::empty();
    let class = |name: &str| format!("{NS}\\{name}");
    let unit = class("UnitEnum");
    registry.define_enum(
        &unit,
        ["Foo", "Bar"]
            .iter()
            .map(|c| EnumCase::new(unit.clone(), *c, None))
            .collect(),
    );
    let int = class("IntEnum");
    registry.define_enum(
        &int,
        [("One", 1), ("Two", 2), ("Neg", -3)]
            .iter()
            .map(|(c, v)| EnumCase::new(int.clone(), *c, Some(Value::Int(*v))))
            .collect(),
    );
    let string = class("StrEnum");
    registry.define_enum(
        &string,
        [
            ("Foo", "foo"),
            ("One", "1"),
            ("Empty", ""),
            ("Slash", "a/b"),
            ("Accent", "é"),
        ]
        .iter()
        .map(|(c, v)| EnumCase::new(string.clone(), *c, Some(Value::from(*v))))
        .collect(),
    );
    registry
}

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

fn array(v: &Value) -> PhpArray {
    v.as_array().cloned().unwrap_or_default()
}

fn number(v: &Value) -> Option<Number> {
    match v {
        Value::Int(i) => Some(Number::Int(*i)),
        Value::Float(f) => Some(Number::Float(*f)),
        _ => None,
    }
}

fn scalar_node(kind: &str) -> Result<Node, Error> {
    match kind {
        "variable" => VariableNode::new("test", None),
        "scalar" => ScalarNode::new("test", None),
        "boolean" => BooleanNode::new("test", None),
        "boolean_nullable" => BooleanNode::with_options("test", None, b".", true),
        "integer" => IntegerNode::new("test", None, None, None),
        "float" => FloatNode::new("test", None, None, None),
        "string" => StringNode::new("test", None),
        "numeric" => NumericNode::new("test", None, None, None),
        other => panic!("unknown kind {other}"),
    }
}

fn enum_node(by: &str, spec: &Value, enums: &Registry) -> Result<Node, Error> {
    match by {
        "values" => EnumNode::new("e", None, list(spec)),
        _ => EnumNode::of_enum("e", None, &text(spec), enums),
    }
}

fn keyed(remove: bool) -> Result<Node, Error> {
    let node = PrototypedArrayNode::new("root", None)?;
    node.set_key_attribute(b"id", remove)?;
    let prototype = ArrayNode::new("", Some(&node))?;
    for name in ["id", "foo", "value"] {
        prototype.add_child(&ScalarNode::new(name, None)?)?;
    }
    node.set_prototype(&prototype)?;
    Ok(node)
}

/// Runs one recorded operation.
fn run(op: &str, args: &[Value], enums: &Registry) -> Result<Value, Error> {
    match op {
        "normalize" => scalar_node(&text(&args[0]))?.normalize(args[1].clone()),
        "finalize_not_empty" => {
            let node = scalar_node(&text(&args[0]))?;
            node.set_allow_empty_value(false)?;
            node.set_info(b"the info");
            node.finalize(args[1].clone())
        }
        "numeric" => {
            let (min, max) = (number(&args[1]), number(&args[2]));
            let node = match text(&args[0]).as_str() {
                "numeric" => NumericNode::new("n", None, min, max)?,
                "integer" => IntegerNode::new("n", None, min, max)?,
                _ => FloatNode::new("n", None, min, max)?,
            };
            node.finalize(args[3].clone())
        }
        "enum_permissible" => {
            let node = enum_node(&text(&args[0]), &args[1], enums)?;
            let trim = matches!(args[2], Value::Bool(true));
            let separator: &[u8] = if trim { b", " } else { b" | " };
            Ok(Value::from(php::string(
                node.get_permissible_values(separator, trim)?,
            )))
        }
        "enum_finalize" => enum_node(&text(&args[0]), &args[1], enums)?.finalize(args[2].clone()),
        "unrecognized" => {
            let node = ArrayNode::new("root", None)?;
            for name in list(&args[0]) {
                node.add_child(&ScalarNode::new(php::to_bytes(&name), None)?)?;
            }
            let mut input = PhpArray::new();
            for key in list(&args[1]) {
                input.insert(
                    Key::from_value(&key).unwrap_or(Key::Int(0)),
                    Value::from("v"),
                );
            }
            node.normalize(Value::Array(input))
        }
        "pre_normalize" => Ok(ArrayNode::new("r", None)?.pre_normalize(args[0].clone())),
        "normalize_config" => {
            let plural = args[2].as_string().map(|s| s.to_vec());
            Processor::normalize_config(
                &array(&args[0]),
                &php::to_bytes(&args[1]),
                plural.as_deref(),
            )
            .map(Value::Array)
        }
        "keyed_normalize" => {
            keyed(matches!(args[0], Value::Bool(true)))?.normalize(args[1].clone())
        }
        "prototyped_merge" => {
            let node = PrototypedArrayNode::new("p", None)?;
            node.set_prototype(&VariableNode::new("v", None)?)?;
            node.set_allow_new_keys(matches!(args[0], Value::Bool(true)))?;
            node.merge(args[1].clone(), args[2].clone())
        }
        "levenshtein" => Ok(Value::Int(php::levenshtein(
            &php::to_bytes(&args[0]),
            &php::to_bytes(&args[1]),
        ) as i64)),
        "compare" => Ok(Value::Int(match php::compare(&args[0], &args[1]) {
            Some(std::cmp::Ordering::Less) => -1,
            Some(std::cmp::Ordering::Equal) => 0,
            Some(std::cmp::Ordering::Greater) => 1,
            None => panic!("uncomparable {:?} {:?}", args[0], args[1]),
        })),
        "sort" => {
            let mut values = list(&args[0]);
            values.sort_by(|a, b| php::compare(a, b).unwrap_or(std::cmp::Ordering::Equal));
            Ok(Value::Array(PhpArray::list(values)))
        }
        other => panic!("unknown op {other}"),
    }
}

#[test]
fn recorded_definition_cases() -> TestResult {
    let json: Vec<Json> = serde_json::from_str(include_str!("data/definition.json"))?;
    let enums = enums();
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
        let expected = outcome(&case[2]).map_err(|e| e.to_string())?;
        let actual = run(op, &args, &enums);
        let same = match (&expected, &actual) {
            // A recorded object other than an enum case decodes as a new
            // object, so it is compared by class and properties (the node
            // returned the object it was given).
            (Outcome::Value(e @ Value::Object(o)), Ok(a))
                if o.downcast_ref::<EnumCase>().is_none() =>
            {
                phpunit_equals(e, a)
            }
            (Outcome::Value(e), Ok(a)) => e == a,
            (Outcome::Throws { class, message }, Err(e)) => {
                e.class().name() == class && e.get_message() == message.as_bytes()
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
