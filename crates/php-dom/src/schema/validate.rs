//! Instance validation (`xmlSchemaValidateDoc`), in libxml's order:
//! for each element its attributes (values as they are read, then the
//! attributes not allowed and fixed-value mismatches in document order,
//! then missing required attributes), its children in document order, the
//! end of its content, then its identity constraints.
//!
//! After the first child the model rejects (or the first element child of
//! simple or empty content), libxml looks at no further children of that
//! element; an unexpected element is not assessed at all.
//!
//! Upstream: none

use std::collections::HashSet;

use super::content::{Cursor, Matched};
use super::types::ValueErrors;
use super::{
    Content, IdcKind, NameTest, Process, QName, Schema, SchemaValidation, TypeRef, ValueConstraint,
    Wildcard, XSI_NS,
};
use crate::dom::{Document, NodeId, NodeType};
use crate::error::{ErrorLevel, LibxmlError, Refusal, RefusalReason};

/// A field value for identity constraints: `None` when absent.
type Key = Option<Vec<String>>;

struct Target {
    scope: usize,
    idc: usize,
    key: Key,
}

struct Scope {
    node: NodeId,
    idcs: Vec<usize>,
    /// Per constraint of `idcs`: the key-sequences seen, in order.
    tables: Vec<Vec<Vec<String>>>,
    /// Keyref entries: (constraint position, key-sequence, node).
    refs: Vec<(usize, Vec<String>, NodeId)>,
}

struct V<'s, 'd> {
    schema: &'s Schema,
    doc: &'d Document,
    file: Option<String>,
    errors: Vec<LibxmlError>,
    ids: HashSet<String>,
    scopes: Vec<Scope>,
    /// Typed attribute values of the element being started, for fields.
    refusal: Option<Refusal>,
}

fn elem_qname(doc: &Document, n: NodeId) -> QName {
    QName::new(doc.namespace_uri(n), doc.local_name(n).unwrap_or_default())
}

fn blank(s: &str) -> bool {
    s.bytes().all(|b| matches!(b, b' ' | b'\t' | b'\n' | b'\r'))
}

pub(crate) fn validate(
    schema: &Schema,
    doc: &Document,
    file: Option<&str>,
) -> Result<SchemaValidation, Refusal> {
    let mut v = V {
        schema,
        doc,
        file: file.map(str::to_owned),
        errors: Vec::new(),
        ids: HashSet::new(),
        scopes: Vec::new(),
        refusal: None,
    };
    if let Some(root) = doc.document_element() {
        let name = elem_qname(doc, root);
        match schema.global_elements.get(&name) {
            Some(&decl) => v.element(root, decl_type(schema, decl), Some(decl)),
            None => v.error(
                root,
                1845,
                "No matching global declaration available for the validation root.",
            ),
        }
    }
    if let Some(r) = v.refusal {
        return Err(r);
    }
    Ok(SchemaValidation {
        valid: v.errors.is_empty(),
        errors: v.errors,
    })
}

fn decl_type(schema: &Schema, decl: usize) -> TypeRef {
    schema.elements[decl].ty
}

enum AttrState {
    /// Assessed against a declaration: its normalized value, if valid,
    /// and the constraint to check.
    Declared(Option<String>, Option<ValueConstraint>),
    NotAllowed(u32),
    StrictMissing,
    Skipped,
}

impl V<'_, '_> {
    fn push(&mut self, node: NodeId, code: u32, message: String) {
        self.errors.push(LibxmlError {
            level: ErrorLevel::Error,
            code,
            message,
            file: self.file.clone(),
            line: self.doc.line(node),
            column: 0,
        });
    }

    fn error(&mut self, node: NodeId, code: u32, msg: &str) {
        let name = elem_qname(self.doc, node).display();
        self.push(node, code, format!("Element '{name}': {msg}"));
    }

    fn attr_error(&mut self, node: NodeId, attr: &QName, code: u32, msg: &str) {
        let name = elem_qname(self.doc, node).display();
        let a = attr.display();
        self.push(
            node,
            code,
            format!("Element '{name}', attribute '{a}': {msg}"),
        );
    }

    fn refuse(&mut self, node: NodeId, what: &str) {
        if self.refusal.is_none() {
            self.refusal = Some(Refusal {
                reason: RefusalReason::UnsupportedInstance { what: what.into() },
                line: self.doc.line(node),
                column: 0,
            });
        }
    }

    /// Validates a value; errors carry `prefix`. `None` when invalid.
    fn value(
        &mut self,
        node: NodeId,
        ty: usize,
        raw: &str,
        attr: Option<&QName>,
    ) -> Option<String> {
        let mut errs: ValueErrors = Vec::new();
        let result = match self.schema.check_value(ty, raw, &mut Some(&mut errs)) {
            Ok(r) => r,
            Err(_) => {
                self.refuse(node, "a non-ASCII digit tested against \\d");
                return None;
            }
        };
        for (code, msg) in errs {
            match attr {
                Some(a) => self.attr_error(node, a, code, &msg),
                None => self.error(node, code, &msg),
            }
        }
        let v = result?;
        if self.schema.is_id_type(ty) && !self.ids.insert(v.clone()) {
            let label = match &self.schema.simple[ty].name {
                Some(n) => format!("the atomic type '{n}'"),
                None => "the local atomic type".into(),
            };
            let msg = format!("'{raw}' is not a valid value of {label}.");
            match attr {
                Some(a) => self.attr_error(node, a, 1824, &msg),
                None => self.error(node, 1824, &msg),
            }
            return None;
        }
        Some(v)
    }

    /// Validates the element `node` against `ty`. `decl` is its element
    /// declaration (for identity constraints), if any.
    fn element(&mut self, node: NodeId, ty: TypeRef, decl: Option<usize>) {
        if self.refusal.is_some() {
            return;
        }
        let doc = self.doc;
        let schema = self.schema;
        // xsi attributes.
        for a in doc.attributes(node) {
            if a.namespace_uri.as_deref() == Some(XSI_NS) {
                match a.local_name.as_str() {
                    "type" => return self.refuse(node, "xsi:type"),
                    "nil" => self.error(node, 1847, "The element is not 'nillable'."),
                    _ => {}
                }
            }
        }
        let is_xsi = |a: &crate::dom::Attribute| {
            a.namespace_uri.as_deref() == Some(XSI_NS)
                && matches!(
                    a.local_name.as_str(),
                    "type" | "nil" | "schemaLocation" | "noNamespaceSchemaLocation"
                )
        };
        // Attributes: their typed values, kept for identity fields.
        let mut typed: Vec<(QName, Option<String>, bool)> = Vec::new();
        match ty {
            TypeRef::Simple(_) => {
                for a in doc.attributes(node) {
                    if is_xsi(a) {
                        continue;
                    }
                    let q = QName::new(a.namespace_uri.as_deref(), &a.local_name);
                    self.attr_error(
                        node,
                        &q,
                        1827,
                        &format!("The attribute '{}' is not allowed.", q.display()),
                    );
                }
            }
            TypeRef::Complex(ct) => {
                let ct = &schema.complex[ct];
                let mut states = Vec::new();
                let mut present = vec![false; ct.attr_uses.len()];
                for a in doc.attributes(node) {
                    if is_xsi(a) {
                        continue;
                    }
                    let q = QName::new(a.namespace_uri.as_deref(), &a.local_name);
                    let state = if let Some(u) = ct
                        .attr_uses
                        .iter()
                        .position(|u| schema.attributes[u.decl].name == q)
                    {
                        present[u] = true;
                        let use_ = &ct.attr_uses[u];
                        let d = &schema.attributes[use_.decl];
                        let v = self.value(node, d.ty, &a.value, Some(&q));
                        let c = use_.constraint.clone().or_else(|| d.constraint.clone());
                        typed.push((q.clone(), v.clone(), true));
                        AttrState::Declared(v, c)
                    } else if let Some(w) =
                        ct.wildcard.as_ref().filter(|w| w.allows(q.ns.as_deref()))
                    {
                        match w.process {
                            Process::Skip => {
                                typed.push((q.clone(), Some(a.value.clone()), false));
                                AttrState::Skipped
                            }
                            p => match schema.global_attributes.get(&q) {
                                Some(&g) => {
                                    let d = &schema.attributes[g];
                                    let v = self.value(node, d.ty, &a.value, Some(&q));
                                    typed.push((q.clone(), v.clone(), true));
                                    AttrState::Declared(v, d.constraint.clone())
                                }
                                None if p == Process::Strict => AttrState::StrictMissing,
                                None => {
                                    typed.push((q.clone(), Some(a.value.clone()), false));
                                    AttrState::Skipped
                                }
                            },
                        }
                    } else {
                        AttrState::NotAllowed(if ct.wildcard.is_some() { 1867 } else { 1866 })
                    };
                    states.push((q, state));
                }
                for (q, state) in states {
                    match state {
                        AttrState::NotAllowed(code) => {
                            let m = format!("The attribute '{}' is not allowed.", q.display());
                            self.attr_error(node, &q, code, &m);
                        }
                        AttrState::StrictMissing => self.attr_error(
                            node,
                            &q,
                            1878,
                            "No matching global attribute declaration available, but demanded by the strict wildcard.",
                        ),
                        AttrState::Declared(Some(v), Some(ValueConstraint::Fixed(f))) if v != f => {
                            let m = format!(
                                "The value '{v}' does not match the fixed value constraint '{f}'."
                            );
                            self.attr_error(node, &q, 1874, &m);
                        }
                        _ => {}
                    }
                }
                for (u, use_) in ct.attr_uses.iter().enumerate() {
                    if use_.required && !present[u] {
                        let n = schema.attributes[use_.decl].name.display();
                        self.error(
                            node,
                            1868,
                            &format!("The attribute '{n}' is required but missing."),
                        );
                    }
                }
            }
        }
        // Identity constraints: this element's own scope, then the fields
        // of every constraint that selects it.
        let opened = match decl {
            Some(d) if !schema.elements[d].idcs.is_empty() => {
                let idcs = schema.elements[d].idcs.clone();
                let n = idcs.len();
                self.scopes.push(Scope {
                    node,
                    idcs,
                    tables: vec![Vec::new(); n],
                    refs: Vec::new(),
                });
                true
            }
            _ => false,
        };
        let targets = self.select(node, &typed);
        // Content.
        self.content(node, ty);
        // End of the element: its targets, then (if a scope) its keyrefs.
        self.finish_targets(node, targets);
        if opened {
            self.close_scope();
        }
    }

    fn content(&mut self, node: NodeId, ty: TypeRef) {
        let doc = self.doc;
        let schema = self.schema;
        let (content, simple_type) = match ty {
            TypeRef::Simple(s) => (None, Some(s)),
            TypeRef::Complex(c) => match &schema.complex[c].content {
                Content::Simple(s) => (Some(&schema.complex[c].content), Some(*s)),
                other => (Some(other), None),
            },
        };
        if let Some(st) = simple_type {
            let mut text = String::new();
            for &c in doc.children(node) {
                match doc.node_type(c) {
                    NodeType::Text | NodeType::CData => {
                        text.push_str(doc.node_value(c).unwrap_or_default());
                    }
                    NodeType::Element => {
                        if content.is_none() {
                            self.error(node, 1828, "Element content is not allowed, because the type definition is simple.");
                        } else {
                            self.error(node, 1842, "Element content is not allowed, because the content type is a simple type definition.");
                        }
                        break;
                    }
                    NodeType::EntityReference => return self.refuse(c, "entity reference"),
                    _ => {}
                }
            }
            self.value(node, st, &text, None);
            return;
        }
        match content {
            Some(Content::Empty) => {
                for &c in doc.children(node) {
                    match doc.node_type(c) {
                        NodeType::Text | NodeType::CData => self.error(
                            node,
                            1841,
                            "Character content is not allowed, because the content type is empty.",
                        ),
                        NodeType::Element => {
                            self.error(
                                node,
                                1841,
                                "Element content is not allowed, because the content type is empty.",
                            );
                            break;
                        }
                        NodeType::EntityReference => return self.refuse(c, "entity reference"),
                        _ => {}
                    }
                }
            }
            Some(Content::Elements { particle, mixed }) => {
                let mut cursor = Cursor::new(schema, particle.as_ref());
                for &c in doc.children(node) {
                    if self.refusal.is_some() {
                        return;
                    }
                    match doc.node_type(c) {
                        NodeType::Text | NodeType::CData => {
                            if !*mixed && !blank(doc.node_value(c).unwrap_or_default()) {
                                self.error(node, 1843, "Character content other than whitespace is not allowed because the content type is 'element-only'.");
                            }
                        }
                        NodeType::Element => {
                            let name = elem_qname(doc, c);
                            match cursor.push(&name) {
                                Some(Matched::Element(e)) => {
                                    self.element(c, schema.elements[e].ty, Some(e));
                                }
                                Some(Matched::Any(w)) => self.wildcard_child(c, w),
                                None => {
                                    let list = expected_text(&cursor);
                                    self.error(
                                        c,
                                        1871,
                                        &format!("This element is not expected.{list}"),
                                    );
                                    return;
                                }
                            }
                        }
                        NodeType::EntityReference => return self.refuse(c, "entity reference"),
                        _ => {}
                    }
                }
                if !cursor.can_end() {
                    let list = expected_text(&cursor);
                    self.error(node, 1871, &format!("Missing child element(s).{list}"));
                }
            }
            _ => {}
        }
    }

    fn wildcard_child(&mut self, c: NodeId, w: &Wildcard) {
        if w.process == Process::Skip {
            return;
        }
        let name = elem_qname(self.doc, c);
        match self.schema.global_elements.get(&name) {
            Some(&d) => self.element(c, decl_type(self.schema, d), Some(d)),
            None if w.process == Process::Strict => self.error(
                c,
                1845,
                "No matching global element declaration available, but demanded by the strict wildcard.",
            ),
            None => self.element(c, TypeRef::Complex(self.schema.any_type()), None),
        }
    }

    /// The constraints in scope that select `node`, innermost scope first
    /// and, within a scope, in reverse declaration order, with the
    /// node's key-sequences.
    fn select(&mut self, node: NodeId, typed: &[(QName, Option<String>, bool)]) -> Vec<Target> {
        let mut out = Vec::new();
        for s in (0..self.scopes.len()).rev() {
            let scope_node = self.scopes[s].node;
            let Some(path) = self.path(scope_node, node) else {
                continue;
            };
            for pos in (0..self.scopes[s].idcs.len()).rev() {
                let idc_id = self.scopes[s].idcs[pos];
                let idc = &self.schema.idcs[idc_id];
                let selected = idc.selector.iter().any(|p| {
                    if p.descendant {
                        path.len() >= p.steps.len()
                            && steps_match(&p.steps, &path[path.len() - p.steps.len()..])
                            && (!path.is_empty() || p.steps.is_empty())
                    } else {
                        path.len() == p.steps.len() && steps_match(&p.steps, &path)
                    }
                });
                if !selected {
                    continue;
                }
                let mut key = Some(Vec::new());
                for f in &idc.fields {
                    match typed.iter().find(|(q, _, _)| q == f) {
                        Some((_, Some(v), _)) => {
                            if let Some(k) = &mut key {
                                k.push(v.clone());
                            }
                        }
                        Some((q, None, _)) => {
                            let q = q.clone();
                            self.attr_error(node, &q, 1877, "Warning: No precomputed value available, the value was either invalid or something strange happened.");
                            key = None;
                        }
                        None => key = None,
                    }
                }
                out.push(Target {
                    scope: s,
                    idc: pos,
                    key,
                });
            }
        }
        out
    }

    /// The element names from `scope` (exclusive) down to `node`; `None`
    /// if `node` is not `scope` or below it.
    fn path(&self, scope: NodeId, node: NodeId) -> Option<Vec<QName>> {
        let mut names = Vec::new();
        let mut at = node;
        while at != scope {
            names.push(elem_qname(self.doc, at));
            at = self.doc.parent(at)?;
        }
        names.reverse();
        Some(names)
    }

    fn finish_targets(&mut self, node: NodeId, targets: Vec<Target>) {
        for t in targets {
            let idc_id = self.scopes[t.scope].idcs[t.idc];
            let idc = &self.schema.idcs[idc_id];
            let name = idc.name.display();
            match (idc.kind, t.key) {
                (IdcKind::Key, None) => self.error(
                    node,
                    1877,
                    &format!(
                        "Not all fields of key identity-constraint '{name}' evaluate to a node."
                    ),
                ),
                (IdcKind::Unique | IdcKind::KeyRef, None) => {}
                (IdcKind::KeyRef, Some(k)) => self.scopes[t.scope].refs.push((t.idc, k, node)),
                (kind, Some(k)) => {
                    if self.scopes[t.scope].tables[t.idc].contains(&k) {
                        let seq = key_text(&k);
                        self.error(
                            node,
                            1877,
                            &format!(
                                "Duplicate key-sequence {seq} in {} identity-constraint '{name}'.",
                                kind.word()
                            ),
                        );
                    } else {
                        self.scopes[t.scope].tables[t.idc].push(k);
                    }
                }
            }
        }
    }

    fn close_scope(&mut self) {
        let Some(scope) = self.scopes.pop() else {
            return;
        };
        for (pos, k, node) in &scope.refs {
            let idc = &self.schema.idcs[scope.idcs[*pos]];
            let found = idc
                .refer
                .and_then(|r| scope.idcs.iter().position(|&i| i == r));
            let ok = found.is_some_and(|p| scope.tables[p].contains(k));
            if !ok {
                let seq = key_text(k);
                let name = idc.name.display();
                self.error(
                    *node,
                    1877,
                    &format!("No match found for key-sequence {seq} of keyref '{name}'."),
                );
            }
        }
    }
}

fn steps_match(steps: &[NameTest], names: &[QName]) -> bool {
    steps.iter().zip(names).all(|(s, n)| match s {
        NameTest::Any => true,
        NameTest::Name(q) => q == n,
    })
}

fn key_text(k: &[String]) -> String {
    let parts: Vec<String> = k.iter().map(|v| format!("'{v}'")).collect();
    format!("[{}]", parts.join(", "))
}

fn expected_text(cursor: &Cursor<'_>) -> String {
    let names: Vec<String> = cursor.expected().iter().flat_map(|a| a.display()).collect();
    match names.len() {
        0 => String::new(),
        1 => format!(" Expected is ( {} ).", names[0]),
        _ => format!(" Expected is one of ( {} ).", names.join(", ")),
    }
}
