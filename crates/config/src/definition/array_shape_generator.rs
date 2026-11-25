//! Port of Symfony\Component\Config\Definition\ArrayShapeGenerator.php:
//! the `PHPDoc` array shape of a configuration tree, as the config builders
//! and `config/reference.php` print it.
//!
//! Symfony source copyright (c) Fabien Potencier; distributed under MIT.
//!
//! Upstream: symfony/config Definition/ArrayShapeGenerator.php

use php_value::{PhpString, Value, json::JsonFlags};

use crate::{
    definition::{
        base_node::{BASE_NODE, Node},
        boolean_node, enum_node,
        exception::{Class, Error},
        float_node, integer_node, numeric_node, scalar_node, string_node,
    },
    php,
};

/// `\Symfony\Component\Config\Loader\ParamConfigurator::class`.
pub const PARAM_CONFIGURATOR: &str = "Symfony\\Component\\Config\\Loader\\ParamConfigurator";

/// `ExprBuilder::TYPE_ANY`, `TYPE_STRING`, `TYPE_BACKED_ENUM`.
const TYPE_ANY: &[u8] = b"any";
const TYPE_STRING: &[u8] = b"string";
const TYPE_BACKED_ENUM: &[u8] = b"backed-enum";

/// `final class ArrayShapeGenerator`.
pub struct ArrayShapeGenerator;

impl ArrayShapeGenerator {
    /// `ArrayShapeGenerator::generate($node)`.
    pub fn generate(node: &Node) -> Result<Vec<u8>, Error> {
        // "*/" anywhere in the shape (a comment, an enum value, a node name) would prematurely
        // close the surrounding doc block, so the slash is escaped to keep the value readable
        let shape = do_generate_php_doc(node, 1)?;
        let shape = replace(&shape, b"*/", b"*\\/");
        Ok(replace(&shape, b"\n", b"\n * "))
    }
}

/// `str_replace($search, $replace, $subject)` for one pair.
fn replace(subject: &[u8], search: &[u8], with: &[u8]) -> Vec<u8> {
    let mut out = Vec::with_capacity(subject.len());
    let mut i = 0;
    while i < subject.len() {
        if subject[i..].starts_with(search) {
            out.extend_from_slice(with);
            i += search.len();
        } else {
            out.push(subject[i]);
            i += 1;
        }
    }
    out
}

fn indent(level: usize) -> Vec<u8> {
    b"    ".repeat(level)
}

fn do_generate_php_doc(node: &Node, nesting_level: usize) -> Result<Vec<u8>, Error> {
    if !node.is_array_node() {
        let type_string: Vec<u8> = if node.is_instance_of(boolean_node::CLASS) {
            if node.has_default_value() && node.get_default_value()?.is_null() {
                b"bool|null".to_vec()
            } else {
                b"bool".to_vec()
            }
        } else if node.is_instance_of(string_node::CLASS) {
            b"string".to_vec()
        } else if node.is_instance_of(numeric_node::CLASS) {
            handle_numeric_node(node).to_vec()
        } else if node.is_instance_of(enum_node::CLASS) {
            node.get_permissible_values(b"|", false)?
        } else if node.is_instance_of(scalar_node::CLASS) {
            b"scalar|null".to_vec()
        } else {
            b"mixed".to_vec()
        };

        if type_string == b"mixed" {
            return Ok(type_string);
        }

        let param = format!("|\\{PARAM_CONFIGURATOR}");
        if let Some(head) = type_string.strip_suffix(b"|null") {
            let mut out = head.to_vec();
            out.extend_from_slice(param.as_bytes());
            out.extend_from_slice(b"|null");
            return Ok(out);
        }

        let mut out = type_string;
        out.extend_from_slice(param.as_bytes());
        return Ok(out);
    }

    if node.is_prototyped_array_node() {
        let is_hashmap = node
            .get_key_attribute()?
            .is_some_and(|k| php::truthy(&Value::String(k)));
        let mut array_shape: Vec<u8> = if is_hashmap {
            b"array<string, ".to_vec()
        } else {
            b"list<".to_vec()
        };
        array_shape.extend(do_generate_php_doc(&node.get_prototype()?, nesting_level)?);
        array_shape.push(b'>');

        let mut parts = get_normalized_types(node, &[b"array", b"any"])?;
        parts.push(array_shape);
        return Ok(php::implode(b"|", parts));
    }

    let children = node.get_children()?;
    if children.is_empty()
        && !node
            .get_parent()
            .is_some_and(|p| p.is_prototyped_array_node())
    {
        return Ok(
            if node.has_default_value() && node.get_default_value()?.is_null() {
                b"array<mixed>|null".to_vec()
            } else {
                b"array<mixed>".to_vec()
            },
        );
    }

    let mut array_shape = b"array{".to_vec();
    array_shape.extend(generate_inline_php_doc_for_node(node)?);
    array_shape.push(b'\n');

    for (_, child) in &children {
        array_shape.extend(indent(nesting_level));
        array_shape.extend(dump_node_key(child, node)?);
        array_shape.extend_from_slice(b": ");

        array_shape.extend(do_generate_php_doc(child, 1 + nesting_level)?);

        array_shape.push(b',');
        if !child.is_array_node() {
            array_shape.extend(generate_inline_php_doc_for_node(child)?);
        }
        array_shape.push(b'\n');
    }

    if node.should_ignore_extra_keys()?
        || node
            .get_normalized_types()
            .iter()
            .any(|t| t.as_bytes() == TYPE_ANY)
    {
        array_shape.extend(indent(nesting_level));
        array_shape.extend_from_slice(b"...<string, mixed>\n");
    }

    array_shape.extend(indent(nesting_level - 1));
    array_shape.push(b'}');

    let mut parts = get_normalized_types(node, &[b"array", b"any"])?;
    parts.push(array_shape);
    Ok(php::implode(b"|", parts))
}

fn dump_node_key(node: &Node, parent: &Node) -> Result<Vec<u8>, Error> {
    let mut name = node.get_name().to_vec();
    let lower = name.to_ascii_lowercase();
    let quoted = name.starts_with(b"@")
        || [&b"int"[..], b"float", b"bool", b"null", b"scalar"].contains(&lower.as_slice())
        || name.iter().any(|&b| b == b'\'' || b == b'"');

    if quoted {
        let mut q = b"'".to_vec();
        q.extend(addslashes(&name));
        q.push(b'\'');
        name = q;
    }

    let optional =
        !node.is_required() || (parent.is_array_node() && parent.should_perform_deep_merging()?);

    if optional {
        name.push(b'?');
    }
    Ok(name)
}

/// `addslashes()`: a backslash before `'`, `"`, `\` and NUL (as `\0`).
fn addslashes(s: &[u8]) -> Vec<u8> {
    let mut out = Vec::with_capacity(s.len());
    for &b in s {
        match b {
            b'\'' | b'"' | b'\\' => {
                out.push(b'\\');
                out.push(b);
            }
            0 => out.extend_from_slice(b"\\0"),
            _ => out.push(b),
        }
    }
    out
}

fn handle_numeric_node(node: &Node) -> &'static [u8] {
    // We could use int<%s, %s> but PhpStorm doesn't support it yet
    // $min = $node->getMin() ?? 'min';
    // $max = $node->getMax() ?? 'max';

    if node.is_instance_of(integer_node::CLASS) {
        return b"int";
    }
    if node.is_instance_of(float_node::CLASS) {
        return b"float";
    }

    b"int|float"
}

/// The `TypeError` of a parameter typed `BaseNode` given a node that only
/// implements `NodeInterface`.
fn require_base_node(node: &Node, function: &str) -> Result<(), Error> {
    if node.is_base_node() {
        return Ok(());
    }
    Err(Error::new(
        Class::TypeError,
        format!(
            "Symfony\\Component\\Config\\Definition\\ArrayShapeGenerator::{function}(): Argument #1 ($node) must be of type {BASE_NODE}, {} given",
            node.class()
        ),
    ))
}

fn generate_inline_php_doc_for_node(node: &Node) -> Result<Vec<u8>, Error> {
    require_base_node(node, "generateInlinePhpDocForNode")?;
    let mut comment = Vec::new();
    if node.is_deprecated() {
        comment.extend_from_slice(b" // Deprecated: ");
        comment.extend(
            node.get_deprecation(&node.get_name(), &node.get_path())?
                .message,
        );
    }

    if let Some(info) = node.hint() {
        comment.extend_from_slice(b" // ");
        comment.extend_from_slice(&info);
    }

    // `!$node instanceof ArrayNode || ($node = $node->getParent()) instanceof
    // PrototypedArrayNode`: an array node reports its prototyped parent's default.
    let subject = if node.is_array_node() {
        node.get_parent().filter(Node::is_prototyped_array_node)
    } else {
        Some(node.clone())
    };
    if let Some(subject) = subject
        && subject.has_default_value()
    {
        comment.extend_from_slice(b" // Default: ");
        let flags = JsonFlags {
            unescaped_slashes: true,
            unescaped_unicode: true,
        };
        // json_encode() returning false concatenates as "".
        if let Some(json) = php::json_encode_with(&subject.get_default_value()?, flags, true) {
            comment.extend(json);
        }
    }

    Ok(rtrim(&collapse_whitespace(&comment)).to_vec())
}

/// `preg_replace('/\s+/', ' ', $s)`: without `/u`, `\s` is the ASCII
/// space, `\t`, `\n`, `\v`, `\f` and `\r`.
fn collapse_whitespace(s: &[u8]) -> Vec<u8> {
    let mut out = Vec::with_capacity(s.len());
    let mut in_space = false;
    for &b in s {
        if matches!(b, b' ' | b'\t' | b'\n' | 0x0b | 0x0c | b'\r') {
            if !in_space {
                out.push(b' ');
            }
            in_space = true;
        } else {
            out.push(b);
            in_space = false;
        }
    }
    out
}

/// `rtrim($s)`: " \t\n\r\0\x0B".
fn rtrim(s: &[u8]) -> &[u8] {
    let end = s
        .iter()
        .rposition(|b| !matches!(b, b' ' | b'\t' | b'\n' | b'\r' | 0 | 0x0b))
        .map_or(0, |i| i + 1);
    &s[..end]
}

/// `getNormalizedTypes($node, $excluded)`.
fn get_normalized_types(node: &Node, excluded: &[&[u8]]) -> Result<Vec<Vec<u8>>, Error> {
    require_base_node(node, "getNormalizedTypes")?;
    let mut types: Vec<Vec<u8>> = node
        .get_normalized_types()
        .iter()
        .map(|t| t.to_vec())
        .filter(|t| !excluded.contains(&t.as_slice()))
        .collect();

    if node.has_default_value() && node.get_default_value()?.is_null() {
        types.push(b"null".to_vec());
    }

    // array_unique(): the first of equal strings stays.
    let mut unique: Vec<Vec<u8>> = Vec::with_capacity(types.len());
    for t in types {
        if !unique.contains(&t) {
            unique.push(t);
        }
    }
    let mut types = unique;
    if let Some(i) = types.iter().position(|t| t == TYPE_BACKED_ENUM) {
        types[i] = b"\\BackedEnum".to_vec();
    }

    if types
        .iter()
        .any(|t| t == TYPE_STRING || t == b"\\BackedEnum")
    {
        types.push(format!("\\{PARAM_CONFIGURATOR}").into_bytes());
    }

    // sort(): PHP 8's comparison of strings.
    let mut values: Vec<Value> = types
        .into_iter()
        .map(|t| Value::String(PhpString::from(t)))
        .collect();
    values.sort_by(|a, b| php::compare(a, b).unwrap_or(std::cmp::Ordering::Equal));
    Ok(values.iter().map(php::to_bytes).collect())
}
