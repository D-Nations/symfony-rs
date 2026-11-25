//! `Symfony\Component\Dotenv\Command`, behind the `console` feature, and
//! what both commands share: the application's globals, `getDotenvPath()`
//! (each command has its own copy upstream), and the PHP built-ins they
//! call (`json_decode()` of composer.json, `var_export()`, `basename()`,
//! `dirname()`).
//!
//! PHP's `$_SERVER` and `$_ENV` are the application's `Globals`, shared
//! with the commands (`SharedGlobals`).
//!
//! Upstream: none

use std::fmt::Write as _;
use std::sync::{Arc, Mutex, MutexGuard, PoisonError};

use php_value::{Key, PhpArray, Value, numeric::var_export_float};
use symfony_console::ConsoleError;

use crate::globals::Globals;

pub mod debug_command;
pub mod dotenv_dump_command;

/// The application's `$_SERVER` and `$_ENV`.
pub type SharedGlobals = Arc<Mutex<Globals>>;

pub(crate) fn lock(globals: &SharedGlobals) -> MutexGuard<'_, Globals> {
    globals.lock().unwrap_or_else(PoisonError::into_inner)
}

/// A Dotenv error as the console reports it: its class and message.
pub(crate) fn console_error(error: crate::exception::Error) -> ConsoleError {
    ConsoleError::Other {
        class: error.class().to_owned(),
        message: String::from_utf8_lossy(&error.message()).into_owned(),
    }
}

pub(crate) use php_value::path::{basename_str as basename, dirname_str as dirname};

pub(crate) fn is_file(path: &str) -> bool {
    std::path::Path::new(path).is_file()
}

/// `getDotenvPath()`: the .env path, and the runtime options it read
/// (`$config`).
pub(crate) fn get_dotenv_path(
    project_dir: &str,
    globals: &Globals,
) -> Result<(String, PhpArray), ConsoleError> {
    let mut config = PhpArray::new();
    let mut project_dir = project_dir.to_owned();

    if is_file(&project_dir) {
        config.insert("dotenv_path", Value::string(basename(&project_dir)));
        project_dir = dirname(&project_dir);
    }

    let composer_file = format!("{project_dir}/composer.json");
    // $config += $_SERVER['APP_RUNTIME_OPTIONS'] ?? (...)['extra']['runtime'] ?? []
    let options = match globals.server.get(&Key::from("APP_RUNTIME_OPTIONS")) {
        Some(value) if !value.is_null() => value.clone(),
        _ => {
            let decoded = if is_file(&composer_file) {
                let text = std::fs::read(&composer_file).map_err(|e| ConsoleError::Other {
                    class: "ErrorException".to_owned(),
                    message: format!("file_get_contents({composer_file}): {e}"),
                })?;
                json_decode(&text)
            } else {
                Value::Array(PhpArray::new())
            };
            let runtime = decoded
                .as_array()
                .and_then(|a| a.get(&Key::from("extra")))
                .and_then(Value::as_array)
                .and_then(|a| a.get(&Key::from("runtime")))
                .filter(|v| !v.is_null())
                .cloned();
            runtime.unwrap_or_else(|| Value::Array(PhpArray::new()))
        }
    };
    let Value::Array(options) = options else {
        return Err(ConsoleError::Other {
            class: "TypeError".to_owned(),
            message: "Unsupported operand types: array + string".to_owned(),
        });
    };
    // array union: existing keys are kept
    for (key, value) in options.iter() {
        if !config.contains_key(key) {
            config.insert(key.clone(), value.clone());
        }
    }

    let dotenv_path = match config.get(&Key::from("dotenv_path")) {
        Some(v) if !v.is_null() => value_string(v),
        _ => ".env".to_owned(),
    };
    Ok((format!("{project_dir}/{dotenv_path}"), config))
}

/// `(string) $value` of a scalar.
pub(crate) fn value_string(value: &Value) -> String {
    value
        .to_php_string()
        .map(|s| s.to_string_lossy().into_owned())
        .unwrap_or_default()
}

/// `json_decode($json, true)`: objects as arrays, `null` for invalid JSON.
/// Integers that do not fit become floats, as in PHP.
fn json_decode(text: &[u8]) -> Value {
    fn convert(json: serde_json::Value) -> Value {
        match json {
            serde_json::Value::Null => Value::Null,
            serde_json::Value::Bool(b) => Value::Bool(b),
            serde_json::Value::Number(n) => match n.as_i64() {
                Some(i) => Value::Int(i),
                None => Value::Float(n.as_f64().unwrap_or(0.0)),
            },
            serde_json::Value::String(s) => Value::string(s),
            serde_json::Value::Array(items) => {
                Value::Array(PhpArray::list(items.into_iter().map(convert)))
            }
            serde_json::Value::Object(map) => {
                let mut array = PhpArray::new();
                for (key, value) in map {
                    array.insert(Key::from(key.as_str()), convert(value));
                }
                Value::Array(array)
            }
        }
    }
    serde_json::from_slice::<serde_json::Value>(text).map_or(Value::Null, convert)
}

/// `var_export($value, true)`.
pub(crate) fn var_export(value: &Value) -> String {
    let mut out = String::new();
    export(value, 0, &mut out);
    out
}

fn export(value: &Value, indent: usize, out: &mut String) {
    match value {
        Value::Null => out.push_str("NULL"),
        Value::Bool(true) => out.push_str("true"),
        Value::Bool(false) => out.push_str("false"),
        Value::Int(i) => out.push_str(&i.to_string()),
        Value::Float(f) => out.push_str(&var_export_float(*f)),
        Value::String(s) => export_string(&s.to_string_lossy(), out),
        Value::Array(array) => {
            out.push_str("array (\n");
            let pad = " ".repeat(indent + 2);
            for (key, item) in array.iter() {
                out.push_str(&pad);
                match key {
                    Key::Int(i) => out.push_str(&i.to_string()),
                    Key::Str(s) => export_string(&s.to_string_lossy(), out),
                }
                out.push_str(" => ");
                if matches!(item, Value::Array(_)) {
                    out.push('\n');
                    out.push_str(&pad);
                }
                export(item, indent + 2, out);
                out.push_str(",\n");
            }
            out.push_str(&" ".repeat(indent));
            out.push(')');
        }
        Value::Object(o) => _ = write!(out, "\\{}::__set_state(array(\n))", o.class()),
    }
}

/// A string in single quotes: `\` and `'` escaped, a NUL byte written as
/// `' . "\0" . '`.
fn export_string(s: &str, out: &mut String) {
    out.push('\'');
    for c in s.chars() {
        match c {
            '\\' => out.push_str("\\\\"),
            '\'' => out.push_str("\\'"),
            '\0' => out.push_str("' . \"\\0\" . '"),
            c => out.push(c),
        }
    }
    out.push('\'');
}

#[cfg(test)]
mod tests {
    use php_value::{PhpArray, Value};

    use super::{basename, dirname, var_export};

    #[test]
    fn paths() {
        assert_eq!("b", basename("/a/b"));
        assert_eq!("b", basename("/a/b/"));
        assert_eq!("/a", dirname("/a/b"));
        assert_eq!("/", dirname("/a"));
        assert_eq!(".", dirname("a"));
    }

    #[test]
    fn exports() {
        let mut array = PhpArray::new();
        array.insert("A", Value::string("it's \\ ok"));
        array.insert(3, Value::Null);
        assert_eq!(
            "array (\n  'A' => 'it\\'s \\\\ ok',\n  3 => NULL,\n)",
            var_export(&Value::Array(array))
        );
        assert_eq!("array (\n)", var_export(&Value::Array(PhpArray::new())));
    }
}
