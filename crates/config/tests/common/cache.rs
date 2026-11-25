//! Fixtures of the cache tests: `ResourceStub` (a self-checking resource
//! whose freshness is set by hand), the resource with a long class name,
//! PHPUnit's mock and stub of `ResourceCheckerInterface`, and the `setUp()`
//! of `ConfigCacheTest` and `ResourceCheckerConfigCacheTest` (two empty files
//! from `tempnam()`).
//!
//! Neither fixture class defines `__serialize()`, so PHP's `serialize()`
//! stores their private properties under mangled names
//! (`"\0<class>\0<name>"`); [`classes()`] restores them from those.
//!
//! Upstream: symfony/config Tests/Resource/ResourceStub.php Tests/Fixtures/ResourceWithVeryVeryVeryVeryVeryVeryVeryVeryLongName.php

use std::sync::{
    Arc, Mutex,
    atomic::{AtomicUsize, Ordering},
};

use php_value::{Key, PhpArray, Value};
use symfony_config::{
    Error, ResourceCheckerInterface, ResourceClasses,
    resource::{ResourceInterface, SelfCheckingResourceInterface},
};

use super::fs::TempDir;

pub const RESOURCE_STUB: &str = "Symfony\\Component\\Config\\Tests\\Resource\\ResourceStub";
pub const LONG_NAME: &str = "Symfony\\Component\\Config\\Tests\\Fixtures\\ResourceWithVeryVeryVeryVeryVeryVeryVeryVeryLongName";

fn mangled(class: &str, name: &str) -> Key {
    let mut key = vec![0];
    key.extend_from_slice(class.as_bytes());
    key.push(0);
    key.extend_from_slice(name.as_bytes());
    Key::from_bytes(&key)
}

/// `ResourceStub`.
#[derive(Debug)]
pub struct ResourceStub {
    fresh: Mutex<bool>,
}

impl Default for ResourceStub {
    fn default() -> Self {
        Self {
            fresh: Mutex::new(true),
        }
    }
}

impl ResourceStub {
    pub fn new() -> Self {
        Self::default()
    }

    /// `setFresh($isFresh)`.
    pub fn set_fresh(&self, fresh: bool) {
        *self
            .fresh
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner) = fresh;
    }
}

impl ResourceInterface for ResourceStub {
    fn class(&self) -> &str {
        RESOURCE_STUB
    }

    fn to_string(&self) -> Vec<u8> {
        b"stub".to_vec()
    }

    fn serialize(&self) -> Result<PhpArray, Error> {
        let mut data = PhpArray::new();
        data.insert(
            mangled(RESOURCE_STUB, "fresh"),
            Value::Bool(
                *self
                    .fresh
                    .lock()
                    .unwrap_or_else(std::sync::PoisonError::into_inner),
            ),
        );
        Ok(data)
    }

    fn as_self_checking(&self) -> Option<&dyn SelfCheckingResourceInterface> {
        Some(self)
    }
}

impl SelfCheckingResourceInterface for ResourceStub {
    fn is_fresh(&self, _timestamp: i64) -> Result<bool, Error> {
        Ok(*self
            .fresh
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner))
    }
}

/// `ResourceWithVeryVeryVeryVeryVeryVeryVeryVeryLongName`.
#[derive(Debug)]
pub struct LongNameResource {
    resource: Vec<u8>,
}

impl LongNameResource {
    pub fn new(resource: &[u8]) -> Self {
        Self {
            resource: resource.to_vec(),
        }
    }
}

impl ResourceInterface for LongNameResource {
    fn class(&self) -> &str {
        LONG_NAME
    }

    fn to_string(&self) -> Vec<u8> {
        LONG_NAME.as_bytes().to_vec()
    }

    fn serialize(&self) -> Result<PhpArray, Error> {
        let mut data = PhpArray::new();
        data.insert(
            mangled(LONG_NAME, "resource"),
            Value::string(self.resource.clone()),
        );
        Ok(data)
    }
}

/// Config's resources and the two fixtures, as the test autoloader finds
/// them.
pub fn classes() -> ResourceClasses {
    let mut classes = ResourceClasses::default();
    classes.register(
        RESOURCE_STUB,
        Arc::new(|data: &PhpArray| {
            let stub = ResourceStub::new();
            if let Some(Value::Bool(fresh)) = data.get(&mangled(RESOURCE_STUB, "fresh")) {
                stub.set_fresh(*fresh);
            }
            Ok(Arc::new(stub) as Arc<dyn ResourceInterface>)
        }),
    );
    classes.register(
        LONG_NAME,
        Arc::new(|data: &PhpArray| {
            let resource = match data.get(&mangled(LONG_NAME, "resource")) {
                Some(Value::String(s)) => s.as_bytes().to_vec(),
                _ => Vec::new(),
            };
            Ok(Arc::new(LongNameResource::new(&resource)) as Arc<dyn ResourceInterface>)
        }),
    );
    classes
}

/// `createMock(ResourceCheckerInterface::class)` with `willReturn()`
/// answers (PHPUnit's `false` for a stub), counting the calls.
#[derive(Debug, Default)]
pub struct MockChecker {
    pub supports: bool,
    pub fresh: bool,
    pub supports_calls: AtomicUsize,
    pub is_fresh_calls: AtomicUsize,
}

impl MockChecker {
    /// `createStub(ResourceCheckerInterface::class)`.
    pub fn stub() -> Arc<Self> {
        Arc::new(Self::default())
    }

    pub fn returning(supports: bool, fresh: bool) -> Arc<Self> {
        Arc::new(Self {
            supports,
            fresh,
            ..Self::default()
        })
    }

    pub fn supports_calls(&self) -> usize {
        self.supports_calls.load(Ordering::SeqCst)
    }

    pub fn is_fresh_calls(&self) -> usize {
        self.is_fresh_calls.load(Ordering::SeqCst)
    }
}

impl ResourceCheckerInterface for MockChecker {
    fn class(&self) -> &'static str {
        "MockObject_ResourceCheckerInterface"
    }

    fn supports(&self, _metadata: &dyn ResourceInterface) -> bool {
        self.supports_calls.fetch_add(1, Ordering::SeqCst);
        self.supports
    }

    fn is_fresh(&self, _resource: &dyn ResourceInterface, _timestamp: i64) -> Result<bool, Error> {
        self.is_fresh_calls.fetch_add(1, Ordering::SeqCst);
        Ok(self.fresh)
    }
}

/// `setUp()`: `tempnam(sys_get_temp_dir(), 'config_')` twice, in a private
/// directory removed with it (`tearDown()`).
pub struct Files {
    pub dir: TempDir,
    pub cache_file: Vec<u8>,
    pub meta_file: Vec<u8>,
}

pub fn set_up() -> Files {
    let dir = TempDir::new();
    let cache_file = dir.join("/config_cache");
    let meta_file = dir.join("/config_meta");
    super::fs::touch(&cache_file, None);
    super::fs::touch(&meta_file, None);
    Files {
        dir,
        cache_file,
        meta_file,
    }
}

/// This test file's real path (`__FILE__`).
pub fn this_file(name: &str) -> Vec<u8> {
    let path = format!("{}/tests/{name}", env!("CARGO_MANIFEST_DIR"));
    symfony_finder::fs::realpath(path.as_bytes()).unwrap_or_else(|| path.into_bytes())
}
