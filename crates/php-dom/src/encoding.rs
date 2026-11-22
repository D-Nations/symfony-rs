//! The encodings libxml decodes itself, as PHP 8.4.27's libxml 2.9.14
//! detects and applies them: the byte-order mark or the first four bytes,
//! then the `encoding` of the XML declaration. Encodings PHP decodes
//! through iconv are refused rather than approximated.
//!
//! Upstream: none

/// What the first four bytes say (`xmlDetectCharEncoding`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Detected {
    /// No detection, or `<?xm` / a UTF-8 byte-order mark (`bom` = 3).
    Utf8 {
        bom: usize,
    },
    Utf16 {
        little_endian: bool,
        bom: usize,
    },
    /// UCS-4 or EBCDIC, which this crate does not decode.
    Other(&'static str),
}

pub(crate) fn detect(b: &[u8]) -> Detected {
    if b.len() < 4 {
        return Detected::Utf8 { bom: 0 };
    }
    match [b[0], b[1], b[2], b[3]] {
        [0, 0, 0, 0x3C] | [0x3C, 0, 0, 0] | [0, 0, 0x3C, 0] | [0, 0x3C, 0, 0] => {
            Detected::Other("UCS-4")
        }
        [0x4C, 0x6F, 0xA7, 0x94] => Detected::Other("EBCDIC"),
        [0, 0x3C, 0, 0x3F] => Detected::Utf16 {
            little_endian: false,
            bom: 0,
        },
        [0x3C, 0, 0x3F, 0] => Detected::Utf16 {
            little_endian: true,
            bom: 0,
        },
        [0xEF, 0xBB, 0xBF, _] => Detected::Utf8 { bom: 3 },
        [0xFE, 0xFF, _, _] => Detected::Utf16 {
            little_endian: false,
            bom: 2,
        },
        [0xFF, 0xFE, _, _] => Detected::Utf16 {
            little_endian: true,
            bom: 2,
        },
        _ => Detected::Utf8 { bom: 0 },
    }
}

/// UTF-16 to UTF-8; `Err(offset)` at the first unit that does not decode.
pub(crate) fn utf16_to_utf8(b: &[u8], little_endian: bool) -> Result<Vec<u8>, usize> {
    let unit = |i: usize| -> Option<u16> {
        let (x, y) = (*b.get(i)?, *b.get(i + 1)?);
        Some(if little_endian {
            u16::from_le_bytes([x, y])
        } else {
            u16::from_be_bytes([x, y])
        })
    };
    let mut out = Vec::with_capacity(b.len());
    let mut i = 0;
    while i + 1 < b.len() {
        let Some(u) = unit(i) else { break };
        let c = match u {
            0xD800..=0xDBFF => match unit(i + 2) {
                Some(l @ 0xDC00..=0xDFFF) => {
                    i += 2;
                    0x10000 + ((u32::from(u) - 0xD800) << 10) + (u32::from(l) - 0xDC00)
                }
                _ => return Err(i),
            },
            0xDC00..=0xDFFF => return Err(i),
            _ => u32::from(u),
        };
        i += 2;
        let Some(ch) = char::from_u32(c) else {
            return Err(i);
        };
        let mut tmp = [0u8; 4];
        out.extend_from_slice(ch.encode_utf8(&mut tmp).as_bytes());
    }
    if i < b.len() {
        return Err(i);
    }
    Ok(out)
}

/// ISO-8859-1 to UTF-8.
pub(crate) fn latin1_to_utf8(b: &[u8]) -> Vec<u8> {
    let mut out = Vec::with_capacity(b.len());
    for &c in b {
        let mut tmp = [0u8; 4];
        out.extend_from_slice(char::from(c).encode_utf8(&mut tmp).as_bytes());
    }
    out
}

/// How a declared encoding name is handled.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Declared {
    /// `UTF-8` / `UTF8`: no conversion.
    Utf8,
    /// `UTF-16` / `UTF16`.
    Utf16,
    /// A name libxml maps to its own UTF-16 decoders.
    Utf16Le,
    Utf16Be,
    /// libxml's built-in ISO-8859-1 decoder, and glibc's names for it.
    Latin1,
    /// libxml's built-in ASCII decoder: input stops at the first byte
    /// above 0x7F, without a diagnostic (recorded).
    Ascii,
    /// Anything else: iconv's business in PHP.
    Other,
}

pub(crate) fn declared(name: &str) -> Declared {
    let upper = name.to_ascii_uppercase();
    match upper.as_str() {
        "UTF-8" | "UTF8" => Declared::Utf8,
        "UTF-16" | "UTF16" => Declared::Utf16,
        "UTF-16LE" => Declared::Utf16Le,
        "UTF-16BE" => Declared::Utf16Be,
        "ISO-8859-1" | "ISO-LATIN-1" | "ISO LATIN 1" | "ISO_8859-1" | "ISO8859-1" | "LATIN1"
        | "L1" => Declared::Latin1,
        "ASCII" | "US-ASCII" => Declared::Ascii,
        _ => Declared::Other,
    }
}
