# M1-C, XML gate X3: XliffFileLoader and XliffUtils

Stage: the second consumer of PLAN.md's gate X3, after XmlUtils
([m1-c-xml-x3.md](m1-c-xml-x3.md)). symfony/translation's XLIFF loader and
what it needs are ported on `php-dom` and symfony-config's XmlUtils, in a
new crate, `symfony-translation`. Recorded November 23 (PHP 8.4.27,
libxml 2.9.14).

## 1. What was ported

| PHP file | Rust | Tests |
|---|---|---|
| `Loader/XliffFileLoader.php`, `Loader/LoaderInterface.php` | `symfony_translation::loader::xliff_file_loader` | XliffFileLoaderTest, 31 of 31 |
| `Util/XliffUtils.php` | `symfony_translation::util::xliff_utils` | XliffUtilsTest, 2 of 4 (2 n/a) |
| `MessageCatalogue.php` and its three interfaces | `symfony_translation::message_catalogue` | MessageCatalogueTest, 18 of 18 |
| `Exception/` (the 6 classes the loader uses) | `symfony_translation::exception` | through the above |

Other changes:
- **php-dom:** `get_elements_by_tag_name()` compares the local name. PHP
  8.4's `DOMDocument::getElementsByTagName('b')` finds `<x:b>` too, and
  `'x:b'` finds nothing (recorded as `tag_names`). XliffUtils was its
  first user.
- **symfony-config:** `XmlUtils::load_file_named()`, `loadFile()` for a
  `$file` that the file system finds elsewhere (a `file://` URL). Its
  messages name the URL, as PHP's do.

symfony/translation is now in `parity/symfony/translation.tsv`: 345 tests,
50 ported, 2 n/a, 293 pending.

## 2. Recorded

`oracle/recorders/translation_xliff.php` (`scripts/record.sh
translation-xliff`) writes `crates/translation/tests/data/xliff.json`;
`tests/recorded_xliff.rs` replays it. Each load gives the catalogue
(locale, messages, metadata, catalogue metadata, resources as their
`__toString()`) or the exception chain (class, message, code), and
`libxml_use_internal_errors()` afterwards.

- **260 loads:**
  - every upstream XLIFF fixture, by relative and absolute path;
  - 42 resources that are not XML: stream wrappers, `file://` URLs with and
    without a host, missing files, directories, near-XML strings;
  - written documents, as strings and files: XLIFF 1.2 (resname, numeric
    ids, duplicates, the needs-translation rule, inline elements, CDATA,
    comments, notes, target attributes, groups, props, prefixed and
    foreign elements and attributes, versions, invalid documents);
    XLIFF 2 (names, segments, notes, metadata, groups, pgs units of every
    switch kind, malformed switches, `cp` code points); encodings: 13
    declared names, UTF-16 files with and without a byte-order mark, a
    UTF-8 byte-order mark.
- **14 `getFileUrl()` paths, 13 `getVersionNumber()` documents, 20
  `getElementsByTagName()` calls,** and `validateSchema()` on a document
  declaring an external entity.

306 of 308 cases match; 2 are listed (section 4). Mutation check: dropping the prefix rule for
children, CDATA in string casts, the namespace filter on attributes, the
mbstring name check, half of the needs-translation rule or the `=` for
integer case keys each fails between 2 and 11 cases.

## 3. What PHP does here (kept)

- **SimpleXML.** `$el->source` sees children with no prefix only: a
  `<x:source>` in the XLIFF namespace is ignored, a `<source xmlns="">`
  is seen. `(string)` joins the text and CDATA of direct children (a
  `<g>` inside a target loses its text). `attributes()` lists attributes
  without a namespace (`xml:lang` is not there). An attribute is truthy
  even when empty or `"0"`, so `resname=""` gives the key `""`.
- **Keys.** A source or name that is a decimal integer is an integer key.
- **pgs.** A pgs unit with `<notes>` always fails with
  `array_map(): Argument #2 ($array) must be of type array,
  SimpleXMLElement given` (upstream passes `attributes()`, an object).
  Case keys that are integers get `=` for plural and ordinal switches, as
  do numeric strings such as `1.5` and ` 1`.
- **Encodings.** Targets, notes (XLIFF 1.2) and pgs messages are
  converted to the declared encoding. Of the names php-dom reads,
  `ISO-LATIN-1`, `ISO LATIN 1`, `ISO_8859-1` and `L1` are unknown to
  mbstring: a ValueError, but only once a unit is converted. `UTF-16` is
  written big-endian without a byte-order mark; characters outside
  Latin-1 or ASCII become `?`.
- **Versions.** `getVersionNumber()` takes the first `version` attribute
  by local name (`v:version` counts). Its `xmlns` fallback never applies:
  namespace declarations are not attributes. An unsupported version is
  Translation's InvalidArgumentException, unwrapped, and leaves
  `libxml_use_internal_errors()` on.
- **Errors.** Every schema error line says `ERROR`: upstream compares the
  string level with the integer `LIBXML_ERR_WARNING`.
- **Resources.** `http`, `https`, `ftp`, `ftps` and `data` URLs (any
  case) and `file://host/…` are not local; `file://localhost/…` is.
  `php://`, `glob://`, `compress.zlib://` and a `phar://` path without an
  archive are not found. An unknown scheme (`foo://bar`) is a relative
  path (`foo:/bar`).

## 4. Not matched

| Case | PHP | Port |
|---|---|---|
| A `phar://` URL into an existing archive | reads the file in the archive | `Class::Refused` |
| `load('foo://bar')` with a file `foo:/bar` | a `FileResource` for the real path of `foo:/bar` | symfony/config's "does not exist" InvalidArgumentException |
| `load('file://localhost/…')` | a `FileResource` for the URL as given | the same exception |

No recorded case falls in the first row. The other two are recorded and
listed (`is_known()` in `tests/recorded_xliff.rs`): symfony/config's
`resolve()` skips `realpath()` for any `scheme://` name, and symfony-finder's
`file_exists()` does not read `file://localhost/`. Both fixes belong to
those crates.

## 5. Gate status

X3 passes for XliffFileLoader and XliffUtils on the recorded corpus. Open:
QtFileLoader
(DOMXPath `evaluate()`), Validator's XmlFileLoader (M5-A).

The oracle image could not be built in this session (its `apt-get` step
had no network); the recorder ran on the same base image
(`php:8.4.27-cli-bookworm`, libxml 2.9.14) without intl and pdo_sqlite,
which the recorder does not use. The packages were the pinned upstream
checkouts (`scripts/fetch-upstream.sh`) behind a stand-in autoloader.
