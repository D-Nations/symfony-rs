//! Reading schema documents (`xmlSchemaParse`): the supported subset of
//! XML Schema 1.0's component syntax, with libxml's errors for the
//! mistakes it was recorded on, and a refusal (with the line) for
//! everything else.
//!
//! Supported: `schema` (`targetNamespace`, `elementFormDefault`), `import`
//! through the caller's resolver, global and local `element`
//! (`name`/`ref`, `type` or an anonymous type, `minOccurs` 0 or 1,
//! `maxOccurs` 1 or `unbounded`, `unique`/`key`/`keyref`), `attribute`
//! (`name`/`ref`, `type`, `use` optional or required, `default`, `fixed`),
//! `attributeGroup`, `group`, `sequence`, `choice`, `any`, `anyAttribute`,
//! `complexType` (`mixed`, `simpleContent` by `extension`), `simpleType`
//! (`restriction` with `enumeration`, `pattern`, `minInclusive`,
//! `maxInclusive`; `list`; `union`), and `annotation` (ignored).
//!
//! Upstream: none

use std::collections::{HashMap, HashSet};

use super::pattern::Pattern;
use super::{
    AttrDecl, AttrUse, Builtin, ComplexType, Content, ElementDecl, Facets, Idc, IdcKind, Max,
    NameTest, NsConstraint, Particle, Process, QName, Schema, SchemaError, SelectorPath,
    SimpleType, Term, TypeRef, ValueConstraint, Variety, Wildcard, XSD_NS,
};
use crate::dom::{Document, NodeId, NodeType};
use crate::error::{ErrorLevel, LibxmlError, LoadError, Refusal, RefusalReason};
use crate::parser::{LoadOptions, Loaded, load_xml};

/// The name libxml gives a schema read from memory.
const MEMORY: &str = "in_memory_buffer";

struct SchemaDoc {
    doc: Document,
    root: NodeId,
    tns: Option<String>,
    qualified: bool,
    /// The file name errors carry.
    name: String,
}

fn refused(line: u32, what: impl Into<String>) -> SchemaError {
    SchemaError::Refused(Refusal {
        reason: RefusalReason::UnsupportedSchema { what: what.into() },
        line,
        column: 0,
    })
}

fn xsd_children(doc: &Document, n: NodeId) -> Vec<NodeId> {
    doc.children(n)
        .iter()
        .copied()
        .filter(|&c| doc.node_type(c) == NodeType::Element)
        .collect()
}

fn is_xsd(doc: &Document, n: NodeId, local: &str) -> bool {
    doc.namespace_uri(n) == Some(XSD_NS) && doc.local_name(n) == Some(local)
}

fn local(doc: &Document, n: NodeId) -> &str {
    doc.local_name(n).unwrap_or_default()
}

fn attr<'d>(doc: &'d Document, n: NodeId, name: &str) -> Option<&'d str> {
    doc.attributes(n)
        .iter()
        .find(|a| a.namespace_uri.is_none() && a.local_name == name)
        .map(|a| a.value.as_str())
}

/// Phase 1: the main document and its imports.
fn read_docs(
    source: &[u8],
    resolve: &mut dyn FnMut(&str) -> Option<Vec<u8>>,
) -> Result<Vec<SchemaDoc>, SchemaError> {
    let loaded = match load_xml(source, LoadOptions::default()) {
        Ok(l) => l,
        Err(LoadError::Empty) => return Err(refused(0, "an empty schema source")),
        Err(LoadError::Refused(r)) => return Err(SchemaError::Refused(r)),
    };
    let Some(doc) = loaded.document else {
        let mut errors = loaded.errors;
        errors.push(LibxmlError {
            level: ErrorLevel::Error,
            code: 3067,
            message: format!("Failed to parse the XML resource '{MEMORY}'."),
            file: None,
            line: 0,
            column: 0,
        });
        return Err(SchemaError::Invalid(errors));
    };
    let mut docs = Vec::new();
    let first = schema_doc(doc, MEMORY, None)?;
    docs.push(first);
    let mut seen: HashSet<String> = HashSet::new();
    let mut i = 0;
    while i < docs.len() {
        let mut imports = Vec::new();
        {
            let d = &docs[i];
            for c in xsd_children(&d.doc, d.root) {
                if is_xsd(&d.doc, c, "import") {
                    let line = d.doc.line(c);
                    let ns = attr(&d.doc, c, "namespace").map(str::to_owned);
                    let Some(loc) = attr(&d.doc, c, "schemaLocation") else {
                        return Err(refused(line, "an import without schemaLocation"));
                    };
                    imports.push((loc.to_owned(), ns, line));
                }
            }
        }
        for (loc, ns, line) in imports {
            if !seen.insert(loc.clone()) {
                continue;
            }
            let Some(bytes) = resolve(&loc) else {
                return Err(SchemaError::Refused(Refusal {
                    reason: RefusalReason::UnresolvedImport { location: loc },
                    line,
                    column: 0,
                }));
            };
            let doc = match load_xml(&bytes, LoadOptions::default()) {
                Ok(Loaded {
                    document: Some(doc),
                    errors,
                }) if errors.is_empty() => doc,
                _ => {
                    return Err(refused(
                        line,
                        format!("imported schema \"{loc}\" with errors"),
                    ));
                }
            };
            let d = schema_doc(doc, &loc, Some(line))?;
            if d.tns != ns {
                return Err(refused(
                    line,
                    "an import whose namespace is not the document's",
                ));
            }
            docs.push(d);
        }
        i += 1;
    }
    Ok(docs)
}

fn schema_doc(
    doc: Document,
    name: &str,
    import_line: Option<u32>,
) -> Result<SchemaDoc, SchemaError> {
    let root = match doc.document_element() {
        Some(r) if is_xsd(&doc, r, "schema") => r,
        _ => {
            if let Some(line) = import_line {
                return Err(refused(line, "an imported document that is not a schema"));
            }
            return Err(SchemaError::Invalid(vec![LibxmlError {
                level: ErrorLevel::Error,
                code: 1772,
                message: format!("The XML document '{name}' is not a schema document."),
                file: None,
                line: 0,
                column: 0,
            }]));
        }
    };
    let line = doc.line(root);
    for a in doc.attributes(root) {
        if a.namespace_uri.is_some() {
            continue;
        }
        match (a.local_name.as_str(), a.value.as_str()) {
            ("targetNamespace" | "version" | "id", _) => {}
            ("elementFormDefault", "qualified" | "unqualified") => {}
            ("attributeFormDefault", "unqualified") => {}
            (other, _) => return Err(refused(line, format!("schema attribute '{other}'"))),
        }
    }
    let tns = attr(&doc, root, "targetNamespace").map(str::to_owned);
    let qualified = attr(&doc, root, "elementFormDefault") == Some("qualified");
    Ok(SchemaDoc {
        doc,
        root,
        tns,
        qualified,
        name: name.to_owned(),
    })
}

type Src = (usize, NodeId);

struct B<'a> {
    docs: &'a [SchemaDoc],
    s: Schema,
    errors: Vec<LibxmlError>,
    type_nodes: HashMap<QName, Src>,
    built_types: HashMap<QName, TypeRef>,
    group_nodes: HashMap<QName, Src>,
    attr_group_nodes: HashMap<QName, Src>,
    attr_groups: HashMap<QName, (Vec<AttrUse>, Option<Wildcard>)>,
    attr_groups_busy: HashSet<QName>,
    global_attr_nodes: HashMap<QName, Src>,
    idc_names: HashMap<QName, usize>,
    /// Keyrefs and the name they refer to, resolved at the end.
    refers: Vec<(usize, QName, u32)>,
}

type R<T> = Result<T, SchemaError>;

impl B<'_> {
    fn doc(&self, d: usize) -> &Document {
        &self.docs[d].doc
    }

    fn line(&self, src: Src) -> u32 {
        self.doc(src.0).line(src.1)
    }

    fn error(&mut self, src: Src, code: u32, message: String) {
        self.errors.push(LibxmlError {
            level: ErrorLevel::Error,
            code,
            message,
            file: Some(self.docs[src.0].name.clone()),
            line: self.line(src),
            column: 0,
        });
    }

    fn no(&self, src: Src, what: impl Into<String>) -> SchemaError {
        refused(self.line(src), what)
    }

    /// Resolves a `QName` written in the schema at `src`.
    fn qname(&self, src: Src, value: &str) -> R<QName> {
        let value = value.trim();
        let (prefix, local) = match value.split_once(':') {
            Some((p, l)) => (Some(p), l),
            None => (None, value),
        };
        match self.doc(src.0).lookup_namespace_uri(src.1, prefix) {
            Some(ns) => Ok(QName::new(Some(ns), local)),
            None if prefix.is_none() => Ok(QName::new(None, local)),
            None => Err(self.no(src, format!("undeclared prefix in QName '{value}'"))),
        }
    }

    fn tns(&self, d: usize) -> Option<&str> {
        self.docs[d].tns.as_deref()
    }

    /// Checks that `src` has only the listed attributes (plus foreign ones).
    fn allow(&self, src: Src, names: &[&str]) -> R<()> {
        for a in self.doc(src.0).attributes(src.1) {
            if a.namespace_uri.is_none()
                && a.local_name != "id"
                && !names.contains(&a.local_name.as_str())
            {
                let el = local(self.doc(src.0), src.1);
                return Err(self.no(src, format!("attribute '{}' on <{el}>", a.local_name)));
            }
        }
        Ok(())
    }

    /// The XSD children of `src`, without annotations.
    fn kids(&self, src: Src) -> R<Vec<NodeId>> {
        let doc = self.doc(src.0);
        let mut out = Vec::new();
        for c in xsd_children(doc, src.1) {
            if doc.namespace_uri(c) != Some(XSD_NS) {
                return Err(self.no((src.0, c), "a non-schema element in a schema component"));
            }
            if local(doc, c) != "annotation" {
                out.push(c);
            }
        }
        Ok(out)
    }

    // ------------------------------------------------------------ types

    fn resolve_type(&mut self, src: Src, q: &QName) -> R<Option<TypeRef>> {
        if q.ns.as_deref() == Some(XSD_NS) {
            if q.local == "anyType" {
                return Ok(Some(TypeRef::Complex(self.s.any_type())));
            }
            return match Builtin::ALL.iter().position(|b| b.name() == q.local) {
                Some(i) => Ok(Some(TypeRef::Simple(i))),
                None => Err(self.no(src, format!("built-in type xs:{}", q.local))),
            };
        }
        if let Some(t) = self.built_types.get(q) {
            return Ok(Some(*t));
        }
        let Some(&node) = self.type_nodes.get(q) else {
            return Ok(None);
        };
        let t = if local(self.doc(node.0), node.1) == "simpleType" {
            TypeRef::Simple(self.simple_type(node, Some(q.display()))?)
        } else {
            // Registered before its content, for recursive models.
            let id = self.s.complex.len();
            self.s.complex.push(ComplexType {
                content: Content::Empty,
                attr_uses: Vec::new(),
                wildcard: None,
            });
            self.built_types.insert(q.clone(), TypeRef::Complex(id));
            let ct = self.complex_type(node)?;
            self.s.complex[id] = ct;
            TypeRef::Complex(id)
        };
        self.built_types.insert(q.clone(), t);
        Ok(Some(t))
    }

    fn simple_ref(&mut self, src: Src, value: &str) -> R<usize> {
        let q = self.qname(src, value)?;
        match self.resolve_type(src, &q)? {
            Some(TypeRef::Simple(s)) => Ok(s),
            Some(TypeRef::Complex(_)) => {
                Err(self.no(src, "a complex type where a simple type is needed"))
            }
            None => Err(self.no(src, format!("unresolved simple type '{value}'"))),
        }
    }

    fn simple_type(&mut self, src: Src, name: Option<String>) -> R<usize> {
        self.allow(src, &["name", "final"])?;
        if attr(self.doc(src.0), src.1, "final").is_some() {
            return Err(self.no(src, "simpleType final"));
        }
        let kids = self.kids(src)?;
        let [k] = kids[..] else {
            return Err(self.no(src, "a simpleType without exactly one derivation"));
        };
        let ks = (src.0, k);
        let doc = self.doc(src.0);
        match local(doc, k) {
            "restriction" => {
                self.allow(ks, &["base"])?;
                let mut kids = self.kids(ks)?;
                let base = if let Some(b) = attr(self.doc(src.0), k, "base") {
                    let b = b.to_owned();
                    self.simple_ref(ks, &b)?
                } else {
                    if kids
                        .first()
                        .is_none_or(|&c| local(self.doc(src.0), c) != "simpleType")
                    {
                        return Err(self.no(ks, "a restriction without a base"));
                    }
                    let inner = kids.remove(0);
                    self.simple_type((src.0, inner), None)?
                };
                let base_t = self.s.simple[base].clone();
                let Variety::Atomic { primitive } = base_t.variety else {
                    return Err(self.no(ks, "a restriction of a list or union type"));
                };
                let mut f = Facets::default();
                for c in kids {
                    let cs = (src.0, c);
                    self.allow(cs, &["value", "fixed"])?;
                    let Some(value) = attr(self.doc(src.0), c, "value").map(str::to_owned) else {
                        return Err(self.no(cs, "a facet without a value"));
                    };
                    match local(self.doc(src.0), c) {
                        "enumeration" => f.enumeration.push(match primitive {
                            Builtin::String | Builtin::AnySimpleType | Builtin::DateTime => value,
                            _ => value.split_whitespace().collect::<Vec<_>>().join(" "),
                        }),
                        "pattern" => match Pattern::compile(&value) {
                            Ok(p) => f.patterns.push(p),
                            Err(why) => {
                                return Err(self.no(cs, format!("pattern '{value}': {why}")));
                            }
                        },
                        n @ ("minInclusive" | "maxInclusive") => {
                            if primitive != Builtin::PositiveInteger {
                                return Err(self.no(cs, format!("{n} on a non-integer type")));
                            }
                            if n == "minInclusive" {
                                f.min_inclusive = Some(value);
                            } else {
                                f.max_inclusive = Some(value);
                            }
                        }
                        other => return Err(self.no(cs, format!("facet '{other}'"))),
                    }
                }
                let mut facets = vec![f];
                facets.extend(base_t.facets);
                Ok(self.push_simple(SimpleType {
                    name,
                    variety: Variety::Atomic { primitive },
                    facets,
                }))
            }
            "list" => {
                self.allow(ks, &["itemType"])?;
                let kids = self.kids(ks)?;
                let item = match (attr(self.doc(src.0), k, "itemType"), kids.as_slice()) {
                    (Some(t), []) => {
                        let t = t.to_owned();
                        self.simple_ref(ks, &t)?
                    }
                    (None, [inner]) => self.simple_type((src.0, *inner), None)?,
                    _ => return Err(self.no(ks, "a list without exactly one item type")),
                };
                if matches!(self.s.simple[item].variety, Variety::List { .. }) {
                    return Err(self.no(ks, "a list of lists"));
                }
                Ok(self.push_simple(SimpleType {
                    name,
                    variety: Variety::List { item },
                    facets: Vec::new(),
                }))
            }
            "union" => {
                self.allow(ks, &["memberTypes"])?;
                let mut members = Vec::new();
                if let Some(list) = attr(self.doc(src.0), k, "memberTypes") {
                    let names: Vec<String> = list.split_whitespace().map(str::to_owned).collect();
                    for n in names {
                        members.push(self.simple_ref(ks, &n)?);
                    }
                }
                for c in self.kids(ks)? {
                    if local(self.doc(src.0), c) != "simpleType" {
                        return Err(self.no((src.0, c), "a union member that is not a simpleType"));
                    }
                    members.push(self.simple_type((src.0, c), None)?);
                }
                if members.is_empty() {
                    return Err(self.no(ks, "a union without members"));
                }
                Ok(self.push_simple(SimpleType {
                    name,
                    variety: Variety::Union { members },
                    facets: Vec::new(),
                }))
            }
            other => Err(self.no(ks, format!("simpleType derivation '{other}'"))),
        }
    }

    fn push_simple(&mut self, t: SimpleType) -> usize {
        self.s.simple.push(t);
        self.s.simple.len() - 1
    }

    fn complex_type(&mut self, src: Src) -> R<ComplexType> {
        self.allow(src, &["name", "mixed"])?;
        let doc = self.doc(src.0);
        let mixed = matches!(attr(doc, src.1, "mixed"), Some("true" | "1"));
        let kids = self.kids(src)?;
        let mut rest: &[NodeId] = &kids;
        if let Some(&first) = kids.first() {
            match local(self.doc(src.0), first) {
                "simpleContent" => {
                    let fs = (src.0, first);
                    if kids.len() != 1 {
                        return Err(self.no(fs, "content after simpleContent"));
                    }
                    self.allow(fs, &[])?;
                    let inner = self.kids(fs)?;
                    let [ext] = inner[..] else {
                        return Err(self.no(fs, "simpleContent without exactly one derivation"));
                    };
                    let es = (src.0, ext);
                    if local(self.doc(src.0), ext) != "extension" {
                        return Err(self.no(es, "simpleContent restriction"));
                    }
                    self.allow(es, &["base"])?;
                    let Some(base) = attr(self.doc(src.0), ext, "base").map(str::to_owned) else {
                        return Err(self.no(es, "an extension without a base"));
                    };
                    let base = self.simple_ref(es, &base)?;
                    let ext_kids = self.kids(es)?;
                    let (uses, wildcard) = self.attributes(src.0, &ext_kids)?;
                    return Ok(ComplexType {
                        content: Content::Simple(base),
                        attr_uses: uses,
                        wildcard,
                    });
                }
                "complexContent" => return Err(self.no((src.0, first), "complexContent")),
                "all" => return Err(self.no((src.0, first), "all")),
                _ => {}
            }
        }
        let mut particle = None;
        if let Some(&first) = rest.first()
            && matches!(
                local(self.doc(src.0), first),
                "sequence" | "choice" | "group"
            )
        {
            particle = self.particle((src.0, first))?;
            rest = &rest[1..];
        }
        let (uses, wildcard) = self.attributes(src.0, rest)?;
        let content = match (particle, mixed) {
            (None, false) => Content::Empty,
            (particle, mixed) => Content::Elements { particle, mixed },
        };
        Ok(ComplexType {
            content,
            attr_uses: uses,
            wildcard,
        })
    }

    // ------------------------------------------------------- attributes

    fn attributes(&mut self, d: usize, nodes: &[NodeId]) -> R<(Vec<AttrUse>, Option<Wildcard>)> {
        let mut uses: Vec<AttrUse> = Vec::new();
        let mut wildcard: Option<Wildcard> = None;
        for &n in nodes {
            let src = (d, n);
            match local(self.doc(d), n) {
                "attribute" => {
                    let u = self.attribute_use(src)?;
                    self.add_use(src, &mut uses, u)?;
                }
                "attributeGroup" => {
                    self.allow(src, &["ref"])?;
                    let Some(r) = attr(self.doc(d), n, "ref").map(str::to_owned) else {
                        return Err(self.no(src, "a local attributeGroup without ref"));
                    };
                    let q = self.qname(src, &r)?;
                    let (gu, gw) = self.attribute_group(src, &q)?;
                    for u in gu {
                        self.add_use(src, &mut uses, u)?;
                    }
                    if let Some(w) = gw {
                        wildcard = Some(self.merge_wildcard(src, wildcard, w)?);
                    }
                }
                "anyAttribute" => {
                    let w = self.wildcard(src)?;
                    wildcard = Some(self.merge_wildcard(src, wildcard, w)?);
                }
                other => return Err(self.no(src, format!("<{other}> among attributes"))),
            }
        }
        Ok((uses, wildcard))
    }

    fn add_use(&self, src: Src, uses: &mut Vec<AttrUse>, u: AttrUse) -> R<()> {
        let name = &self.s.attributes[u.decl].name;
        if uses.iter().any(|e| &self.s.attributes[e.decl].name == name) {
            return Err(self.no(src, "a duplicate attribute use"));
        }
        uses.push(u);
        Ok(())
    }

    fn merge_wildcard(&self, src: Src, have: Option<Wildcard>, w: Wildcard) -> R<Wildcard> {
        match have {
            None => Ok(w),
            Some(h) if h == w => Ok(h),
            Some(_) => Err(self.no(src, "two different attribute wildcards")),
        }
    }

    fn constraint(&self, src: Src) -> R<Option<ValueConstraint>> {
        let doc = self.doc(src.0);
        match (attr(doc, src.1, "default"), attr(doc, src.1, "fixed")) {
            (None, None) => Ok(None),
            (Some(v), None) => Ok(Some(ValueConstraint::Default(v.to_owned()))),
            (None, Some(v)) => Ok(Some(ValueConstraint::Fixed(v.to_owned()))),
            (Some(_), Some(_)) => Err(self.no(src, "both default and fixed")),
        }
    }

    fn attribute_use(&mut self, src: Src) -> R<AttrUse> {
        self.allow(src, &["name", "ref", "type", "use", "default", "fixed"])?;
        let doc = self.doc(src.0);
        let required = match attr(doc, src.1, "use") {
            None | Some("optional") => false,
            Some("required") => true,
            Some(other) => return Err(self.no(src, format!("attribute use '{other}'"))),
        };
        let constraint = self.constraint(src)?;
        let decl = if let Some(r) = attr(doc, src.1, "ref").map(str::to_owned) {
            if attr(self.doc(src.0), src.1, "type").is_some() || !self.kids(src)?.is_empty() {
                return Err(self.no(src, "an attribute ref with a type"));
            }
            let q = self.qname(src, &r)?;
            let Some(id) = self.global_attribute(&q)? else {
                return Err(self.no(src, format!("unresolved attribute ref '{r}'")));
            };
            // libxml 2.9.14 does not check a `fixed` value given where a
            // global attribute is referenced (XLIFF 2.0's
            // `<data xml:space="default">` is valid; recorded as
            // "written data space"); only the declaration's own counts.
            return Ok(AttrUse {
                decl: id,
                required,
                constraint: None,
            });
        } else {
            let Some(name) = attr(doc, src.1, "name").map(str::to_owned) else {
                return Err(self.no(src, "an attribute without name or ref"));
            };
            let ty = self.attribute_type(src)?;
            self.s.attributes.push(AttrDecl {
                name: QName::new(None, &name),
                ty,
                constraint: None,
            });
            self.s.attributes.len() - 1
        };
        Ok(AttrUse {
            decl,
            required,
            constraint,
        })
    }

    fn attribute_type(&mut self, src: Src) -> R<usize> {
        let kids = self.kids(src)?;
        match (
            attr(self.doc(src.0), src.1, "type").map(str::to_owned),
            kids.as_slice(),
        ) {
            (Some(t), []) => self.simple_ref(src, &t),
            (None, [inner]) if local(self.doc(src.0), *inner) == "simpleType" => {
                self.simple_type((src.0, *inner), None)
            }
            (None, []) => Ok(0),
            _ => Err(self.no(src, "an attribute with an unsupported type definition")),
        }
    }

    fn global_attribute(&mut self, q: &QName) -> R<Option<usize>> {
        if let Some(&id) = self.s.global_attributes.get(q) {
            return Ok(Some(id));
        }
        let Some(&src) = self.global_attr_nodes.get(q) else {
            return Ok(None);
        };
        self.allow(src, &["name", "type", "default", "fixed"])?;
        let ty = self.attribute_type(src)?;
        let constraint = self.constraint(src)?;
        self.s.attributes.push(AttrDecl {
            name: q.clone(),
            ty,
            constraint,
        });
        let id = self.s.attributes.len() - 1;
        self.s.global_attributes.insert(q.clone(), id);
        Ok(Some(id))
    }

    fn attribute_group(&mut self, src: Src, q: &QName) -> R<(Vec<AttrUse>, Option<Wildcard>)> {
        if let Some(g) = self.attr_groups.get(q) {
            return Ok(g.clone());
        }
        let Some(&node) = self.attr_group_nodes.get(q) else {
            return Err(self.no(src, format!("unresolved attributeGroup '{}'", q.display())));
        };
        if !self.attr_groups_busy.insert(q.clone()) {
            return Err(self.no(src, "a circular attributeGroup"));
        }
        self.allow(node, &["name"])?;
        let kids = self.kids(node)?;
        let g = self.attributes(node.0, &kids)?;
        self.attr_groups.insert(q.clone(), g.clone());
        Ok(g)
    }

    fn wildcard(&self, src: Src) -> R<Wildcard> {
        self.allow(
            src,
            &["namespace", "processContents", "minOccurs", "maxOccurs"],
        )?;
        let doc = self.doc(src.0);
        let tns = self.tns(src.0).map(str::to_owned);
        let ns = match attr(doc, src.1, "namespace").map(str::trim) {
            None | Some("##any") => NsConstraint::Any,
            Some("##other") => NsConstraint::Other(tns),
            Some(list) => NsConstraint::List(
                list.split_whitespace()
                    .map(|t| match t {
                        "##local" => None,
                        "##targetNamespace" => tns.clone(),
                        uri => Some(uri.to_owned()),
                    })
                    .collect(),
            ),
        };
        let process = match attr(doc, src.1, "processContents") {
            None | Some("strict") => Process::Strict,
            Some("lax") => Process::Lax,
            Some("skip") => Process::Skip,
            Some(other) => return Err(self.no(src, format!("processContents '{other}'"))),
        };
        if !self.kids(src)?.is_empty() {
            return Err(self.no(src, "content in a wildcard"));
        }
        Ok(Wildcard { ns, process })
    }

    // -------------------------------------------------------- particles

    fn occurs(&self, src: Src) -> R<(u32, Max)> {
        let doc = self.doc(src.0);
        let min = match attr(doc, src.1, "minOccurs").map(str::trim) {
            None | Some("1") => 1,
            Some("0") => 0,
            Some(v) => return Err(self.no(src, format!("minOccurs=\"{v}\""))),
        };
        let max = match attr(doc, src.1, "maxOccurs").map(str::trim) {
            None | Some("1") => Max::One,
            Some("unbounded") => Max::Unbounded,
            Some(v) => return Err(self.no(src, format!("maxOccurs=\"{v}\""))),
        };
        Ok((min, max))
    }

    fn particle(&mut self, src: Src) -> R<Option<Particle>> {
        let (min, max) = self.occurs(src)?;
        let doc = self.doc(src.0);
        let term = match local(doc, src.1) {
            "element" => Term::Element(self.local_element(src)?),
            "any" => Term::Any(self.wildcard(src)?),
            "sequence" | "choice" => {
                self.allow(src, &["minOccurs", "maxOccurs"])?;
                let is_seq = local(self.doc(src.0), src.1) == "sequence";
                let mut parts = Vec::new();
                for c in self.kids(src)? {
                    if let Some(p) = self.particle((src.0, c))? {
                        parts.push(p);
                    }
                }
                if parts.is_empty() {
                    return Ok(None);
                }
                if is_seq {
                    Term::Sequence(parts)
                } else {
                    Term::Choice(parts)
                }
            }
            "group" => {
                self.allow(src, &["ref", "minOccurs", "maxOccurs"])?;
                let Some(r) = attr(doc, src.1, "ref").map(str::to_owned) else {
                    return Err(self.no(src, "a local group without ref"));
                };
                let q = self.qname(src, &r)?;
                let Some(&g) = self.group_nodes.get(&q) else {
                    return Err(self.no(src, format!("unresolved group '{r}'")));
                };
                self.allow(g, &["name"])?;
                let kids = self.kids(g)?;
                let [inner] = kids[..] else {
                    return Err(self.no(g, "a group without exactly one model group"));
                };
                let is = (g.0, inner);
                if !matches!(local(self.doc(g.0), inner), "sequence" | "choice") {
                    return Err(self.no(is, "a group of 'all'"));
                }
                if self.occurs(is)? != (1, Max::One) {
                    return Err(self.no(is, "occurrence bounds on a group's model group"));
                }
                match self.particle(is)? {
                    Some(p) => p.term,
                    None => return Ok(None),
                }
            }
            other => return Err(self.no(src, format!("<{other}> in a content model"))),
        };
        if max == Max::Unbounded
            && let Term::Choice(branches) = &term
            && branches
                .iter()
                .any(|b| !matches!(b.term, Term::Element(_) | Term::Any(_)))
        {
            return Err(self.no(src, "a repeatable choice with a group or sequence branch"));
        }
        Ok(Some(Particle { min, max, term }))
    }

    fn local_element(&mut self, src: Src) -> R<usize> {
        let doc = self.doc(src.0);
        if let Some(r) = attr(doc, src.1, "ref").map(str::to_owned) {
            self.allow(src, &["ref", "minOccurs", "maxOccurs"])?;
            let q = self.qname(src, &r)?;
            return if let Some(&id) = self.s.global_elements.get(&q) {
                Ok(id)
            } else {
                self.error(
                    src,
                    3004,
                    format!(
                        "Element '{{{XSD_NS}}}element', attribute 'ref': The QName value '{r}' does not resolve to a(n) element declaration."
                    ),
                );
                Ok(self.placeholder_element(q))
            };
        }
        self.allow(src, &["name", "type", "minOccurs", "maxOccurs"])?;
        let Some(name) = attr(doc, src.1, "name").map(str::to_owned) else {
            return Err(self.no(src, "an element without name or ref"));
        };
        let ns = if self.docs[src.0].qualified {
            self.tns(src.0).map(str::to_owned)
        } else {
            None
        };
        let q = QName { ns, local: name };
        let id = self.placeholder_element(q);
        self.element_body(src, id, false)?;
        Ok(id)
    }

    fn placeholder_element(&mut self, name: QName) -> usize {
        self.s.elements.push(ElementDecl {
            name,
            ty: TypeRef::Complex(self.s.any_type()),
            idcs: Vec::new(),
        });
        self.s.elements.len() - 1
    }

    /// The type and identity constraints of the element declaration `id`.
    fn element_body(&mut self, src: Src, id: usize, global: bool) -> R<()> {
        let mut kids = self.kids(src)?;
        let doc = self.doc(src.0);
        let ty = if let Some(t) = attr(doc, src.1, "type").map(str::to_owned) {
            let q = self.qname(src, &t)?;
            match self.resolve_type(src, &q)? {
                Some(t) => t,
                None if global => {
                    let name = self.s.elements[id].name.display();
                    self.error(
                        src,
                        3004,
                        format!(
                            "element decl. '{name}', attribute 'type': The QName value '{t}' does not resolve to a(n) type definition."
                        ),
                    );
                    TypeRef::Complex(self.s.any_type())
                }
                None => return Err(self.no(src, format!("unresolved type '{t}'"))),
            }
        } else if let Some(&first) = kids.first()
            && matches!(local(self.doc(src.0), first), "complexType" | "simpleType")
        {
            kids.remove(0);
            if local(self.doc(src.0), first) == "complexType" {
                let ct = self.complex_type((src.0, first))?;
                self.s.complex.push(ct);
                TypeRef::Complex(self.s.complex.len() - 1)
            } else {
                TypeRef::Simple(self.simple_type((src.0, first), None)?)
            }
        } else {
            TypeRef::Complex(self.s.any_type())
        };
        self.s.elements[id].ty = ty;
        for c in kids {
            let cs = (src.0, c);
            let kind = match local(self.doc(src.0), c) {
                "unique" => IdcKind::Unique,
                "key" => IdcKind::Key,
                "keyref" => IdcKind::KeyRef,
                other => return Err(self.no(cs, format!("<{other}> in an element declaration"))),
            };
            let idc = self.identity(cs, kind)?;
            self.s.elements[id].idcs.push(idc);
        }
        Ok(())
    }

    // --------------------------------------------------------- identity

    fn identity(&mut self, src: Src, kind: IdcKind) -> R<usize> {
        self.allow(src, &["name", "refer"])?;
        let doc = self.doc(src.0);
        let Some(name) = attr(doc, src.1, "name").map(str::to_owned) else {
            return Err(self.no(src, "an identity constraint without a name"));
        };
        let q = QName::new(self.tns(src.0), &name);
        let kids = self.kids(src)?;
        let Some((&sel, fields)) = kids.split_first() else {
            return Err(self.no(src, "an identity constraint without a selector"));
        };
        if local(self.doc(src.0), sel) != "selector" || fields.is_empty() {
            return Err(self.no(src, "an identity constraint without selector and fields"));
        }
        let ss = (src.0, sel);
        self.allow(ss, &["xpath"])?;
        let xpath = attr(self.doc(src.0), sel, "xpath")
            .unwrap_or_default()
            .to_owned();
        let mut selector = Vec::new();
        for branch in xpath.split('|') {
            selector.push(self.selector_path(ss, branch.trim())?);
        }
        let mut field_names = Vec::new();
        for &f in fields {
            let fs = (src.0, f);
            if local(self.doc(src.0), f) != "field" {
                return Err(self.no(fs, "an identity constraint child that is not a field"));
            }
            self.allow(fs, &["xpath"])?;
            let x = attr(self.doc(src.0), f, "xpath")
                .unwrap_or_default()
                .trim()
                .to_owned();
            let Some(a) = x.strip_prefix('@') else {
                return Err(self.no(fs, format!("field xpath '{x}'")));
            };
            field_names.push(self.xpath_name(fs, a)?);
        }
        let id = self.s.idcs.len();
        if self.idc_names.insert(q.clone(), id).is_some() {
            return Err(self.no(src, "a duplicate identity-constraint name"));
        }
        if kind == IdcKind::KeyRef {
            let Some(r) = attr(self.doc(src.0), src.1, "refer").map(str::to_owned) else {
                return Err(self.no(src, "a keyref without refer"));
            };
            let rq = self.qname(src, &r)?;
            self.refers.push((id, rq, self.line(src)));
        }
        self.s.idcs.push(Idc {
            name: q,
            kind,
            selector,
            fields: field_names,
            refer: None,
        });
        Ok(id)
    }

    /// A name test in an XPath: unprefixed names have no namespace.
    fn xpath_name(&self, src: Src, s: &str) -> R<QName> {
        if s.is_empty() || s.contains(['*', '[', '(', '@', ' ']) {
            return Err(self.no(src, format!("xpath name test '{s}'")));
        }
        match s.split_once(':') {
            None => Ok(QName::new(None, s)),
            Some((p, l)) => match self.doc(src.0).lookup_namespace_uri(src.1, Some(p)) {
                Some(ns) => Ok(QName::new(Some(ns), l)),
                None => Err(self.no(src, format!("undeclared prefix in xpath '{s}'"))),
            },
        }
    }

    fn selector_path(&self, src: Src, s: &str) -> R<SelectorPath> {
        if s == "." {
            return Ok(SelectorPath {
                descendant: false,
                steps: Vec::new(),
            });
        }
        let (descendant, rest) = if let Some(r) = s.strip_prefix(".//") {
            (true, r)
        } else if let Some(r) = s.strip_prefix("./") {
            (false, r)
        } else {
            (false, s)
        };
        let mut steps = Vec::new();
        for step in rest.split('/') {
            let step = step.trim();
            if step == "*" {
                steps.push(NameTest::Any);
            } else {
                steps.push(NameTest::Name(self.xpath_name(src, step)?));
            }
        }
        Ok(SelectorPath { descendant, steps })
    }
}

/// Reads `source` as a schema.
pub(crate) fn parse(
    source: &[u8],
    resolve: &mut dyn FnMut(&str) -> Option<Vec<u8>>,
) -> Result<Schema, SchemaError> {
    let docs = read_docs(source, resolve)?;
    let mut b = B {
        docs: &docs,
        s: Schema::with_builtins(),
        errors: Vec::new(),
        type_nodes: HashMap::new(),
        built_types: HashMap::new(),
        group_nodes: HashMap::new(),
        attr_group_nodes: HashMap::new(),
        attr_groups: HashMap::new(),
        attr_groups_busy: HashSet::new(),
        global_attr_nodes: HashMap::new(),
        idc_names: HashMap::new(),
        refers: Vec::new(),
    };
    // Registration of the global components, in document order.
    let mut globals: Vec<(Src, usize)> = Vec::new();
    for (d, sd) in docs.iter().enumerate() {
        let mut past_imports = false;
        for c in xsd_children(&sd.doc, sd.root) {
            let src = (d, c);
            let ns = sd.doc.namespace_uri(c);
            let name = local(&sd.doc, c).to_owned();
            if ns != Some(XSD_NS) {
                return Err(b.no(src, "a non-schema element at the top level"));
            }
            let comp_name = attr(&sd.doc, c, "name").map(|n| QName::new(sd.tns.as_deref(), n));
            match name.as_str() {
                "annotation" => continue,
                "import" if !past_imports => continue,
                "include" | "redefine" => return Err(b.no(src, format!("<{name}>"))),
                "notation" => return Err(b.no(src, "<notation>")),
                "element" | "attribute" | "simpleType" | "complexType" | "group"
                | "attributeGroup" => {
                    past_imports = true;
                    let Some(q) = comp_name else {
                        return Err(b.no(src, format!("a global <{name}> without a name")));
                    };
                    match name.as_str() {
                        "element" => {
                            if b.s.global_elements.contains_key(&q) {
                                b.error(
                                    src,
                                    1761,
                                    format!(
                                        "Element '{{{XSD_NS}}}element': A global element declaration '{}' does already exist.",
                                        q.display()
                                    ),
                                );
                                continue;
                            }
                            let id = b.placeholder_element(q.clone());
                            b.s.global_elements.insert(q, id);
                            globals.push((src, id));
                        }
                        "attribute" => {
                            if b.global_attr_nodes.insert(q, src).is_some() {
                                return Err(b.no(src, "a duplicate global attribute"));
                            }
                        }
                        "simpleType" | "complexType" => {
                            if b.type_nodes.insert(q, src).is_some() {
                                return Err(b.no(src, "a duplicate global type"));
                            }
                        }
                        "group" => {
                            if b.group_nodes.insert(q, src).is_some() {
                                return Err(b.no(src, "a duplicate group"));
                            }
                        }
                        _ => {
                            if b.attr_group_nodes.insert(q, src).is_some() {
                                return Err(b.no(src, "a duplicate attributeGroup"));
                            }
                        }
                    }
                }
                _ => {
                    b.error(
                        src,
                        3033,
                        format!(
                            "Element '{{{XSD_NS}}}schema': The content is not valid. Expected is ((include | import | redefine | annotation)*, (((simpleType | complexType | group | attributeGroup) | element | attribute | notation), annotation*)*)."
                        ),
                    );
                    return Err(SchemaError::Invalid(b.errors));
                }
            }
        }
    }
    for (src, id) in globals {
        b.allow(src, &["name", "type"])?;
        b.element_body(src, id, true)?;
    }
    // Every named component is checked, used or not, as libxml does.
    let names: Vec<QName> = b.type_nodes.keys().cloned().collect();
    for q in names {
        let src = b.type_nodes[&q];
        b.resolve_type(src, &q)?;
    }
    let names: Vec<QName> = b.global_attr_nodes.keys().cloned().collect();
    for q in names {
        b.global_attribute(&q)?;
    }
    let refers = std::mem::take(&mut b.refers);
    for (id, rq, line) in refers {
        match b.idc_names.get(&rq) {
            Some(&r) if b.s.idcs[r].kind != IdcKind::KeyRef => b.s.idcs[id].refer = Some(r),
            _ => return Err(refused(line, format!("keyref refer '{}'", rq.display()))),
        }
    }
    if !b.errors.is_empty() {
        return Err(SchemaError::Invalid(b.errors));
    }
    Ok(b.s)
}

impl Schema {
    /// The built-in types (indexed as [`Builtin::ALL`]) and `xs:anyType`.
    fn with_builtins() -> Schema {
        let mut s = Schema {
            elements: Vec::new(),
            global_elements: HashMap::new(),
            attributes: Vec::new(),
            global_attributes: HashMap::new(),
            simple: Vec::new(),
            complex: Vec::new(),
            idcs: Vec::new(),
        };
        let nmtoken = Builtin::ALL
            .iter()
            .position(|&b| b == Builtin::NmToken)
            .unwrap_or_default();
        for b in Builtin::ALL {
            s.simple.push(SimpleType {
                name: Some(format!("xs:{}", b.name())),
                variety: if b == Builtin::NmTokens {
                    Variety::List { item: nmtoken }
                } else {
                    Variety::Atomic { primitive: b }
                },
                facets: Vec::new(),
            });
        }
        let any = Wildcard {
            ns: NsConstraint::Any,
            process: Process::Lax,
        };
        s.complex.push(ComplexType {
            content: Content::Elements {
                particle: Some(Particle {
                    min: 0,
                    max: Max::Unbounded,
                    term: Term::Any(any.clone()),
                }),
                mixed: true,
            },
            attr_uses: Vec::new(),
            wildcard: Some(any),
        });
        s
    }

    /// `xs:anyType`.
    pub(crate) fn any_type(&self) -> usize {
        0
    }
}
