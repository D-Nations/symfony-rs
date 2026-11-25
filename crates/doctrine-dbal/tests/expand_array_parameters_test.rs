//! ExpandArrayParametersTest.php at doctrine/dbal
//! c435cd7ea33e17d2a89288caf994cf3d4549b879. Names and assertions are
//! PHPUnit's; the provider rows are upstream's, recorded by running its
//! providers (tests/data/php.json).
//!
//! Upstream: doctrine/dbal tests/Connection/ExpandArrayParametersTest.php

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    reason = "test helpers; a panic is a test failure"
)]
#![allow(non_snake_case)]

mod common;

use std::collections::BTreeMap;

use common::{array, bytes, provider, types};
use doctrine_dbal::sql::Parser;
use doctrine_dbal::{ArrayParametersException, ExpandArrayParameters, WrapperParameterType};
use php_value::vectors::phpunit_equals;
use php_value::{Key, PhpArray, Value};

type Expanded = (Vec<u8>, Vec<Value>, BTreeMap<usize, WrapperParameterType>);

fn expand_array_parameters(
    sql: &[u8],
    params: &PhpArray,
    types: &BTreeMap<Key, WrapperParameterType>,
) -> Result<Expanded, ArrayParametersException> {
    let parser = Parser::new(true);
    let mut visitor = ExpandArrayParameters::new(params, types);

    parser.parse(sql, &mut visitor)?;

    Ok((
        visitor.get_sql().as_bytes().to_vec(),
        visitor.get_parameters().to_vec(),
        visitor.get_types().clone(),
    ))
}

#[test]
fn testExpandListParameters() {
    let rows = provider(
        "Connection/ExpandArrayParametersTest",
        "testExpandListParameters",
    );
    assert_eq!(rows.len(), 31);
    for (name, args) in rows {
        let (query, params, types_) =
            expand_array_parameters(&bytes(&args[0]), &array(&args[1]), &types(&args[2]))
                .unwrap_or_else(|e| panic!("{name}: {e}"));

        assert_eq!(
            String::from_utf8_lossy(&query),
            String::from_utf8_lossy(&bytes(&args[3])),
            "{name}: Query was not rewritten correctly."
        );
        assert!(
            phpunit_equals(
                &Value::Array(array(&args[4])),
                &Value::Array(PhpArray::list(params))
            ),
            "{name}: Params dont match"
        );
        let expected: BTreeMap<usize, WrapperParameterType> = types(&args[5])
            .into_iter()
            .map(|(key, value)| {
                (
                    usize::try_from(key.as_int().expect("int key")).expect("index"),
                    value,
                )
            })
            .collect();
        assert_eq!(expected, types_, "{name}: Types dont match");
    }
}

#[test]
fn testMissingNamedParameter() {
    let rows = provider(
        "Connection/ExpandArrayParametersTest",
        "testMissingNamedParameter",
    );
    assert_eq!(rows.len(), 4);
    for (name, args) in rows {
        let types_ = args.get(2).map(types).unwrap_or_default();
        let result = expand_array_parameters(&bytes(&args[0]), &array(&args[1]), &types_);
        assert!(
            matches!(
                result,
                Err(ArrayParametersException::MissingNamedParameter(_))
            ),
            "{name}: {result:?}"
        );
    }
}

#[test]
fn testMissingPositionalParameter() {
    let rows = provider(
        "Connection/ExpandArrayParametersTest",
        "testMissingPositionalParameter",
    );
    assert_eq!(rows.len(), 2);
    for (name, args) in rows {
        let result = expand_array_parameters(&bytes(&args[0]), &array(&args[1]), &BTreeMap::new());
        assert!(
            matches!(
                result,
                Err(ArrayParametersException::MissingPositionalParameter(_))
            ),
            "{name}: {result:?}"
        );
    }
}
