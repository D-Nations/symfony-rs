//! Port of Symfony\Component\Translation\Loader\XliffFileLoader.php, on
//! `php-dom` and symfony-config's `XmlUtils`.
//!
//! Upstream reads the DOM through `SimpleXML` and XPath. What it uses of
//! them is written here on php-dom's tree, as PHP 8.4.27 behaves
//! (recorded, `scripts/record.sh translation-xliff`):
//! - `$el->name` (and `foreach`, `isset()`): the child elements with that
//!   local name and no namespace prefix (no namespace, or the default
//!   one); a prefixed child is not seen, whatever its namespace;
//! - `(string) $el`: the text and CDATA of its direct children only;
//! - `$el->attributes()`: the attributes without a namespace, keyed by
//!   name; truthy when there is one; each attribute truthy even when
//!   empty or `"0"`; `attributes($ns)`: those in namespace `$ns`;
//! - `xpath('//p:name')` / `xpath('.//p:name')`: the elements (descendants)
//!   with that namespace and local name, in document order.
//!
//! The resource is a string as in PHP: `<?xml…` is a document, anything
//! else a path, read through PHP's stream wrappers as far as
//! `stream_is_local()`, `file_exists()` and `is_file()` tell them apart
//! (recorded): `http`, `https`, `ftp`, `ftps`, `data` and a `file://` URL
//! with a host other than `localhost` are not local; `php`, `glob` and
//! `compress.zlib` URLs do not exist; a `file://` URL names its path; an
//! unknown scheme is a plain path. A `phar://` URL into an existing archive
//! is refused ([`Class::Refused`]); one without an archive does not exist.
//!
//! A catalogue loaded from a file gets symfony/config's `FileResource` for
//! the resource as given (its real path, or the `file://` URL as given).
//!
//! Symfony source copyright (c) Fabien Potencier; distributed under MIT.
//!
//! Upstream: symfony/translation Loader/XliffFileLoader.php Loader/LoaderInterface.php

use std::rc::Rc;

use php_dom::{Document, NodeId, NodeType};
use php_value::{
    Key, Number, PhpArray, Value,
    numeric::{hexdec, is_numeric, string_to_int},
};
use symfony_config::XmlUtils;
use symfony_config::resource::FileResource;

use crate::exception::{Class, Error, Previous};
use crate::message_catalogue::{INTL_DOMAIN_SUFFIX, MessageCatalogue};
use crate::util::xliff_utils::XliffUtils;

use super::local::{Location, locate, quoted};

const XLIFF_12_NS: &str = "urn:oasis:names:tc:xliff:document:1.2";
const XLIFF_20_NS: &str = "urn:oasis:names:tc:xliff:document:2.0";
const METADATA_NS: &str = "urn:oasis:names:tc:xliff:metadata:2.0";
const PGS_NS: &str = "urn:oasis:names:tc:xliff:pgs:1.0";

/// `LoaderInterface`.
pub trait LoaderInterface {
    /// `load($resource, $locale, $domain)`.
    fn load(
        &self,
        resource: &[u8],
        locale: &[u8],
        domain: &[u8],
    ) -> Result<MessageCatalogue, Error>;
}

/// `XliffFileLoader`.
#[derive(Clone, Copy, Debug, Default)]
pub struct XliffFileLoader;

impl LoaderInterface for XliffFileLoader {
    fn load(
        &self,
        resource: &[u8],
        locale: &[u8],
        domain: &[u8],
    ) -> Result<MessageCatalogue, Error> {
        let xml_string = resource.starts_with(b"<?xml");
        let mut path = None;
        if !xml_string {
            match locate(resource) {
                Location::Remote => {
                    return Err(Error::new(
                        Class::InvalidResource,
                        quoted("This is not a local file \"", resource, "\"."),
                    ));
                }
                Location::Absent => {
                    return Err(Error::new(
                        Class::NotFoundResource,
                        quoted("File \"", resource, "\" not found."),
                    ));
                }
                Location::Phar => {
                    return Err(Error::new(
                        Class::Refused,
                        quoted("phar archive \"", resource, "\" is not read"),
                    ));
                }
                Location::Path(p) => {
                    if std::fs::metadata(&p).is_err() {
                        return Err(Error::new(
                            Class::NotFoundResource,
                            quoted("File \"", resource, "\" not found."),
                        ));
                    }
                    if !p.is_file() {
                        return Err(Error::new(
                            Class::InvalidResource,
                            quoted(
                                "This is neither a file nor an XLIFF string \"",
                                resource,
                                "\".",
                            ),
                        ));
                    }
                    path = Some(p);
                }
            }
        }

        let loaded = match &path {
            None => XmlUtils::parse(resource, None),
            Some(p) => XmlUtils::load_file_named(p, resource, None),
        };
        let dom = match loaded {
            Ok(dom) => dom,
            Err(e) if e.class().is_a(&symfony_config::Class::InvalidArgument) => {
                let mut message = quoted("Unable to load \"", resource, "\": ");
                message.extend_from_slice(e.get_message());
                return Err(Error::with_previous(
                    Class::InvalidResource,
                    message,
                    e.get_code(),
                    Previous::Config(e),
                ));
            }
            Err(e) => {
                return Err(Error::new(
                    Class::Config(Box::new(e.clone())),
                    e.get_message(),
                ));
            }
        };

        match XliffUtils::validate_schema(&dom)? {
            Ok(errors) if errors.is_empty() => {}
            Ok(errors) => {
                let mut message = quoted("Invalid resource provided: \"", resource, "\"; Errors: ");
                message.extend_from_slice(XliffUtils::get_errors_as_string(&errors).as_bytes());
                return Err(Error::new(Class::InvalidResource, message));
            }
            Err(refusal) => {
                // decision 14: the exception upstream throws for an invalid
                // document, with the refusal's text
                let mut message = quoted("Invalid resource provided: \"", resource, "\"; Errors: ");
                message.extend_from_slice(refusal.to_string().as_bytes());
                return Err(Error::new(Class::InvalidResource, message));
            }
        }

        let catalogue = MessageCatalogue::new(locale, PhpArray::new());
        extract(&dom, &catalogue, domain)?;

        // `is_file($resource)`: a path was found to be a file above
        if path.is_some() {
            let file = FileResource::new(resource)
                .map_err(|e| Error::new(Class::Config(Box::new(e.clone())), e.get_message()))?;
            catalogue.add_resource(Rc::new(file));
        }

        Ok(catalogue)
    }
}

fn extract(dom: &Document, catalogue: &MessageCatalogue, domain: &[u8]) -> Result<(), Error> {
    let version = XliffUtils::get_version_number(dom);
    let encoding = dom
        .encoding()
        .filter(|e| !e.is_empty())
        .map(str::to_ascii_uppercase);
    let encoding = encoding.as_deref();

    if version == "1.2" {
        extract_xliff1(dom, catalogue, domain, encoding)?;
    }
    if matches!(version.as_str(), "2.0" | "2.1" | "2.2") {
        extract_xliff2(dom, catalogue, domain, encoding)?;
    }
    Ok(())
}

// SimpleXML, as the loader uses it ------------------------------------------

/// `$el->name`: child elements named `name` without a namespace prefix.
fn children(dom: &Document, el: NodeId, name: &str) -> Vec<NodeId> {
    dom.children(el)
        .iter()
        .copied()
        .filter(|&c| {
            dom.node_type(c) == NodeType::Element
                && dom.local_name(c) == Some(name)
                && (dom.namespace_uri(c).is_none() || dom.prefix(c).is_none())
        })
        .collect()
}

fn child(dom: &Document, el: NodeId, name: &str) -> Option<NodeId> {
    children(dom, el, name).into_iter().next()
}

/// `(string) $el`.
fn text(dom: &Document, el: NodeId) -> String {
    let mut out = String::new();
    for &c in dom.children(el) {
        if matches!(dom.node_type(c), NodeType::Text | NodeType::CData)
            && let Some(v) = dom.node_value(c)
        {
            out.push_str(v);
        }
    }
    out
}

fn text_of(dom: &Document, el: Option<NodeId>) -> String {
    el.map(|e| text(dom, e)).unwrap_or_default()
}

/// `$el->attributes()`: name and value of each attribute without a
/// namespace.
fn attributes(dom: &Document, el: NodeId) -> Vec<(&str, &str)> {
    dom.attributes(el)
        .iter()
        .filter(|a| a.namespace_uri.is_none())
        .map(|a| (a.local_name.as_str(), a.value.as_str()))
        .collect()
}

/// `$el->attributes()[$name]` (`null` when absent).
fn attribute<'a>(dom: &'a Document, el: NodeId, name: &str) -> Option<&'a str> {
    dom.attributes(el)
        .iter()
        .find(|a| a.namespace_uri.is_none() && a.local_name == name)
        .map(|a| a.value.as_str())
}

/// `$el->attributes($ns)[$name]`.
fn attribute_ns<'a>(dom: &'a Document, el: NodeId, ns: &str, name: &str) -> Option<&'a str> {
    dom.attributes(el)
        .iter()
        .find(|a| a.namespace_uri.as_deref() == Some(ns) && a.local_name == name)
        .map(|a| a.value.as_str())
}

/// `xpath('.//p:name')` from `el` (the document for `//p:name`).
fn descendants(dom: &Document, el: NodeId, ns: &str, name: &str) -> Vec<NodeId> {
    dom.get_elements_by_tag_name(el, "*")
        .into_iter()
        .filter(|&e| dom.namespace_uri(e) == Some(ns) && dom.local_name(e) == Some(name))
        .collect()
}

fn attributes_array(dom: &Document, el: NodeId) -> PhpArray {
    let mut array = PhpArray::new();
    for (name, value) in attributes(dom, el) {
        array.insert(Key::from_bytes(name.as_bytes()), Value::string(value));
    }
    array
}

fn string_key(s: &str) -> Key {
    Key::from_bytes(s.as_bytes())
}

// XLIFF 1.2 -------------------------------------------------------------------

fn extract_xliff1(
    dom: &Document,
    catalogue: &MessageCatalogue,
    domain: &[u8],
    encoding: Option<&str>,
) -> Result<(), Error> {
    for file in descendants(dom, dom.root(), XLIFF_12_NS, "file") {
        for prop in descendants(dom, file, XLIFF_12_NS, "prop") {
            // prop-type is required by the schema
            let key = attribute(dom, prop, "prop-type").unwrap_or_default();
            catalogue.set_catalogue_metadata(
                key.as_bytes(),
                Value::string(text(dom, prop)),
                domain,
            );
        }

        for translation in descendants(dom, file, XLIFF_12_NS, "trans-unit") {
            let resname = attribute(dom, translation, "resname");
            let source_el = child(dom, translation, "source");
            if resname.is_none() && source_el.is_none() {
                continue;
            }

            // an attribute is truthy even when empty
            let source_text = text_of(dom, source_el);
            let source = resname.map_or_else(|| source_text.clone(), str::to_owned);

            let target_el = child(dom, translation, "target");
            if let Some(target) = target_el
                && attribute(dom, target, "state") == Some("needs-translation")
            {
                let t = text(dom, target);
                if t == source || t == source_text {
                    continue;
                }
            }

            let target = utf8_to_charset(&text_of(dom, target_el.or(source_el)), encoding)?;
            catalogue.set(source.as_bytes(), &target, domain);

            let mut metadata = PhpArray::new();
            metadata.insert("source", Value::string(source_text.as_str()));
            let mut file_meta = PhpArray::new();
            file_meta.insert(
                "original",
                Value::string(attribute(dom, file, "original").unwrap_or_default()),
            );
            metadata.insert("file", Value::Array(file_meta));

            let notes = parse_notes_metadata(dom, &children(dom, translation, "note"), encoding)?;
            if !notes.is_empty() {
                metadata.insert("notes", Value::Array(notes));
            }

            if let Some(target) = target_el {
                let attrs = attributes_array(dom, target);
                if !attrs.is_empty() {
                    metadata.insert("target-attributes", Value::Array(attrs));
                }
            }

            if let Some(id) = attribute(dom, translation, "id") {
                metadata.insert("id", Value::string(id));
            }

            catalogue.set_metadata(source.as_bytes(), Value::Array(metadata), domain);
        }
    }
    Ok(())
}

fn parse_notes_metadata(
    dom: &Document,
    notes: &[NodeId],
    encoding: Option<&str>,
) -> Result<PhpArray, Error> {
    let mut out = PhpArray::new();
    for &note in notes {
        let mut n = PhpArray::new();
        n.insert(
            "content",
            Value::string(utf8_to_charset(&text(dom, note), encoding)?),
        );
        if let Some(priority) = attribute(dom, note, "priority") {
            n.insert("priority", Value::Int(string_to_int(priority.as_bytes())));
        }
        if let Some(from) = attribute(dom, note, "from") {
            n.insert("from", Value::string(from));
        }
        let _ = out.push(Value::Array(n));
    }
    Ok(out)
}

// XLIFF 2 ---------------------------------------------------------------------

fn notes_array(dom: &Document, notes: NodeId) -> PhpArray {
    let mut out = PhpArray::new();
    for note in children(dom, notes, "note") {
        let mut n = attributes_array(dom, note);
        n.insert("content", Value::string(text(dom, note)));
        let _ = out.push(Value::Array(n));
    }
    out
}

fn extract_xliff2(
    dom: &Document,
    catalogue: &MessageCatalogue,
    domain: &[u8],
    encoding: Option<&str>,
) -> Result<(), Error> {
    for meta in descendants(dom, dom.root(), METADATA_NS, "meta") {
        let key = attribute(dom, meta, "type").unwrap_or_default();
        catalogue.set_catalogue_metadata(key.as_bytes(), Value::string(text(dom, meta)), domain);
    }

    for unit in descendants(dom, dom.root(), XLIFF_20_NS, "unit") {
        if let Some(switch) = attribute_ns(dom, unit, PGS_NS, "switch") {
            extract_xliff2_pgs_unit(dom, unit, catalogue, domain, switch, encoding)?;
            continue;
        }

        for segment in children(dom, unit, "segment") {
            let source_el = child(dom, segment, "source");
            let source = match attribute(dom, unit, "name") {
                Some(name) => name.to_owned(),
                None => text_of(dom, source_el),
            };

            let target_el = child(dom, segment, "target");
            let target = utf8_to_charset(&text_of(dom, target_el.or(source_el)), encoding)?;
            catalogue.set(source.as_bytes(), &target, domain);

            let mut metadata = PhpArray::new();
            let segment_attributes = attributes_array(dom, segment);
            if !segment_attributes.is_empty() {
                metadata.insert("segment-attributes", Value::Array(segment_attributes));
            }
            if let Some(target) = target_el {
                let attrs = attributes_array(dom, target);
                if !attrs.is_empty() {
                    metadata.insert("target-attributes", Value::Array(attrs));
                }
            }
            if let Some(notes) = child(dom, unit, "notes") {
                metadata.insert("notes", Value::Array(notes_array(dom, notes)));
            }

            catalogue.set_metadata(source.as_bytes(), Value::Array(metadata), domain);
        }
    }
    Ok(())
}

/// A `pgs:switch` entry.
struct Switch {
    kind: String,
    variable: String,
}

fn extract_xliff2_pgs_unit(
    dom: &Document,
    unit: NodeId,
    catalogue: &MessageCatalogue,
    domain: &[u8],
    pgs_switch: &str,
    encoding: Option<&str>,
) -> Result<(), Error> {
    let switches = parse_pgs_switch(pgs_switch)?;
    let source = attribute(dom, unit, "name")
        .or_else(|| attribute(dom, unit, "id"))
        .unwrap_or_default()
        .to_owned();

    let mut cases = PhpArray::new();
    for segment in children(dom, unit, "segment") {
        let Some(case) = attribute_ns(dom, segment, PGS_NS, "case") else {
            continue;
        };
        let element = child(dom, segment, "target").or_else(|| child(dom, segment, "source"));
        let text = element
            .map(|e| extract_pgs_segment_text(dom, e, &switches))
            .unwrap_or_default();
        cases.insert(string_key(case), Value::string(text));
    }

    let mut intl_domain = domain.to_vec();
    intl_domain.extend_from_slice(INTL_DOMAIN_SUFFIX.as_bytes());
    let message = utf8_to_charset(&build_icu_message(&switches, &cases), encoding)?;
    catalogue.set(source.as_bytes(), &message, &intl_domain);

    let mut metadata = PhpArray::new();
    metadata.insert("pgs-switch", Value::string(pgs_switch));
    if let Some(notes) = child(dom, unit, "notes") {
        if !children(dom, notes, "note").is_empty() {
            // array_map('strval', $noteNode->attributes() ?? []): attributes()
            // is a SimpleXMLElement, never an array
            return Err(Error::new(
                Class::TypeError,
                "array_map(): Argument #2 ($array) must be of type array, SimpleXMLElement given",
            ));
        }
        metadata.insert("notes", Value::Array(PhpArray::new()));
    }

    catalogue.set_metadata(source.as_bytes(), Value::Array(metadata), &intl_domain);
    Ok(())
}

/// PHP's `trim()` characters.
fn is_trim(c: char) -> bool {
    matches!(c, ' ' | '\t' | '\n' | '\r' | '\0' | '\x0B')
}

/// PCRE's `\s`.
fn is_pcre_space(c: char) -> bool {
    matches!(c, ' ' | '\t' | '\n' | '\x0B' | '\x0C' | '\r')
}

fn parse_pgs_switch(pgs_switch: &str) -> Result<Vec<Switch>, Error> {
    let trimmed = pgs_switch.trim_matches(is_trim);
    if trimmed.is_empty() {
        return Err(Error::new(
            Class::InvalidResource,
            "The pgs:switch attribute must not be empty.",
        ));
    }

    // preg_split('/\s+/', …)
    let mut items = Vec::new();
    let mut current = String::new();
    let mut in_space = false;
    for c in trimmed.chars() {
        if is_pcre_space(c) {
            if !in_space {
                items.push(std::mem::take(&mut current));
                in_space = true;
            }
        } else {
            in_space = false;
            current.push(c);
        }
    }
    items.push(current);

    let mut switches = Vec::new();
    for item in items {
        match item.split_once(':') {
            Some((kind, variable)) if !kind.is_empty() && !variable.is_empty() => {
                switches.push(Switch {
                    kind: kind.to_owned(),
                    variable: variable.to_owned(),
                });
            }
            _ => {
                return Err(Error::new(
                    Class::InvalidResource,
                    format!("The pgs:switch token \"{item}\" must use the \"type:variable\" form."),
                ));
            }
        }
    }
    Ok(switches)
}

fn extract_pgs_segment_text(dom: &Document, element: NodeId, switches: &[Switch]) -> String {
    let plural: Vec<&str> = switches
        .iter()
        .filter(|s| s.kind == "plural" || s.kind == "ordinal")
        .map(|s| s.variable.as_str())
        .collect();
    let mut out = String::new();
    collect_pgs_text(dom, element, &plural, &mut out);
    out
}

fn collect_pgs_text(dom: &Document, node: NodeId, plural: &[&str], out: &mut String) {
    for &child in dom.children(node) {
        match dom.node_type(child) {
            NodeType::Text | NodeType::CData => {
                out.push_str(dom.node_value(child).unwrap_or_default());
                continue;
            }
            NodeType::Element => {}
            _ => continue,
        }
        match dom.local_name(child).unwrap_or_default() {
            "ph" => {
                let disp = dom.get_attribute(child, "disp");
                if !disp.is_empty() && plural.contains(&disp) {
                    out.push('#');
                } else if !disp.is_empty() {
                    out.push('{');
                    out.push_str(disp);
                    out.push('}');
                }
            }
            "pc" | "mrk" => collect_pgs_text(dom, child, plural, out),
            "cp" => {
                let hex = dom.get_attribute(child, "hex");
                if !hex.is_empty() && hex.bytes().all(|b| b.is_ascii_hexdigit()) {
                    let codepoint = match hexdec(hex.as_bytes()) {
                        Number::Int(i) => i,
                        // past PHP_INT_MAX: a float, above 0x10FFFF
                        Number::Float(_) => continue,
                    };
                    if codepoint <= 0x0010_FFFF
                        && !(0xD800..=0xDFFF).contains(&codepoint)
                        && let Some(c) = u32::try_from(codepoint).ok().and_then(char::from_u32)
                    {
                        out.push(c);
                    }
                }
            }
            "sc" | "ec" | "sm" | "em" => {}
            _ => collect_pgs_text(dom, child, plural, out),
        }
    }
}

fn icu_type(kind: &str) -> &'static str {
    match kind {
        "plural" => "plural",
        "ordinal" => "selectordinal",
        _ => "select",
    }
}

fn key_string(key: &Key) -> String {
    match key {
        Key::Int(i) => i.to_string(),
        Key::Str(s) => s.to_string_lossy().into_owned(),
    }
}

/// `formatIcuCase($caseValue, $switchType)`: `=` before a numeric case of
/// a plural or ordinal switch (an integer key is numeric).
fn format_icu_case(case: &Key, kind: &str) -> String {
    let value = key_string(case);
    let numeric = match case {
        Key::Int(_) => true,
        Key::Str(s) => is_numeric(s.as_bytes()),
    };
    if (kind == "plural" || kind == "ordinal") && numeric {
        format!("={value}")
    } else {
        value
    }
}

fn build_icu_message(switches: &[Switch], cases: &PhpArray) -> String {
    let Some((outer, inner)) = switches.split_first() else {
        return String::new();
    };
    let icu = icu_type(&outer.kind);
    let mut icu_cases = Vec::new();

    if inner.is_empty() {
        for (case, text) in cases.iter() {
            let text = text
                .as_string()
                .map(|s| s.to_string_lossy().into_owned())
                .unwrap_or_default();
            icu_cases.push(format!("{} {{{text}}}", format_icu_case(case, &outer.kind)));
        }
    } else {
        let mut grouped = PhpArray::new();
        for (case, text) in cases.iter() {
            let key = key_string(case);
            let (first, rest) = match key.split_once(' ') {
                Some((a, b)) => (a.to_owned(), b.to_owned()),
                None => (key.clone(), "other".to_owned()),
            };
            let first = string_key(&first);
            let mut group = grouped
                .get(&first)
                .and_then(Value::as_array)
                .cloned()
                .unwrap_or_default();
            group.insert(string_key(&rest), text.clone());
            grouped.insert(first, Value::Array(group));
        }
        for (case, group) in grouped.iter() {
            let group = group.as_array().cloned().unwrap_or_default();
            icu_cases.push(format!(
                "{} {{{}}}",
                format_icu_case(case, &outer.kind),
                build_icu_message(inner, &group)
            ));
        }
    }

    format!("{{{}, {icu}, {}}}", outer.variable, icu_cases.join(" "))
}

/// `utf8ToCharset($content, $encoding)`: `mb_convert_encoding()` to the
/// document's encoding, for the names php-dom reads a document in.
fn utf8_to_charset(content: &str, encoding: Option<&str>) -> Result<Vec<u8>, Error> {
    let Some(encoding) = encoding.filter(|e| *e != "UTF-8") else {
        return Ok(content.as_bytes().to_vec());
    };
    let mut out = Vec::with_capacity(content.len());
    match encoding {
        "UTF8" => out.extend_from_slice(content.as_bytes()),
        "UTF-16" | "UTF16" | "UTF-16BE" | "UTF-16LE" => {
            let le = encoding == "UTF-16LE";
            for unit in content.encode_utf16() {
                out.extend_from_slice(&if le {
                    unit.to_le_bytes()
                } else {
                    unit.to_be_bytes()
                });
            }
        }
        "ISO-8859-1" | "ISO8859-1" | "LATIN1" => {
            out.extend(
                content
                    .chars()
                    .map(|c| u8::try_from(u32::from(c)).unwrap_or(b'?')),
            );
        }
        "ASCII" | "US-ASCII" => {
            out.extend(
                content
                    .chars()
                    .map(|c| if c.is_ascii() { c as u8 } else { b'?' }),
            );
        }
        _ => {
            return Err(Error::new(
                Class::ValueError,
                format!(
                    "mb_convert_encoding(): Argument #2 ($to_encoding) must be a valid encoding, \"{encoding}\" given"
                ),
            ));
        }
    }
    Ok(out)
}
