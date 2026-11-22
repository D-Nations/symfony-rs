//! Element content against a content model: a cursor over the particle
//! tree, and the "Expected is ( … )" lists libxml prints when a child is
//! not expected or missing.
//!
//! Matching is plain XML Schema (the schemas obey Unique Particle
//! Attribution, so the first way a name can be consumed is the only one).
//! The lists follow libxml's automaton, as observed on recorded probes
//! (docs/milestones/m1-c-xml-x2.md, "Expected lists"):
//!
//! - a particle repeated without bound lists its own next occurrence
//!   before what follows it;
//! - inside a choice that may repeat (`maxOccurs="unbounded"`), neither
//!   the choice's next round nor any repetition within the round is
//!   listed: once the round may end, only what follows the choice is;
//! - a repeatable choice with a branch that may be empty hides what was
//!   listed before it: the list restarts after the choice.
//!
//! Particles are limited, when the schema is read, to `minOccurs` 0 or 1
//! and `maxOccurs` 1 or `unbounded`, and a repeatable choice's branches to
//! elements and wildcards, the shapes these rules were observed on.
//!
//! Upstream: none

use super::QName;
use super::{Max, Particle, Schema, Term, Wildcard};

/// One entry of an expected list.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum Atom<'s> {
    Element(&'s QName),
    Any(&'s Wildcard),
}

impl Atom<'_> {
    /// libxml's spelling: `{ns}local`, `*`, `{ns}*`, `##other{ns}*`. A
    /// wildcard with a namespace list is one entry per namespace.
    pub(crate) fn display(&self) -> Vec<String> {
        match self {
            Atom::Element(q) => vec![q.display()],
            Atom::Any(w) => match &w.ns {
                super::NsConstraint::Any => vec!["*".into()],
                super::NsConstraint::Other(tns) => vec![match tns {
                    Some(t) => format!("##other{{{t}}}*"),
                    None => "##other*".into(),
                }],
                super::NsConstraint::List(list) => list
                    .iter()
                    .map(|n| match n {
                        Some(n) => format!("{{{n}}}*"),
                        None => "*".into(),
                    })
                    .collect(),
            },
        }
    }
}

#[derive(Clone, Copy, Debug)]
struct Frame<'s> {
    p: &'s Particle,
    /// The branch (choice) or child (sequence) the cursor is in.
    idx: usize,
}

/// What a leaf of the model matched.
#[derive(Clone, Copy, Debug)]
pub(crate) enum Matched<'s> {
    Element(usize),
    Any(&'s Wildcard),
}

pub(crate) struct Cursor<'s> {
    schema: &'s Schema,
    root: Option<&'s Particle>,
    frames: Vec<Frame<'s>>,
}

fn counted_choice(p: &Particle) -> bool {
    matches!(p.term, Term::Choice(_)) && p.max == Max::Unbounded
}

fn nullable(p: &Particle) -> bool {
    p.min == 0 || term_nullable(&p.term)
}

fn term_nullable(t: &Term) -> bool {
    match t {
        Term::Element(_) | Term::Any(_) => false,
        Term::Sequence(c) => c.iter().all(nullable),
        Term::Choice(c) => c.iter().any(nullable),
    }
}

/// A repeatable choice with a branch that may be empty.
fn hiding(p: &Particle) -> bool {
    counted_choice(p)
        && match &p.term {
            Term::Choice(c) => c.iter().any(nullable),
            _ => false,
        }
}

/// Listing state: atoms so far, deduplicated in order.
struct List<'s> {
    atoms: Vec<Atom<'s>>,
}

impl<'s> List<'s> {
    fn add(&mut self, a: Atom<'s>) {
        if !self.atoms.contains(&a) {
            self.atoms.push(a);
        }
    }
}

impl<'s> Cursor<'s> {
    pub(crate) fn new(schema: &'s Schema, root: Option<&'s Particle>) -> Self {
        Cursor {
            schema,
            root,
            frames: Vec::new(),
        }
    }

    fn leaf_matches(&self, t: &Term, name: &QName) -> bool {
        match t {
            Term::Element(e) => self.schema.elements[*e].name == *name,
            Term::Any(w) => w.allows(name.ns.as_deref()),
            _ => false,
        }
    }

    /// The frames for entering `p` and consuming `name` first.
    fn enter(&self, p: &'s Particle, name: &QName) -> Option<Vec<Frame<'s>>> {
        match &p.term {
            Term::Element(_) | Term::Any(_) => self
                .leaf_matches(&p.term, name)
                .then(|| vec![Frame { p, idx: 0 }]),
            Term::Sequence(children) => {
                for (j, c) in children.iter().enumerate() {
                    if let Some(sub) = self.enter(c, name) {
                        let mut v = vec![Frame { p, idx: j }];
                        v.extend(sub);
                        return Some(v);
                    }
                    if !nullable(c) {
                        return None;
                    }
                }
                None
            }
            Term::Choice(branches) => {
                for (j, c) in branches.iter().enumerate() {
                    if let Some(sub) = self.enter(c, name) {
                        let mut v = vec![Frame { p, idx: j }];
                        v.extend(sub);
                        return Some(v);
                    }
                }
                None
            }
        }
    }

    /// Whether the round frame `k` is in may end where the cursor is.
    fn complete(&self, k: usize) -> bool {
        let f = self.frames[k];
        let deeper = k + 1 >= self.frames.len() || self.complete(k + 1);
        match &f.p.term {
            Term::Element(_) | Term::Any(_) => true,
            Term::Sequence(children) => deeper && children[f.idx + 1..].iter().all(nullable),
            Term::Choice(_) => deeper,
        }
    }

    /// Consumes a child element; `None` if the model does not allow it
    /// here (the cursor is then unchanged).
    pub(crate) fn push(&mut self, name: &QName) -> Option<Matched<'s>> {
        let found = self.advance(name)?;
        self.frames = found;
        let leaf = self.frames.last()?;
        Some(match &leaf.p.term {
            Term::Element(e) => Matched::Element(*e),
            Term::Any(w) => Matched::Any(w),
            _ => return None,
        })
    }

    fn advance(&self, name: &QName) -> Option<Vec<Frame<'s>>> {
        let root = self.root?;
        if self.frames.is_empty() {
            return self.enter(root, name);
        }
        for k in (0..self.frames.len()).rev() {
            let f = self.frames[k];
            let deeper_done = k + 1 >= self.frames.len() || self.complete(k + 1);
            if let Term::Sequence(children) = &f.p.term
                && deeper_done
            {
                for (j, c) in children.iter().enumerate().skip(f.idx + 1) {
                    if let Some(sub) = self.enter(c, name) {
                        let mut v = self.frames[..k].to_vec();
                        v.push(Frame { p: f.p, idx: j });
                        v.extend(sub);
                        return Some(v);
                    }
                    if !nullable(c) {
                        break;
                    }
                }
            }
            if f.p.max == Max::Unbounded
                && self.complete(k)
                && let Some(sub) = self.enter(f.p, name)
            {
                let mut v = self.frames[..k].to_vec();
                v.extend(sub);
                return Some(v);
            }
            if !self.complete(k) {
                return None;
            }
        }
        None
    }

    /// Whether the content may end here.
    pub(crate) fn can_end(&self) -> bool {
        match self.root {
            None => true,
            Some(root) => {
                if self.frames.is_empty() {
                    nullable(root)
                } else {
                    self.complete(0)
                }
            }
        }
    }

    /// Lists `first(p)`; a hiding choice clears what was listed before it.
    fn first(&self, p: &'s Particle, out: &mut List<'s>) {
        match &p.term {
            Term::Element(e) => out.add(Atom::Element(&self.schema.elements[*e].name)),
            Term::Any(w) => out.add(Atom::Any(w)),
            Term::Sequence(children) => {
                for c in children {
                    self.first(c, out);
                    if !nullable(c) {
                        break;
                    }
                }
            }
            Term::Choice(branches) => {
                if hiding(p) {
                    out.atoms.clear();
                    return;
                }
                for b in branches {
                    self.first(b, out);
                }
            }
        }
    }

    /// Lists the following siblings of frame `k`'s current child, up to
    /// the first that cannot be empty. `true` if all of them may be.
    fn siblings(&self, k: usize, out: &mut List<'s>) -> bool {
        let f = self.frames[k];
        if let Term::Sequence(children) = &f.p.term {
            for c in &children[f.idx + 1..] {
                self.first(c, out);
                if !nullable(c) {
                    return false;
                }
            }
        }
        true
    }

    /// The elements libxml lists as expected where the cursor is.
    pub(crate) fn expected(&self) -> Vec<Atom<'s>> {
        let mut out = List { atoms: Vec::new() };
        let Some(root) = self.root else {
            return out.atoms;
        };
        if self.frames.is_empty() {
            self.first(root, &mut out);
            return out.atoms;
        }
        // The outermost repeatable choice the cursor is in.
        let region = self.frames.iter().position(|f| counted_choice(f.p));
        let mut k = self.frames.len();
        if let Some(h) = region {
            // Within the round: only what it still requires.
            let mut inner = List { atoms: Vec::new() };
            let mut open = true;
            for j in (h + 1..self.frames.len()).rev() {
                if !self.siblings(j, &mut inner) {
                    open = false;
                    break;
                }
            }
            if !open {
                return inner.atoms;
            }
            k = h;
        }
        while k > 0 {
            k -= 1;
            let f = self.frames[k];
            let loops = f.p.max == Max::Unbounded && region.is_none_or(|h| k < h);
            let rest_nullable = match &f.p.term {
                Term::Sequence(children) => children[f.idx + 1..].iter().all(nullable),
                _ => true,
            };
            if loops && rest_nullable && !counted_choice(f.p) {
                self.first(f.p, &mut out);
            }
            if !self.siblings(k, &mut out) {
                break;
            }
        }
        out.atoms
    }
}
