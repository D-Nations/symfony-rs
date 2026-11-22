# M1-C, XML gate X3: XmlUtils (first consumer)

Stage: the first consumer half of PLAN.md's gate X3. `Config/Util/XmlUtils.php`
and its two exceptions are ported on `php-dom` (X1 parser, X2 schemas;
[m1-c-xml-x1.md](m1-c-xml-x1.md), [m1-c-xml-x2.md](m1-c-xml-x2.md)). The
loaders that use XmlUtils (XLIFF, Qt, Validator's XML mapping) are later
stages and are not part of it. Started and recorded November 22 on the
oracle (PHP 8.4.27, libxml 2.9.14).

## 1. What was ported

| PHP file | Rust | Tests |
|---|---|---|
| `Util/XmlUtils.php` | `symfony_config::util::xml_utils` (`XmlUtils::parse`, `load_file`, `convert_dom_element_to_array`, `phpize`, `get_xml_errors`) | XmlUtilsTest, 7 of 7 |
| `Util/Exception/XmlParsingException.php`, `InvalidXmlException.php` | `symfony_config::util::exception`; `Class::XmlParsing`, `Class::InvalidXml` (both `is_a(&Class::InvalidArgument)`) | through XmlUtilsTest |

`php-dom` gained `libxml_use_internal_errors()` (a per-thread flag; PHP's
is per request) and a `Display` for schema refusals. No other XmlUtils
operation needed new DOM API: the X1 inventory listed them all. XmlUtils
itself needs no SimpleXML or XPath; those belong to the loaders.

## 2. Recorded

`oracle/recorders/config_xml_utils.php` (`scripts/record.sh
config-xml-utils`, also run by `scripts/record.sh config`) writes
`crates/config/tests/data/xml_utils.json`; `tests/recorded_xml_utils.rs`
replays it:

- **`phpize()`:** 3,045 values. Upstream's provider, edge cases of every
  branch (case of `null`/`true`/`false`, leading zeros and octal, int
  limits and overflow, `0x`/`0X` and `0b` with a final newline, which
  PCRE's `$` accepts, floats with whitespace, `INF`, non-ASCII digits),
  and generated strings over the characters the branches test.
- **`convertDomElementToArray()`:** 71 documents, with and without the
  prefix check.
- **`parse()`:** 97 outcomes (exception class, message, code, previous
  exception): malformed documents, document types, `''`, schema files
  (valid, invalid, malformed, not a schema, duplicate declarations,
  missing, a directory), imports by relative path, absolute path,
  `file:` URL and `%20`, from two working directories, and 15 callables
  (results of every type, `\Exception` and `\Error`, callables that
  validate and leave libxml errors).
- **`loadFile()`:** 23 outcomes (directory, missing, empty and
  whitespace-only files, malformed, document type, schema and callable
  results, relative names).
- The internal-errors flag a callable receives, under both settings, and
  that `parse('')` leaves the flag set.

A deliberate change to the error-list format, the attribute prefix check
or `bindec()`'s overflow each fails the replay.

## 3. Findings

- **Imports** of a schema read from memory (`schemaValidateSource()`)
  resolve against the working directory, not the schema file's directory.
  `%XX` escapes are decoded; a location with a raw space fails without
  libxml's usual "failed to load external entity" warning. The port
  reads exactly the files PHP would read (`file:` URLs and paths); any
  other scheme, a space, or a file that cannot be read is not supplied,
  and php-dom refuses the schema (`UnresolvedImport`).
- **Schema errors name the document** as the working directory plus `/`.
  The port uses `std::env::current_dir()`. That is PHP's form on Linux,
  where it was recorded; PHP's form on Windows was not recorded.
- **A string that names a PHP function is a callable.** `parse($c, 'dir')`
  calls `dir($dom, $internalErrors)` and fails with dir()'s TypeError. In
  Rust the caller chooses `Validation::Schema` or `Validation::Callable`,
  so the case is not replayed.
- **`parse('')`** throws `loadXML()`'s ValueError before XmlUtils restores
  `libxml_use_internal_errors()`, so the setting stays on. A callable's
  `\Error` does the same. Both are kept.
- **Validity errors.** With `validateOnParse`, a malformed document's
  errors start with `[ERROR 522] Validation failed: no DTD found !`. X1
  already gives it.
- **`convertDomElementToArray()`:**
  - A CDATA section is text.
  - The last non-empty text node wins.
  - A processing instruction or an entity reference among the children
    reaches the `\DOMElement` parameter and is PHP's TypeError; the port
    gives it without ", called in <file> on line <n>". The prefix check
    skips such nodes for a prefixed element.
  - An empty element seen twice keeps one `null`, because `isset()` is
    false for `null`.
  - Attributes with the same local name keep the last one.

## 4. Not matched

| Case | PHP | Port |
|---|---|---|
| An import of `part.xsd` that is not in the working directory | warnings 1549 and 3084, then error 3004 | XmlParsingException: `imported schema "part.xsd" is not loaded (line 1, column 0)` |
| An import of `imp/a b.xsd` (raw space) | error 3004 | the same refusal |

Both are listed in the replay's `REFUSED` table, which checks the refusal.
PLAN.md decision 14 (accepted) covers them.

## 5. Gate status

X3 **passes for XmlUtils** on the recorded corpus: XmlUtilsTest 7 of 7, and
every recorded `phpize()`, conversion, `parse()` and `loadFile()` outcome
matches, except the two refused imports above. X3 stays open for the
loaders that consume it:
- XliffFileLoader and XliffUtils: the `xml.xsd` resolver mapping and the
  SimpleXML/XPath subset.
- QtFileLoader: DOMXPath `evaluate()`.
- Validator's XmlFileLoader: SimpleXML iteration (M5-A).
