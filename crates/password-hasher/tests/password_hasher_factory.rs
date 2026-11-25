//! PasswordHasherFactoryTest.php at symfony/password-hasher
//! 6934d16beaa4677f2c4584229fff1b51099dd7af. Names and assertions are
//! PHPUnit's. The test's user classes (`SomeUser`, `SomeChildUser`,
//! `HasherAwareUser`), PHPUnit's stub of `PasswordAuthenticatedUserInterface`
//! and Security Core's `InMemoryUser` are objects that tell their class
//! and its parents.
//!
//! Upstream: symfony/password-hasher Tests/Hasher/PasswordHasherFactoryTest.php
#![allow(non_snake_case, clippy::unwrap_used)]

use std::sync::Arc;

use php_value::{PhpArray, Value};
use symfony_password_hasher::{
    Class, HasherConfig, HasherSubject, MessageDigestPasswordHasher, MigratingPasswordHasher,
    NativePasswordHasher, PASSWORD_BCRYPT, PasswordAuthenticatedUserInterface,
    PasswordHasherAwareInterface, PasswordHasherFactory, PasswordHasherFactoryInterface,
    PasswordHasherInterface, PhpClass, SODIUM_CRYPTO_PWHASH_STRPREFIX, SodiumPasswordHasher,
    UserObject,
};

const PASSWORD_AUTHENTICATED_USER_INTERFACE: &str =
    "Symfony\\Component\\Security\\Core\\User\\PasswordAuthenticatedUserInterface";
const SOME_USER: &str = "Symfony\\Component\\PasswordHasher\\Tests\\Hasher\\SomeUser";
const SOME_CHILD_USER: &str = "Symfony\\Component\\PasswordHasher\\Tests\\Hasher\\SomeChildUser";
const HASHER_AWARE_USER: &str =
    "Symfony\\Component\\PasswordHasher\\Tests\\Hasher\\HasherAwareUser";
const MESSAGE_DIGEST: &str =
    "Symfony\\Component\\PasswordHasher\\Hasher\\MessageDigestPasswordHasher";
const NATIVE: &str = "Symfony\\Component\\PasswordHasher\\Hasher\\NativePasswordHasher";
const SODIUM: &str = "Symfony\\Component\\PasswordHasher\\Hasher\\SodiumPasswordHasher";

/// An object of `class` with `parents`; `hasher_name` makes it a
/// `PasswordHasherAwareInterface` (whose name may be `null`).
#[allow(clippy::option_option)]
struct User {
    class: &'static str,
    parents: &'static [&'static str],
    hasher_name: Option<Option<String>>,
}

impl UserObject for User {
    fn php_class(&self) -> PhpClass<'_> {
        PhpClass::new(self.class, self.parents)
    }

    fn as_password_hasher_aware(&self) -> Option<&dyn PasswordHasherAwareInterface> {
        self.hasher_name
            .as_ref()
            .map(|_| self as &dyn PasswordHasherAwareInterface)
    }
}

impl PasswordHasherAwareInterface for User {
    fn get_password_hasher_name(&self) -> Option<String> {
        self.hasher_name.clone().flatten()
    }
}

impl PasswordAuthenticatedUserInterface for User {
    fn get_password(&self) -> Option<String> {
        None
    }
}

/// `$this->createStub(PasswordAuthenticatedUserInterface::class)`.
fn stub() -> User {
    User {
        class: "MockObject_PasswordAuthenticatedUserInterface_1a2b3c4d",
        parents: &[PASSWORD_AUTHENTICATED_USER_INTERFACE],
        hasher_name: None,
    }
}

/// `new HasherAwareUser()`.
fn hasher_aware_user() -> User {
    User {
        class: HASHER_AWARE_USER,
        parents: &[
            SOME_USER,
            PASSWORD_AUTHENTICATED_USER_INTERFACE,
            "Symfony\\Component\\PasswordHasher\\Hasher\\PasswordHasherAwareInterface",
        ],
        hasher_name: Some(Some("hasher_name".to_owned())),
    }
}

/// `SomeChildUser::class`.
fn some_child_user_class() -> HasherSubject<'static> {
    HasherSubject::Class(PhpClass::new(
        SOME_CHILD_USER,
        &[SOME_USER, PASSWORD_AUTHENTICATED_USER_INTERFACE],
    ))
}

/// `SomeUser::class`.
fn some_user_class() -> HasherSubject<'static> {
    HasherSubject::Class(PhpClass::new(
        SOME_USER,
        &[PASSWORD_AUTHENTICATED_USER_INTERFACE],
    ))
}

fn array(pairs: Vec<(&str, Value)>) -> PhpArray {
    let mut array = PhpArray::new();
    for (key, value) in pairs {
        array.insert(key, value);
    }
    array
}

fn list(values: Vec<Value>) -> Value {
    Value::Array(PhpArray::list(values))
}

fn digest(algorithm: &str) -> Arc<dyn PasswordHasherInterface> {
    Arc::new(MessageDigestPasswordHasher::new(algorithm, true, 5000))
}

fn hasher(config: HasherConfig) -> HasherConfig {
    config
}

#[test]
fn testGetHasherWithMessageDigestHasher() {
    let factory = PasswordHasherFactory::new([(
        PASSWORD_AUTHENTICATED_USER_INTERFACE,
        HasherConfig::options(array(vec![
            ("class", Value::string(MESSAGE_DIGEST)),
            (
                "arguments",
                list(vec![
                    Value::string("sha512"),
                    Value::Bool(true),
                    Value::Int(5),
                ]),
            ),
        ])),
    )]);

    let user = stub();
    let hasher = factory
        .get_password_hasher(HasherSubject::Object(&user))
        .unwrap();
    let expected_hasher = MessageDigestPasswordHasher::new("sha512", true, 5);

    assert_eq!(
        expected_hasher.hash("foo", Some("moo")).unwrap(),
        hasher.hash("foo", Some("moo")).unwrap()
    );
}

#[test]
fn testGetHasherWithService() {
    let factory = PasswordHasherFactory::new([(
        PASSWORD_AUTHENTICATED_USER_INTERFACE,
        HasherConfig::Hasher(digest("sha1")),
    )]);

    let user = stub();
    let hasher = factory
        .get_password_hasher(HasherSubject::Object(&user))
        .unwrap();
    let expected_hasher = MessageDigestPasswordHasher::new("sha1", true, 5000);
    assert_eq!(
        expected_hasher.hash("foo", Some("")).unwrap(),
        hasher.hash("foo", Some("")).unwrap()
    );
}

#[test]
fn testGetHasherWithInstance() {
    let factory = PasswordHasherFactory::new([(
        PASSWORD_AUTHENTICATED_USER_INTERFACE,
        HasherConfig::instance(digest("sha1"), PhpArray::new()),
    )]);

    let user = stub();
    let hasher = factory
        .get_password_hasher(HasherSubject::Object(&user))
        .unwrap();
    let expected_hasher = MessageDigestPasswordHasher::new("sha1", true, 5000);
    assert_eq!(
        expected_hasher.hash("foo", Some("")).unwrap(),
        hasher.hash("foo", Some("")).unwrap()
    );
}

#[test]
fn testGetHasherWithClassName() {
    let factory = PasswordHasherFactory::new([(
        PASSWORD_AUTHENTICATED_USER_INTERFACE,
        HasherConfig::Hasher(digest("sha1")),
    )]);

    let hasher = factory
        .get_password_hasher(some_child_user_class())
        .unwrap();
    let expected_hasher = MessageDigestPasswordHasher::new("sha1", true, 5000);
    assert_eq!(
        expected_hasher.hash("foo", Some("")).unwrap(),
        hasher.hash("foo", Some("")).unwrap()
    );
}

#[test]
fn testGetHasherConfiguredForConcreteClassWithService() {
    let factory = PasswordHasherFactory::new([(
        "Symfony\\Component\\Security\\Core\\User\\InMemoryUser",
        HasherConfig::Hasher(digest("sha1")),
    )]);

    // new InMemoryUser('user', 'pass')
    let user = User {
        class: "Symfony\\Component\\Security\\Core\\User\\InMemoryUser",
        parents: &[
            "Symfony\\Component\\Security\\Core\\User\\UserInterface",
            PASSWORD_AUTHENTICATED_USER_INTERFACE,
            "Symfony\\Component\\Security\\Core\\User\\EquatableInterface",
            "Stringable",
        ],
        hasher_name: None,
    };
    let hasher = factory
        .get_password_hasher(HasherSubject::Object(&user))
        .unwrap();
    let expected_hasher = MessageDigestPasswordHasher::new("sha1", true, 5000);
    assert_eq!(
        expected_hasher.hash("foo", Some("")).unwrap(),
        hasher.hash("foo", Some("")).unwrap()
    );
}

#[test]
fn testGetHasherConfiguredForConcreteClassWithClassName() {
    let factory = PasswordHasherFactory::new([(
        "Symfony\\Component\\PasswordHasher\\Tests\\Hasher\\SomeUser",
        HasherConfig::Hasher(digest("sha1")),
    )]);

    let hasher = factory
        .get_password_hasher(some_child_user_class())
        .unwrap();
    let expected_hasher = MessageDigestPasswordHasher::new("sha1", true, 5000);
    assert_eq!(
        expected_hasher.hash("foo", Some("")).unwrap(),
        hasher.hash("foo", Some("")).unwrap()
    );
}

#[test]
fn testGetHasherConfiguredWithAuto() {
    let factory = PasswordHasherFactory::new([(
        "auto",
        HasherConfig::options(array(vec![("algorithm", Value::string("auto"))])),
    )]);

    let hasher = factory.get_password_hasher(HasherSubject::name("auto"));
    assert!(hasher.is_ok());
}

#[test]
fn testGetNamedHasherForHasherAware() {
    let factory = PasswordHasherFactory::new([
        (HASHER_AWARE_USER, HasherConfig::Hasher(digest("sha256"))),
        ("hasher_name", HasherConfig::Hasher(digest("sha1"))),
    ]);

    let user = hasher_aware_user();
    let hasher = factory
        .get_password_hasher(HasherSubject::Object(&user))
        .unwrap();
    let expected_hasher = MessageDigestPasswordHasher::new("sha1", true, 5000);
    assert_eq!(
        expected_hasher.hash("foo", Some("")).unwrap(),
        hasher.hash("foo", Some("")).unwrap()
    );
}

#[test]
fn testGetNullNamedHasherForHasherAware() {
    let factory = PasswordHasherFactory::new([
        (HASHER_AWARE_USER, HasherConfig::Hasher(digest("sha1"))),
        ("hasher_name", HasherConfig::Hasher(digest("sha256"))),
    ]);

    let mut user = hasher_aware_user();
    user.hasher_name = Some(None);
    let hasher = factory
        .get_password_hasher(HasherSubject::Object(&user))
        .unwrap();
    let expected_hasher = MessageDigestPasswordHasher::new("sha1", true, 5000);
    assert_eq!(
        expected_hasher.hash("foo", Some("")).unwrap(),
        hasher.hash("foo", Some("")).unwrap()
    );
}

#[test]
fn testGetInvalidNamedHasherForHasherAware() {
    let factory = PasswordHasherFactory::new([
        (HASHER_AWARE_USER, HasherConfig::Hasher(digest("sha1"))),
        ("hasher_name", HasherConfig::Hasher(digest("sha256"))),
    ]);

    let mut user = hasher_aware_user();
    user.hasher_name = Some(Some("invalid_hasher_name".to_owned()));

    let error = factory
        .get_password_hasher(HasherSubject::Object(&user))
        .err()
        .unwrap();
    assert!(error.is_runtime());
}

#[test]
fn testGetHasherForHasherAwareWithClassName() {
    let factory = PasswordHasherFactory::new([
        (HASHER_AWARE_USER, HasherConfig::Hasher(digest("sha1"))),
        ("hasher_name", HasherConfig::Hasher(digest("sha256"))),
    ]);

    let hasher = factory
        .get_password_hasher(HasherSubject::Class(PhpClass::new(
            HASHER_AWARE_USER,
            &[SOME_USER, PASSWORD_AUTHENTICATED_USER_INTERFACE],
        )))
        .unwrap();
    let expected_hasher = MessageDigestPasswordHasher::new("sha1", true, 5000);
    assert_eq!(
        expected_hasher.hash("foo", Some("")).unwrap(),
        hasher.hash("foo", Some("")).unwrap()
    );
}

fn assert_migrate_from(
    hasher: &Arc<dyn PasswordHasherInterface>,
    digest: &dyn PasswordHasherInterface,
) {
    assert!(
        hasher
            .as_ref()
            .downcast_ref::<MigratingPasswordHasher>()
            .is_some()
    );

    let sodium = SodiumPasswordHasher::new(None, None).unwrap();
    assert!(
        hasher
            .verify(&sodium.hash("foo", None).unwrap(), "foo", None)
            .unwrap()
    );
    let bcrypt = NativePasswordHasher::new(None, None, None, Some(PASSWORD_BCRYPT)).unwrap();
    assert!(
        hasher
            .verify(&bcrypt.hash("foo", None).unwrap(), "foo", None)
            .unwrap()
    );
    assert!(
        hasher
            .verify(&digest.hash("foo", None).unwrap(), "foo", None)
            .unwrap()
    );
    assert!(
        hasher
            .hash("foo", None)
            .unwrap()
            .starts_with(SODIUM_CRYPTO_PWHASH_STRPREFIX)
    );
}

#[test]
fn testMigrateFrom() {
    assert!(SodiumPasswordHasher::is_supported());

    let digest = digest("sha256");
    let factory = PasswordHasherFactory::new([
        ("digest_hasher", HasherConfig::Hasher(digest.clone())),
        (
            SOME_USER,
            HasherConfig::options(array(vec![
                ("algorithm", Value::string("sodium")),
                (
                    "migrate_from",
                    list(vec![
                        Value::string("bcrypt"),
                        Value::string("digest_hasher"),
                    ]),
                ),
            ])),
        ),
    ]);

    let hasher = factory.get_password_hasher(some_user_class()).unwrap();
    assert_migrate_from(&hasher, digest.as_ref());
}

#[test]
fn testMissingClass() {
    let factory = PasswordHasherFactory::new([(
        SOME_USER,
        HasherConfig::options(array(vec![("arguments", list(vec![]))])),
    )]);
    let error = factory
        .get_password_hasher(some_user_class())
        .err()
        .unwrap();
    assert_eq!(&Class::InvalidArgument, error.class());
    assert_eq!(
        "\"class\" must be set in {\"arguments\":[]}",
        error.message()
    );
}

#[test]
fn testMissingArguments() {
    let factory = PasswordHasherFactory::new([(
        SOME_USER,
        HasherConfig::options(array(vec![("class", Value::string("stdClass"))])),
    )]);
    let error = factory
        .get_password_hasher(some_user_class())
        .err()
        .unwrap();
    assert_eq!(&Class::InvalidArgument, error.class());
    assert_eq!(
        "\"arguments\" must be set in {\"class\":\"stdClass\"}",
        error.message()
    );
}

#[test]
fn testDefaultMigratingHashers() {
    let is_migrating = |config: HasherConfig| {
        let factory = PasswordHasherFactory::new([(SOME_USER, hasher(config))]);
        let hasher = factory.get_password_hasher(some_user_class()).unwrap();
        hasher
            .as_ref()
            .downcast_ref::<MigratingPasswordHasher>()
            .is_some()
    };

    assert!(is_migrating(HasherConfig::options(array(vec![
        ("class", Value::string(NATIVE)),
        ("arguments", list(vec![])),
    ]))));

    assert!(is_migrating(HasherConfig::options(array(vec![
        ("algorithm", Value::string("bcrypt")),
        ("cost", Value::Int(11)),
    ]))));

    assert!(SodiumPasswordHasher::is_supported());

    assert!(is_migrating(HasherConfig::options(array(vec![
        ("class", Value::string(SODIUM)),
        ("arguments", list(vec![])),
    ]))));
}

#[test]
fn testMigrateFromWithCustomInstance() {
    assert!(SodiumPasswordHasher::is_supported());

    let sodium: Arc<dyn PasswordHasherInterface> =
        Arc::new(SodiumPasswordHasher::new(None, None).unwrap());
    let digest = digest("sha256");
    let factory = PasswordHasherFactory::new([
        ("digest_hasher", HasherConfig::Hasher(digest.clone())),
        (
            SOME_USER,
            HasherConfig::instance(
                sodium,
                array(vec![(
                    "migrate_from",
                    list(vec![
                        Value::string("bcrypt"),
                        Value::string("digest_hasher"),
                    ]),
                )]),
            ),
        ),
    ]);

    let hasher = factory.get_password_hasher(some_user_class()).unwrap();
    assert_migrate_from(&hasher, digest.as_ref());
}
