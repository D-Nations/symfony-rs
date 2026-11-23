//! How PHP stores a named input variable into an array (`$_GET`, `$_POST`,
//! `$_COOKIE`, `parse_str()`), and `parse_str()` itself, on bytes.
//!
//! Written from the PHP manual ("Variables From External Sources",
//! "Arrays", `parse_str()`) and PHP 8.4.27's recorded answers:
//! crates/http-foundation's request vectors (`scripts/record.sh
//! request-vars`) for the name rules, and `parse_str()`'s own vectors
//! (`tests/data/parse_str.json`, `scripts/record.sh parse-str`). No PHP
//! source code is used (PLAN.md decision 8). The rules were first written
//! for crates/http-foundation's `query.rs` (M1-D request rewrite) and moved
//! here, unchanged, so `parse_str()` shares them.
//!
//! Upstream: none

use crate::array::{Key, PhpArray};
use crate::string::PhpString;
use crate::url::urldecode;
use crate::value::Value;

/// PHP's `max_input_vars` default.
pub const MAX_INPUT_VARS: usize = 1000;
/// PHP's `max_input_nesting_level` default: a name with more `[...]`
/// groups is dropped.
pub const MAX_INPUT_NESTING_LEVEL: usize = 64;

/// What happens when a name without brackets is already present.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Duplicate {
    /// The later value replaces the earlier one in place.
    Replace,
    /// The first value stays (`$_COOKIE`).
    KeepFirst,
}

/// What [`register`] did.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Registered {
    /// The value was stored.
    pub stored: bool,
    /// The top-level entry was created or replaced (a string, or an array
    /// that replaced a string), rather than an existing array added to.
    pub replaced: bool,
}

/// One `[...]` group of a name.
#[derive(Debug, PartialEq, Eq)]
enum Group {
    /// `[]`: the next integer index.
    Append,
    Key(Vec<u8>),
}

/// A registered name: the top-level key and its groups.
#[derive(Debug, PartialEq, Eq)]
struct Name {
    base: Vec<u8>,
    groups: Vec<Group>,
}

impl Name {
    /// `None` when PHP stores nothing for this name.
    ///
    /// Recorded rules: the name ends at a NUL byte; leading spaces are
    /// dropped; an empty name, or one starting with `[`, stores nothing.
    /// Before the first `[`, spaces and dots become `_`. When that `[` has
    /// no `]` after it, it is not an array: every space, dot and `[` of the
    /// name becomes `_`. Otherwise each `[...]` group ends at the first `]`;
    /// anything after a group other than `[` is ignored, as is an unclosed
    /// later group. More than `max_input_nesting_level` groups store
    /// nothing.
    fn parse(name: &[u8]) -> Option<Self> {
        let name = match name.iter().position(|&b| b == 0) {
            Some(nul) => &name[..nul],
            None => name,
        };
        let start = name.iter().position(|&b| b != b' ')?;
        let name = &name[start..];
        let Some(open) = name.iter().position(|&b| b == b'[') else {
            return Some(Self::plain(name, b" ."));
        };
        if open == 0 {
            return None;
        }
        if !name[open + 1..].contains(&b']') {
            return Some(Self::plain(name, b" .["));
        }
        let mut groups = Vec::new();
        let mut rest = &name[open..];
        while let Some(inside) = rest.strip_prefix(b"[") {
            let Some(close) = inside.iter().position(|&b| b == b']') else {
                break;
            };
            groups.push(Group::of(&inside[..close]));
            rest = &inside[close + 1..];
        }
        if groups.len() > MAX_INPUT_NESTING_LEVEL {
            return None;
        }
        Some(Self {
            base: underscored(&name[..open], b" ."),
            groups,
        })
    }

    fn plain(name: &[u8], converted: &[u8]) -> Self {
        Self {
            base: underscored(name, converted),
            groups: Vec::new(),
        }
    }
}

impl Group {
    /// `[]`, and a group holding exactly one whitespace byte (`[ ]`,
    /// `[\t]`, ...), append; anything else is the key as written, spaces
    /// included (`[ b]`, `[  ]`).
    fn of(inside: &[u8]) -> Self {
        match inside {
            [] => Self::Append,
            [b' ' | b'\t' | b'\n' | b'\r' | 0x0b | 0x0c] => Self::Append,
            key => Self::Key(key.to_vec()),
        }
    }
}

fn underscored(name: &[u8], converted: &[u8]) -> Vec<u8> {
    name.iter()
        .map(|&b| if converted.contains(&b) { b'_' } else { b })
        .collect()
}

/// The top-level key PHP stores `name` under, `None` when it stores
/// nothing for it.
pub fn top_level_key(name: &[u8]) -> Option<Key> {
    Name::parse(name).map(|name| Key::from_bytes(&name.base))
}

/// Stores `value` under `name` (`base[a][b]...`) in `vars`, as PHP stores
/// an input variable.
pub fn register(
    vars: &mut PhpArray,
    name: &[u8],
    value: PhpString,
    duplicate: Duplicate,
) -> Registered {
    let Some(name) = Name::parse(name) else {
        return Registered::default();
    };
    let base = Key::from_bytes(&name.base);
    if name.groups.is_empty() {
        if duplicate == Duplicate::KeepFirst && vars.contains_key(&base) {
            return Registered::default();
        }
        vars.insert(base, Value::String(value));
        return Registered {
            stored: true,
            replaced: true,
        };
    }
    // A new or replaced array stays even when the value itself cannot be
    // appended.
    let replaced = !matches!(vars.get(&base), Some(Value::Array(_)));
    let Some(map) = array_at(vars, base) else {
        return Registered::default();
    };
    Registered {
        stored: store(map, &name.groups, value),
        replaced,
    }
}

/// The array stored under `key`, created at the end if absent and
/// replacing any other value in place.
fn array_at(vars: &mut PhpArray, key: Key) -> Option<&mut PhpArray> {
    if !matches!(vars.get(&key), Some(Value::Array(_))) {
        vars.insert(key.clone(), Value::Array(PhpArray::new()));
    }
    match vars.get_mut(&key)? {
        Value::Array(array) => Some(array),
        _ => None,
    }
}

/// Stores `value` at `groups` below `map`. False when an append found the
/// next index taken (after `PHP_INT_MAX`), in which case nothing more is
/// stored.
fn store(mut map: &mut PhpArray, groups: &[Group], value: PhpString) -> bool {
    let Some((last, parents)) = groups.split_last() else {
        return false;
    };
    for group in parents {
        let Some(key) = key_for(map, group) else {
            return false;
        };
        let Some(child) = array_at(map, key) else {
            return false;
        };
        map = child;
    }
    let Some(key) = key_for(map, last) else {
        return false;
    };
    map.insert(key, Value::String(value));
    true
}

/// The array key a group stands for: the key as written (an int when it is
/// one in PHP's sense), or for `[]` the array's next index (`None` past
/// `PHP_INT_MAX`).
fn key_for(map: &PhpArray, group: &Group) -> Option<Key> {
    match group {
        Group::Key(key) => Some(Key::from_bytes(key)),
        Group::Append => map.next_index().map(Key::Int),
    }
}

/// `parse_str($string, $result)`: `&`-separated `name=value` pairs, each
/// `urldecode()`d and stored as [`register`] stores them; empty pairs are
/// skipped, and only the first `max_input_vars` other pairs are read (as
/// recorded; PHP warns about the rest, which is not reproduced). The input
/// ends at its first raw NUL byte (recorded: `x=1&a\0b=1&y=2` gives `x`
/// and `a`, no `y`); an encoded `%00` is kept.
pub fn parse_str(string: &[u8]) -> PhpArray {
    let string = match string.iter().position(|&b| b == 0) {
        Some(nul) => &string[..nul],
        None => string,
    };
    let mut vars = PhpArray::new();
    let pairs = string.split(|&b| b == b'&').filter(|pair| !pair.is_empty());
    for pair in pairs.take(MAX_INPUT_VARS) {
        let (name, value) = match pair.iter().position(|&b| b == b'=') {
            Some(at) => (&pair[..at], &pair[at + 1..]),
            None => (pair, &b""[..]),
        };
        register(
            &mut vars,
            &urldecode(name),
            PhpString::from(urldecode(value)),
            Duplicate::Replace,
        );
    }
    vars
}

#[cfg(test)]
mod tests {
    use super::*;

    fn s(text: &str) -> Value {
        Value::String(PhpString::from(text))
    }

    #[test]
    fn names_follow_phps_rules() {
        let vars = parse_str(b"a.b=1&c[d][]=2&c[d][]=3&[x=4&e[=5&5=6");
        assert_eq!(vars.get(&Key::from_bytes(b"a_b")), Some(&s("1")));
        assert_eq!(vars.get(&Key::from_bytes(b"e_")), Some(&s("5")));
        assert_eq!(vars.get(&Key::Int(5)), Some(&s("6")));
        let Some(Value::Array(c)) = vars.get(&Key::from_bytes(b"c")) else {
            panic!("c is not an array");
        };
        let Some(Value::Array(d)) = c.get(&Key::from_bytes(b"d")) else {
            panic!("c[d] is not an array");
        };
        assert_eq!(d.get(&Key::Int(1)), Some(&s("3")));
        assert_eq!(vars.len(), 4);
    }
}
