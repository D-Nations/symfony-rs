//! Query\QueryBuilder against real PHP: the call sequences of
//! `oracle/recorders/doctrine_dbal.php` ("query" in `tests/data/php.json`),
//! each replayed on the platform it was recorded with, comparing every
//! call's result, the final SQL or exception, the parameters and their
//! types.
//!
//! Upstream: none

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    reason = "test helpers; a panic is a test failure"
)]

mod common;

use common::{DefaultPlatform, StubPlatform, TestConnection, data, ordered_types, wrapper_type};
use doctrine_dbal::platforms::AbstractPlatform;
use doctrine_dbal::query::expression::ExpressionPart;
use doctrine_dbal::query::{ConflictResolutionMode, QueryBuilder, UnionType};
use doctrine_dbal::{
    ArrayParameterType, ConnectionStandIn, Exception, ParameterType, WrapperParameterType,
};
use php_value::vectors::{base64, decode};
use php_value::{Key, PhpArray, Value};
use serde_json::Value as Json;

/// What a call gave: PHP's `["ok"]`, `["sql", base64]`, `["value", typed]`
/// or `["throws", class, message]`.
#[derive(Debug, PartialEq)]
enum Outcome {
    Ok,
    Sql(String),
    Value(Value),
    Type(WrapperParameterType),
    Throws(String, String),
}

fn expected(json: &Json) -> Outcome {
    match json[0].as_str() {
        Some("ok") => Outcome::Ok,
        Some("sql") => {
            Outcome::Sql(String::from_utf8(base64(json[1].as_str().unwrap()).unwrap()).unwrap())
        }
        Some("value") if json[1][0] == "enum" => Outcome::Type(wrapper_type(&json[1])),
        Some("value") => Outcome::Value(decode(&json[1]).unwrap()),
        Some("throws") => Outcome::Throws(
            json[1].as_str().unwrap().to_owned(),
            json[2].as_str().unwrap().to_owned(),
        ),
        _ => panic!("unknown outcome {json}"),
    }
}

fn thrown(e: &Exception) -> Outcome {
    Outcome::Throws(e.class().to_owned(), e.message().to_owned())
}

fn str_arg(json: &Json) -> &str {
    json.as_str()
        .unwrap_or_else(|| panic!("not a string: {json}"))
}

fn opt_str(json: &Json) -> Option<&str> {
    json.as_str()
}

fn strs(args: &[Json]) -> Vec<&str> {
    args.iter().map(str_arg).collect()
}

fn key(json: &Json) -> Key {
    match json {
        Json::Number(n) => Key::Int(n.as_i64().unwrap()),
        Json::String(s) => Key::from(s.as_str()),
        _ => panic!("not a key: {json}"),
    }
}

fn value(json: &Json) -> Value {
    match json {
        Json::Null => Value::Null,
        Json::Bool(b) => Value::Bool(*b),
        Json::Number(n) => Value::Int(n.as_i64().unwrap()),
        Json::String(s) => Value::from(s.as_str()),
        Json::Array(items) => Value::Array(PhpArray::list(items.iter().map(value))),
        Json::Object(_) => panic!("no objects in calls"),
    }
}

fn parameter_type(json: &Json) -> WrapperParameterType {
    let name = str_arg(json);
    if let Some(case) = name.strip_prefix("ParameterType::") {
        ParameterType::from_name(case).unwrap().into()
    } else if let Some(case) = name.strip_prefix("ArrayParameterType::") {
        ArrayParameterType::from_name(case).unwrap().into()
    } else {
        name.into()
    }
}

fn pairs(json: &Json) -> Vec<(&str, &str)> {
    json.as_array()
        .unwrap()
        .iter()
        .map(|pair| (str_arg(&pair[0]), str_arg(&pair[1])))
        .collect()
}

/// A predicate argument: a string, or `["expr", method, ...arguments]`
/// built with `$qb->expr()`.
fn predicate<C: ConnectionStandIn>(qb: &QueryBuilder<'_, C>, json: &Json) -> ExpressionPart {
    match json {
        Json::String(s) => s.as_str().into(),
        Json::Array(items) if items[0] == "expr" => {
            let expr = qb.expr();
            let method = str_arg(&items[1]);
            let args = &items[2..];
            match method {
                "and" | "or" => {
                    let parts: Vec<ExpressionPart> =
                        args[1..].iter().map(|a| predicate(qb, a)).collect();
                    let first = predicate(qb, &args[0]);
                    if method == "and" {
                        expr.and(first, parts).into()
                    } else {
                        expr.or(first, parts).into()
                    }
                }
                "like" => expr
                    .like(
                        str_arg(&args[0]),
                        str_arg(&args[1]),
                        args.get(2).and_then(opt_str),
                    )
                    .into(),
                "notLike" => expr
                    .not_like(
                        str_arg(&args[0]),
                        str_arg(&args[1]),
                        args.get(2).and_then(opt_str),
                    )
                    .into(),
                "in" | "notIn" => {
                    let list: Vec<&str> = args[1].as_array().unwrap().iter().map(str_arg).collect();
                    if method == "in" {
                        expr.r#in(str_arg(&args[0]), &list).into()
                    } else {
                        expr.not_in(str_arg(&args[0]), &list).into()
                    }
                }
                "isNull" => expr.is_null(str_arg(&args[0])).into(),
                "isNotNull" => expr.is_not_null(str_arg(&args[0])).into(),
                "comparison" => expr
                    .comparison(str_arg(&args[0]), str_arg(&args[1]), str_arg(&args[2]))
                    .into(),
                other => panic!("unknown expr method {other}"),
            }
        }
        _ => panic!("not a predicate: {json}"),
    }
}

fn predicates<C: ConnectionStandIn>(
    qb: &QueryBuilder<'_, C>,
    args: &[Json],
) -> (ExpressionPart, Vec<ExpressionPart>) {
    let first = predicate(qb, &args[0]);
    let rest = args[1..].iter().map(|a| predicate(qb, a)).collect();
    (first, rest)
}

#[allow(clippy::too_many_lines, reason = "one arm per QueryBuilder method")]
fn call<C: ConnectionStandIn>(qb: &mut QueryBuilder<'_, C>, call: &[Json]) -> Outcome {
    let method = str_arg(&call[0]);
    let args = &call[1..];
    let arg = |i: usize| str_arg(&args[i]);
    let opt = |i: usize| args.get(i).and_then(opt_str);
    match method {
        "getSQL" => match qb.get_sql() {
            Ok(sql) => return Outcome::Sql(sql),
            Err(e) => return thrown(&e),
        },
        "getParameter" => return Outcome::Value(qb.get_parameter(key(&args[0]))),
        "getParameterType" => return Outcome::Type(qb.get_parameter_type(key(&args[0]))),
        "getFirstResult" => return Outcome::Value(Value::Int(qb.get_first_result())),
        "getMaxResults" => {
            return Outcome::Value(qb.get_max_results().map_or(Value::Null, Value::Int));
        }
        "createNamedParameter" => {
            return Outcome::Value(Value::from(qb.create_named_parameter(
                value(&args[0]),
                parameter_type(&args[1]),
                opt(2),
            )));
        }
        "createPositionalParameter" => {
            return Outcome::Value(Value::from(
                qb.create_positional_parameter(value(&args[0]), parameter_type(&args[1])),
            ));
        }
        "select" => {
            qb.select(&strs(args));
        }
        "addSelect" => {
            qb.add_select(arg(0), &strs(&args[1..]));
        }
        "distinct" => {
            qb.distinct(args.first().is_none_or(|b| b.as_bool().unwrap()));
        }
        "from" => {
            qb.from(arg(0), opt(1));
        }
        "join" | "innerJoin" | "leftJoin" | "rightJoin" | "fullJoin" => {
            let (from_alias, join, alias, condition) = (arg(0), arg(1), arg(2), opt(3));
            match method {
                "join" => qb.join(from_alias, join, alias, condition),
                "innerJoin" => qb.inner_join(from_alias, join, alias, condition),
                "leftJoin" => qb.left_join(from_alias, join, alias, condition),
                "rightJoin" => qb.right_join(from_alias, join, alias, condition),
                _ => qb.full_join(from_alias, join, alias, condition),
            };
        }
        "where" | "andWhere" | "orWhere" | "having" | "andHaving" | "orHaving" => {
            let (first, rest) = predicates(qb, args);
            match method {
                "where" => qb.r#where(first, rest),
                "andWhere" => qb.and_where(first, rest),
                "orWhere" => qb.or_where(first, rest),
                "having" => qb.having(first, rest),
                "andHaving" => qb.and_having(first, rest),
                _ => qb.or_having(first, rest),
            };
        }
        "groupBy" => {
            qb.group_by(arg(0), &strs(&args[1..]));
        }
        "addGroupBy" => {
            qb.add_group_by(arg(0), &strs(&args[1..]));
        }
        "orderBy" => {
            qb.order_by(arg(0), opt(1));
        }
        "addOrderBy" => {
            qb.add_order_by(arg(0), opt(1));
        }
        "setMaxResults" => {
            qb.set_max_results(args[0].as_i64());
        }
        "setFirstResult" => {
            qb.set_first_result(args[0].as_i64().unwrap());
        }
        "forUpdate" => {
            let mode = opt(0).map_or_else(ConflictResolutionMode::default, |m| {
                ConflictResolutionMode::from_name(m).unwrap()
            });
            qb.for_update(mode);
        }
        "union" => {
            qb.union(arg(0));
        }
        "addUnion" => {
            let r#type =
                opt(1).map_or_else(UnionType::default, |t| UnionType::from_name(t).unwrap());
            if let Err(e) = qb.add_union(arg(0), r#type) {
                return thrown(&e.into());
            }
        }
        "with" => {
            let columns: Option<Vec<&str>> = args
                .get(2)
                .and_then(Json::as_array)
                .map(|c| c.iter().map(str_arg).collect());
            if let Err(e) = qb.with(arg(0), arg(1), columns.as_deref()) {
                return thrown(&e.into());
            }
        }
        "insert" => {
            qb.insert(arg(0));
        }
        "update" => {
            qb.update(arg(0));
        }
        "delete" => {
            qb.delete(arg(0));
        }
        "set" => {
            qb.set(arg(0), arg(1));
        }
        "setValue" => {
            qb.set_value(arg(0), arg(1));
        }
        "values" => {
            qb.values(&pairs(&args[0]));
        }
        "setParameter" => {
            qb.set_parameter(key(&args[0]), value(&args[1]), parameter_type(&args[2]));
        }
        "setParameters" => {
            let params: PhpArray = args[0]
                .as_array()
                .unwrap()
                .iter()
                .map(|pair| (key(&pair[0]), value(&pair[1])))
                .collect();
            let types: Vec<(Key, WrapperParameterType)> = args
                .get(1)
                .map(|t| {
                    t.as_array()
                        .unwrap()
                        .iter()
                        .map(|pair| (key(&pair[0]), parameter_type(&pair[1])))
                        .collect()
                })
                .unwrap_or_default();
            qb.set_parameters(params, types);
        }
        "resetWhere" => {
            qb.reset_where();
        }
        "resetGroupBy" => {
            qb.reset_group_by();
        }
        "resetHaving" => {
            qb.reset_having();
        }
        "resetOrderBy" => {
            qb.reset_order_by();
        }
        other => panic!("unknown call {other}"),
    }
    Outcome::Ok
}

fn replay<P: AbstractPlatform>(platform: P, case: &Json) -> usize {
    let name = format!("{} ({})", str_arg(&case[0]), str_arg(&case[1]));
    let conn = TestConnection { platform };
    let mut qb = QueryBuilder::new(&conn);
    let calls = case[2].as_array().unwrap();
    let results = case[3].as_array().unwrap();
    assert_eq!(calls.len(), results.len(), "{name}");

    for (i, (c, result)) in calls.iter().zip(results).enumerate() {
        let actual = call(&mut qb, c.as_array().unwrap());
        assert_eq!(expected(result), actual, "{name}: call {i} {c}");
    }

    let actual = qb.get_sql().map_or_else(|e| thrown(&e), Outcome::Sql);
    let mut expected_sql = expected(&case[4]);
    if str_arg(&case[0]) == "numeric alias unknown" {
        // PHP's array key makes the alias "9" an int, which
        // UnknownAlias::new() refuses with a TypeError; the port throws
        // the UnknownAlias that PHP means (README.md, Differences).
        assert!(matches!(&expected_sql, Outcome::Throws(class, _) if class == "TypeError"));
        expected_sql = Outcome::Throws(
            "Doctrine\\DBAL\\Query\\Exception\\UnknownAlias".to_owned(),
            "The given alias \"9\" is not part of any FROM or JOIN clause table. \
             The currently registered aliases are: 1."
                .to_owned(),
        );
    }
    assert_eq!(expected_sql, actual, "{name}: final SQL");

    assert_eq!(
        decode(&case[5]).unwrap(),
        Value::Array(qb.get_parameters().clone()),
        "{name}: parameters"
    );
    assert_eq!(
        ordered_types(&case[6]),
        qb.get_parameter_types(),
        "{name}: parameter types"
    );

    calls.len()
}

#[test]
fn query_builder_call_sequences() {
    let cases = data()["query"].as_array().unwrap();
    assert_eq!(cases.len(), 112);
    let mut calls = 0;
    for case in cases {
        calls += match str_arg(&case[1]) {
            "stub" => replay(StubPlatform, case),
            "default" => replay(DefaultPlatform, case),
            other => panic!("unknown platform {other}"),
        };
    }
    assert!(calls > 300, "{calls} calls");
}
