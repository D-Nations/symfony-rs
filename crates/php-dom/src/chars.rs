//! XML 1.0 (fifth edition) character classes: `Char`, `S`, `NameStartChar`
//! and `NameChar`, as libxml applies them without `XML_PARSE_OLD10`.
//!
//! Upstream: none

/// `S`, tested on a byte as libxml does (`IS_BLANK_CH`).
pub(crate) fn is_blank(b: u8) -> bool {
    matches!(b, 0x20 | 0x09 | 0x0A | 0x0D)
}

/// `Char`.
pub(crate) fn is_char(c: u32) -> bool {
    matches!(c, 0x9 | 0xA | 0xD | 0x20..=0xD7FF | 0xE000..=0xFFFD | 0x10000..=0x0010_FFFF)
}

/// `NameStartChar`.
pub(crate) fn is_name_start(c: u32) -> bool {
    matches!(c,
        0x3A | 0x41..=0x5A | 0x5F | 0x61..=0x7A | 0xC0..=0xD6 | 0xD8..=0xF6 | 0xF8..=0x2FF
        | 0x370..=0x37D | 0x37F..=0x1FFF | 0x200C..=0x200D | 0x2070..=0x218F
        | 0x2C00..=0x2FEF | 0x3001..=0xD7FF | 0xF900..=0xFDCF | 0xFDF0..=0xFFFD
        | 0x10000..=0xEFFFF)
}

/// `NameChar`.
pub(crate) fn is_name_char(c: u32) -> bool {
    is_name_start(c)
        || matches!(c, 0x2D | 0x2E | 0x30..=0x39 | 0xB7 | 0x300..=0x36F | 0x203F..=0x2040)
}

/// The ASCII bytes libxml's name fast path accepts as a first character.
pub(crate) fn is_ascii_name_start(b: u8) -> bool {
    b.is_ascii_alphabetic() || b == b'_' || b == b':'
}

/// The ASCII bytes libxml's name fast path accepts after the first.
pub(crate) fn is_ascii_name_char(b: u8) -> bool {
    b.is_ascii_alphanumeric() || matches!(b, b'_' | b'-' | b':' | b'.')
}

/// `PubidChar`.
pub(crate) fn is_pubid_char(b: u8) -> bool {
    b.is_ascii_alphanumeric()
        || matches!(
            b,
            0x20 | 0x0D
                | 0x0A
                | b'-'
                | b'\''
                | b'('
                | b')'
                | b'+'
                | b','
                | b'.'
                | b'/'
                | b':'
                | b'='
                | b'?'
                | b';'
                | b'!'
                | b'*'
                | b'#'
                | b'@'
                | b'$'
                | b'_'
                | b'%'
        )
}
