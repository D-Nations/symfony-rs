//! Port of Symfony\UX\TwigComponent\Twig\TwigPreLexer.php (trial subset):
//! rewrites self-closing `<twig:Name attr="..." :prop="expr"/>` tags into
//! `{{ component('Name', { ... }) }}`, as ComponentLexer does before
//! Twig's lexer runs. Comments and `{{ }}`/`{% %}` contents are copied
//! unchanged. Refused with a location: components with content
//! (`<twig:X>...</twig:X>`), `<twig:block>`, `<twig:component is=...>`,
//! `{{...}}` attribute spreading and `verbatim`.
//!
//! Upstream: symfony/ux-twig-component src/Twig/TwigPreLexer.php src/Twig/ComponentLexer.php

use crate::Error;

struct PreLexer<'a> {
    name: &'a str,
    input: Vec<u8>,
    pos: usize,
    line: usize,
}

pub fn pre_lex(name: &str, source: &str) -> Result<String, Error> {
    if !source.contains("<twig:") {
        return Ok(source.to_owned());
    }
    let input = source
        .replace("\r\n", "\n")
        .replace('\r', "\n")
        .into_bytes();
    let mut lexer = PreLexer {
        name,
        input,
        pos: 0,
        line: 1,
    };
    let out = lexer.run()?;
    String::from_utf8(out).map_err(|_| Error::syntax("Invalid UTF-8 after pre-lexing.", name, 1))
}

impl PreLexer<'_> {
    fn err(&self, message: impl Into<String>) -> Error {
        Error::syntax(message, self.name, self.line)
    }

    fn check(&self, s: &str) -> bool {
        self.input[self.pos..].starts_with(s.as_bytes())
    }

    fn consume(&mut self, s: &str) -> bool {
        if self.check(s) {
            self.pos += s.len();
            true
        } else {
            false
        }
    }

    fn consume_until(&mut self, end: &str) -> Vec<u8> {
        let rest = &self.input[self.pos..];
        let len = rest
            .windows(end.len())
            .position(|w| w == end.as_bytes())
            .unwrap_or(rest.len());
        let content = rest[..len].to_vec();
        self.line += content.iter().filter(|b| **b == b'\n').count();
        self.pos += len;
        content
    }

    fn consume_whitespace(&mut self) {
        while self.pos < self.input.len() && b" \t\n\r\0\x0B".contains(&self.input[self.pos]) {
            if self.input[self.pos] == b'\n' {
                self.line += 1;
            }
            self.pos += 1;
        }
        if self.consume("#") {
            self.consume_until("\n");
            self.consume_whitespace();
        }
    }

    fn consume_component_name(&mut self, message: &str) -> Result<String, Error> {
        let start = self.pos;
        while self.pos < self.input.len()
            && (self.input[self.pos].is_ascii_alphanumeric()
                || b"_:@-.".contains(&self.input[self.pos]))
        {
            self.pos += 1;
        }
        if self.pos == start {
            return Err(self.err(message));
        }
        Ok(String::from_utf8_lossy(&self.input[start..self.pos]).into_owned())
    }

    fn expect_char(&mut self, c: u8) -> Result<(), Error> {
        match self.input.get(self.pos) {
            None => Err(self.err(format!(
                "Expected '{}' but reached the end of the file.",
                c as char
            ))),
            Some(b) if *b != c => Err(self.err(format!(
                "Expected '{}' but found '{}'.",
                c as char, *b as char
            ))),
            Some(_) => {
                self.pos += 1;
                Ok(())
            }
        }
    }

    fn run(&mut self) -> Result<Vec<u8>, Error> {
        let mut out = Vec::new();
        while self.pos < self.input.len() {
            let c = self.input[self.pos];
            if c != b'{' && c != b'<' {
                let len = self.input[self.pos..]
                    .iter()
                    .position(|b| *b == b'{' || *b == b'<')
                    .unwrap_or(self.input.len() - self.pos);
                let text = &self.input[self.pos..self.pos + len];
                self.line += text.iter().filter(|b| **b == b'\n').count();
                out.extend_from_slice(text);
                self.pos += len;
                continue;
            }
            if self.check("{%") {
                let rest = String::from_utf8_lossy(&self.input[self.pos + 2..]);
                if rest
                    .trim_start_matches(['-', '~'])
                    .trim_start()
                    .starts_with("verbatim")
                {
                    return Err(
                        self.err("The Twig trial compiler does not support the \"verbatim\" tag.")
                    );
                }
            }
            if self.consume("{#") {
                out.extend_from_slice(b"{#");
                out.extend(self.consume_until("#}"));
                if self.consume("#}") {
                    out.extend_from_slice(b"#}");
                }
                continue;
            }
            let mut tag = false;
            for (open, close) in [("{{", "}}"), ("{%", "%}")] {
                if !tag && self.consume(open) {
                    tag = true;
                    out.extend_from_slice(open.as_bytes());
                    // consumeTwigExpressionContent(): the trial skips
                    // quoted strings so a "}}" inside one does not end it.
                    out.extend(self.consume_expression(close));
                    if self.consume(close) {
                        out.extend_from_slice(close.as_bytes());
                    }
                }
            }
            if tag {
                continue;
            }
            if self.consume("<twig:") {
                let name = self.consume_component_name(
                    "Expected component name when resolving the \"<twig:\" syntax.",
                )?;
                if name == "block" || name.eq_ignore_ascii_case("component") {
                    return Err(self.err(format!(
                        "The Twig trial compiler does not support \"<twig:{name}>\"."
                    )));
                }
                let attributes = self.consume_attributes(&name)?;
                if !self.consume("/>") {
                    return Err(self.err(format!(
                        "The Twig trial compiler does not support components with content (\"<twig:{name}>\")."
                    )));
                }
                let attributes = if attributes.is_empty() {
                    String::new()
                } else {
                    format!(", {{ {attributes} }}")
                };
                out.extend(format!("{{{{ component('{name}'{attributes}) }}}}").into_bytes());
                continue;
            }
            out.push(c);
            self.pos += 1;
        }
        Ok(out)
    }

    fn consume_expression(&mut self, close: &str) -> Vec<u8> {
        let start = self.pos;
        let mut quote = None;
        while self.pos < self.input.len() {
            let b = self.input[self.pos];
            match quote {
                Some(_) if b == b'\\' => self.pos += 1,
                Some(q) if b == q => quote = None,
                Some(_) => {}
                None if b == b'\'' || b == b'"' => quote = Some(b),
                None if self.check(close) => break,
                None => {}
            }
            if b == b'\n' {
                self.line += 1;
            }
            self.pos += 1;
        }
        self.pos = self.pos.min(self.input.len());
        self.input[start..self.pos].to_vec()
    }

    /// TwigPreLexer::consumeAttributes().
    fn consume_attributes(&mut self, component: &str) -> Result<String, Error> {
        let mut attributes = Vec::new();
        while self.pos < self.input.len() && !self.check(">") && !self.check("/>") {
            self.consume_whitespace();
            if self.check(">") || self.check("/>") {
                break;
            }
            if self.check("{{...") || self.check("{{ ...") {
                return Err(
                    self.err("The Twig trial compiler does not support attribute spreading.")
                );
            }
            let dynamic = self.consume(":");
            let message =
                format!("Expected attribute name when parsing the \"<twig:{component}\" syntax.");
            let key = self.consume_component_name(&message)?;
            let quoted_key = if key.contains(['-', ':', '@']) {
                format!("'{key}'")
            } else {
                key.clone()
            };
            if !self.check("=") {
                self.consume_whitespace();
                if dynamic {
                    return Err(self.err(format!(
                        "Expected \"=\" after \":{key}\" when parsing the \"<twig:{component}\" syntax."
                    )));
                }
                attributes.push(format!("{quoted_key}: true"));
                self.consume_whitespace();
                continue;
            }
            self.expect_char(b'=')?;
            let quote = match self.input.get(self.pos) {
                Some(q @ (b'\'' | b'"')) => *q,
                Some(other) => {
                    return Err(self.err(format!(
                        "Expected one of [.'\"] but found '{}'.",
                        *other as char
                    )));
                }
                None => return Err(self.err("Unexpected end of input.")),
            };
            self.pos += 1;
            let value = if dynamic {
                String::from_utf8_lossy(&self.consume_until(&(quote as char).to_string()))
                    .into_owned()
            } else {
                self.consume_attribute_value(quote)?
            };
            attributes.push(format!(
                "{quoted_key}: {}",
                if value.is_empty() {
                    "''".to_owned()
                } else {
                    value
                }
            ));
            self.expect_char(quote)?;
            self.consume_whitespace();
        }
        Ok(attributes.join(", "))
    }

    /// TwigPreLexer::consumeAttributeValue().
    fn consume_attribute_value(&mut self, quote: u8) -> Result<String, Error> {
        let mut parts = Vec::new();
        let mut current = Vec::new();
        let literal =
            |bytes: &[u8]| format!("'{}'", String::from_utf8_lossy(bytes).replace('\'', "\\'"));
        while self.pos < self.input.len() && self.input[self.pos] != quote {
            if self.input[self.pos] == b'\n' {
                self.line += 1;
            }
            if self.consume("{{") {
                if !current.is_empty() {
                    parts.push(literal(&current));
                    current.clear();
                }
                self.consume_whitespace();
                let expr = String::from_utf8_lossy(&self.consume_until("}}"))
                    .trim_end()
                    .to_owned();
                parts.push(format!("({expr})"));
                self.expect_char(b'}')?;
                self.expect_char(b'}')?;
                continue;
            }
            current.push(self.input[self.pos]);
            self.pos += 1;
        }
        if !current.is_empty() {
            parts.push(literal(&current));
        }
        Ok(parts.join("~"))
    }
}
