//! Reads tests/data/php.json (`scripts/record.sh css-selector`): the
//! upstream data providers, with their Node, Token and Specificity objects
//! rebuilt from the recorded properties, the DOM rows and the extra cases.
//!
//! Upstream: none
#![allow(dead_code)]

use std::sync::OnceLock;

use php_value::vectors::base64;
use serde_json::Value as Json;
use symfony_css_selector::node::{
    AttributeNode, ClassNode, CombinedSelectorNode, ElementNode, FunctionNode, HashNode,
    MatchingNode, NegationNode, Node, PseudoNode, RelationNode, SelectorNode, Specificity,
    SpecificityAdjustmentNode,
};
use symfony_css_selector::parser::{Token, TokenType};
use symfony_css_selector::{CssSelectorConverter, Exception};

/// A recorded PHP value.
#[derive(Debug, Clone, PartialEq)]
pub enum V {
    Null,
    Bool(bool),
    Int(i64),
    Str(Vec<u8>),
    Array(Vec<(V, V)>),
    Object(String, Vec<(String, V)>),
    Throws(String, String),
}

impl V {
    pub fn bytes(&self) -> Vec<u8> {
        match self {
            V::Str(s) => s.clone(),
            other => panic!("not a string: {other:?}"),
        }
    }

    pub fn opt_bytes(&self) -> Option<Vec<u8>> {
        match self {
            V::Null => None,
            other => Some(other.bytes()),
        }
    }

    pub fn int(&self) -> i64 {
        match self {
            V::Int(i) => *i,
            other => panic!("not an int: {other:?}"),
        }
    }

    pub fn list(&self) -> Vec<V> {
        match self {
            V::Array(pairs) => pairs.iter().map(|(_, v)| v.clone()).collect(),
            other => panic!("not an array: {other:?}"),
        }
    }

    pub fn prop(&self, name: &str) -> V {
        match self {
            V::Object(_, props) => props
                .iter()
                .find(|(n, _)| n == name)
                .map_or(V::Null, |(_, v)| v.clone()),
            other => panic!("not an object: {other:?}"),
        }
    }

    fn class(&self) -> &str {
        match self {
            V::Object(class, _) => class.rsplit('\\').next().unwrap_or(class),
            other => panic!("not an object: {other:?}"),
        }
    }
}

pub fn decode(json: &Json) -> V {
    let tag = json[0].as_str().unwrap_or_default();
    match tag {
        "null" => V::Null,
        "bool" => V::Bool(json[1].as_bool().unwrap_or_default()),
        "int" => V::Int(json[1].as_i64().unwrap_or_default()),
        "string" => V::Str(base64(json[1].as_str().unwrap_or_default()).expect("base64")),
        "array" => V::Array(
            json[1]
                .as_array()
                .expect("pairs")
                .iter()
                .map(|p| (decode(&p[0]), decode(&p[1])))
                .collect(),
        ),
        "object" => V::Object(
            json[1].as_str().unwrap_or_default().to_owned(),
            json[2]
                .as_array()
                .expect("properties")
                .iter()
                .map(|p| {
                    (
                        String::from_utf8(decode(&p[0]).bytes()).expect("name"),
                        decode(&p[1]),
                    )
                })
                .collect(),
        ),
        "throws" => V::Throws(
            json[1].as_str().unwrap_or_default().to_owned(),
            json[2].as_str().unwrap_or_default().to_owned(),
        ),
        other => panic!("unsupported recorded value {other}"),
    }
}

pub fn data() -> &'static Json {
    static DATA: OnceLock<Json> = OnceLock::new();
    DATA.get_or_init(|| serde_json::from_str(include_str!("../data/php.json")).expect("php.json"))
}

/// The rows of `test`'s provider in upstream test class `class` (its path
/// under Tests/, e.g. `Parser/ParserTest`): (row name, arguments).
pub fn provider(class: &str, test: &str) -> Vec<(String, Vec<V>)> {
    let rows = data()["providers"][class][test]["rows"]
        .as_array()
        .unwrap_or_else(|| panic!("no provider rows for {class}::{test}"));
    assert!(!rows.is_empty(), "{class}::{test} has no rows");
    rows.iter()
        .map(|row| {
            (
                row[0].as_str().unwrap_or_default().to_owned(),
                row[1]
                    .as_array()
                    .expect("arguments")
                    .iter()
                    .map(decode)
                    .collect(),
            )
        })
        .collect()
}

pub fn token(v: &V) -> Token {
    let type_ = match v.prop("type") {
        V::Null => None,
        t => Some(
            TokenType::from_name(&String::from_utf8(t.bytes()).expect("type")).expect("token type"),
        ),
    };
    let position = match v.prop("position") {
        V::Null => None,
        p => Some(usize::try_from(p.int()).expect("position")),
    };
    Token::new(type_, v.prop("value").opt_bytes().as_deref(), position)
}

pub fn specificity(v: &V) -> Specificity {
    Specificity::new(v.prop("a").int(), v.prop("b").int(), v.prop("c").int())
}

pub fn node(v: &V) -> Node {
    let sel = || node(&v.prop("selector"));
    let ob = |name: &str| v.prop(name).opt_bytes();
    let b = |name: &str| v.prop(name).bytes();
    match v.class() {
        "AttributeNode" => AttributeNode::new(
            sel(),
            ob("namespace").as_deref(),
            &b("attribute"),
            &b("operator"),
            ob("value").as_deref(),
        )
        .into(),
        "ClassNode" => ClassNode::new(sel(), &b("name")).into(),
        "CombinedSelectorNode" => {
            CombinedSelectorNode::new(sel(), &b("combinator"), node(&v.prop("subSelector"))).into()
        }
        "ElementNode" => {
            ElementNode::new(ob("namespace").as_deref(), ob("element").as_deref()).into()
        }
        "FunctionNode" => FunctionNode::new(
            sel(),
            &b("name"),
            v.prop("arguments").list().iter().map(token).collect(),
        )
        .into(),
        "HashNode" => HashNode::new(sel(), &b("id")).into(),
        "MatchingNode" => {
            MatchingNode::new(sel(), v.prop("arguments").list().iter().map(node).collect()).into()
        }
        "NegationNode" => NegationNode::new(sel(), node(&v.prop("subSelector"))).into(),
        "PseudoNode" => PseudoNode::new(sel(), &b("identifier")).into(),
        "RelationNode" => RelationNode::new(
            sel(),
            v.prop("arguments")
                .list()
                .iter()
                .map(|pair| {
                    let pair = pair.list();
                    (pair[0].bytes(), node(&pair[1]))
                })
                .collect(),
        )
        .into(),
        "SelectorNode" => {
            SelectorNode::new(node(&v.prop("tree")), ob("pseudoElement").as_deref()).into()
        }
        "SpecificityAdjustmentNode" => SpecificityAdjustmentNode::new(
            sel(),
            v.prop("arguments").list().iter().map(node).collect(),
        )
        .into(),
        other => panic!("unknown node class {other}"),
    }
}

/// Printable bytes, for assertion messages.
pub fn show(bytes: &[u8]) -> String {
    String::from_utf8_lossy(bytes).into_owned()
}

/// A result as the recorder writes it: the string, or `Throws(class,
/// message)`.
pub fn outcome(result: Result<Vec<u8>, Exception>) -> V {
    match result {
        Ok(s) => V::Str(s),
        Err(e) => V::Throws(e.kind().class().to_owned(), show(e.message())),
    }
}

/// Checks a recorded outcome; a `TypeError`'s message is compared up to
/// where PHP adds the calling file and line.
pub fn assert_outcome(expected: &V, actual: Result<Vec<u8>, Exception>, context: &str) {
    let actual = outcome(actual);
    match (expected, &actual) {
        (V::Throws(class, message), V::Throws(aclass, amessage)) if class == "TypeError" => {
            assert_eq!(class, aclass, "{context}");
            assert!(
                message.starts_with(amessage.as_str()),
                "{context}: {message} vs {amessage}"
            );
        }
        _ => assert_eq!(expected, &actual, "{context}"),
    }
}

pub fn converter(html: bool) -> CssSelectorConverter {
    CssSelectorConverter::new(html)
}
