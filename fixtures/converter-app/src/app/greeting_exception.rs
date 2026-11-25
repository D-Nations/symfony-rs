//! `Fixture\Service\GreetingException` (oracle/apps/converter-fixture/src/Service/GreetingException.php).
//!
//! An exception class: name, message, code and previous exception
//! (converter contract 0.3, section 5). Thrown as a shared `Throwable`;
//! `instanceof GreetingException` is a downcast.
//!
//! Upstream: none

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
