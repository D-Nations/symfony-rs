# M1-C, XML gate X3: QtFileLoader

Stage: the third consumer of PLAN.md's gate X3, after XmlUtils
([m1-c-xml-x3.md](m1-c-xml-x3.md)) and the XLIFF loader
([m1-c-xml-x3-xliff.md](m1-c-xml-x3-xliff.md)). symfony/translation's Qt
Linguist loader is ported in `symfony-translation`, with the part of
`DOMXPath` it needs in `php-dom`. Recorded November 23 (PHP 8.4.27,
libxml 2.9.14).

## 1. What was ported

| PHP file | Rust | Tests |
|---|---|---|
| `Loader/QtFileLoader.php` | `symfony_translation::loader::qt_file_loader` | QtFileLoaderTest, 5 of 5 |
| `DOMXPath::evaluate()` (a subset) | `php_dom::xpath::evaluate` | recorded `evaluate` |

Other changes:
- **symfony-translation:** the resource checks the two loaders share
  (`stream_is_local()`, `file_exists()`) moved to `loader/local.rs`;
  `Class::PhpError` is PHP's `\Error`.

symfony/translation in `parity/symfony/translation.tsv`: 345 tests, 55
ported, 2 n/a, 288 pending.

## 2. Recorded

`oracle/recorders/translation_qt.php` (`scripts/record.sh translation-qt`)
writes `crates/translation/tests/data/qt.json`; `tests/recorded_qt.rs`
replays it. Each load gives the catalogue (locale, messages with their key
types, resources) or the throwable chain (class, message, code), and
`libxml_use_internal_errors()` before and after.

- **96 loads:** upstream's `resources.ts` with several domains and
  locales; 24 resources that are not `.ts` files (wrappers, `file://` URLs,
  missing files, directories, empty and invalid XML); 50 written documents
  (TS at any depth, several contexts and TS elements, namespaces and
  prefixes, text split by comments or elements, CDATA, character and
  entity references, a document type, missing and repeated elements,
  `'0'` and `''`, integer keys, text content of children); 10 domains that
  change the XPath expression.
- **38 `evaluate()` calls** over one document: absolute, relative and `//`
  paths, name and `text()` tests, predicates, unions, whitespace, and
  invalid expressions.

133 of 134 cases match; 1 load is listed and 1 `evaluate()` is refused
(section 4). Mutation check: dropping the namespace test on names, CDATA
in `text()`, the document element as the start of relative paths, the
`'0'` rule, the "exactly one" rule or the empty result for an invalid
expression each fails between 1 and 26 cases.

## 3. What PHP does here (kept)

- **The domain is XPath text.** It is pasted unescaped between `"`. A
  domain with an odd number of `"` gives an unterminated literal:
  `evaluate()` warns and returns `false`, `false->length` is `null`, and
  the catalogue is empty. A domain such as
  `x"] | //message/source[text()="a` makes a valid union, which is
  evaluated: it can match a `<source>`, and the walk then starts there.
  A NUL byte ends the expression (libxml reads a C string).
- **Exactly one match.** Two contexts with the same name give an empty
  catalogue and no resource.
- **The walk.** From the name, `nextSibling->parentNode->parentNode` is
  the TS; every `message` under it is read, in every context, by local
  name (a `q:message` too). A name with no next sibling is
  `Error: Call to a member function getElementsByTagName() on null`, and
  `libxml_use_internal_errors()` stays `true`.
- **Values.** The text content of the first `translation` (children
  included) when it is not `''` or `'0'`; the text content of the first
  `source`, `''` when there is none. A later message with the same source
  replaces the value in place.
- **Names.** `TS`, `context` and `name` match only elements with no
  namespace; a `text()` child equal to the domain is enough
  (`resources<b/>other` matches, `res<!-- c -->ources` does not), and CDATA
  is a `text()` node.
- **Relative paths** in `evaluate()` start at the document element.

## 4. Not matched

| Case | PHP | Port |
|---|---|---|
| `load('file://localhost/…')` | a `FileResource` for the URL as given | symfony/config's "does not exist" InvalidArgumentException (the XLIFF loader's listed case) |
| `evaluate('//q:context')` (a prefixed name) | the element, with the document's namespaces registered | `XPathError::Refused` |
| A domain that makes valid XPath outside the subset | evaluated | `Class::Refused` |

The first two rows are recorded and listed (`is_known()` and
`REFUSED_EXPRESSIONS` in `tests/recorded_qt.rs`). No recorded load falls
in the third row.

## 5. Gate status

X3 passes for QtFileLoader on the recorded corpus. Open: Validator's
XmlFileLoader (M5-A).

As for the XLIFF stage, the oracle image could not be rebuilt in this
session; the recorder ran in the existing `symfony-rs-oracle:8.4.27`
image with the pinned upstream checkouts behind a stand-in autoloader.
