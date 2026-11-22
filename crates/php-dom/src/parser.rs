//! `DOMDocument::loadXML()` as PHP 8.4.27 runs it on libxml 2.9.14: the
//! document, its prolog, elements, attributes, character data, references,
//! comments, processing instructions and CDATA sections, with the errors
//! libxml reports, in its order and at its positions (line, and column in
//! characters), including its recovery after an error: which errors stop
//! the parse, which ones continue, and where.
//!
//! Written from the recorded answers in tests/data/parse.json and the XML
//! 1.0 and Namespaces specifications; no libxml or PHP source is used.
//! The document type declaration is in dtd.rs.
//!
//! Upstream: none

use std::collections::HashMap;

use crate::chars::{
    is_ascii_name_char, is_ascii_name_start, is_blank, is_char, is_name_char, is_name_start,
};
use crate::dom::{Attribute, Document, Element, Kind, NodeId};
use crate::encoding::{self, Declared, Detected};
use crate::error::{ErrorLevel, LibxmlError, LoadError, Refusal, RefusalReason};
use crate::uri;

pub(crate) const XML_NS: &str = "http://www.w3.org/XML/1998/namespace";
const XMLNS_NS: &str = "http://www.w3.org/2000/xmlns/";
/// libxml's `xmlParserMaxDepth`.
const MAX_DEPTH: usize = 256;
/// `XML_MAX_NAME_LENGTH`.
const MAX_NAME_LENGTH: usize = 50_000;
/// `XML_MAX_TEXT_LENGTH`.
pub(crate) const MAX_TEXT_LENGTH: usize = 10_000_000;

/// The `DOMDocument` settings a load uses.
#[derive(Clone, Copy, Debug, Default)]
pub struct LoadOptions {
    /// `$dom->validateOnParse = true` (Symfony's XmlUtils sets it, with
    /// `LIBXML_NONET | LIBXML_COMPACT`, which change nothing observable
    /// here: nothing is ever fetched).
    pub validate_on_parse: bool,
}

/// What `loadXML()` leaves behind: the document when it returned `true`,
/// and `libxml_get_errors()` in either case.
#[derive(Clone, Debug)]
pub struct Loaded {
    pub document: Option<Document>,
    pub errors: Vec<LibxmlError>,
}

/// `$dom->loadXML($source)`.
pub fn load_xml(source: &[u8], options: LoadOptions) -> Result<Loaded, LoadError> {
    if source.is_empty() {
        return Err(LoadError::Empty);
    }
    let mut p = Parser::new(source, options)?;
    p.parse_document();
    if let Some(r) = p.refusal {
        return Err(LoadError::Refused(r));
    }
    let document = p.well_formed.then_some(p.doc);
    Ok(Loaded {
        document,
        errors: p.errors,
    })
}

pub(crate) struct Input {
    pub(crate) buf: Vec<u8>,
    pub(crate) pos: usize,
    pub(crate) end: usize,
    pub(crate) line: u32,
    pub(crate) col: u32,
}

impl Input {
    pub(crate) fn new(mut buf: Vec<u8>) -> Self {
        // A NUL byte ends the input, as it does for libxml.
        if let Some(nul) = buf.iter().position(|&b| b == 0) {
            buf.truncate(nul);
        }
        let end = buf.len();
        Input {
            buf,
            pos: 0,
            end,
            line: 1,
            col: 1,
        }
    }
}

struct Open {
    /// The local name, as libxml keeps it for its messages.
    local: String,
    prefix: Option<String>,
    line: u32,
    ns_count: usize,
    node: Option<NodeId>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum EntityKind {
    InternalGeneral,
    ExternalGeneral,
    Unparsed,
    InternalParameter,
    ExternalParameter,
}

pub(crate) struct Entity {
    pub(crate) kind: EntityKind,
    /// The replacement text of an internal entity.
    pub(crate) content: Vec<u8>,
    /// Parsed once on first reference (libxml's `checked`).
    pub(crate) checked: bool,
    /// Expanded value for attribute use, once computed.
    pub(crate) expanded: Option<Result<String, ()>>,
}

pub(crate) struct Parser {
    pub(crate) input: Input,
    /// Inputs suspended by a parameter-entity reference.
    pub(crate) input_stack: Vec<Input>,
    /// libxml's switch to ISO-8859-1 after a UTF-8 error.
    latin1: bool,
    pub(crate) errors: Vec<LibxmlError>,
    pub(crate) well_formed: bool,
    pub(crate) halted: bool,
    validate: bool,
    /// `validateOnParse` as set (libxml then loads external resources).
    validate_on_parse: bool,
    pub(crate) doc: Document,
    parents: Vec<NodeId>,
    names: Vec<Open>,
    ns: Vec<(Option<String>, String)>,
    /// Whether the tree is built (not inside an entity's first parse).
    build: bool,
    pub(crate) in_subset: bool,
    pub(crate) has_doctype: bool,
    /// The DOCTYPE declared something (no "no DTD found" then).
    pub(crate) dtd_has_decls: bool,
    pub(crate) has_external_subset: bool,
    pub(crate) has_pe_refs: bool,
    pub(crate) standalone: i8,
    pub(crate) entities: HashMap<String, Entity>,
    pub(crate) pentities: HashMap<String, Entity>,
    /// Attributes declared with a type other than CDATA: (element, attribute).
    pub(crate) special_atts: HashMap<(String, String), ()>,
    pub(crate) depth: u32,
    in_attribute_value: bool,
    pub(crate) refusal: Option<Refusal>,
    /// Characters of expanded entity text produced so far (a bound).
    expansion_budget: usize,
    version: Option<String>,
    encoding_name: Option<String>,
    /// The input has a decoder already (UTF-16 detected).
    has_decoder: bool,
}

/// One of `xmlFatalErr`'s fixed messages, by code.
pub(crate) fn fatal_message(code: u32) -> &'static str {
    match code {
        1 => "internal error",
        4 => "Document is empty",
        5 => "Extra content at the end of the document",
        6 => "CharRef: invalid hexadecimal value",
        7 => "CharRef: invalid decimal value",
        8 => "CharRef: invalid value",
        23 => "EntityRef: expecting ';'",
        24 => "PEReference: no name",
        25 => "PEReference: expecting ';'",
        33 => "String not started expecting ' or \"",
        34 => "String not closed expecting \" or '",
        36 => "EntityValue: \" or ' expected",
        37 => "EntityValue: \" or ' expected",
        38 => "Unescaped '<' not allowed in attributes values",
        39 => "AttValue: \" or ' expected",
        43 => "SystemLiteral \" or ' expected",
        44 => "Unfinished System or Public ID \" or ' expected",
        46 => "xmlParsePI : no target name",
        48 => "NOTATION: Name expected here",
        49 => "'>' required to close NOTATION declaration",
        52 => "MixedContentDecl : '|' or ')*' expected",
        54 => "ContentDecl : Name or '(' expected",
        55 => "ContentDecl : ',' '|' or ')' expected",
        57 => "parsing XML declaration: '?>' expected",
        69 => "MixedContentDecl : '#PCDATA' expected",
        60 => "Content error in the external subset",
        61 => "DOCTYPE improperly terminated",
        62 => "Sequence ']]>' not allowed in content",
        64 => "Invalid PI name",
        70 => "SYSTEM or PUBLIC, the URI is missing",
        71 => "PUBLIC, the Public Identifier is missing",
        73 => "expected '>'",
        74 => "EndTag: '</' not found",
        75 => "expected '='",
        78 => "standalone accepts only 'yes' or 'no'",
        79 => "Invalid XML encoding name",
        80 => "Comment must not contain '--' (double-hyphen)",
        84 => "Entity value required",
        85 => "chunk is not well balanced",
        86 => "extra content at the end of well balanced chunk",
        88 => "PEReferences forbidden in internal subset",
        89 => "Detected an entity reference loop",
        92 => "Fragment not allowed",
        96 => "Malformed declaration expecting version",
        110 => "Name too long",
        _ => "Unregistered error message",
    }
}

impl Parser {
    fn new(source: &[u8], options: LoadOptions) -> Result<Self, LoadError> {
        let refuse = |reason| {
            Err(LoadError::Refused(Refusal {
                reason,
                line: 1,
                column: 1,
            }))
        };
        let (buf, has_decoder) = match encoding::detect(source) {
            Detected::Utf8 { bom } => (source[bom..].to_vec(), false),
            Detected::Utf16 { little_endian, bom } => {
                match encoding::utf16_to_utf8(&source[bom..], little_endian) {
                    Ok(b) => (b, true),
                    Err(offset) => {
                        return refuse(RefusalReason::InvalidUtf16 {
                            offset: offset + bom,
                        });
                    }
                }
            }
            Detected::Other(name) => return refuse(RefusalReason::Encoding { name: name.into() }),
        };
        Ok(Parser {
            input: Input::new(buf),
            input_stack: Vec::new(),
            latin1: false,
            errors: Vec::new(),
            well_formed: true,
            halted: false,
            validate: options.validate_on_parse,
            validate_on_parse: options.validate_on_parse,
            doc: Document::new(),
            parents: Vec::new(),
            names: Vec::new(),
            ns: Vec::new(),
            build: true,
            in_subset: false,
            has_doctype: false,
            dtd_has_decls: false,
            has_external_subset: false,
            has_pe_refs: false,
            standalone: -1,
            entities: HashMap::new(),
            pentities: HashMap::new(),
            special_atts: HashMap::new(),
            depth: 0,
            in_attribute_value: false,
            refusal: None,
            expansion_budget: 0,
            version: None,
            encoding_name: None,
            has_decoder,
        })
    }

    // ---- diagnostics ----------------------------------------------------

    pub(crate) fn report(&mut self, level: ErrorLevel, code: u32, message: &str) {
        if self.halted {
            return;
        }
        // Inside a parameter entity, libxml reports the position of the
        // input that referred to it.
        let (line, column) = match self.input_stack.last() {
            Some(below) => (below.line, below.col),
            None => (self.input.line, self.input.col),
        };
        self.errors.push(LibxmlError {
            level,
            code,
            message: message.trim().to_owned(),
            file: None,
            line,
            column,
        });
    }

    /// `xmlFatalErr` / `xmlFatalErrMsg`: the document is not well-formed.
    pub(crate) fn fatal(&mut self, code: u32, message: &str) {
        self.report(ErrorLevel::Fatal, code, message);
        self.well_formed = false;
    }

    pub(crate) fn fatal_code(&mut self, code: u32) {
        self.fatal(code, fatal_message(code));
    }

    /// `xmlFatalErr` with an extra text: "message: info".
    pub(crate) fn fatal_info(&mut self, code: u32, info: &str) {
        let m = format!("{}: {}", fatal_message(code), info);
        self.fatal(code, &m);
    }

    pub(crate) fn error(&mut self, code: u32, message: &str) {
        self.report(ErrorLevel::Error, code, message);
    }

    pub(crate) fn warning(&mut self, code: u32, message: &str) {
        self.report(ErrorLevel::Warning, code, message);
    }

    pub(crate) fn halt(&mut self) {
        self.halted = true;
    }

    pub(crate) fn refuse(&mut self, reason: RefusalReason) {
        if self.refusal.is_none() {
            self.refusal = Some(Refusal {
                reason,
                line: self.input.line,
                column: self.input.col,
            });
        }
        self.halt();
    }

    pub(crate) fn validate_option(&self) -> bool {
        self.validate_on_parse
    }

    fn sax(&self) -> bool {
        self.well_formed && !self.halted && self.build
    }

    // ---- input ----------------------------------------------------------

    pub(crate) fn byte_at(&self, i: usize) -> u8 {
        if i < self.input.end {
            self.input.buf[i]
        } else {
            0
        }
    }

    /// `RAW` / `CUR`.
    pub(crate) fn raw(&self) -> u8 {
        if self.halted {
            0
        } else {
            self.byte_at(self.input.pos)
        }
    }

    /// `NXT(n)`.
    pub(crate) fn nxt(&self, n: usize) -> u8 {
        if self.halted {
            0
        } else {
            self.byte_at(self.input.pos + n)
        }
    }

    pub(crate) fn cmp(&self, s: &[u8]) -> bool {
        !self.halted
            && self.input.buf[self.input.pos.min(self.input.end)..self.input.end].starts_with(s)
    }

    pub(crate) fn remaining(&self) -> usize {
        self.input.end.saturating_sub(self.input.pos)
    }

    /// `SKIP(n)`: ASCII known to be there.
    pub(crate) fn skip(&mut self, n: usize) {
        self.input.pos = (self.input.pos + n).min(self.input.end.max(self.input.pos));
        self.input.col += n as u32;
    }

    /// `NEXT1`.
    pub(crate) fn next1(&mut self) {
        if self.input.pos < self.input.end {
            self.input.col += 1;
            self.input.pos += 1;
        }
    }

    /// `NEXTL(l)`.
    pub(crate) fn nextl(&mut self, l: usize) {
        if self.byte_at(self.input.pos) == b'\n' {
            self.input.line += 1;
            self.input.col = 1;
        } else {
            self.input.col += 1;
        }
        self.input.pos = (self.input.pos + l).min(self.input.end.max(self.input.pos));
    }

    /// Decodes the UTF-8 sequence at `i` as libxml's current-character
    /// reader does: `Some((value, length))`, or `None` for an error.
    fn decode_strict(&self, i: usize) -> Option<(u32, usize)> {
        let c = self.byte_at(i);
        let b1 = self.byte_at(i + 1);
        if (c & 0x40) == 0 || c == 0xC0 || (b1 & 0xC0) != 0x80 {
            return None;
        }
        if (c & 0xE0) == 0xE0 {
            let b2 = self.byte_at(i + 2);
            if (b2 & 0xC0) != 0x80 {
                return None;
            }
            if (c & 0xF0) == 0xF0 {
                let b3 = self.byte_at(i + 3);
                if (c & 0xF8) != 0xF0 || (b3 & 0xC0) != 0x80 {
                    return None;
                }
                let v = (u32::from(c & 7) << 18)
                    | (u32::from(b1 & 0x3F) << 12)
                    | (u32::from(b2 & 0x3F) << 6)
                    | u32::from(b3 & 0x3F);
                return (v >= 0x10000).then_some((v, 4));
            }
            let v = (u32::from(c & 0xF) << 12) | (u32::from(b1 & 0x3F) << 6) | u32::from(b2 & 0x3F);
            return (v >= 0x800).then_some((v, 3));
        }
        let v = (u32::from(c & 0x1F) << 6) | u32::from(b1 & 0x3F);
        (v >= 0x80).then_some((v, 2))
    }

    fn bytes_message(&self, i: usize) -> String {
        format!(
            "Input is not proper UTF-8, indicate encoding !\nBytes: 0x{:02X} 0x{:02X} 0x{:02X} 0x{:02X}",
            self.byte_at(i),
            self.byte_at(i + 1),
            self.byte_at(i + 2),
            self.byte_at(i + 3)
        )
    }

    /// `CUR_CHAR(l)`: the current character and its length in bytes. A
    /// CR before LF is stepped over; a lone CR reads as LF.
    pub(crate) fn cur_char(&mut self) -> (u32, usize) {
        if self.halted {
            return (0, 0);
        }
        let i = self.input.pos;
        let c = self.byte_at(i);
        if c < 0x80 || self.latin1 {
            if c == 0x0D {
                if self.byte_at(i + 1) == 0x0A {
                    self.input.pos += 1;
                }
                return (0x0A, 1);
            }
            return (u32::from(c), 1);
        }
        match self.decode_strict(i) {
            Some((v, l)) => {
                if !is_char(v) {
                    self.fatal(9, &format!("Char 0x{v:X} out of allowed range"));
                }
                (v, l)
            }
            None => {
                if self.remaining() < 4 {
                    return (0, 0);
                }
                let m = self.bytes_message(i);
                self.fatal(9, &m);
                self.latin1 = true;
                (u32::from(c), 1)
            }
        }
    }

    /// `NEXT` (`xmlNextChar`).
    pub(crate) fn next(&mut self) {
        if self.halted || self.input.pos >= self.input.end {
            return;
        }
        let i = self.input.pos;
        let c = self.byte_at(i);
        if c == b'\n' {
            self.input.line += 1;
            self.input.col = 1;
        } else {
            self.input.col += 1;
        }
        if self.latin1 || c < 0x80 {
            self.input.pos += 1;
            return;
        }
        let b1 = self.byte_at(i + 1);
        let fail = |p: &mut Parser| {
            let m = if p.remaining() < 4 {
                "Input is not proper UTF-8, indicate encoding !".to_owned()
            } else {
                p.bytes_message(i)
            };
            p.fatal(9, &m);
            p.latin1 = true;
            p.input.pos += 1;
        };
        if c == 0xC0 || (b1 & 0xC0) != 0x80 {
            return fail(self);
        }
        if (c & 0xE0) == 0xE0 {
            let b2 = self.byte_at(i + 2);
            if (b2 & 0xC0) != 0x80 {
                return fail(self);
            }
            let v;
            if (c & 0xF0) == 0xF0 {
                let b3 = self.byte_at(i + 3);
                if (c & 0xF8) != 0xF0 || (b3 & 0xC0) != 0x80 {
                    return fail(self);
                }
                v = (u32::from(c & 7) << 18)
                    | (u32::from(b1 & 0x3F) << 12)
                    | (u32::from(b2 & 0x3F) << 6)
                    | u32::from(b3 & 0x3F);
                self.input.pos += 4;
            } else {
                v = (u32::from(c & 0xF) << 12) | (u32::from(b1 & 0x3F) << 6) | u32::from(b2 & 0x3F);
                self.input.pos += 3;
            }
            if (0xD800..0xE000).contains(&v) || (0xFFFE..0x10000).contains(&v) || v >= 0x110000 {
                self.fatal(9, &format!("Char 0x{v:X} out of allowed range"));
            }
        } else {
            self.input.pos += 2;
        }
        self.input.pos = self.input.pos.min(self.input.end);
    }

    /// `SKIP_BLANKS` outside the DTD; the number skipped.
    pub(crate) fn skip_blanks(&mut self) -> usize {
        if self.in_subset {
            return self.skip_blanks_dtd();
        }
        let mut n = 0;
        while !self.halted
            && is_blank(self.byte_at(self.input.pos))
            && self.input.pos < self.input.end
        {
            if self.byte_at(self.input.pos) == b'\n' {
                self.input.line += 1;
                self.input.col = 1;
            } else {
                self.input.col += 1;
            }
            self.input.pos += 1;
            n += 1;
        }
        n
    }

    fn chars_text(chars: &[u32]) -> String {
        chars.iter().filter_map(|&c| char::from_u32(c)).collect()
    }

    fn bytes_text(&self, from: usize, to: usize) -> String {
        let s = &self.input.buf[from.min(to)..to];
        if self.latin1 {
            s.iter().map(|&b| char::from(b)).collect()
        } else {
            String::from_utf8_lossy(s).into_owned()
        }
    }

    // ---- names ----------------------------------------------------------

    /// `xmlParseName`.
    pub(crate) fn parse_name(&mut self) -> Option<String> {
        let start = self.input.pos;
        let b = self.raw();
        if is_ascii_name_start(b) {
            let mut i = start + 1;
            while is_ascii_name_char(self.byte_at(i)) {
                i += 1;
            }
            let next = self.byte_at(i);
            if next > 0 && next < 0x80 {
                let count = i - start;
                if count > MAX_NAME_LENGTH {
                    self.fatal_info(110, "Name");
                    return None;
                }
                let name = self.bytes_text(start, i);
                self.input.pos = i;
                self.input.col += count as u32;
                return Some(name);
            }
        }
        self.parse_name_complex(false)
    }

    /// `xmlParseNCName`.
    fn parse_ncname(&mut self) -> Option<String> {
        let start = self.input.pos;
        let b = self.raw();
        if b.is_ascii_alphabetic() || b == b'_' {
            let mut i = start + 1;
            while {
                let c = self.byte_at(i);
                c.is_ascii_alphanumeric() || matches!(c, b'_' | b'-' | b'.')
            } {
                i += 1;
            }
            let next = self.byte_at(i);
            if next > 0 && next < 0x80 {
                let count = i - start;
                if count > MAX_NAME_LENGTH {
                    self.fatal_info(110, "NCName");
                    return None;
                }
                let name = self.bytes_text(start, i);
                self.input.pos = i;
                self.input.col += count as u32;
                return Some(name);
            }
        }
        self.parse_name_complex(true)
    }

    fn parse_name_complex(&mut self, nc: bool) -> Option<String> {
        let (c, l) = self.cur_char();
        if c == 0x20
            || c == u32::from(b'>')
            || c == u32::from(b'/')
            || !is_name_start(c)
            || (nc && c == u32::from(b':'))
        {
            return None;
        }
        let mut chars = vec![c];
        let mut len = l;
        self.nextl(l);
        loop {
            let (c, l) = self.cur_char();
            if c == 0x20
                || c == u32::from(b'>')
                || c == u32::from(b'/')
                || !is_name_char(c)
                || (nc && c == u32::from(b':'))
                || l == 0
            {
                break;
            }
            len += l;
            if nc && len > MAX_NAME_LENGTH {
                self.fatal_info(110, "NCName");
                return None;
            }
            chars.push(c);
            self.nextl(l);
        }
        if self.halted {
            return None;
        }
        if !nc && len > MAX_NAME_LENGTH {
            self.fatal_info(110, "Name");
            return None;
        }
        Some(Self::chars_text(&chars))
    }

    /// `xmlParseNmtoken`.
    pub(crate) fn parse_nmtoken(&mut self) -> Option<String> {
        let mut chars = Vec::new();
        loop {
            let (c, l) = self.cur_char();
            if l == 0 || !is_name_char(c) {
                break;
            }
            chars.push(c);
            self.nextl(l);
            if chars.len() > MAX_NAME_LENGTH {
                self.fatal_info(110, "NmToken");
                return None;
            }
        }
        (!chars.is_empty()).then(|| Self::chars_text(&chars))
    }

    /// `xmlParseQName`: (prefix, local name).
    fn parse_qname(&mut self) -> Option<(Option<String>, String)> {
        let Some(l) = self.parse_ncname() else {
            if self.raw() == b':'
                && let Some(l) = self.parse_name()
            {
                self.error(202, &format!("Failed to parse QName '{l}'"));
                return Some((None, l));
            }
            return None;
        };
        if self.raw() != b':' {
            return Some((None, l));
        }
        self.next();
        let p = l;
        let Some(l) = self.parse_ncname() else {
            if self.halted {
                return None;
            }
            self.error(202, &format!("Failed to parse QName '{p}:'"));
            let tmp = match self.parse_nmtoken() {
                Some(t) => format!("{p}:{t}"),
                None => {
                    if self.halted {
                        return None;
                    }
                    format!("{p}:")
                }
            };
            return Some((None, tmp));
        };
        if self.raw() == b':' {
            self.error(202, &format!("Failed to parse QName '{p}:{l}:'"));
            self.next();
            return match self.parse_name() {
                Some(t) => Some((Some(p), format!("{l}:{t}"))),
                None => {
                    if self.halted {
                        return None;
                    }
                    Some((Some(p), format!("{l}:")))
                }
            };
        }
        Some((Some(p), l))
    }

    // ---- document -------------------------------------------------------

    /// `xmlParseDocument`.
    fn parse_document(&mut self) {
        if self.raw() == 0 {
            self.fatal_code(4);
            return;
        }
        if self.cmp(b"<?xml") && is_blank(self.nxt(5)) {
            self.parse_xml_decl();
            if self.halted {
                return;
            }
            self.skip_blanks();
        }
        let version = self.version.clone().unwrap_or_else(|| "1.0".into());
        let standalone = self.standalone > 0;
        self.doc
            .set_declaration(version, self.encoding_name.clone(), standalone);
        self.parse_misc();
        if self.cmp(b"<!DOCTYPE") {
            self.in_subset = false;
            self.parse_doctype_decl();
            if self.raw() == b'[' {
                self.parse_internal_subset();
                if self.halted {
                    return;
                }
            }
            if self.has_external_subset && self.well_formed && self.validate_on_parse {
                let system_id = self.external_subset_id();
                self.refuse(RefusalReason::ExternalSubset { system_id });
                return;
            }
            self.in_subset = false;
            self.parse_misc();
        }
        if self.raw() != b'<' {
            self.fatal(4, "Start tag expected, '<' not found");
        } else {
            self.parse_element();
            self.parse_misc();
            if self.raw() != 0 {
                self.fatal_code(5);
            }
            self.halt();
        }
    }

    /// `xmlParseMisc`.
    fn parse_misc(&mut self) {
        while !self.halted {
            self.skip_blanks();
            if self.raw() == b'<' && self.nxt(1) == b'?' {
                self.parse_pi();
            } else if self.cmp(b"<!--") {
                self.parse_comment();
            } else {
                break;
            }
        }
    }

    /// `xmlParseXMLDecl`.
    fn parse_xml_decl(&mut self) {
        self.skip(5);
        self.skip_blanks();
        let version = self.parse_version_info();
        match version {
            None => self.fatal_code(96),
            Some(v) => {
                if v != "1.0" {
                    if v.starts_with("1.") {
                        self.warning(97, &format!("Unsupported version '{v}'"));
                    } else {
                        self.fatal(108, &format!("Unsupported version '{v}'"));
                    }
                }
                self.version = Some(v);
            }
        }
        if !is_blank(self.raw()) {
            if self.raw() == b'?' && self.nxt(1) == b'>' {
                self.skip(2);
                return;
            }
            self.fatal(65, "Blank needed here");
        }
        self.parse_encoding_decl();
        if self.halted {
            return;
        }
        if self.encoding_name.is_some() && !is_blank(self.raw()) {
            if self.raw() == b'?' && self.nxt(1) == b'>' {
                self.skip(2);
                return;
            }
            self.fatal(65, "Blank needed here");
        }
        self.skip_blanks();
        self.standalone = self.parse_sd_decl();
        self.skip_blanks();
        if self.raw() == b'?' && self.nxt(1) == b'>' {
            self.skip(2);
        } else if self.raw() == b'>' {
            self.fatal_code(57);
            self.next();
        } else {
            self.fatal_code(57);
            // MOVETO_ENDTAG: the position moves, the column does not.
            while self.input.pos < self.input.end && self.input.buf[self.input.pos] != b'>' {
                self.input.pos += 1;
            }
            self.next();
        }
    }

    /// A quoted string after `name`, `S? '=' S?` (version, encoding).
    fn parse_version_info(&mut self) -> Option<String> {
        if !self.cmp(b"version") {
            return None;
        }
        self.skip(7);
        self.skip_blanks();
        if self.raw() != b'=' {
            self.fatal_code(75);
            return None;
        }
        self.next();
        self.skip_blanks();
        let q = self.raw();
        if q != b'"' && q != b'\'' {
            self.fatal_code(33);
            return None;
        }
        self.next();
        let v = self.parse_version_num();
        if self.raw() != q {
            self.fatal_code(34);
        } else {
            self.next();
        }
        v
    }

    fn parse_version_num(&mut self) -> Option<String> {
        let mut s = String::new();
        let c = self.raw();
        if !c.is_ascii_digit() {
            return None;
        }
        s.push(char::from(c));
        self.next();
        if self.raw() != b'.' {
            return None;
        }
        s.push('.');
        self.next();
        while self.raw().is_ascii_digit() {
            s.push(char::from(self.raw()));
            self.next();
        }
        Some(s)
    }

    /// `xmlParseEncodingDecl` and the switch it causes.
    fn parse_encoding_decl(&mut self) {
        self.skip_blanks();
        if !self.cmp(b"encoding") {
            return;
        }
        self.skip(8);
        self.skip_blanks();
        if self.raw() != b'=' {
            self.fatal_code(75);
            return;
        }
        self.next();
        self.skip_blanks();
        let q = self.raw();
        if q != b'"' && q != b'\'' {
            self.fatal_code(33);
            return;
        }
        self.next();
        let name = self.parse_enc_name();
        if self.raw() != q {
            self.fatal_code(34);
            return;
        }
        self.next();
        let Some(name) = name else { return };
        match encoding::declared(&name) {
            Declared::Utf16 => {
                if !self.has_decoder {
                    self.fatal(81, "Document labelled UTF-16 but has UTF-8 content");
                }
            }
            Declared::Utf8 => {}
            Declared::Utf16Le | Declared::Utf16Be if self.has_decoder => {}
            Declared::Latin1 if !self.has_decoder => {
                let rest =
                    encoding::latin1_to_utf8(&self.input.buf[self.input.pos..self.input.end]);
                self.replace_rest(rest);
            }
            Declared::Ascii if !self.has_decoder => {
                let pos = self.input.pos;
                let stop = self.input.buf[pos..self.input.end]
                    .iter()
                    .position(|&b| b >= 0x80)
                    .map_or(self.input.end, |n| pos + n);
                self.input.end = stop;
                self.input.buf.truncate(stop);
            }
            _ => {
                self.encoding_name = Some(name.clone());
                self.refuse(RefusalReason::Encoding { name });
                return;
            }
        }
        self.encoding_name = Some(name);
    }

    fn replace_rest(&mut self, rest: Vec<u8>) {
        let pos = self.input.pos;
        self.input.buf.truncate(pos);
        self.input.buf.extend_from_slice(&rest);
        if let Some(nul) = self.input.buf[pos..].iter().position(|&b| b == 0) {
            self.input.buf.truncate(pos + nul);
        }
        self.input.end = self.input.buf.len();
    }

    /// `xmlParseEncName`.
    fn parse_enc_name(&mut self) -> Option<String> {
        let c = self.raw();
        if !c.is_ascii_alphabetic() {
            self.fatal_code(79);
            return None;
        }
        let mut s = String::new();
        s.push(char::from(c));
        self.next();
        while {
            let c = self.raw();
            c.is_ascii_alphanumeric() || matches!(c, b'.' | b'_' | b'-')
        } {
            s.push(char::from(self.raw()));
            self.next();
        }
        Some(s)
    }

    /// `xmlParseSDDecl`.
    fn parse_sd_decl(&mut self) -> i8 {
        let mut standalone = -2;
        self.skip_blanks();
        if !self.cmp(b"standalone") {
            return standalone;
        }
        self.skip(10);
        self.skip_blanks();
        if self.raw() != b'=' {
            self.fatal_code(75);
            return standalone;
        }
        self.next();
        self.skip_blanks();
        let q = self.raw();
        if q != b'"' && q != b'\'' {
            self.fatal_code(33);
            return standalone;
        }
        self.next();
        if self.raw() == b'n' && self.nxt(1) == b'o' {
            standalone = 0;
            self.skip(2);
        } else if self.raw() == b'y' && self.nxt(1) == b'e' && self.nxt(2) == b's' {
            standalone = 1;
            self.skip(3);
        } else {
            self.fatal_code(78);
        }
        if self.raw() != q {
            self.fatal_code(34);
        } else {
            self.next();
        }
        standalone
    }

    // ---- elements -------------------------------------------------------

    fn current_parent(&self) -> NodeId {
        self.parents
            .last()
            .copied()
            .unwrap_or_else(|| self.doc.root())
    }

    /// `xmlParseElement`.
    fn parse_element(&mut self) {
        if self.parse_element_start() != 0 {
            return;
        }
        self.parse_content_internal();
        if self.halted {
            return;
        }
        if self.raw() == 0 {
            self.premature_end();
            return;
        }
        self.parse_element_end();
    }

    fn premature_end(&mut self) {
        if let Some(open) = self.names.last() {
            let m = format!(
                "Premature end of data in tag {} line {}",
                open.local, open.line
            );
            self.fatal(77, &m);
        }
    }

    /// `xmlParseContentInternal`.
    fn parse_content_internal(&mut self) {
        let base = self.names.len();
        while self.raw() != 0 && !self.halted {
            let (pos, line, col) = (self.input.pos, self.input.line, self.input.col);
            if self.raw() == b'<' && self.nxt(1) == b'?' {
                self.parse_pi();
            } else if self.cmp(b"<![CDATA[") {
                self.parse_cdsect();
            } else if self.cmp(b"<!--") {
                self.parse_comment();
            } else if self.raw() == b'<' {
                if self.nxt(1) == b'/' {
                    if self.names.len() <= base {
                        break;
                    }
                    self.parse_element_end();
                } else {
                    self.parse_element_start();
                }
            } else if self.raw() == b'&' {
                self.parse_reference();
            } else {
                self.parse_char_data();
            }
            if (pos, line, col) == (self.input.pos, self.input.line, self.input.col) && !self.halted
            {
                self.fatal_info(1, "detected an error in element content");
                self.halt();
                break;
            }
        }
    }

    /// `xmlParseElementStart`: 0 when content follows, 1 for an empty
    /// element, -1 on failure.
    fn parse_element_start(&mut self) -> i32 {
        if self.names.len() > MAX_DEPTH {
            self.fatal(
                1,
                &format!("Excessive depth in document: {MAX_DEPTH} use XML_PARSE_HUGE option"),
            );
            self.halt();
            return -1;
        }
        let line = self.input.line;
        let ns_before = self.ns.len();
        let Some((prefix, local, node)) = self.parse_start_tag() else {
            self.ns.truncate(ns_before);
            return -1;
        };
        if self.halted {
            return -1;
        }
        self.names.push(Open {
            local: local.clone(),
            prefix,
            line,
            ns_count: ns_before,
            node,
        });
        if let Some(n) = node {
            self.parents.push(n);
        }
        if self.raw() == b'/' && self.nxt(1) == b'>' {
            self.skip(2);
            self.pop_element();
            return 1;
        }
        if self.raw() == b'>' {
            self.next1();
            return 0;
        }
        self.fatal(
            73,
            &format!("Couldn't find end of Start Tag {local} line {line}"),
        );
        self.pop_element();
        -1
    }

    fn pop_element(&mut self) {
        if let Some(open) = self.names.pop() {
            if open.node.is_some() {
                self.parents.pop();
            }
            self.ns.truncate(open.ns_count);
        }
    }

    /// `xmlParseElementEnd` / `xmlParseEndTag2`.
    fn parse_element_end(&mut self) {
        let Some(open) = self.names.last() else {
            return;
        };
        let (local, prefix, line) = (open.local.clone(), open.prefix.clone(), open.line);
        self.skip(2);
        let matched = match &prefix {
            None => self.parse_name_and_compare(&local),
            Some(p) => self.parse_qname_and_compare(&local, p),
        };
        if self.halted {
            return;
        }
        self.skip_blanks();
        if self.raw() != b'>' {
            self.fatal_code(73);
        } else {
            self.next1();
        }
        if let Err(name) = matched {
            let name = name.unwrap_or_else(|| "unparsable".into());
            self.fatal(
                76,
                &format!("Opening and ending tag mismatch: {local} line {line} and {name}"),
            );
        }
        self.pop_element();
    }

    fn parse_name_and_compare(&mut self, other: &str) -> Result<(), Option<String>> {
        let o = other.as_bytes();
        let mut i = self.input.pos;
        let mut k = 0;
        while k < o.len() && self.byte_at(i) != 0 && self.byte_at(i) == o[k] {
            i += 1;
            k += 1;
        }
        if k == o.len() && (self.byte_at(i) == b'>' || is_blank(self.byte_at(i))) {
            self.input.col += (i - self.input.pos) as u32;
            self.input.pos = i;
            return Ok(());
        }
        match self.parse_name() {
            Some(n) if n == other => Ok(()),
            n => Err(n),
        }
    }

    fn parse_qname_and_compare(&mut self, name: &str, prefix: &str) -> Result<(), Option<String>> {
        let mut i = self.input.pos;
        let p = prefix.as_bytes();
        let mut k = 0;
        while k < p.len() && self.byte_at(i) != 0 && self.byte_at(i) == p[k] {
            i += 1;
            k += 1;
        }
        if k == p.len() && self.byte_at(i) == b':' {
            i += 1;
            let n = name.as_bytes();
            let mut k = 0;
            while k < n.len() && self.byte_at(i) != 0 && self.byte_at(i) == n[k] {
                i += 1;
                k += 1;
            }
            if k == n.len() && (self.byte_at(i) == b'>' || is_blank(self.byte_at(i))) {
                self.input.col += (i - self.input.pos) as u32;
                self.input.pos = i;
                return Ok(());
            }
        }
        match self.parse_qname() {
            Some((p2, l)) if l == name && p2.as_deref() == Some(prefix) => Ok(()),
            Some((_, l)) => Err(Some(l)),
            None => Err(None),
        }
    }

    fn lookup_ns(&self, prefix: Option<&str>) -> Option<String> {
        if prefix == Some("xml") {
            return Some(XML_NS.into());
        }
        for (p, uri) in self.ns.iter().rev() {
            if p.as_deref() == prefix {
                if prefix.is_none() && uri.is_empty() {
                    return None;
                }
                return Some(uri.clone());
            }
        }
        None
    }

    /// `xmlParseStartTag2`: the element's prefix, local name and node.
    #[allow(clippy::type_complexity)]
    fn parse_start_tag(&mut self) -> Option<(Option<String>, String, Option<NodeId>)> {
        if self.raw() != b'<' {
            return None;
        }
        self.next1();
        let Some((prefix, local)) = self.parse_qname() else {
            self.fatal(68, "StartTag: invalid element name");
            return None;
        };
        let elem_qname = match &prefix {
            Some(p) => format!("{p}:{local}"),
            None => local.clone(),
        };
        self.skip_blanks();
        // (prefix, local, value)
        let mut atts: Vec<(Option<String>, String, String)> = Vec::new();
        let ns_start = self.ns.len();
        while self.raw() != b'>'
            && !(self.raw() == b'/' && self.nxt(1) == b'>')
            && {
                let b = self.raw();
                b >= 0x20 || matches!(b, 0x09 | 0x0A | 0x0D)
            }
            && !self.halted
        {
            let (pos, line, col) = (self.input.pos, self.input.line, self.input.col);
            let parsed = self.parse_attribute(&elem_qname);
            let got_name = parsed.is_some();
            let mut got_value = false;
            if let Some((aprefix, aname, value)) = parsed
                && let Some(value) = value
            {
                got_value = true;
                if aprefix.is_none() && aname == "xmlns" {
                    self.default_ns_decl(&value, ns_start);
                } else if aprefix.as_deref() == Some("xmlns") {
                    self.prefixed_ns_decl(&aname, &value, ns_start);
                } else {
                    atts.push((aprefix, aname, value));
                }
            }
            if self.halted {
                break;
            }
            if self.raw() == b'>' || (self.raw() == b'/' && self.nxt(1) == b'>') {
                break;
            }
            if self.skip_blanks() == 0 {
                self.fatal(65, "attributes construct error");
                break;
            }
            if (pos, line, col) == (self.input.pos, self.input.line, self.input.col)
                && !got_name
                && !got_value
            {
                self.fatal_info(1, "xmlParseStartTag: problem parsing attributes");
                break;
            }
        }
        // Namespaces of the attributes, and duplicates.
        let mut resolved: Vec<Option<String>> = Vec::with_capacity(atts.len());
        for i in 0..atts.len() {
            let nsname = match &atts[i].0 {
                Some(p) => {
                    let n = self.lookup_ns(Some(p));
                    if n.is_none() {
                        let m = format!(
                            "Namespace prefix {p} for {} on {local} is not defined",
                            atts[i].1
                        );
                        self.error(201, &m);
                    }
                    n
                }
                None => None,
            };
            for j in 0..i {
                if atts[i].1 == atts[j].1 {
                    if atts[i].0 == atts[j].0 {
                        let m = match &atts[i].0 {
                            Some(p) => format!("Attribute {p}:{} redefined", atts[i].1),
                            None => format!("Attribute {} redefined", atts[i].1),
                        };
                        self.fatal(42, &m);
                        break;
                    }
                    if nsname.is_some() && resolved[j] == nsname {
                        let m = format!(
                            "Namespaced Attribute {} in '{}' redefined",
                            atts[i].1,
                            nsname.as_deref().unwrap_or_default()
                        );
                        self.error(203, &m);
                        break;
                    }
                }
            }
            resolved.push(nsname);
        }
        let nsname = self.lookup_ns(prefix.as_deref());
        if let Some(p) = &prefix
            && nsname.is_none()
        {
            self.error(
                201,
                &format!("Namespace prefix {p} on {local} is not defined"),
            );
        }
        // SAX: start of element.
        let mut node = None;
        if self.well_formed && !self.halted {
            if self.validate && self.build {
                if !self.dtd_has_decls {
                    self.error(522, "Validation failed: no DTD found !");
                }
                self.validate = false;
            }
            if self.build {
                let (name, el_local, el_prefix) = match (&prefix, &nsname) {
                    (Some(p), None) => (format!("{p}:{local}"), format!("{p}:{local}"), None),
                    (Some(p), Some(_)) => (format!("{p}:{local}"), local.clone(), Some(p.clone())),
                    (None, _) => (local.clone(), local.clone(), None),
                };
                let attributes = atts
                    .iter()
                    .zip(&resolved)
                    .map(|((ap, an, v), ns)| match (ap, ns) {
                        (Some(p), Some(_)) => Attribute {
                            name: format!("{p}:{an}"),
                            local_name: an.clone(),
                            prefix: Some(p.clone()),
                            namespace_uri: ns.clone(),
                            value: v.clone(),
                        },
                        (Some(p), None) => Attribute {
                            name: format!("{p}:{an}"),
                            local_name: format!("{p}:{an}"),
                            prefix: None,
                            namespace_uri: None,
                            value: v.clone(),
                        },
                        (None, _) => Attribute {
                            name: an.clone(),
                            local_name: an.clone(),
                            prefix: None,
                            namespace_uri: None,
                            value: v.clone(),
                        },
                    })
                    .collect();
                let parent = self.current_parent();
                let el = Element {
                    name,
                    local_name: el_local,
                    prefix: el_prefix,
                    namespace_uri: nsname.clone(),
                    attributes,
                    namespaces: self.ns[ns_start..].to_vec(),
                };
                node = Some(self.doc.append(parent, Kind::Element(el), self.input.line));
            }
        }
        Some((prefix, local, node))
    }

    fn default_ns_decl(&mut self, url: &str, ns_start: usize) {
        if !url.is_empty() {
            match uri::parse(url) {
                None => self.error(99, &format!("xmlns: '{url}' is not a valid URI")),
                Some(u) => {
                    if !u.has_scheme {
                        self.warning(100, &format!("xmlns: URI {url} is not absolute"));
                    }
                }
            }
            if url == XML_NS {
                self.error(200, "xml namespace URI cannot be the default namespace");
                return;
            }
            if url == XMLNS_NS {
                self.error(200, "reuse of the xmlns namespace name is forbidden");
                return;
            }
        }
        if self.ns[ns_start..].iter().any(|(p, _)| p.is_none()) {
            self.fatal(42, "Attribute xmlns redefined");
        } else {
            self.push_ns(None, url);
        }
    }

    fn prefixed_ns_decl(&mut self, name: &str, url: &str, ns_start: usize) {
        if name == "xml" {
            if url != XML_NS {
                self.error(200, "xml namespace prefix mapped to wrong URI");
            }
            return;
        }
        if url == XML_NS {
            self.error(200, "xml namespace URI mapped to wrong prefix");
            return;
        }
        if name == "xmlns" {
            self.error(200, "redefinition of the xmlns prefix is forbidden");
            return;
        }
        if url == XMLNS_NS {
            self.error(200, "reuse of the xmlns namespace name is forbidden");
            return;
        }
        if url.is_empty() {
            self.error(
                200,
                &format!("xmlns:{name}: Empty XML namespace is not allowed"),
            );
            return;
        }
        if uri::parse(url).is_none() {
            self.error(99, &format!("xmlns:{name}: '{url}' is not a valid URI"));
        }
        if self.ns[ns_start..]
            .iter()
            .any(|(p, _)| p.as_deref() == Some(name))
        {
            self.fatal(42, &format!("Attribute xmlns:{name} redefined"));
        } else {
            self.push_ns(Some(name.to_owned()), url);
        }
    }

    fn push_ns(&mut self, prefix: Option<String>, url: &str) {
        if let Some((_, u)) = self.ns.iter().rev().find(|(p, _)| *p == prefix)
            && u == url
        {
            return;
        }
        self.ns.push((prefix, url.to_owned()));
    }

    /// `xmlParseAttribute2`: (prefix, local name, value). `None` when no
    /// name could be read; a `None` value when the value failed.
    #[allow(clippy::type_complexity)]
    fn parse_attribute(&mut self, elem: &str) -> Option<(Option<String>, String, Option<String>)> {
        let Some((prefix, name)) = self.parse_qname() else {
            self.fatal(68, "error parsing attribute name");
            return None;
        };
        let qname = match &prefix {
            Some(p) => format!("{p}:{name}"),
            None => name.clone(),
        };
        let normalize = self.special_atts.contains_key(&(elem.to_owned(), qname));
        self.skip_blanks();
        if self.raw() != b'=' {
            self.fatal(
                41,
                &format!("Specification mandates value for attribute {name}"),
            );
            return None;
        }
        self.next();
        self.skip_blanks();
        let mut value = self.parse_att_value(normalize);
        if normalize && let Some(v) = &mut value {
            // The second pass over spaces that character references added.
            *v = v
                .split(' ')
                .filter(|p| !p.is_empty())
                .collect::<Vec<_>>()
                .join(" ");
        }
        if prefix.as_deref() == Some("xml")
            && name == "space"
            && let Some(v) = &value
            && v != "default"
            && v != "preserve"
        {
            self.warning(
                102,
                &format!(
                    "Invalid value \"{v}\" for xml:space : \"default\" or \"preserve\" expected"
                ),
            );
        }
        Some((prefix, name, value))
    }

    /// `xmlParseAttValueInternal`.
    pub(crate) fn parse_att_value(&mut self, normalize: bool) -> Option<String> {
        let q = self.raw();
        if q != b'"' && q != b'\'' {
            self.fatal_code(39);
            return None;
        }
        // The ASCII fast path, which only reports a length problem.
        let start = self.input.pos + 1;
        let mut i = start;
        while i < self.input.end {
            let c = self.input.buf[i];
            if c == q || !(0x20..=0x7F).contains(&c) || c == b'&' || c == b'<' {
                break;
            }
            i += 1;
        }
        if i - start > MAX_TEXT_LENGTH {
            self.fatal(40, "AttValue length too long");
            return None;
        }
        if !normalize && i < self.input.end && self.input.buf[i] == q {
            let v = self.bytes_text(start, i);
            self.input.col += (i - start + 2) as u32;
            self.input.pos = i + 1;
            return Some(v);
        }
        self.parse_att_value_complex(normalize)
    }

    fn parse_att_value_complex(&mut self, normalize: bool) -> Option<String> {
        let limit = self.raw();
        self.in_attribute_value = true;
        self.next();
        let mut buf = String::new();
        let mut in_space = false;
        let (mut c, mut l) = self.cur_char();
        while self.raw() != limit && is_char(c) && c != u32::from(b'<') && !self.halted {
            if buf.len() > MAX_TEXT_LENGTH {
                self.fatal(40, "AttValue length too long");
                self.in_attribute_value = false;
                return None;
            }
            if c == u32::from(b'&') {
                in_space = false;
                if self.nxt(1) == b'#' {
                    let v = self.parse_char_ref();
                    if let Some(ch) = char::from_u32(v).filter(|_| v != 0) {
                        buf.push(ch);
                    }
                } else if let Some(name) = self.parse_entity_ref()
                    && let Some(text) = self.attribute_entity_text(&name)
                {
                    buf.push_str(&text);
                }
            } else {
                if matches!(c, 0x20 | 0x0D | 0x0A | 0x09) {
                    if !buf.is_empty() || !normalize {
                        if !normalize || !in_space {
                            buf.push(' ');
                        }
                        in_space = true;
                    }
                } else {
                    in_space = false;
                    if let Some(ch) = char::from_u32(c) {
                        buf.push(ch);
                    }
                }
                self.nextl(l);
            }
            (c, l) = self.cur_char();
        }
        self.in_attribute_value = false;
        if self.halted {
            return None;
        }
        if in_space && normalize {
            while buf.ends_with(' ') {
                buf.pop();
            }
        }
        if self.raw() == b'<' {
            self.fatal_code(38);
        } else if self.raw() != limit {
            if c != 0 && !is_char(c) {
                self.fatal(9, "invalid character in attribute value");
            } else {
                self.fatal(40, "AttValue: ' expected");
            }
            return None;
        } else {
            self.next();
        }
        Some(buf)
    }

    // ---- references -----------------------------------------------------

    /// `xmlParseCharRef`: the character, or 0.
    pub(crate) fn parse_char_ref(&mut self) -> u32 {
        let mut val: u32 = 0;
        let mut count = 0;
        if self.raw() == b'&' && self.nxt(1) == b'#' && self.nxt(2) == b'x' {
            self.skip(3);
            while self.raw() != b';' && !self.halted {
                count += 1;
                let c = self.raw();
                if c.is_ascii_digit() {
                    val = val * 16 + u32::from(c - b'0');
                } else if (b'a'..=b'f').contains(&c) && count < 20 {
                    val = val * 16 + u32::from(c - b'a') + 10;
                } else if (b'A'..=b'F').contains(&c) && count < 20 {
                    val = val * 16 + u32::from(c - b'A') + 10;
                } else {
                    self.fatal_code(6);
                    val = 0;
                    break;
                }
                val = val.min(0x110000);
                self.next();
                count += 1;
            }
            if self.raw() == b';' {
                self.next1();
            }
        } else if self.raw() == b'&' && self.nxt(1) == b'#' {
            self.skip(2);
            while self.raw() != b';' && !self.halted {
                let c = self.raw();
                if c.is_ascii_digit() {
                    val = val * 10 + u32::from(c - b'0');
                } else {
                    self.fatal_code(7);
                    val = 0;
                    break;
                }
                val = val.min(0x110000);
                self.next();
            }
            if self.raw() == b';' {
                self.next1();
            }
        } else {
            self.fatal_code(8);
        }
        if val >= 0x110000 {
            self.fatal(9, "xmlParseCharRef: character reference out of bounds");
        } else if is_char(val) {
            return val;
        } else {
            self.fatal(9, &format!("xmlParseCharRef: invalid xmlChar value {val}"));
        }
        0
    }

    /// `xmlParseEntityRef`: the entity's name when it is declared or
    /// predefined (`&lt;` and friends come back as `"lt"` etc.).
    pub(crate) fn parse_entity_ref(&mut self) -> Option<String> {
        if self.raw() != b'&' {
            return None;
        }
        self.next();
        let Some(name) = self.parse_name() else {
            self.fatal(68, "xmlParseEntityRef: no name");
            return None;
        };
        if self.raw() != b';' {
            self.fatal_code(23);
            return None;
        }
        self.next();
        if predefined(&name).is_some() {
            return Some(name);
        }
        if self.halted {
            return None;
        }
        let Some(ent) = self.entities.get(&name) else {
            if self.standalone == 1 || (!self.has_external_subset && !self.has_pe_refs) {
                self.fatal(26, &format!("Entity '{name}' not defined"));
                return None;
            }
            // Only a warning-level error; the reference still becomes a node.
            self.error(27, &format!("Entity '{name}' not defined"));
            return Some(name);
        };
        let (kind, has_lt) = (ent.kind, ent.content.contains(&b'<'));
        if kind == EntityKind::Unparsed {
            self.fatal(28, &format!("Entity reference to unparsed entity {name}"));
        } else if self.in_attribute_value && kind == EntityKind::ExternalGeneral {
            self.fatal(
                29,
                &format!("Attribute references external entity '{name}'"),
            );
        } else if self.in_attribute_value && has_lt {
            self.fatal(
                38,
                &format!("'<' in entity '{name}' is not allowed in attributes values"),
            );
        }
        Some(name)
    }

    /// The text an entity reference stands for in an attribute value.
    fn attribute_entity_text(&mut self, name: &str) -> Option<String> {
        if let Some(c) = predefined(name) {
            return Some(c.to_string());
        }
        let ent = self.entities.get(name)?;
        if ent.kind != EntityKind::InternalGeneral {
            return None;
        }
        if let Some(e) = &ent.expanded {
            return e.clone().ok();
        }
        let content = ent.content.clone();
        if let Some(e) = self.entities.get_mut(name) {
            e.expanded = Some(Err(()));
        }
        self.depth += 1;
        let rep = self.decode_entities(&content);
        self.depth -= 1;
        if let Some(e) = self.entities.get_mut(name) {
            e.expanded = Some(rep.clone().ok_or(()));
        }
        rep
    }

    /// `xmlStringDecodeEntities` with general references substituted:
    /// the replacement text of an entity value used in an attribute.
    fn decode_entities(&mut self, s: &[u8]) -> Option<String> {
        if self.depth > 40 {
            self.fatal_code(89);
            return None;
        }
        let mut out = String::new();
        let text = String::from_utf8_lossy(s).into_owned();
        let b = text.as_bytes();
        let mut i = 0;
        while i < b.len() {
            if b[i] == b'&' && b.get(i + 1) == Some(&b'#') {
                let (v, n) = string_char_ref(&b[i..]);
                match v {
                    Ok(c) => {
                        if let Some(ch) = char::from_u32(c) {
                            out.push(ch);
                        }
                        i += n;
                    }
                    Err((code, msg)) => {
                        self.fatal(code, &msg);
                        return None;
                    }
                }
            } else if b[i] == b'&' {
                let rest = &text[i + 1..];
                let nl = name_len(rest);
                if nl == 0 {
                    self.fatal(68, "xmlParseStringEntityRef: no name");
                    i += 1;
                    continue;
                }
                let name = &rest[..nl];
                if rest.as_bytes().get(nl) != Some(&b';') {
                    self.fatal_code(23);
                    i += 1 + nl;
                    continue;
                }
                i += 2 + nl;
                if let Some(c) = predefined(name) {
                    out.push(c);
                    continue;
                }
                let name = name.to_owned();
                let Some(ent) = self.entities.get(&name) else {
                    if self.standalone == 1 || (!self.has_external_subset && !self.has_pe_refs) {
                        self.fatal(26, &format!("Entity '{name}' not defined"));
                    } else {
                        self.error(27, &format!("Entity '{name}' not defined"));
                    }
                    continue;
                };
                if ent.kind == EntityKind::ExternalGeneral {
                    self.fatal(
                        29,
                        &format!("Attribute references external entity '{name}'"),
                    );
                    continue;
                }
                let (content, has_lt) = (ent.content.clone(), ent.content.contains(&b'<'));
                if has_lt {
                    self.fatal(
                        38,
                        &format!("'<' in entity '{name}' is not allowed in attributes values"),
                    );
                }
                self.depth += 1;
                let rep = self.decode_entities(&content);
                self.depth -= 1;
                let rep = rep?;
                self.expansion_budget += rep.len();
                if self.expansion_budget > MAX_TEXT_LENGTH {
                    self.fatal_code(89);
                    return None;
                }
                out.push_str(&rep);
            } else {
                let ch = text[i..].chars().next().unwrap_or('\u{FFFD}');
                out.push(ch);
                i += ch.len_utf8();
            }
        }
        Some(out)
    }

    /// `xmlParseReference` (content).
    fn parse_reference(&mut self) {
        if self.raw() != b'&' {
            return;
        }
        if self.nxt(1) == b'#' {
            let line = self.input.line;
            let v = self.parse_char_ref();
            if v == 0 {
                return;
            }
            if self.sax()
                && let Some(ch) = char::from_u32(v)
            {
                let parent = self.current_parent();
                self.doc
                    .append_text(parent, ch.encode_utf8(&mut [0; 4]), line);
            }
            return;
        }
        let Some(name) = self.parse_entity_ref() else {
            return;
        };
        if !self.well_formed {
            return;
        }
        if let Some(c) = predefined(&name) {
            if self.sax() {
                let parent = self.current_parent();
                let line = self.input.line;
                self.doc
                    .append_text(parent, c.encode_utf8(&mut [0; 4]), line);
            }
            return;
        }
        let Some(ent) = self.entities.get(&name) else {
            if self.sax() {
                let parent = self.current_parent();
                let line = self.input.line;
                self.doc.append(parent, Kind::EntityRef(name), line);
            }
            return;
        };
        let (kind, checked) = (ent.kind, ent.checked);
        if !checked {
            match kind {
                EntityKind::InternalGeneral => {
                    let content = ent.content.clone();
                    self.depth += 1;
                    let ret = self.parse_balanced_chunk(&content);
                    self.depth -= 1;
                    // Marked after the parse, as libxml does: a reference to
                    // the entity inside its own text recurses until the depth
                    // limit reports the loop.
                    if let Some(e) = self.entities.get_mut(&name) {
                        e.checked = true;
                    }
                    match ret {
                        Chunk::Ok => {}
                        Chunk::Loop => {
                            self.fatal_code(89);
                            return;
                        }
                        Chunk::Failed => {
                            self.fatal(26, &format!("Entity '{name}' failed to parse"));
                            return;
                        }
                    }
                }
                EntityKind::ExternalGeneral if self.validate_on_parse => {
                    self.refuse(RefusalReason::ExternalEntity { name });
                    return;
                }
                _ => {}
            }
        }
        if self.sax() {
            let parent = self.current_parent();
            let line = self.input.line;
            self.doc.append(parent, Kind::EntityRef(name), line);
        }
    }

    /// `xmlParseBalancedChunkMemoryInternal`: an entity's replacement
    /// text parsed as content, in a context of its own (positions start
    /// at line 1, column 1; no tree is built).
    fn parse_balanced_chunk(&mut self, content: &[u8]) -> Chunk {
        if self.depth > 40 {
            return Chunk::Loop;
        }
        let saved_input = std::mem::replace(&mut self.input, Input::new(content.to_vec()));
        let saved_names = std::mem::take(&mut self.names);
        let saved_wf = self.well_formed;
        let saved_build = self.build;
        let saved_validate = self.validate;
        let saved_errors = self.errors.len();
        self.well_formed = true;
        self.build = false;
        self.validate = false;
        self.depth += 1;
        self.parse_content_internal();
        if !self.halted && !self.names.is_empty() {
            self.premature_end();
        }
        if self.raw() == b'<' && self.nxt(1) == b'/' {
            self.fatal_code(85);
        } else if self.raw() != 0 {
            self.fatal_code(86);
        }
        if !self.names.is_empty() {
            self.fatal_code(85);
        }
        self.depth -= 1;
        let loop_seen = self.errors[saved_errors..]
            .last()
            .is_some_and(|e| e.code == 89);
        let ok = self.well_formed;
        self.input = saved_input;
        self.names = saved_names;
        self.well_formed = saved_wf;
        self.build = saved_build;
        self.validate = saved_validate;
        if ok {
            Chunk::Ok
        } else if loop_seen {
            Chunk::Loop
        } else {
            Chunk::Failed
        }
    }

    // ---- character data -------------------------------------------------

    fn characters(&mut self, text: &str) {
        if !text.is_empty() && self.sax() {
            let parent = self.current_parent();
            let line = self.input.line;
            if parent != self.doc.root() {
                self.doc.append_text(parent, text, line);
            }
        }
    }

    /// `xmlParseCharData`: its ASCII fast path, then the general one.
    fn parse_char_data(&mut self) {
        let mut i = self.input.pos;
        loop {
            // get_more_space
            loop {
                while self.byte_at(i) == b' ' {
                    i += 1;
                    self.input.col += 1;
                }
                if self.byte_at(i) == b'\n' {
                    while self.byte_at(i) == b'\n' {
                        self.input.line += 1;
                        self.input.col = 1;
                        i += 1;
                    }
                    continue;
                }
                break;
            }
            if self.byte_at(i) == b'<' {
                self.deliver_to(i);
                return;
            }
            // get_more
            loop {
                while test_char_data(self.byte_at(i)) {
                    i += 1;
                    self.input.col += 1;
                }
                if self.byte_at(i) == b'\n' {
                    while self.byte_at(i) == b'\n' {
                        self.input.line += 1;
                        self.input.col = 1;
                        i += 1;
                    }
                    continue;
                }
                if self.byte_at(i) == b']' {
                    if self.byte_at(i + 1) == b']' && self.byte_at(i + 2) == b'>' {
                        self.fatal_code(62);
                        self.input.pos = i + 1;
                        return;
                    }
                    i += 1;
                    self.input.col += 1;
                    continue;
                }
                break;
            }
            self.deliver_to(i);
            if self.byte_at(i) == b'\n' && self.byte_at(i + 1) == b'\n' {
                // The CR is dropped; the LF starts the next run.
                self.input.pos = i + 1;
                i += 2;
                self.input.line += 1;
                self.input.col = 1;
                if fast_char(self.byte_at(i)) {
                    continue;
                }
                break;
            }
            if self.byte_at(i) == b'<' || self.byte_at(i) == b'&' {
                return;
            }
            i = self.input.pos;
            if !fast_char(self.byte_at(i)) {
                break;
            }
        }
        self.parse_char_data_complex();
    }

    /// Delivers the bytes from the position to `i` as character data.
    fn deliver_to(&mut self, i: usize) {
        if i > self.input.pos {
            let t = self.bytes_text(self.input.pos, i);
            self.characters(&t);
        }
        self.input.pos = i;
    }

    fn parse_char_data_complex(&mut self) {
        let mut text = String::new();
        let (mut c, mut l) = self.cur_char();
        while c != u32::from(b'<') && c != u32::from(b'&') && is_char(c) && l != 0 {
            if c == u32::from(b']') && self.nxt(1) == b']' && self.nxt(2) == b'>' {
                self.fatal_code(62);
            }
            if let Some(ch) = char::from_u32(c) {
                text.push(ch);
            }
            self.nextl(l);
            (c, l) = self.cur_char();
        }
        self.characters(&text);
        if c != 0 && !is_char(c) {
            self.fatal(9, &format!("PCDATA invalid Char value {c}"));
            self.nextl(l);
        }
    }

    // ---- comments, PIs, CDATA ------------------------------------------

    /// `xmlParseComment`: libxml's ASCII fast path, then the general one.
    pub(crate) fn parse_comment(&mut self) {
        if !self.cmp(b"<!--") {
            return;
        }
        self.skip(4);
        let mut buf: Option<Vec<u8>> = None;
        let mut i = self.input.pos;
        if self.byte_at(i) == b'\n' {
            while self.byte_at(i) == b'\n' {
                self.input.line += 1;
                self.input.col = 1;
                i += 1;
            }
        }
        // get_more
        loop {
            while {
                let c = self.byte_at(i);
                (c > b'-' && c <= 0x7F) || (0x20..b'-').contains(&c) || c == 0x09
            } {
                i += 1;
                self.input.col += 1;
            }
            if self.byte_at(i) == b'\n' {
                while self.byte_at(i) == b'\n' {
                    self.input.line += 1;
                    self.input.col = 1;
                    i += 1;
                }
                continue;
            }
            if i > self.input.pos {
                buf.get_or_insert_with(Vec::new)
                    .extend_from_slice(&self.input.buf[self.input.pos..i]);
            }
            self.input.pos = i;
            if self.byte_at(i) == b'\n' && self.byte_at(i + 1) == b'\n' {
                self.input.pos = i + 1;
                i += 2;
                self.input.line += 1;
                self.input.col = 1;
                continue;
            }
            if self.byte_at(i) == b'-' {
                if self.byte_at(i + 1) == b'-' {
                    if self.byte_at(i + 2) == b'>' {
                        self.skip(3);
                        let text = String::from_utf8_lossy(buf.as_deref().unwrap_or_default())
                            .into_owned();
                        self.comment(text);
                        return;
                    }
                    match &buf {
                        Some(b) => {
                            let shown = String::from_utf8_lossy(&b[..b.len().min(50)]).into_owned();
                            self.fatal(80, &format!("Double hyphen within comment: <!--{shown}"));
                        }
                        None => self.fatal(80, "Double hyphen within comment"),
                    }
                    if self.halted {
                        return;
                    }
                    i += 1;
                    self.input.col += 1;
                }
                i += 1;
                self.input.col += 1;
                continue;
            }
            break;
        }
        self.parse_comment_complex(buf.unwrap_or_default());
    }

    fn parse_comment_complex(&mut self, buf: Vec<u8>) {
        let mut text = String::from_utf8_lossy(&buf).into_owned();
        let (q, ql) = self.cur_char();
        if q == 0 || ql == 0 {
            self.fatal(45, "Comment not terminated");
            return;
        }
        if !is_char(q) {
            self.fatal(9, &format!("xmlParseComment: invalid xmlChar value {q}"));
            return;
        }
        self.nextl(ql);
        let (r, rl) = self.cur_char();
        if r == 0 || rl == 0 {
            self.fatal(45, "Comment not terminated");
            return;
        }
        if !is_char(r) {
            self.fatal(9, &format!("xmlParseComment: invalid xmlChar value {r}"));
            return;
        }
        self.nextl(rl);
        let (mut q, mut r) = (q, r);
        let (mut cur, mut l) = self.cur_char();
        if cur == 0 || l == 0 {
            self.fatal(45, "Comment not terminated");
            return;
        }
        while is_char(cur)
            && l != 0
            && (cur != u32::from(b'>') || r != u32::from(b'-') || q != u32::from(b'-'))
        {
            if r == u32::from(b'-') && q == u32::from(b'-') {
                self.fatal_code(80);
            }
            if let Some(ch) = char::from_u32(q) {
                text.push(ch);
            }
            q = r;
            r = cur;
            self.nextl(l);
            (cur, l) = self.cur_char();
        }
        if cur == 0 || l == 0 {
            let shown: String = text.chars().take(50).collect();
            self.fatal(45, &format!("Comment not terminated \n<!--{shown}"));
        } else if !is_char(cur) {
            self.fatal(9, &format!("xmlParseComment: invalid xmlChar value {cur}"));
        } else {
            self.next();
            self.comment(text);
        }
    }

    fn comment(&mut self, text: String) {
        if self.sax() && !self.in_subset {
            let parent = self.current_parent();
            let line = self.input.line;
            self.doc.append(parent, Kind::Comment(text), line);
        }
    }

    /// `xmlParsePI`.
    pub(crate) fn parse_pi(&mut self) {
        if !(self.raw() == b'<' && self.nxt(1) == b'?') {
            return;
        }
        self.skip(2);
        let Some(target) = self.parse_pi_target() else {
            self.fatal_code(46);
            return;
        };
        if self.raw() == b'?' && self.nxt(1) == b'>' {
            self.skip(2);
            self.pi(target, String::new());
            return;
        }
        if self.skip_blanks() == 0 {
            self.fatal(65, &format!("ParsePI: PI {target} space expected"));
        }
        let mut data = String::new();
        let (mut c, mut l) = self.cur_char();
        while is_char(c) && l != 0 && (c != u32::from(b'?') || self.nxt(1) != b'>') {
            if let Some(ch) = char::from_u32(c) {
                data.push(ch);
            }
            self.nextl(l);
            (c, l) = self.cur_char();
        }
        if c != u32::from(b'?') {
            self.fatal(47, &format!("ParsePI: PI {target} never end ..."));
        } else {
            self.skip(2);
            self.pi(target, data);
        }
    }

    fn pi(&mut self, target: String, data: String) {
        if self.sax() && !self.in_subset {
            let parent = self.current_parent();
            let line = self.input.line;
            self.doc.append(parent, Kind::Pi { target, data }, line);
        }
    }

    fn parse_pi_target(&mut self) -> Option<String> {
        let name = self.parse_name()?;
        let b = name.as_bytes();
        if b.len() >= 3
            && b[0].eq_ignore_ascii_case(&b'x')
            && b[1].eq_ignore_ascii_case(&b'm')
            && b[2].eq_ignore_ascii_case(&b'l')
        {
            if name == "xml" {
                self.fatal(
                    64,
                    "XML declaration allowed only at the start of the document",
                );
                return Some(name);
            } else if b.len() == 3 {
                self.fatal_code(64);
                return Some(name);
            }
            if name != "xml-stylesheet" && name != "xml-model" {
                self.warning(64, "xmlParsePITarget: invalid name prefix 'xml'");
            }
        }
        if name.contains(':') {
            self.error(205, &format!("colons are forbidden from PI names '{name}'"));
        }
        Some(name)
    }

    /// `xmlParseCDSect`.
    fn parse_cdsect(&mut self) {
        if !self.cmp(b"<![CDATA[") {
            return;
        }
        self.skip(9);
        let (r, rl) = self.cur_char();
        if !is_char(r) || rl == 0 {
            self.fatal_code(63);
            return;
        }
        self.nextl(rl);
        let (s, sl) = self.cur_char();
        if !is_char(s) || sl == 0 {
            self.fatal_code(63);
            return;
        }
        self.nextl(sl);
        let (mut r, mut s) = (r, s);
        let mut text = String::new();
        let (mut cur, mut l) = self.cur_char();
        while is_char(cur)
            && l != 0
            && (r != u32::from(b']') || s != u32::from(b']') || cur != u32::from(b'>'))
        {
            if let Some(ch) = char::from_u32(r) {
                text.push(ch);
            }
            r = s;
            s = cur;
            self.nextl(l);
            (cur, l) = self.cur_char();
        }
        if cur != u32::from(b'>') {
            let shown: String =
                String::from_utf8_lossy(&text.as_bytes()[..text.len().min(50)]).into_owned();
            self.fatal(63, &format!("CData section not finished\n{shown}"));
            return;
        }
        self.nextl(l);
        if self.sax() {
            let parent = self.current_parent();
            let line = self.input.line;
            let children = self.doc.children(parent);
            if let Some(&last) = children.last()
                && self.doc.node_type(last) == crate::dom::NodeType::CData
            {
                self.doc.append_cdata(last, &text);
            } else {
                self.doc.append(parent, Kind::CData(text), line);
            }
        }
    }
}

pub(crate) enum Chunk {
    Ok,
    Loop,
    Failed,
}

/// The five predefined entities.
pub(crate) fn predefined(name: &str) -> Option<char> {
    match name {
        "lt" => Some('<'),
        "gt" => Some('>'),
        "amp" => Some('&'),
        "apos" => Some('\''),
        "quot" => Some('"'),
        _ => None,
    }
}

/// The bytes that keep libxml's character-data fast path going.
fn fast_char(c: u8) -> bool {
    (0x20..=0x7F).contains(&c) || c == 0x09 || c == 0x0A
}

/// libxml's `test_char_data`: ASCII that character data's fast path
/// takes without a closer look.
fn test_char_data(c: u8) -> bool {
    c == 0x09 || ((0x20..=0x7F).contains(&c) && c != b'<' && c != b'&' && c != b']')
}

/// The length in bytes of the `Name` at the start of `s`.
pub(crate) fn name_len(s: &str) -> usize {
    let mut n = 0;
    for (k, ch) in s.char_indices() {
        let c = u32::from(ch);
        let ok = if k == 0 {
            is_name_start(c)
        } else {
            is_name_char(c)
        };
        if !ok {
            break;
        }
        n = k + ch.len_utf8();
    }
    n
}

/// `xmlParseStringCharRef` on `&#...;` at the start of `b`: the
/// character and the bytes read, or the error.
pub(crate) fn string_char_ref(b: &[u8]) -> (Result<u32, (u32, String)>, usize) {
    let mut val: u32 = 0;
    let mut i;
    if b.get(2) == Some(&b'x') {
        i = 3;
        while i < b.len() && b[i] != b';' {
            let c = b[i];
            let d = match c {
                b'0'..=b'9' => c - b'0',
                b'a'..=b'f' => c - b'a' + 10,
                b'A'..=b'F' => c - b'A' + 10,
                _ => return (Err((6, fatal_message(6).into())), i),
            };
            val = (val * 16 + u32::from(d)).min(0x110000);
            i += 1;
        }
    } else {
        i = 2;
        while i < b.len() && b[i] != b';' {
            let c = b[i];
            if !c.is_ascii_digit() {
                return (Err((7, fatal_message(7).into())), i);
            }
            val = (val * 10 + u32::from(c - b'0')).min(0x110000);
            i += 1;
        }
    }
    if i < b.len() {
        i += 1;
    }
    if val >= 0x110000 {
        return (
            Err((
                9,
                "xmlParseStringCharRef: character reference out of bounds".into(),
            )),
            i,
        );
    }
    if !is_char(val) {
        return (
            Err((
                9,
                format!("xmlParseStringCharRef: invalid xmlChar value {val}"),
            )),
            i,
        );
    }
    (Ok(val), i)
}
