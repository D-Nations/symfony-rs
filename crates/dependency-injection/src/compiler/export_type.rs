//! symfony/var-exporter's `ProxyHelper::exportType()` for a parameter,
//! as `ResolveNamedArgumentsPass` and `ResolveBindingsPass` call it.
//!
//! PHP's reflection gives a type object; the class metadata gives the
//! type as PHP prints it (`?Foo`, `int|string`, `(A&B)|null`), which is
//! parsed here. `Foo|null` is a nullable named type, as PHP reflects it.
//!
//! Upstream: none (symfony/var-exporter ProxyHelper.php is not a DI file)

use php_value::Value;

use crate::class_metadata::ParameterMetadata;

/// The types `ReflectionNamedType::isBuiltin()` is true for.
const BUILTIN: [&str; 15] = [
    "int", "float", "string", "bool", "false", "true", "null", "array", "object", "callable",
    "iterable", "mixed", "void", "never", "resource",
];

fn is_builtin(name: &str) -> bool {
    BUILTIN.iter().any(|b| b.eq_ignore_ascii_case(name))
}

/// The class that declares the parameter's function: its name and its
/// parent's, for `self` and `parent`.
#[derive(Debug, Clone, Copy, Default)]
pub(crate) struct DeclaringClass<'a> {
    pub name: Option<&'a str>,
    pub parent: Option<&'a str>,
}

/// Splits `text` on `glue` outside parentheses.
fn split_top(text: &str, glue: char) -> Vec<&str> {
    let mut parts = Vec::new();
    let mut depth = 0usize;
    let mut start = 0;
    for (i, c) in text.char_indices() {
        match c {
            '(' => depth += 1,
            ')' => depth = depth.saturating_sub(1),
            c if c == glue && depth == 0 => {
                parts.push(&text[start..i]);
                start = i + c.len_utf8();
            }
            _ => {}
        }
    }
    parts.push(&text[start..]);
    parts
}

/// One named type of the list: `null` if dropped.
fn export_named(name: &str, no_builtin: bool, class: DeclaringClass<'_>) -> Option<String> {
    let name = name.trim().trim_start_matches('\\');
    if no_builtin && is_builtin(name) {
        return None;
    }
    let mut name = name.to_string();
    if (name == "parent" || name == "self") && class.name.is_some() {
        name = if name == "parent" {
            class.parent.unwrap_or("parent").to_string()
        } else {
            class.name.unwrap_or_default().to_string()
        };
    }
    let prefix = if no_builtin || is_builtin(&name) || name == "static" {
        ""
    } else {
        "\\"
    };
    Some(format!("{prefix}{name}"))
}

/// `ProxyHelper::exportType($parameter, $noBuiltin)`.
pub(crate) fn export_type(
    parameter: &ParameterMetadata,
    no_builtin: bool,
    class: DeclaringClass<'_>,
) -> Option<String> {
    let declared = parameter.type_.as_deref()?.trim();
    if declared.is_empty() {
        return None;
    }
    let default_null = matches!(parameter.default, Some(Value::Null));

    // A nullable named type: `?Foo`, `Foo|null` or `null|Foo`.
    let (nullable, single) = if let Some(rest) = declared.strip_prefix('?') {
        (true, Some(rest))
    } else {
        let parts = split_top(declared, '|');
        let non_null: Vec<&str> = parts
            .iter()
            .copied()
            .filter(|p| !p.trim().eq_ignore_ascii_case("null"))
            .collect();
        if parts.len() == 2 && non_null.len() == 1 && !non_null[0].contains('(') {
            (true, Some(non_null[0]))
        } else if parts.len() == 1 && !declared.contains('&') {
            (false, Some(declared))
        } else {
            (false, None)
        }
    };

    if let Some(name) = single {
        let Some(exported) = export_named(name, no_builtin, class) else {
            return Some(String::new());
        };
        let bare = name.trim().trim_start_matches('\\');
        let allows_null =
            nullable || bare.eq_ignore_ascii_case("mixed") || bare.eq_ignore_ascii_case("null");
        let question = !no_builtin
            && (allows_null || default_null)
            && !bare.eq_ignore_ascii_case("mixed")
            && !bare.eq_ignore_ascii_case("null");
        return Some(format!("{}{exported}", if question { "?" } else { "" }));
    }

    let (parts, glue) = if split_top(declared, '|').len() > 1 {
        (split_top(declared, '|'), "|")
    } else {
        (split_top(declared, '&'), "&")
    };
    let mut types: Vec<String> = Vec::new();
    for part in parts {
        let part = part.trim();
        if let Some(inner) = part.strip_prefix('(').and_then(|p| p.strip_suffix(')')) {
            let mut inner_types: Vec<String> = split_top(inner, '&')
                .into_iter()
                .filter_map(|n| export_named(n, no_builtin, class))
                .collect();
            inner_types.sort();
            types.push(format!("({})", inner_types.join("&")));
            continue;
        }
        if let Some(name) = export_named(part, no_builtin, class) {
            types.push(name);
        }
    }
    if types.is_empty() {
        return Some(String::new());
    }
    types.sort();
    Some(types.join(glue))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn export(declared: &str, no_builtin: bool) -> Option<String> {
        export_type(
            &ParameterMetadata::new("p", Some(declared)),
            no_builtin,
            DeclaringClass {
                name: Some("App\\Foo"),
                parent: Some("App\\Base"),
            },
        )
    }

    #[test]
    fn named_types() {
        assert_eq!(Some("\\App\\Bar".into()), export("App\\Bar", false));
        assert_eq!(Some("App\\Bar".into()), export("App\\Bar", true));
        assert_eq!(Some("?\\App\\Bar".into()), export("?App\\Bar", false));
        assert_eq!(Some("?\\App\\Bar".into()), export("App\\Bar|null", false));
        assert_eq!(Some("App\\Bar".into()), export("?App\\Bar", true));
        assert_eq!(Some("int".into()), export("int", false));
        assert_eq!(Some(String::new()), export("int", true));
        assert_eq!(Some("mixed".into()), export("mixed", false));
        assert_eq!(Some("\\App\\Foo".into()), export("self", false));
        assert_eq!(Some("\\App\\Base".into()), export("parent", false));
        assert_eq!(Some("static".into()), export("static", false));
    }

    #[test]
    fn composite_types() {
        assert_eq!(Some("int|string".into()), export("string|int", false));
        assert_eq!(Some("\\A|\\B|null".into()), export("B|A|null", false));
        assert_eq!(Some("A|B".into()), export("B|A|null", true));
        assert_eq!(Some("\\A&\\B".into()), export("B&A", false));
        assert_eq!(Some("(\\A&\\B)|null".into()), export("(B&A)|null", false));
    }

    #[test]
    fn default_null_is_nullable() {
        let p = ParameterMetadata::new("p", Some("App\\Bar")).with_default(Value::Null);
        assert_eq!(
            Some("?\\App\\Bar".into()),
            export_type(&p, false, DeclaringClass::default())
        );
    }
}
