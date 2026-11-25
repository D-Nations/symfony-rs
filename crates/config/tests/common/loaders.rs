//! Test doubles for the loader tests: what PHPUnit's `createMock()` /
//! `createStub()` give upstream (a loader, a resolver, a locator with
//! configured answers that count their calls), and upstream's
//! `ProjectLoader1` and `TestFileLoader`.
//!
//! Upstream: symfony/config Tests/Loader/LoaderTest.php Tests/Loader/FileLoaderTest.php
#![allow(
    clippy::unused_self,
    reason = "test code ports PHPUnit tests and helpers as they are"
)]

use std::sync::{
    Arc, Mutex, RwLock, Weak,
    atomic::{AtomicBool, AtomicUsize, Ordering},
};

use php_value::Value;
use symfony_config::{
    Class, Error,
    file_locator::{FileLocatorInterface, Located},
    loader::{FileLoader, Loader, LoaderInterface, LoaderResolverInterface},
};

type Call = (Value, Option<Vec<u8>>);

fn call(resource: &Value, r#type: Option<&[u8]>) -> Call {
    (resource.clone(), r#type.map(<[u8]>::to_vec))
}

/// `createMock(LoaderInterface::class)`: `supports()` answers
/// `supports`, `load()` returns `returns`.
pub struct MockLoader {
    supports: bool,
    returns: Value,
    pub supports_calls: AtomicUsize,
    pub load_calls: Mutex<Vec<Call>>,
    resolver: RwLock<Option<Arc<dyn LoaderResolverInterface>>>,
}

impl MockLoader {
    pub fn new(supports: bool, returns: Value) -> Arc<Self> {
        Arc::new(Self {
            supports,
            returns,
            supports_calls: AtomicUsize::new(0),
            load_calls: Mutex::new(Vec::new()),
            resolver: RwLock::new(None),
        })
    }

    /// `createStub(LoaderInterface::class)`: supports nothing.
    pub fn stub() -> Arc<Self> {
        Self::new(false, Value::Null)
    }

    pub fn supports_count(&self) -> usize {
        self.supports_calls.load(Ordering::SeqCst)
    }

    pub fn loads(&self) -> Vec<Call> {
        self.load_calls
            .lock()
            .map(|c| c.clone())
            .unwrap_or_default()
    }
}

impl LoaderInterface for MockLoader {
    fn class(&self) -> &'static str {
        "MockObject_LoaderInterface"
    }

    fn load(&self, resource: &Value, r#type: Option<&[u8]>) -> Result<Value, Error> {
        if let Ok(mut calls) = self.load_calls.lock() {
            calls.push(call(resource, r#type));
        }
        Ok(self.returns.clone())
    }

    fn supports(&self, _resource: &Value, _type: Option<&[u8]>) -> bool {
        self.supports_calls.fetch_add(1, Ordering::SeqCst);
        self.supports
    }

    fn get_resolver(&self) -> Result<Arc<dyn LoaderResolverInterface>, Error> {
        self.resolver
            .read()
            .ok()
            .and_then(|r| r.clone())
            .ok_or_else(|| Error::new(Class::TypeError, "no resolver"))
    }

    fn set_resolver(&self, resolver: Arc<dyn LoaderResolverInterface>) {
        if let Ok(mut r) = self.resolver.write() {
            *r = Some(resolver);
        }
    }
}

/// `createMock(LoaderResolverInterface::class)` whose `resolve()`
/// returns `returns`.
pub struct MockResolver {
    returns: Option<Arc<dyn LoaderInterface>>,
    pub calls: Mutex<Vec<Call>>,
}

impl MockResolver {
    pub fn new(returns: Option<Arc<dyn LoaderInterface>>) -> Arc<Self> {
        Arc::new(Self {
            returns,
            calls: Mutex::new(Vec::new()),
        })
    }

    pub fn resolves(&self) -> Vec<Call> {
        self.calls.lock().map(|c| c.clone()).unwrap_or_default()
    }
}

impl LoaderResolverInterface for MockResolver {
    fn resolve(&self, resource: &Value, r#type: Option<&[u8]>) -> Option<Arc<dyn LoaderInterface>> {
        if let Ok(mut calls) = self.calls.lock() {
            calls.push(call(resource, r#type));
        }
        self.returns.clone()
    }
}

/// `createMock(FileLocatorInterface::class)` / `createStub()` whose
/// `locate()` returns the answers in turn (`willReturn(...)`), the last
/// one again once they run out.
pub struct MockLocator {
    answers: Vec<Located>,
    pub calls: AtomicUsize,
}

impl MockLocator {
    pub fn new(answers: Vec<Located>) -> Arc<Self> {
        Arc::new(Self {
            answers,
            calls: AtomicUsize::new(0),
        })
    }

    pub fn count(&self) -> usize {
        self.calls.load(Ordering::SeqCst)
    }
}

impl FileLocatorInterface for MockLocator {
    fn locate(
        &self,
        _name: &[u8],
        _current: Option<&[u8]>,
        _first: bool,
    ) -> Result<Located, Error> {
        let n = self.calls.fetch_add(1, Ordering::SeqCst);
        Ok(self
            .answers
            .get(n)
            .or(self.answers.last())
            .cloned()
            .unwrap_or(Located::Path(Vec::new())))
    }
}

/// `class ProjectLoader1 extends Loader`: supports strings whose
/// extension is "foo"; `load()` returns null.
pub struct ProjectLoader1 {
    pub loader: Loader,
}

impl ProjectLoader1 {
    pub fn new() -> Arc<Self> {
        Arc::new_cyclic(|this: &Weak<Self>| {
            let this: Weak<dyn LoaderInterface> = this.clone();
            Self {
                loader: Loader::new(None, this),
            }
        })
    }
}

/// `pathinfo($path, PATHINFO_EXTENSION)`.
fn extension(path: &[u8]) -> &[u8] {
    let base = path.rsplit(|&c| c == b'/').next().unwrap_or_default();
    match base.iter().rposition(|&c| c == b'.') {
        Some(at) => &base[at + 1..],
        None => b"",
    }
}

impl LoaderInterface for ProjectLoader1 {
    fn class(&self) -> &'static str {
        "Symfony\\Component\\Config\\Tests\\Loader\\ProjectLoader1"
    }

    fn load(&self, _resource: &Value, _type: Option<&[u8]>) -> Result<Value, Error> {
        Ok(Value::Null)
    }

    fn supports(&self, resource: &Value, _type: Option<&[u8]>) -> bool {
        matches!(resource, Value::String(s) if extension(s.as_bytes()) == b"foo")
    }

    fn get_resolver(&self) -> Result<Arc<dyn LoaderResolverInterface>, Error> {
        self.loader.get_resolver()
    }

    fn set_resolver(&self, resolver: Arc<dyn LoaderResolverInterface>) {
        self.loader.set_resolver(resolver);
    }
}

/// `class TestFileLoader extends FileLoader`: returns the resource;
/// supports everything unless told otherwise.
pub struct TestFileLoader {
    pub file: FileLoader,
    supports: AtomicBool,
}

impl TestFileLoader {
    pub fn new(locator: Arc<dyn FileLocatorInterface>) -> Arc<Self> {
        Arc::new_cyclic(|this: &Weak<Self>| {
            let this: Weak<dyn LoaderInterface> = this.clone();
            Self {
                file: FileLoader::new(locator, None, this),
                supports: AtomicBool::new(true),
            }
        })
    }

    pub fn set_supports(&self, supports: bool) {
        self.supports.store(supports, Ordering::SeqCst);
    }

    pub fn add_loading(&self, resource: &str) {
        FileLoader::add_loading(resource.as_bytes());
    }

    /// `import($resource)`.
    pub fn import(&self, resource: &str) -> Result<Value, Error> {
        self.file
            .import(&Value::string(resource), None, false, None, &Value::Null)
    }
}

impl LoaderInterface for TestFileLoader {
    fn class(&self) -> &'static str {
        "Symfony\\Component\\Config\\Tests\\Loader\\TestFileLoader"
    }

    fn load(&self, resource: &Value, _type: Option<&[u8]>) -> Result<Value, Error> {
        Ok(resource.clone())
    }

    fn supports(&self, _resource: &Value, _type: Option<&[u8]>) -> bool {
        self.supports.load(Ordering::SeqCst)
    }

    fn get_resolver(&self) -> Result<Arc<dyn LoaderResolverInterface>, Error> {
        self.file.loader().get_resolver()
    }

    fn set_resolver(&self, resolver: Arc<dyn LoaderResolverInterface>) {
        self.file.loader().set_resolver(resolver);
    }

    fn as_file_loader(&self) -> Option<&FileLoader> {
        Some(&self.file)
    }
}
