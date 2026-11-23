//! Port of Symfony\Component\Config\Loader\FileLoader.php: imports with
//! glob patterns and exclusions, through the file locator, detecting
//! circular imports.
//!
//! Upstream's `protected static $loading` (shared by every FileLoader of
//! the PHP process, which serves one request) is per thread here, as
//! Config's placeholders are. `class_exists($resource)` triggers PHP's
//! autoloader; a FileLoader is given that answer by
//! [`FileLoader::set_class_exists`] (none by default). `glob()` collects
//! its paths before `import()` loads them, where upstream's generator
//! interleaves the two; the paths are the same unless a load changes
//! the directory being globbed.
//!
//! Symfony source copyright (c) Fabien Potencier; distributed under MIT.
//!
//! Upstream: symfony/config Loader/FileLoader.php

use std::{
    cell::RefCell,
    sync::{Arc, RwLock, Weak},
};

use php_value::{Key, PhpArray, Value};
use symfony_finder::fs;

use super::{Loader, LoaderInterface};
use crate::{
    Class, Error,
    exception::{file_loader_import_circular_reference_exception, loader_load_exception},
    file_locator::{FileLocatorInterface, Located},
    php,
    resource::{FileExistenceResource, GlobResource},
};

thread_local! {
    /// `FileLoader::$loading`: the resources being loaded, in order.
    static LOADING: RefCell<Vec<Vec<u8>>> = const { RefCell::new(Vec::new()) };
}

/// What `glob()` sets its `&$resource` argument to.
#[derive(Debug)]
pub enum GlobResult {
    /// The pattern's resource.
    Glob(GlobResource),
    /// The prefix was not found (with `$ignoreErrors`): a
    /// FileExistenceResource for each path the locator tried.
    Existence(Vec<FileExistenceResource>),
}

type ClassExists = Arc<dyn Fn(&[u8]) -> bool + Send + Sync>;

/// `abstract class FileLoader extends Loader`, embedded in a concrete
/// loader.
pub struct FileLoader {
    loader: Loader,
    locator: Arc<dyn FileLocatorInterface>,
    current_dir: RwLock<Option<Vec<u8>>>,
    class_exists: RwLock<Option<ClassExists>>,
}

impl FileLoader {
    /// `parent::__construct($locator, $env)`; `this` is the embedding
    /// loader (from `Arc::new_cyclic`).
    pub fn new(
        locator: Arc<dyn FileLocatorInterface>,
        env: Option<&[u8]>,
        this: Weak<dyn LoaderInterface>,
    ) -> Self {
        Self {
            loader: Loader::new(env, this),
            locator,
            current_dir: RwLock::new(None),
            class_exists: RwLock::new(None),
        }
    }

    /// The embedded Loader.
    pub fn loader(&self) -> &Loader {
        &self.loader
    }

    /// `setCurrentDir($dir)`.
    pub fn set_current_dir(&self, dir: &[u8]) {
        *self.current_dir.write().unwrap_or_else(|e| e.into_inner()) = Some(dir.to_vec());
    }

    fn current_dir(&self) -> Option<Vec<u8>> {
        self.current_dir
            .read()
            .unwrap_or_else(|e| e.into_inner())
            .clone()
    }

    /// `getLocator()`.
    pub fn get_locator(&self) -> Arc<dyn FileLocatorInterface> {
        Arc::clone(&self.locator)
    }

    /// What `class_exists()` answers for a resource string (PHP's
    /// autoloader).
    pub fn set_class_exists(&self, class_exists: impl Fn(&[u8]) -> bool + Send + Sync + 'static) {
        *self.class_exists.write().unwrap_or_else(|e| e.into_inner()) = Some(Arc::new(class_exists));
    }

    fn class_exists(&self, name: &[u8]) -> bool {
        self.class_exists
            .read()
            .unwrap_or_else(|e| e.into_inner())
            .as_ref()
            .is_some_and(|f| f(name))
    }

    /// Adds a resource to the loading set (upstream's tests write
    /// `self::$loading[$resource] = true` from a subclass).
    pub fn add_loading(resource: &[u8]) {
        LOADING.with(|l| {
            let mut l = l.borrow_mut();
            if !l.iter().any(|r| r == resource) {
                l.push(resource.to_vec());
            }
        });
    }

    /// `import($resource, $type, $ignoreErrors, $sourceResource,
    /// $exclude)`: `exclude` is a pattern, a list of them, or null.
    pub fn import(
        &self,
        resource: &Value,
        r#type: Option<&[u8]>,
        ignore_errors: bool,
        source_resource: Option<&[u8]>,
        exclude: &Value,
    ) -> Result<Value, Error> {
        let mut excluded = PhpArray::new();
        let patterns: Vec<Value> = match exclude {
            Value::Null => Vec::new(),
            Value::Array(a) => a.values().cloned().collect(),
            other => vec![other.clone()],
        };
        for pattern in patterns {
            let pattern = string_argument(&pattern, "glob", "pattern")?;
            let (paths, _) = self.glob(&pattern, true, false, true, PhpArray::new())?;
            for path in paths {
                // normalize Windows slashes and remove trailing slashes
                let key = rtrim_slashes(&normalize(&path));
                excluded.insert(Key::from_bytes(&key), Value::Bool(true));
            }
        }

        let mut resource = resource.clone();
        match &resource {
            Value::String(s) if !self.class_exists(s.as_bytes()) => {
                let s = s.as_bytes().to_vec();
                let glob_chars = |s: &[u8]| s.iter().position(|c| b"*?{[".contains(c));
                let mut s = s;
                let mut is_glob_pattern = glob_chars(&s).is_some();

                if !is_glob_pattern && !excluded.is_empty() {
                    s = rtrim_slashes(&normalize(&s));
                    s.extend_from_slice(b"/**/*");
                    is_glob_pattern = true;
                }

                if is_glob_pattern && !s.contains(&b'\n') {
                    let mut ret = Vec::new();
                    let i = glob_chars(&s).unwrap_or(s.len());
                    let mut is_subpath = i != 0 && s[..i].contains(&b'/');
                    let (paths, _) = self.glob(
                        &s,
                        false,
                        ignore_errors || !is_subpath,
                        false,
                        excluded.clone(),
                    )?;
                    let r#type_for = if r#type == Some(b"glob") { None } else { r#type };
                    for path in paths {
                        let res = self.do_import(
                            Value::string(path),
                            r#type_for,
                            ignore_errors,
                            source_resource,
                        )?;
                        if !res.is_null() {
                            ret.push(res);
                        }
                        is_subpath = true;
                    }

                    if is_subpath {
                        return Ok(match ret.len() {
                            0 => Value::Null,
                            1 => ret.remove(0),
                            _ => Value::Array(PhpArray::list(ret)),
                        });
                    }
                }
                resource = Value::string(s);
            }
            Value::Array(a) if !excluded.is_empty() => {
                let mut a = a.clone();
                a.insert(Key::from("_excluded"), Value::Array(excluded));
                resource = Value::Array(a);
            }
            _ => {}
        }

        self.do_import(resource, r#type, ignore_errors, source_resource)
    }

    /// `glob($pattern, $recursive, &$resource, $ignoreErrors,
    /// $forExclusion, $excluded)`: the paths found, and what upstream
    /// assigns to `$resource`.
    pub fn glob(
        &self,
        pattern: &[u8],
        recursive: bool,
        ignore_errors: bool,
        for_exclusion: bool,
        excluded: PhpArray,
    ) -> Result<(Vec<Vec<u8>>, GlobResult), Error> {
        let i = pattern
            .iter()
            .position(|c| b"*?{[".contains(c))
            .unwrap_or(pattern.len());
        let (prefix, pattern) = if i == pattern.len() {
            (pattern.to_vec(), Vec::new())
        } else if i == 0 || !pattern[..i].contains(&b'/') {
            let mut p = b"/".to_vec();
            p.extend_from_slice(pattern);
            (b".".to_vec(), p)
        } else {
            let prefix = fs::dirname(&pattern[..=i]);
            let rest = pattern.get(prefix.len()..).unwrap_or_default().to_vec();
            (prefix, rest)
        };

        let current_dir = self.current_dir();
        let prefix = match self.locator.locate(&prefix, current_dir.as_deref(), true) {
            Ok(Located::Path(p)) => p,
            Ok(Located::Paths(_)) => {
                return Err(Error::new(
                    Class::TypeError,
                    "Symfony\\Component\\Config\\Resource\\GlobResource::__construct(): Argument #1 ($prefix) must be of type string, array given",
                ));
            }
            Err(e) if *e.class() == Class::FileLocatorFileNotFound => {
                if !ignore_errors {
                    return Err(e);
                }

                let resources = e
                    .get_paths()
                    .iter()
                    .map(|p| FileExistenceResource::new(p))
                    .collect();
                return Ok((Vec::new(), GlobResult::Existence(resources)));
            }
            Err(e) => return Err(e),
        };
        let resource = GlobResource::new(&prefix, &pattern, recursive, for_exclusion, excluded)?;
        let paths = resource.get_iterator()?;

        Ok((paths, GlobResult::Glob(resource)))
    }

    fn do_import(
        &self,
        resource: Value,
        r#type: Option<&[u8]>,
        ignore_errors: bool,
        source_resource: Option<&[u8]>,
    ) -> Result<Value, Error> {
        let mut resource = resource;
        let result = self.try_import(&mut resource, r#type);
        match result {
            Ok(v) => Ok(v),
            Err(e) if *e.class() == Class::FileLoaderImportCircularReference => Err(e),
            Err(e) if e.class().is_exception() => {
                if ignore_errors {
                    return Ok(Value::Null);
                }
                // prevent embedded imports from nesting multiple
                // exceptions
                if e.class().is_loader_load() {
                    return Err(e);
                }

                Err(loader_load_exception(
                    &resource,
                    source_resource,
                    0,
                    Some(e),
                    r#type,
                ))
            }
            Err(e) => Err(e),
        }
    }

    /// The `try` block of `doImport()`; `resource` is updated as upstream
    /// reassigns `$resource`, for the exception built from it.
    fn try_import(&self, resource: &mut Value, r#type: Option<&[u8]>) -> Result<Value, Error> {
        let mut loader = self.loader.resolve(resource, r#type)?;

        if let Some(aware) = loader.as_directory_aware() {
            let Some(dir) = self.current_dir() else {
                return Err(Error::new(
                    Class::TypeError,
                    format!(
                        "{}::forDirectory(): Argument #1 ($currentDirectory) must be of type string, null given",
                        loader.class()
                    ),
                ));
            };
            loader = aware.for_directory(&dir);
        }

        let Some(file_loader) = loader.as_file_loader() else {
            return loader.load(resource, r#type);
        };

        if let Some(dir) = self.current_dir() {
            let name = string_argument(resource, "Symfony\\Component\\Config\\FileLocatorInterface::locate", "name")?;
            *resource = file_loader
                .get_locator()
                .locate(&name, Some(&dir), false)?
                .to_value();
        }

        let resources: Vec<Value> = match &*resource {
            Value::Array(a) => (0..a.len())
                .map(|i| {
                    a.get(&Key::Int(i64::try_from(i).unwrap_or(i64::MAX)))
                        .cloned()
                        .unwrap_or(Value::Null)
                })
                .collect(),
            other => vec![other.clone()],
        };
        let count = resources.len();
        for (i, candidate) in resources.iter().enumerate() {
            let key = offset(candidate, true)?;
            if LOADING.with(|l| l.borrow().contains(&key)) {
                if i == count - 1 {
                    let keys = LOADING.with(|l| l.borrow().clone());
                    return Err(file_loader_import_circular_reference_exception(&keys));
                }
            } else {
                *resource = candidate.clone();
                break;
            }
        }
        let key = offset(resource, false)?;
        Self::add_loading(&key);

        let ret = loader.load(resource, r#type);
        LOADING.with(|l| l.borrow_mut().retain(|r| *r != key));

        ret
    }
}

/// An array offset as `isset($a[$v])` / `$a[$v] = …` reads it: the key's
/// string form, or PHP's TypeError for arrays and objects.
fn offset(value: &Value, isset: bool) -> Result<Vec<u8>, Error> {
    match value {
        Value::Null => Ok(Vec::new()),
        Value::Bool(b) => Ok(if *b { b"1".to_vec() } else { b"0".to_vec() }),
        Value::Float(f) if f.is_finite() => Ok((f.trunc() as i64).to_string().into_bytes()),
        Value::Int(_) | Value::String(_) | Value::Float(_) => Ok(php::to_bytes(value)),
        other => Err(Error::new(
            Class::TypeError,
            if isset {
                format!(
                    "Cannot access offset of type {} in isset or empty",
                    other.debug_type()
                )
            } else {
                format!(
                    "Cannot access offset of type {} on array",
                    other.debug_type()
                )
            },
        )),
    }
}

/// A `string` parameter in PHP's coercive mode: scalars convert, null,
/// arrays and objects are a TypeError.
fn string_argument(value: &Value, function: &str, parameter: &str) -> Result<Vec<u8>, Error> {
    match value {
        Value::String(s) => Ok(s.as_bytes().to_vec()),
        Value::Int(_) | Value::Float(_) | Value::Bool(_) => Ok(php::to_bytes(value)),
        other => Err(Error::new(
            Class::TypeError,
            format!(
                "{function}(): Argument #1 (${parameter}) must be of type string, {} given",
                php::error_type_name(other)
            ),
        )),
    }
}

fn normalize(path: &[u8]) -> Vec<u8> {
    path.iter()
        .map(|&c| if c == b'\\' { b'/' } else { c })
        .collect()
}

fn rtrim_slashes(path: &[u8]) -> Vec<u8> {
    let mut end = path.len();
    while end > 0 && path[end - 1] == b'/' {
        end -= 1;
    }
    path[..end].to_vec()
}
