//! Port of Twig\Lexer.php (trial subset).
//!
//! Supported: text, `{{ }}`, `{% %}`, `{# #}` (and `{## ##}` documentation
//! comments, read as comments), the `-` and `~` whitespace modifiers,
//! names, numbers, single- and double-quoted strings without
//! interpolation, punctuation and the core operator set. Refused with a
//! location: `verbatim`, `line`, string interpolation (`#{`).
//!
//! Upstream: twig/twig src/Lexer.php

use crate::{Error, Number};

#[derive(Debug, Clone, PartialEq)]
pub enum TokenKind {
    Text(String),
    VarStart,
    VarEnd,
    BlockStart,
    BlockEnd,
    Name(String),
    Number(Number),
    String(String),
    Operator(String),
    Punctuation(char),
    Eof,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Token {
    pub kind: TokenKind,
    pub line: usize,
}

impl Token {
    /// `Token::toEnglish()` plus the value, for messages.
    pub fn describe(&self) -> (&'static str, String) {
        match &self.kind {
            TokenKind::Text(s) => ("text", s.clone()),
            TokenKind::VarStart => ("begin of print statement", String::new()),
            TokenKind::VarEnd => ("end of print statement", String::new()),
            TokenKind::BlockStart => ("begin of statement block", String::new()),
            TokenKind::BlockEnd => ("end of statement block", String::new()),
            TokenKind::Name(s) => ("name", s.clone()),
            TokenKind::Number(n) => ("number", n.to_string()),
            TokenKind::String(s) => ("string", s.clone()),
            TokenKind::Operator(s) => ("operator", s.clone()),
            TokenKind::Punctuation(c) => ("punctuation", c.to_string()),
            TokenKind::Eof => ("end of template", String::new()),
        }
    }
}

/// The operator tokens of CoreExtension::getExpressionParsers() (names
/// and aliases). The real lexer builds this list from the registered
/// extensions; the trial fixes it to the core set.
const OPERATORS: &[&str] = &[
    "not",
    "...",
    "-",
    "+",
    "?:",
    "? :",
    "??",
    "or",
    "xor",
    "and",
    "b-or",
    "b-xor",
    "b-and",
    "==",
    "!=",
    "<=>",
    "<",
    ">",
    ">=",
    "<=",
    "not in",
    "in",
    "matches",
    "starts with",
    "ends with",
    "has some",
    "has every",
    "===",
    "!==",
    "..",
    "~",
    "*",
    "/",
    "//",
    "%",
    "**",
    "?",
    "=",
    "is",
    "is not",
    "|",
    "(",
    ".",
    "?.",
    "[",
    "=>",
];

const PUNCTUATION: &str = "()[]{}?:.,|";
const LINE_CHARS: &[u8] = b" \t\0\x0B";

#[derive(Clone, Copy, PartialEq)]
enum State {
    Data,
    Block,
    Var,
}

struct Lexer<'a> {
    name: &'a str,
    code: &'a [u8],
    cursor: usize,
    line: usize,
    tokens: Vec<Token>,
    brackets: Vec<(char, usize)>,
    var_line: usize,
}

fn is_space(b: u8) -> bool {
    // PCRE's \s: space, \t, \n, \v, \f, \r
    matches!(b, b' ' | b'\t' | b'\n' | 0x0B | 0x0C | b'\r')
}

fn is_name_start(b: u8) -> bool {
    b.is_ascii_alphabetic() || b == b'_' || b >= 0x7f
}

fn is_name_char(b: u8) -> bool {
    is_name_start(b) || b.is_ascii_digit()
}

fn normalize_newlines(s: &str) -> String {
    s.replace("\r\n", "\n").replace('\r', "\n")
}

pub fn tokenize(name: &str, source: &str) -> Result<Vec<Token>, Error> {
    let mut lexer = Lexer {
        name,
        code: source.as_bytes(),
        cursor: 0,
        line: 1,
        tokens: Vec::new(),
        brackets: Vec::new(),
        var_line: 1,
    };
    lexer.run()?;
    Ok(lexer.tokens)
}

impl Lexer<'_> {
    fn err(&self, message: impl Into<String>, line: usize) -> Error {
        Error::syntax(message, self.name, line)
    }

    fn text(&self, from: usize, to: usize) -> &str {
        // The source is a &str and every cut is at an ASCII byte.
        std::str::from_utf8(&self.code[from..to]).unwrap_or_default()
    }

    fn advance(&mut self, len: usize) {
        let chunk = &self.code[self.cursor..self.cursor + len];
        let text = String::from_utf8_lossy(chunk);
        self.line += normalize_newlines(&text).matches('\n').count();
        self.cursor += len;
    }

    fn push(&mut self, kind: TokenKind, line: usize) {
        if let TokenKind::Text(t) = &kind
            && t.is_empty()
        {
            return;
        }
        self.tokens.push(Token { kind, line });
    }

    fn at(&self, s: &str) -> bool {
        self.code[self.cursor..].starts_with(s.as_bytes())
    }

    fn run(&mut self) -> Result<(), Error> {
        let mut state = State::Data;
        while self.cursor < self.code.len() {
            state = match state {
                State::Data => self.lex_data()?,
                State::Block | State::Var => self.lex_inside(state)?,
            };
        }
        if state != State::Data {
            let what = if state == State::Block {
                "block"
            } else {
                "variable"
            };
            return Err(self.err(format!("Unclosed \"{what}\"."), self.var_line));
        }
        let line = self.line;
        self.push(TokenKind::Eof, line);
        if let Some((expect, line)) = self.brackets.pop() {
            return Err(self.err(format!("Unclosed \"{expect}\"."), line));
        }
        Ok(())
    }

    /// The next `{{`, `{%` or `{#` at or after the cursor, with its
    /// whitespace modifier.
    fn next_tag(&self) -> Option<(usize, &'static str, Option<u8>)> {
        let mut i = self.cursor;
        while i + 1 < self.code.len() {
            if self.code[i] == b'{' {
                let kind = match self.code[i + 1] {
                    b'{' => Some("{{"),
                    b'%' => Some("{%"),
                    b'#' => Some("{#"),
                    _ => None,
                };
                if let Some(kind) = kind {
                    let modifier = self
                        .code
                        .get(i + 2)
                        .copied()
                        .filter(|b| *b == b'-' || *b == b'~');
                    return Some((i, kind, modifier));
                }
            }
            i += 1;
        }
        None
    }

    fn lex_data(&mut self) -> Result<State, Error> {
        let Some((pos, kind, modifier)) = self.next_tag() else {
            let text = normalize_newlines(self.text(self.cursor, self.code.len()));
            let line = self.line;
            self.push(TokenKind::Text(text), line);
            self.advance(self.code.len() - self.cursor);
            return Ok(State::Data);
        };
        let raw = self.text(self.cursor, pos).to_owned();
        let trimmed = match modifier {
            Some(b'-') => raw.trim_end_matches(|c: char| c.is_ascii() && is_space(c as u8)),
            Some(_) => {
                raw.trim_end_matches(|c: char| c.is_ascii() && LINE_CHARS.contains(&(c as u8)))
            }
            None => raw.as_str(),
        };
        let line = self.line;
        self.push(TokenKind::Text(normalize_newlines(trimmed)), line);
        self.advance(pos - self.cursor);
        let open = 2 + usize::from(modifier.is_some());
        match kind {
            "{#" => {
                self.advance(open);
                self.lex_comment()?;
                Ok(State::Data)
            }
            "{%" => {
                let line = self.line;
                self.advance(open);
                let rest = self.text(self.cursor, self.code.len()).trim_start();
                for refused in ["verbatim", "line "] {
                    if rest.starts_with(refused) {
                        return Err(self.err(
                            format!(
                                "The Twig trial compiler does not support the \"{}\" tag.",
                                refused.trim()
                            ),
                            line,
                        ));
                    }
                }
                self.push(TokenKind::BlockStart, line);
                self.var_line = self.line;
                Ok(State::Block)
            }
            _ => {
                let line = self.line;
                self.advance(open);
                self.push(TokenKind::VarStart, line);
                self.var_line = self.line;
                Ok(State::Var)
            }
        }
    }

    fn lex_comment(&mut self) -> Result<(), Error> {
        let Some(off) = find(&self.code[self.cursor..], b"#}") else {
            return Err(self.err("Unclosed comment.", self.line));
        };
        let end = self.cursor + off;
        let before = if end > self.cursor {
            self.code[end - 1]
        } else {
            0
        };
        self.advance(off + 2);
        match before {
            b'-' => self.skip_while(is_space, None),
            b'~' => self.skip_while(|b| LINE_CHARS.contains(&b), None),
            _ => self.skip_newline(),
        }
        Ok(())
    }

    fn skip_while(&mut self, pred: impl Fn(u8) -> bool, then_newline: Option<()>) {
        let mut n = 0;
        while self.cursor + n < self.code.len() && pred(self.code[self.cursor + n]) {
            n += 1;
        }
        self.advance(n);
        if then_newline.is_some() && self.at("\n") {
            self.advance(1);
        }
    }

    fn skip_newline(&mut self) {
        if self.at("\r\n") {
            self.advance(2);
        } else if self.at("\n") || self.at("\r") {
            self.advance(1);
        }
    }

    /// The closing `}}`/`%}` with its modifier, after optional whitespace;
    /// returns the length of the whitespace before it.
    fn closing(&self, state: State) -> Option<(usize, Option<u8>)> {
        let close = if state == State::Var { "}}" } else { "%}" };
        let mut ws = 0;
        while self.cursor + ws < self.code.len() && is_space(self.code[self.cursor + ws]) {
            ws += 1;
        }
        let rest = &self.code[self.cursor + ws..];
        for modifier in *b"-~" {
            if rest.first() == Some(&modifier) && rest[1..].starts_with(close.as_bytes()) {
                return Some((ws, Some(modifier)));
            }
        }
        rest.starts_with(close.as_bytes()).then_some((ws, None))
    }

    fn lex_inside(&mut self, state: State) -> Result<State, Error> {
        if self.brackets.is_empty()
            && let Some((ws, modifier)) = self.closing(state)
        {
            self.advance(ws);
            let line = self.line;
            self.push(
                if state == State::Var {
                    TokenKind::VarEnd
                } else {
                    TokenKind::BlockEnd
                },
                line,
            );
            self.advance(2 + usize::from(modifier.is_some()));
            match (modifier, state) {
                (Some(b'-'), State::Block) => self.skip_while(is_space, Some(())),
                (Some(b'-'), _) => self.skip_while(is_space, None),
                (Some(_), _) => self.skip_while(|b| LINE_CHARS.contains(&b), None),
                (None, State::Block) => self.skip_newline(),
                (None, _) => {}
            }
            return Ok(State::Data);
        }
        self.lex_expression(state)?;
        Ok(state)
    }

    fn lex_expression(&mut self, state: State) -> Result<(), Error> {
        let mut ws = 0;
        while self.cursor + ws < self.code.len() && is_space(self.code[self.cursor + ws]) {
            ws += 1;
        }
        if ws > 0 {
            self.advance(ws);
            if self.cursor >= self.code.len() {
                let what = if state == State::Block {
                    "block"
                } else {
                    "variable"
                };
                return Err(self.err(format!("Unclosed \"{what}\"."), self.var_line));
            }
            if self.brackets.is_empty() && self.closing(state).is_some() {
                return Ok(());
            }
        }
        let line = self.line;
        if let Some((op, len)) = self.match_operator() {
            if op == "(" || op == "[" {
                self.check_bracket(op.chars().next().unwrap_or('('))?;
            }
            self.push(TokenKind::Operator(op), line);
            self.advance(len);
            return Ok(());
        }
        let c = self.code[self.cursor];
        if is_name_start(c) {
            let mut n = 1;
            while self.cursor + n < self.code.len() && is_name_char(self.code[self.cursor + n]) {
                n += 1;
            }
            let name = self.text(self.cursor, self.cursor + n).to_owned();
            self.push(TokenKind::Name(name), line);
            self.advance(n);
            return Ok(());
        }
        if c.is_ascii_digit() {
            let (number, len) = self.lex_number()?;
            self.push(TokenKind::Number(number), line);
            self.advance(len);
            return Ok(());
        }
        if PUNCTUATION.as_bytes().contains(&c) {
            self.check_bracket(c as char)?;
            self.push(TokenKind::Punctuation(c as char), line);
            self.advance(1);
            return Ok(());
        }
        if c == b'\'' || c == b'"' {
            let (value, len) = self.lex_string(c)?;
            self.push(TokenKind::String(value), line);
            self.advance(len);
            return Ok(());
        }
        if c == b'#' {
            let mut n = 0;
            while self.cursor + n < self.code.len()
                && !matches!(self.code[self.cursor + n], b'\r' | b'\n')
            {
                n += 1;
            }
            self.advance(n);
            return Ok(());
        }
        let ch = self
            .text(self.cursor, self.code.len())
            .chars()
            .next()
            .unwrap_or('?');
        Err(self.err(format!("Unexpected character \"{ch}\"."), line))
    }

    fn match_operator(&self) -> Option<(String, usize)> {
        let rest = &self.code[self.cursor..];
        let mut best: Option<(String, usize)> = None;
        let mut ops: Vec<&str> = OPERATORS.to_vec();
        ops.sort_by_key(|o| std::cmp::Reverse(o.len()));
        for op in ops {
            let Some(len) = match_spaced(rest, op) else {
                continue;
            };
            let bytes = op.as_bytes();
            if bytes[bytes.len() - 1].is_ascii_alphabetic() {
                let next = rest.get(len).copied();
                if !matches!(next, Some(b) if is_space(b) || b"()[{".contains(&b)) {
                    continue;
                }
            }
            if bytes[0].is_ascii_alphabetic() {
                // (?<![\.\|]\s|.[\.\|])
                let p1 = self.cursor.checked_sub(1).map(|i| self.code[i]);
                let p2 = self.cursor.checked_sub(2).map(|i| self.code[i]);
                let after_dot_space =
                    matches!(p2, Some(b'.' | b'|')) && matches!(p1, Some(b) if is_space(b));
                let after_dot = self.cursor >= 2 && matches!(p1, Some(b'.' | b'|'));
                if after_dot_space || after_dot {
                    continue;
                }
            }
            if best.as_ref().is_none_or(|(_, l)| len > *l) {
                best = Some((op.to_owned(), len));
            }
            break;
        }
        best
    }

    fn lex_number(&self) -> Result<(Number, usize), Error> {
        let code = &self.code[self.cursor..];
        let digits = |from: usize| {
            let mut i = from;
            if !code.get(i).is_some_and(u8::is_ascii_digit) {
                return None;
            }
            while code.get(i).is_some_and(u8::is_ascii_digit)
                || (code.get(i) == Some(&b'_') && code.get(i + 1).is_some_and(u8::is_ascii_digit))
            {
                i += 1;
            }
            Some(i)
        };
        let mut end = digits(0).unwrap_or(1);
        let mut float = false;
        if code.get(end) == Some(&b'.')
            && let Some(e) = digits(end + 1)
        {
            end = e;
            float = true;
        }
        if matches!(code.get(end), Some(b'e' | b'E')) {
            let sign = usize::from(matches!(code.get(end + 1), Some(b'+' | b'-')));
            if let Some(e) = digits(end + 1 + sign) {
                end = e;
                float = true;
            }
        }
        let text: String = self
            .text(self.cursor, self.cursor + end)
            .chars()
            .filter(|c| *c != '_')
            .collect();
        // `0 + "..."` in PHP: an integer unless it has a fraction, an
        // exponent or overflows.
        let number = match (float, text.parse::<i64>()) {
            (false, Ok(i)) => Number::Int(i),
            _ => Number::Float(
                text.parse()
                    .map_err(|_| self.err("Invalid number.", self.line))?,
            ),
        };
        Ok((number, end))
    }

    fn lex_string(&self, quote: u8) -> Result<(String, usize), Error> {
        let code = &self.code[self.cursor..];
        let mut i = 1;
        while i < code.len() {
            match code[i] {
                b'\\' => i += 2,
                b'#' if quote == b'"' && code.get(i + 1) == Some(&b'{') => {
                    return Err(self.err(
                        "The Twig trial compiler does not support string interpolation.",
                        self.line,
                    ));
                }
                b if b == quote => {
                    let inner = normalize_newlines(self.text(self.cursor + 1, self.cursor + i));
                    return Ok((strip_c_slashes(&inner), i + 1));
                }
                _ => i += 1,
            }
        }
        Err(self.err(format!("Unclosed \"{}\".", quote as char), self.line))
    }

    fn check_bracket(&mut self, c: char) -> Result<(), Error> {
        match c {
            '(' | '[' | '{' => self.brackets.push((c, self.line)),
            ')' | ']' | '}' => {
                let Some((expect, line)) = self.brackets.pop() else {
                    return Err(self.err(format!("Unexpected \"{c}\"."), self.line));
                };
                let wanted = match expect {
                    '(' => ')',
                    '[' => ']',
                    _ => '}',
                };
                if c != wanted {
                    return Err(self.err(format!("Unclosed \"{expect}\"."), line));
                }
            }
            _ => {}
        }
        Ok(())
    }
}

/// Matches `op` at the start of `input`, where a space in `op` stands for
/// any run of whitespace (`\s+`). Returns the matched length.
fn match_spaced(input: &[u8], op: &str) -> Option<usize> {
    let mut i = 0;
    for &b in op.as_bytes() {
        if b == b' ' {
            let start = i;
            while input.get(i).is_some_and(|c| is_space(*c)) {
                i += 1;
            }
            if i == start {
                return None;
            }
        } else if input.get(i) == Some(&b) {
            i += 1;
        } else {
            return None;
        }
    }
    Some(i)
}

fn find(haystack: &[u8], needle: &[u8]) -> Option<usize> {
    haystack.windows(needle.len()).position(|w| w == needle)
}

/// Lexer::stripcslashes() for the escapes the trial accepts.
fn strip_c_slashes(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let mut chars = s.chars();
    while let Some(c) = chars.next() {
        if c != '\\' {
            out.push(c);
            continue;
        }
        match chars.next() {
            None => out.push('\\'),
            Some('f') => out.push('\u{c}'),
            Some('n') => out.push('\n'),
            Some('r') => out.push('\r'),
            Some('t') => out.push('\t'),
            Some('v') => out.push('\u{b}'),
            Some(other) => out.push(other),
        }
    }
    out
}
