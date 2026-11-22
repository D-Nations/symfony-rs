//! `DOMDocument::schemaValidateSource()`: XML Schema 1.0 validation of a
//! parsed document, with the diagnostics libxml 2.9.14 reports, for the
//! schema features Symfony's schemas use (inventory in
//! docs/milestones/m1-c-xml-x2.md). Written from PHP's observed behaviour
//! (`scripts/record.sh php-dom-schema`) and the XML Schema 1.0
//! recommendation; no PHP or libxml source is used (PLAN.md decisions 8
//! and 9).
//!
//! A schema construct outside that set is refused when the schema is read
//! (`RefusalReason::UnsupportedSchema`, with its line), never skipped. So is
//! an instance construct the validator does not model (`xsi:type`).
//!
//! Upstream: none

mod content;
mod load;
mod pattern;
mod types;
mod validate;

use std::collections::HashMap;

use crate::dom::Document;
use crate::error::{LibxmlError, Refusal};

pub(crate) const XSD_NS: &str = "http://www.w3.org/2001/XMLSchema";
pub(crate) const XSI_NS: &str = "http://www.w3.org/2001/XMLSchema-instance";

/// What `schemaValidateSource()` leaves behind: its result and
/// `libxml_get_errors()`. When the schema itself is invalid, the errors are
/// the schema's and the document is not looked at.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SchemaValidation {
    pub valid: bool,
    pub errors: Vec<LibxmlError>,
}

/// A parsed schema (`xmlSchema`), ready to validate documents.
#[derive(Debug)]
pub struct Schema {
    pub(crate) elements: Vec<ElementDecl>,
    pub(crate) global_elements: HashMap<QName, usize>,
    pub(crate) attributes: Vec<AttrDecl>,
    pub(crate) global_attributes: HashMap<QName, usize>,
    pub(crate) simple: Vec<SimpleType>,
    pub(crate) complex: Vec<ComplexType>,
    pub(crate) idcs: Vec<Idc>,
}

/// How a schema source failed to become a [`Schema`].
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum SchemaError {
    /// libxml rejects the schema: these are its errors, and
    /// `schemaValidateSource()` returns `false`.
    Invalid(Vec<LibxmlError>),
    /// The schema uses something this crate does not implement, or imports
    /// a document the resolver did not supply.
    Refused(Refusal),
}

impl Schema {
    /// Reads a schema as `schemaValidateSource($source)` does.
    /// `resolve` receives each `<xs:import>`'s `schemaLocation` as written
    /// and returns that document's bytes; nothing is read from a file or
    /// the network here. `None` refuses the schema.
    pub fn parse(
        source: &[u8],
        resolve: &mut dyn FnMut(&str) -> Option<Vec<u8>>,
    ) -> Result<Schema, SchemaError> {
        load::parse(source, resolve)
    }

    /// Validates `document` (its errors name `document_file`, the
    /// document's URL: PHP's working directory plus `/` for a document
    /// loaded from a string).
    pub fn validate(
        &self,
        document: &Document,
        document_file: Option<&str>,
    ) -> Result<SchemaValidation, Refusal> {
        validate::validate(self, document, document_file)
    }
}

/// `$document->schemaValidateSource($source)`, reading the schema and
/// validating in one call, as PHP does.
pub fn schema_validate_source(
    document: &Document,
    source: &[u8],
    resolve: &mut dyn FnMut(&str) -> Option<Vec<u8>>,
    document_file: Option<&str>,
) -> Result<SchemaValidation, Refusal> {
    match Schema::parse(source, resolve) {
        Ok(schema) => schema.validate(document, document_file),
        Err(SchemaError::Invalid(errors)) => Ok(SchemaValidation {
            valid: false,
            errors,
        }),
        Err(SchemaError::Refused(r)) => Err(r),
    }
}

/// An expanded name.
#[derive(Clone, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub(crate) struct QName {
    pub(crate) ns: Option<String>,
    pub(crate) local: String,
}

impl QName {
    pub(crate) fn new(ns: Option<&str>, local: &str) -> QName {
        QName {
            ns: ns.map(str::to_owned),
            local: local.to_owned(),
        }
    }

    /// libxml's `{namespace}local` form, or `local` without a namespace.
    pub(crate) fn display(&self) -> String {
        match &self.ns {
            Some(ns) => format!("{{{ns}}}{}", self.local),
            None => self.local.clone(),
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum TypeRef {
    Simple(usize),
    Complex(usize),
}

#[derive(Debug)]
pub(crate) struct ElementDecl {
    pub(crate) name: QName,
    pub(crate) ty: TypeRef,
    pub(crate) idcs: Vec<usize>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum ValueConstraint {
    Default(String),
    Fixed(String),
}

#[derive(Debug)]
pub(crate) struct AttrDecl {
    pub(crate) name: QName,
    pub(crate) ty: usize,
    pub(crate) constraint: Option<ValueConstraint>,
}

#[derive(Clone, Debug)]
pub(crate) struct AttrUse {
    pub(crate) decl: usize,
    pub(crate) required: bool,
    pub(crate) constraint: Option<ValueConstraint>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum NsConstraint {
    Any,
    /// `##other`: neither the target namespace (`None` when the schema has
    /// none) nor no namespace.
    Other(Option<String>),
    /// An explicit list; `None` stands for `##local`.
    List(Vec<Option<String>>),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Process {
    Strict,
    Lax,
    Skip,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct Wildcard {
    pub(crate) ns: NsConstraint,
    pub(crate) process: Process,
}

impl Wildcard {
    pub(crate) fn allows(&self, ns: Option<&str>) -> bool {
        match &self.ns {
            NsConstraint::Any => true,
            NsConstraint::Other(tns) => ns.is_some() && ns != tns.as_deref(),
            NsConstraint::List(list) => list.iter().any(|n| n.as_deref() == ns),
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Max {
    One,
    Unbounded,
}

#[derive(Clone, Debug)]
pub(crate) struct Particle {
    /// 0 or 1: other bounds are refused when the schema is read.
    pub(crate) min: u32,
    pub(crate) max: Max,
    pub(crate) term: Term,
}

#[derive(Clone, Debug)]
pub(crate) enum Term {
    Element(usize),
    Any(Wildcard),
    Sequence(Vec<Particle>),
    Choice(Vec<Particle>),
}

#[derive(Clone, Debug)]
pub(crate) enum Content {
    Empty,
    /// `simpleContent`: the value's simple type.
    Simple(usize),
    /// Element content; `None` is a `mixed` type with no particle.
    Elements {
        particle: Option<Particle>,
        mixed: bool,
    },
}

#[derive(Debug)]
pub(crate) struct ComplexType {
    pub(crate) content: Content,
    pub(crate) attr_uses: Vec<AttrUse>,
    pub(crate) wildcard: Option<Wildcard>,
}

/// The built-in simple types the schemas use; others are refused.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Builtin {
    AnySimpleType,
    String,
    NmToken,
    NmTokens,
    NcName,
    Id,
    AnyUri,
    HexBinary,
    PositiveInteger,
    DateTime,
    Language,
}

impl Builtin {
    pub(crate) const ALL: [Builtin; 11] = [
        Builtin::AnySimpleType,
        Builtin::String,
        Builtin::NmToken,
        Builtin::NmTokens,
        Builtin::NcName,
        Builtin::Id,
        Builtin::AnyUri,
        Builtin::HexBinary,
        Builtin::PositiveInteger,
        Builtin::DateTime,
        Builtin::Language,
    ];

    /// The local name in the XML Schema namespace.
    pub(crate) fn name(self) -> &'static str {
        match self {
            Builtin::AnySimpleType => "anySimpleType",
            Builtin::String => "string",
            Builtin::NmToken => "NMTOKEN",
            Builtin::NmTokens => "NMTOKENS",
            Builtin::NcName => "NCName",
            Builtin::Id => "ID",
            Builtin::AnyUri => "anyURI",
            Builtin::HexBinary => "hexBinary",
            Builtin::PositiveInteger => "positiveInteger",
            Builtin::DateTime => "dateTime",
            Builtin::Language => "language",
        }
    }
}

#[derive(Clone, Debug)]
pub(crate) enum Variety {
    /// An atomic type; `primitive` is the built-in it restricts.
    Atomic {
        primitive: Builtin,
    },
    List {
        item: usize,
    },
    Union {
        members: Vec<usize>,
    },
}

#[derive(Clone, Debug, Default)]
pub(crate) struct Facets {
    pub(crate) enumeration: Vec<String>,
    /// The `pattern` facets of one restriction step (any may match).
    pub(crate) patterns: Vec<pattern::Pattern>,
    pub(crate) min_inclusive: Option<String>,
    pub(crate) max_inclusive: Option<String>,
}

#[derive(Clone, Debug)]
pub(crate) struct SimpleType {
    /// `None` for an anonymous type ("local atomic type" in messages).
    pub(crate) name: Option<String>,
    pub(crate) variety: Variety,
    /// The restriction chain's facets, the type's own first.
    pub(crate) facets: Vec<Facets>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum IdcKind {
    Unique,
    Key,
    KeyRef,
}

impl IdcKind {
    pub(crate) fn word(self) -> &'static str {
        match self {
            IdcKind::Unique => "unique",
            IdcKind::Key => "key",
            IdcKind::KeyRef => "keyref",
        }
    }
}

/// A name test of a restricted XPath step.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum NameTest {
    Name(QName),
    Any,
}

/// One branch of a selector: optionally `.//`, then child steps (`.` is
/// the scope element itself when there are no steps).
#[derive(Clone, Debug)]
pub(crate) struct SelectorPath {
    pub(crate) descendant: bool,
    pub(crate) steps: Vec<NameTest>,
}

#[derive(Debug)]
pub(crate) struct Idc {
    pub(crate) name: QName,
    pub(crate) kind: IdcKind,
    pub(crate) selector: Vec<SelectorPath>,
    /// The fields: each an attribute name (`@name`).
    pub(crate) fields: Vec<QName>,
    /// For a keyref, the key or unique it refers to.
    pub(crate) refer: Option<usize>,
}
