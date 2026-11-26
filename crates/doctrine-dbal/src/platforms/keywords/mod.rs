//! Port of Doctrine\DBAL\Platforms\Keywords\KeywordList.php and
//! SQLiteKeywords.php (both deprecated upstream): the reserved words a
//! platform quotes in names.
//!
//! Doctrine source copyright (c) Doctrine Project; distributed under MIT.
//!
//! Upstream: doctrine/dbal src/Platforms/Keywords/KeywordList.php src/Platforms/Keywords/SQLiteKeywords.php

use std::collections::HashSet;
use std::sync::OnceLock;

/// Abstract interface for a SQL reserved keyword dictionary
/// (`KeywordList`): a class and its `getKeywords()`.
#[derive(Debug)]
pub struct KeywordList {
    class: &'static str,
    get_keywords: fn() -> &'static [&'static str],
    keywords: OnceLock<HashSet<String>>,
}

impl KeywordList {
    /// A keyword list of `class` whose `getKeywords()` is `get_keywords`.
    pub fn new(class: &'static str, get_keywords: fn() -> &'static [&'static str]) -> Self {
        Self {
            class,
            get_keywords,
            keywords: OnceLock::new(),
        }
    }

    /// `static::class`.
    pub fn class(&self) -> &'static str {
        self.class
    }

    /// Checks if the given word is a keyword of this dialect/vendor
    /// platform (`strtoupper()`: ASCII only).
    pub fn is_keyword(&self, word: &str) -> bool {
        self.keywords
            .get_or_init(|| {
                (self.get_keywords)()
                    .iter()
                    .map(|keyword| keyword.to_ascii_uppercase())
                    .collect()
            })
            .contains(&word.to_ascii_uppercase())
    }
}

/// SQLite Keywordlist (`new SQLiteKeywords()`).
pub struct SQLiteKeywords;

impl SQLiteKeywords {
    pub const CLASS: &'static str = "Doctrine\\DBAL\\Platforms\\Keywords\\SQLiteKeywords";

    /// `new SQLiteKeywords()`.
    #[allow(clippy::new_ret_no_self, reason = "the list is a KeywordList")]
    pub fn new() -> KeywordList {
        KeywordList::new(Self::CLASS, Self::get_keywords)
    }

    /// `getKeywords()`.
    pub fn get_keywords() -> &'static [&'static str] {
        &[
            "ABORT", "ACTION", "ADD", "AFTER", "ALL", "ALTER", "ANALYZE", "AND", "AS", "ASC", "ATTACH", "AUTOINCREMENT", "BEFORE", "BEGIN", "BETWEEN", "BY", "CASCADE", "CASE", "CAST", "CHECK", "COLLATE", "COLUMN", "COMMIT", "CONFLICT", "CONSTRAINT", "CREATE", "CROSS", "CURRENT_DATE", "CURRENT_TIME", "CURRENT_TIMESTAMP", "DATABASE", "DEFAULT", "DEFERRABLE", "DEFERRED", "DELETE", "DESC", "DETACH", "DISTINCT", "DROP", "EACH", "ELSE", "END", "ESCAPE", "EXCEPT", "EXCLUSIVE", "EXISTS", "EXPLAIN", "FAIL", "FOR", "FOREIGN", "FROM", "FULL", "GLOB", "GROUP", "HAVING", "IF", "IGNORE", "IMMEDIATE", "IN", "INDEX", "INDEXED", "INITIALLY", "INNER", "INSERT", "INSTEAD", "INTERSECT", "INTO", "IS", "ISNULL", "JOIN", "KEY", "LEFT", "LIKE", "LIMIT", "MATCH", "NATURAL", "NO", "NOT", "NOTNULL", "NULL", "OF", "OFFSET", "ON", "OR", "ORDER", "OUTER", "PLAN", "PRAGMA", "PRIMARY", "QUERY", "RAISE", "REFERENCES", "REGEXP", "REINDEX", "RELEASE", "RENAME", "REPLACE", "RESTRICT", "RIGHT", "ROLLBACK", "ROW", "SAVEPOINT", "SELECT", "SET", "TABLE", "TEMP", "TEMPORARY", "THEN", "TO", "TRANSACTION", "TRIGGER", "UNION", "UNIQUE", "UPDATE", "USING", "VACUUM", "VALUES", "VIEW", "VIRTUAL", "WHEN", "WHERE", 
        ]
    }
}
