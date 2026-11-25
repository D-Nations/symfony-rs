//! The document tree `DOMDocument::loadXML()` builds, with the read-only
//! part of PHP's DOM API that Symfony's XML loaders call (inventory in
//! docs/milestones/m1-c-xml-x1.md). Values are UTF-8, as PHP's DOM
//! always returns them.
//!
//! Upstream: none

/// A node of a [`Document`]. Only valid for the document it came from.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct NodeId(usize);

/// PHP's `XML_*_NODE` constants for the node types a parse produces.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum NodeType {
    Element,
    Text,
    CData,
    EntityReference,
    ProcessingInstruction,
    Comment,
    Document,
    DocumentType,
}

impl NodeType {
    /// `$node->nodeType`.
    pub fn as_int(self) -> u8 {
        match self {
            NodeType::Element => 1,
            NodeType::Text => 3,
            NodeType::CData => 4,
            NodeType::EntityReference => 5,
            NodeType::ProcessingInstruction => 7,
            NodeType::Comment => 8,
            NodeType::Document => 9,
            NodeType::DocumentType => 10,
        }
    }
}

/// An attribute node (namespace declarations are not attributes in
/// PHP's DOM and are not listed).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Attribute {
    /// `nodeName`: the qualified name as written.
    pub name: String,
    /// `localName`; the whole name when its prefix is not declared.
    pub local_name: String,
    /// `prefix`; `None` where PHP returns `''`.
    pub prefix: Option<String>,
    /// `namespaceURI`.
    pub namespace_uri: Option<String>,
    /// `value`, with entity references expanded.
    pub value: String,
}

#[derive(Clone, Debug)]
pub(crate) struct Element {
    pub(crate) name: String,
    pub(crate) local_name: String,
    pub(crate) prefix: Option<String>,
    pub(crate) namespace_uri: Option<String>,
    pub(crate) attributes: Vec<Attribute>,
    /// The namespace declarations on this element (prefix, URI), in
    /// document order; `None` is the default namespace.
    pub(crate) namespaces: Vec<(Option<String>, String)>,
}

#[derive(Clone, Debug)]
pub(crate) enum Kind {
    Document,
    DocumentType {
        name: String,
        public_id: String,
        system_id: String,
    },
    Element(Element),
    Text(String),
    CData(String),
    Comment(String),
    Pi {
        target: String,
        data: String,
    },
    EntityRef(String),
}

#[derive(Clone, Debug)]
struct Node {
    kind: Kind,
    parent: Option<NodeId>,
    children: Vec<NodeId>,
    line: u32,
}

/// A parsed document (`DOMDocument`).
#[derive(Clone, Debug)]
pub struct Document {
    nodes: Vec<Node>,
    encoding: Option<String>,
    version: String,
    standalone: bool,
}

impl Document {
    pub(crate) fn new() -> Self {
        Document {
            nodes: vec![Node {
                kind: Kind::Document,
                parent: None,
                children: Vec::new(),
                line: 0,
            }],
            encoding: None,
            version: "1.0".into(),
            standalone: false,
        }
    }

    pub(crate) fn set_declaration(
        &mut self,
        version: String,
        encoding: Option<String>,
        standalone: bool,
    ) {
        self.version = version;
        self.encoding = encoding;
        self.standalone = standalone;
    }

    pub(crate) fn append(&mut self, parent: NodeId, kind: Kind, line: u32) -> NodeId {
        let id = NodeId(self.nodes.len());
        self.nodes.push(Node {
            kind,
            parent: Some(parent),
            children: Vec::new(),
            line,
        });
        self.nodes[parent.0].children.push(id);
        id
    }

    /// Adjacent CDATA sections are merged, as libxml's SAX2 handler does.
    pub(crate) fn append_cdata(&mut self, node: NodeId, text: &str) {
        if let Kind::CData(existing) = &mut self.nodes[node.0].kind {
            existing.push_str(text);
        }
    }

    /// Character data: appended to the last child when it is a text node,
    /// as libxml's SAX2 handler does.
    pub(crate) fn append_text(&mut self, parent: NodeId, text: &str, line: u32) {
        if let Some(&last) = self.nodes[parent.0].children.last()
            && let Kind::Text(existing) = &mut self.nodes[last.0].kind
        {
            existing.push_str(text);
            return;
        }
        self.append(parent, Kind::Text(text.to_owned()), line);
    }

    /// The document node itself.
    pub fn root(&self) -> NodeId {
        NodeId(0)
    }

    /// `$doc->documentElement`.
    pub fn document_element(&self) -> Option<NodeId> {
        self.children(self.root())
            .iter()
            .copied()
            .find(|&c| self.node_type(c) == NodeType::Element)
    }

    /// `$doc->encoding` (and `xmlEncoding`): the declared name as written.
    pub fn encoding(&self) -> Option<&str> {
        self.encoding.as_deref()
    }

    /// `$doc->xmlVersion`.
    pub fn xml_version(&self) -> &str {
        &self.version
    }

    /// `$doc->xmlStandalone`.
    pub fn xml_standalone(&self) -> bool {
        self.standalone
    }

    /// `$node->childNodes`, in document order.
    pub fn children(&self, id: NodeId) -> &[NodeId] {
        &self.nodes[id.0].children
    }

    /// `$node->parentNode`.
    pub fn parent(&self, id: NodeId) -> Option<NodeId> {
        self.nodes[id.0].parent
    }

    /// `$node->nextSibling`.
    pub fn next_sibling(&self, id: NodeId) -> Option<NodeId> {
        self.sibling(id, 1)
    }

    /// `$node->previousSibling`.
    pub fn previous_sibling(&self, id: NodeId) -> Option<NodeId> {
        self.sibling(id, -1)
    }

    fn sibling(&self, id: NodeId, step: isize) -> Option<NodeId> {
        let parent = self.parent(id)?;
        let siblings = self.children(parent);
        let at = siblings.iter().position(|&c| c == id)?;
        let next = at.checked_add_signed(step)?;
        siblings.get(next).copied()
    }

    /// `$node->nodeType`.
    pub fn node_type(&self, id: NodeId) -> NodeType {
        match &self.nodes[id.0].kind {
            Kind::Document => NodeType::Document,
            Kind::DocumentType { .. } => NodeType::DocumentType,
            Kind::Element(_) => NodeType::Element,
            Kind::Text(_) => NodeType::Text,
            Kind::CData(_) => NodeType::CData,
            Kind::Comment(_) => NodeType::Comment,
            Kind::Pi { .. } => NodeType::ProcessingInstruction,
            Kind::EntityRef(_) => NodeType::EntityReference,
        }
    }

    /// `$node->nodeName`.
    pub fn node_name(&self, id: NodeId) -> &str {
        match &self.nodes[id.0].kind {
            Kind::Document => "#document",
            Kind::DocumentType { name, .. } | Kind::EntityRef(name) => name,
            Kind::Element(e) => &e.name,
            Kind::Text(_) => "#text",
            Kind::CData(_) => "#cdata-section",
            Kind::Comment(_) => "#comment",
            Kind::Pi { target, .. } => target,
        }
    }

    /// `$node->localName` (elements only; `null` otherwise).
    pub fn local_name(&self, id: NodeId) -> Option<&str> {
        match &self.nodes[id.0].kind {
            Kind::Element(e) => Some(&e.local_name),
            _ => None,
        }
    }

    /// `$node->prefix`; `None` where PHP returns `''`.
    pub fn prefix(&self, id: NodeId) -> Option<&str> {
        match &self.nodes[id.0].kind {
            Kind::Element(e) => e.prefix.as_deref(),
            _ => None,
        }
    }

    /// `$node->namespaceURI`.
    pub fn namespace_uri(&self, id: NodeId) -> Option<&str> {
        match &self.nodes[id.0].kind {
            Kind::Element(e) => e.namespace_uri.as_deref(),
            _ => None,
        }
    }

    /// `$node->nodeValue` for character data, comments and processing
    /// instructions (`null` for elements, the document and references).
    pub fn node_value(&self, id: NodeId) -> Option<&str> {
        match &self.nodes[id.0].kind {
            Kind::Text(s) | Kind::CData(s) | Kind::Comment(s) => Some(s),
            Kind::Pi { data, .. } => Some(data),
            _ => None,
        }
    }

    /// `$pi->target` and `$pi->data`.
    pub fn processing_instruction(&self, id: NodeId) -> Option<(&str, &str)> {
        match &self.nodes[id.0].kind {
            Kind::Pi { target, data } => Some((target, data)),
            _ => None,
        }
    }

    /// `$doctype->name`, `publicId`, `systemId` (`''` when absent).
    pub fn document_type(&self, id: NodeId) -> Option<(&str, &str, &str)> {
        match &self.nodes[id.0].kind {
            Kind::DocumentType {
                name,
                public_id,
                system_id,
            } => Some((name, public_id, system_id)),
            _ => None,
        }
    }

    /// `$node->textContent`: the concatenated character data below the
    /// node. Entity references contribute nothing (their replacement
    /// text is not kept).
    pub fn text_content(&self, id: NodeId) -> String {
        let mut out = String::new();
        match &self.nodes[id.0].kind {
            Kind::Text(s) | Kind::CData(s) | Kind::Comment(s) => out.push_str(s),
            Kind::Pi { data, .. } => out.push_str(data),
            Kind::Document | Kind::DocumentType { .. } => {}
            _ => self.collect_text(id, &mut out),
        }
        out
    }

    fn collect_text(&self, id: NodeId, out: &mut String) {
        for &c in self.children(id) {
            match &self.nodes[c.0].kind {
                Kind::Text(s) | Kind::CData(s) => out.push_str(s),
                Kind::Element(_) => self.collect_text(c, out),
                _ => {}
            }
        }
    }

    /// `$node->attributes` of an element, in document order.
    pub fn attributes(&self, id: NodeId) -> &[Attribute] {
        match &self.nodes[id.0].kind {
            Kind::Element(e) => &e.attributes,
            _ => &[],
        }
    }

    /// `$element->getAttribute($name)`: the first attribute whose
    /// qualified name is `name`, or `''`.
    pub fn get_attribute(&self, id: NodeId, name: &str) -> &str {
        self.attributes(id)
            .iter()
            .find(|a| a.name == name)
            .map_or("", |a| a.value.as_str())
    }

    /// `$element->hasAttribute($name)`.
    pub fn has_attribute(&self, id: NodeId, name: &str) -> bool {
        self.attributes(id).iter().any(|a| a.name == name)
    }

    /// `$node->lookupNamespaceURI($prefix)`: the namespace bound to
    /// `prefix` (`None`: the default namespace) where the element `id` is,
    /// from its own and its ancestors' declarations. `xml` is always bound.
    pub fn lookup_namespace_uri(&self, id: NodeId, prefix: Option<&str>) -> Option<&str> {
        if prefix == Some("xml") {
            return Some("http://www.w3.org/XML/1998/namespace");
        }
        let mut at = Some(id);
        while let Some(n) = at {
            if let Kind::Element(e) = &self.nodes[n.0].kind
                && let Some((_, uri)) = e
                    .namespaces
                    .iter()
                    .rev()
                    .find(|(p, _)| p.as_deref() == prefix)
            {
                return (!uri.is_empty()).then_some(uri.as_str());
            }
            at = self.parent(n);
        }
        None
    }

    /// `$node->getLineNo()`: the line where libxml created the node.
    pub fn line(&self, id: NodeId) -> u32 {
        self.nodes[id.0].line
    }

    /// `$node->getElementsByTagName($name)`: the descendant elements whose
    /// local name is `name` (`'*'` for all), in document order. PHP 8.4
    /// compares the local name only, so a qualified name such as `x:b`
    /// matches nothing (recorded: symfony-translation's `tag_names`).
    pub fn get_elements_by_tag_name(&self, id: NodeId, name: &str) -> Vec<NodeId> {
        let mut out = Vec::new();
        let mut stack: Vec<NodeId> = self.children(id).iter().rev().copied().collect();
        while let Some(n) = stack.pop() {
            if let Kind::Element(e) = &self.nodes[n.0].kind {
                if name == "*" || e.local_name == name {
                    out.push(n);
                }
                stack.extend(self.children(n).iter().rev().copied());
            }
        }
        out
    }
}
