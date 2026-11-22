# M1-C, XML gate X1: the parser (`php-dom`)

Stage: the parser half of PLAN.md's XML gates. X2 (XSD validation) and X3
(`XmlUtils` and its consumers) are not part of it. Started and recorded
November 22 on the oracle (PHP 8.4.27, libxml 2.9.14, `LIBXML_DOTTED_VERSION`
pinned in the vectors).

## 1. What the consumers need

Read from the pinned sources (`upstream/symfony`). At this pin,
symfony/dependency-injection and symfony/routing ship **no XML file
loaders** any more: `Loader/` holds only the YAML, PHP, closure, glob and
directory loaders, and `Loader/schema/` is leftover. The XML consumers are:

| Consumer | Through | DOM / libxml use |
|---|---|---|
| `Config/Util/XmlUtils.php` | itself | `new DOMDocument`, `validateOnParse = true`, `loadXML($s, LIBXML_NONET \| LIBXML_COMPACT)`, `normalizeDocument()`, `childNodes` + `nodeType` (rejects `XML_DOCUMENT_TYPE_NODE`), `schemaValidateSource()` (X2), `libxml_use_internal_errors`, `libxml_get_errors` (level, code, message, file, line, column), `libxml_clear_errors`; in `convertDomElementToArray`: `prefix`, `attributes` (name ⇒ node, `->prefix`, `->value`), `childNodes`, `DOMText` / `DOMComment` instance checks, `nodeValue`, `localName` |
| `Translation/Loader/XliffFileLoader.php`, `Util/XliffUtils.php` | `XmlUtils::parse` / `loadFile` | `getElementsByTagName('xliff')`, `attributes->getNamedItem('version' \| 'xmlns')`, `nodeValue`, `encoding`, `schemaValidateSource` (X2), `normalizeDocument`; `simplexml_import_dom` with `registerXPathNamespace` + `xpath('//xliff:file')`, `'.//xliff:prop'`, `'.//xliff:trans-unit'`, `'//mda:meta'`, `'//xliff:unit'`, child access (`->source`, `->target`, `->note`, `->segment`, `->notes`), `attributes()` with and without a namespace; `dom_import_simplexml`, `childNodes`, `nodeType` (text, CDATA, element), `textContent`, `localName`, `getAttribute` |
| `Translation/Loader/QtFileLoader.php` | `XmlUtils::loadFile` | `new DOMXPath`, `evaluate('//TS/context/name[text()="…"]')`, `item(0)`, `nextSibling`, `parentNode`, `getElementsByTagName('message' \| 'translation' \| 'source')`, `nodeValue` |
| `Validator/Mapping/Loader/XmlFileLoader.php` | `XmlUtils::loadFile` with an XSD (X2) | `simplexml_import_dom`, SimpleXML iteration, `XmlUtils::phpize` |
| `Validator/Constraints/XmlValidator.php` | itself | `simplexml_load_string($v, …, LIBXML_NONET)`, `dom_import_simplexml(...)->ownerDocument`, `schemaValidateSource`, the error list |
| `DependencyInjection/Loader/IniFileLoader.php` | `XmlUtils::phpize` only | none |

So X1 must give: `loadXML()` with and without `validateOnParse`, the tree
(document, document type, element, attribute, text, CDATA, comment, PI,
entity reference) with names, prefixes, namespace URIs, values and line
numbers, the read-only navigation above, and libxml's error list exactly.
X3 adds the SimpleXML view and the small XPath subset (descendant and
child steps with a namespace prefix, one `text()="…"` predicate) on top.

## 2. Options (searched again November 22)

The stack row in PLAN.md is updated. New since November 19: **exml**
(a C-to-Rust translation of libxml2 **2.11.8**; part of its modules are
still `unsafe`), **xmloxide** (a pure Rust reimplementation of libxml2,
W3C conformance suite, DOM arena) and **oxml** (parser, tree, XPath,
entity budgets, never fetches). None targets 2.9.14, whose messages and
recovery the oracle records (2.11 rewrote several messages and the
recovery after errors); exml's `unsafe` and translated C conflict with the
code rules. Decision 9's hand-written crate stands.

## 3. The recording

`oracle/recorders/php_dom_parse.php` (`scripts/record.sh php-dom` →
`crates/php-dom/tests/data/parse.json`, one case per line). Each input is
loaded twice: with XmlUtils's settings (and `XmlUtils::parse()` itself),
and as a plain `new DOMDocument(); loadXML()`. Recorded: the `loadXML`
result, every `libxml_get_errors()` entry (level, code, trimmed message,
line, column) and the tree after `normalizeDocument()`.

472 documents: 377 written for the gate (well-formed documents,
namespaces, encodings including UTF-16 with and without a byte-order mark
and ISO-8859-1, CDATA, comments, PIs, character and entity references,
nesting to depth 10,000, a 60,000-character name and a 20 MB attribute,
every malformed construct the parser has a branch for, document types,
internal subsets, parameter entities, XXE attempts through `file://`,
`http://` and `php://filter`, billion laughs, quadratic blowup and
recursive entities) and 95 files (upstream's XML fixtures from Config,
Translation and Validator, and the demo's 58 translation files). 1,346
recorded errors.

What the recording found (all now pinned by the replay test):

- `validateOnParse` adds `[ERROR 522] Validation failed: no DTD found !` to
  every document without declarations, at the end of the root start tag;
  it is not reported once a fatal error has disabled the tree.
- libxml carries on after most fatal errors: an attribute error cascades
  into "attributes construct error", "Couldn't find end of Start Tag" and
  "Extra content at the end of the document"; an end tag always closes
  the current element (each mismatch reported once); an invalid character
  is skipped. Depth (257 open elements), document end, unsupported
  encodings and some content errors stop it.
- Columns count characters. Quirks kept: a lone CR reads as LF without
  starting a line; `]]>` in text skips one byte without moving the column;
  after "'?>' expected" in the declaration the parser jumps to the next
  `>` without moving the column.
- Invalid UTF-8 with at least four bytes left reports the bytes and
  switches the rest of the input to ISO-8859-1; with fewer it is the end
  of input. A NUL byte ends the input. US-ASCII input stops silently at
  the first byte above 0x7F.
- Entities are never substituted in content (reference nodes), but each
  is parsed once on first reference; a recursive entity reports 21 loop
  errors (one per nesting level until the depth limit, two levels per
  reference). In attribute values they are expanded. External entities are
  not substituted, but with `validateOnParse` libxml **reads** an external
  DTD subset, external parameter entities and external entities referred
  to in content (the recording reads `/etc/debian_version` that way).
- `<![CDATA[` with fewer than two characters before the end reports
  "Unregistered error message" (libxml has no text for that code there).
- Errors inside a parameter entity are reported at the position of the
  text that referred to it.

## 4. The crate

`crates/php-dom`: `load_xml(bytes, LoadOptions { validate_on_parse })`
→ `Loaded { document: Option<Document>, errors }` or a `LoadError`
(`Empty` for PHP's `ValueError`; `Refused` with a line and column). No
dependency, no `unsafe`. Nothing is read from a file or the network: what
libxml would fetch is refused, which is also what the gate requires.
Depth is bounded at libxml's 256, names at 50,000 bytes, attribute values
at 10,000,000, entity nesting at libxml's 40, and attribute-entity
expansion at 10,000,000 characters in total.

Result: **457 of 472 recorded documents give the same answer in both
modes** (load result, every error, the tree). The other 15 are listed,
with the reason, in `tests/parse.rs`'s `KNOWN` (the test fails if one of
them starts matching, or anything else differs):

- 8 refused because PHP would read a file or the network (external DTD
  subset, external entity referenced in content, external parameter
  entity);
- 5 refused for their encoding (windows-1252, ISO-8859-15 and an unknown
  name are iconv's in PHP; one UTF-16 document with a lone surrogate);
- 2 billion-laughs documents: both fail, but libxml's amplification check
  (which depends on bytes consumed) reports 9 and 7 loop errors where
  this crate reports its own bound.

Not claimed: validity errors (5xx) and I/O warnings (15xx) for documents
with a document type (XmlUtils rejects every document type), the
document type's `internalSubset` text, DTD-defaulted attributes and
namespaces, `normalizeDocument()` (an identity on every recorded parse;
X3 decides whether it needs more).

## 5. Gate status

X1 is **passed for the recorded corpus, with the refusals above**.
Open for the owner: decision 14 in PLAN.md (how X3 reports refusals; the
DI/Routing loaders no longer exist at the pin). X2 and X3 come next.
