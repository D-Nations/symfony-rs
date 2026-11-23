//! MessageCatalogueTest.php at symfony/translation v8.1.5. Names and
//! assertions are PHPUnit's. The `ResourceInterface` stubs are a struct
//! whose `__toString()` returns the stubbed text.
//!
//! Upstream: symfony/translation Tests/MessageCatalogueTest.php
#![allow(non_snake_case)]

use std::rc::Rc;

use php_value::{Key, PhpArray, Value};
use symfony_config::Error as ConfigError;
use symfony_config::resource::ResourceInterface;
use symfony_translation::{Class, MessageCatalogue};

#[derive(Debug)]
struct Stub(&'static str);

impl ResourceInterface for Stub {
    fn class(&self) -> &str {
        "Stub"
    }

    fn to_string(&self) -> Vec<u8> {
        self.0.as_bytes().to_vec()
    }

    fn serialize(&self) -> Result<PhpArray, ConfigError> {
        Ok(PhpArray::new())
    }
}

fn stub(text: &'static str) -> Rc<dyn ResourceInterface> {
    Rc::new(Stub(text))
}

fn texts(resources: &[Rc<dyn ResourceInterface>]) -> Vec<String> {
    resources
        .iter()
        .map(|r| String::from_utf8_lossy(&r.to_string()).into_owned())
        .collect()
}

/// `['k' => 'v', …]`
fn messages(pairs: &[(&str, &str)]) -> PhpArray {
    let mut a = PhpArray::new();
    for (k, v) in pairs {
        a.insert(Key::from_bytes(k.as_bytes()), Value::string(*v));
    }
    a
}

/// `['domain' => [...], …]`
fn domains(pairs: &[(&str, PhpArray)]) -> PhpArray {
    let mut a = PhpArray::new();
    for (k, v) in pairs {
        a.insert(Key::from_bytes(k.as_bytes()), Value::Array(v.clone()));
    }
    a
}

fn catalogue(locale: &str, pairs: &[(&str, PhpArray)]) -> MessageCatalogue {
    MessageCatalogue::new(locale, domains(pairs))
}

fn get(c: &MessageCatalogue, id: &str, domain: &str) -> String {
    c.get(id.as_bytes(), domain.as_bytes())
        .to_string_lossy()
        .into_owned()
}

#[test]
fn testGetLocale() {
    let catalogue = MessageCatalogue::new("en", PhpArray::new());

    assert_eq!(catalogue.get_locale().as_bytes(), b"en");
}

#[test]
fn testGetDomains() {
    let catalogue = catalogue(
        "en",
        &[
            ("domain1", PhpArray::new()),
            ("domain2", PhpArray::new()),
            ("domain2+intl-icu", PhpArray::new()),
            ("domain3+intl-icu", PhpArray::new()),
        ],
    );

    assert_eq!(
        catalogue.get_domains(),
        PhpArray::list(["domain1", "domain2", "domain3"].map(Value::string))
    );
}

#[test]
fn testAll() {
    let all = domains(&[
        ("domain1", messages(&[("foo", "foo")])),
        ("domain2", messages(&[("bar", "bar")])),
    ]);
    let catalogue = MessageCatalogue::new("en", all.clone());

    assert_eq!(catalogue.all(Some(b"domain1")), messages(&[("foo", "foo")]));
    assert_eq!(catalogue.all(Some(b"domain88")), PhpArray::new());
    assert_eq!(catalogue.all(None), all);

    // ['domain1+intl-icu' => …] + $messages + ['domain2+intl-icu' => …, 'domain3+intl-icu' => …]
    let mut with_intl = domains(&[("domain1+intl-icu", messages(&[("foo", "bar")]))]);
    with_intl.union(&all);
    with_intl.union(&domains(&[
        ("domain2+intl-icu", messages(&[("bar", "foo")])),
        ("domain3+intl-icu", messages(&[("biz", "biz")])),
    ]));
    let catalogue = MessageCatalogue::new("en", with_intl);

    assert_eq!(catalogue.all(Some(b"domain1")), messages(&[("foo", "bar")]));
    assert_eq!(catalogue.all(Some(b"domain2")), messages(&[("bar", "foo")]));
    assert_eq!(catalogue.all(Some(b"domain3")), messages(&[("biz", "biz")]));

    let expected = domains(&[
        ("domain1", messages(&[("foo", "bar")])),
        ("domain2", messages(&[("bar", "foo")])),
        ("domain3", messages(&[("biz", "biz")])),
    ]);
    assert_eq!(catalogue.all(None), expected);
}

#[test]
fn testAllIntlIcu() {
    let catalogue = catalogue(
        "en",
        &[
            ("domain1+intl-icu", messages(&[("foo", "bar")])),
            ("domain2+intl-icu", messages(&[("bar", "foo")])),
            ("domain2", messages(&[("biz", "biz")])),
        ],
    );

    // separated domains
    assert_eq!(
        catalogue.all(Some(b"domain1+intl-icu")),
        messages(&[("foo", "bar")])
    );
    assert_eq!(
        catalogue.all(Some(b"domain2+intl-icu")),
        messages(&[("bar", "foo")])
    );

    // merged, intl-icu ignored (assertSame: order too)
    let merged = catalogue.all(Some(b"domain2"));
    assert_eq!(merged, messages(&[("bar", "foo"), ("biz", "biz")]));
    assert!(
        merged
            .keys()
            .eq([Key::from_bytes(b"bar"), Key::from_bytes(b"biz")].iter())
    );

    // intl-icu ignored
    let expected = domains(&[
        ("domain1", messages(&[("foo", "bar")])),
        ("domain2", messages(&[("bar", "foo"), ("biz", "biz")])),
    ]);
    let all = catalogue.all(None);
    assert_eq!(all, expected);
    assert!(all.keys().eq(expected.keys()));
}

#[test]
fn testHas() {
    let catalogue = catalogue(
        "en",
        &[
            ("domain1", messages(&[("foo", "foo")])),
            ("domain2+intl-icu", messages(&[("bar", "bar")])),
        ],
    );

    assert!(catalogue.has(b"foo", b"domain1"));
    assert!(catalogue.has(b"bar", b"domain2"));
    assert!(!catalogue.has(b"bar", b"domain1"));
    assert!(!catalogue.has(b"foo", b"domain88"));
}

#[test]
fn testGetSet() {
    let catalogue = catalogue(
        "en",
        &[
            ("domain1", messages(&[("foo", "foo")])),
            ("domain2", messages(&[("bar", "bar")])),
            ("domain2+intl-icu", messages(&[("bar", "foo")])),
        ],
    );
    catalogue.set(b"foo1", b"foo1", b"domain1");

    assert_eq!(get(&catalogue, "foo", "domain1"), "foo");
    assert_eq!(get(&catalogue, "foo1", "domain1"), "foo1");
    assert_eq!(get(&catalogue, "bar", "domain2"), "foo");
}

#[test]
fn testAdd() {
    let catalogue = catalogue(
        "en",
        &[
            ("domain1", messages(&[("foo", "foo")])),
            ("domain2", messages(&[("bar", "bar")])),
        ],
    );
    catalogue.add(&messages(&[("foo1", "foo1")]), b"domain1");

    assert_eq!(get(&catalogue, "foo", "domain1"), "foo");
    assert_eq!(get(&catalogue, "foo1", "domain1"), "foo1");

    catalogue.add(&messages(&[("foo", "bar")]), b"domain1");
    assert_eq!(get(&catalogue, "foo", "domain1"), "bar");
    assert_eq!(get(&catalogue, "foo1", "domain1"), "foo1");

    catalogue.add(&messages(&[("foo", "bar")]), b"domain88");
    assert_eq!(get(&catalogue, "foo", "domain88"), "bar");
}

#[test]
fn testAddIntlIcu() {
    let catalogue = catalogue("en", &[("domain1+intl-icu", messages(&[("foo", "foo")]))]);
    catalogue.add(&messages(&[("foo1", "foo1")]), b"domain1");
    catalogue.add(&messages(&[("foo", "bar")]), b"domain1");

    assert_eq!(get(&catalogue, "foo", "domain1"), "bar");
    assert_eq!(get(&catalogue, "foo1", "domain1"), "foo1");
}

#[test]
fn testReplace() {
    let catalogue = catalogue(
        "en",
        &[
            ("domain1", messages(&[("foo", "foo")])),
            ("domain1+intl-icu", messages(&[("bar", "bar")])),
        ],
    );
    let replacement = messages(&[("foo1", "foo1")]);
    catalogue.replace(&replacement, b"domain1");

    assert_eq!(catalogue.all(Some(b"domain1")), replacement);
}

#[test]
fn testAddCatalogue() {
    let r = stub("r");
    let r1 = stub("r1");

    let catalogue = catalogue("en", &[("domain1", messages(&[("foo", "foo")]))]);
    catalogue.add_resource(Rc::clone(&r));

    let catalogue1 = self::catalogue(
        "en",
        &[
            ("domain1", messages(&[("foo1", "foo1")])),
            ("domain2+intl-icu", messages(&[("bar", "bar")])),
        ],
    );
    catalogue1.add_resource(Rc::clone(&r1));

    catalogue.add_catalogue(&catalogue1).expect("same locale");

    assert_eq!(get(&catalogue, "foo", "domain1"), "foo");
    assert_eq!(get(&catalogue, "foo1", "domain1"), "foo1");
    assert_eq!(get(&catalogue, "bar", "domain2"), "bar");
    assert_eq!(get(&catalogue, "bar", "domain2+intl-icu"), "bar");

    assert_eq!(texts(&catalogue.get_resources()), ["r", "r1"]);
}

#[test]
fn testAddFallbackCatalogue() {
    let r = stub("r");
    let r1 = stub("r1");
    let r2 = stub("r2");

    let catalogue = catalogue(
        "fr_FR",
        &[
            ("domain1", messages(&[("foo", "foo")])),
            ("domain2", messages(&[("bar", "bar")])),
        ],
    );
    catalogue.add_resource(r);

    let catalogue1 = self::catalogue(
        "fr",
        &[("domain1", messages(&[("foo", "bar"), ("foo1", "foo1")]))],
    );
    catalogue1.add_resource(r1);

    let catalogue2 = MessageCatalogue::new("en", PhpArray::new());
    catalogue2.add_resource(r2);

    catalogue
        .add_fallback_catalogue(&catalogue1)
        .expect("no cycle");
    catalogue1
        .add_fallback_catalogue(&catalogue2)
        .expect("no cycle");

    assert_eq!(get(&catalogue, "foo", "domain1"), "foo");
    assert_eq!(get(&catalogue, "foo1", "domain1"), "foo1");

    assert_eq!(texts(&catalogue.get_resources()), ["r", "r1", "r2"]);
}

#[test]
fn testAddFallbackCatalogueWithParentCircularReference() {
    let main = MessageCatalogue::new("en_US", PhpArray::new());
    let fallback = MessageCatalogue::new("fr_FR", PhpArray::new());

    fallback.add_fallback_catalogue(&main).expect("no cycle");

    let e = main.add_fallback_catalogue(&fallback).expect_err("a cycle");
    assert_eq!(*e.class(), Class::Logic);
}

#[test]
fn testAddFallbackCatalogueWithFallbackCircularReference() {
    let fr = MessageCatalogue::new("fr", PhpArray::new());
    let en = MessageCatalogue::new("en", PhpArray::new());
    let es = MessageCatalogue::new("es", PhpArray::new());

    fr.add_fallback_catalogue(&en).expect("no cycle");
    es.add_fallback_catalogue(&en).expect("no cycle");

    let e = en.add_fallback_catalogue(&fr).expect_err("a cycle");
    assert_eq!(*e.class(), Class::Logic);
}

#[test]
fn testAddCatalogueWhenLocaleIsNotTheSameAsTheCurrentOne() {
    let catalogue = MessageCatalogue::new("en", PhpArray::new());

    let e = catalogue
        .add_catalogue(&MessageCatalogue::new("fr", PhpArray::new()))
        .expect_err("another locale");
    assert_eq!(*e.class(), Class::Logic);
}

#[test]
fn testGetAddResource() {
    let catalogue = MessageCatalogue::new("en", PhpArray::new());
    let r = stub("r");
    catalogue.add_resource(Rc::clone(&r));
    catalogue.add_resource(Rc::clone(&r));
    let r1 = stub("r1");
    catalogue.add_resource(r1);

    assert_eq!(texts(&catalogue.get_resources()), ["r", "r1"]);
}

#[test]
fn testMetadataDelete() {
    let catalogue = MessageCatalogue::new("en", PhpArray::new());
    assert_eq!(
        catalogue.get_metadata(b"", b""),
        Value::Array(PhpArray::new()),
        "Metadata is empty"
    );
    catalogue.delete_metadata(b"key", b"messages");
    catalogue.delete_metadata(b"", b"messages");
    catalogue.delete_metadata(b"", b"");
}

#[test]
fn testMetadataSetGetDelete() {
    let catalogue = MessageCatalogue::new("en", PhpArray::new());
    catalogue.set_metadata(b"key", Value::string("value"), b"messages");
    assert_eq!(
        catalogue.get_metadata(b"key", b"messages"),
        Value::string("value"),
        "Metadata 'key' = 'value'"
    );

    catalogue.set_metadata(b"key2", Value::Array(PhpArray::new()), b"messages");
    assert_eq!(
        catalogue.get_metadata(b"key2", b"messages"),
        Value::Array(PhpArray::new()),
        "Metadata key2 is array"
    );

    catalogue.delete_metadata(b"key2", b"messages");
    assert_eq!(
        catalogue.get_metadata(b"key2", b"messages"),
        Value::Null,
        "Metadata key2 should is deleted."
    );

    catalogue.delete_metadata(b"key2", b"domain");
    assert_eq!(
        catalogue.get_metadata(b"key2", b"domain"),
        Value::Null,
        "Metadata key2 should is deleted."
    );
}

#[test]
fn testMetadataMerge() {
    let meta = |pairs: &[(&str, &str, &str)]| {
        let mut a = PhpArray::new();
        for (domain, key, value) in pairs {
            let k = Key::from_bytes(domain.as_bytes());
            let mut inner = a
                .get(&k)
                .and_then(Value::as_array)
                .cloned()
                .unwrap_or_default();
            inner.insert(Key::from_bytes(key.as_bytes()), Value::string(*value));
            a.insert(k, Value::Array(inner));
        }
        Value::Array(a)
    };

    let cat1 = MessageCatalogue::new("en", PhpArray::new());
    cat1.set_metadata(b"a", Value::string("b"), b"messages");
    assert_eq!(
        cat1.get_metadata(b"", b""),
        meta(&[("messages", "a", "b")]),
        "Cat1 contains messages metadata."
    );

    let cat2 = MessageCatalogue::new("en", PhpArray::new());
    cat2.set_metadata(b"b", Value::string("c"), b"domain");
    assert_eq!(
        cat2.get_metadata(b"", b""),
        meta(&[("domain", "b", "c")]),
        "Cat2 contains domain metadata."
    );

    cat1.add_catalogue(&cat2).expect("same locale");
    assert_eq!(
        cat1.get_metadata(b"", b""),
        meta(&[("messages", "a", "b"), ("domain", "b", "c")]),
        "Cat1 contains merged metadata."
    );
}
