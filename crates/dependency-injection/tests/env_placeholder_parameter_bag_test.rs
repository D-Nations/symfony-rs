//! Tests/ParameterBag/EnvPlaceholderParameterBagTest.php at
//! symfony/dependency-injection 9b34cad8. Names and assertions are
//! PHPUnit's. `testEnumEnvVarProcessorPassesRegex` writes the name that
//! `EnvConfigurator('FOO')->enum(StringBackedEnum::class)` gives
//! (`EnvConfigurator` is a loader class, not ported).
//!
//! Upstream: symfony/dependency-injection Tests/ParameterBag/EnvPlaceholderParameterBagTest.php
#![allow(non_snake_case, clippy::unwrap_used)]

use php_value::{PhpArray, Value, array::Key, object::EnumCase};
use symfony_dependency_injection::{ErrorKind, ParameterBag};

const STRING_BACKED_ENUM: &str =
    "Symfony\\Component\\DependencyInjection\\Tests\\Fixtures\\StringBackedEnum";

fn bag() -> ParameterBag {
    ParameterBag::env_placeholder(&PhpArray::new()).unwrap()
}

fn string(value: &Value) -> String {
    match value {
        Value::String(s) => s.to_string_lossy().into_owned(),
        other => panic!("not a string: {}", other.debug_type()),
    }
}

/// `assertStringMatchesFormat()` for formats whose only directive is
/// `%s` (one or more characters other than a newline).
fn matches_format(format: &str, text: &str) -> bool {
    fn go(parts: &[&str], text: &str, first: bool) -> bool {
        let Some((head, rest)) = parts.split_first() else {
            return text.is_empty();
        };
        if first {
            return text.starts_with(head) && go(rest, &text[head.len()..], false);
        }
        // `%s` then `head`.
        let mut i = 1;
        while i <= text.len() {
            if !text.is_char_boundary(i) {
                i += 1;
                continue;
            }
            if text[..i].contains('\n') {
                return false;
            }
            let after = &text[i..];
            if after.starts_with(head) && go(rest, &after[head.len()..], false) {
                return true;
            }
            i += 1;
        }
        false
    }
    let parts: Vec<&str> = format.split("%s").collect();
    go(&parts, text, true)
}

fn all(bag: &ParameterBag, name: &str) -> Value {
    bag.all().get(&Key::from(name)).unwrap().clone()
}

#[test]
fn testEnumEnvVarProcessorPassesRegex() {
    let bag = bag();
    let name = format!("env(enum:{STRING_BACKED_ENUM}:FOO)");
    assert!(matches!(bag.get(&name).unwrap(), Value::String(_)));
}

#[test]
fn testGetThrowsInvalidArgumentExceptionIfEnvNameContainsNonWordCharacters() {
    let bag = bag();
    let e = bag.get("env(%foo%)").unwrap_err();
    assert!(e.is(&ErrorKind::InvalidArgument));
    assert_eq!(
        "The given env var name \"env(%foo%)\" contains invalid characters (allowed characters: letters, digits, hyphens, backslashes, dots and colons).",
        e.message()
    );
}

#[test]
fn testMergeWillNotDuplicateIdenticalParameters() {
    let env_variable_name = "DB_HOST";
    let parameter = format!("env({env_variable_name})");
    let first_bag = bag();

    // initialize placeholders
    first_bag.get(&parameter).unwrap();
    let second_bag = first_bag.clone();

    first_bag.merge_env_placeholders(&second_bag);
    let merged_placeholders = first_bag.get_env_placeholders();

    let placeholder_for_variable = merged_placeholders.get(env_variable_name).unwrap();
    let placeholder = &placeholder_for_variable[0];

    assert_eq!(1, placeholder_for_variable.len());
    assert!(placeholder.contains(env_variable_name));
}

#[test]
fn testMergeWhereFirstBagIsEmptyWillWork() {
    let env_variable_name = "DB_HOST";
    let parameter = format!("env({env_variable_name})");
    let first_bag = bag();
    let second_bag = bag();

    // initialize placeholder only in second bag
    second_bag.get(&parameter).unwrap();

    assert!(first_bag.get_env_placeholders().is_empty());

    first_bag.merge_env_placeholders(&second_bag);
    let merged_placeholders = first_bag.get_env_placeholders();

    let placeholder_for_variable = merged_placeholders.get(env_variable_name).unwrap();
    let placeholder = &placeholder_for_variable[0];

    assert_eq!(1, placeholder_for_variable.len());
    assert!(placeholder.contains(env_variable_name));
}

#[test]
fn testMergeWherePlaceholderOnlyExistsInSecond() {
    let unique_env_name = "DB_HOST";
    let common_env_name = "DB_USER";

    let unique_param_name = format!("env({unique_env_name})");
    let common_param_name = format!("env({common_env_name})");

    let first_bag = bag();
    // initialize common placeholder
    first_bag.get(&common_param_name).unwrap();
    let second_bag = first_bag.clone();

    // initialize unique placeholder
    second_bag.get(&unique_param_name).unwrap();

    first_bag.merge_env_placeholders(&second_bag);
    let merged = first_bag.get_env_placeholders();

    assert_eq!(1, merged.get(unique_env_name).unwrap().len());
    // second bag has same placeholder for commonEnvName
    assert_eq!(1, merged.get(common_env_name).unwrap().len());
}

#[test]
fn testMergeWithDifferentIdentifiersForPlaceholders() {
    let env_name = "DB_USER";
    let param_name = format!("env({env_name})");

    let first_bag = bag();
    let second_bag = bag();
    // initialize placeholders
    let first_placeholder = first_bag.get(&param_name).unwrap();
    let second_placeholder = second_bag.get(&param_name).unwrap();

    first_bag.merge_env_placeholders(&second_bag);
    let merged = first_bag.get_env_placeholders();

    assert!(!first_placeholder.identical(&second_placeholder));
    assert_eq!(2, merged.get(env_name).unwrap().len());
}

#[test]
fn testResolveEnvRequiresStrings() {
    let mut bag = bag();
    bag.get("env(INT_VAR)").unwrap();
    bag.set("env(INT_VAR)", Value::Int(2)).unwrap();
    let e = bag.resolve().unwrap_err();
    assert!(e.is(&ErrorKind::Runtime));
    assert_eq!(
        "The default value of env parameter \"INT_VAR\" must be a string or null, \"int\" given.",
        e.message()
    );
}

#[test]
fn testGetDefaultScalarEnv() {
    let mut bag = bag();
    bag.set("env(INT_VAR)", Value::Int(2)).unwrap();
    let e = bag.get("env(INT_VAR)").unwrap_err();
    assert!(e.is(&ErrorKind::Runtime));
    assert_eq!(
        "The default value of an env() parameter must be a string or null, but \"int\" given to \"env(INT_VAR)\".",
        e.message()
    );
}

#[test]
fn testGetDefaultEnv() {
    let mut bag = bag();
    assert!(matches_format(
        "env_%s_INT_VAR_%s",
        &string(&bag.get("env(INT_VAR)").unwrap())
    ));
    bag.set("env(INT_VAR)", Value::from("2")).unwrap();
    assert!(matches_format(
        "env_%s_INT_VAR_%s",
        &string(&bag.get("env(INT_VAR)").unwrap())
    ));
    assert!(Value::from("2").identical(&all(&bag, "env(INT_VAR)")));
    bag.resolve().unwrap();
    assert!(matches_format(
        "env_%s_INT_VAR_%s",
        &string(&bag.get("env(INT_VAR)").unwrap())
    ));
    assert!(Value::from("2").identical(&all(&bag, "env(INT_VAR)")));
}

#[test]
fn testResolveEnvAllowsNull() {
    let mut bag = bag();
    bag.get("env(NULL_VAR)").unwrap();
    bag.set("env(NULL_VAR)", Value::Null).unwrap();
    bag.resolve().unwrap();
    assert!(all(&bag, "env(NULL_VAR)").is_null());
}

#[test]
fn testResolveThrowsOnBadDefaultValue() {
    let mut bag = bag();
    bag.get("env(ARRAY_VAR)").unwrap();
    bag.set("env(ARRAY_VAR)", Value::Array(PhpArray::new()))
        .unwrap();
    let e = bag.resolve().unwrap_err();
    assert!(e.is(&ErrorKind::Runtime));
    assert_eq!(
        "The default value of env parameter \"ARRAY_VAR\" must be a string or null, \"array\" given.",
        e.message()
    );
}

#[test]
fn testGetEnvAllowsNull() {
    let mut bag = bag();
    bag.set("env(NULL_VAR)", Value::Null).unwrap();
    bag.get("env(NULL_VAR)").unwrap();
    bag.resolve().unwrap();

    assert!(all(&bag, "env(NULL_VAR)").is_null());
}

#[test]
fn testGetEnvAllowsDot() {
    let mut bag = bag();
    bag.set("env(dynamic.var)", Value::from("foo")).unwrap();
    bag.get("env(dynamic.var)").unwrap();
    bag.resolve().unwrap();

    assert!(Value::from("foo").identical(&all(&bag, "env(dynamic.var)")));
}

#[test]
fn testGetThrowsOnBadDefaultValue() {
    let mut bag = bag();
    bag.set("env(ARRAY_VAR)", Value::Array(PhpArray::new()))
        .unwrap();
    let e = bag.get("env(ARRAY_VAR)").unwrap_err();
    assert!(e.is(&ErrorKind::Runtime));
    assert_eq!(
        "The default value of an env() parameter must be a string or null, but \"array\" given to \"env(ARRAY_VAR)\".",
        e.message()
    );
}

#[test]
fn testDefaultToNullAllowed() {
    let mut bag = bag();
    bag.resolve().unwrap();
    assert!(!bag.get("env(default::BAR)").unwrap().is_null());
}

#[test]
fn testExtraCharsInProcessor() {
    let mut bag = bag();
    bag.resolve().unwrap();
    assert!(matches_format(
        "env_%s_key_a_b_c_FOO_%s",
        &string(&bag.get("env(key:a.b-c:FOO)").unwrap())
    ));
}

#[test]
fn testGetEnum() {
    let mut bag = bag();
    let bar =
        Value::Object(EnumCase::new(STRING_BACKED_ENUM, "Bar", Some(Value::from("bar"))).object());
    bag.set("ENUM_VAR", bar.clone()).unwrap();
    let value = bag.get("ENUM_VAR").unwrap();
    assert_eq!(
        STRING_BACKED_ENUM,
        value.as_object().unwrap().class(),
        "assertInstanceOf(StringBackedEnum::class)"
    );
    assert!(bar.identical(&value));
}

/// The placeholder's shape: the bag's prefix, the env name with `:-.\`
/// as `_`, and an xxh128 hash.
#[test]
fn placeholder_shape() {
    let bag = bag();
    let placeholder = string(&bag.get("env(json:FOO)").unwrap());
    let prefix = bag.get_env_placeholder_unique_prefix();
    assert_eq!(20, prefix.len());
    assert!(placeholder.starts_with(&format!("{prefix}_json_FOO_")));
    assert_eq!(prefix.len() + "_json_FOO_".len() + 32, placeholder.len());
    // A second read gives the first placeholder.
    assert_eq!(placeholder, string(&bag.get("env(json:FOO)").unwrap()));
}
