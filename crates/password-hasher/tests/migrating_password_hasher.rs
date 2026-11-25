//! MigratingPasswordHasherTest.php at symfony/password-hasher
//! 6934d16beaa4677f2c4584229fff1b51099dd7af. Names and assertions are
//! PHPUnit's; the mocks are a hasher that counts its calls and checks
//! their arguments.
//!
//! Upstream: symfony/password-hasher Tests/Hasher/MigratingPasswordHasherTest.php
#![allow(non_snake_case, clippy::unwrap_used)]

use std::sync::{
    Arc,
    atomic::{AtomicUsize, Ordering},
};

use symfony_password_hasher::{
    Error, MigratingPasswordHasher, NativePasswordHasher, PasswordHasherInterface,
};

/// `createMock(PasswordHasherInterface::class)`: `verify()` checks its
/// arguments when `expected` is set, and returns `answer`.
#[derive(Default)]
struct MockHasher {
    answer: bool,
    expected: Option<(&'static str, &'static str, Option<&'static str>)>,
    calls: AtomicUsize,
}

impl PasswordHasherInterface for MockHasher {
    fn hash(&self, _plain: &str, _salt: Option<&str>) -> Result<String, Error> {
        self.calls.fetch_add(1, Ordering::SeqCst);
        Ok(String::new())
    }

    fn verify(&self, hashed: &str, plain: &str, salt: Option<&str>) -> Result<bool, Error> {
        self.calls.fetch_add(1, Ordering::SeqCst);
        if let Some(expected) = self.expected {
            assert_eq!(expected, (hashed, plain, salt));
        }
        Ok(self.answer)
    }

    fn needs_rehash(&self, _hashed: &str) -> bool {
        self.calls.fetch_add(1, Ordering::SeqCst);
        false
    }

    fn class_name(&self) -> &'static str {
        "Mock_PasswordHasherInterface"
    }
}

#[test]
fn testValidation() {
    let best_hasher =
        Arc::new(NativePasswordHasher::new(Some(4), Some(12000), Some(4), None).unwrap());

    let extra_hasher = Arc::new(MockHasher::default());

    let hasher = MigratingPasswordHasher::new(best_hasher, vec![extra_hasher.clone()]);

    assert!(hasher.needs_rehash("foo"));

    let hash = hasher.hash("foo", Some("salt")).unwrap();
    assert!(!hasher.needs_rehash(&hash));

    assert!(hasher.verify(&hash, "foo", Some("salt")).unwrap());
    assert!(!hasher.verify(&hash, "bar", Some("salt")).unwrap());

    // never() hash, verify, needsRehash
    assert_eq!(0, extra_hasher.calls.load(Ordering::SeqCst));
}

#[test]
fn testFallback() {
    let best_hasher: Arc<dyn PasswordHasherInterface> =
        Arc::new(NativePasswordHasher::new(Some(4), Some(12000), Some(4), None).unwrap());

    let extra_hasher1 = Arc::new(MockHasher {
        answer: true,
        expected: Some(("abc", "foo", Some("salt"))),
        ..MockHasher::default()
    });

    let hasher = MigratingPasswordHasher::new(best_hasher.clone(), vec![extra_hasher1.clone()]);

    assert!(hasher.verify("abc", "foo", Some("salt")).unwrap());
    assert!(extra_hasher1.calls.load(Ordering::SeqCst) >= 1);

    let extra_hasher2 = Arc::new(MockHasher::default());

    let hasher = MigratingPasswordHasher::new(best_hasher.clone(), vec![extra_hasher2.clone()]);

    assert!(!hasher.verify("abc", "foo", Some("salt")).unwrap());

    let hasher = MigratingPasswordHasher::new(best_hasher, vec![extra_hasher2, extra_hasher1]);

    assert!(hasher.verify("abc", "foo", Some("salt")).unwrap());
}
