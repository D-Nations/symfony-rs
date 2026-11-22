//! Replays tests/data/dumpers.json, recorded from the PHP oracle by
//! oracle/recorders/config_dumpers.php (`scripts/record.sh config`): the
//! YAML and XML reference dumpers and ArrayShapeGenerator on generated
//! trees (defaults and examples of every kind, info and deprecations at
//! each depth, key widths, prototypes of every class, nested and keyed
//! prototypes, key attributes "0" and "", normalized types, quoted shape
//! keys, `dumpAtPath()` on 25 paths, a node that is not a BaseNode).
//! Each output must be identical, and each exception must have the same
//! class and message; a TypeError's message is compared without PHP's
//! ", called in <file> on line <n>".
//!
//! Upstream: none
#![allow(non_snake_case)]

mod common;

use std::sync::Arc;

use common::configuration::CustomNode;
use php_value::{
    Number, PhpString, Value,
    constants::Registry,
    object::EnumCase,
    vectors::{Outcome, decode, outcome},
};
use serde_json::Value as Json;
use symfony_config::{
    Error, Node,
    definition::{
        ArrayNode, ArrayShapeGenerator, BooleanNode, EnumNode, FloatNode, IntegerNode, NumericNode,
        PrototypedArrayNode, ScalarNode, StringNode, VariableNode,
        dumper::{XmlReferenceDumper, YamlReferenceDumper},
    },
};

type Fail = Box<dyn std::error::Error + Send + Sync>;
type TestResult = Result<(), Fail>;

const NS: &str = "Oracle\\ConfigDumpers";

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
        [("One", 1), ("Two", 2)]
            .iter()
            .map(|(c, v)| EnumCase::new(int.clone(), *c, Some(Value::Int(*v))))
            .collect(),
    );
    let string = class("StrEnum");
    registry.define_enum(
        &string,
        [("Foo", "foo"), ("Slash", "a/b"), ("Accent", "é")]
            .iter()
            .map(|(c, v)| EnumCase::new(string.clone(), *c, Some(Value::from(*v))))
            .collect(),
    );
    registry
}

fn str_of(json: &Json) -> Result<&str, Fail> {
    json.as_str()
        .ok_or_else(|| format!("expected a string, got {json}").into())
}

fn number(json: &Json) -> Result<Option<Number>, Fail> {
    if json.is_null() {
        return Ok(None);
    }
    Ok(match decode(json)? {
        Value::Int(i) => Some(Number::Int(i)),
        Value::Float(f) => Some(Number::Float(f)),
        _ => None,
    })
}

fn strings(json: &Json) -> Result<Vec<PhpString>, Fail> {
    json.as_array()
        .ok_or("expected a list")?
        .iter()
        .map(|s| Ok(PhpString::from(str_of(s)?)))
        .collect()
}

/// config_dumpers.php's `build()`.
fn build(spec: &Json, parent: Option<&Node>, enums: &Registry) -> Result<Node, Fail> {
    let name = str_of(&spec["n"])?.as_bytes();
    let node = match str_of(&spec["c"])? {
        "array" => ArrayNode::new(name, parent)?,
        "proto" => PrototypedArrayNode::new(name, parent)?,
        "scalar" => ScalarNode::new(name, parent)?,
        "bool" => BooleanNode::new(name, parent)?,
        "nbool" => BooleanNode::with_options(name, parent, b".", true)?,
        "int" => IntegerNode::new(name, parent, number(&spec["min"])?, number(&spec["max"])?)?,
        "float" => FloatNode::new(name, parent, number(&spec["min"])?, number(&spec["max"])?)?,
        "numeric" => NumericNode::new(name, parent, number(&spec["min"])?, number(&spec["max"])?)?,
        "string" => StringNode::new(name, parent)?,
        "enum" => {
            let values = match spec["values"].as_array() {
                Some(values) => values.iter().map(decode).collect::<Result<_, _>>()?,
                None => Vec::new(),
            };
            EnumNode::with_options(name, parent, values, b".", spec["fqcn"].as_str(), enums)?
        }
        "variable" => VariableNode::new(name, parent)?,
        "custom" => {
            return Ok(Node::custom(
                b"custom_node",
                None,
                b".",
                Arc::new(CustomNode("Oracle\\ConfigDumpers\\CustomNode")),
            )?);
        }
        other => return Err(format!("unknown class {other}").into()),
    };
    if let Some(info) = spec["info"].as_str() {
        node.set_info(info.as_bytes());
    }
    if !spec["ex"].is_null() {
        node.set_example(decode(&spec["ex"])?);
    }
    if spec["req"].as_bool() == Some(true) {
        node.set_required(true);
    }
    if let Some(dep) = spec["dep"].as_array() {
        node.set_deprecated(
            str_of(&dep[0])?.as_bytes(),
            str_of(&dep[1])?.as_bytes(),
            str_of(&dep[2])?.as_bytes(),
        );
    }
    if !spec["types"].is_null() {
        node.set_normalized_types(strings(&spec["types"])?);
    }
    if node.is_array_node() {
        if let Some(add) = spec["addif"].as_bool() {
            node.set_add_if_not_set(add)?;
        }
        if let Some(extra) = spec["extra"].as_array() {
            node.set_ignore_extra_keys(
                extra[0].as_bool().ok_or("ignore")?,
                extra[1].as_bool().ok_or("remove")?,
            )?;
        }
        if let Some(deep) = spec["deep"].as_bool() {
            node.set_perform_deep_merging(deep)?;
        }
        if let Some(xml) = spec["xml"].as_array() {
            let mut remappings = Vec::new();
            for pair in xml {
                remappings.push((
                    PhpString::from(str_of(&pair[0])?),
                    PhpString::from(str_of(&pair[1])?),
                ));
            }
            node.set_xml_remappings(remappings)?;
        }
    }
    if node.is_prototyped_array_node() {
        if let Some(key) = spec["key"].as_array() {
            node.set_key_attribute(
                str_of(&key[0])?.as_bytes(),
                key[1].as_bool().ok_or("remove")?,
            )?;
        }
        if !spec["p"].is_null() {
            node.set_prototype(&build(&spec["p"], Some(&node), enums)?)?;
        }
        if spec["nulldef"].as_bool() == Some(true) {
            node.set_null_as_default()?;
        }
    }
    if spec.get("def").is_some() {
        node.set_default_value(decode(&spec["def"])?)?;
    }
    for child in spec["ch"].as_array().into_iter().flatten() {
        node.add_child(&build(child, Some(&node), enums)?)?;
    }
    Ok(node)
}

/// PHP's TypeError message without ", called in <file> on line <n>".
fn without_called_in(message: &str) -> &str {
    message
        .find(", called in ")
        .map_or(message, |i| &message[..i])
}

fn check(
    case: &str,
    what: &str,
    expected: &Json,
    actual: Result<Vec<u8>, Error>,
    failures: &mut Vec<String>,
) -> TestResult {
    let ok = match (outcome(expected)?, &actual) {
        (Outcome::Value(Value::String(s)), Ok(bytes)) => s.as_bytes() == bytes.as_slice(),
        (Outcome::Throws { class, message }, Err(e)) => {
            let expected = if class == "TypeError" {
                without_called_in(&message)
            } else {
                &message
            };
            e.class().name() == class && e.get_message() == expected.as_bytes()
        }
        _ => false,
    };
    if !ok {
        let actual = match actual {
            Ok(bytes) => String::from_utf8_lossy(&bytes).into_owned(),
            Err(e) => e.to_string(),
        };
        failures.push(format!(
            "{case} {what}: expected {:?}, got {actual:?}",
            outcome(expected)?
        ));
    }
    Ok(())
}

#[test]
fn recorded_dumpers() -> TestResult {
    let data: Json = serde_json::from_str(include_str!("data/dumpers.json"))?;
    let enums = enums();
    let mut failures = Vec::new();
    let mut count = 0;
    for case in data.as_array().ok_or("expected a list")? {
        let name = str_of(&case["name"])?;
        let tree = &case["tree"];
        let fresh = || build(tree, None, &enums);

        check(
            name,
            "yaml",
            &case["yaml"],
            YamlReferenceDumper::new().dump_node(&fresh()?),
            &mut failures,
        )?;
        for at in case["yaml_at"].as_array().ok_or("yaml_at")? {
            let path = str_of(&at[0])?;
            check(
                name,
                &format!("yaml at {path:?}"),
                &at[1],
                YamlReferenceDumper::new().dump_node_at_path(&fresh()?, path.as_bytes()),
                &mut failures,
            )?;
            count += 1;
        }
        check(
            name,
            "xml",
            &case["xml"],
            XmlReferenceDumper::new().dump_node(&fresh()?, None),
            &mut failures,
        )?;
        check(
            name,
            "xml with a namespace",
            &case["xml_ns"],
            XmlReferenceDumper::new().dump_node(&fresh()?, Some(b"urn:ns")),
            &mut failures,
        )?;
        check(
            name,
            "xml with namespace \"0\"",
            &case["xml_ns_zero"],
            XmlReferenceDumper::new().dump_node(&fresh()?, Some(b"0")),
            &mut failures,
        )?;
        check(
            name,
            "shape",
            &case["shape"],
            ArrayShapeGenerator::generate(&fresh()?),
            &mut failures,
        )?;
        count += 5;
    }
    assert!(
        failures.is_empty(),
        "{} of {count} differ:\n{}",
        failures.len(),
        failures.join("\n")
    );
    Ok(())
}
