//! XliffFileLoaderTest.php at symfony/translation v8.1.5. Names and
//! assertions are PHPUnit's; the fixtures are upstream's Tests/Fixtures,
//! copied to tests/fixtures.
//!
//! `assertSame([], libxml_get_errors())` and `libxml_get_last_error()`
//! hold by construction: php-dom keeps no global error buffer (each call
//! returns its errors). `getResources()` is compared as the resources'
//! class and `__toString()`, which is what `assertEquals()` compares for a
//! FileResource.
//!
//! Upstream: symfony/translation Tests/Loader/XliffFileLoaderTest.php
#![allow(non_snake_case)]

use php_dom::libxml_use_internal_errors;
use php_value::{Key, PhpArray, Value};
use symfony_config::resource::{FileResource, ResourceInterface};
use symfony_translation::{
    Class, INTL_DOMAIN_SUFFIX, LoaderInterface, MessageCatalogue, XliffFileLoader,
};

fn fixture(name: &str) -> Vec<u8> {
    format!("{}/tests/fixtures/{name}", env!("CARGO_MANIFEST_DIR")).into_bytes()
}

fn load(resource: &[u8], locale: &str, domain: &str) -> MessageCatalogue {
    match XliffFileLoader.load(resource, locale.as_bytes(), domain.as_bytes()) {
        Ok(catalogue) => catalogue,
        Err(e) => panic!("{e}"),
    }
}

fn load_err(resource: &[u8], domain: &str) -> symfony_translation::Error {
    match XliffFileLoader.load(resource, b"en", domain.as_bytes()) {
        Ok(c) => panic!("expected an exception, got {c:?}"),
        Err(e) => e,
    }
}

fn s(v: &str) -> Value {
    Value::string(v)
}

fn array(pairs: &[(&str, Value)]) -> Value {
    let mut a = PhpArray::new();
    for (k, v) in pairs {
        a.insert(Key::from_bytes(k.as_bytes()), v.clone());
    }
    Value::Array(a)
}

fn list(values: &[Value]) -> Value {
    Value::Array(PhpArray::list(values.iter().cloned()))
}

fn assert_contains_only_string(messages: &PhpArray) {
    assert!(messages.values().all(|v| matches!(v, Value::String(_))));
}

fn keys(messages: &PhpArray) -> Vec<Key> {
    messages.keys().cloned().collect()
}

fn get(metadata: &Value, key: &str) -> Value {
    metadata
        .as_array()
        .and_then(|a| a.get(&Key::from_bytes(key.as_bytes())))
        .cloned()
        .unwrap_or(Value::Null)
}

fn at(metadata: &Value, index: i64) -> Value {
    metadata
        .as_array()
        .and_then(|a| a.get(&Key::Int(index)))
        .cloned()
        .unwrap_or(Value::Null)
}

fn count(value: &Value) -> usize {
    value.as_array().map_or(0, PhpArray::len)
}

fn intl(domain: &str) -> String {
    format!("{domain}{INTL_DOMAIN_SUFFIX}")
}

#[test]
fn testLoadRawXliff() {
    let resource = r#"<?xml version="1.0" encoding="utf-8"?>
<xliff xmlns="urn:oasis:names:tc:xliff:document:1.2" version="1.2">
  <file source-language="en" datatype="plaintext" original="file.ext">
    <body>
      <trans-unit id="1">
        <source>foo</source>
        <target>bar</target>
      </trans-unit>
      <trans-unit id="2">
        <source>extra</source>
      </trans-unit>
      <trans-unit id="3">
        <source>key</source>
        <target></target>
      </trans-unit>
      <trans-unit id="4">
        <source>test</source>
        <target state="needs-translation">with</target>
        <note>note</note>
      </trans-unit>
      <trans-unit id="5">
        <source>baz</source>
        <target state="needs-translation">baz</target>
      </trans-unit>
      <trans-unit id="6" resname="buz">
        <source>baz</source>
        <target state="needs-translation">buz</target>
      </trans-unit>
    </body>
  </file>
</xliff>"#;

    let catalogue = load(resource.as_bytes(), "en", "domain1");

    assert_eq!(catalogue.get_locale().as_bytes(), b"en");
    assert_contains_only_string(&catalogue.all(Some(b"domain1")));
    assert_eq!(
        keys(&catalogue.all(Some(b"domain1"))),
        ["foo", "extra", "key", "test"].map(|k| Key::from_bytes(k.as_bytes()))
    );
}

#[test]
fn testLoadWithResname() {
    let catalogue = load(&fixture("resname.xlf"), "en", "domain1");

    assert_eq!(
        Value::Array(catalogue.all(Some(b"domain1"))),
        array(&[
            ("foo", s("bar")),
            ("bar", s("baz")),
            ("baz", s("foo")),
            ("qux", s("qux source"))
        ])
    );
}

#[test]
fn testIncompleteResource() {
    let catalogue = load(&fixture("resources.xlf"), "en", "domain1");

    assert_eq!(
        Value::Array(catalogue.all(Some(b"domain1"))),
        array(&[
            ("foo", s("bar")),
            ("extra", s("extra")),
            ("key", s("")),
            ("test", s("with"))
        ])
    );
}

#[test]
fn testEncoding() {
    let catalogue = load(&fixture("encoding.xlf"), "en", "domain1");

    // mb_convert_encoding('föö', 'ISO-8859-1', 'UTF-8')
    assert_eq!(catalogue.get(b"bar", b"domain1").as_bytes(), b"f\xF6\xF6");
    assert_eq!(catalogue.get(b"foo", b"domain1").as_bytes(), b"b\xE4r");
    assert_eq!(
        catalogue.get_metadata(b"foo", b"domain1"),
        array(&[
            ("source", s("foo")),
            ("file", array(&[("original", s("file.ext"))])),
            (
                "notes",
                list(&[array(&[("content", Value::string(&b"b\xE4z"[..]))])])
            ),
            ("id", s("1")),
        ])
    );
}

#[test]
fn testTargetAttributesAreStoredCorrectly() {
    let catalogue = load(&fixture("with-attributes.xlf"), "en", "domain1");

    let metadata = catalogue.get_metadata(b"foo", b"domain1");
    assert_eq!(
        get(&get(&metadata, "target-attributes"), "state"),
        s("translated")
    );
}

#[test]
fn testLoadInvalidResource() {
    assert_eq!(
        *load_err(&fixture("resources.php"), "domain1").class(),
        Class::InvalidResource
    );
}

#[test]
fn testLoadResourceDoesNotValidate() {
    assert_eq!(
        *load_err(&fixture("non-valid.xlf"), "domain1").class(),
        Class::InvalidResource
    );
}

#[test]
fn testLoadNonExistingResource() {
    assert_eq!(
        *load_err(&fixture("non-existing.xlf"), "domain1").class(),
        Class::NotFoundResource
    );
}

#[test]
fn testLoadThrowsAnExceptionIfFileNotLocal() {
    assert_eq!(
        *load_err(b"http://example.com/resources.xlf", "domain1").class(),
        Class::InvalidResource
    );
}

#[test]
fn testDocTypeIsNotAllowed() {
    let e = load_err(&fixture("withdoctype.xlf"), "domain1");
    assert_eq!(*e.class(), Class::InvalidResource);
    assert!(String::from_utf8_lossy(e.get_message()).contains("Document types are not allowed."));
}

#[test]
fn testParseEmptyFile() {
    let resource = fixture("empty.xlf");
    let e = load_err(&resource, "domain1");
    assert_eq!(*e.class(), Class::InvalidResource);
    let expected = format!("Unable to load \"{}\":", String::from_utf8_lossy(&resource));
    assert!(String::from_utf8_lossy(e.get_message()).contains(&expected));
}

#[test]
fn testLoadNotes() {
    let catalogue = load(&fixture("withnote.xlf"), "en", "domain1");

    assert_eq!(
        catalogue.get_metadata(b"foo", b"domain1"),
        array(&[
            ("source", s("foo")),
            ("file", array(&[("original", s("file.ext"))])),
            (
                "notes",
                list(&[array(&[("content", s("foo")), ("priority", Value::Int(1))])])
            ),
            ("id", s("1")),
        ])
    );
    // message without target
    assert_eq!(
        catalogue.get_metadata(b"extra", b"domain1"),
        array(&[
            ("source", s("extrasource")),
            ("file", array(&[("original", s("file.ext"))])),
            (
                "notes",
                list(&[array(&[("content", s("bar")), ("from", s("foo"))])])
            ),
            ("id", s("2")),
        ])
    );
    // message with empty target
    assert_eq!(
        catalogue.get_metadata(b"key", b"domain1"),
        array(&[
            ("source", s("key")),
            ("file", array(&[("original", s("file.ext"))])),
            (
                "notes",
                list(&[
                    array(&[("content", s("baz"))]),
                    array(&[
                        ("content", s("qux")),
                        ("priority", Value::Int(2)),
                        ("from", s("bar"))
                    ]),
                ])
            ),
            ("id", s("123")),
        ])
    );
}

#[test]
fn testLoadVersion2WithCatalogMeta() {
    let catalogue = load(&fixture("resources-catalog-meta.xlf"), "en", "messages");

    assert_eq!(
        catalogue.get_catalogue_metadata(b"", b"messages"),
        array(&[("key", s("value"))])
    );
}

#[test]
fn testLoadWithMultipleFileNodes() {
    let catalogue = load(&fixture("resources-multi-files.xlf"), "en", "domain1");

    assert_eq!(
        catalogue.get_metadata(b"foo", b"domain1"),
        array(&[
            ("source", s("foo")),
            ("file", array(&[("original", s("file.ext"))])),
            ("id", s("1")),
        ])
    );
    assert_eq!(
        catalogue.get_metadata(b"test", b"domain1"),
        array(&[
            ("source", s("test")),
            ("file", array(&[("original", s("otherfile.ext"))])),
            ("notes", list(&[array(&[("content", s("note"))])])),
            ("id", s("4")),
        ])
    );
}

#[test]
fn testLoadVersion2WithName() {
    let catalogue = load(&fixture("resources-2.0-name.xlf"), "en", "domain1");

    assert_eq!(
        Value::Array(catalogue.all(Some(b"domain1"))),
        array(&[
            ("foo", s("bar")),
            ("bar", s("baz")),
            ("baz", s("foo")),
            ("qux", s("qux source"))
        ])
    );
}

#[test]
fn testLoadVersion2WithSegmentAttributes() {
    let catalogue = load(
        &fixture("resources-2.0-segment-attributes.xlf"),
        "en",
        "domain1",
    );

    // test for "foo" metadata
    assert!(catalogue.defines(b"foo", b"domain1"));
    let metadata = catalogue.get_metadata(b"foo", b"domain1");
    assert!(count(&metadata) > 0);
    let attributes = get(&metadata, "segment-attributes");
    assert_eq!(count(&attributes), 1);
    assert_eq!(get(&attributes, "state"), s("translated"));

    // test for "key" metadata
    assert!(catalogue.defines(b"key", b"domain1"));
    let metadata = catalogue.get_metadata(b"key", b"domain1");
    assert!(count(&metadata) > 0);
    let attributes = get(&metadata, "segment-attributes");
    assert_eq!(count(&attributes), 2);
    assert_eq!(get(&attributes, "state"), s("translated"));
    assert_eq!(get(&attributes, "subState"), s("My Value"));
}

#[test]
fn testLoadVersion22WithPgsPlural() {
    let catalogue = load(&fixture("resources-2.2-pgs-plural.xlf"), "fr", "domain1");
    let intl_domain = intl("domain1");

    assert!(catalogue.defines(b"file_deleted", intl_domain.as_bytes()));
    assert_eq!(
        catalogue
            .get(b"file_deleted", intl_domain.as_bytes())
            .to_string_lossy(),
        "{file_count, plural, =0 {Vous n'avez supprimé aucun fichier.} =1 {Vous avez supprimé un fichier.} other {Vous avez supprimé # fichiers.}}"
    );
    assert_eq!(
        get(
            &catalogue.get_metadata(b"file_deleted", intl_domain.as_bytes()),
            "pgs-switch"
        ),
        s("plural:file_count")
    );
}

#[test]
fn testLoadVersion22WithPgsGender() {
    let catalogue = load(&fixture("resources-2.2-pgs-gender.xlf"), "fr", "domain1");
    let intl_domain = intl("domain1");

    assert!(catalogue.defines(b"party_invite", intl_domain.as_bytes()));
    assert_eq!(
        catalogue
            .get(b"party_invite", intl_domain.as_bytes())
            .to_string_lossy(),
        "{host_gender, select, feminine {Vous êtes invité à sa fête} masculine {Vous êtes invité à sa fête} other {Vous êtes invité à leur fête}}"
    );
}

#[test]
fn testLoadVersion22WithPgsCombined() {
    let catalogue = load(&fixture("resources-2.2-pgs-combined.xlf"), "fr", "domain1");
    let intl_domain = intl("domain1");

    assert!(catalogue.defines(b"party_host", intl_domain.as_bytes()));

    let expected = "{host_gender, select, feminine {{guest_count, plural, =0 {{host_name} n'a invité personne à sa fête.} =1 {{host_name} a invité un convive à sa fête.} other {{host_name} a invité # convives à sa fête.}}} masculine {{guest_count, plural, =0 {{host_name} n'a invité personne à sa fête.} =1 {{host_name} a invité un convive à sa fête.} other {{host_name} a invité # convives à sa fête.}}} other {{guest_count, plural, =0 {{host_name} n'a invité personne à leur fête.} =1 {{host_name} a invité un convive à leur fête.} other {{host_name} a invité # convives à leur fête.}}}}";

    assert_eq!(
        catalogue
            .get(b"party_host", intl_domain.as_bytes())
            .to_string_lossy(),
        expected
    );
}

/// provideMalformedPgsSwitchValues()
const MALFORMED_PGS_SWITCH_VALUES: &[(&str, &str)] = &[
    ("empty", ""),
    ("whitespace", "   "),
    ("missing variable", "plural"),
    ("trailing colon", "plural:"),
    ("leading colon", ":file_count"),
];

#[test]
fn testLoadVersion22RejectsMalformedPgsSwitch() {
    for (name, pgs_switch) in MALFORMED_PGS_SWITCH_VALUES {
        let xml = format!(
            r#"<?xml version="1.0" encoding="UTF-8"?>
<xliff xmlns="urn:oasis:names:tc:xliff:document:2.0" xmlns:pgs="urn:oasis:names:tc:xliff:pgs:1.0"
       version="2.2" srcLang="en" trgLang="fr">
    <file id="f1">
        <unit id="tu1" name="file_deleted" pgs:switch="{pgs_switch}">
            <segment id="seg1" pgs:case="other">
                <source>You deleted files.</source>
                <target>Vous avez supprimé des fichiers.</target>
            </segment>
        </unit>
    </file>
</xliff>"#
        );

        let e = match XliffFileLoader.load(xml.as_bytes(), b"fr", b"domain1") {
            Ok(_) => panic!("{name}: expected an exception"),
            Err(e) => e,
        };
        assert_eq!(*e.class(), Class::InvalidResource, "{name}");
    }
}

#[test]
fn testExtractPgsSegmentPreservesInlineElements() {
    let xml = r#"<?xml version="1.0" encoding="UTF-8"?>
<xliff xmlns="urn:oasis:names:tc:xliff:document:2.0" xmlns:pgs="urn:oasis:names:tc:xliff:pgs:1.0"
       version="2.2" srcLang="en" trgLang="fr">
    <file id="f1">
        <unit id="tu1" name="greeting" pgs:switch="select:audience">
            <segment id="seg1" pgs:case="formal">
                <source>Hello there</source>
                <target>Bonjour <pc id="1">Madame</pc> <mrk id="m1" type="term">Dupont</mrk><cp hex="00A0"/>!</target>
            </segment>
        </unit>
    </file>
</xliff>"#;

    let catalogue = load(xml.as_bytes(), "fr", "domain1");
    let intl_domain = intl("domain1");

    assert_eq!(
        catalogue
            .get(b"greeting", intl_domain.as_bytes())
            .to_string_lossy(),
        "{audience, select, formal {Bonjour Madame Dupont\u{00A0}!}}"
    );
}

#[test]
fn testExtractPgsSegmentSkipsInvalidCpCodepoints() {
    let xml = r#"<?xml version="1.0" encoding="UTF-8"?>
<xliff xmlns="urn:oasis:names:tc:xliff:document:2.0" xmlns:pgs="urn:oasis:names:tc:xliff:pgs:1.0"
       version="2.2" srcLang="en" trgLang="fr">
    <file id="f1">
        <unit id="tu1" name="greeting" pgs:switch="select:audience">
            <segment id="seg1" pgs:case="formal">
                <source>Hello there</source>
                <target>Bonjour<cp hex="D800"/><cp hex="DFFF"/>!</target>
            </segment>
        </unit>
    </file>
</xliff>"#;

    // No PHP warning to collect: the port raises none.
    let catalogue = load(xml.as_bytes(), "fr", "domain1");
    let intl_domain = intl("domain1");
    assert_eq!(
        catalogue
            .get(b"greeting", intl_domain.as_bytes())
            .to_string_lossy(),
        "{audience, select, formal {Bonjour!}}"
    );
}

/// `assertEquals([new FileResource($resource)], $catalogue->getResources())`.
fn assert_file_resource(catalogue: &MessageCatalogue, resource: &[u8]) {
    let want = FileResource::new(resource).expect("FileResource");
    let got: Vec<Vec<u8>> = catalogue
        .get_resources()
        .iter()
        .map(|r| {
            assert_eq!(r.class(), symfony_config::resource::file_resource::CLASS);
            r.to_string()
        })
        .collect();
    assert_eq!(got, [want.to_string()]);
}

#[test]
fn testLoadFile() {
    let resource = fixture("resources.xlf");
    let catalogue = load(&resource, "en", "domain1");

    assert_eq!(catalogue.get_locale().as_bytes(), b"en");
    assert_file_resource(&catalogue, &resource);
    assert_contains_only_string(&catalogue.all(Some(b"domain1")));
}

#[test]
fn testLoadWithInternalErrorsEnabled() {
    let internal_errors = libxml_use_internal_errors(Some(true));

    let resource = fixture("resources.xlf");
    let catalogue = load(&resource, "en", "domain1");

    assert_eq!(catalogue.get_locale().as_bytes(), b"en");
    assert_file_resource(&catalogue, &resource);

    libxml_use_internal_errors(Some(internal_errors));
}

#[test]
fn testLoadWithExternalEntitiesDisabled() {
    let resource = fixture("resources.xlf");
    let catalogue = load(&resource, "en", "domain1");

    assert_eq!(catalogue.get_locale().as_bytes(), b"en");
    assert_file_resource(&catalogue, &resource);
}

fn assert_version2(name: &str) {
    let resource = fixture(name);
    let catalogue = load(&resource, "en", "domain1");

    assert_eq!(catalogue.get_locale().as_bytes(), b"en");
    assert_file_resource(&catalogue, &resource);

    let domains = catalogue.all(None);
    assert_eq!(
        count(
            domains
                .get(&Key::from_bytes(b"domain1"))
                .unwrap_or(&Value::Null)
        ),
        3
    );
    assert_contains_only_string(&catalogue.all(Some(b"domain1")));

    // target attributes (assertEquals: 1 == '1')
    assert_eq!(
        catalogue.get_metadata(b"bar", b"domain1"),
        array(&[("target-attributes", array(&[("order", s("1"))]))])
    );
}

#[test]
fn testLoadVersion2() {
    assert_version2("resources-2.0.xlf");
}

#[test]
fn testLoadVersion21() {
    assert_version2("resources-2.1.xlf");
}

#[test]
fn testLoadVersion22() {
    assert_version2("resources-2.2.xlf");
}

#[test]
fn testLoadVersion2WithNoteMeta() {
    let resource = fixture("resources-notes-meta.xlf");
    let catalogue = load(&resource, "en", "domain1");
    assert_eq!(catalogue.get_locale().as_bytes(), b"en");
    assert_file_resource(&catalogue, &resource);

    assert!(catalogue.defines(b"foo", b"domain1"));
    let notes = get(&catalogue.get_metadata(b"foo", b"domain1"), "notes");
    assert_eq!(count(&notes), 3);
    assert_eq!(get(&at(&notes, 0), "category"), s("state"));
    assert_eq!(get(&at(&notes, 0), "content"), s("new"));
    assert_eq!(get(&at(&notes, 1), "category"), s("approved"));
    assert_eq!(get(&at(&notes, 1), "content"), s("true"));
    assert_eq!(get(&at(&notes, 2), "category"), s("section"));
    assert_eq!(get(&at(&notes, 2), "priority"), s("1"));
    assert_eq!(get(&at(&notes, 2), "content"), s("user login"));

    assert!(catalogue.defines(b"baz", b"domain1"));
    let notes = get(&catalogue.get_metadata(b"baz", b"domain1"), "notes");
    assert_eq!(count(&notes), 2);
    assert_eq!(get(&at(&notes, 0), "id"), s("x"));
    assert_eq!(get(&at(&notes, 0), "content"), s("x_content"));
    assert_eq!(get(&at(&notes, 1), "appliesTo"), s("target"));
    assert_eq!(get(&at(&notes, 1), "category"), s("quality"));
    assert_eq!(get(&at(&notes, 1), "content"), s("Fuzzy"));
}

#[test]
fn testLoadVersion2WithMultiSegmentUnit() {
    let resource = fixture("resources-2.0-multi-segment-unit.xlf");
    let catalog = load(&resource, "en", "domain1");

    assert_eq!(catalog.get_locale().as_bytes(), b"en");
    assert_file_resource(&catalog, &resource);

    for id in ["foo", "bar"] {
        assert!(catalog.defines(id.as_bytes(), b"domain1"));
        let notes = get(&catalog.get_metadata(id.as_bytes(), b"domain1"), "notes");
        assert_eq!(count(&notes), 1);
        assert_eq!(get(&at(&notes, 0), "category"), s("processed"));
        assert_eq!(get(&at(&notes, 0), "content"), s("true"));
    }
}
