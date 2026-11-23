# Symfony Translation

Private workspace crate: a port of part of `symfony/translation` v8.1.5
(`d9e1caba`), as installed by symfony/demo. MIT; Symfony source and tests
copyright Fabien Potencier. Unofficial, not affiliated with or endorsed by
Symfony. The XLIFF schemas in `resources/schemas/` are the files
symfony/translation ships (OASIS XLIFF 1.2 transitional, 2.0 and 2.2;
W3C `xml.xsd`), under their own notices.

So far: the XLIFF and Qt loaders and what they need (PLAN.md gate X3).
Stage logs: [m1-c-xml-x3-xliff.md](../../docs/milestones/m1-c-xml-x3-xliff.md),
[m1-c-xml-x3-qt.md](../../docs/milestones/m1-c-xml-x3-qt.md).

| Contract | Source | Evidence |
|---|---|---|
| MessageCatalogue: messages by domain with `+intl-icu` domains (`all()`, `get()`, `set()`, `add()`, `replace()`, `has()`, `defines()`, `getDomains()`), metadata and catalogue metadata (get, set, delete, merged by `addCatalogue()`), resources keyed by their text, fallback catalogues with both circular-reference checks; PHP's array keys (integer strings are integers) and order | `MessageCatalogue.php`, `MessageCatalogueInterface.php`, `MetadataAwareInterface.php`, `CatalogueMetadataAwareInterface.php` | MessageCatalogueTest (18 of 18); every recorded catalogue |
| XliffUtils: `getVersionNumber()` (first `xliff` element by local name, first `version` attribute by local name, else `1.2`), `validateSchema()` against the 1.2, 2.0 (also for 2.1) and 2.2 schemas with `xml.xsd` supplied for the rewritten URL only, `getErrorsAsString()` (every line `ERROR`, as upstream's level comparison gives), `getFileUrl()` | `Util/XliffUtils.php` | XliffUtilsTest (2 of 4, 2 n/a); recorded `file_urls`, `versions`, every validation error text |
| XliffFileLoader: XML strings and files, the wrapper rules of `stream_is_local()` / `file_exists()` / `is_file()`, XmlUtils errors wrapped in InvalidResourceException with their code, schema errors; XLIFF 1.2 (resname, the needs-translation rule, notes with `priority` and `from`, target attributes, ids, `prop` catalogue metadata, several files) and XLIFF 2 (names, segments, segment and target attributes, notes, `mda:meta`, pgs units built into ICU messages in the `+intl-icu` domain); SimpleXML's view of the document; targets and notes converted to the document's encoding as `mb_convert_encoding()` does, its ValueError for names it does not know; the TypeError a pgs unit with notes raises; symfony/config's `FileResource` for a file | `Loader/XliffFileLoader.php`, `Loader/LoaderInterface.php` | XliffFileLoaderTest (31 of 31); 260 recorded loads (2 listed) |
| QtFileLoader: the same wrapper rules without `is_file()`, XmlUtils's `loadFile()` errors as `Unable to load "%s".` with the code and previous exception, the domain pasted unescaped into `//TS/context/name[text()="%s"]` (php-dom's XPath subset; an invalid expression finds nothing), exactly one match, every `message` under its TS by local name, first `translation` and `source` text content, truthy translations only, PHP's array keys, the `\Error` for a name with no next sibling (internal errors left on), symfony/config's `FileResource` | `Loader/QtFileLoader.php` | QtFileLoaderTest (5 of 5); 96 recorded loads (1 listed), 38 recorded `evaluate()` calls (1 refused) |
| The exceptions the loaders throw | `Exception/*.php` (6 files) | through the tests above |

Vectors: `scripts/record.sh translation-xliff` runs
`oracle/recorders/translation_xliff.php` and writes `tests/data/xliff.json`,
which `tests/recorded_xliff.rs` replays; `scripts/record.sh translation-qt`
runs `oracle/recorders/translation_qt.php` and writes `tests/data/qt.json`,
which `tests/recorded_qt.rs` replays.

Differences from PHP, all stated:
- symfony/config's `FileResource` does not resolve two names as PHP does,
  so loading them fails with its "does not exist" InvalidArgumentException
  where PHP adds the resource: `foo://bar` (PHP's `realpath()` reads it as
  the relative path `foo:/bar`) and a `file://localhost/…` URL (PHP's
  `file_exists()` reads it). Listed in `tests/recorded_xliff.rs` and
  `tests/recorded_qt.rs` (`is_known()`); the fix belongs in symfony-config.
- QtFileLoader: a domain that makes the XPath expression valid but
  outside php-dom's subset (for example with a `p:name` step) is refused
  (`Class::Refused`); PHP would evaluate it.
- A `phar://` URL into an existing archive is refused (`Class::Refused`);
  PHP would read the archive.
- A document php-dom cannot validate (a construct outside its schema
  feature set) is an InvalidResourceException carrying php-dom's refusal
  text, as PLAN.md decision 14 does for parse refusals.
- No PHP warnings: an unknown stream wrapper (`foo://`) is a plain path,
  as in PHP, without its warning.
- A fallback catalogue's link to its parent is weak; PHP keeps the parent
  alive.
