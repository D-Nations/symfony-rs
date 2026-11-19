//! PHP's process globals as Dotenv sees them: `$_ENV`, `$_SERVER`,
//! `getenv()` / `putenv()`, and the shell that runs `$(...)` commands.
//!
//! `putenv()` writes an overlay that `getenv()` reads before the real
//! process environment: changing the process environment is `unsafe` in
//! Rust (other threads may read it), which the crates forbid. A program
//! that wants child processes to inherit the variables passes them on
//! (as the command runner here does) or copies them at startup itself.
//!
//! Upstream: none

use std::collections::BTreeMap;

use php_value::{Key, PhpArray, PhpString, Value};

/// Runs a `$(...)` command of an .env value.
pub trait Commands: Send + Sync {
    /// Runs `command_line` with `/bin/sh -c` and exactly the environment
    /// `env`: the standard output on success, the error output when the
    /// command fails.
    fn run(&self, command_line: &[u8], env: &[(Vec<u8>, Vec<u8>)]) -> Result<Vec<u8>, Vec<u8>>;
}

/// `/bin/sh -c`, as Symfony Process runs a shell command line on Unix.
#[derive(Debug, Clone, Copy, Default)]
pub struct Shell;

impl Commands for Shell {
    fn run(&self, command_line: &[u8], env: &[(Vec<u8>, Vec<u8>)]) -> Result<Vec<u8>, Vec<u8>> {
        let mut command = std::process::Command::new("/bin/sh");
        command.arg("-c").arg(os(command_line)).env_clear();
        for (name, value) in env {
            command.env(os(name), os(value));
        }
        command.stdin(std::process::Stdio::null());
        match command.output() {
            Ok(output) if output.status.success() => Ok(output.stdout),
            Ok(output) => Err(output.stderr),
            Err(e) => Err(e.to_string().into_bytes()),
        }
    }
}

#[cfg(unix)]
fn os(bytes: &[u8]) -> std::ffi::OsString {
    use std::os::unix::ffi::OsStrExt;
    std::ffi::OsStr::from_bytes(bytes).to_os_string()
}

#[cfg(not(unix))]
fn os(bytes: &[u8]) -> std::ffi::OsString {
    String::from_utf8_lossy(bytes).into_owned().into()
}

/// `$_ENV`, `$_SERVER` and the environment `getenv()` reads.
pub struct Globals {
    /// `$_ENV`.
    pub env: PhpArray,
    /// `$_SERVER`.
    pub server: PhpArray,
    /// `putenv()` settings: a value, or `None` once unset.
    overlay: BTreeMap<Vec<u8>, Option<Vec<u8>>>,
    /// Whether `getenv()` falls back to the real process environment.
    process: bool,
    /// What runs `$(...)` commands.
    pub commands: Box<dyn Commands>,
}

impl std::fmt::Debug for Globals {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Globals")
            .field("env", &self.env)
            .field("server", &self.server)
            .field("overlay", &self.overlay)
            .field("process", &self.process)
            .finish_non_exhaustive()
    }
}

impl Default for Globals {
    fn default() -> Self {
        Self::new()
    }
}

impl Globals {
    /// Empty globals: no `$_ENV`, no `$_SERVER`, and `getenv()` sees only
    /// what `putenv()` set.
    pub fn new() -> Self {
        Self {
            env: PhpArray::new(),
            server: PhpArray::new(),
            overlay: BTreeMap::new(),
            process: false,
            commands: Box::new(Shell),
        }
    }

    /// The globals of PHP's CLI with PHP's built-in `variables_order`
    /// (`EGPCS`, when no php.ini sets it): the process environment in
    /// `$_ENV` and `$_SERVER`, and `getenv()` reading it. Pass
    /// `with_env: false` for php.ini's `GPCS`, which leaves `$_ENV` empty.
    pub fn from_process(with_env: bool) -> Self {
        let mut globals = Self::new();
        globals.process = true;
        for (name, value) in std::env::vars_os() {
            let name = name.to_string_lossy().into_owned();
            let value = Value::from(value.to_string_lossy().as_ref());
            if with_env {
                globals.env.insert(name.as_str(), value.clone());
            }
            globals.server.insert(name.as_str(), value);
        }
        globals
    }

    /// `getenv($name)`.
    pub fn getenv(&self, name: &[u8]) -> Option<Vec<u8>> {
        if let Some(setting) = self.overlay.get(name) {
            return setting.clone();
        }
        if !self.process {
            return None;
        }
        let name = std::str::from_utf8(name).ok()?;
        std::env::var_os(name).map(|v| v.to_string_lossy().into_owned().into_bytes())
    }

    /// `getenv()`: every variable.
    pub fn getenv_all(&self) -> Vec<(Vec<u8>, Vec<u8>)> {
        let mut all: BTreeMap<Vec<u8>, Vec<u8>> = BTreeMap::new();
        if self.process {
            for (name, value) in std::env::vars_os() {
                all.insert(
                    name.to_string_lossy().into_owned().into_bytes(),
                    value.to_string_lossy().into_owned().into_bytes(),
                );
            }
        }
        for (name, setting) in &self.overlay {
            match setting {
                Some(value) => all.insert(name.clone(), value.clone()),
                None => all.remove(name),
            };
        }
        all.into_iter().collect()
    }

    /// `putenv($setting)`: `NAME=value` sets, `NAME` unsets.
    pub fn putenv(&mut self, setting: &[u8]) {
        match setting.iter().position(|&b| b == b'=') {
            Some(i) => {
                self.overlay
                    .insert(setting[..i].to_vec(), Some(setting[i + 1..].to_vec()));
            }
            None => {
                self.overlay.insert(setting.to_vec(), None);
            }
        }
    }
}

/// `$array[$name]` as a key.
pub(crate) fn key(name: &[u8]) -> Key {
    Key::from(PhpString::from(name))
}

/// `isset($array[$name])`.
pub(crate) fn isset(array: &PhpArray, name: &[u8]) -> bool {
    array.get(&key(name)).is_some_and(|v| !v.is_null())
}

/// `$array[$name] ?? null`, as set.
pub(crate) fn get<'a>(array: &'a PhpArray, name: &[u8]) -> Option<&'a Value> {
    array.get(&key(name)).filter(|v| !v.is_null())
}

/// `(string) $value`.
pub(crate) fn string(value: &Value) -> Vec<u8> {
    match value {
        Value::Array(_) => b"Array".to_vec(),
        other => other
            .to_php_string()
            .map(|s| s.as_bytes().to_vec())
            .unwrap_or_default(),
    }
}
