//! The container dumpers: `XmlDumper`, `YamlDumper` and `GraphvizDumper`
//! write a `ContainerBuilder` as XML, YAML or a Graphviz dot graph.
//!
//! `PhpDumper` and `Preloader` are not ported: the Rust emitter replaces
//! them.
//!
//! The PHP `Symfony\Component\ExpressionLanguage\Expression` values that
//! the XML and YAML dumpers write are any object of that class; its text
//! is its string form (`StringableObject`).
//!
//! Upstream: none

#[expect(
    clippy::module_inception,
    reason = "one file per upstream file: Dumper.php"
)]
mod dumper;
mod dumper_interface;
mod xml_dumper;
mod yaml_dumper;

pub use dumper::Dumper;
pub use dumper_interface::DumperInterface;
pub use xml_dumper::XmlDumper;
pub use yaml_dumper::YamlDumper;

use php_value::{PhpString, Value};

use crate::reference::Reference;

/// `Symfony\Component\ExpressionLanguage\Expression`.
pub(crate) const EXPRESSION: &str = "Symfony\\Component\\ExpressionLanguage\\Expression";

/// `(string) $value` of an `Expression`, when `value` is one.
pub(crate) fn expression_text(value: &Value) -> Option<PhpString> {
    let object = value.as_object()?;
    if object.class() != EXPRESSION {
        return None;
    }
    Some(object.to_php_string().unwrap_or_default())
}

/// `(string) $value` for the values a dumper interpolates: scalars as
/// PHP casts them, a `Reference` as its id, a `\Stringable` object as
/// its string form, anything else as `""`.
pub(crate) fn to_php_string(value: &Value) -> PhpString {
    if let Some(reference) = Reference::from_value(value) {
        return PhpString::from(reference.id());
    }
    match value {
        Value::Array(_) => PhpString::from("Array"),
        other => other.to_php_string_or_stringable().unwrap_or_default(),
    }
}
