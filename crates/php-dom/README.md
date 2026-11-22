# php-dom

Private workspace crate (PLAN.md decision 9): the part of PHP's DOM
extension that Symfony's XML consumers use, with the diagnostics libxml
reports while parsing. MIT. No PHP or libxml source is used: every rule is
written from PHP 8.4.27's observed behaviour on libxml 2.9.14, recorded in
the oracle (`scripts/record.sh php-dom` → `tests/data/parse.json`), and the
XML 1.0 and Namespaces specifications. Stage log:
[docs/milestones/m1-c-xml-x1.md](../../docs/milestones/m1-c-xml-x1.md).

Nothing here reads a file or the network. What libxml would fetch is
refused (`LoadError::Refused`, with a line and column).

| Contract | Evidence |
|---|---|
| `load_xml(bytes, LoadOptions { validate_on_parse })` = `DOMDocument::loadXML()` with XmlUtils's settings (`validateOnParse`, `LIBXML_NONET \| LIBXML_COMPACT`) or plain: whether it loads, and `libxml_get_errors()` (level, code, trimmed message, line, column), in libxml's order and with its recovery after errors | 457 of 472 recorded documents, both modes, every error |
| The tree: document (`encoding`, `xmlVersion`, `xmlStandalone`), document type (`name`, `publicId`, `systemId`), elements (`nodeName`, `localName`, `prefix`, `namespaceURI`, `getLineNo()`, attributes with the same fields and `value`), text, CDATA, comments, PIs, entity references | Same documents |
| Read-only navigation: `childNodes`, `parentNode`, siblings, `documentElement`, `textContent`, `getAttribute`, `hasAttribute`, `getElementsByTagName` | Unit use in consumers (X3) |
| Encodings: UTF-8 (with or without a byte-order mark), UTF-16 LE/BE (byte-order mark or `<?` detection), ISO-8859-1 and its glibc names, US-ASCII; invalid UTF-8 as libxml reports it | Recorded encoding cases |
| Bounds: 256 open elements, 50,000-byte names, 10,000,000-byte attribute values, entity nesting 40, 10,000,000 characters of attribute-entity expansion | Recorded limit cases |
| `''` → `LoadError::Empty` (PHP's `ValueError` text) | Recorded |

Refused (`LoadError::Refused`), where PHP would read a file or the network
or use iconv: an external DTD subset or an external parameter entity
with `validate_on_parse`, a reference in content to an external entity
with `validate_on_parse`, encodings other than the ones above, UCS-4 and
EBCDIC, invalid UTF-16.

Not claimed: validity errors (5xx) and I/O warnings (15xx) for documents
with a document type; `internalSubset`; DTD-defaulted attributes and
namespace declarations; libxml's entity-amplification check (billion
laughs fails here too, with different errors); `normalizeDocument()`
(an identity on every recorded parse); SimpleXML, XPath and XSD (gates X2,
X3).
