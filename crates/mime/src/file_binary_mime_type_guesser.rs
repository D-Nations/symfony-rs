//! Port of Symfony\Component\Mime\FileBinaryMimeTypeGuesser.php: the MIME
//! type that the `file` command prints.
//!
//! As upstream, the command runs through the shell (`shell_exec()` is
//! `/bin/sh -c`), with the path quoted as `escapeshellarg()` quotes it
//! on Unix (`'...'`, a `'` written `'\''`) and prefixed with `./` when it
//! starts with `-`. Whether `file` exists is asked once per process, as
//! upstream's `static $supported`. On Windows the guesser is not
//! supported, as upstream.
//!
//! A custom command is a `sprintf()` format with one `%s`; `%%` is a `%`.
//! A format with any other conversion is refused (`Class::NotPorted`).
//!
//! Symfony source copyright (c) Fabien Potencier; distributed under MIT.
//!
//! Upstream: symfony/mime FileBinaryMimeTypeGuesser.php

use std::sync::OnceLock;

use symfony_finder::fs;

use crate::{
    exception::{Class, Error, Result},
    mime_types::MimeTypeGuesser,
};

const CLASS: &str = "Symfony\\Component\\Mime\\FileBinaryMimeTypeGuesser";

/// `new FileBinaryMimeTypeGuesser($cmd)`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FileBinaryMimeTypeGuesser {
    cmd: String,
}

impl Default for FileBinaryMimeTypeGuesser {
    fn default() -> Self {
        Self::new()
    }
}

/// `escapeshellarg($arg)` on Unix.
pub fn escapeshellarg(arg: &[u8]) -> Vec<u8> {
    let mut out = Vec::with_capacity(arg.len() + 2);
    out.push(b'\'');
    for &b in arg {
        if b == b'\'' {
            out.extend_from_slice(b"'\\''");
        } else {
            out.push(b);
        }
    }
    out.push(b'\'');
    out
}

/// `shell_exec($command)`: what the command prints, or `None` (PHP's
/// null) when it prints nothing or cannot run.
#[cfg(unix)]
fn shell_exec(command: &[u8]) -> Option<Vec<u8>> {
    use std::os::unix::ffi::OsStrExt;
    let output = std::process::Command::new("/bin/sh")
        .arg("-c")
        .arg(std::ffi::OsStr::from_bytes(command))
        .stdin(std::process::Stdio::null())
        .output()
        .ok()?;
    (!output.stdout.is_empty()).then_some(output.stdout)
}

#[cfg(not(unix))]
fn shell_exec(_command: &[u8]) -> Option<Vec<u8>> {
    None
}

/// PHP's `trim()` with its default characters.
fn trim(s: &[u8]) -> &[u8] {
    let blank = |b: &u8| matches!(b, b' ' | b'\t' | b'\n' | b'\r' | b'\0' | b'\x0B');
    let start = s.iter().position(|b| !blank(b)).unwrap_or(s.len());
    let end = s.iter().rposition(|b| !blank(b)).map_or(start, |i| i + 1);
    &s[start..end]
}

/// `preg_match('#^([a-z0-9\-]+/[a-z0-9\-\+\.]+)#i', $type, $match)`:
/// `$match[1]`.
fn leading_type(s: &[u8]) -> Option<&[u8]> {
    let first = s
        .iter()
        .take_while(|b| b.is_ascii_alphanumeric() || **b == b'-')
        .count();
    if first == 0 || s.get(first) != Some(&b'/') {
        return None;
    }
    let rest = &s[first + 1..];
    let second = rest
        .iter()
        .take_while(|b| b.is_ascii_alphanumeric() || matches!(b, b'-' | b'+' | b'.'))
        .count();
    (second > 0).then(|| &s[..first + 1 + second])
}

impl FileBinaryMimeTypeGuesser {
    /// The default command, `file -b --mime -- %s 2>/dev/null`.
    pub fn new() -> Self {
        Self::with_command("file -b --mime -- %s 2>/dev/null")
    }

    /// A command whose output starts with the MIME type; `%s` is replaced
    /// with the quoted file name.
    pub fn with_command(cmd: impl Into<String>) -> Self {
        Self { cmd: cmd.into() }
    }

    /// `sprintf($this->cmd, $arg)`.
    fn command(&self, arg: &[u8]) -> Result<Vec<u8>> {
        let cmd = self.cmd.as_bytes();
        let mut out = Vec::with_capacity(cmd.len() + arg.len());
        let mut used = false;
        let mut i = 0;
        while i < cmd.len() {
            if cmd[i] != b'%' {
                out.push(cmd[i]);
                i += 1;
                continue;
            }
            match cmd.get(i + 1) {
                Some(b'%') => out.push(b'%'),
                Some(b's') if !used => {
                    out.extend_from_slice(arg);
                    used = true;
                }
                _ => {
                    return Err(Error::new(
                        Class::NotPorted,
                        format!("FileBinaryMimeTypeGuesser command format {:?}", self.cmd),
                    ));
                }
            }
            i += 2;
        }
        // without a %s, sprintf() ignores the argument
        Ok(out)
    }
}

impl MimeTypeGuesser for FileBinaryMimeTypeGuesser {
    fn is_guesser_supported(&self) -> bool {
        static SUPPORTED: OnceLock<bool> = OnceLock::new();
        *SUPPORTED.get_or_init(|| {
            if cfg!(windows) {
                return false;
            }
            shell_exec(b"command -v file").is_some_and(|out| !trim(&out).is_empty())
        })
    }

    fn guess_mime_type(&self, path: &[u8]) -> Result<Option<String>> {
        if !fs::is_file(path) || !fs::is_readable(path) {
            return Err(Error::new(
                Class::InvalidArgument,
                format!(
                    "The \"{}\" file does not exist or is not readable.",
                    String::from_utf8_lossy(path)
                ),
            ));
        }
        if !self.is_guesser_supported() {
            return Err(Error::new(
                Class::Logic,
                format!("The \"{CLASS}\" guesser is not supported."),
            ));
        }
        let mut arg = Vec::with_capacity(path.len() + 2);
        if path.starts_with(b"-") {
            arg.extend_from_slice(b"./");
        }
        arg.extend_from_slice(path);
        let command = self.command(&escapeshellarg(&arg))?;
        let output = shell_exec(&command).unwrap_or_default();
        Ok(leading_type(trim(&output)).map(|t| String::from_utf8_lossy(t).into_owned()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn quotes_as_escapeshellarg() {
        assert_eq!(escapeshellarg(b"a b"), b"'a b'");
        assert_eq!(escapeshellarg(b"it's"), b"'it'\\''s'");
        assert_eq!(escapeshellarg(b""), b"''");
    }

    #[test]
    fn reads_the_leading_type() {
        assert_eq!(
            leading_type(b"image/gif; charset=binary"),
            Some(&b"image/gif"[..])
        );
        assert_eq!(
            leading_type(b"application/vnd.ms-excel.sheet.macroEnabled.12"),
            Some(&b"application/vnd.ms-excel.sheet.macroEnabled.12"[..])
        );
        assert_eq!(leading_type(b"cannot open `x'"), None);
        assert_eq!(leading_type(b"/x"), None);
        assert_eq!(leading_type(b"a/"), None);
    }

    #[test]
    fn formats_the_command() {
        let g = FileBinaryMimeTypeGuesser::with_command("x %s 100%%");
        assert_eq!(g.command(b"'p'"), Ok(b"x 'p' 100%".to_vec()));
        let g = FileBinaryMimeTypeGuesser::with_command("x %d");
        assert_eq!(
            g.command(b"'p'").map_err(|e| e.class()),
            Err(Class::NotPorted)
        );
    }
}
