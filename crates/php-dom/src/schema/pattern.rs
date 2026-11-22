//! The `pattern` facet: XML Schema 1.0 regular expressions (Part 2,
//! appendix F), matched against the whole value. Compiled to a Thompson
//! automaton and run by simulation, so matching is linear in the value
//! whatever the pattern (no backtracking).
//!
//! Supported: literal characters, the single-character escapes, `.`,
//! `\s` `\S` `\d` `\D`, character class expressions with ranges and
//! negation, groups, `|`, and the quantifiers `?` `*` `+` `{n}` `{n,}`
//! `{n,m}` (m up to 100). Refused when the schema is read: `\i` `\c` `\w`
//! their complements, `\p{..}` `\P{..}` and class subtraction.
//!
//! Upstream: none

#[derive(Clone, Debug)]
enum Item {
    Range(char, char),
    Space(bool),
    Digit(bool),
}

#[derive(Clone, Debug)]
struct Class {
    negated: bool,
    items: Vec<Item>,
}

/// Whether `c` is a decimal digit (`\p{Nd}`). Outside ASCII the answer
/// needs Unicode's category table, which this crate does not carry.
fn digit(c: char) -> Result<bool, ()> {
    if c.is_ascii() {
        Ok(c.is_ascii_digit())
    } else if c.is_numeric() {
        Err(())
    } else {
        Ok(false)
    }
}

impl Class {
    fn matches(&self, c: char) -> Result<bool, ()> {
        let mut hit = false;
        for item in &self.items {
            let m = match *item {
                Item::Range(a, b) => a <= c && c <= b,
                Item::Space(pos) => matches!(c, ' ' | '\t' | '\n' | '\r') == pos,
                Item::Digit(pos) => digit(c)? == pos,
            };
            if m {
                hit = true;
                break;
            }
        }
        Ok(hit != self.negated)
    }
}

#[derive(Clone, Debug)]
enum Node {
    Class(Class, usize),
    Split(usize, usize),
    Jump(usize),
    Match,
}

/// A compiled pattern facet, with its text for messages.
#[derive(Clone, Debug)]
pub(crate) struct Pattern {
    pub(crate) source: String,
    nodes: Vec<Node>,
    start: usize,
}

#[derive(Clone, Debug)]
enum Ast {
    Class(Class),
    Concat(Vec<Ast>),
    Alt(Vec<Ast>),
    Repeat(Box<Ast>, u32, Option<u32>),
}

struct Parser<'a> {
    s: &'a [char],
    i: usize,
}

const MAX_COUNT: u32 = 100;

impl Parser<'_> {
    fn peek(&self) -> Option<char> {
        self.s.get(self.i).copied()
    }

    fn alt(&mut self) -> Result<Ast, String> {
        let mut branches = vec![self.branch()?];
        while self.peek() == Some('|') {
            self.i += 1;
            branches.push(self.branch()?);
        }
        Ok(if branches.len() == 1 {
            branches.remove(0)
        } else {
            Ast::Alt(branches)
        })
    }

    fn branch(&mut self) -> Result<Ast, String> {
        let mut pieces = Vec::new();
        while let Some(c) = self.peek() {
            if c == '|' || c == ')' {
                break;
            }
            let atom = self.atom()?;
            pieces.push(self.quantifier(atom)?);
        }
        Ok(Ast::Concat(pieces))
    }

    fn quantifier(&mut self, atom: Ast) -> Result<Ast, String> {
        let (min, max) = match self.peek() {
            Some('?') => (0, Some(1)),
            Some('*') => (0, None),
            Some('+') => (1, None),
            Some('{') => {
                self.i += 1;
                let min = self.number()?;
                let max = if self.peek() == Some(',') {
                    self.i += 1;
                    if self.peek() == Some('}') {
                        None
                    } else {
                        Some(self.number()?)
                    }
                } else {
                    Some(min)
                };
                if self.peek() != Some('}') {
                    return Err("malformed quantifier".into());
                }
                if max.is_some_and(|m| m < min) {
                    return Err("quantifier maximum below its minimum".into());
                }
                if min > MAX_COUNT || max.is_some_and(|m| m > MAX_COUNT) {
                    return Err(format!("quantifier bound above {MAX_COUNT}"));
                }
                (min, max)
            }
            _ => return Ok(atom),
        };
        self.i += 1;
        Ok(Ast::Repeat(Box::new(atom), min, max))
    }

    fn number(&mut self) -> Result<u32, String> {
        let start = self.i;
        while self.peek().is_some_and(|c| c.is_ascii_digit()) {
            self.i += 1;
        }
        let digits: String = self.s[start..self.i].iter().collect();
        digits
            .parse()
            .map_err(|_| "malformed quantifier".to_string())
    }

    fn atom(&mut self) -> Result<Ast, String> {
        let c = self.peek().ok_or("unexpected end")?;
        self.i += 1;
        match c {
            '(' => {
                let inner = self.alt()?;
                if self.peek() != Some(')') {
                    return Err("unclosed group".into());
                }
                self.i += 1;
                Ok(inner)
            }
            '[' => Ok(Ast::Class(self.class_expr()?)),
            '.' => Ok(Ast::Class(Class {
                negated: true,
                items: vec![Item::Range('\n', '\n'), Item::Range('\r', '\r')],
            })),
            '\\' => Ok(Ast::Class(self.escape()?)),
            '?' | '*' | '+' | '{' | '}' | ']' => Err(format!("unexpected '{c}'")),
            _ => Ok(Ast::Class(Class {
                negated: false,
                items: vec![Item::Range(c, c)],
            })),
        }
    }

    /// After a backslash.
    fn escape(&mut self) -> Result<Class, String> {
        let c = self.peek().ok_or("unexpected end")?;
        self.i += 1;
        let single = |ch| Class {
            negated: false,
            items: vec![Item::Range(ch, ch)],
        };
        Ok(match c {
            'n' => single('\n'),
            'r' => single('\r'),
            't' => single('\t'),
            '\\' | '|' | '.' | '-' | '^' | '?' | '*' | '+' | '{' | '}' | '(' | ')' | '[' | ']' => {
                single(c)
            }
            's' => Class {
                negated: false,
                items: vec![Item::Space(true)],
            },
            'S' => Class {
                negated: false,
                items: vec![Item::Space(false)],
            },
            'd' => Class {
                negated: false,
                items: vec![Item::Digit(true)],
            },
            'D' => Class {
                negated: false,
                items: vec![Item::Digit(false)],
            },
            _ => return Err(format!("escape '\\{c}'")),
        })
    }

    /// After `[`.
    fn class_expr(&mut self) -> Result<Class, String> {
        let negated = self.peek() == Some('^');
        if negated {
            self.i += 1;
        }
        let mut items = Vec::new();
        let mut first = true;
        loop {
            let c = self.peek().ok_or("unclosed class")?;
            if c == ']' && !first {
                self.i += 1;
                break;
            }
            if c == '[' {
                return Err("nested class".into());
            }
            if c == '-' && !first && self.s.get(self.i + 1) == Some(&'[') {
                return Err("class subtraction".into());
            }
            first = false;
            self.i += 1;
            let low = if c == '\\' {
                let class = self.escape()?;
                match class.items.as_slice() {
                    [Item::Range(a, b)] if a == b => *a,
                    _ => {
                        items.extend(class.items);
                        continue;
                    }
                }
            } else {
                c
            };
            if self.peek() == Some('-')
                && self
                    .s
                    .get(self.i + 1)
                    .is_some_and(|&n| n != ']' && n != '[')
            {
                self.i += 1;
                let h = self.peek().ok_or("unclosed class")?;
                self.i += 1;
                let high = if h == '\\' {
                    let class = self.escape()?;
                    match class.items.as_slice() {
                        [Item::Range(a, b)] if a == b => *a,
                        _ => return Err("class range bound".into()),
                    }
                } else {
                    h
                };
                if high < low {
                    return Err("class range out of order".into());
                }
                items.push(Item::Range(low, high));
            } else {
                items.push(Item::Range(low, low));
            }
        }
        Ok(Class { negated, items })
    }
}

struct Builder {
    nodes: Vec<Node>,
}

/// A fragment: its entry and the node indices whose out-edge is open
/// (filled with the next fragment's entry).
struct Frag {
    start: usize,
    outs: Vec<usize>,
}

const OPEN: usize = usize::MAX;

impl Builder {
    fn push(&mut self, n: Node) -> usize {
        self.nodes.push(n);
        self.nodes.len() - 1
    }

    fn patch(&mut self, outs: &[usize], to: usize) {
        for &o in outs {
            match &mut self.nodes[o] {
                Node::Class(_, next) | Node::Jump(next) => {
                    if *next == OPEN {
                        *next = to;
                    }
                }
                Node::Split(a, b) => {
                    if *a == OPEN {
                        *a = to;
                    } else if *b == OPEN {
                        *b = to;
                    }
                }
                Node::Match => {}
            }
        }
    }

    fn empty(&mut self) -> Frag {
        let j = self.push(Node::Jump(OPEN));
        Frag {
            start: j,
            outs: vec![j],
        }
    }

    fn build(&mut self, ast: &Ast) -> Frag {
        match ast {
            Ast::Class(c) => {
                let n = self.push(Node::Class(c.clone(), OPEN));
                Frag {
                    start: n,
                    outs: vec![n],
                }
            }
            Ast::Concat(parts) => {
                let mut frag = self.empty();
                for p in parts {
                    let next = self.build(p);
                    self.patch(&frag.outs, next.start);
                    frag.outs = next.outs;
                }
                frag
            }
            Ast::Alt(branches) => {
                let mut outs = Vec::new();
                let mut start = None;
                for b in branches.iter().rev() {
                    let f = self.build(b);
                    outs.extend(f.outs);
                    start = Some(match start {
                        None => f.start,
                        Some(rest) => self.push(Node::Split(f.start, rest)),
                    });
                }
                match start {
                    Some(start) => Frag { start, outs },
                    None => self.empty(),
                }
            }
            Ast::Repeat(inner, min, max) => {
                let mut frag = self.empty();
                for _ in 0..*min {
                    let f = self.build(inner);
                    self.patch(&frag.outs, f.start);
                    frag.outs = f.outs;
                }
                match max {
                    None => {
                        let f = self.build(inner);
                        let split = self.push(Node::Split(f.start, OPEN));
                        self.patch(&f.outs, split);
                        self.patch(&frag.outs, split);
                        frag.outs = vec![split];
                    }
                    Some(max) => {
                        let mut opens = Vec::new();
                        for _ in *min..*max {
                            let f = self.build(inner);
                            let split = self.push(Node::Split(f.start, OPEN));
                            self.patch(&frag.outs, split);
                            opens.push(split);
                            frag.outs = f.outs;
                        }
                        frag.outs.extend(opens);
                    }
                }
                frag
            }
        }
    }
}

impl Pattern {
    /// Compiles a `pattern` facet's value; `Err` names what is not
    /// supported or malformed.
    pub(crate) fn compile(source: &str) -> Result<Pattern, String> {
        let chars: Vec<char> = source.chars().collect();
        let mut p = Parser { s: &chars, i: 0 };
        let ast = p.alt()?;
        if p.i != chars.len() {
            return Err(format!("unexpected '{}'", chars[p.i]));
        }
        let mut b = Builder { nodes: Vec::new() };
        let frag = b.build(&ast);
        let m = b.push(Node::Match);
        b.patch(&frag.outs, m);
        Ok(Pattern {
            source: source.to_owned(),
            nodes: b.nodes,
            start: frag.start,
        })
    }

    fn add(&self, set: &mut Vec<usize>, seen: &mut [bool], n: usize) {
        let mut stack = vec![n];
        while let Some(n) = stack.pop() {
            if n == OPEN || seen[n] {
                continue;
            }
            seen[n] = true;
            match self.nodes[n] {
                Node::Jump(next) => stack.push(next),
                Node::Split(a, b) => {
                    stack.push(b);
                    stack.push(a);
                }
                _ => set.push(n),
            }
        }
    }

    /// Whether the whole of `value` matches. `Err` when a character's
    /// class needs Unicode data this crate does not have (a non-ASCII
    /// digit against `\d`).
    pub(crate) fn matches(&self, value: &str) -> Result<bool, ()> {
        let mut current = Vec::new();
        let mut seen = vec![false; self.nodes.len()];
        self.add(&mut current, &mut seen, self.start);
        for c in value.chars() {
            let mut next = Vec::new();
            let mut seen = vec![false; self.nodes.len()];
            for &n in &current {
                if let Node::Class(class, to) = &self.nodes[n]
                    && class.matches(c)?
                {
                    self.add(&mut next, &mut seen, *to);
                }
            }
            if next.is_empty() {
                return Ok(false);
            }
            current = next;
        }
        Ok(current
            .iter()
            .any(|&n| matches!(self.nodes[n], Node::Match)))
    }
}

#[cfg(test)]
mod tests {
    use super::Pattern;

    fn m(p: &str, v: &str) -> bool {
        Pattern::compile(p).unwrap().matches(v).unwrap()
    }

    #[test]
    fn schema_patterns() {
        assert!(m(r"x-[^\s]+", "x-1"));
        assert!(!m(r"x-[^\s]+", "x-"));
        assert!(!m(r"x-[^\s]+", "x-a b"));
        assert!(m(r"(text|image)(/.+)*", "image/png/x"));
        assert!(!m(r"(text|image)(/.+)*", "text/"));
        assert!(m(r"(-?\d+|#);(-?\d+|#)", "-1;#"));
        assert!(!m(r"(-?\d+|#);(-?\d+|#)", "1;2;3"));
        assert!(m(r"[^\s:]+:[^\s:]+", "a:b"));
        assert!(!m(r"[^\s:]+:[^\s:]+", "a:b:c"));
        assert!(m("a{2,3}", "aaa"));
        assert!(!m("a{2,3}", "aaaa"));
        assert!(m("[a-c-]x", "-x"));
    }

    #[test]
    fn refused() {
        for p in [r"\i\c*", r"\p{L}", "[a-z-[aeiou]]", r"\w", "a{1,1000}"] {
            assert!(Pattern::compile(p).is_err(), "{p}");
        }
    }

    #[test]
    fn linear_on_nested_quantifiers() {
        // `.` excludes a newline: every split of the slashes is explored
        // before the value fails, which a backtracking matcher pays for.
        let v = format!(
            "text{}
",
            "/a".repeat(5000)
        );
        assert!(!m(r"(text|image)(/.+)*", &v));
        assert!(m(
            r"(text|image)(/.+)*",
            &format!("text{}!", "/a".repeat(5000))
        ));
    }
}
