//! `Fixture\Service\GreetingException` (oracle/apps/converter-fixture/src/Service/GreetingException.php).
//!
//! An exception class: name, message, code and previous exception
//! (converter contract 0.1, section 5). Not an HTTP exception, so it reaches
//! the kernel through `Exception::from_throwable`.
//!
//! Upstream: none

use symfony_http_kernel::Exception;

#[derive(Debug)]
pub struct GreetingException {
    pub message: String,
    pub code: i64,
    pub previous: Option<Box<dyn std::error::Error + Send + Sync>>,
}

impl GreetingException {
    pub const CLASS: &'static str = "Fixture\\Service\\GreetingException";

    pub fn new(message: impl Into<String>) -> Self {
        Self {
            message: message.into(),
            code: 0,
            previous: None,
        }
    }
}

impl std::fmt::Display for GreetingException {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.message)
    }
}

impl std::error::Error for GreetingException {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        self.previous
            .as_deref()
            .map(|e| e as &(dyn std::error::Error + 'static))
    }
}

impl From<GreetingException> for Exception {
    fn from(error: GreetingException) -> Self {
        Exception::from_throwable(error)
    }
}
