//! Port of tests/Schema/Name/Parser/GenericNameParserTest.php at
//! doctrine/dbal c435cd7ea33e17d2a89288caf994cf3d4549b879. Names and
//! assertions are PHPUnit's; the data providers build objects, so their
//! rows are written in place. `GenericNameParser::parse()` is
//! `parse_generic_name()`.
//!
//! Upstream: doctrine/dbal tests/Schema/Name/Parser/GenericNameParserTest.php

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    reason = "test helpers; a panic is a test failure"
)]
#![allow(non_snake_case)]

use doctrine_dbal::schema::name::{Identifier, parse_generic_name};

fn q(value: &str) -> Identifier {
    Identifier::quoted(value).unwrap()
}

fn u(value: &str) -> Identifier {
    Identifier::unquoted(value).unwrap()
}

#[test]
fn testValidInput() {
    let rows: Vec<(&str, Vec<Identifier>)> = vec![
        ("table", vec![u("table")]),
        ("schema.table", vec![u("schema"), u("table")]),
        ("\"example.com\"", vec![q("example.com")]),
        ("`example.com`", vec![q("example.com")]),
        ("[example.com]", vec![q("example.com")]),
        (
            "a.\"b\".c.`d`.e.[f].g",
            vec![u("a"), q("b"), u("c"), q("d"), u("e"), q("f"), u("g")],
        ),
        ("\"schema\".\"table\"", vec![q("schema"), q("table")]),
        ("`schema`.`table`", vec![q("schema"), q("table")]),
        ("[schema].[table]", vec![q("schema"), q("table")]),
        (
            "schema.\"example.com\"",
            vec![u("schema"), q("example.com")],
        ),
        (
            "\"a\"\"b\".`c``d`.[e]]f]",
            vec![q("a\"b"), q("c`d"), q("e]f")],
        ),
        (
            "schéma.\"übermäßigkeit\".`àçcênt`.[éxtrême].çhâràctér",
            vec![
                u("schéma"),
                q("übermäßigkeit"),
                q("àçcênt"),
                q("éxtrême"),
                u("çhâràctér"),
            ],
        ),
        (
            "\" spaced identifier \".more",
            vec![q(" spaced identifier "), u("more")],
        ),
        ("0.\"0\".`0`.[0]", vec![u("0"), q("0"), q("0"), q("0")]),
    ];
    for (input, expected) in rows {
        let name = parse_generic_name(input).unwrap();
        assert_eq!(name.get_identifiers(), expected.as_slice(), "{input}");
    }
}

#[test]
fn testInvalidInput() {
    const NS: &str = "Doctrine\\DBAL\\Schema\\Name\\Parser\\Exception\\";
    for (input, expected) in [
        ("", "ExpectedNextIdentifier"),
        ("\"example.com", "UnableToParseIdentifier"),
        ("`example.com", "UnableToParseIdentifier"),
        ("[example.com", "UnableToParseIdentifier"),
        ("schema.\"example.com", "UnableToParseIdentifier"),
        ("schema.[example.com", "UnableToParseIdentifier"),
        ("schema.`example.com", "UnableToParseIdentifier"),
        ("schema.", "ExpectedNextIdentifier"),
        ("schema..", "UnableToParseIdentifier"),
        (".table", "UnableToParseIdentifier"),
        ("schema.table name", "ExpectedDot"),
        ("\"schema.[example.com]", "UnableToParseIdentifier"),
    ] {
        let e = parse_generic_name(input).unwrap_err();
        assert!(e.is_instance_of(&format!("{NS}{expected}")), "{input}: {e}");
    }
}
