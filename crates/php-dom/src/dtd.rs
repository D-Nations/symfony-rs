//! The document type declaration as libxml 2.9.14 parses it for PHP's
//! `loadXML()`: the name and external identifier, and the internal subset
//! (entity, element, attribute-list and notation declarations, comments,
//! processing instructions, parameter-entity references), with libxml's
//! diagnostics. Nothing is fetched: an external subset or an external
//! parameter entity that libxml would load is refused (error.rs).
//!
//! Upstream: none

use crate::chars::{is_blank, is_char, is_pubid_char};
use crate::dom::Kind;
use crate::error::RefusalReason;
use crate::parser::{Entity, EntityKind, Input, Parser, string_char_ref};
use crate::uri;

impl Parser {
    /// `SKIP_BLANKS` inside the DTD, where blanks are read with `NEXT`,
    /// the end of a parameter entity returns to the text that referred
    /// to it, and references are expanded once inside one.
    pub(crate) fn skip_blanks_dtd(&mut self) -> usize {
        let mut n = 0;
        loop {
            if self.halted {
                break;
            }
            let c = self.raw();
            if is_blank(c) {
                self.next();
            } else if c == b'%' {
                let expand = !self.input_stack.is_empty();
                if !expand || is_blank(self.nxt(1)) || self.nxt(1) == 0 {
                    break;
                }
                self.parse_pe_reference();
            } else if c == 0 {
                if self.input_stack.is_empty() {
                    break;
                }
                self.pop_input();
            } else {
                break;
            }
            n += 1;
        }
        n
    }

    fn pop_input(&mut self) {
        if let Some(prev) = self.input_stack.pop() {
            self.input = prev;
        }
    }

    pub(crate) fn external_subset_id(&self) -> String {
        self.doc
            .children(self.doc.root())
            .iter()
            .find_map(|&c| self.doc.document_type(c).map(|(_, _, s)| s.to_owned()))
            .unwrap_or_default()
    }

    /// `xmlParseDocTypeDecl`.
    pub(crate) fn parse_doctype_decl(&mut self) {
        self.has_doctype = true;
        self.skip(9);
        self.skip_blanks();
        let name = self.parse_name();
        if name.is_none() {
            self.fatal(68, "xmlParseDocTypeDecl : no DOCTYPE name !");
        }
        self.skip_blanks();
        let (system, public) = self.parse_external_id(true);
        if system.is_some() || public.is_some() {
            self.has_external_subset = true;
        }
        self.skip_blanks();
        if self.well_formed && !self.halted {
            let kind = Kind::DocumentType {
                name: name.unwrap_or_default(),
                public_id: public.unwrap_or_default(),
                system_id: system.unwrap_or_default(),
            };
            let line = self.input.line;
            let root = self.doc.root();
            self.doc.append(root, kind, line);
        }
        if self.halted || self.raw() == b'[' {
            return;
        }
        if self.raw() != b'>' {
            self.fatal_code(61);
        }
        self.next();
    }

    /// `xmlParseExternalID`: (system literal, public literal).
    fn parse_external_id(&mut self, strict: bool) -> (Option<String>, Option<String>) {
        if self.cmp(b"SYSTEM") {
            self.skip(6);
            if self.skip_blanks() == 0 {
                self.fatal(65, "Space required after 'SYSTEM'");
            }
            let uri = self.parse_system_literal();
            if uri.is_none() {
                self.fatal_code(70);
            }
            return (uri, None);
        }
        if self.cmp(b"PUBLIC") {
            self.skip(6);
            if self.skip_blanks() == 0 {
                self.fatal(65, "Space required after 'PUBLIC'");
            }
            let public = self.parse_pubid_literal();
            if public.is_none() {
                self.fatal_code(71);
            }
            if strict {
                if self.skip_blanks() == 0 {
                    self.fatal(65, "Space required after the Public Identifier");
                }
            } else {
                if self.skip_blanks() == 0 {
                    return (None, public);
                }
                if self.raw() != b'\'' && self.raw() != b'"' {
                    return (None, public);
                }
            }
            let uri = self.parse_system_literal();
            if uri.is_none() {
                self.fatal_code(70);
            }
            return (uri, public);
        }
        (None, None)
    }

    fn parse_system_literal(&mut self) -> Option<String> {
        let stop = self.raw();
        if stop != b'"' && stop != b'\'' {
            self.fatal_code(43);
            return None;
        }
        self.next();
        let mut s = String::new();
        let (mut c, mut l) = self.cur_char();
        while is_char(c) && l != 0 && c != u32::from(stop) {
            if let Some(ch) = char::from_u32(c) {
                s.push(ch);
            }
            self.nextl(l);
            (c, l) = self.cur_char();
        }
        if !is_char(c) || l == 0 {
            self.fatal_code(44);
        } else {
            self.next();
        }
        Some(s)
    }

    fn parse_pubid_literal(&mut self) -> Option<String> {
        let stop = self.raw();
        if stop != b'"' && stop != b'\'' {
            self.fatal_code(43);
            return None;
        }
        self.next();
        let mut s = String::new();
        while is_pubid_char(self.raw()) && self.raw() != stop {
            s.push(char::from(self.raw()));
            self.next();
        }
        if self.raw() == stop {
            self.next();
        } else {
            self.fatal_code(44);
        }
        Some(s)
    }

    /// `xmlParseInternalSubset`.
    pub(crate) fn parse_internal_subset(&mut self) {
        self.in_subset = true;
        if self.raw() == b'[' {
            self.next();
            while (self.raw() != b']' || !self.input_stack.is_empty()) && !self.halted {
                let (pos, depth) = (self.input.pos, self.input_stack.len());
                self.skip_blanks();
                self.parse_markup_decl();
                self.parse_pe_reference();
                if self.halted {
                    break;
                }
                if self.input.pos == pos && self.input_stack.len() == depth {
                    if self.raw() == 0 && !self.input_stack.is_empty() {
                        self.pop_input();
                        continue;
                    }
                    self.fatal_info(
                        1,
                        "xmlParseInternalSubset: error detected in Markup declaration",
                    );
                    if self.input_stack.is_empty() {
                        break;
                    }
                    self.pop_input();
                }
            }
            if self.raw() == b']' {
                self.next();
                self.skip_blanks();
            }
        }
        if self.halted {
            return;
        }
        self.in_subset = false;
        if self.raw() != b'>' {
            self.fatal_code(61);
            return;
        }
        self.next();
    }

    /// `xmlParseMarkupDecl`.
    fn parse_markup_decl(&mut self) {
        if self.raw() != b'<' {
            return;
        }
        if self.nxt(1) == b'!' {
            match self.nxt(2) {
                b'E' if self.nxt(3) == b'L' => self.parse_element_decl(),
                b'E' if self.nxt(3) == b'N' => self.parse_entity_decl(),
                b'A' => self.parse_attribute_list_decl(),
                b'N' => self.parse_notation_decl(),
                b'-' => self.parse_comment(),
                _ => {}
            }
        } else if self.nxt(1) == b'?' {
            self.parse_pi();
        }
    }

    /// `xmlParsePEReference`.
    fn parse_pe_reference(&mut self) {
        if self.raw() != b'%' {
            return;
        }
        self.next();
        let Some(name) = self.parse_name() else {
            self.fatal(24, "PEReference: no name");
            return;
        };
        if self.raw() != b';' {
            self.fatal_code(25);
            return;
        }
        self.next();
        match self.pentities.get(&name) {
            None => {
                if self.standalone == 1 || (!self.has_external_subset && !self.has_pe_refs) {
                    self.fatal(26, &format!("PEReference: %{name}; not found"));
                } else {
                    self.warning(27, &format!("PEReference: %{name}; not found"));
                }
            }
            Some(ent) if ent.kind == EntityKind::ExternalParameter => {
                if self.validate_option() {
                    self.refuse(RefusalReason::ExternalParameterEntity { name });
                    return;
                }
            }
            Some(ent) => {
                let content = ent.content.clone();
                if self.input_stack.len() >= 40 {
                    self.fatal_code(89);
                    self.halt();
                    return;
                }
                let input = Input::new(content);
                let prev = std::mem::replace(&mut self.input, input);
                self.input_stack.push(prev);
            }
        }
        self.has_pe_refs = true;
    }

    /// `xmlParseEntityDecl`.
    fn parse_entity_decl(&mut self) {
        if !self.cmp(b"<!ENTITY") {
            return;
        }
        let depth = self.input_stack.len();
        self.skip(8);
        if self.skip_blanks() == 0 {
            self.fatal(65, "Space required after '<!ENTITY'");
        }
        let mut parameter = false;
        if self.raw() == b'%' {
            self.next();
            if self.skip_blanks() == 0 {
                self.fatal(65, "Space required after '%'");
            }
            parameter = true;
        }
        let Some(name) = self.parse_name() else {
            self.fatal(68, "xmlParseEntityDecl: no name");
            return;
        };
        if name.contains(':') {
            self.error(
                205,
                &format!("colons are forbidden from entities names '{name}'"),
            );
        }
        if self.skip_blanks() == 0 {
            self.fatal(65, "Space required after the entity name");
        }
        let mut decl: Option<(EntityKind, Vec<u8>)> = None;
        if self.raw() == b'"' || self.raw() == b'\'' {
            if let Some(value) = self.parse_entity_value() {
                let kind = if parameter {
                    EntityKind::InternalParameter
                } else {
                    EntityKind::InternalGeneral
                };
                decl = Some((kind, value));
            }
        } else {
            let (system, public) = self.parse_external_id(true);
            if system.is_none() && public.is_none() {
                self.fatal_code(84);
            }
            let mut ok = true;
            if let Some(s) = &system {
                match uri::parse(s) {
                    None => self.error(91, &format!("Invalid URI: {s}")),
                    Some(u) => {
                        if u.has_fragment {
                            self.fatal_code(92);
                            ok = !parameter;
                        }
                    }
                }
            }
            if parameter {
                if ok && system.is_some() {
                    decl = Some((EntityKind::ExternalParameter, Vec::new()));
                }
            } else {
                if self.raw() != b'>' && self.skip_blanks() == 0 {
                    self.fatal(65, "Space required before 'NDATA'");
                }
                if self.cmp(b"NDATA") {
                    self.skip(5);
                    if self.skip_blanks() == 0 {
                        self.fatal(65, "Space required after 'NDATA'");
                    }
                    let _ = self.parse_name();
                    decl = Some((EntityKind::Unparsed, Vec::new()));
                } else {
                    decl = Some((EntityKind::ExternalGeneral, Vec::new()));
                }
            }
        }
        if self.halted {
            return;
        }
        if let Some((kind, content)) = decl
            && self.well_formed
        {
            self.dtd_has_decls = true;
            let table = if parameter {
                &mut self.pentities
            } else {
                &mut self.entities
            };
            table.entry(name.clone()).or_insert(Entity {
                kind,
                content,
                checked: false,
                expanded: None,
            });
        }
        self.skip_blanks();
        if self.raw() == b'>' {
            if depth != self.input_stack.len() {
                self.fatal(
                    90,
                    "Entity declaration doesn't start and stop in the same entity",
                );
            }
            self.next();
        } else {
            self.fatal(
                37,
                &format!("xmlParseEntityDecl: entity {name} not terminated"),
            );
            self.halt();
        }
    }

    /// `xmlParseEntityValue`: the replacement text, with character and
    /// parameter-entity references expanded and general ones kept.
    fn parse_entity_value(&mut self) -> Option<Vec<u8>> {
        let stop = self.raw();
        self.next();
        let mut raw = String::new();
        let (mut c, mut l) = self.cur_char();
        while is_char(c) && l != 0 && c != u32::from(stop) && !self.halted {
            if let Some(ch) = char::from_u32(c) {
                raw.push(ch);
            }
            self.nextl(l);
            (c, l) = self.cur_char();
        }
        if self.halted {
            return None;
        }
        if c != u32::from(stop) {
            self.fatal_code(37);
            return None;
        }
        self.next();
        let b = raw.as_bytes();
        let mut i = 0;
        while i < b.len() {
            if b[i] == b'%' || (b[i] == b'&' && b.get(i + 1) != Some(&b'#')) {
                let tmp = b[i];
                let n = crate::parser::name_len(&raw[i + 1..]);
                if n == 0 || b.get(i + 1 + n) != Some(&b';') {
                    self.fatal(
                        87,
                        &format!(
                            "EntityValue: '{}' forbidden except for entities references",
                            char::from(tmp)
                        ),
                    );
                    return None;
                }
                if tmp == b'%' && self.input_stack.is_empty() {
                    self.fatal_code(88);
                    return None;
                }
                i += n + 1;
            }
            i += 1;
        }
        self.decode_pe_refs(&raw)
    }

    /// `xmlStringDecodeEntities` with parameter references only.
    fn decode_pe_refs(&mut self, s: &str) -> Option<Vec<u8>> {
        if self.depth > 40 {
            self.fatal_code(89);
            return None;
        }
        let b = s.as_bytes();
        let mut out = Vec::new();
        let mut i = 0;
        while i < b.len() {
            if b[i] == b'&' && b.get(i + 1) == Some(&b'#') {
                let (v, n) = string_char_ref(&b[i..]);
                match v {
                    Ok(c) => {
                        if let Some(ch) = char::from_u32(c) {
                            out.extend_from_slice(ch.encode_utf8(&mut [0; 4]).as_bytes());
                        }
                        i += n;
                    }
                    Err((code, msg)) => {
                        self.fatal(code, &msg);
                        return None;
                    }
                }
            } else if b[i] == b'%' {
                let n = crate::parser::name_len(&s[i + 1..]);
                let name = s[i + 1..i + 1 + n].to_owned();
                i += n + 2;
                match self.pentities.get(&name) {
                    None => {
                        if self.standalone == 1 || (!self.has_external_subset && !self.has_pe_refs)
                        {
                            self.fatal(26, &format!("PEReference: %{name}; not found"));
                        } else {
                            self.warning(27, &format!("PEReference: %{name}; not found"));
                        }
                    }
                    Some(ent) if ent.kind == EntityKind::ExternalParameter => {
                        self.refuse(RefusalReason::ExternalParameterEntity { name });
                        return None;
                    }
                    Some(ent) => {
                        let content = String::from_utf8_lossy(&ent.content).into_owned();
                        self.depth += 1;
                        let rep = self.decode_pe_refs(&content);
                        self.depth -= 1;
                        out.extend_from_slice(&rep?);
                    }
                }
            } else {
                out.push(b[i]);
                i += 1;
            }
        }
        Some(out)
    }

    /// `xmlParseElementDecl` (the content model is checked, not kept).
    fn parse_element_decl(&mut self) {
        if !self.cmp(b"<!ELEMENT") {
            return;
        }
        self.skip(9);
        if self.skip_blanks() == 0 {
            self.fatal(65, "Space required after 'ELEMENT'");
            return;
        }
        let Some(name) = self.parse_name() else {
            self.fatal(68, "xmlParseElementDecl: no name for Element");
            return;
        };
        if self.skip_blanks() == 0 {
            self.fatal(65, "Space required after the element name");
        }
        let ok = if self.cmp(b"EMPTY") {
            self.skip(5);
            true
        } else if self.cmp(b"ANY") {
            self.skip(3);
            true
        } else if self.raw() == b'(' {
            self.parse_element_content_decl()
        } else {
            if self.raw() == b'%' && self.input_stack.is_empty() {
                self.fatal(
                    21,
                    "PEReference: forbidden within markup decl in internal subset",
                );
            } else {
                self.fatal(54, "xmlParseElementDecl: 'EMPTY', 'ANY' or '(' expected");
            }
            return;
        };
        self.skip_blanks();
        if self.raw() == b'>' {
            self.next();
            if ok && self.well_formed {
                self.dtd_has_decls = true;
            }
        } else {
            self.fatal_code(73);
        }
        let _ = name;
    }

    fn parse_element_content_decl(&mut self) -> bool {
        self.next();
        if self.halted {
            return false;
        }
        self.skip_blanks();
        let ok = if self.cmp(b"#PCDATA") {
            self.parse_mixed_content_decl()
        } else {
            self.parse_children_content_decl(1)
        };
        self.skip_blanks();
        ok
    }

    fn parse_mixed_content_decl(&mut self) -> bool {
        self.skip(7);
        self.skip_blanks();
        if self.raw() == b')' {
            self.next();
            if self.raw() == b'*' {
                self.next();
            }
            return true;
        }
        while self.raw() == b'|' && !self.halted {
            self.next();
            self.skip_blanks();
            if self.parse_name().is_none() {
                self.fatal(68, "xmlParseElementMixedContentDecl : Name expected");
                return false;
            }
            self.skip_blanks();
        }
        if self.raw() == b')' && self.nxt(1) == b'*' {
            self.skip(2);
            true
        } else {
            self.fatal_code(52);
            false
        }
    }

    fn parse_children_content_decl(&mut self, depth: u32) -> bool {
        if depth > 128 {
            self.fatal(55, &format!("xmlParseElementChildrenContentDecl : depth {depth} too deep, use XML_PARSE_HUGE"));
            return false;
        }
        self.skip_blanks();
        if self.raw() == b'(' {
            self.next();
            self.skip_blanks();
            if !self.parse_children_content_decl(depth + 1) {
                return false;
            }
            self.skip_blanks();
        } else {
            if self.parse_name().is_none() {
                self.fatal_code(54);
                return false;
            }
            if matches!(self.raw(), b'?' | b'*' | b'+') {
                self.next();
            }
        }
        self.skip_blanks();
        let mut sep = 0u8;
        while self.raw() != b')' && !self.halted {
            let c = self.raw();
            if c == b',' || c == b'|' {
                if sep == 0 {
                    sep = c;
                } else if sep != c {
                    self.fatal(
                        66,
                        &format!(
                            "xmlParseElementChildrenContentDecl : '{}' expected",
                            char::from(sep)
                        ),
                    );
                    return false;
                }
                self.next();
            } else {
                self.fatal_code(55);
                return false;
            }
            self.skip_blanks();
            if self.raw() == b'(' {
                self.next();
                self.skip_blanks();
                if !self.parse_children_content_decl(depth + 1) {
                    return false;
                }
                self.skip_blanks();
            } else {
                if self.parse_name().is_none() {
                    self.fatal_code(54);
                    return false;
                }
                if matches!(self.raw(), b'?' | b'*' | b'+') {
                    self.next();
                }
            }
            self.skip_blanks();
        }
        self.next();
        if matches!(self.raw(), b'?' | b'*' | b'+') {
            self.next();
        }
        true
    }

    /// `xmlParseAttributeListDecl`.
    fn parse_attribute_list_decl(&mut self) {
        if !self.cmp(b"<!ATTLIST") {
            return;
        }
        self.skip(9);
        if self.skip_blanks() == 0 {
            self.fatal(65, "Space required after '<!ATTLIST'");
        }
        let Some(elem) = self.parse_name() else {
            self.fatal(68, "ATTLIST: no name for Element");
            return;
        };
        self.skip_blanks();
        while self.raw() != b'>' && !self.halted {
            let Some(attr) = self.parse_name() else {
                self.fatal(68, "ATTLIST: no name for Attribute");
                break;
            };
            if self.skip_blanks() == 0 {
                self.fatal(65, "Space required after the attribute name");
                break;
            }
            let Some(cdata) = self.parse_attribute_type() else {
                break;
            };
            if self.skip_blanks() == 0 {
                self.fatal(65, "Space required after the attribute type");
                break;
            }
            if !self.parse_default_decl() {
                break;
            }
            if self.raw() != b'>' && self.skip_blanks() == 0 {
                self.fatal(65, "Space required after the attribute default value");
                break;
            }
            if self.well_formed && !self.halted {
                self.dtd_has_decls = true;
                if !cdata {
                    self.special_atts
                        .entry((elem.clone(), attr.clone()))
                        .or_insert(());
                }
            }
        }
        if self.raw() == b'>' {
            self.next();
        }
    }

    /// `xmlParseAttributeType`: `Some(is CDATA)`, `None` on error.
    fn parse_attribute_type(&mut self) -> Option<bool> {
        for (kw, cdata) in [
            (&b"CDATA"[..], true),
            (b"IDREFS", false),
            (b"IDREF", false),
            (b"ID", false),
            (b"ENTITY", false),
            (b"ENTITIES", false),
            (b"NMTOKENS", false),
            (b"NMTOKEN", false),
        ] {
            if self.cmp(kw) {
                self.skip(kw.len());
                return Some(cdata);
            }
        }
        if self.cmp(b"NOTATION") {
            self.skip(8);
            if self.skip_blanks() == 0 {
                self.fatal(65, "Space required after 'NOTATION'");
                return None;
            }
            if self.raw() != b'(' {
                self.fatal_code(48);
                return None;
            }
            loop {
                self.next();
                self.skip_blanks();
                if self.parse_name().is_none() {
                    self.fatal(68, "Name expected in NOTATION declaration");
                    return None;
                }
                self.skip_blanks();
                if self.raw() != b'|' {
                    break;
                }
            }
            if self.raw() != b')' {
                self.fatal_code(49);
                return None;
            }
            self.next();
            return Some(false);
        }
        if self.raw() != b'(' {
            self.fatal(50, "'(' required to start ATTLIST enumeration");
            return None;
        }
        loop {
            self.next();
            self.skip_blanks();
            if self.parse_nmtoken().is_none() {
                self.fatal(67, "NmToken expected in ATTLIST enumeration");
                return None;
            }
            self.skip_blanks();
            if self.raw() != b'|' {
                break;
            }
        }
        if self.raw() != b')' {
            // The tokens read so far still make a type.
            self.fatal(51, "')' required to finish ATTLIST enumeration");
            return Some(false);
        }
        self.next();
        Some(false)
    }

    /// `xmlParseDefaultDecl`.
    fn parse_default_decl(&mut self) -> bool {
        if self.cmp(b"#REQUIRED") {
            self.skip(9);
            return true;
        }
        if self.cmp(b"#IMPLIED") {
            self.skip(8);
            return true;
        }
        if self.cmp(b"#FIXED") {
            self.skip(6);
            if self.skip_blanks() == 0 {
                self.fatal(65, "Space required after '#FIXED'");
            }
        }
        if self.parse_att_value(false).is_none() {
            // libxml reports it under the code of the error before it.
            let code = self.errors.last().map_or(1, |e| e.code);
            self.fatal(code, "Attribute default value declaration error");
        }
        true
    }

    /// `xmlParseNotationDecl`.
    fn parse_notation_decl(&mut self) {
        if !self.cmp(b"<!NOTATION") {
            return;
        }
        self.skip(10);
        if self.skip_blanks() == 0 {
            self.fatal(65, "Space required after '<!NOTATION'");
            return;
        }
        let Some(name) = self.parse_name() else {
            self.fatal_code(48);
            return;
        };
        if name.contains(':') {
            self.error(
                205,
                &format!("colons are forbidden from notation names '{name}'"),
            );
        }
        if self.skip_blanks() == 0 {
            self.fatal(65, "Space required after the NOTATION name'");
            return;
        }
        let _ = self.parse_external_id(false);
        self.skip_blanks();
        if self.raw() == b'>' {
            self.next();
            if self.well_formed && !self.halted {
                self.dtd_has_decls = true;
            }
        } else {
            self.fatal_code(49);
        }
    }
}
