//! Port of Symfony\Component\CssSelector\Node\*.php: the parsed selector
//! tree. `NodeInterface` is closed here (the node classes are
//! `@internal` upstream), so the nodes are one enum; `__toString()` is
//! [`Node::to_php_string`], which keeps PHP's truthiness (`"0"` is false).
//!
//! Upstream: symfony/css-selector Node/NodeInterface.php Node/AbstractNode.php Node/Specificity.php Node/AttributeNode.php Node/ClassNode.php Node/CombinedSelectorNode.php Node/ElementNode.php Node/FunctionNode.php Node/HashNode.php Node/MatchingNode.php Node/NegationNode.php Node/PseudoNode.php Node/RelationNode.php Node/SelectorNode.php Node/SpecificityAdjustmentNode.php

use std::cmp::Ordering;

use crate::parser::Token;
use crate::php::{cat, strtolower, truthy, truthy_opt};

/// `Specificity`: `a*100 + b*10 + c`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Specificity {
    a: i64,
    b: i64,
    c: i64,
}

impl Specificity {
    pub const A_FACTOR: i64 = 100;
    pub const B_FACTOR: i64 = 10;
    pub const C_FACTOR: i64 = 1;

    pub fn new(a: i64, b: i64, c: i64) -> Self {
        Self { a, b, c }
    }

    #[must_use]
    pub fn plus(&self, specificity: &Specificity) -> Self {
        Self::new(
            self.a + specificity.a,
            self.b + specificity.b,
            self.c + specificity.c,
        )
    }

    pub fn get_value(&self) -> i64 {
        self.a * Self::A_FACTOR + self.b * Self::B_FACTOR + self.c * Self::C_FACTOR
    }

    /// -1 if this is lower than the argument, 0 if equal, 1 if higher.
    pub fn compare_to(&self, specificity: &Specificity) -> i32 {
        match (self.a, self.b, self.c).cmp(&(specificity.a, specificity.b, specificity.c)) {
            Ordering::Less => -1,
            Ordering::Equal => 0,
            Ordering::Greater => 1,
        }
    }
}

/// `max` by `compareTo() === 1`, from `(0, 0, 0)`, as the `:is()` and
/// `:has()` nodes reduce their arguments.
fn highest<'a>(specificities: impl Iterator<Item = Specificity> + 'a) -> Specificity {
    specificities.fold(Specificity::new(0, 0, 0), |c, n| {
        if n.compare_to(&c) == 1 { n } else { c }
    })
}

/// A node of the selector tree.
#[derive(Debug, Clone, PartialEq)]
pub enum Node {
    Attribute(AttributeNode),
    Class(ClassNode),
    CombinedSelector(CombinedSelectorNode),
    Element(ElementNode),
    Function(FunctionNode),
    Hash(HashNode),
    Matching(MatchingNode),
    Negation(NegationNode),
    Pseudo(PseudoNode),
    Relation(RelationNode),
    Selector(SelectorNode),
    SpecificityAdjustment(SpecificityAdjustmentNode),
}

impl Node {
    /// `getNodeName()`: the class name without `Node`.
    pub fn get_node_name(&self) -> &'static str {
        match self {
            Self::Attribute(_) => "Attribute",
            Self::Class(_) => "Class",
            Self::CombinedSelector(_) => "CombinedSelector",
            Self::Element(_) => "Element",
            Self::Function(_) => "Function",
            Self::Hash(_) => "Hash",
            Self::Matching(_) => "Matching",
            Self::Negation(_) => "Negation",
            Self::Pseudo(_) => "Pseudo",
            Self::Relation(_) => "Relation",
            Self::Selector(_) => "Selector",
            Self::SpecificityAdjustment(_) => "SpecificityAdjustment",
        }
    }

    pub fn get_specificity(&self) -> Specificity {
        let one = |a, b, c| Specificity::new(a, b, c);
        match self {
            Self::Attribute(n) => n.selector.get_specificity().plus(&one(0, 1, 0)),
            Self::Class(n) => n.selector.get_specificity().plus(&one(0, 1, 0)),
            Self::CombinedSelector(n) => n
                .selector
                .get_specificity()
                .plus(&n.sub_selector.get_specificity()),
            Self::Element(n) => one(0, 0, i64::from(truthy_opt(n.element.as_deref()))),
            Self::Function(n) => n.selector.get_specificity().plus(&one(0, 1, 0)),
            Self::Hash(n) => n.selector.get_specificity().plus(&one(1, 0, 0)),
            Self::Matching(n) => n
                .selector
                .get_specificity()
                .plus(&highest(n.arguments.iter().map(Node::get_specificity))),
            Self::Negation(n) => n
                .selector
                .get_specificity()
                .plus(&n.sub_selector.get_specificity()),
            Self::Pseudo(n) => n.selector.get_specificity().plus(&one(0, 1, 0)),
            Self::Relation(n) => n.selector.get_specificity().plus(&highest(
                n.arguments.iter().map(|(_, a)| a.get_specificity()),
            )),
            Self::Selector(n) => n.get_specificity(),
            Self::SpecificityAdjustment(n) => n.selector.get_specificity(),
        }
    }

    /// `__toString()`.
    #[expect(clippy::too_many_lines, reason = "one match over the node kinds")]
    pub fn to_php_string(&self) -> Vec<u8> {
        let name = self.get_node_name().as_bytes();
        match self {
            Self::Attribute(n) => {
                let attribute = match n.namespace.as_deref() {
                    Some(ns) if truthy(ns) => cat(&[ns, b"|", &n.attribute]),
                    _ => n.attribute.clone(),
                };
                if n.operator == b"exists" {
                    cat(&[
                        name,
                        b"[",
                        &n.selector.to_php_string(),
                        b"[",
                        &attribute,
                        b"]]",
                    ])
                } else {
                    cat(&[
                        name,
                        b"[",
                        &n.selector.to_php_string(),
                        b"[",
                        &attribute,
                        b" ",
                        &n.operator,
                        b" '",
                        n.value.as_deref().unwrap_or_default(),
                        b"']]",
                    ])
                }
            }
            Self::Class(n) => cat(&[name, b"[", &n.selector.to_php_string(), b".", &n.name, b"]"]),
            Self::CombinedSelector(n) => {
                let combinator: &[u8] = if n.combinator == b" " {
                    b"<followed>"
                } else {
                    &n.combinator
                };
                cat(&[
                    name,
                    b"[",
                    &n.selector.to_php_string(),
                    b" ",
                    combinator,
                    b" ",
                    &n.sub_selector.to_php_string(),
                    b"]",
                ])
            }
            Self::Element(n) => {
                let element: &[u8] = match n.element.as_deref() {
                    Some(e) if truthy(e) => e,
                    _ => b"*",
                };
                let inner = match n.namespace.as_deref() {
                    Some(ns) if truthy(ns) => cat(&[ns, b"|", element]),
                    _ => element.to_vec(),
                };
                cat(&[name, b"[", &inner, b"]"])
            }
            Self::Function(n) => {
                let arguments: Vec<Vec<u8>> = n
                    .arguments
                    .iter()
                    .map(|t| cat(&[b"'", t.get_value().unwrap_or_default(), b"'"]))
                    .collect();
                let arguments = arguments.join(&b", "[..]);
                let arguments = if truthy(&arguments) {
                    cat(&[b"[", &arguments, b"]"])
                } else {
                    Vec::new()
                };
                cat(&[
                    name,
                    b"[",
                    &n.selector.to_php_string(),
                    b":",
                    &n.name,
                    b"(",
                    &arguments,
                    b")]",
                ])
            }
            Self::Hash(n) => cat(&[name, b"[", &n.selector.to_php_string(), b"#", &n.id, b"]"]),
            Self::Matching(n) => cat(&[
                name,
                b"[",
                &n.selector.to_php_string(),
                b":is(",
                &ltrim_star_list(&n.arguments),
                b")]",
            ]),
            Self::Negation(n) => cat(&[
                name,
                b"[",
                &n.selector.to_php_string(),
                b":not(",
                &n.sub_selector.to_php_string(),
                b")]",
            ]),
            Self::Pseudo(n) => cat(&[
                name,
                b"[",
                &n.selector.to_php_string(),
                b":",
                &n.identifier,
                b"]",
            ]),
            Self::Relation(n) => {
                let parts: Vec<Vec<u8>> = n
                    .arguments
                    .iter()
                    .map(|(combinator, node)| {
                        if combinator == b" " {
                            node.to_php_string()
                        } else {
                            cat(&[combinator, b" ", &node.to_php_string()])
                        }
                    })
                    .collect();
                cat(&[
                    name,
                    b"[",
                    &n.selector.to_php_string(),
                    b":has(",
                    &parts.join(&b", "[..]),
                    b")]",
                ])
            }
            Self::Selector(n) => n.to_php_string(),
            Self::SpecificityAdjustment(n) => cat(&[
                name,
                b"[",
                &n.selector.to_php_string(),
                b":where(",
                &ltrim_star_list(&n.arguments),
                b")]",
            ]),
        }
    }
}

/// `implode(', ', array_map(fn ($n) => ltrim((string) $n, '*'), ...))`.
fn ltrim_star_list(nodes: &[Node]) -> Vec<u8> {
    let parts: Vec<Vec<u8>> = nodes
        .iter()
        .map(|n| {
            let s = n.to_php_string();
            let start = s.iter().position(|&b| b != b'*').unwrap_or(s.len());
            s[start..].to_vec()
        })
        .collect();
    parts.join(&b", "[..])
}

macro_rules! into_node {
    ($($ty:ident => $variant:ident),*) => {$(
        impl From<$ty> for Node {
            fn from(node: $ty) -> Self {
                Node::$variant(node)
            }
        }
    )*};
}

into_node!(
    AttributeNode => Attribute,
    ClassNode => Class,
    CombinedSelectorNode => CombinedSelector,
    ElementNode => Element,
    FunctionNode => Function,
    HashNode => Hash,
    MatchingNode => Matching,
    NegationNode => Negation,
    PseudoNode => Pseudo,
    RelationNode => Relation,
    SelectorNode => Selector,
    SpecificityAdjustmentNode => SpecificityAdjustment
);

/// `<selector>[<namespace>|<attribute> <operator> <value>]`.
#[derive(Debug, Clone, PartialEq)]
pub struct AttributeNode {
    selector: Box<Node>,
    namespace: Option<Vec<u8>>,
    attribute: Vec<u8>,
    operator: Vec<u8>,
    value: Option<Vec<u8>>,
}

impl AttributeNode {
    pub fn new(
        selector: impl Into<Node>,
        namespace: Option<&[u8]>,
        attribute: &[u8],
        operator: &[u8],
        value: Option<&[u8]>,
    ) -> Self {
        Self {
            selector: Box::new(selector.into()),
            namespace: namespace.map(<[u8]>::to_vec),
            attribute: attribute.to_vec(),
            operator: operator.to_vec(),
            value: value.map(<[u8]>::to_vec),
        }
    }

    pub fn get_selector(&self) -> &Node {
        &self.selector
    }

    pub fn get_namespace(&self) -> Option<&[u8]> {
        self.namespace.as_deref()
    }

    pub fn get_attribute(&self) -> &[u8] {
        &self.attribute
    }

    pub fn get_operator(&self) -> &[u8] {
        &self.operator
    }

    pub fn get_value(&self) -> Option<&[u8]> {
        self.value.as_deref()
    }
}

/// `<selector>.<name>`.
#[derive(Debug, Clone, PartialEq)]
pub struct ClassNode {
    selector: Box<Node>,
    name: Vec<u8>,
}

impl ClassNode {
    pub fn new(selector: impl Into<Node>, name: &[u8]) -> Self {
        Self {
            selector: Box::new(selector.into()),
            name: name.to_vec(),
        }
    }

    pub fn get_selector(&self) -> &Node {
        &self.selector
    }

    pub fn get_name(&self) -> &[u8] {
        &self.name
    }
}

/// `<selector> <combinator> <subSelector>`.
#[derive(Debug, Clone, PartialEq)]
pub struct CombinedSelectorNode {
    selector: Box<Node>,
    combinator: Vec<u8>,
    sub_selector: Box<Node>,
}

impl CombinedSelectorNode {
    pub fn new(
        selector: impl Into<Node>,
        combinator: &[u8],
        sub_selector: impl Into<Node>,
    ) -> Self {
        Self {
            selector: Box::new(selector.into()),
            combinator: combinator.to_vec(),
            sub_selector: Box::new(sub_selector.into()),
        }
    }

    pub fn get_selector(&self) -> &Node {
        &self.selector
    }

    pub fn get_combinator(&self) -> &[u8] {
        &self.combinator
    }

    pub fn get_sub_selector(&self) -> &Node {
        &self.sub_selector
    }
}

/// `<namespace>|<element>`.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct ElementNode {
    namespace: Option<Vec<u8>>,
    element: Option<Vec<u8>>,
}

impl ElementNode {
    pub fn new(namespace: Option<&[u8]>, element: Option<&[u8]>) -> Self {
        Self {
            namespace: namespace.map(<[u8]>::to_vec),
            element: element.map(<[u8]>::to_vec),
        }
    }

    pub fn get_namespace(&self) -> Option<&[u8]> {
        self.namespace.as_deref()
    }

    pub fn get_element(&self) -> Option<&[u8]> {
        self.element.as_deref()
    }
}

/// `<selector>:<name>(<arguments>)`; the name is lowercased.
#[derive(Debug, Clone, PartialEq)]
pub struct FunctionNode {
    selector: Box<Node>,
    name: Vec<u8>,
    arguments: Vec<Token>,
}

impl FunctionNode {
    pub fn new(selector: impl Into<Node>, name: &[u8], arguments: Vec<Token>) -> Self {
        Self {
            selector: Box::new(selector.into()),
            name: strtolower(name),
            arguments,
        }
    }

    pub fn get_selector(&self) -> &Node {
        &self.selector
    }

    pub fn get_name(&self) -> &[u8] {
        &self.name
    }

    pub fn get_arguments(&self) -> &[Token] {
        &self.arguments
    }
}

/// `<selector>#<id>`.
#[derive(Debug, Clone, PartialEq)]
pub struct HashNode {
    selector: Box<Node>,
    id: Vec<u8>,
}

impl HashNode {
    pub fn new(selector: impl Into<Node>, id: &[u8]) -> Self {
        Self {
            selector: Box::new(selector.into()),
            id: id.to_vec(),
        }
    }

    pub fn get_selector(&self) -> &Node {
        &self.selector
    }

    pub fn get_id(&self) -> &[u8] {
        &self.id
    }
}

/// `<selector>:is(<subSelectorList>)`.
#[derive(Debug, Clone, PartialEq)]
pub struct MatchingNode {
    pub selector: Box<Node>,
    pub arguments: Vec<Node>,
}

impl MatchingNode {
    pub fn new(selector: impl Into<Node>, arguments: Vec<Node>) -> Self {
        Self {
            selector: Box::new(selector.into()),
            arguments,
        }
    }
}

/// `<selector>:not(<identifier>)`.
#[derive(Debug, Clone, PartialEq)]
pub struct NegationNode {
    selector: Box<Node>,
    sub_selector: Box<Node>,
}

impl NegationNode {
    pub fn new(selector: impl Into<Node>, sub_selector: impl Into<Node>) -> Self {
        Self {
            selector: Box::new(selector.into()),
            sub_selector: Box::new(sub_selector.into()),
        }
    }

    pub fn get_selector(&self) -> &Node {
        &self.selector
    }

    pub fn get_sub_selector(&self) -> &Node {
        &self.sub_selector
    }
}

/// `<selector>:<identifier>`; the identifier is lowercased.
#[derive(Debug, Clone, PartialEq)]
pub struct PseudoNode {
    selector: Box<Node>,
    identifier: Vec<u8>,
}

impl PseudoNode {
    pub fn new(selector: impl Into<Node>, identifier: &[u8]) -> Self {
        Self {
            selector: Box::new(selector.into()),
            identifier: strtolower(identifier),
        }
    }

    pub fn get_selector(&self) -> &Node {
        &self.selector
    }

    pub fn get_identifier(&self) -> &[u8] {
        &self.identifier
    }
}

/// `<selector>:has(<subselector>)`: `(combinator, selector)` pairs.
#[derive(Debug, Clone, PartialEq)]
pub struct RelationNode {
    selector: Box<Node>,
    arguments: Vec<(Vec<u8>, Node)>,
}

impl RelationNode {
    pub fn new(selector: impl Into<Node>, arguments: Vec<(Vec<u8>, Node)>) -> Self {
        Self {
            selector: Box::new(selector.into()),
            arguments,
        }
    }

    pub fn get_selector(&self) -> &Node {
        &self.selector
    }

    pub fn get_arguments(&self) -> &[(Vec<u8>, Node)] {
        &self.arguments
    }
}

/// `<selector>(::|:)<pseudoElement>`; the pseudo-element is lowercased,
/// and a false one (`""`, `"0"`) is none.
#[derive(Debug, Clone, PartialEq)]
pub struct SelectorNode {
    tree: Box<Node>,
    pseudo_element: Option<Vec<u8>>,
}

impl SelectorNode {
    pub fn new(tree: impl Into<Node>, pseudo_element: Option<&[u8]>) -> Self {
        Self {
            tree: Box::new(tree.into()),
            pseudo_element: pseudo_element.filter(|p| truthy(p)).map(strtolower),
        }
    }

    pub fn get_tree(&self) -> &Node {
        &self.tree
    }

    pub fn get_pseudo_element(&self) -> Option<&[u8]> {
        self.pseudo_element.as_deref()
    }

    pub fn get_specificity(&self) -> Specificity {
        let c = i64::from(truthy_opt(self.pseudo_element.as_deref()));
        self.tree.get_specificity().plus(&Specificity::new(0, 0, c))
    }

    pub fn to_php_string(&self) -> Vec<u8> {
        let pseudo = match self.pseudo_element.as_deref() {
            Some(p) if truthy(p) => cat(&[b"::", p]),
            _ => Vec::new(),
        };
        cat(&[b"Selector[", &self.tree.to_php_string(), &pseudo, b"]"])
    }
}

/// `<selector>:where(<subSelectorList>)`.
#[derive(Debug, Clone, PartialEq)]
pub struct SpecificityAdjustmentNode {
    pub selector: Box<Node>,
    pub arguments: Vec<Node>,
}

impl SpecificityAdjustmentNode {
    pub fn new(selector: impl Into<Node>, arguments: Vec<Node>) -> Self {
        Self {
            selector: Box::new(selector.into()),
            arguments,
        }
    }
}
