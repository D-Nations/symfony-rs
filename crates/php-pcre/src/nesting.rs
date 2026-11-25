//! Lowers the group nesting of deep patterns before translation:
//! `fancy-regex` parses at most 64 nested groups where PCRE accepts 250,
//! and `Gitignore::toRegex` nests one group per negated line. A branch
//! made of zero-width lookarounds followed by a plain `(?:...)` group is
//! distributed over that group's alternatives (`(?!a)(?:b|c)` becomes
//! `(?!a)b|(?!a)c`), which matches the same text in the same order
//! because the lookarounds consume nothing and capture nothing. Patterns
//! with inline options, `\Q`, or nesting below the limit are left alone.
//!
//! Upstream: none

/// Depth from which patterns are rewritten.
const LIMIT: usize = 48;

#[derive(Debug, Clone)]
enum Node {
    Text(Vec<u8>),
    Group {
        open: Vec<u8>,
        branches: Vec<Vec<Node>>,
        /// `)` and its quantifier
        close: Vec<u8>,
    },
}

struct Parser<'a> {
    p: &'a [u8],
    pos: usize,
}

impl Parser<'_> {
    fn branches(&mut self, depth: usize, max: &mut usize) -> Option<Vec<Vec<Node>>> {
        *max = (*max).max(depth);
        let mut branches = vec![Vec::new()];
        while self.pos < self.p.len() {
            let c = self.p[self.pos];
            match c {
                b'|' => {
                    self.pos += 1;
                    branches.push(Vec::new());
                }
                b')' => return Some(branches),
                b'\\' => {
                    if self.p.get(self.pos + 1) == Some(&b'Q') {
                        return None;
                    }
                    let end = (self.pos + 2).min(self.p.len());
                    push_text(branches.last_mut()?, &self.p[self.pos..end]);
                    self.pos = end;
                }
                b'[' => {
                    let start = self.pos;
                    self.skip_class()?;
                    let text = self.p[start..self.pos].to_vec();
                    push_text(branches.last_mut()?, &text);
                }
                b'(' => {
                    let open = self.group_open()?;
                    let inner = self.branches(depth + 1, max)?;
                    if self.p.get(self.pos) != Some(&b')') {
                        return None;
                    }
                    let start = self.pos;
                    self.pos += 1;
                    self.skip_quantifier();
                    let close = self.p[start..self.pos].to_vec();
                    branches.last_mut()?.push(Node::Group {
                        open,
                        branches: inner,
                        close,
                    });
                }
                _ => {
                    push_text(branches.last_mut()?, &[c]);
                    self.pos += 1;
                }
            }
        }
        Some(branches)
    }

    fn group_open(&mut self) -> Option<Vec<u8>> {
        let rest = &self.p[self.pos..];
        let len = if rest.starts_with(b"(?<=") || rest.starts_with(b"(?<!") {
            4
        } else if rest.starts_with(b"(?:")
            || rest.starts_with(b"(?=")
            || rest.starts_with(b"(?!")
            || rest.starts_with(b"(?>")
        {
            3
        } else if rest.starts_with(b"(?") {
            // named groups, comments, options and the rest: not rewritten
            return None;
        } else {
            1
        };
        self.pos += len;
        Some(rest[..len].to_vec())
    }

    fn skip_class(&mut self) -> Option<()> {
        self.pos += 1;
        if self.p.get(self.pos) == Some(&b'^') {
            self.pos += 1;
        }
        if self.p.get(self.pos) == Some(&b']') {
            self.pos += 1;
        }
        while self.pos < self.p.len() {
            match self.p[self.pos] {
                b'\\' => self.pos += 2,
                b']' => {
                    self.pos += 1;
                    return Some(());
                }
                b'[' if self.p.get(self.pos + 1) == Some(&b':') => {
                    let close = self.p[self.pos..].windows(2).position(|w| w == b":]")?;
                    self.pos += close + 2;
                }
                _ => self.pos += 1,
            }
        }
        None
    }

    fn skip_quantifier(&mut self) {
        match self.p.get(self.pos) {
            Some(b'*' | b'+' | b'?') => self.pos += 1,
            Some(b'{') => {
                let rest = &self.p[self.pos..];
                if let Some(close) = rest.iter().position(|&c| c == b'}') {
                    if rest[1..close]
                        .iter()
                        .all(|c| c.is_ascii_digit() || *c == b',')
                    {
                        self.pos += close + 1;
                    } else {
                        return;
                    }
                } else {
                    return;
                }
            }
            _ => return,
        }
        if matches!(self.p.get(self.pos), Some(b'?' | b'+')) {
            self.pos += 1;
        }
    }
}

fn push_text(branch: &mut Vec<Node>, text: &[u8]) {
    if let Some(Node::Text(t)) = branch.last_mut() {
        t.extend_from_slice(text);
    } else {
        branch.push(Node::Text(text.to_vec()));
    }
}

fn is_lookaround(open: &[u8]) -> bool {
    matches!(open, b"(?=" | b"(?!" | b"(?<=" | b"(?<!")
}

fn has_capture(branches: &[Vec<Node>]) -> bool {
    branches.iter().flatten().any(|n| match n {
        Node::Text(_) => false,
        Node::Group { open, branches, .. } => open == b"(" || has_capture(branches),
    })
}

fn flatten(branches: Vec<Vec<Node>>) -> Vec<Vec<Node>> {
    let mut out = Vec::new();
    for branch in branches {
        let branch: Vec<Node> = branch
            .into_iter()
            .map(|n| match n {
                Node::Group {
                    open,
                    branches,
                    close,
                } => Node::Group {
                    open,
                    branches: flatten(branches),
                    close,
                },
                t @ Node::Text(_) => t,
            })
            .collect();
        let distributable = branch.len() >= 2
            && matches!(branch.last(), Some(Node::Group { open, close, .. }) if open == b"(?:" && close == b")")
            && branch[..branch.len() - 1].iter().all(|n| {
                matches!(n, Node::Group { open, branches, close } if is_lookaround(open) && close == b")" && !has_capture(branches))
            });
        if !distributable {
            out.push(branch);
            continue;
        }
        let mut branch = branch;
        let Some(Node::Group {
            branches: inner, ..
        }) = branch.pop()
        else {
            continue;
        };
        for alternative in inner {
            let mut b = branch.clone();
            b.extend(alternative);
            out.push(b);
        }
    }
    out
}

fn write(branches: &[Vec<Node>], out: &mut Vec<u8>) {
    for (i, branch) in branches.iter().enumerate() {
        if i > 0 {
            out.push(b'|');
        }
        for node in branch {
            match node {
                Node::Text(t) => out.extend_from_slice(t),
                Node::Group {
                    open,
                    branches,
                    close,
                } => {
                    out.extend_from_slice(open);
                    write(branches, out);
                    out.extend_from_slice(close);
                }
            }
        }
    }
}

/// The pattern with less nesting, or None to keep it as it is.
pub(super) fn reduce(body: &[u8]) -> Option<Vec<u8>> {
    let mut parser = Parser { p: body, pos: 0 };
    let mut max = 0;
    let branches = parser.branches(0, &mut max)?;
    if max < LIMIT || parser.pos != body.len() {
        return None;
    }
    let mut out = Vec::with_capacity(body.len() * 2);
    write(&flatten(branches), &mut out);
    Some(out)
}
