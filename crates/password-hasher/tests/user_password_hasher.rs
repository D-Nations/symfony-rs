//! UserPasswordHasherTest.php at symfony/password-hasher
//! 6934d16beaa4677f2c4584229fff1b51099dd7af, with the fixtures
//! TestLegacyPasswordAuthenticatedUser.php and
//! TestPasswordAuthenticatedUser.php. Names and assertions are PHPUnit's;
//! the mocks check their arguments and count their calls.
//!
//! Upstream: symfony/password-hasher Tests/Hasher/UserPasswordHasherTest.php Tests/Fixtures/TestLegacyPasswordAuthenticatedUser.php Tests/Fixtures/TestPasswordAuthenticatedUser.php
#![allow(non_snake_case, clippy::unwrap_used)]

use std::sync::{
    Arc, Mutex,
    atomic::{AtomicUsize, Ordering},
};

use symfony_password_hasher::{
    Error, HasherConfig, HasherSubject, LegacyPasswordAuthenticatedUserInterface,
    NativePasswordHasher, PasswordAuthenticatedUserInterface, PasswordHasherFactory,
    PasswordHasherFactoryInterface, PasswordHasherInterface, PhpClass, UserObject,
    UserPasswordHasher, UserPasswordHasherInterface,
};

const LEGACY_USER: &str =
    "Symfony\\Component\\PasswordHasher\\Tests\\Fixtures\\TestLegacyPasswordAuthenticatedUser";
const USER: &str =
    "Symfony\\Component\\PasswordHasher\\Tests\\Fixtures\\TestPasswordAuthenticatedUser";
const LEGACY_PARENTS: &[&str] = &[
    "Symfony\\Component\\Security\\Core\\User\\LegacyPasswordAuthenticatedUserInterface",
    "Symfony\\Component\\Security\\Core\\User\\PasswordAuthenticatedUserInterface",
    "Symfony\\Component\\Security\\Core\\User\\UserInterface",
];
const PARENTS: &[&str] =
    &["Symfony\\Component\\Security\\Core\\User\\PasswordAuthenticatedUserInterface"];

/// TestLegacyPasswordAuthenticatedUser.php
struct TestLegacyPasswordAuthenticatedUser {
    password: Option<String>,
    salt: Option<String>,
}

impl UserObject for TestLegacyPasswordAuthenticatedUser {
    fn php_class(&self) -> PhpClass<'_> {
        PhpClass::new(LEGACY_USER, LEGACY_PARENTS)
    }
}

impl PasswordAuthenticatedUserInterface for TestLegacyPasswordAuthenticatedUser {
    fn get_password(&self) -> Option<String> {
        self.password.clone()
    }

    fn as_legacy(&self) -> Option<&dyn LegacyPasswordAuthenticatedUserInterface> {
        Some(self)
    }
}

impl LegacyPasswordAuthenticatedUserInterface for TestLegacyPasswordAuthenticatedUser {
    fn get_salt(&self) -> Option<String> {
        self.salt.clone()
    }
}

/// TestPasswordAuthenticatedUser.php (and, for testNeedsRehash, Security
/// Core's `InMemoryUser`, whose password the test sets by binding).
struct TestPasswordAuthenticatedUser {
    password: Mutex<Option<String>>,
}

impl TestPasswordAuthenticatedUser {
    fn new(password: Option<&str>) -> Self {
        Self {
            password: Mutex::new(password.map(str::to_owned)),
        }
    }
}

impl UserObject for TestPasswordAuthenticatedUser {
    fn php_class(&self) -> PhpClass<'_> {
        PhpClass::new(USER, PARENTS)
    }
}

impl PasswordAuthenticatedUserInterface for TestPasswordAuthenticatedUser {
    fn get_password(&self) -> Option<String> {
        self.password.lock().unwrap().clone()
    }
}

type Expected = (Option<&'static str>, &'static str, Option<&'static str>);

/// `createMock(PasswordHasherInterface::class)` expecting one `hash()` or
/// one `verify()` call with these arguments.
struct MockHasher {
    hash: Option<(&'static str, Option<&'static str>)>,
    verify: Option<Expected>,
    calls: AtomicUsize,
}

impl PasswordHasherInterface for MockHasher {
    fn hash(&self, plain: &str, salt: Option<&str>) -> Result<String, Error> {
        self.calls.fetch_add(1, Ordering::SeqCst);
        assert_eq!(self.hash, Some((plain, salt)));
        Ok("hash".to_owned())
    }

    fn verify(&self, hashed: &str, plain: &str, salt: Option<&str>) -> Result<bool, Error> {
        self.calls.fetch_add(1, Ordering::SeqCst);
        let expected = self.verify.unwrap();
        assert_eq!(expected.0, Some(hashed));
        assert_eq!(expected.1, plain);
        assert_eq!(expected.2, salt);
        Ok(true)
    }

    fn needs_rehash(&self, _hashed: &str) -> bool {
        unreachable!()
    }

    fn class_name(&self) -> &'static str {
        "Mock_PasswordHasherInterface"
    }
}

fn factory_for(class: &str, hasher: Arc<MockHasher>) -> Arc<PasswordHasherFactory> {
    Arc::new(PasswordHasherFactory::new([(
        class,
        HasherConfig::Hasher(hasher),
    )]))
}

#[test]
fn testHashWithLegacyUser() {
    let user = TestLegacyPasswordAuthenticatedUser {
        password: None,
        salt: Some("userSalt".to_owned()),
    };

    let password_hasher = Arc::new(MockHasher {
        hash: Some(("plainPassword", Some("userSalt"))),
        verify: None,
        calls: AtomicUsize::new(0),
    });

    let password_hasher_factory = factory_for(LEGACY_USER, password_hasher.clone());

    let user_password_hasher = UserPasswordHasher::new(password_hasher_factory);

    let encoded = user_password_hasher
        .hash_password(&user, "plainPassword")
        .unwrap();
    assert_eq!("hash", encoded);
    assert_eq!(1, password_hasher.calls.load(Ordering::SeqCst));
}

#[test]
fn testHashWithPasswordAuthenticatedUser() {
    let user = TestPasswordAuthenticatedUser::new(None);

    let password_hasher = Arc::new(MockHasher {
        hash: Some(("plainPassword", None)),
        verify: None,
        calls: AtomicUsize::new(0),
    });

    let password_hasher_factory = factory_for(USER, password_hasher.clone());

    let user_password_hasher = UserPasswordHasher::new(password_hasher_factory);

    let hashed_password = user_password_hasher
        .hash_password(&user, "plainPassword")
        .unwrap();

    assert_eq!("hash", hashed_password);
    assert_eq!(1, password_hasher.calls.load(Ordering::SeqCst));
}

#[test]
fn testVerifyWithLegacyUser() {
    let user = TestLegacyPasswordAuthenticatedUser {
        password: Some("hash".to_owned()),
        salt: Some("userSalt".to_owned()),
    };

    let password_hasher = Arc::new(MockHasher {
        hash: None,
        verify: Some((Some("hash"), "plainPassword", Some("userSalt"))),
        calls: AtomicUsize::new(0),
    });

    let password_hasher_factory = factory_for(LEGACY_USER, password_hasher.clone());

    let user_password_hasher = UserPasswordHasher::new(password_hasher_factory);

    let is_valid = user_password_hasher
        .is_password_valid(&user, "plainPassword")
        .unwrap();
    assert!(is_valid);
    assert_eq!(1, password_hasher.calls.load(Ordering::SeqCst));
}

#[test]
fn testVerify() {
    let user = TestPasswordAuthenticatedUser::new(Some("hash"));

    let password_hasher = Arc::new(MockHasher {
        hash: None,
        verify: Some((Some("hash"), "plainPassword", None)),
        calls: AtomicUsize::new(0),
    });

    let password_hasher_factory = factory_for(USER, password_hasher.clone());

    let user_password_hasher = UserPasswordHasher::new(password_hasher_factory);

    let is_valid = user_password_hasher
        .is_password_valid(&user, "plainPassword")
        .unwrap();
    assert!(is_valid);
    assert_eq!(1, password_hasher.calls.load(Ordering::SeqCst));
}

/// `createMock(PasswordHasherFactoryInterface::class)`: `getPasswordHasher()`
/// returns the given hashers in turn, once each.
struct MockFactory {
    hashers: Mutex<Vec<Arc<dyn PasswordHasherInterface>>>,
    calls: AtomicUsize,
}

impl PasswordHasherFactoryInterface for MockFactory {
    fn get_password_hasher(
        &self,
        user: HasherSubject<'_>,
    ) -> Result<Arc<dyn PasswordHasherInterface>, Error> {
        assert!(matches!(user, HasherSubject::Object(_)));
        self.calls.fetch_add(1, Ordering::SeqCst);
        Ok(self.hashers.lock().unwrap().remove(0))
    }
}

#[test]
fn testNeedsRehash() {
    let user = TestPasswordAuthenticatedUser::new(None);
    let hasher: Arc<dyn PasswordHasherInterface> =
        Arc::new(NativePasswordHasher::new(Some(4), Some(20000), Some(4), None).unwrap());

    let other: Arc<dyn PasswordHasherInterface> =
        Arc::new(NativePasswordHasher::new(Some(5), Some(20000), Some(5), None).unwrap());
    let password_hasher_factory = Arc::new(MockFactory {
        hashers: Mutex::new(vec![hasher.clone(), hasher.clone(), other, hasher]),
        calls: AtomicUsize::new(0),
    });

    let password_hasher = UserPasswordHasher::new(password_hasher_factory.clone());

    let new_password = password_hasher.hash_password(&user, "foo").unwrap();
    *user.password.lock().unwrap() = Some(new_password);
    assert!(!password_hasher.needs_rehash(&user).unwrap());
    assert!(password_hasher.needs_rehash(&user).unwrap());
    assert!(!password_hasher.needs_rehash(&user).unwrap());
    assert_eq!(4, password_hasher_factory.calls.load(Ordering::SeqCst));
}
