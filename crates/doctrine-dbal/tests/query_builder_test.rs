//! QueryBuilderTest.php at doctrine/dbal
//! c435cd7ea33e17d2a89288caf994cf3d4549b879. Names and assertions are
//! PHPUnit's; the provider rows are upstream's, recorded by running its
//! providers (tests/data/php.json). `(string) $qb` is `get_sql()`. The
//! tests that execute the query (`testFetch*`, `testExecute*`) need a
//! Connection and a Result, which are not ported (parity manifest:
//! pending).
//!
//! Upstream: doctrine/dbal tests/Query/QueryBuilderTest.php

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    reason = "test helpers; a panic is a test failure"
)]
#![allow(non_snake_case)]

mod common;

use common::{StubPlatform, TestConnection, provider};
use doctrine_dbal::query::{
    DEFAULT_PARAMETER_TYPE, QueryBuilder, QueryException, QueryExceptionKind, UnionType,
};
use doctrine_dbal::{ArrayParameterType, Exception, ParameterType, WrapperParameterType};
use php_value::object::StdClass;
use php_value::{Key, PhpArray, PhpString, Value};

type Conn = TestConnection<StubPlatform>;

/// `setUp()`: the connection mock and its platform stub.
fn conn() -> Conn {
    TestConnection {
        platform: StubPlatform,
    }
}

/// `(string) $qb`.
fn string(qb: &QueryBuilder<'_, Conn>) -> String {
    qb.get_sql().unwrap()
}

/// The `QueryException` that `getSQL()` throws.
fn query_exception(result: Result<String, Exception>) -> QueryException {
    let e = match result {
        Err(e) if e.is_instance_of(QueryExceptionKind::Query.class()) => e,
        other => panic!("expected a QueryException, got {other:?}"),
    };
    let message = String::from_utf8_lossy(e.message().as_bytes()).into_owned();
    match e.class() {
        c if c == QueryExceptionKind::NonUniqueAlias.class() => {
            QueryException::with_kind(QueryExceptionKind::NonUniqueAlias, message)
        }
        c if c == QueryExceptionKind::UnknownAlias.class() => {
            QueryException::with_kind(QueryExceptionKind::UnknownAlias, message)
        }
        _ => QueryException::new(message),
    }
}

#[test]
fn testSimpleSelectWithoutFrom() {
    let conn = conn();
    let mut qb = QueryBuilder::new(&conn);

    qb.select(&["some_function()"]);

    assert_eq!("SELECT some_function()", string(&qb));
}

#[test]
fn testSimpleSelect() {
    let conn = conn();
    let mut qb = QueryBuilder::new(&conn);

    qb.select(&["u.id"]).from("users", Some("u"));

    assert_eq!("SELECT u.id FROM users u", string(&qb));
}

#[test]
fn testSimpleSelectWithDistinct() {
    let conn = conn();
    let mut qb = QueryBuilder::new(&conn);

    qb.select(&["u.id"]).distinct(true).from("users", Some("u"));

    assert_eq!("SELECT DISTINCT u.id FROM users u", string(&qb));
}

#[test]
fn testSelectWithSimpleWhere() {
    let conn = conn();
    let mut qb = QueryBuilder::new(&conn);
    let expr = qb.expr();

    qb.select(&["u.id"])
        .from("users", Some("u"))
        .r#where(expr.and(expr.eq("u.nickname", "?"), []), []);

    assert_eq!("SELECT u.id FROM users u WHERE u.nickname = ?", string(&qb));
}

#[test]
fn testSelectWithLeftJoin() {
    let conn = conn();
    let mut qb = QueryBuilder::new(&conn);
    let expr = qb.expr();

    qb.select(&["u.*", "p.*"])
        .from("users", Some("u"))
        .left_join("u", "phones", "p", Some(&expr.eq("p.user_id", "u.id")));

    assert_eq!(
        "SELECT u.*, p.* FROM users u LEFT JOIN phones p ON p.user_id = u.id",
        string(&qb)
    );
}

#[test]
fn testSelectWithJoin() {
    let conn = conn();
    let mut qb = QueryBuilder::new(&conn);
    let expr = qb.expr();

    qb.select(&["u.*", "p.*"]).from("users", Some("u")).join(
        "u",
        "phones",
        "p",
        Some(&expr.eq("p.user_id", "u.id")),
    );

    assert_eq!(
        "SELECT u.*, p.* FROM users u INNER JOIN phones p ON p.user_id = u.id",
        string(&qb)
    );
}

#[test]
fn testSelectWithJoinNoCondition() {
    let conn = conn();
    let mut qb = QueryBuilder::new(&conn);

    qb.select(&["u.*", "p.*"])
        .from("users", Some("u"))
        .join("u", "phones", "p", None);

    assert_eq!(
        "SELECT u.*, p.* FROM users u INNER JOIN phones p",
        string(&qb)
    );
}

#[test]
fn testSelectWithInnerJoin() {
    let conn = conn();
    let mut qb = QueryBuilder::new(&conn);
    let expr = qb.expr();

    qb.select(&["u.*", "p.*"])
        .from("users", Some("u"))
        .inner_join("u", "phones", "p", Some(&expr.eq("p.user_id", "u.id")));

    assert_eq!(
        "SELECT u.*, p.* FROM users u INNER JOIN phones p ON p.user_id = u.id",
        string(&qb)
    );
}

#[test]
fn testSelectWithRightJoin() {
    let conn = conn();
    let mut qb = QueryBuilder::new(&conn);
    let expr = qb.expr();

    qb.select(&["u.*", "p.*"])
        .from("users", Some("u"))
        .right_join("u", "phones", "p", Some(&expr.eq("p.user_id", "u.id")));

    assert_eq!(
        "SELECT u.*, p.* FROM users u RIGHT JOIN phones p ON p.user_id = u.id",
        string(&qb)
    );
}

#[test]
fn testSelectWithFullJoin() {
    let conn = conn();
    let mut qb = QueryBuilder::new(&conn);
    let expr = qb.expr();

    qb.select(&["u.*", "p.*"])
        .from("users", Some("u"))
        .full_join("u", "phones", "p", Some(&expr.eq("p.user_id", "u.id")));

    assert_eq!(
        "SELECT u.*, p.* FROM users u FULL JOIN phones p ON p.user_id = u.id",
        string(&qb)
    );
}

#[test]
fn testSelectWithAndWhereConditions() {
    let conn = conn();
    let mut qb = QueryBuilder::new(&conn);

    qb.select(&["u.*", "p.*"])
        .from("users", Some("u"))
        .r#where("u.username = ?", [])
        .and_where("u.name = ?", []);

    assert_eq!(
        "SELECT u.*, p.* FROM users u WHERE (u.username = ?) AND (u.name = ?)",
        string(&qb)
    );
}

#[test]
fn testSelectWithOrWhereConditions() {
    let conn = conn();
    let mut qb = QueryBuilder::new(&conn);

    qb.select(&["u.*", "p.*"])
        .from("users", Some("u"))
        .r#where("u.username = ?", [])
        .or_where("u.name = ?", []);

    assert_eq!(
        "SELECT u.*, p.* FROM users u WHERE (u.username = ?) OR (u.name = ?)",
        string(&qb)
    );
}

#[test]
fn testSelectWithOrOrWhereConditions() {
    let conn = conn();
    let mut qb = QueryBuilder::new(&conn);

    qb.select(&["u.*", "p.*"])
        .from("users", Some("u"))
        .or_where("u.username = ?", [])
        .or_where("u.name = ?", []);

    assert_eq!(
        "SELECT u.*, p.* FROM users u WHERE (u.username = ?) OR (u.name = ?)",
        string(&qb)
    );
}

#[test]
fn testSelectWithAndOrWhereConditions() {
    let conn = conn();
    let mut qb = QueryBuilder::new(&conn);

    qb.select(&["u.*", "p.*"])
        .from("users", Some("u"))
        .r#where("u.username = ?", [])
        .and_where("u.username = ?", [])
        .or_where("u.name = ?", [])
        .and_where("u.name = ?", []);

    assert_eq!(
        "SELECT u.*, p.* FROM users u WHERE (((u.username = ?) AND (u.username = ?)) OR (u.name = ?)) AND (u.name = ?)",
        string(&qb)
    );
}

#[test]
fn testSelectGroupBy() {
    let conn = conn();
    let mut qb = QueryBuilder::new(&conn);

    qb.select(&["u.*", "p.*"])
        .from("users", Some("u"))
        .group_by("u.id", &[]);

    assert_eq!("SELECT u.*, p.* FROM users u GROUP BY u.id", string(&qb));
}

#[test]
fn testSelectAddGroupBy() {
    let conn = conn();
    let mut qb = QueryBuilder::new(&conn);

    qb.select(&["u.*", "p.*"])
        .from("users", Some("u"))
        .group_by("u.id", &[])
        .add_group_by("u.foo", &[]);

    assert_eq!(
        "SELECT u.*, p.* FROM users u GROUP BY u.id, u.foo",
        string(&qb)
    );
}

#[test]
fn testSelectAddGroupBys() {
    let conn = conn();
    let mut qb = QueryBuilder::new(&conn);

    qb.select(&["u.*", "p.*"])
        .from("users", Some("u"))
        .group_by("u.id", &[])
        .add_group_by("u.foo", &["u.bar"]);

    assert_eq!(
        "SELECT u.*, p.* FROM users u GROUP BY u.id, u.foo, u.bar",
        string(&qb)
    );
}

#[test]
fn testSelectHaving() {
    let conn = conn();
    let mut qb = QueryBuilder::new(&conn);

    qb.select(&["u.*", "p.*"])
        .from("users", Some("u"))
        .group_by("u.id", &[])
        .having("u.name = ?", []);

    assert_eq!(
        "SELECT u.*, p.* FROM users u GROUP BY u.id HAVING u.name = ?",
        string(&qb)
    );
}

#[test]
fn testSelectAndHaving() {
    let conn = conn();
    let mut qb = QueryBuilder::new(&conn);

    qb.select(&["u.*", "p.*"])
        .from("users", Some("u"))
        .group_by("u.id", &[])
        .and_having("u.name = ?", []);

    assert_eq!(
        "SELECT u.*, p.* FROM users u GROUP BY u.id HAVING u.name = ?",
        string(&qb)
    );
}

#[test]
fn testSelectHavingAndHaving() {
    let conn = conn();
    let mut qb = QueryBuilder::new(&conn);

    qb.select(&["u.*", "p.*"])
        .from("users", Some("u"))
        .group_by("u.id", &[])
        .having("u.name = ?", [])
        .and_having("u.username = ?", []);

    assert_eq!(
        "SELECT u.*, p.* FROM users u GROUP BY u.id HAVING (u.name = ?) AND (u.username = ?)",
        string(&qb)
    );
}

#[test]
fn testSelectHavingOrHaving() {
    let conn = conn();
    let mut qb = QueryBuilder::new(&conn);

    qb.select(&["u.*", "p.*"])
        .from("users", Some("u"))
        .group_by("u.id", &[])
        .having("u.name = ?", [])
        .or_having("u.username = ?", []);

    assert_eq!(
        "SELECT u.*, p.* FROM users u GROUP BY u.id HAVING (u.name = ?) OR (u.username = ?)",
        string(&qb)
    );
}

#[test]
fn testSelectOrHavingOrHaving() {
    let conn = conn();
    let mut qb = QueryBuilder::new(&conn);

    qb.select(&["u.*", "p.*"])
        .from("users", Some("u"))
        .group_by("u.id", &[])
        .or_having("u.name = ?", [])
        .or_having("u.username = ?", []);

    assert_eq!(
        "SELECT u.*, p.* FROM users u GROUP BY u.id HAVING (u.name = ?) OR (u.username = ?)",
        string(&qb)
    );
}

#[test]
fn testSelectHavingAndOrHaving() {
    let conn = conn();
    let mut qb = QueryBuilder::new(&conn);

    qb.select(&["u.*", "p.*"])
        .from("users", Some("u"))
        .group_by("u.id", &[])
        .having("u.name = ?", [])
        .or_having("u.username = ?", [])
        .and_having("u.username = ?", []);

    assert_eq!(
        "SELECT u.*, p.* FROM users u GROUP BY u.id HAVING ((u.name = ?) OR (u.username = ?)) AND (u.username = ?)",
        string(&qb)
    );
}

#[test]
fn testSelectOrderBy() {
    let conn = conn();
    let mut qb = QueryBuilder::new(&conn);

    qb.select(&["u.*", "p.*"])
        .from("users", Some("u"))
        .order_by("u.name", None);

    assert_eq!("SELECT u.*, p.* FROM users u ORDER BY u.name", string(&qb));
}

#[test]
fn testSelectAddOrderBy() {
    let conn = conn();
    let mut qb = QueryBuilder::new(&conn);

    qb.select(&["u.*", "p.*"])
        .from("users", Some("u"))
        .order_by("u.name", None)
        .add_order_by("u.username", Some("DESC"));

    assert_eq!(
        "SELECT u.*, p.* FROM users u ORDER BY u.name, u.username DESC",
        string(&qb)
    );
}

#[test]
fn testSelectAddAddOrderBy() {
    let conn = conn();
    let mut qb = QueryBuilder::new(&conn);

    qb.select(&["u.*", "p.*"])
        .from("users", Some("u"))
        .add_order_by("u.name", None)
        .add_order_by("u.username", Some("DESC"));

    assert_eq!(
        "SELECT u.*, p.* FROM users u ORDER BY u.name, u.username DESC",
        string(&qb)
    );
}

#[test]
fn testEmptySelect() {
    let conn = conn();
    let mut qb = QueryBuilder::new(&conn);
    let qb2: *const QueryBuilder<'_, Conn> = qb.select(&[]);

    assert!(std::ptr::eq(&raw const qb, qb2));

    query_exception(qb.get_sql());
}

#[test]
fn testSelectAddSelect() {
    let conn = conn();
    let mut qb = QueryBuilder::new(&conn);

    qb.select(&["u.*"])
        .add_select("p.*", &[])
        .from("users", Some("u"));

    assert_eq!("SELECT u.*, p.* FROM users u", string(&qb));
}

#[test]
fn testSelectMultipleFrom() {
    let conn = conn();
    let mut qb = QueryBuilder::new(&conn);

    qb.select(&["u.*"])
        .add_select("p.*", &[])
        .from("users", Some("u"))
        .from("phonenumbers", Some("p"));

    assert_eq!("SELECT u.*, p.* FROM users u, phonenumbers p", string(&qb));
}

#[test]
fn testUpdate() {
    let conn = conn();
    let mut qb = QueryBuilder::new(&conn);
    qb.update("users").set("foo", "?").set("bar", "?");

    assert_eq!("UPDATE users SET foo = ?, bar = ?", string(&qb));
}

#[test]
fn testUpdateWhere() {
    let conn = conn();
    let mut qb = QueryBuilder::new(&conn);
    qb.update("users").set("foo", "?").r#where("foo = ?", []);

    assert_eq!("UPDATE users SET foo = ? WHERE foo = ?", string(&qb));
}

#[test]
fn testDelete() {
    let conn = conn();
    let mut qb = QueryBuilder::new(&conn);
    qb.delete("users");

    assert_eq!("DELETE FROM users", string(&qb));
}

#[test]
fn testDeleteWhere() {
    let conn = conn();
    let mut qb = QueryBuilder::new(&conn);
    qb.delete("users").r#where("u.foo = ?", []);

    assert_eq!("DELETE FROM users WHERE u.foo = ?", string(&qb));
}

#[test]
fn testInsertValues() {
    let conn = conn();
    let mut qb = QueryBuilder::new(&conn);
    qb.insert("users").values(&[("foo", "?"), ("bar", "?")]);

    assert_eq!("INSERT INTO users (foo, bar) VALUES(?, ?)", string(&qb));
}

#[test]
fn testInsertReplaceValues() {
    let conn = conn();
    let mut qb = QueryBuilder::new(&conn);
    qb.insert("users")
        .values(&[("foo", "?"), ("bar", "?")])
        .values(&[("bar", "?"), ("foo", "?")]);

    assert_eq!("INSERT INTO users (bar, foo) VALUES(?, ?)", string(&qb));
}

#[test]
fn testInsertSetValue() {
    let conn = conn();
    let mut qb = QueryBuilder::new(&conn);
    qb.insert("users")
        .set_value("foo", "bar")
        .set_value("bar", "?")
        .set_value("foo", "?");

    assert_eq!("INSERT INTO users (foo, bar) VALUES(?, ?)", string(&qb));
}

#[test]
fn testInsertValuesSetValue() {
    let conn = conn();
    let mut qb = QueryBuilder::new(&conn);
    qb.insert("users")
        .values(&[("foo", "?")])
        .set_value("bar", "?");

    assert_eq!("INSERT INTO users (foo, bar) VALUES(?, ?)", string(&qb));
}

#[test]
fn testSetMaxResults() {
    let rows = provider("Query/QueryBuilderTest", "testSetMaxResults");
    assert_eq!(rows.len(), 2);
    for (name, args) in rows {
        let max_results = args[0][1].as_i64();
        assert!(max_results.is_some() || args[0][0] == "null", "{name}");

        let conn = conn();
        let mut qb = QueryBuilder::new(&conn);
        qb.set_max_results(max_results);

        assert_eq!(max_results, qb.get_max_results(), "{name}");
    }
}

#[test]
fn testSetFirstResult() {
    let conn = conn();
    let mut qb = QueryBuilder::new(&conn);
    qb.set_first_result(10);

    assert_eq!(10, qb.get_first_result());
}

fn prepare_query_builder_to_reset(conn: &Conn) -> QueryBuilder<'_, Conn> {
    let mut qb = QueryBuilder::new(conn);
    qb.select(&["u.*"])
        .distinct(true)
        .from("users", Some("u"))
        .r#where("u.name = ?", [])
        .order_by("u.name", Some("ASC"));

    assert_eq!(
        "SELECT DISTINCT u.* FROM users u WHERE u.name = ? ORDER BY u.name ASC",
        string(&qb)
    );

    qb
}

#[test]
fn testResetDistinct() {
    let conn = conn();
    let mut qb = prepare_query_builder_to_reset(&conn);
    qb.distinct(false);

    assert_eq!(
        "SELECT u.* FROM users u WHERE u.name = ? ORDER BY u.name ASC",
        string(&qb)
    );
}

#[test]
fn testResetWhere() {
    let conn = conn();
    let mut qb = prepare_query_builder_to_reset(&conn);
    qb.reset_where();

    assert_eq!(
        "SELECT DISTINCT u.* FROM users u ORDER BY u.name ASC",
        string(&qb)
    );
}

#[test]
fn testResetOrderBy() {
    let conn = conn();
    let mut qb = prepare_query_builder_to_reset(&conn);
    qb.reset_order_by();

    assert_eq!(
        "SELECT DISTINCT u.* FROM users u WHERE u.name = ?",
        string(&qb)
    );
}

fn prepare_grouped_query_builder_to_reset(conn: &Conn) -> QueryBuilder<'_, Conn> {
    let mut qb = QueryBuilder::new(conn);
    qb.select(&["u.country", "COUNT(*)"])
        .from("users", Some("u"))
        .group_by("u.country", &[])
        .having("COUNT(*) > ?", [])
        .order_by("COUNT(*)", Some("DESC"));

    assert_eq!(
        "SELECT u.country, COUNT(*) FROM users u GROUP BY u.country HAVING COUNT(*) > ? ORDER BY COUNT(*) DESC",
        string(&qb)
    );

    qb
}

#[test]
fn testResetHaving() {
    let conn = conn();
    let mut qb = prepare_grouped_query_builder_to_reset(&conn);
    qb.reset_having();

    assert_eq!(
        "SELECT u.country, COUNT(*) FROM users u GROUP BY u.country ORDER BY COUNT(*) DESC",
        string(&qb)
    );
}

#[test]
fn testResetGroupBy() {
    let conn = conn();
    let mut qb = prepare_grouped_query_builder_to_reset(&conn);
    qb.reset_group_by();

    assert_eq!(
        "SELECT u.country, COUNT(*) FROM users u HAVING COUNT(*) > ? ORDER BY COUNT(*) DESC",
        string(&qb)
    );
}

#[test]
fn testCreateNamedParameter() {
    let conn = conn();
    let mut qb = QueryBuilder::new(&conn);

    let parameter = qb.create_named_parameter(10, ParameterType::Integer, None);
    let predicate = qb.expr().eq("u.name", &parameter);
    qb.select(&["u.*"])
        .from("users", Some("u"))
        .r#where(predicate, []);

    assert_eq!(
        "SELECT u.* FROM users u WHERE u.name = :dcValue1",
        string(&qb)
    );
    assert_eq!(Value::Int(10), qb.get_parameter("dcValue1"));
    assert_eq!(
        WrapperParameterType::Parameter(ParameterType::Integer),
        qb.get_parameter_type("dcValue1")
    );
}

#[test]
fn testCreateNamedParameterCustomPlaceholder() {
    let conn = conn();
    let mut qb = QueryBuilder::new(&conn);

    let parameter = qb.create_named_parameter(10, ParameterType::Integer, Some(":test"));
    let predicate = qb.expr().eq("u.name", &parameter);
    qb.select(&["u.*"])
        .from("users", Some("u"))
        .r#where(predicate, []);

    assert_eq!("SELECT u.* FROM users u WHERE u.name = :test", string(&qb));
    assert_eq!(Value::Int(10), qb.get_parameter("test"));
    assert_eq!(
        WrapperParameterType::Parameter(ParameterType::Integer),
        qb.get_parameter_type("test")
    );
}

#[test]
fn testCreatePositionalParameter() {
    let conn = conn();
    let mut qb = QueryBuilder::new(&conn);

    let parameter = qb.create_positional_parameter(10, ParameterType::Integer);
    let predicate = qb.expr().eq("u.name", &parameter);
    qb.select(&["u.*"])
        .from("users", Some("u"))
        .r#where(predicate, []);

    assert_eq!("SELECT u.* FROM users u WHERE u.name = ?", string(&qb));
    assert_eq!(Value::Int(10), qb.get_parameter(0));
    assert_eq!(
        WrapperParameterType::Parameter(ParameterType::Integer),
        qb.get_parameter_type(0)
    );
}

#[test]
fn testReferenceJoinFromJoin() {
    let conn = conn();
    let mut qb = QueryBuilder::new(&conn);

    qb.select(&["COUNT(DISTINCT news.id)"])
        .from("cb_newspages", Some("news"))
        .inner_join(
            "news",
            "nodeversion",
            "nv",
            Some("nv.refId = news.id AND nv.refEntityname='News'"),
        )
        .inner_join(
            "invalid",
            "nodetranslation",
            "nt",
            Some("nv.nodetranslation = nt.id"),
        )
        .inner_join("nt", "node", "n", Some("nt.node = n.id"))
        .r#where("nt.lang = :lang AND n.deleted != 1", []);

    let e = query_exception(qb.get_sql());
    assert_eq!(QueryExceptionKind::UnknownAlias, e.kind());
    assert_eq!(
        "The given alias \"invalid\" is not part of any FROM or JOIN clause table. \
         The currently registered aliases are: news, nv.",
        e.message()
    );
}

#[test]
fn testSelectFromMasterWithWhereOnJoinedTables() {
    let conn = conn();
    let mut qb = QueryBuilder::new(&conn);

    qb.select(&["COUNT(DISTINCT news.id)"])
        .from("newspages", Some("news"))
        .inner_join(
            "news",
            "nodeversion",
            "nv",
            Some("nv.refId = news.id AND nv.refEntityname='Entity\\News'"),
        )
        .inner_join(
            "nv",
            "nodetranslation",
            "nt",
            Some("nv.nodetranslation = nt.id"),
        )
        .inner_join("nt", "node", "n", Some("nt.node = n.id"))
        .r#where("nt.lang = ?", [])
        .and_where("n.deleted = 0", []);

    assert_eq!(
        "SELECT COUNT(DISTINCT news.id) FROM newspages news \
         INNER JOIN nodeversion nv ON nv.refId = news.id AND nv.refEntityname='Entity\\News' \
         INNER JOIN nodetranslation nt ON nv.nodetranslation = nt.id \
         INNER JOIN node n ON nt.node = n.id WHERE (nt.lang = ?) AND (n.deleted = 0)",
        qb.get_sql().unwrap()
    );
}

#[test]
fn testSelectWithMultipleFromAndJoins() {
    let conn = conn();
    let mut qb = QueryBuilder::new(&conn);

    qb.select(&["DISTINCT u.id"])
        .from("users", Some("u"))
        .from("articles", Some("a"))
        .inner_join("u", "permissions", "p", Some("p.user_id = u.id"))
        .inner_join("a", "comments", "c", Some("c.article_id = a.id"))
        .r#where("u.id = a.user_id", [])
        .and_where("p.read = 1", []);

    assert_eq!(
        "SELECT DISTINCT u.id FROM users u \
         INNER JOIN permissions p ON p.user_id = u.id, articles a \
         INNER JOIN comments c ON c.article_id = a.id \
         WHERE (u.id = a.user_id) AND (p.read = 1)",
        qb.get_sql().unwrap()
    );
}

#[test]
fn testSelectWithJoinsWithMultipleOnConditionsParseOrder() {
    let conn = conn();
    let mut qb = QueryBuilder::new(&conn);

    qb.select(&["a.id"])
        .from("table_a", Some("a"))
        .join("a", "table_b", "b", Some("a.fk_b = b.id"))
        .join(
            "b",
            "table_c",
            "c",
            Some("c.fk_b = b.id AND b.language = ?"),
        )
        .join("a", "table_d", "d", Some("a.fk_d = d.id"))
        .join("c", "table_e", "e", Some("e.fk_c = c.id AND e.fk_d = d.id"));

    assert_eq!(
        "SELECT a.id \
         FROM table_a a \
         INNER JOIN table_b b ON a.fk_b = b.id \
         INNER JOIN table_d d ON a.fk_d = d.id \
         INNER JOIN table_c c ON c.fk_b = b.id AND b.language = ? \
         INNER JOIN table_e e ON e.fk_c = c.id AND e.fk_d = d.id",
        string(&qb)
    );
}

#[test]
fn testSelectWithMultipleFromsAndJoinsWithMultipleOnConditionsParseOrder() {
    let conn = conn();
    let mut qb = QueryBuilder::new(&conn);

    qb.select(&["a.id"])
        .from("table_a", Some("a"))
        .from("table_f", Some("f"))
        .join("a", "table_b", "b", Some("a.fk_b = b.id"))
        .join(
            "b",
            "table_c",
            "c",
            Some("c.fk_b = b.id AND b.language = ?"),
        )
        .join("a", "table_d", "d", Some("a.fk_d = d.id"))
        .join("c", "table_e", "e", Some("e.fk_c = c.id AND e.fk_d = d.id"))
        .join("f", "table_g", "g", Some("f.fk_g = g.id"));

    assert_eq!(
        "SELECT a.id \
         FROM table_a a \
         INNER JOIN table_b b ON a.fk_b = b.id \
         INNER JOIN table_d d ON a.fk_d = d.id \
         INNER JOIN table_c c ON c.fk_b = b.id AND b.language = ? \
         INNER JOIN table_e e ON e.fk_c = c.id AND e.fk_d = d.id, \
         table_f f \
         INNER JOIN table_g g ON f.fk_g = g.id",
        string(&qb)
    );
}

#[test]
fn testClone() {
    let conn = conn();
    let mut qb = QueryBuilder::new(&conn);

    qb.select(&["u.id"])
        .from("users", Some("u"))
        .r#where("u.id = :test", []);

    // `(object) 1`
    let mut properties = PhpArray::new();
    properties.insert("scalar", Value::Int(1));
    qb.set_parameter(
        ":test",
        StdClass::object(properties),
        DEFAULT_PARAMETER_TYPE,
    );

    let clone = qb.clone();

    assert_eq!(string(&qb), string(&clone));

    qb.and_where("u.id = 1", []);

    assert_ne!(qb.get_parameters(), clone.get_parameters());
}

#[test]
fn testSimpleSelectWithoutTableAlias() {
    let conn = conn();
    let mut qb = QueryBuilder::new(&conn);

    qb.select(&["id"]).from("users", None);

    assert_eq!("SELECT id FROM users", string(&qb));
}

#[test]
fn testSimpleSelectWithMatchingTableAlias() {
    let conn = conn();
    let mut qb = QueryBuilder::new(&conn);

    qb.select(&["id"]).from("users", Some("users"));

    assert_eq!("SELECT id FROM users", string(&qb));
}

#[test]
fn testSelectWithSimpleWhereWithoutTableAlias() {
    let conn = conn();
    let mut qb = QueryBuilder::new(&conn);

    qb.select(&["id", "name"])
        .from("users", None)
        .r#where("awesome=9001", []);

    assert_eq!("SELECT id, name FROM users WHERE awesome=9001", string(&qb));
}

#[test]
fn testComplexSelectWithoutTableAliases() {
    let conn = conn();
    let mut qb = QueryBuilder::new(&conn);

    qb.select(&["DISTINCT users.id"])
        .from("users", None)
        .from("articles", None)
        .inner_join("users", "permissions", "p", Some("p.user_id = users.id"))
        .inner_join(
            "articles",
            "comments",
            "c",
            Some("c.article_id = articles.id"),
        )
        .r#where("users.id = articles.user_id", [])
        .and_where("p.read = 1", []);

    assert_eq!(
        "SELECT DISTINCT users.id FROM users \
         INNER JOIN permissions p ON p.user_id = users.id, articles \
         INNER JOIN comments c ON c.article_id = articles.id \
         WHERE (users.id = articles.user_id) AND (p.read = 1)",
        qb.get_sql().unwrap()
    );
}

#[test]
fn testComplexSelectWithSomeTableAliases() {
    let conn = conn();
    let mut qb = QueryBuilder::new(&conn);

    qb.select(&["u.id"])
        .from("users", Some("u"))
        .from("articles", None)
        .inner_join("u", "permissions", "p", Some("p.user_id = u.id"))
        .inner_join(
            "articles",
            "comments",
            "c",
            Some("c.article_id = articles.id"),
        );

    assert_eq!(
        "SELECT u.id FROM users u \
         INNER JOIN permissions p ON p.user_id = u.id, articles \
         INNER JOIN comments c ON c.article_id = articles.id",
        qb.get_sql().unwrap()
    );
}

#[test]
fn testSelectAllFromTableWithoutTableAlias() {
    let conn = conn();
    let mut qb = QueryBuilder::new(&conn);

    qb.select(&["users.*"]).from("users", None);

    assert_eq!("SELECT users.* FROM users", string(&qb));
}

#[test]
fn testSelectAllWithoutTableAlias() {
    let conn = conn();
    let mut qb = QueryBuilder::new(&conn);

    qb.select(&["*"]).from("users", None);

    assert_eq!("SELECT * FROM users", string(&qb));
}

#[test]
fn testSelectWithCTE() {
    let conn = conn();
    let mut qb = QueryBuilder::new(&conn);

    let mut cte_query_builder1 = qb.sub();
    cte_query_builder1
        .select(&["ta.id", "ta.name", "ta.table_b_id"])
        .from("table_a", Some("ta"))
        .r#where("ta.name LIKE :name", []);

    let mut cte_query_builder2 = qb.sub();
    cte_query_builder2
        .select(&["ca.id AS virtual_id, ca.name AS virtual_name"])
        .from("cte_a", Some("ca"))
        .join("ca", "table_b", "tb", Some("ca.table_b_id = tb.id"));

    qb.with("cte_a", cte_query_builder1, None)
        .unwrap()
        .with(
            "cte_b",
            cte_query_builder2,
            Some(&["virtual_id", "virtual_name"]),
        )
        .unwrap()
        .select(&["cb.*"])
        .from("cte_b", Some("cb"));

    assert_eq!(
        "WITH cte_a AS (SELECT ta.id, ta.name, ta.table_b_id FROM table_a ta WHERE ta.name LIKE :name)\
         , cte_b (virtual_id, virtual_name) AS \
         (SELECT ca.id AS virtual_id, ca.name AS virtual_name \
         FROM cte_a ca INNER JOIN table_b tb ON ca.table_b_id = tb.id) \
         SELECT cb.* FROM cte_b cb",
        string(&qb)
    );
}

#[test]
fn testSelectWithCTEAndEmptyColumns() {
    let conn = conn();
    let mut qb = QueryBuilder::new(&conn);
    let e = qb.with("cte_a", "SELECT 1 as id", Some(&[])).unwrap_err();

    assert_eq!(QueryExceptionKind::Query, e.kind());
    assert_eq!(
        "Columns defined in CTE \"cte_a\" should not be an empty array.",
        e.message()
    );
}

#[test]
fn testGetParameterType() {
    let conn = conn();
    let mut qb = QueryBuilder::new(&conn);

    qb.select(&["*"]).from("users", None);

    assert_eq!(
        WrapperParameterType::Parameter(ParameterType::String),
        qb.get_parameter_type("name")
    );

    qb.r#where("name = :name", []);
    qb.set_parameter("name", "foo", DEFAULT_PARAMETER_TYPE);

    assert_eq!(
        WrapperParameterType::Parameter(ParameterType::String),
        qb.get_parameter_type("name")
    );

    qb.set_parameter("name", "foo", ParameterType::Integer);

    assert_eq!(
        WrapperParameterType::Parameter(ParameterType::Integer),
        qb.get_parameter_type("name")
    );
}

#[test]
fn testGetParameterTypes() {
    let conn = conn();
    let mut qb = QueryBuilder::new(&conn);

    qb.select(&["*"]).from("users", None);

    assert_eq!(qb.get_parameter_types(), &[]);

    qb.r#where("name = :name", []);
    qb.set_parameter("name", "foo", DEFAULT_PARAMETER_TYPE);

    assert_eq!(
        qb.get_parameter_types(),
        &[(Key::from("name"), ParameterType::String.into())]
    );

    qb.r#where("is_active = :isActive", []);
    qb.set_parameter("isActive", true, ParameterType::Boolean);

    assert_eq!(
        qb.get_parameter_types(),
        &[
            (Key::from("name"), ParameterType::String.into()),
            (Key::from("isActive"), ParameterType::Boolean.into()),
        ]
    );
}

#[test]
fn testArrayParameters() {
    let conn = conn();
    let mut qb = QueryBuilder::new(&conn);

    qb.select(&["*"]).from("users", None);

    assert_eq!(qb.get_parameter_types(), &[]);

    qb.r#where("id IN (:ids)", []);
    qb.set_parameter(
        "ids",
        PhpArray::list([Value::Int(1), Value::Int(2), Value::Int(3)]),
        ArrayParameterType::Integer,
    );

    qb.and_where("name IN (:names)", []);
    qb.set_parameter(
        "names",
        PhpArray::list([Value::from("john"), Value::from("jane")]),
        ArrayParameterType::String,
    );

    qb.and_where("hash IN (:hashes)", []);
    qb.set_parameter(
        "hashes",
        PhpArray::list([
            Value::String(PhpString::from(vec![0xDE, 0xAD, 0xBE, 0xEF])),
            Value::String(PhpString::from(vec![0xC0, 0xDE, 0xF0, 0x0D])),
        ]),
        ArrayParameterType::Binary,
    );

    assert_eq!(
        WrapperParameterType::Array(ArrayParameterType::Integer),
        qb.get_parameter_type("ids")
    );
    assert_eq!(
        WrapperParameterType::Array(ArrayParameterType::String),
        qb.get_parameter_type("names")
    );
    assert_eq!(
        WrapperParameterType::Array(ArrayParameterType::Binary),
        qb.get_parameter_type("hashes")
    );

    assert_eq!(
        qb.get_parameter_types(),
        &[
            (Key::from("ids"), ArrayParameterType::Integer.into()),
            (Key::from("names"), ArrayParameterType::String.into()),
            (Key::from("hashes"), ArrayParameterType::Binary.into()),
        ]
    );
}

#[test]
fn testJoinWithNonUniqueAliasThrowsException() {
    let conn = conn();
    let mut qb = QueryBuilder::new(&conn);

    qb.select(&["a.id"]).from("table_a", Some("a")).join(
        "a",
        "table_b",
        "a",
        Some("a.fk_b = a.id"),
    );

    let e = query_exception(qb.get_sql());
    assert_eq!(QueryExceptionKind::NonUniqueAlias, e.kind());
    assert_eq!(
        "The given alias \"a\" is not unique in FROM and JOIN clause table. The currently registered aliases are: a.",
        e.message()
    );
}

#[test]
fn testUnionOnlyThrowException() {
    let conn = conn();
    let mut qb = QueryBuilder::new(&conn);
    qb.union("SELECT 1 AS field_one");

    let e = query_exception(qb.get_sql());
    assert_eq!(QueryExceptionKind::Query, e.kind());
    assert_eq!(
        "Insufficient UNION parts give, need at least 2. \
         Please use union() and addUnion() to set enough UNION parts.",
        e.message()
    );
}

#[test]
fn testUnionWAllAndLimitClauseReturnsUnionAllQuery() {
    let conn = conn();
    let mut qb = QueryBuilder::new(&conn);
    qb.union("SELECT 1 AS field_one")
        .add_union("SELECT 2 as field_one", UnionType::All)
        .unwrap()
        .set_max_results(Some(10))
        .set_first_result(10);

    assert_eq!(
        "SELECT 1 AS field_one UNION ALL SELECT 2 as field_one LIMIT 10 OFFSET 10",
        qb.get_sql().unwrap()
    );
}

#[test]
fn testUnionAllWithOrderByReturnsUnionAllQueryWithOrderBy() {
    let conn = conn();
    let mut qb = QueryBuilder::new(&conn);
    qb.union("SELECT 1 AS field_one")
        .add_union("SELECT 2 as field_one", UnionType::All)
        .unwrap()
        .order_by("field_one", Some("ASC"));

    assert_eq!(
        "SELECT 1 AS field_one UNION ALL SELECT 2 as field_one ORDER BY field_one ASC",
        qb.get_sql().unwrap()
    );
}

#[test]
fn testOnlyAddUnionThrowQueryException() {
    let conn = conn();
    let mut qb = QueryBuilder::new(&conn);
    let e = qb
        .add_union("SELECT 1 AS field_one", UnionType::Distinct)
        .unwrap_err();

    assert_eq!(QueryExceptionKind::Query, e.kind());
}

#[test]
fn testUnionAndAddUnionReturnsUnionQuery() {
    let conn = conn();
    let mut qb = QueryBuilder::new(&conn);
    qb.union("SELECT 1 AS field_one")
        .add_union("SELECT 2 as field_one", UnionType::Distinct)
        .unwrap();

    assert_eq!(
        "SELECT 1 AS field_one UNION SELECT 2 as field_one",
        qb.get_sql().unwrap()
    );
}

#[test]
fn testUnionIsDistinctByDefault() {
    let conn = conn();
    let mut qb = QueryBuilder::new(&conn);
    qb.union("SELECT 1 AS field_one")
        .add_union("SELECT 2 as field_one", UnionType::default())
        .unwrap();

    assert_eq!(
        "SELECT 1 AS field_one UNION SELECT 2 as field_one",
        qb.get_sql().unwrap()
    );
}

#[test]
fn testUnionAndOrderByReturnsUnionQueryWithOrderBy() {
    let conn = conn();
    let mut qb = QueryBuilder::new(&conn);
    qb.union("SELECT 1 AS field_one")
        .add_union("SELECT 2 as field_one", UnionType::Distinct)
        .unwrap()
        .order_by("field_one", Some("ASC"));

    assert_eq!(
        "SELECT 1 AS field_one UNION SELECT 2 as field_one ORDER BY field_one ASC",
        qb.get_sql().unwrap()
    );
}
