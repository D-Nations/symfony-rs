//! Port of Symfony\Component\Dotenv\Dotenv.php: parses .env files and
//! loads them into `$_ENV`, `$_SERVER` and (with `usePutenv`) `getenv()`,
//! all of which live in a [`Globals`] here.
//!
//! The PCRE patterns of upstream are written as matchers, each next to
//! the pattern it implements (possessive quantifiers, a lookbehind, and
//! the recursive pattern for balanced `$(...)`). `$(...)` runs through
//! `Globals::commands` (`/bin/sh -c`, with the environment Symfony
//! Process gives a command on the CLI); on Windows it fails as upstream
//! does. `.env.local.php` is read as the literal array `composer dump-env`
//! writes, not run (see [`crate::local_php`]).
//!
//! Symfony source copyright (c) Fabien Potencier; distributed under MIT.
//!
//! Upstream: symfony/dotenv Dotenv.php

use php_value::{PhpArray, PhpString, Value, numeric::string_to_int};

use crate::{
    exception::{Error, FormatException, FormatExceptionContext, PathException},
    globals::{Globals, get, isset, key, string},
    local_php,
};

/// `Dotenv::VARNAME_REGEX`, as upstream writes it.
pub const VARNAME_REGEX: &str = "(?i:_*[A-Z][A-Z0-9_]*+)";

/// PHP's `\s` without `/u`.
fn is_space(b: u8) -> bool {
    matches!(b, b' ' | b'\t' | b'\n' | 0x0b | 0x0c | b'\r')
}

/// PHP's default `trim()` characters.
fn is_trim(b: u8) -> bool {
    matches!(b, b' ' | b'\t' | b'\n' | b'\r' | 0 | 0x0b)
}

fn rtrim(s: &[u8]) -> &[u8] {
    let end = s.iter().rposition(|&b| !is_trim(b)).map_or(0, |p| p + 1);
    &s[..end]
}

fn rtrim_chars<'a>(s: &'a [u8], chars: &[u8]) -> &'a [u8] {
    let end = s
        .iter()
        .rposition(|b| !chars.contains(b))
        .map_or(0, |p| p + 1);
    &s[..end]
}

/// `str_replace($from, $to, $subject)` for one pair.
fn replace(subject: &[u8], from: &[u8], to: &[u8]) -> Vec<u8> {
    let mut out = Vec::with_capacity(subject.len());
    let mut i = 0;
    while i < subject.len() {
        if subject[i..].starts_with(from) {
            out.extend_from_slice(to);
            i += from.len();
        } else {
            out.push(subject[i]);
            i += 1;
        }
    }
    out
}

/// `_*[A-Z][A-Z0-9_]*+`, case-insensitive, at `i`: its length.
fn varname(s: &[u8], i: usize) -> Option<usize> {
    let underscores = s[i..].iter().take_while(|&&b| b == b'_').count();
    let first = i + underscores;
    if !s.get(first).is_some_and(u8::is_ascii_alphabetic) {
        return None;
    }
    let rest = s[first + 1..]
        .iter()
        .take_while(|b| b.is_ascii_alphanumeric() || **b == b'_')
        .count();
    Some(underscores + 1 + rest)
}

/// `$name` as an env var name (keys are names in `values`).
fn str_of(name: &[u8]) -> String {
    String::from_utf8_lossy(name).into_owned()
}

fn value(s: &[u8]) -> Value {
    Value::String(PhpString::from(s))
}

/// An ordered set of names, as PHP's `array_flip(explode(',', ...))`.
#[derive(Debug, Default, Clone)]
struct Names(Vec<Vec<u8>>);

impl Names {
    fn parse(list: &[u8]) -> Self {
        let mut names = Self::default();
        for name in list.split(|&b| b == b',') {
            names.insert(name);
        }
        names
    }

    fn has(&self, name: &[u8]) -> bool {
        self.0.iter().any(|n| n == name)
    }

    fn insert(&mut self, name: &[u8]) {
        if !self.has(name) {
            self.0.push(name.to_vec());
        }
    }

    fn remove(&mut self, name: &[u8]) {
        self.0.retain(|n| n != name);
    }

    fn join(&self) -> Vec<u8> {
        self.0.join(&b',')
    }
}

/// `$_SERVER[$k] ?? $_ENV[$k] ?? null`.
fn server_or_env<'a>(g: &'a Globals, name: &[u8]) -> Option<&'a Value> {
    get(&g.server, name).or_else(|| get(&g.env, name))
}

/// `array_flip(explode(',', $_SERVER['SYMFONY_DOTENV_VARS'] ?? $_ENV['SYMFONY_DOTENV_VARS'] ?? ''))`
fn dotenv_vars(g: &Globals) -> Names {
    let list = server_or_env(g, b"SYMFONY_DOTENV_VARS")
        .map(string)
        .unwrap_or_default();
    Names::parse(&list)
}

/// The same, without `''`.
fn loaded_vars(g: &Globals) -> Names {
    let mut names = dotenv_vars(g);
    names.remove(b"");
    names
}

/// `isset($_ENV[$name]) || isset($_SERVER[$name]) && !str_starts_with($name, 'HTTP_')`
fn is_external(g: &Globals, name: &[u8]) -> bool {
    isset(&g.env, name) || (isset(&g.server, name) && !name.starts_with(b"HTTP_"))
}

/// `'' !== $s && '$' / "\0" / '\\'`: whether the env key needs resolving.
fn needs_resolving(s: &[u8]) -> bool {
    s.contains(&b'$') || s.contains(&0) || s.contains(&b'\\')
}

/// `preg_match('/\$\{?'.preg_quote($name, '/').'(?![A-Za-z0-9_])/', $value)`
fn is_self_referencing(name: &[u8], value: &[u8]) -> bool {
    value.iter().enumerate().any(|(i, &b)| {
        if b != b'$' {
            return false;
        }
        let at = |k: usize| {
            value[k..].starts_with(name)
                && !value
                    .get(k + name.len())
                    .is_some_and(|c| c.is_ascii_alphanumeric() || *c == b'_')
        };
        (value.get(i + 1) == Some(&b'{') && at(i + 2)) || at(i + 1)
    })
}

/// `filter_var($value, FILTER_VALIDATE_BOOL)`.
fn filter_bool(value: &Value) -> bool {
    let s = match value {
        Value::Array(_) | Value::Object(_) => return false,
        other => string(other),
    };
    let trimmed = String::from_utf8_lossy(s.trim_ascii()).to_ascii_lowercase();
    matches!(trimmed.as_str(), "1" | "true" | "on" | "yes")
}

/// `(int) $value`.
fn to_int(value: &Value) -> i64 {
    match value {
        Value::Null => 0,
        Value::Bool(b) => i64::from(*b),
        Value::Int(i) => *i,
        Value::Float(f) if f.is_finite() => *f as i64,
        Value::Float(_) => 0,
        Value::String(s) => string_to_int(s),
        Value::Array(a) => i64::from(!a.is_empty()),
        Value::Object(_) => 1,
    }
}

/// Manages .env files.
#[derive(Debug, Clone)]
pub struct Dotenv {
    env_key: String,
    debug_key: String,
    path: String,
    cursor: usize,
    lineno: i64,
    data: Vec<u8>,
    end: usize,
    values: PhpArray,
    overridden_values: PhpArray,
    loaded_raw_vars: Names,
    prod_envs: Vec<String>,
    use_putenv: bool,
    defer_putenv: bool,
    pending_putenv: Names,
}

impl Default for Dotenv {
    fn default() -> Self {
        Self::new()
    }
}

impl Dotenv {
    /// `new Dotenv()`: `APP_ENV` and `APP_DEBUG`.
    pub fn new() -> Self {
        Self::with_keys("APP_ENV", "APP_DEBUG")
    }

    /// `new Dotenv($envKey, $debugKey)`.
    pub fn with_keys(env_key: &str, debug_key: &str) -> Self {
        Self {
            env_key: env_key.to_owned(),
            debug_key: debug_key.to_owned(),
            path: String::new(),
            cursor: 0,
            lineno: 0,
            data: Vec::new(),
            end: 0,
            values: PhpArray::new(),
            overridden_values: PhpArray::new(),
            loaded_raw_vars: Names::default(),
            prod_envs: vec!["prod".to_owned()],
            use_putenv: false,
            defer_putenv: false,
            pending_putenv: Names::default(),
        }
    }

    /// `setProdEnvs()`.
    pub fn set_prod_envs(mut self, prod_envs: &[&str]) -> Self {
        self.prod_envs = prod_envs.iter().map(|&e| e.to_owned()).collect();
        self
    }

    /// `usePutenv()`: whether `putenv()` defines the variables too.
    pub fn use_putenv(mut self, use_putenv: bool) -> Self {
        self.use_putenv = use_putenv;
        self
    }

    /// `load($path, ...$extraPaths)`: loads one or several .env files.
    pub fn load(&mut self, g: &mut Globals, paths: &[&str]) -> Result<(), Error> {
        self.defer_putenv = true;
        let result = self
            .do_load(g, false, paths)
            .and_then(|()| self.resolve_loaded_vars(g));
        self.defer_putenv = false;
        result
    }

    /// `loadEnv()`: loads a .env file and the corresponding .env.local,
    /// .env.$env and .env.$env.local files if they exist.
    ///
    /// .env.local is always ignored in test env because tests should
    /// produce the same results for everyone. .env.dist is loaded when it
    /// exists and .env is not found.
    pub fn load_env(
        &mut self,
        g: &mut Globals,
        path: &str,
        env_key: Option<&str>,
        default_env: &str,
        test_envs: &[&str],
        override_existing_vars: bool,
    ) -> Result<(), Error> {
        self.populate_path(g, path);

        self.defer_putenv = true;
        let result = self.do_load_env(
            g,
            path,
            env_key,
            default_env,
            test_envs,
            override_existing_vars,
        );
        let resolved = self.resolve_loaded_vars(g);
        self.defer_putenv = false;
        resolved.and(result)
    }

    fn do_load_env(
        &mut self,
        g: &mut Globals,
        path: &str,
        env_key: Option<&str>,
        default_env: &str,
        test_envs: &[&str],
        override_existing_vars: bool,
    ) -> Result<(), Error> {
        let k = env_key.unwrap_or(&self.env_key).to_owned();
        let k = k.as_bytes();
        let is_file = |p: &str| std::path::Path::new(p).is_file();

        let dist = format!("{path}.dist");
        if is_file(path) || !is_file(&dist) {
            self.do_load(g, override_existing_vars, &[path])?;
        } else {
            self.do_load(g, override_existing_vars, &[&dist])?;
        }

        let mut env = match server_or_env(g, k) {
            None => {
                let mut values = PhpArray::new();
                values.insert(key(k), Value::from(default_env));
                self.populate(g, &values, override_existing_vars);
                Value::from(default_env)
            }
            Some(env) if needs_resolving(&string(env)) => {
                let env = string(env);
                value(&self.resolve_env_key(g, &env, k)?)
            }
            Some(env) => env.clone(),
        };

        let is_test = matches!(&env, Value::String(s) if test_envs.iter().any(|t| t.as_bytes() == s.as_bytes()));
        let local = format!("{path}.local");
        if !is_test && is_file(&local) {
            self.do_load(g, override_existing_vars, &[&local])?;
            env = server_or_env(g, k).cloned().unwrap_or(env);
            let text = string(&env);
            if needs_resolving(&text) {
                env = value(&self.resolve_env_key(g, &text, k)?);
            }
        }

        let env = str_of(&string(&env));
        if env == "local" {
            return Ok(());
        }

        let p = format!("{path}.{env}");
        if is_file(&p) {
            self.do_load(g, override_existing_vars, &[&p])?;
        }

        let p = format!("{path}.{env}.local");
        if is_file(&p) {
            self.do_load(g, override_existing_vars, &[&p])?;
        }
        Ok(())
    }

    /// `bootEnv()`: loads env vars from .env.local.php if the file exists
    /// or from the other .env files otherwise, then sets the debug var
    /// from the env.
    pub fn boot_env(
        &mut self,
        g: &mut Globals,
        path: &str,
        default_env: &str,
        test_envs: &[&str],
        override_existing_vars: bool,
    ) -> Result<(), Error> {
        let p = format!("{path}.local.php");
        let env = if std::path::Path::new(&p).is_file() {
            local_php::read(&p)?
        } else {
            None
        };
        let k = self.env_key.clone();
        let k = k.as_bytes();

        let use_local = env.as_ref().is_some_and(|env| {
            override_existing_vars
                || !isset(env, k)
                || server_or_env(g, k)
                    .or_else(|| get(env, k))
                    .zip(get(env, k))
                    .is_some_and(|(current, own)| current.identical(own))
        });
        match env {
            Some(env) if use_local => {
                self.populate_path(g, path);
                self.populate(g, &env, override_existing_vars);
            }
            _ => {
                let env_key = self.env_key.clone();
                self.load_env(
                    g,
                    path,
                    Some(&env_key),
                    default_env,
                    test_envs,
                    override_existing_vars,
                )?;
            }
        }

        let env = g.env.clone();
        g.server.union(&env);

        let k = self.debug_key.clone();
        let k = k.as_bytes();
        let debug = if let Some(debug) = get(&g.server, k) {
            debug.clone()
        } else {
            let app_env = get(&g.server, self.env_key.as_bytes());
            let prod = matches!(app_env, Some(Value::String(s)) if self.prod_envs.iter().any(|e| e.as_bytes() == s.as_bytes()));
            Value::Bool(!prod)
        };
        let on = to_int(&debug) != 0 || (!matches!(debug, Value::Bool(_)) && filter_bool(&debug));
        let flag = Value::from(if on { "1" } else { "0" });
        g.server.insert(key(k), flag.clone());
        g.env.insert(key(k), flag);
        Ok(())
    }

    /// `overload()`: loads one or several .env files, overriding existing
    /// vars.
    pub fn overload(&mut self, g: &mut Globals, paths: &[&str]) -> Result<(), Error> {
        self.defer_putenv = true;
        let result = self
            .do_load(g, true, paths)
            .and_then(|()| self.resolve_loaded_vars(g));
        self.defer_putenv = false;
        result
    }

    /// `populate()`: sets values as environment variables (via putenv,
    /// `$_ENV`, and `$_SERVER`).
    pub fn populate(&mut self, g: &mut Globals, values: &PhpArray, override_existing_vars: bool) {
        let mut update_loaded_vars = false;
        let mut loaded_vars = dotenv_vars(g);

        for (name, val) in values.iter() {
            let name = string(&name.to_value());
            let not_http_name = !name.starts_with(b"HTTP_");
            if isset(&g.server, &name) && not_http_name && !isset(&g.env, &name) {
                let server = g.server.get(&key(&name)).cloned().unwrap_or(Value::Null);
                g.env.insert(key(&name), server);
            }

            // don't check existence with getenv() because of thread safety issues
            if !loaded_vars.has(&name) && !override_existing_vars && isset(&g.env, &name) {
                continue;
            }

            if self.use_putenv {
                if self.defer_putenv {
                    self.pending_putenv.insert(&name);
                } else {
                    g.putenv(&[&name[..], b"=", &string(val)].concat());
                }
            }

            g.env.insert(key(&name), val.clone());
            if not_http_name {
                g.server.insert(key(&name), val.clone());
            }

            if !loaded_vars.has(&name) {
                loaded_vars.insert(&name);
                update_loaded_vars = true;
            }
        }

        if update_loaded_vars {
            loaded_vars.remove(b"");
            let list = loaded_vars.join();
            g.env.insert("SYMFONY_DOTENV_VARS", value(&list));
            g.server.insert("SYMFONY_DOTENV_VARS", value(&list));

            if self.use_putenv {
                g.putenv(&[&b"SYMFONY_DOTENV_VARS="[..], &list].concat());
            }
        }
    }

    /// `parse()`: parses the contents of an .env file; `path` is the file
    /// the data comes from, for error messages.
    pub fn parse(&mut self, g: &Globals, data: &[u8], path: &str) -> Result<PhpArray, Error> {
        self.start(data, path);
        let loaded_vars = loaded_vars(g);
        let result = self.parse_values(g, Some(&loaded_vars));
        self.values = PhpArray::new();
        self.reset();
        result
    }

    fn start(&mut self, data: &[u8], path: &str) {
        path.clone_into(&mut self.path);
        self.data = replace(&replace(data, b"\r\n", b"\n"), b"\r", b"\n");
        self.lineno = 1;
        self.cursor = 0;
        self.end = self.data.len();
        self.values = PhpArray::new();
    }

    fn reset(&mut self) {
        self.path.clear();
        self.data.clear();
        self.lineno = 0;
        self.cursor = 0;
        self.end = 0;
    }

    /// `parse()`'s loop (`loaded_vars` set) or `parseRaw()`'s (`None`).
    fn parse_values(
        &mut self,
        g: &Globals,
        loaded_vars: Option<&Names>,
    ) -> Result<PhpArray, Error> {
        let mut name = Vec::new();
        let mut in_value = false;

        self.skip_empty_lines();

        while self.cursor < self.end {
            if in_value {
                let raw = self.lex_value()?;
                let resolved = match loaded_vars {
                    Some(loaded) => self.resolve_value(g, &raw, loaded)?,
                    None => raw,
                };
                self.values.insert(key(&name), value(&resolved));
                in_value = false;
            } else {
                name = self.lex_varname()?;
                in_value = true;
            }
        }

        if in_value {
            self.values.insert(key(&name), value(b""));
        }

        Ok(std::mem::take(&mut self.values))
    }

    fn parse_raw(&mut self, g: &Globals, data: &[u8], path: &str) -> Result<PhpArray, Error> {
        self.start(data, path);
        let result = self.parse_values(g, None);
        self.values = PhpArray::new();
        result
    }

    /// Resolves a raw value by expanding commands, variables, backslash
    /// escapes, and restoring literal $ markers.
    fn resolve_value(
        &mut self,
        g: &Globals,
        value: &[u8],
        loaded_vars: &Names,
    ) -> Result<Vec<u8>, Error> {
        let resolved = self.resolve_commands(g, value, loaded_vars)?;
        let resolved = self.resolve_variables(g, &resolved, loaded_vars)?;
        let resolved = replace(&resolved, b"\\\\", b"\\");
        Ok(replace(&resolved, b"\0", b"$"))
    }

    fn byte(&self, i: usize) -> Option<u8> {
        self.data.get(i).copied()
    }

    fn lex_varname(&mut self) -> Result<Vec<u8>, Error> {
        // var name + optional export: /(export[ \t]++)?(VARNAME_REGEX)/A
        let start = self.cursor;
        let mut export = false;
        let mut name_at = start;
        if self.data[start..].starts_with(b"export") {
            let blanks = self.data[start + 6..]
                .iter()
                .take_while(|b| matches!(b, b' ' | b'\t'))
                .count();
            if blanks > 0 && varname(&self.data, start + 6 + blanks).is_some() {
                export = true;
                name_at = start + 6 + blanks;
            }
        }
        let Some(length) = varname(&self.data, name_at) else {
            return Err(self.format_exception("Invalid character in variable name"));
        };
        let matched = self.data[start..name_at + length].to_vec();
        let name = self.data[name_at..name_at + length].to_vec();
        self.move_cursor(&matched);

        match self.byte(self.cursor) {
            None | Some(b'\n' | b'#') => {
                if export {
                    return Err(self.format_exception("Unable to unset an environment variable"));
                }
                Err(self.format_exception("Missing = in the environment variable declaration"))
            }
            Some(b' ' | b'\t') => Err(self.format_exception(
                "Whitespace characters are not supported after the variable name",
            )),
            Some(b'=') => {
                self.cursor += 1;
                Ok(name)
            }
            Some(_) => {
                Err(self.format_exception("Missing = in the environment variable declaration"))
            }
        }
    }

    fn lex_value(&mut self) -> Result<Vec<u8>, Error> {
        // /[ \t]*+(?:#.*)?$/Am: nothing but blanks and a comment
        let blanks = self.data[self.cursor..]
            .iter()
            .take_while(|b| matches!(b, b' ' | b'\t'))
            .count();
        let after = self.cursor + blanks;
        let empty = match self.byte(after) {
            None | Some(b'\n') => Some(after),
            Some(b'#') => Some(
                after
                    + self.data[after..]
                        .iter()
                        .take_while(|&&b| b != b'\n')
                        .count(),
            ),
            Some(_) => None,
        };
        if let Some(end) = empty {
            let matched = self.data[self.cursor..end].to_vec();
            self.move_cursor(&matched);
            self.skip_empty_lines();
            return Ok(Vec::new());
        }

        if matches!(self.data[self.cursor], b' ' | b'\t') {
            return Err(self.format_exception("Whitespace are not supported before the value"));
        }

        let mut v = Vec::new();

        loop {
            match self.data[self.cursor] {
                b'\'' => {
                    let mut len = 0;
                    loop {
                        len += 1;
                        if self.cursor + len == self.end {
                            self.cursor += len;
                            return Err(self.format_exception("Missing quote to end the value"));
                        }
                        if self.data[self.cursor + len] == b'\'' {
                            break;
                        }
                    }

                    // In single-quoted strings, $ is literal and \ has no special meaning.
                    // Double backslashes so they survive the unescape in resolveValue(),
                    // and mark $ as \x00 so it's not treated as a variable reference.
                    let inner = &self.data[self.cursor + 1..self.cursor + len];
                    v.extend(replace(&replace(inner, b"\\", b"\\\\"), b"$", b"\0"));
                    self.cursor += 1 + len;
                }
                b'"' => {
                    let mut value = Vec::new();

                    self.cursor += 1;
                    if self.cursor == self.end {
                        return Err(self.format_exception("Missing quote to end the value"));
                    }

                    while self.data[self.cursor] != b'"'
                        || (self.data[self.cursor - 1] == b'\\'
                            && self.data[self.cursor - 2] != b'\\')
                    {
                        value.push(self.data[self.cursor]);
                        self.cursor += 1;

                        if self.cursor == self.end {
                            return Err(self.format_exception("Missing quote to end the value"));
                        }
                    }
                    self.cursor += 1;
                    let value = replace(&value, b"\\\"", b"\"");
                    let value = replace(&value, b"\\r", b"\r");
                    let value = replace(&value, b"\\n", b"\n");
                    // Mark escaped $ (\$) as \x00 so it's treated as literal
                    v.extend(protect_escaped_dollars(&value));
                }
                _ => {
                    let mut value = Vec::new();
                    let mut prev_chr = self.data[self.cursor - 1];
                    while self.cursor < self.end
                        && !matches!(self.data[self.cursor], b'\n' | b'"' | b'\'')
                        && !(matches!(prev_chr, b' ' | b'\t') && self.data[self.cursor] == b'#')
                    {
                        if self.data[self.cursor] == b'\\'
                            && matches!(self.byte(self.cursor + 1), Some(b'"' | b'\''))
                        {
                            self.cursor += 1;
                        }

                        prev_chr = self.data[self.cursor];
                        value.push(prev_chr);

                        if self.data[self.cursor] == b'$'
                            && self.byte(self.cursor + 1) == Some(b'(')
                        {
                            self.cursor += 1;
                            let nested = self.lex_nested_expression()?;
                            value.push(b'(');
                            value.extend(nested);
                            value.push(b')');
                        }

                        self.cursor += 1;
                    }
                    let value = rtrim(&value).to_vec();
                    let resolved_value = protect_escaped_dollars(&value);

                    if resolved_value == value
                        && value.iter().any(|&b| is_space(b))
                        && !value.contains(&b'$')
                    {
                        return Err(self.format_exception(
                            "A value containing spaces must be surrounded by quotes",
                        ));
                    }

                    v.extend(resolved_value);

                    if self.byte(self.cursor) == Some(b'#') {
                        break;
                    }
                }
            }
            if self.cursor >= self.end || self.data[self.cursor] == b'\n' {
                break;
            }
        }

        self.skip_empty_lines();

        Ok(v)
    }

    /// `lexNestedExpression()`. Where upstream would read past the end of
    /// the data (a value ending right after `$(`), this fails like an
    /// unclosed parenthesis.
    fn lex_nested_expression(&mut self) -> Result<Vec<u8>, Error> {
        self.cursor += 1;
        let mut value = Vec::new();

        loop {
            let Some(c) = self.byte(self.cursor) else {
                return Err(self.format_exception("Missing closing parenthesis."));
            };
            if c == b'\n' || c == b')' {
                break;
            }
            value.push(c);

            if c == b'(' {
                value.extend(self.lex_nested_expression()?);
                value.push(b')');
            }

            self.cursor += 1;

            if self.cursor == self.end {
                return Err(self.format_exception("Missing closing parenthesis."));
            }
        }

        if self.byte(self.cursor) == Some(b'\n') {
            return Err(self.format_exception("Missing closing parenthesis."));
        }

        Ok(value)
    }

    /// `/(?:\s*+(?:#[^\n]*+)?+)++/A`: blanks, newlines and comments.
    fn skip_empty_lines(&mut self) {
        let mut i = self.cursor;
        loop {
            while i < self.end && is_space(self.data[i]) {
                i += 1;
            }
            if i < self.end && self.data[i] == b'#' {
                while i < self.end && self.data[i] != b'\n' {
                    i += 1;
                }
            } else {
                break;
            }
        }
        let matched = self.data[self.cursor..i].to_vec();
        self.move_cursor(&matched);
    }

    /// The variables of the values parsed so far that a command sees.
    fn command_env(&self, g: &Globals, loaded_vars: &Names) -> Vec<(Vec<u8>, Vec<u8>)> {
        let mut env: Vec<(Vec<u8>, Vec<u8>)> = Vec::new();
        for (name, value) in self.values.iter() {
            let name = string(&name.to_value());
            if loaded_vars.has(&name)
                || (!isset(&g.env, &name)
                    && !(isset(&g.server, &name) && !name.starts_with(b"HTTP_")))
            {
                env.push((name, string(value)));
            }
        }
        env
    }

    /// `resolveCommands()`: `$(...)` replaced by the output of `echo $(...)`
    /// (`\$(...)` kept as `$(...)`).
    fn resolve_commands(
        &mut self,
        g: &Globals,
        value: &[u8],
        loaded_vars: &Names,
    ) -> Result<Vec<u8>, Error> {
        if !value.contains(&b'$') {
            return Ok(value.to_vec());
        }

        let mut out = Vec::new();
        let mut i = 0;
        while i < value.len() {
            // (\\)?\$(?<cmd>\(([^()]|\g<cmd>)+\))
            let escaped = value[i] == b'\\' && value.get(i + 1) == Some(&b'$');
            let dollar = if escaped { i + 1 } else { i };
            let found = (value[dollar] == b'$')
                .then(|| balanced(value, dollar + 1))
                .flatten();
            let Some(end) = found else {
                out.push(value[i]);
                i += 1;
                continue;
            };
            let matched = &value[i..end];
            if escaped {
                out.extend_from_slice(&matched[1..]);
            } else {
                out.extend(self.run_command(g, matched, loaded_vars)?);
            }
            i = end;
        }
        Ok(out)
    }

    fn run_command(
        &mut self,
        g: &Globals,
        command: &[u8],
        loaded_vars: &Names,
    ) -> Result<Vec<u8>, Error> {
        if cfg!(windows) {
            return Err(Error::Logic(
                "Resolving commands is not supported on Windows.".to_owned(),
            ));
        }

        let mut env = self.command_env(g, loaded_vars);
        for (name, val) in process_default_env(g) {
            if !env.iter().any(|(n, _)| *n == name) {
                env.push((name, val));
            }
        }
        env.retain(|(name, _)| {
            !matches!(name.as_slice(), b"" | b"argc" | b"argv" | b"ARGC" | b"ARGV")
                && !name.contains(&b'=')
                && !name.contains(&0)
        });

        let command_line = [&b"echo "[..], command].concat();
        match g.commands.run(&command_line, &env) {
            Ok(output) => Ok(rtrim_chars(&output, b"\n\r").to_vec()),
            Err(error_output) => {
                let mut message = b"Issue expanding a command (".to_vec();
                message.extend(error_output);
                message.push(b')');
                Err(self.format_exception(message))
            }
        }
    }

    /// `resolveVariables()`: `$NAME`, `${NAME}`, `${NAME:-default}` and
    /// `${NAME:=default}`.
    fn resolve_variables(
        &mut self,
        g: &Globals,
        value: &[u8],
        loaded_vars: &Names,
    ) -> Result<Vec<u8>, Error> {
        if !value.contains(&b'$') {
            return Ok(value.to_vec());
        }

        let mut out = Vec::new();
        let mut i = 0;
        while i < value.len() {
            if let Some(m) = self.variable_at(value, i) {
                out.extend(self.expand_variable(g, value, &m, loaded_vars)?);
                i = m.end;
            } else {
                out.push(value[i]);
                i += 1;
            }
        }
        Ok(out)
    }

    /// `(?<!\\)(?P<backslashes>\\*)\$(?!\()(?P<opening_brace>\{)?(?P<name>VARNAME)?(?P<default_value>:[-=][^\}]*+)?(?P<closing_brace>\})?`
    /// at `i`.
    fn variable_at(&self, s: &[u8], i: usize) -> Option<Variable> {
        if i > 0 && s[i - 1] == b'\\' {
            return None;
        }
        let backslashes = s[i..].iter().take_while(|&&b| b == b'\\').count();
        let dollar = i + backslashes;
        if s.get(dollar) != Some(&b'$') || s.get(dollar + 1) == Some(&b'(') {
            return None;
        }
        let mut j = dollar + 1;
        let opening_brace = s.get(j) == Some(&b'{');
        if opening_brace {
            j += 1;
        }
        let name = varname(s, j).map(|len| {
            let r = j..j + len;
            j += len;
            r
        });
        let default =
            (s.get(j) == Some(&b':') && matches!(s.get(j + 1), Some(b'-' | b'='))).then(|| {
                let len = 2 + s[j + 2..].iter().take_while(|&&b| b != b'}').count();
                let r = j..j + len;
                j += len;
                r
            });
        let closing_brace = s.get(j) == Some(&b'}');
        if closing_brace {
            j += 1;
        }
        Some(Variable {
            start: i,
            end: j,
            backslashes,
            opening_brace,
            name,
            default,
            closing_brace,
        })
    }

    fn expand_variable(
        &mut self,
        g: &Globals,
        s: &[u8],
        m: &Variable,
        loaded_vars: &Names,
    ) -> Result<Vec<u8>, Error> {
        let whole = &s[m.start..m.end];
        // odd number of backslashes means the $ character is escaped
        if m.backslashes % 2 == 1 {
            return Ok(whole[1..].to_vec());
        }

        // unescaped $ not followed by variable name; a group that did not
        // match is still set (empty) when a later one matched
        if m.name.is_none() && m.default.is_none() && !m.closing_brace {
            return Ok(whole.to_vec());
        }

        if m.opening_brace && !m.closing_brace {
            return Err(self.format_exception("Unclosed braces on variable expansion"));
        }

        let name = m.name.clone().map_or(&b""[..], |r| &s[r]);
        let mut is_external = false;
        let own = get(&self.values, name).map(string);
        let mut val = if loaded_vars.has(name) && own.is_some() {
            own.clone().unwrap_or_default()
        } else if let Some(v) = get(&g.env, name) {
            is_external = true;
            string(v)
        } else if let Some(v) = get(&g.server, name).filter(|_| !name.starts_with(b"HTTP_")) {
            is_external = true;
            string(v)
        } else if let Some(own) = own {
            own
        } else {
            is_external = true;
            g.getenv(name).unwrap_or_default()
        };

        if !val.is_empty() && !loaded_vars.has(name) {
            if is_external {
                // unlike values parsed from a .env file, external ones are not escaped yet
                val = replace(&val, b"\\", b"\\\\");
            }
            val = replace(&val, b"$", b"\0");
        }

        let default = m.default.clone().map(|r| &s[r]);
        if val.is_empty()
            && let Some(default) = default
        {
            if let Some(&bad) = default
                .iter()
                .find(|b| matches!(b, b'\'' | b'"' | b'{' | b'$'))
            {
                return Err(self.format_exception(format!(
                    "Unsupported character \"{}\" found in the default value of variable \"${}\".",
                    char::from(bad),
                    str_of(name)
                )));
            }

            val = default[2..].to_vec();

            if default[1] == b'=' {
                self.values.insert(key(name), value(&val));
            }
        }

        if !m.opening_brace && m.closing_brace {
            val.push(b'}');
        }

        let mut out = vec![b'\\'; m.backslashes];
        out.extend(val);
        Ok(out)
    }

    fn move_cursor(&mut self, text: &[u8]) {
        self.cursor += text.len();
        self.lineno += text.iter().filter(|&&b| b == b'\n').count() as i64;
    }

    fn format_exception(&self, message: impl AsRef<[u8]>) -> Error {
        Error::Format(FormatException::new(
            message,
            FormatExceptionContext::new(
                self.data.clone(),
                self.path.clone(),
                self.lineno,
                self.cursor,
            ),
        ))
    }

    fn do_load(
        &mut self,
        g: &mut Globals,
        override_existing_vars: bool,
        paths: &[&str],
    ) -> Result<(), Error> {
        for &path in paths {
            let p = std::path::Path::new(path);
            let data = match std::fs::read(p) {
                Ok(data) if !p.is_dir() => data,
                _ => return Err(PathException::new(path).into()),
            };

            if data.starts_with(b"\xEF\xBB\xBF") {
                return Err(FormatException::new(
                    "Loading files starting with a byte-order-mark (BOM) is not supported.",
                    FormatExceptionContext::new(data, path, 1, 0),
                )
                .into());
            }

            if data.contains(&0) {
                return Err(FormatException::new(
                    "Loading files containing NUL bytes is not supported.",
                    FormatExceptionContext::new(data, path, 1, 0),
                )
                .into());
            }

            let values = self.parse_raw(g, &data, path)?;

            let loaded_vars = loaded_vars(g);

            for (name, val) in values.iter() {
                let name = string(&name.to_value());
                let val = string(val);
                let already_external = is_external(g, &name);
                if !isset(&self.overridden_values, &name) {
                    if already_external {
                        let external = get(&g.env, &name)
                            .or_else(|| get(&g.server, &name))
                            .cloned()
                            .unwrap_or(Value::Null);
                        self.overridden_values.insert(key(&name), external);
                    } else if is_self_referencing(&name, &val)
                        && let Some(external) = g.getenv(&name)
                    {
                        // the OS provides the value but neither $_ENV nor $_SERVER is populated;
                        // $localOnly skips the request-scoped SAPI env and its HTTP_* vars
                        self.overridden_values.insert(key(&name), value(&external));
                    }
                }
                if loaded_vars.has(&name) || override_existing_vars || !already_external {
                    self.loaded_raw_vars.insert(&name);
                }
            }

            self.populate(g, &values, override_existing_vars);
        }
        Ok(())
    }

    /// Eagerly resolves a raw env key value so that `loadEnv()` can determine
    /// which additional .env files to load before full deferred resolution.
    fn resolve_env_key(
        &mut self,
        g: &mut Globals,
        val: &[u8],
        name: &[u8],
    ) -> Result<Vec<u8>, Error> {
        let loaded_vars = loaded_vars(g);

        // Save and clear own value so self-referencing defaults work
        let env_backup = get(&g.env, name).cloned();
        let server_backup = get(&g.server, name).cloned();
        g.env.remove(&key(name));
        g.server.remove(&key(name));
        let getenv_backup = self.use_putenv.then(|| g.getenv(name).unwrap_or_default());
        if self.use_putenv {
            g.putenv(name);
        }

        self.values = PhpArray::new();
        self.reset();

        // as upstream, nothing is restored when resolving fails
        let resolved = self.resolve_commands(g, val, &loaded_vars)?;
        let resolved = self.resolve_variables(g, &resolved, &loaded_vars)?;

        if let Some(backup) = env_backup {
            g.env.insert(key(name), backup);
        }
        if let Some(backup) = server_backup {
            g.server.insert(key(name), backup);
        }
        if let Some(backup) = getenv_backup {
            g.putenv(&[name, b"=", &backup].concat());
        }

        self.values = PhpArray::new();

        let resolved = replace(&resolved, b"\0", b"$");
        Ok(replace(&resolved, b"\\\\", b"\\"))
    }

    fn resolve_loaded_vars(&mut self, g: &mut Globals) -> Result<(), Error> {
        let result = self.do_resolve_loaded_vars(g);
        self.values = PhpArray::new();
        self.overridden_values = PhpArray::new();
        self.pending_putenv = Names::default();
        self.reset();
        result
    }

    fn do_resolve_loaded_vars(&mut self, g: &mut Globals) -> Result<(), Error> {
        let loaded_vars = loaded_vars(g);

        let mut raw_vars = std::mem::take(&mut self.loaded_raw_vars);
        raw_vars.remove(b"SYMFONY_DOTENV_VARS");

        self.values = PhpArray::new();
        self.reset();

        let env_value =
            |g: &Globals, name: &[u8]| get(&g.env, name).map(string).unwrap_or_default();

        // Detect variables that were originally defined as self-referencing
        // so their own raw value is hidden during resolution, allowing the
        // external value or the default to trigger correctly.
        let self_referencing: Vec<Vec<u8>> = raw_vars
            .0
            .iter()
            .filter(|name| is_self_referencing(name, &env_value(g, name)))
            .cloned()
            .collect();

        let mut pass = 0;
        let mut resolved = PhpArray::new();
        while pass < 5 {
            resolved = PhpArray::new();
            for name in &raw_vars.0 {
                let val = env_value(g, name);
                if !val.contains(&b'$') {
                    continue;
                }

                let is_self = self_referencing.contains(name);
                let mut backups = None;
                if is_self {
                    let env_backup = get(&g.env, name).cloned();
                    let server_backup = get(&g.server, name).cloned();
                    if let Some(overridden) = get(&self.overridden_values, name).map(string) {
                        let hidden =
                            replace(&replace(&overridden, b"\\\\", b"\\\\\\\\"), b"$", b"\0");
                        g.env.insert(key(name), value(&hidden));
                        g.server.insert(key(name), value(&hidden));
                    } else {
                        g.env.remove(&key(name));
                        g.server.remove(&key(name));
                    }
                    let getenv_backup = self.use_putenv.then(|| g.getenv(name).unwrap_or_default());
                    if self.use_putenv {
                        match get(&self.overridden_values, name).map(string) {
                            Some(overridden) => g.putenv(&[&name[..], b"=", &overridden].concat()),
                            None => g.putenv(name),
                        }
                    }
                    backups = Some((env_backup, server_backup, getenv_backup));
                }

                // as upstream, nothing is restored when resolving fails
                let resolved_value = self.resolve_commands(g, &val, &loaded_vars)?;
                let resolved_value = self.resolve_variables(g, &resolved_value, &loaded_vars)?;

                if let Some((env_backup, server_backup, getenv_backup)) = backups {
                    if let Some(backup) = env_backup {
                        g.env.insert(key(name), backup);
                    }
                    if let Some(backup) = server_backup {
                        g.server.insert(key(name), backup);
                    }
                    if let Some(backup) = getenv_backup {
                        g.putenv(&[&name[..], b"=", &backup].concat());
                    }
                }

                if val != resolved_value {
                    resolved.insert(key(name), value(&resolved_value));
                }
            }
            if resolved.is_empty() {
                break;
            }
            self.populate(g, &resolved, true);
            pass += 1;
        }
        if pass == 5 && !resolved.is_empty() {
            let names: Vec<String> = resolved
                .keys()
                .map(|k| str_of(&string(&k.to_value())))
                .collect();
            return Err(Error::VariableCircularReference(format!(
                "Too many levels of variable indirection in env vars: {}.",
                names.join(", ")
            )));
        }

        // Restore literal $ signs and unescape backslashes
        let mut restored = PhpArray::new();
        for name in &raw_vars.0 {
            let val = env_value(g, name);
            let new_value = replace(&replace(&val, b"\0", b"$"), b"\\\\", b"\\");
            if val != new_value {
                restored.insert(key(name), value(&new_value));
            }
        }
        if !restored.is_empty() {
            self.populate(g, &restored, true);
        }

        if self.use_putenv && !self.pending_putenv.0.is_empty() {
            for name in std::mem::take(&mut self.pending_putenv).0 {
                let val = env_value(g, &name);
                g.putenv(&[&name[..], b"=", &val].concat());
            }
        }
        Ok(())
    }

    fn populate_path(&mut self, g: &mut Globals, path: &str) {
        g.env.insert("SYMFONY_DOTENV_PATH", Value::from(path));
        g.server.insert("SYMFONY_DOTENV_PATH", Value::from(path));

        if !self.use_putenv {
            return;
        }
        if self.defer_putenv {
            self.pending_putenv.insert(b"SYMFONY_DOTENV_PATH");
        } else {
            g.putenv(format!("SYMFONY_DOTENV_PATH={path}").as_bytes());
        }
    }
}

/// A `$` variable reference found by [`Dotenv::variable_at`].
#[derive(Debug, Clone)]
struct Variable {
    start: usize,
    end: usize,
    backslashes: usize,
    opening_brace: bool,
    name: Option<std::ops::Range<usize>>,
    default: Option<std::ops::Range<usize>>,
    closing_brace: bool,
}

/// `\(([^()]|\g<cmd>)+\)` at `i`: the end of the balanced group.
fn balanced(s: &[u8], i: usize) -> Option<usize> {
    if s.get(i) != Some(&b'(') {
        return None;
    }
    let mut j = i + 1;
    let mut items = 0;
    loop {
        match s.get(j) {
            Some(b')') if items > 0 => return Some(j + 1),
            Some(b'(') => {
                j = balanced(s, j)?;
                items += 1;
            }
            Some(b')') | None => return None,
            Some(_) => {
                j += 1;
                items += 1;
            }
        }
    }
}

/// Converts \$ (escaped dollar) to \x00 (literal marker), handling
/// even/odd backslash counts correctly: \$ → \x00, \\$ → \\$ (unchanged).
fn protect_escaped_dollars(value: &[u8]) -> Vec<u8> {
    if !value.contains(&b'$') {
        return value.to_vec();
    }

    // /\\+\$/
    let mut out = Vec::with_capacity(value.len());
    let mut i = 0;
    while i < value.len() {
        if value[i] == b'\\' {
            let run = value[i..].iter().take_while(|&&b| b == b'\\').count();
            if value.get(i + run) == Some(&b'$') {
                if run % 2 == 1 {
                    out.extend(std::iter::repeat_n(b'\\', run - 1));
                    out.push(0);
                } else {
                    out.extend(std::iter::repeat_n(b'\\', run));
                    out.push(b'$');
                }
                i += run + 1;
                continue;
            }
            out.extend(std::iter::repeat_n(b'\\', run));
            i += run;
            continue;
        }
        out.push(value[i]);
        i += 1;
    }
    out
}

/// What Symfony Process gives a command on the CLI besides its own
/// variables: `$_ENV`, then `getenv()` restricted to the names in
/// `$_SERVER` (or all of it when none is there).
fn process_default_env(g: &Globals) -> Vec<(Vec<u8>, Vec<u8>)> {
    let all = g.getenv_all();
    let in_server: Vec<(Vec<u8>, Vec<u8>)> = all
        .iter()
        .filter(|(name, _)| g.server.contains_key(&key(name)))
        .cloned()
        .collect();
    let getenv = if in_server.is_empty() { all } else { in_server };

    let mut env: Vec<(Vec<u8>, Vec<u8>)> = Vec::new();
    for (name, val) in g.env.iter() {
        if let Value::Array(_) | Value::Object(_) = val {
            continue;
        }
        if matches!(val, Value::Bool(false)) {
            continue;
        }
        env.push((string(&name.to_value()), string(val)));
    }
    for (name, val) in getenv {
        if !env.iter().any(|(n, _)| *n == name) {
            env.push((name, val));
        }
    }
    env
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn balanced_groups_need_content() {
        assert_eq!(balanced(b"(a)", 0), Some(3));
        assert_eq!(balanced(b"(a(b)c)x", 0), Some(7));
        assert_eq!(balanced(b"()", 0), None);
        assert_eq!(balanced(b"(a(b)", 0), None);
    }

    #[test]
    fn escaped_dollars() {
        assert_eq!(protect_escaped_dollars(b"\\$a"), b"\0a");
        assert_eq!(protect_escaped_dollars(b"\\\\$a"), b"\\\\$a");
        assert_eq!(protect_escaped_dollars(b"\\\\\\$a"), b"\\\\\0a");
        assert_eq!(protect_escaped_dollars(b"a\\b"), b"a\\b");
    }

    #[test]
    fn self_references() {
        assert!(is_self_referencing(b"FOO", b"${FOO:-x}"));
        assert!(is_self_referencing(b"FOO", b"a$FOO"));
        assert!(!is_self_referencing(b"FOO", b"$FOOBAR"));
    }
}
