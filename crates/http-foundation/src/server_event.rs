//! Port of Symfony\Component\HttpFoundation\ServerEvent.php: an event for
//! Server-Sent Events, written as its fields (`: comment`, `id:`,
//! `retry:`, `event:`, `data:` lines) and a blank line.
//!
//! PHP's `getIterator()` yields the head, each data field, then `"\n"`;
//! [`ServerEvent::parts`] gives the same parts in order. The comment, id
//! and type are written when PHP finds them truthy, so `'0'` is left out,
//! as in PHP; the retry when it is above 0.
//!
//! Data is a string or a list of strings. PHP also takes any iterable (a
//! generator is read as the event is written); here a generator's lines
//! are collected first, which writes the same bytes.
//!
//! Symfony source copyright (c) Fabien Potencier; distributed under MIT.
//!
//! Upstream: symfony/http-foundation ServerEvent.php

use php_value::PhpString;

/// `ServerEvent::class`.
pub const CLASS: &str = "Symfony\\Component\\HttpFoundation\\ServerEvent";

/// The event's `data`: `string|iterable<string>`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum EventData {
    /// A string; `''` writes no data field.
    Text(PhpString),
    /// An iterable: one field per item, even an empty one.
    Lines(Vec<PhpString>),
}

impl From<&str> for EventData {
    fn from(text: &str) -> Self {
        Self::Text(PhpString::from(text))
    }
}

impl From<PhpString> for EventData {
    fn from(text: PhpString) -> Self {
        Self::Text(text)
    }
}

impl From<Vec<PhpString>> for EventData {
    fn from(lines: Vec<PhpString>) -> Self {
        Self::Lines(lines)
    }
}

/// `Symfony\Component\HttpFoundation\ServerEvent`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ServerEvent {
    data: EventData,
    kind: Option<PhpString>,
    retry: Option<i64>,
    id: Option<PhpString>,
    comment: Option<PhpString>,
}

/// PHP's truthiness of a `?string`: not null, `''` or `'0'`.
fn truthy(value: Option<&PhpString>) -> Option<&PhpString> {
    value.filter(|v| !matches!(v.as_bytes(), b"" | b"0"))
}

impl ServerEvent {
    /// `new ServerEvent($data, $type, $retry, $id, $comment)`.
    pub fn new(
        data: impl Into<EventData>,
        kind: Option<PhpString>,
        retry: Option<i64>,
        id: Option<PhpString>,
        comment: Option<PhpString>,
    ) -> Self {
        Self {
            data: data.into(),
            kind,
            retry,
            id,
            comment,
        }
    }

    /// `new ServerEvent($data)`.
    pub fn data(data: impl Into<EventData>) -> Self {
        Self::new(data, None, None, None, None)
    }

    /// `getData()`.
    pub fn get_data(&self) -> &EventData {
        &self.data
    }

    /// `setData($data)`.
    pub fn set_data(&mut self, data: impl Into<EventData>) -> &mut Self {
        self.data = data.into();
        self
    }

    /// `getType()`.
    pub fn get_type(&self) -> Option<&PhpString> {
        self.kind.as_ref()
    }

    /// `setType($type)`.
    pub fn set_type(&mut self, kind: impl Into<PhpString>) -> &mut Self {
        self.kind = Some(kind.into());
        self
    }

    /// `getRetry()`.
    pub fn get_retry(&self) -> Option<i64> {
        self.retry
    }

    /// `setRetry($retry)`.
    pub fn set_retry(&mut self, retry: Option<i64>) -> &mut Self {
        self.retry = retry;
        self
    }

    /// `getId()`.
    pub fn get_id(&self) -> Option<&PhpString> {
        self.id.as_ref()
    }

    /// `setId($id)`.
    pub fn set_id(&mut self, id: impl Into<PhpString>) -> &mut Self {
        self.id = Some(id.into());
        self
    }

    /// `getComment()`.
    pub fn get_comment(&self) -> Option<&PhpString> {
        self.comment.as_ref()
    }

    /// `setComment($comment)`.
    pub fn set_comment(&mut self, comment: impl Into<PhpString>) -> &mut Self {
        self.comment = Some(comment.into());
        self
    }

    /// `getIterator()`: the head, each data field, then `"\n"`.
    pub fn parts(&self) -> Vec<PhpString> {
        let mut head = Vec::new();
        if let Some(comment) = truthy(self.comment.as_ref()) {
            head.extend_from_slice(&field(b": ", comment.as_bytes()));
        }
        if let Some(id) = truthy(self.id.as_ref()) {
            head.extend_from_slice(b"id: ");
            head.extend_from_slice(&single_line(id.as_bytes()));
            head.push(b'\n');
        }
        if let Some(retry) = self.retry.filter(|r| *r > 0) {
            head.extend_from_slice(format!("retry: {retry}\n").as_bytes());
        }
        if let Some(kind) = truthy(self.kind.as_ref()) {
            head.extend_from_slice(b"event: ");
            head.extend_from_slice(&single_line(kind.as_bytes()));
            head.push(b'\n');
        }
        let mut parts = vec![PhpString::from(head)];

        match &self.data {
            EventData::Lines(lines) => {
                for line in lines {
                    parts.push(PhpString::from(field(b"data: ", line.as_bytes())));
                }
            }
            EventData::Text(text) if !text.as_bytes().is_empty() => {
                parts.push(PhpString::from(field(b"data: ", text.as_bytes())));
            }
            EventData::Text(_) => {}
        }

        parts.push(PhpString::from("\n"));
        parts
    }
}

/// `field($prefix, $value)`: one prefixed line per line of the value
/// (`preg_split("/\r\n|[\r\n]/", $value)`).
fn field(prefix: &[u8], value: &[u8]) -> Vec<u8> {
    let mut out = prefix.to_vec();
    let mut at = 0;
    while at < value.len() {
        match value[at] {
            b'\r' | b'\n' => {
                let width = if value[at..].starts_with(b"\r\n") {
                    2
                } else {
                    1
                };
                out.push(b'\n');
                out.extend_from_slice(prefix);
                at += width;
            }
            byte => {
                out.push(byte);
                at += 1;
            }
        }
    }
    out.push(b'\n');
    out
}

/// `singleLine($value)`: `\r` and `\n` removed.
fn single_line(value: &[u8]) -> Vec<u8> {
    value
        .iter()
        .copied()
        .filter(|b| !matches!(b, b'\r' | b'\n'))
        .collect()
}
