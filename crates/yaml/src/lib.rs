//! Port of the symfony/yaml component. See README.md for the contract and
//! what is ported so far.
//!
//! Symfony source copyright (c) Fabien Potencier; distributed under MIT.
//!
//! Upstream: symfony/yaml Yaml.php

#![forbid(unsafe_code)]

pub mod escaper;
pub mod exception;
pub mod inline;
pub mod parser_state;
pub mod tag;
mod timestamp;
pub mod unescaper;

use std::ops::{BitOr, BitOrAssign};

pub use exception::{DumpException, InvalidArgumentException, ParseException};
pub use inline::{Environment, Inline, References};
pub use tag::TaggedValue;

/// The `Yaml::PARSE_*` and `Yaml::DUMP_*` bit flags, with PHP's values.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash)]
pub struct Flags(pub u32);

impl Flags {
    pub const NONE: Self = Self(0);
    pub const DUMP_OBJECT: Self = Self(1);
    pub const PARSE_EXCEPTION_ON_INVALID_TYPE: Self = Self(2);
    pub const PARSE_OBJECT: Self = Self(4);
    pub const PARSE_OBJECT_FOR_MAP: Self = Self(8);
    pub const DUMP_EXCEPTION_ON_INVALID_TYPE: Self = Self(16);
    pub const PARSE_DATETIME: Self = Self(32);
    pub const DUMP_OBJECT_AS_MAP: Self = Self(64);
    pub const DUMP_MULTI_LINE_LITERAL_BLOCK: Self = Self(128);
    pub const PARSE_CONSTANT: Self = Self(256);
    pub const PARSE_CUSTOM_TAGS: Self = Self(512);
    pub const DUMP_EMPTY_ARRAY_AS_SEQUENCE: Self = Self(1024);
    pub const DUMP_NULL_AS_TILDE: Self = Self(2048);
    pub const DUMP_NUMERIC_KEY_AS_STRING: Self = Self(4096);
    pub const DUMP_NULL_AS_EMPTY: Self = Self(8192);
    pub const DUMP_COMPACT_NESTED_MAPPING: Self = Self(16384);
    pub const DUMP_FORCE_DOUBLE_QUOTES_ON_VALUES: Self = Self(32768);
    pub const PARSE_EXCEPTION_ON_ALIAS: Self = Self(65536);

    /// Every flag with its constant name.
    pub const NAMED: [(&'static str, Self); 17] = [
        ("DUMP_OBJECT", Self::DUMP_OBJECT),
        (
            "PARSE_EXCEPTION_ON_INVALID_TYPE",
            Self::PARSE_EXCEPTION_ON_INVALID_TYPE,
        ),
        ("PARSE_OBJECT", Self::PARSE_OBJECT),
        ("PARSE_OBJECT_FOR_MAP", Self::PARSE_OBJECT_FOR_MAP),
        (
            "DUMP_EXCEPTION_ON_INVALID_TYPE",
            Self::DUMP_EXCEPTION_ON_INVALID_TYPE,
        ),
        ("PARSE_DATETIME", Self::PARSE_DATETIME),
        ("DUMP_OBJECT_AS_MAP", Self::DUMP_OBJECT_AS_MAP),
        (
            "DUMP_MULTI_LINE_LITERAL_BLOCK",
            Self::DUMP_MULTI_LINE_LITERAL_BLOCK,
        ),
        ("PARSE_CONSTANT", Self::PARSE_CONSTANT),
        ("PARSE_CUSTOM_TAGS", Self::PARSE_CUSTOM_TAGS),
        (
            "DUMP_EMPTY_ARRAY_AS_SEQUENCE",
            Self::DUMP_EMPTY_ARRAY_AS_SEQUENCE,
        ),
        ("DUMP_NULL_AS_TILDE", Self::DUMP_NULL_AS_TILDE),
        (
            "DUMP_NUMERIC_KEY_AS_STRING",
            Self::DUMP_NUMERIC_KEY_AS_STRING,
        ),
        ("DUMP_NULL_AS_EMPTY", Self::DUMP_NULL_AS_EMPTY),
        (
            "DUMP_COMPACT_NESTED_MAPPING",
            Self::DUMP_COMPACT_NESTED_MAPPING,
        ),
        (
            "DUMP_FORCE_DOUBLE_QUOTES_ON_VALUES",
            Self::DUMP_FORCE_DOUBLE_QUOTES_ON_VALUES,
        ),
        ("PARSE_EXCEPTION_ON_ALIAS", Self::PARSE_EXCEPTION_ON_ALIAS),
    ];

    /// `$flags & FLAG`.
    pub fn contains(self, flag: Self) -> bool {
        self.0 & flag.0 != 0
    }
}

impl BitOr for Flags {
    type Output = Self;

    fn bitor(self, other: Self) -> Self {
        Self(self.0 | other.0)
    }
}

impl BitOrAssign for Flags {
    fn bitor_assign(&mut self, other: Self) {
        self.0 |= other.0;
    }
}
