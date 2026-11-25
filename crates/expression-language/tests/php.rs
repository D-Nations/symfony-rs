//! The behaviour symfony/expression-language's tests leave open, replayed
//! from `tests/data/php.json`, recorded from PHP 8.4.27 by
//! `oracle/recorders/expression_language.php` (`scripts/record.sh
//! expression-language`): every binary and unary operator on 48 operands,
//! `..` ranges, `matches` patterns, the lexer, and 322 expressions
//! through `ExpressionLanguage` (parse tree, dump, compiled PHP,
//! evaluation). PHP's warnings are recorded but not compared: this port
//! returns the value PHP goes on with.
//!
//! Upstream: none
#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::cast_possible_truncation
)]

mod common;

use common::vectors::{b64, check, check_text, data, decode, language, text};
use php_value::{Key, PhpArray, PhpString, Value};
use serde_json::Value as Json;
use symfony_expression_language::{
    Functions, Lexer,
    node::{BinaryNode, ConstantNode, Node, UnaryNode},
};

/// Fails with the first differences, after counting them all.
fn report(name: &str, total: usize, failures: &[String]) {
    assert!(
        failures.is_empty(),
        "{name}: {} of {total} differ from PHP:\n{}",
        failures.len(),
        failures
            .iter()
            .take(40)
            .cloned()
            .collect::<Vec<_>>()
            .join("\n")
    );
    eprintln!("{name}: {total} cases match PHP");
}

fn constant(value: &Value) -> Node {
    Node::Constant(ConstantNode::new(value.clone()))
}

#[test]
fn binary_operators_match_php() {
    let data = data();
    let values: Vec<Value> = data["values"]
        .as_array()
        .unwrap()
        .iter()
        .map(decode)
        .collect();
    let outcomes = data["outcomes"].as_array().unwrap();
    let (mut total, mut failures) = (0, Vec::new());
    for (operator, rows) in data["binary"].as_object().unwrap() {
        for row in rows.as_array().unwrap() {
            let (i, j) = (
                row[0].as_u64().unwrap() as usize,
                row[1].as_u64().unwrap() as usize,
            );
            let expected = &outcomes[row[2].as_u64().unwrap() as usize];
            let node = Node::Binary(BinaryNode::new(
                operator.as_str(),
                constant(&values[i]),
                constant(&values[j]),
            ));
            let actual = node.evaluate(&Functions::new(), &PhpArray::new());
            total += 1;
            if let Some(difference) = check(expected, &actual) {
                failures.push(format!(
                    "{:?} {operator} {:?}: {difference}",
                    values[i], values[j]
                ));
            }
        }
    }
    report("binary operators", total, &failures);
}

#[test]
fn unary_operators_match_php() {
    let data = data();
    let values: Vec<Value> = data["values"]
        .as_array()
        .unwrap()
        .iter()
        .map(decode)
        .collect();
    let outcomes = data["outcomes"].as_array().unwrap();
    let (mut total, mut failures) = (0, Vec::new());
    for (operator, rows) in data["unary"].as_object().unwrap() {
        for row in rows.as_array().unwrap() {
            let i = row[0].as_u64().unwrap() as usize;
            let expected = &outcomes[row[1].as_u64().unwrap() as usize];
            let node = Node::Unary(UnaryNode::new(operator.as_str(), constant(&values[i])));
            let actual = node.evaluate(&Functions::new(), &PhpArray::new());
            total += 1;
            if let Some(difference) = check(expected, &actual) {
                failures.push(format!("{operator} {:?}: {difference}", values[i]));
            }
        }
    }
    report("unary operators", total, &failures);
}

/// Both reject the pattern with the same `SyntaxError` but give another
/// reason after "is not valid: ": php-pcre compiles with fancy-regex, and
/// its texts are not PCRE2's.
fn same_pattern_error(
    expected: &Json,
    actual: &Result<Value, symfony_expression_language::Error>,
) -> bool {
    let (Some("throws"), Err(e)) = (expected[0].as_str(), actual) else {
        return false;
    };
    let php = b64(&expected[2]);
    let ours = e.message().as_bytes();
    let marker: &[u8] = b" is not valid: ";
    let head = |m: &[u8]| {
        m.windows(marker.len())
            .position(|w| w == marker)
            .map(|at| m[..at].to_vec())
    };
    e.class().name() == expected[1].as_str().unwrap_or("")
        && head(&php).is_some()
        && head(&php) == head(ours)
        && php.ends_with(b" around position 0.")
        && ours.ends_with(b" around position 0.")
}

fn pairs(name: &str, operator: &str) {
    let data = data();
    let rows = data[name].as_array().unwrap();
    let (mut failures, mut pcre_messages) = (Vec::new(), 0);
    for row in rows {
        let (left, right) = (decode(&row[0]), decode(&row[1]));
        // `matches` takes the subject on the left
        let (l, r) = if operator == "matches" {
            (&right, &left)
        } else {
            (&left, &right)
        };
        let node = Node::Binary(BinaryNode::new(operator, constant(l), constant(r)));
        let actual = node.evaluate(&Functions::new(), &PhpArray::new());
        if let Some(difference) = check(&row[2], &actual) {
            if operator == "matches" && same_pattern_error(&row[2], &actual) {
                pcre_messages += 1;
                continue;
            }
            failures.push(format!("{l:?} {operator} {r:?}: {difference}"));
        }
    }
    if pcre_messages > 0 {
        eprintln!("{name}: {pcre_messages} invalid patterns rejected with php-pcre's own reason");
    }
    report(name, rows.len(), &failures);
}

#[test]
fn ranges_match_php() {
    pairs("range", "..");
}

#[test]
fn matches_matches_php() {
    pairs("matches", "matches");
}

#[test]
fn lexer_matches_php() {
    let data = data();
    let rows = data["lexer"].as_array().unwrap();
    let mut failures = Vec::new();
    for row in rows {
        let expression = b64(&row[0]);
        let shown = String::from_utf8_lossy(&expression).into_owned();
        let actual = Lexer::new().tokenize(&expression);
        match (row[1][0].as_str(), actual) {
            (Some("tokens"), Ok(stream)) => {
                let expected = row[1][1].as_array().unwrap();
                let ours = stream.tokens();
                let same = expected.len() == ours.len()
                    && expected.iter().zip(ours).all(|(e, t)| {
                        e[0].as_str() == Some(t.kind.as_str())
                            && common::vectors::same(&decode(&e[1]), &t.value)
                            && e[2].as_u64().map(|c| c as usize) == t.cursor
                    });
                if !same {
                    failures.push(format!("{shown:?}: PHP {expected:?}, ours {stream}"));
                }
            }
            (Some("throws"), Err(e)) => {
                if e.message().as_bytes() != b64(&row[1][2]) || e.class().name() != row[1][1] {
                    failures.push(format!(
                        "{shown:?}: PHP threw {}, we threw {e}",
                        text(&row[1][2])
                    ));
                }
            }
            (_, Ok(stream)) => failures.push(format!(
                "{shown:?}: PHP threw {}, we gave {stream}",
                text(&row[1][2])
            )),
            (_, Err(e)) => failures.push(format!("{shown:?}: PHP gave tokens, we threw {e}")),
        }
    }
    report("lexer", rows.len(), &failures);
}

fn vars(data: &Json) -> PhpArray {
    let mut vars = PhpArray::new();
    for pair in data["vars"].as_array().unwrap() {
        vars.insert(Key::from(pair[0].as_str().unwrap()), decode(&pair[1]));
    }
    vars
}

#[test]
fn expressions_match_php() {
    let data = data();
    let vars = vars(&data);
    let names = PhpArray::list(vars.keys().map(Key::to_value));
    let rows = data["cases"].as_array().unwrap();
    let language = language();
    let mut failures = Vec::new();
    for row in rows {
        let expression = b64(&row[0]);
        let shown = String::from_utf8_lossy(&expression).into_owned();
        let parsed = language.parse(expression.as_slice(), &names, 0);
        let tree = parsed.clone().map(|p| p.nodes().to_php_string());
        let dump = parsed.and_then(|p| p.nodes().dump());
        let compiled = language.compile(expression.as_slice(), &names);
        let evaluated = language.evaluate(expression.as_slice(), &vars);
        for (what, difference) in [
            ("parse", check_text(&row[1], &tree)),
            ("dump", check_text(&row[2], &dump)),
            ("compile", check_text(&row[3], &compiled)),
            ("evaluate", check(&row[4], &evaluated)),
        ] {
            if let Some(difference) = difference {
                failures.push(format!("{shown:?} {what}: {difference}"));
            }
        }
    }
    report("expressions", rows.len(), &failures);
}

/// The compiled source of the expressions upstream's tests `eval()`.
#[test]
fn compiled_sources_match_php() {
    let data = data();
    let rows = data["compiled"].as_array().unwrap();
    let language = language();
    let mut failures = Vec::new();
    for row in rows {
        let expression = b64(&row[0]);
        let Value::Array(names) = decode(&row[1]) else {
            panic!("names")
        };
        let compiled: Result<PhpString, _> = language.compile(expression.as_slice(), &names);
        if let Some(difference) = check_text(&row[3], &compiled) {
            failures.push(format!(
                "{:?}: {difference}",
                String::from_utf8_lossy(&expression)
            ));
        }
    }
    report("compiled", rows.len(), &failures);
}
