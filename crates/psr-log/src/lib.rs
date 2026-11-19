//! Port of psr/log (PSR-3): LoggerInterface.php, LogLevel.php,
//! NullLogger.php, with AbstractLogger.php and LoggerTrait.php as the
//! trait's provided methods.
//!
//! PSR-3's `$level` is any value and an unknown one throws
//! InvalidArgumentException; here it is the closed `LogLevel` enum, so that
//! error cannot occur. `$context` is an ordered list of named values; the
//! `exception` key holds a `Throwable`, as PSR-3 specifies.
//!
//! psr/log copyright (c) PHP Framework Interoperability Group; MIT.
//!
//! Upstream: psr/log src/LoggerInterface.php src/LogLevel.php src/NullLogger.php src/AbstractLogger.php src/LoggerTrait.php

#![forbid(unsafe_code)]

use std::{fmt, sync::Arc};

/// The eight RFC 5424 levels. `as_str()` is the PSR-3 string form.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum LogLevel {
    Emergency,
    Alert,
    Critical,
    Error,
    Warning,
    Notice,
    Info,
    Debug,
}

impl LogLevel {
    pub const ALL: [Self; 8] = [
        Self::Emergency,
        Self::Alert,
        Self::Critical,
        Self::Error,
        Self::Warning,
        Self::Notice,
        Self::Info,
        Self::Debug,
    ];

    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Emergency => "emergency",
            Self::Alert => "alert",
            Self::Critical => "critical",
            Self::Error => "error",
            Self::Warning => "warning",
            Self::Notice => "notice",
            Self::Info => "info",
            Self::Debug => "debug",
        }
    }
}

impl fmt::Display for LogLevel {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

/// A context value.
#[derive(Clone)]
pub enum ContextValue {
    String(String),
    /// The `exception` entry.
    Exception(Arc<dyn std::error::Error + Send + Sync>),
}

impl From<&str> for ContextValue {
    fn from(value: &str) -> Self {
        Self::String(value.to_owned())
    }
}

impl From<String> for ContextValue {
    fn from(value: String) -> Self {
        Self::String(value)
    }
}

/// Exceptions compare by message: they have no other comparable content.
impl PartialEq for ContextValue {
    fn eq(&self, other: &Self) -> bool {
        match (self, other) {
            (Self::String(a), Self::String(b)) => a == b,
            (Self::Exception(a), Self::Exception(b)) => a.to_string() == b.to_string(),
            _ => false,
        }
    }
}

impl fmt::Debug for ContextValue {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::String(s) => write!(f, "{s:?}"),
            Self::Exception(e) => write!(f, "Exception({e})"),
        }
    }
}

/// `array $context`, in order.
pub type Context = Vec<(String, ContextValue)>;

/// Builds a context from pairs: `context([("event", "foo")])`.
pub fn context<K: Into<String>, V: Into<ContextValue>>(
    pairs: impl IntoIterator<Item = (K, V)>,
) -> Context {
    pairs
        .into_iter()
        .map(|(k, v)| (k.into(), v.into()))
        .collect()
}

/// Describes a logger instance. Messages may contain `{placeholder}`s that
/// implementors replace with context values.
pub trait LoggerInterface: Send + Sync {
    /// Logs with an arbitrary level.
    fn log(&self, level: LogLevel, message: &str, context: Context);

    /// System is unusable.
    fn emergency(&self, message: &str, context: Context) {
        self.log(LogLevel::Emergency, message, context);
    }

    /// Action must be taken immediately.
    fn alert(&self, message: &str, context: Context) {
        self.log(LogLevel::Alert, message, context);
    }

    /// Critical conditions.
    fn critical(&self, message: &str, context: Context) {
        self.log(LogLevel::Critical, message, context);
    }

    /// Runtime errors that do not require immediate action.
    fn error(&self, message: &str, context: Context) {
        self.log(LogLevel::Error, message, context);
    }

    /// Exceptional occurrences that are not errors.
    fn warning(&self, message: &str, context: Context) {
        self.log(LogLevel::Warning, message, context);
    }

    /// Normal but significant events.
    fn notice(&self, message: &str, context: Context) {
        self.log(LogLevel::Notice, message, context);
    }

    /// Interesting events.
    fn info(&self, message: &str, context: Context) {
        self.log(LogLevel::Info, message, context);
    }

    /// Detailed debug information.
    fn debug(&self, message: &str, context: Context) {
        self.log(LogLevel::Debug, message, context);
    }
}

/// Discards every record, so callers need no `if logger` checks.
#[derive(Debug, Default, Clone, Copy)]
pub struct NullLogger;

impl LoggerInterface for NullLogger {
    fn log(&self, _: LogLevel, _: &str, _: Context) {}
}

#[cfg(test)]
mod tests {
    use std::sync::Mutex;

    use super::*;

    #[derive(Default)]
    struct Recorder(Mutex<Vec<(LogLevel, String, Context)>>);

    impl LoggerInterface for Recorder {
        fn log(&self, level: LogLevel, message: &str, context: Context) {
            if let Ok(mut records) = self.0.lock() {
                records.push((level, message.to_owned(), context));
            }
        }
    }

    #[test]
    fn level_methods_log_at_their_level() {
        let logger = Recorder::default();
        logger.emergency("m", Context::new());
        logger.alert("m", Context::new());
        logger.critical("m", Context::new());
        logger.error("m", Context::new());
        logger.warning("m", Context::new());
        logger.notice("m", Context::new());
        logger.info("m", Context::new());
        logger.debug("m", context([("event", "foo")]));
        let records = logger.0.lock().map(|r| r.clone()).unwrap_or_default();
        let levels: Vec<&str> = records.iter().map(|(l, _, _)| l.as_str()).collect();
        assert_eq!(
            levels,
            [
                "emergency",
                "alert",
                "critical",
                "error",
                "warning",
                "notice",
                "info",
                "debug"
            ]
        );
        assert_eq!(records[7].2, context([("event", "foo")]));
    }

    #[test]
    fn null_logger_accepts_everything() {
        NullLogger.debug("ignored {x}", context([("x", "y")]));
    }
}
