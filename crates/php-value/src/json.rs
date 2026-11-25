//! `json_encode()` ([`encode`], and [`encode_string`] for one string)
//! and `json_decode()` ([`decode_assoc`], [`decode_object`]), written
//! from PHP's documentation and its recorded answers (no PHP source).
//!
//! Upstream: none

mod decode;
mod encode;
pub use decode::{JsonError, decode_assoc, decode_object};
pub use encode::{
    EncodeFailure, Encoded, JSON_FORCE_OBJECT, JSON_HEX_AMP, JSON_HEX_APOS, JSON_HEX_QUOT,
    JSON_HEX_TAG, JSON_INVALID_UTF8_IGNORE, JSON_INVALID_UTF8_SUBSTITUTE, JSON_NUMERIC_CHECK,
    JSON_PARTIAL_OUTPUT_ON_ERROR, JSON_PRESERVE_ZERO_FRACTION, JSON_PRETTY_PRINT,
    JSON_THROW_ON_ERROR, JSON_UNESCAPED_LINE_TERMINATORS, JSON_UNESCAPED_SLASHES,
    JSON_UNESCAPED_UNICODE, encode,
};

/// The `json_encode()` flags that affect strings.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct JsonFlags {
    pub unescaped_slashes: bool,
    pub unescaped_unicode: bool,
}

/// `json_encode($string, $flags)`, as recorded from PHP 8.4.27
/// (tests/data/php.json): `"` `\` and `\b \f \n \r \t` escaped by name,
/// other control characters as `\u00XX`, DEL kept, `/` as `\/` unless
/// `JSON_UNESCAPED_SLASHES`, non-ASCII as `\uXXXX` (surrogate pairs above
/// U+FFFF) unless `JSON_UNESCAPED_UNICODE`, U+2028 and U+2029 always
/// escaped, and `false` (here `None`) for invalid UTF-8.
pub fn encode_string(bytes: &[u8], flags: JsonFlags) -> Option<String> {
    let mut bits = 0;
    if flags.unescaped_slashes {
        bits |= JSON_UNESCAPED_SLASHES;
    }
    if flags.unescaped_unicode {
        bits |= JSON_UNESCAPED_UNICODE;
    }
    encode::escape(bytes, bits)
}
