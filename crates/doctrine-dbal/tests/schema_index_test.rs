//! Port of tests/Schema/IndexTest.php at doctrine/dbal
//! c435cd7ea33e17d2a89288caf994cf3d4549b879. Names and assertions are
//! PHPUnit's; the data provider and `#[TestWith]` rows are written in
//! place.
//!
//! Upstream: doctrine/dbal tests/Schema/IndexTest.php

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    reason = "test helpers; a panic is a test failure"
)]
#![allow(non_snake_case)]

mod schema_support;

use doctrine_dbal::schema::name::{Identifier, UnqualifiedName};
use doctrine_dbal::schema::{Index, IndexType, IndexedColumn};
use php_value::{PhpArray, Value, php_array};
use schema_support::{expect_deprecation, expect_exception, expect_no_deprecation};

const LINK_6787: &str = "https://github.com/doctrine/dbal/pull/6787";
const LINK_6886: &str = "https://github.com/doctrine/dbal/pull/6886";

fn index(
    name: Option<&str>,
    columns: &[&str],
    unique: bool,
    primary: bool,
    flags: &[&str],
    options: PhpArray,
) -> Index {
    Index::new(
        name,
        columns.iter().copied(),
        unique,
        primary,
        flags.iter().copied(),
        options,
    )
    .unwrap()
}

fn create_index(unique: bool, primary: bool, options: PhpArray) -> Index {
    index(Some("foo"), &["bar", "baz"], unique, primary, &[], options)
}

fn strings(values: &[&str]) -> Vec<String> {
    values.iter().map(|v| (*v).to_owned()).collect()
}

#[test]
fn testCreateIndex() {
    let idx = create_index(false, false, PhpArray::new());
    assert_eq!(
        idx.get_object_name().unwrap(),
        UnqualifiedName::unquoted("foo").unwrap()
    );
    let columns = idx.get_columns();
    assert_eq!(columns.len(), 2);
    assert_eq!(columns, strings(&["bar", "baz"]));
    assert!(!idx.is_unique());
    assert!(!idx.is_primary());
}

#[test]
fn testCreatePrimary() {
    let idx = create_index(false, true, PhpArray::new());
    assert!(idx.is_unique());
    assert!(idx.is_primary());
}

#[test]
fn testCreateUnique() {
    let idx = create_index(true, false, PhpArray::new());
    assert!(idx.is_unique());
    assert!(!idx.is_primary());
}

#[test]
fn testFulfilledByUnique() {
    let idx1 = create_index(true, false, PhpArray::new());
    let idx2 = create_index(true, false, PhpArray::new());
    let idx3 = create_index(false, false, PhpArray::new());
    assert!(idx1.is_fulfilled_by(&idx2));
    assert!(!idx1.is_fulfilled_by(&idx3));
}

#[test]
fn testFulfilledByPrimary() {
    let idx1 = create_index(true, true, PhpArray::new());
    let idx2 = create_index(true, true, PhpArray::new());
    let idx3 = create_index(true, false, PhpArray::new());
    assert!(idx1.is_fulfilled_by(&idx2));
    assert!(!idx1.is_fulfilled_by(&idx3));
}

#[test]
fn testFulfilledByIndex() {
    let idx1 = create_index(false, false, PhpArray::new());
    let idx2 = create_index(false, false, PhpArray::new());
    let pri = create_index(true, true, PhpArray::new());
    let uniq = create_index(true, false, PhpArray::new());
    assert!(idx1.is_fulfilled_by(&idx2));
    assert!(idx1.is_fulfilled_by(&pri));
    assert!(idx1.is_fulfilled_by(&uniq));
}

fn partial_indexes() -> (Index, Index, Index) {
    let without = index(
        Some("without"),
        &["col1", "col2"],
        true,
        false,
        &[],
        php_array![],
    );
    let partial = index(
        Some("partial"),
        &["col1", "col2"],
        true,
        false,
        &[],
        php_array!["where" => "col1 IS NULL"],
    );
    let another = index(
        Some("another"),
        &["col1", "col2"],
        true,
        false,
        &[],
        php_array!["where" => "col1 IS NULL"],
    );
    (without, partial, another)
}

#[test]
fn testFulfilledWithPartial() {
    let (without, partial, another) = partial_indexes();
    assert!(!partial.is_fulfilled_by(&without));
    assert!(!without.is_fulfilled_by(&partial));
    assert!(partial.is_fulfilled_by(&partial));
    assert!(partial.is_fulfilled_by(&another));
    assert!(another.is_fulfilled_by(&partial));
}

#[test]
fn testOverrulesWithPartial() {
    let (without, partial, another) = partial_indexes();
    assert!(!partial.overrules(&without));
    assert!(!without.overrules(&partial));
    assert!(partial.overrules(&partial));
    assert!(partial.overrules(&another));
    assert!(another.overrules(&partial));
}

#[test]
fn testFulfilledWithLength() {
    let rows: [(&str, &[&str], PhpArray, PhpArray, bool); 5] = [
        ("empty", &["column"], php_array![], php_array![], true),
        (
            "same",
            &["column"],
            php_array![0_i64 => 64],
            php_array![0_i64 => 64],
            true,
        ),
        (
            "different",
            &["column"],
            php_array![0_i64 => 32],
            php_array![0_i64 => 64],
            false,
        ),
        (
            "sparse-different-positions",
            &["column1", "column2"],
            php_array![0_i64 => 32],
            php_array![1_i64 => 32],
            false,
        ),
        (
            "sparse-same-positions",
            &["column1", "column2"],
            php_array![0_i64 => Value::Null, 1_i64 => 32],
            php_array![1_i64 => 32],
            true,
        ),
    ];
    for (row, columns, lengths1, lengths2, expected) in rows {
        let index1 = index(
            Some("index1"),
            columns,
            false,
            false,
            &[],
            php_array!["lengths" => lengths1],
        );
        let index2 = index(
            Some("index2"),
            columns,
            false,
            false,
            &[],
            php_array!["lengths" => lengths2],
        );
        assert_eq!(index1.is_fulfilled_by(&index2), expected, "{row}");
        assert_eq!(index2.is_fulfilled_by(&index1), expected, "{row}");
    }
}

#[test]
fn testFlags() {
    let mut idx1 = create_index(false, false, PhpArray::new());
    assert!(!idx1.has_flag("clustered"));
    assert_eq!(idx1.get_flags(), [] as [std::string::String; 0]);

    idx1.add_flag("clustered");
    assert!(idx1.has_flag("clustered"));
    assert!(idx1.has_flag("CLUSTERED"));
    assert_eq!(idx1.get_flags(), strings(&["clustered"]));
    assert!(idx1.is_clustered());

    idx1.remove_flag("clustered");
    assert!(!idx1.has_flag("clustered"));
    assert_eq!(idx1.get_flags(), [] as [std::string::String; 0]);
    assert!(!idx1.is_clustered());
}

#[test]
fn testIndexQuotes() {
    let index = index(
        Some("foo"),
        &["`bar`", "`baz`"],
        false,
        false,
        &[],
        php_array![],
    );
    assert!(index.spans_columns(&strings(&["bar", "baz"])));
    assert!(index.has_column_at_position("bar", 0));
    assert!(index.has_column_at_position("baz", 1));
    assert!(!index.has_column_at_position("bar", 1));
    assert!(!index.has_column_at_position("baz", 0));
}

#[test]
fn testOptions() {
    let idx1 = create_index(false, false, PhpArray::new());
    assert!(!idx1.has_option("where"));
    assert!(idx1.get_options().is_empty());

    let idx2 = create_index(false, false, php_array!["where" => "name IS NULL"]);
    assert!(idx2.has_option("where"));
    assert!(idx2.has_option("WHERE"));
    assert_eq!(
        idx2.get_option("where"),
        Some(&Value::string("name IS NULL"))
    );
    assert_eq!(
        idx2.get_option("WHERE"),
        Some(&Value::string("name IS NULL"))
    );
    assert_eq!(idx2.get_options(), &php_array!["where" => "name IS NULL"]);
}

#[test]
fn testEmptyName() {
    expect_deprecation("https://github.com/doctrine/dbal/pull/6646", || {
        index(None, &["user_id"], false, false, &[], php_array![]);
    });
}

#[test]
fn testGetObjectName() {
    let index = index(
        Some("idx_user_id"),
        &["user_id"],
        false,
        false,
        &[],
        php_array![],
    );
    assert_eq!(
        index.get_object_name().unwrap().get_identifier(),
        &Identifier::unquoted("idx_user_id").unwrap()
    );
}

#[test]
fn testEmptyColumns() {
    let index = expect_deprecation(LINK_6787, || {
        index(Some("idx_user_name"), &[], false, false, &[], php_array![])
    });
    expect_exception("InvalidState", index.get_indexed_columns());
}

#[test]
fn testInvalidColumnName() {
    let index = expect_deprecation(LINK_6787, || {
        index(
            Some("idx_user_name"),
            &["user.name"],
            false,
            false,
            &[],
            php_array![],
        )
    });
    expect_exception("InvalidState", index.get_indexed_columns());
}

#[test]
fn testPrimaryKeyWithColumnLength() {
    let index = expect_deprecation(LINK_6787, || {
        index(
            Some("primary"),
            &["id"],
            false,
            true,
            &[],
            php_array!["lengths" => php_array![0_i64 => 32]],
        )
    });
    expect_exception("InvalidState", index.get_indexed_columns());
}

#[test]
fn testPrimaryKeyWithNullColumnLength() {
    expect_no_deprecation(LINK_6787, || {
        index(
            Some("primary"),
            &["id"],
            false,
            true,
            &[],
            php_array!["lengths" => php_array![0_i64 => Value::Null]],
        );
    });
}

#[test]
fn testNonIntegerColumnLength() {
    let index = expect_deprecation(LINK_6787, || {
        index(
            Some("idx_user_name"),
            &["name"],
            false,
            false,
            &[],
            php_array!["lengths" => php_array![0_i64 => "8"]],
        )
    });
    assert_eq!(
        index.get_indexed_columns().unwrap(),
        [IndexedColumn::new(UnqualifiedName::unquoted("name").unwrap(), Some(8)).unwrap()]
    );
}

#[test]
fn testNonPositiveColumnLength() {
    let index = expect_deprecation(LINK_6787, || {
        index(
            Some("idx_user_name"),
            &["name"],
            false,
            false,
            &[],
            php_array!["lengths" => php_array![0_i64 => -1]],
        )
    });
    expect_exception("InvalidState", index.get_indexed_columns());
}

#[test]
fn testGetIndexedColumns() {
    let index = index(
        Some("idx_user_name"),
        &["first_name", "last_name"],
        false,
        false,
        &[],
        php_array!["lengths" => php_array![0_i64 => 16]],
    );
    let indexed_columns = index.get_indexed_columns().unwrap();
    assert_eq!(indexed_columns.len(), 2);
    assert_eq!(
        indexed_columns[0].get_column_name(),
        &UnqualifiedName::unquoted("first_name").unwrap()
    );
    assert_eq!(indexed_columns[0].get_length(), Some(16));
    assert_eq!(
        indexed_columns[1].get_column_name(),
        &UnqualifiedName::unquoted("last_name").unwrap()
    );
    assert_eq!(indexed_columns[1].get_length(), None);
}

#[test]
fn testUnsupportedFlag() {
    expect_deprecation(LINK_6886, || {
        index(
            Some("idx_user_name"),
            &["name"],
            false,
            false,
            &["banana"],
            php_array![],
        );
    });
}

#[test]
fn testConflictInFlagsSignificantForTypeInference() {
    for (is_unique, flags) in [
        (true, &["fulltext"][..]),
        (true, &["spatial"][..]),
        (false, &["fulltext", "spatial"][..]),
    ] {
        let index = expect_deprecation(LINK_6886, || {
            index(
                Some("idx_user_name"),
                &["name"],
                is_unique,
                false,
                flags,
                php_array![],
            )
        });
        expect_exception("InvalidState", index.get_type());
    }
}

#[test]
fn testConflictInFlagsInsignificantForTypeInference() {
    for (flags, expected_type) in [
        (["fulltext", "clustered"], IndexType::Fulltext),
        (["spatial", "clustered"], IndexType::Spatial),
        (["nonclustered", "clustered"], IndexType::Regular),
    ] {
        let index = expect_deprecation(LINK_6886, || {
            index(
                Some("idx_user_name"),
                &["name"],
                false,
                false,
                &flags,
                php_array![],
            )
        });
        assert_eq!(index.get_type().unwrap(), expected_type, "{flags:?}");
    }
}

#[test]
fn testParseType() {
    for (is_unique, flags, expected_type) in [
        (false, &[][..], IndexType::Regular),
        (false, &["fulltext"][..], IndexType::Fulltext),
        (false, &["spatial"][..], IndexType::Spatial),
        (true, &[][..], IndexType::Unique),
    ] {
        let index = expect_no_deprecation(LINK_6886, || {
            index(
                Some("idx_user_name"),
                &["user_id"],
                is_unique,
                false,
                flags,
                php_array![],
            )
        });
        assert_eq!(index.get_type().unwrap(), expected_type, "{flags:?}");
    }
}

#[test]
fn testGetPredicate() {
    for predicate in [None, Some("is_active = 1")] {
        let index = expect_no_deprecation(LINK_6886, || {
            index(
                Some("idx_user_name"),
                &["user_id"],
                false,
                false,
                &[],
                php_array!["where" => predicate.map_or(Value::Null, Value::string)],
            )
        });
        assert_eq!(
            index.get_predicate().unwrap().as_deref(),
            predicate,
            "{predicate:?}"
        );
    }
}

#[test]
fn testEmptyPredicate() {
    let index = expect_deprecation(LINK_6886, || {
        index(
            Some("idx_user_name"),
            &["user_id"],
            false,
            false,
            &[],
            php_array!["where" => ""],
        )
    });
    expect_exception("InvalidState", index.get_predicate());
}

#[test]
fn testPartialIndexWithConflictingFlags() {
    for flag in ["fulltext", "spatial", "clustered"] {
        expect_deprecation(LINK_6886, || {
            index(
                Some("idx_user_name"),
                &["user_id"],
                false,
                false,
                &[flag],
                php_array!["where" => "is_active = 1"],
            );
        });
    }
}
