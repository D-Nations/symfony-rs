# php-dom

Private workspace crate (PLAN.md decision 9): the part of PHP's DOM
extension that Symfony's XML consumers use, with the diagnostics libxml
reports while parsing and validating against an XML Schema. MIT. No PHP or libxml source is used: every rule is
written from PHP 8.4.27's observed behaviour on libxml 2.9.14, recorded in
the oracle (`scripts/record.sh php-dom` → `tests/data/parse.json`,
`scripts/record.sh php-dom-schema` → `tests/data/schema.json`), and the
XML 1.0, Namespaces and XML Schema 1.0 specifications. Stage logs:
[m1-c-xml-x1.md](../../docs/milestones/m1-c-xml-x1.md) (parser),
[m1-c-xml-x2.md](../../docs/milestones/m1-c-xml-x2.md) (schemas).

Nothing here reads a file or the network. What libxml would fetch is
refused (`LoadError::Refused`, with a line and column).

| Contract | Evidence |
|---|---|
| `load_xml(bytes, LoadOptions { validate_on_parse })` = `DOMDocument::loadXML()` with XmlUtils's settings (`validateOnParse`, `LIBXML_NONET \| LIBXML_COMPACT`) or plain: whether it loads, and `libxml_get_errors()` (level, code, trimmed message, line, column), in libxml's order and with its recovery after errors | 457 of 472 recorded documents, both modes, every error |
| The tree: document (`encoding`, `xmlVersion`, `xmlStandalone`), document type (`name`, `publicId`, `systemId`), elements (`nodeName`, `localName`, `prefix`, `namespaceURI`, `getLineNo()`, attributes with the same fields and `value`), text, CDATA, comments, PIs, entity references | Same documents |
| Read-only navigation: `childNodes`, `parentNode`, siblings, `documentElement`, `textContent`, `getAttribute`, `hasAttribute`, `getElementsByTagName` (by local name, as PHP 8.4 compares it) | Unit use in consumers (X3); symfony-translation's recorded `tag_names` |
| Encodings: UTF-8 (with or without a byte-order mark), UTF-16 LE/BE (byte-order mark or `<?` detection), ISO-8859-1 and its glibc names, US-ASCII; invalid UTF-8 as libxml reports it | Recorded encoding cases |
| Bounds: 256 open elements, 50,000-byte names, 10,000,000-byte attribute values, entity nesting 40, 10,000,000 characters of attribute-entity expansion | Recorded limit cases |
| `''` → `LoadError::Empty` (PHP's `ValueError` text) | Recorded |
| `Schema::parse(bytes, resolver)` and `Schema::validate(&doc, file)` = `schemaValidateSource()` after XmlUtils's `loadXML()`: the result and `libxml_get_errors()` (level, code, message, file, line, column), in libxml's order, for the schema features listed in the X2 stage log (those of the XLIFF 1.2/2.0/2.2, `xml.xsd` and constraint-mapping schemas) | 1,321 of 1,325 recorded documents against 65 schemas, 922 errors; the other 4 refused |
| Schema errors libxml reports for a malformed schema (not well-formed, not a schema, duplicate global element, unresolved type or element reference, unknown top-level element) | Recorded |
| Imports read only through the caller's resolver; patterns matched in linear time | `imports_come_only_from_the_resolver`, `linear_on_nested_quantifiers` |
| `libxml_use_internal_errors($use)`: sets the flag and returns the previous value (`None` reads it); per thread, starting `false`. Nothing here raises warnings or keeps a global error buffer, so the flag is what callers read back (XmlUtils, [m1-c-xml-x3.md](../../docs/milestones/m1-c-xml-x3.md)) | `returns_the_previous_value`; symfony-config's recorded `flags` |

Refused (`LoadError::Refused`), where PHP would read a file or the network
or use iconv: an external DTD subset or an external parameter entity
with `validate_on_parse`, a reference in content to an external entity
with `validate_on_parse`, encodings other than the ones above, UCS-4 and
EBCDIC, invalid UTF-16.

Refused (`SchemaError::Refused`, `RefusalReason::UnsupportedSchema` with
the schema line) for any schema construct outside that list, and
(`UnresolvedImport`) for an import the resolver does not supply; at
validation (`UnsupportedInstance`), `xsi:type` and entity references.

Differences from the XML Schema recommendation kept because libxml 2.9.14
has them (all recorded): `dateTime` values are not whitespace-collapsed,
`NMTOKENS` accepts an empty list, `anyURI` accepts spaces, a `fixed` value
on an attribute reference is not checked, and the "Expected is ( … )"
lists follow libxml's automaton rather than the set of allowed names.

Not claimed: validity errors (5xx) and I/O warnings (15xx) for documents
with a document type; `internalSubset`; DTD-defaulted attributes and
namespace declarations; libxml's entity-amplification check (billion
laughs fails here too, with different errors); `normalizeDocument()`
(an identity on every recorded parse); `schemaValidate($path)` reading a
file (the caller reads it); SimpleXML (symfony-translation reads the
tree as SimpleXML would) and XPath (QtFileLoader, still open in X3).
