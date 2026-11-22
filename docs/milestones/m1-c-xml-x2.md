# M1-C, XML gate X2: schema validation (`php-dom`)

Stage: the XSD half of PLAN.md's XML gates, on top of X1's parser
([m1-c-xml-x1.md](m1-c-xml-x1.md)). X3 (`XmlUtils` and its consumers) is
not part of it. Recorded November 22 on the oracle (PHP 8.4.27, libxml
2.9.14).

## 1. Which schemas, and what they use

At this pin, Symfony validates against these schemas:

| Consumer | Call | Schema |
|---|---|---|
| `Translation/Util/XliffUtils::validateSchema()` (XliffFileLoader) | `schemaValidateSource()` with the `xml.xsd` import rewritten to a `file://` URL | `xliff-core-1.2-transitional.xsd` (1.2), `xliff-core-2.0.xsd` (2.0 and 2.1), `xliff-core-2.2.xsd` (2.2), each importing `xml.xsd` |
| `Validator/Mapping/Loader/XmlFileLoader` | `XmlUtils::loadFile($path, $xsd)` → `schemaValidateSource()` | `constraint-mapping-1.0.xsd` |
| `Config/Util/XmlUtils::parse()` | `schemaValidateSource()` of a file, or a callable | any; upstream's tests use `Tests/Fixtures/Util/schema.xsd` |
| `Validator/Constraints/XmlValidator` | `schemaValidate($path, $flags \| LIBXML_NONET)` | the application's; upstream's tests use `example.xsd` and the malformed `invalid.xsd` |

The XSD features those schemas use, which are what this stage supports:

- **Schema documents:** `schema` (`targetNamespace`, `elementFormDefault`), `import` (namespace and location), `annotation` (ignored, wherever it appears).
- **Declarations:** global and local `element` (`name` or `ref`, `type` or an anonymous `complexType`/`simpleType`, none for `anyType`), global and local `attribute` (`name` or `ref`, `type` or anonymous, `use` optional/required, `default`, `fixed`), `attributeGroup` definitions and references, `group` definitions and references.
- **Content models:** `sequence`, `choice`, `any` (`##any`, `##other`, namespace lists with `##local` and `##targetNamespace`; `strict`, `lax`, `skip`), `anyAttribute`, `complexType` with `mixed` and `simpleContent` by `extension`.
- **Occurrence bounds:** `minOccurs` 0/1 and `maxOccurs` 1/`unbounded` only. These are the only values the schemas use: choices repeat without bound, elements and sequences repeat without bound, and nothing has a numeric maximum above 1.
- **Simple types:**
  - `restriction` with `enumeration`, `pattern`, `minInclusive`, `maxInclusive`;
  - `list` (of atomic or union types);
  - `union` (member types and anonymous members).
- **Built-in types:** `string`, `NMTOKEN`, `NMTOKENS`, `NCName`, `ID`, `anyURI`, `hexBinary`, `positiveInteger`, `dateTime`, `language`, `anySimpleType`, `anyType`.
- **Identity constraints:** `unique`, `key`, `keyref`. Selectors use the restricted XPath `.`, `x/y`, `./x/y`, `.//x` and `|`; fields are `@name`.
- **Patterns:** XSD regular expressions with classes, negation, `\s` `\d` and their complements, groups, alternation and quantifiers.

Anything else is **refused when the schema is read** (`RefusalReason::UnsupportedSchema`, with the line): `include`/`redefine`, `complexContent`, `all`, other built-in types, other facets, `\i` `\c` `\w` `\p{}` and class subtraction in patterns, other occurrence bounds, a repeatable choice with a sequence or group branch, `nillable`, `substitutionGroup`, `abstract`, `block`/`final`, `form`, `attributeFormDefault="qualified"`, prohibited attributes. The validator refuses `xsi:type` and entity references in the document (`UnsupportedInstance`).

**Imports.** libxml reads the import's `schemaLocation` from the file system or the network. Here it is handed to the caller's resolver; if the resolver doesn't supply the document, the schema is refused (`UnresolvedImport`). X3 supplies the bundled `xml.xsd` for the URL XliffUtils writes. XmlValidator's own `schemaValidate($path)` reads a file, so X3 has to decide how that maps.

## 2. Options (searched again November 22)

Pure-Rust XSD: [uppsala](https://docs.rs/crate/uppsala/0.4.0) (XSD 1.1, its own messages), [xsd-schema](https://docs.rs/crate/xsd-schema/0.1.0), [xmlschema](https://docs.rs/crate/xmlschema/latest) (early), [fastxml](https://docs.rs/crate/fastxml/0.2.0/source/README.md) (its validation is new and tested against libxml, but it does not reproduce libxml's messages or expected-element lists), [xsd](https://docs.rs/crate/xsd/0.5.0) (datatypes only). None gives libxml 2.9.14's error codes, message texts, attribute-error order or "Expected is ( … )" lists. XliffFileLoader puts those into its exception messages, so the hand-written validator (decision 9) stands. The stack row in PLAN.md is updated.

## 3. The recording

`oracle/recorders/php_dom_schema.php` (`scripts/record.sh php-dom-schema` →
`crates/php-dom/tests/data/schema.json`). Each document is loaded as
XmlUtils loads it (`validateOnParse`, `LIBXML_NONET | LIBXML_COMPACT`,
`normalizeDocument()`). It is then validated with `schemaValidateSource()`, using
XliffUtils's own rewritten schema sources (reached by reflection) for
XLIFF. Recorded: the result and every error (level, code, message, file, line,
column). The imported `xml.xsd` is stored with its URL, so the replay needs no
files.

1,325 documents against 65 schemas, with 922 recorded errors:

- **Fixtures:**
  - 70 XLIFF 1.2 files (upstream's fixtures and the demo's 58 translations);
  - 14 XLIFF 2.x fixtures;
  - 4 constraint-mapping fixtures;
  - 3 Config fixtures.
- **Mutations of fixtures (419):** one change at a time to `resources.xlf`, `resources-tool-info.xlf`, `resources-2.0.xlf`, `resources-notes-meta.xlf`, `resources-2.2.xlf` and `constraint-mapping.xml`. Each mutation adds an unknown attribute or child, adds text, removes children, swaps or duplicates children, or removes, replaces or empties each attribute.
- **Written XLIFF cases (33):** identity constraints, `xml:*` attributes, dates, enumerations, inline elements, foreign namespaces.
- **Feature schemas, one per feature (258 cases):** sequences, choices, groups, every built-in type and facet on valid and invalid values, attributes, content types, wildcards, identity constraints.
- **Content-model probes (480):** 40 models, each over the same 12 child sequences.
- **26 cases on libxml's behaviour after errors.**
- **Malformed schemas and schemas using refused features (10).**
- **XmlValidatorTest's `example.xsd` and `invalid.xsd` (8).**

What the recording found (all pinned by the replay):

- **Content models.** After the first child the model rejects, libxml looks at no further children of that element. The rejected child itself is not assessed. Empty and simple content stop at their first element child, and a simple value is then the text before it. Each non-whitespace text node in element-only content is one error.
- **Expected lists.** These follow libxml's automaton, not the set of names that could actually come next:
  - an unbounded element or sequence lists its own next occurrence before what follows;
  - inside a repeatable choice, nothing repeatable is listed, and once the round may end only what follows the choice is;
  - a repeatable choice with a branch that may be empty (XLIFF 1.2's `body`, constraint-mapping's `class`) clears the list, so "This element is not expected." has no list at all there.
  The 480 probes pin this for every supported shape. Bounded counts (`maxOccurs="3"`) and sequences or groups as branches of a repeatable choice behave differently again, so they are refused.
- **Attributes,** in three passes:
  1. values are validated as they are read;
  2. then, in document order, attributes not allowed (1866 without a wildcard, 1867 with one) and fixed-value mismatches;
  3. then missing required attributes.
  A `fixed` value written on an attribute **reference** is not checked by libxml 2.9.14: XLIFF 2.0's `<data xml:space="default">` is valid. Only a local declaration's `fixed` value is checked.
- **Values:**
  - `dateTime` is not whitespace-collapsed (a leading space is invalid);
  - `NMTOKENS` accepts the empty string;
  - `anyURI` accepts spaces but not `[`, a bad `%` escape or an empty scheme;
  - `24:00:00` is a valid time;
  - a list reports its first bad item, then itself;
  - union members are tried silently;
  - anonymous types are "the local atomic/list/union type".
- **Identity constraints:**
  - they are checked when the selected element ends, innermost scope first and in reverse declaration order within a scope;
  - keyrefs are checked when their scope ends;
  - a field whose attribute value is invalid gives libxml's "No precomputed value available" warning (one per constraint), and the key then counts as missing;
  - an `ID` that repeats is reported as an invalid `xs:ID` value.
- **Schema errors** (1761 duplicate global element, 1772 not a schema, 3004 unresolved type or ref, 3033 unknown top-level element, 3067 with the parser's errors for a malformed schema) carry the file name `in_memory_buffer`. Document errors carry the document's URL: PHP's working directory plus `/` for a string.

## 4. The crate

`php_dom::schema`:
- `Schema::parse(bytes, resolver)` → a `Schema`, `SchemaError::Invalid(errors)` or `SchemaError::Refused(refusal)`;
- `Schema::validate(&document, document_file)` → `SchemaValidation { valid, errors }` or a refusal;
- `schema_validate_source()` does both, as PHP does.

There is no dependency and no `unsafe`. Patterns run on a Thompson automaton, so matching is linear in the value whatever the pattern (pinned by a 10,000-character test). The only way an import is read is through the resolver.

Result: **1,321 of 1,325 recorded documents give PHP's answer** (result and every error). The other 4 are refused on purpose, and `tests/schema.rs`'s `KNOWN` checks that they are refused:
- `xsi:type`;
- `complexContent`;
- `all`;
- `minOccurs="2" maxOccurs="3"`.

A deliberate change to the hiding-choice rule or to ID uniqueness makes 4 and 2 more cases fail, which shows the replay really compares.

## 5. Gate status

X2 **passes for the recorded corpus**: every schema Symfony validates against at this pin loads, and the documents above validate as in PHP. Constructs outside the feature list are refused at load time, never skipped.

Open for X3:
- the resolver mapping for XliffUtils (`file://…/xml.xsd`) and for XmlValidator's file paths;
- `XmlUtils` itself;
- the SimpleXML/XPath subset;
- reporting a schema refusal as decision 14 (accepted) does for parse refusals: an `XmlParsingException` carrying the refusal's text.
