//! Port of Symfony\Component\Yaml\Dumper.php: PHP values to block YAML,
//! switching to inline YAML ([`Inline::dump`]) at the given level.
//!
//! Symfony source copyright (c) Fabien Potencier; distributed under MIT.
//!
//! Upstream: symfony/yaml Dumper.php

use std::sync::Arc;

use php_value::{
    PhpString, Value,
    object::{ArrayObject, StdClass, to_array},
};

use crate::{
    Environment, Flags,
    exception::{DumpError, InvalidArgumentException},
    inline::{Inline, is_hash},
    tag::TaggedValue,
};

/// PHP truthiness (`!$value` is its negation).
fn truthy(value: &Value) -> bool {
    match value {
        Value::Null => false,
        Value::Bool(b) => *b,
        Value::Int(i) => *i != 0,
        Value::Float(f) => *f != 0.0,
        Value::String(s) => !(s.is_empty() || s.as_bytes() == b"0"),
        Value::Array(a) => !a.is_empty(),
        Value::Object(_) => true,
    }
}

fn as_tagged(value: &Value) -> Option<&TaggedValue> {
    value
        .as_object()
        .and_then(|o| o.downcast_ref::<TaggedValue>())
}

/// `$input instanceof \ArrayObject || $input instanceof \stdClass`.
fn is_map_object(value: &Value) -> bool {
    value.as_object().is_some_and(|o| {
        o.downcast_ref::<ArrayObject>().is_some() || o.downcast_ref::<StdClass>().is_some()
    })
}

/// `\is_string($value) && str_contains($value, "\n") && !str_contains($value, "\r")`.
fn multi_line(value: &Value) -> Option<&[u8]> {
    let s = value.as_string()?.as_bytes();
    (s.contains(&b'\n') && !s.contains(&b'\r')).then_some(s)
}

/// The YAML dumper.
#[derive(Debug, Clone)]
pub struct Dumper {
    indentation: usize,
    inline: Inline,
}

impl Dumper {
    /// `new Dumper($indentation)`: the amount of spaces to use for
    /// indentation of nested nodes (PHP's default is 4).
    pub fn new(indentation: i64) -> Result<Self, InvalidArgumentException> {
        Self::with_environment(Arc::new(Environment::new()), indentation)
    }

    pub fn with_environment(
        environment: Arc<Environment>,
        indentation: i64,
    ) -> Result<Self, InvalidArgumentException> {
        if indentation < 1 {
            return Err(InvalidArgumentException {
                message: "The indentation must be greater than zero.".to_owned(),
            });
        }
        Ok(Self {
            indentation: indentation as usize,
            inline: Inline::new(environment),
        })
    }

    /// Dumps a PHP value to YAML: `inline` is the level where it switches
    /// to inline YAML, `indent` the level of indentation (used
    /// internally).
    pub fn dump(
        &self,
        input: &Value,
        inline: i64,
        indent: usize,
        flags: Flags,
    ) -> Result<PhpString, DumpError> {
        if flags.contains(Flags::DUMP_NULL_AS_EMPTY) && flags.contains(Flags::DUMP_NULL_AS_TILDE) {
            return Err(DumpError::InvalidArgument(InvalidArgumentException {
                message: "The Yaml::DUMP_NULL_AS_EMPTY and Yaml::DUMP_NULL_AS_TILDE flags cannot be used together.".to_owned(),
            }));
        }

        Ok(PhpString::from(
            self.do_dump(input, inline, indent, flags, 0)?,
        ))
    }

    fn inline_dump(
        &self,
        value: &Value,
        flags: Flags,
        root_level: bool,
    ) -> Result<Vec<u8>, DumpError> {
        Ok(self
            .inline
            .dump(value, flags, root_level)?
            .as_bytes()
            .to_vec())
    }

    /// `$dumpAsMap ? Inline::dump($key, $flags).':' : '-'`
    fn key(&self, key: &Value, dump_as_map: bool, flags: Flags) -> Result<Vec<u8>, DumpError> {
        Ok(if dump_as_map {
            let mut key = self.inline_dump(key, flags, false)?;
            key.push(b':');
            key
        } else {
            b"-".to_vec()
        })
    }

    fn do_dump(
        &self,
        input: &Value,
        inline: i64,
        indent: usize,
        flags: Flags,
        nesting_level: i64,
    ) -> Result<Vec<u8>, DumpError> {
        let mut output: Vec<u8> = Vec::new();
        let prefix = vec![b' '; indent];
        let mut dump_object_as_inline_map = true;

        if flags.contains(Flags::DUMP_OBJECT_AS_MAP) && is_map_object(input) {
            dump_object_as_inline_map = to_array(input).is_empty();
        }

        let tagged = as_tagged(input);
        if inline <= 0
            || (!matches!(input, Value::Array(_)) && tagged.is_none() && dump_object_as_inline_map)
            || !truthy(input)
        {
            output.extend_from_slice(&prefix);
            output.extend(self.inline_dump(input, flags, nesting_level == 0)?);
        } else if let Some(tagged) = tagged {
            output.extend(self.dump_tagged_value(
                tagged,
                inline,
                indent,
                flags,
                &prefix,
                nesting_level,
            )?);
        } else {
            let dump_as_map = is_hash(input);
            let compact_nested_mapping =
                flags.contains(Flags::DUMP_COMPACT_NESTED_MAPPING) && !dump_as_map;

            let entries = match input {
                Value::Array(array) => array.clone(),
                other => to_array(other),
            };
            for (key, value) in entries.iter() {
                if !output.is_empty() && output.last() != Some(&b'\n') {
                    output.push(b'\n');
                }

                let mut key = key.to_value();
                if let Value::Int(i) = key
                    && flags.contains(Flags::DUMP_NUMERIC_KEY_AS_STRING)
                {
                    key = Value::from(i.to_string().as_str());
                }

                if flags.contains(Flags::DUMP_MULTI_LINE_LITERAL_BLOCK)
                    && let Some(text) = multi_line(value)
                {
                    output.extend_from_slice(&prefix);
                    output.extend(self.key(&key, dump_as_map, flags)?);
                    output.extend_from_slice(b" |");
                    output.extend(self.get_block_indentation_indicator(text));
                    output.extend_from_slice(get_block_chomping_indicator(text));
                    output.extend(self.dump_block_scalar_lines(text, &prefix));

                    continue;
                }

                if let Some(tagged) = as_tagged(value) {
                    output.extend_from_slice(&prefix);
                    output.extend(self.key(&key, dump_as_map, flags)?);
                    output.extend_from_slice(b" !");
                    output.extend_from_slice(tagged.get_tag().as_bytes());

                    let inner = tagged.get_value();
                    if flags.contains(Flags::DUMP_MULTI_LINE_LITERAL_BLOCK)
                        && let Some(text) = multi_line(inner)
                    {
                        output.extend_from_slice(b" |");
                        output.extend(self.get_block_indentation_indicator(text));
                        output.extend_from_slice(get_block_chomping_indicator(text));
                        output.extend(self.dump_block_scalar_lines(text, &prefix));

                        continue;
                    }

                    if inline - 1 <= 0 || is_scalar_or_null(inner) {
                        output.push(b' ');
                        output.extend(self.do_dump(
                            inner,
                            inline - 1,
                            0,
                            flags,
                            nesting_level + 1,
                        )?);
                        output.push(b'\n');
                    } else {
                        output.push(b'\n');
                        let indent = if dump_as_map {
                            indent + self.indentation
                        } else {
                            indent + 2
                        };
                        output.extend(self.do_dump(
                            inner,
                            inline - 1,
                            indent,
                            flags,
                            nesting_level + 1,
                        )?);
                    }

                    continue;
                }

                let mut dump_object_as_inline_map = true;

                if flags.contains(Flags::DUMP_OBJECT_AS_MAP) && is_map_object(value) {
                    dump_object_as_inline_map = to_array(value).is_empty();
                }

                let is_array = matches!(value, Value::Array(_));
                let will_be_inlined =
                    inline - 1 <= 0 || !is_array && dump_object_as_inline_map || !truthy(value);
                let compact = compact_nested_mapping && is_array && is_hash(value);

                output.extend_from_slice(&prefix);
                output.extend(self.key(&key, dump_as_map, flags)?);
                output.push(if will_be_inlined || compact {
                    b' '
                } else {
                    b'\n'
                });
                if compact {
                    let nested =
                        self.do_dump(value, inline - 1, indent + 2, flags, nesting_level + 1)?;
                    output.extend_from_slice(nested.get(indent + 2..).unwrap_or(b""));
                } else {
                    let indent = if will_be_inlined {
                        0
                    } else {
                        indent + self.indentation
                    };
                    output.extend(self.do_dump(
                        value,
                        inline - 1,
                        indent,
                        flags,
                        nesting_level + 1,
                    )?);
                }
                if will_be_inlined {
                    output.push(b'\n');
                }
            }
        }

        Ok(output)
    }

    fn dump_tagged_value(
        &self,
        value: &TaggedValue,
        inline: i64,
        indent: usize,
        flags: Flags,
        prefix: &[u8],
        nesting_level: i64,
    ) -> Result<Vec<u8>, DumpError> {
        let mut output = prefix.to_vec();
        if !prefix.is_empty() {
            output.push(b' ');
        }
        output.push(b'!');
        output.extend_from_slice(value.get_tag().as_bytes());

        let inner = value.get_value();
        if flags.contains(Flags::DUMP_MULTI_LINE_LITERAL_BLOCK)
            && let Some(text) = multi_line(inner)
        {
            output.extend_from_slice(b" |");
            output.extend(self.get_block_indentation_indicator(text));
            output.extend_from_slice(get_block_chomping_indicator(text));
            output.extend(self.dump_block_scalar_lines(text, prefix));

            return Ok(output);
        }

        if inline - 1 <= 0 || is_scalar_or_null(inner) {
            output.push(b' ');
            output.extend(self.do_dump(inner, inline - 1, 0, flags, nesting_level + 1)?);
            output.push(b'\n');
            return Ok(output);
        }

        output.push(b'\n');
        output.extend(self.do_dump(inner, inline - 1, indent, flags, nesting_level + 1)?);
        Ok(output)
    }

    fn dump_block_scalar_lines(&self, value: &[u8], prefix: &[u8]) -> Vec<u8> {
        let mut output = Vec::new();

        for row in value.split(|&b| b == b'\n') {
            if row.is_empty() {
                output.push(b'\n');
            } else {
                output.push(b'\n');
                output.extend_from_slice(prefix);
                output.extend(std::iter::repeat_n(b' ', self.indentation));
                output.extend_from_slice(row);
            }
        }

        output
    }

    fn get_block_indentation_indicator(&self, value: &[u8]) -> Vec<u8> {
        // The spec requires a block indentation indicator when the first line that is not empty starts with a space, even if that line holds only spaces
        // http://www.yaml.org/spec/1.2/spec.html#id2793979
        match value.split(|&b| b == b'\n').find(|line| !line.is_empty()) {
            Some(line) if line.starts_with(b" ") => self.indentation.to_string().into_bytes(),
            _ => Vec::new(),
        }
    }
}

/// `null === $value || \is_scalar($value)`.
fn is_scalar_or_null(value: &Value) -> bool {
    !matches!(value, Value::Array(_) | Value::Object(_))
}

fn get_block_chomping_indicator(value: &[u8]) -> &'static [u8] {
    // a block made of line breaks only has no content, so only the keep indicator preserves them
    if value == b"\n" || value.ends_with(b"\n\n") {
        return b"+";
    }

    if value.ends_with(b"\n") {
        return b"";
    }

    b"-"
}
