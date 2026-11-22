//! `libxml_use_internal_errors()`: libxml's per-request setting that keeps
//! errors in its buffer instead of raising PHP warnings. Nothing in this
//! crate raises warnings or keeps a global buffer (every load and
//! validation returns its errors), so the setting is only what callers
//! read back: Symfony's XmlUtils sets it around its work, restores the
//! caller's value and hands that value to a validation callable. PHP keeps
//! it per request; here it is per thread.
//!
//! Upstream: none

use std::cell::Cell;

thread_local! {
    static USE_INTERNAL_ERRORS: Cell<bool> = const { Cell::new(false) };
}

/// `libxml_use_internal_errors($use_errors)`: sets the flag (unless
/// `None`, PHP's `null`) and returns its previous value. It starts `false`.
pub fn libxml_use_internal_errors(use_errors: Option<bool>) -> bool {
    USE_INTERNAL_ERRORS.with(|flag| match use_errors {
        Some(value) => flag.replace(value),
        None => flag.get(),
    })
}

#[cfg(test)]
mod tests {
    use super::libxml_use_internal_errors;

    #[test]
    fn returns_the_previous_value() {
        assert!(!libxml_use_internal_errors(None));
        assert!(!libxml_use_internal_errors(Some(true)));
        assert!(libxml_use_internal_errors(None));
        assert!(libxml_use_internal_errors(Some(false)));
        assert!(!libxml_use_internal_errors(None));
    }
}
