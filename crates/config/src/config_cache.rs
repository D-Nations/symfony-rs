//! Port of Symfony\Component\Config\ConfigCacheInterface.php,
//! ResourceCheckerInterface.php, ResourceCheckerConfigCache.php and
//! ConfigCache.php: a cache file with a meta file listing the resources
//! it was built from, fresh while the resource checkers say so.
//!
//! **Files.** `write()` dumps the content with symfony-filesystem's
//! atomic `dumpFile()`, then (with metadata) the meta file
//! (`serialize()` of the resources, each `O:<len>:"<class>":...` with its
//! `__serialize()` array) and a `.json` copy that upstream derives from
//! the serialized text: objects nested in an array get the class prefix
//! `Tracking\`, `\0Class\0` property-name prefixes are cut, the result is
//! unserialized with no class allowed and `json_encode()`d, and
//! `"__PHP_Incomplete_Class_Name":"Tracking\\` becomes `"@type":"`. When
//! that encoding fails PHP formats an array with `%s`: the file then
//! reads `{"resources":Array}`. Each file is then set to
//! `0666 & ~umask()`, failures ignored. `OPcache` has no counterpart.
//!
//! **Reading the meta file.** `unserialize()` with every class allowed
//! and a callback that fails for a class PHP cannot load: here the
//! classes are those of a [`ResourceClasses`] registry (Config's four
//! filesystem resources by default, and `stdClass`). An unknown class,
//! a syntax error or data after the value (a PHP warning raised inside
//! `isFresh()`) make the cache stale. A value that is not an array or
//! object is iterated as nothing (PHP warns), so the cache is fresh; an
//! element that is not a resource is a `\TypeError` from the first
//! checker's `supports()`. The `TypeError`'s "called in <file> on line
//! <n>" suffix names PHP's file and is not reproduced.
//!
//! Symfony source copyright (c) Fabien Potencier; distributed under MIT.
//!
//! Upstream: symfony/config ConfigCacheInterface.php ResourceCheckerInterface.php ResourceCheckerConfigCache.php ConfigCache.php

use std::{collections::HashMap, fmt, sync::Arc};

use php_value::{
    PhpArray, PhpString, Value,
    constants::Registry,
    json::JsonFlags,
    serialize::{Classes, serialize, unserialize},
};
use symfony_filesystem::{Filesystem, php as fsphp};
use symfony_finder::fs;

use crate::{
    Class, Error,
    resource::{
        DirectoryResource, FileExistenceResource, FileResource, GlobResource, ResourceInterface,
        SelfCheckingResourceChecker, SkippingResourceChecker,
    },
};

pub const RESOURCE_CHECKER_CONFIG_CACHE_CLASS: &str =
    "Symfony\\Component\\Config\\ResourceCheckerConfigCache";
pub const CONFIG_CACHE_CLASS: &str = "Symfony\\Component\\Config\\ConfigCache";
pub const RESOURCE_INTERFACE: &str = "Symfony\\Component\\Config\\Resource\\ResourceInterface";

/// `ConfigCacheInterface`.
pub trait ConfigCacheInterface {
    /// `getPath()`: the cache file path.
    fn get_path(&self) -> &[u8];

    /// `isFresh()`.
    fn is_fresh(&mut self) -> Result<bool, Error>;

    /// `write($content, $metadata)`: the content, and the resources it
    /// depends on when `metadata` is given.
    fn write(
        &mut self,
        content: &[u8],
        metadata: Option<&[Arc<dyn ResourceInterface>]>,
    ) -> Result<(), Error>;
}

/// `ResourceCheckerInterface`.
pub trait ResourceCheckerInterface: Send + Sync {
    /// The class name, as PHP's messages give it.
    fn class(&self) -> &str;

    /// `supports($metadata)`: whether this checker validates the resource.
    fn supports(&self, metadata: &dyn ResourceInterface) -> bool;

    /// `isFresh($resource, $timestamp)`, `timestamp` being the cache
    /// file's modification time.
    fn is_fresh(&self, resource: &dyn ResourceInterface, timestamp: i64) -> Result<bool, Error>;
}

/// Restores a resource from its unserialized properties.
pub type Restore =
    Arc<dyn Fn(&PhpArray) -> Result<Arc<dyn ResourceInterface>, Error> + Send + Sync>;

/// The resource classes the meta file may name: what PHP's autoloader
/// would find.
#[derive(Clone)]
pub struct ResourceClasses {
    classes: HashMap<String, (String, Restore)>,
}

impl Default for ResourceClasses {
    /// Config's filesystem resources.
    fn default() -> Self {
        let mut classes = Self {
            classes: HashMap::new(),
        };
        classes.register(
            crate::resource::file_resource::CLASS,
            Arc::new(|data| Ok(Arc::new(FileResource::from_serialized(data)?) as _)),
        );
        classes.register(
            crate::resource::file_existence_resource::CLASS,
            Arc::new(|data| Ok(Arc::new(FileExistenceResource::from_serialized(data)?) as _)),
        );
        classes.register(
            crate::resource::directory_resource::CLASS,
            Arc::new(|data| Ok(Arc::new(DirectoryResource::from_serialized(data)?) as _)),
        );
        classes.register(
            crate::resource::glob_resource::CLASS,
            Arc::new(|data| Ok(Arc::new(GlobResource::from_serialized(data)?) as _)),
        );
        classes
    }
}

impl fmt::Debug for ResourceClasses {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_set()
            .entries(self.classes.values().map(|(c, _)| c))
            .finish()
    }
}

impl ResourceClasses {
    /// No class at all.
    pub fn empty() -> Self {
        Self {
            classes: HashMap::new(),
        }
    }

    /// Makes `class` loadable; class names compare case-insensitively.
    pub fn register(&mut self, class: &str, restore: Restore) -> &mut Self {
        self.classes
            .insert(class.to_ascii_lowercase(), (class.to_owned(), restore));
        self
    }

    fn get(&self, class: &str) -> Option<&Restore> {
        self.classes
            .get(&class.to_ascii_lowercase())
            .map(|(_, r)| r)
    }
}

/// A value of the unserialized meta file.
enum Meta {
    Resource(Arc<dyn ResourceInterface>),
    Other(Value),
}

/// symfony-filesystem's exception, as Config passes it on.
fn io_error(e: symfony_filesystem::Error) -> Error {
    let class = Class::Other {
        class: e.class().name().to_owned(),
        exception: true,
    };
    Error::new(class, e.message().to_vec())
}

/// `new ResourceCheckerConfigCache($file, $resourceCheckers, $metaFile)`.
pub struct ResourceCheckerConfigCache {
    file: Vec<u8>,
    resource_checkers: Vec<Arc<dyn ResourceCheckerInterface>>,
    meta_file: Vec<u8>,
    classes: ResourceClasses,
}

impl fmt::Debug for ResourceCheckerConfigCache {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("ResourceCheckerConfigCache")
            .field("file", &String::from_utf8_lossy(&self.file))
            .field("meta_file", &String::from_utf8_lossy(&self.meta_file))
            .field(
                "resource_checkers",
                &self
                    .resource_checkers
                    .iter()
                    .map(|c| c.class().to_owned())
                    .collect::<Vec<_>>(),
            )
            .finish()
    }
}

impl ResourceCheckerConfigCache {
    /// `meta_file` defaults to `$file.'.meta'`.
    pub fn new(
        file: &[u8],
        resource_checkers: impl IntoIterator<Item = Arc<dyn ResourceCheckerInterface>>,
        meta_file: Option<&[u8]>,
    ) -> Self {
        let meta_file = meta_file.map_or_else(
            || {
                let mut m = file.to_vec();
                m.extend_from_slice(b".meta");
                m
            },
            <[u8]>::to_vec,
        );
        Self {
            file: file.to_vec(),
            resource_checkers: resource_checkers.into_iter().collect(),
            meta_file,
            classes: ResourceClasses::default(),
        }
    }

    /// The classes the meta file may name (Config's filesystem resources
    /// by default).
    pub fn with_classes(mut self, classes: ResourceClasses) -> Self {
        self.classes = classes;
        self
    }

    /// The meta file path.
    pub fn meta_file(&self) -> &[u8] {
        &self.meta_file
    }

    /// `safelyUnserialize($file)`: `None` where PHP gives false.
    fn safely_unserialize(&self, file: &[u8]) -> Result<Option<Value>, Error> {
        let content = Filesystem::new().read_file(file).map_err(io_error)?;
        if serialized_length(&content) != Some(content.len()) {
            // a syntax error, or data after the value: PHP warns, and the
            // warning raised in this file aborts
            return Ok(None);
        }
        let mut allowed = Classes::new();
        allowed.allow("stdClass");
        for (class, _) in self.classes.classes.values() {
            allowed.allow(class);
        }
        // `b:0;` is false too, which upstream reads as a failure
        let Some(value) = unserialize(&content, &allowed, &Registry::default())
            .filter(|v| !matches!(v, Value::Bool(false)))
        else {
            return Ok(None);
        };
        if contains_incomplete(&value) {
            // unserialize_callback_func: "Class not found"
            return Ok(None);
        }
        Ok(Some(value))
    }

    /// An unserialized object of a registered resource class as that
    /// resource; anything else unchanged.
    fn restore(&self, value: Value) -> Result<Meta, Error> {
        if let Value::Object(object) = &value
            && let Some(restore) = self.classes.get(object.class())
        {
            return match restore(&object.properties()) {
                Ok(resource) => Ok(Meta::Resource(resource)),
                // a typed property left out: PHP fails only when it is read
                Err(e) if *e.class() == Class::Error => {
                    Ok(Meta::Resource(Arc::new(Uninitialized {
                        class: object.class().to_owned(),
                        error: e,
                    })))
                }
                Err(e) => Err(e),
            };
        }
        Ok(Meta::Other(value))
    }

    fn write_files(
        &self,
        content: &[u8],
        metadata: Option<&[Arc<dyn ResourceInterface>]>,
    ) -> Result<(), Error> {
        let mode = 0o666;
        let umask = fsphp::umask();
        let filesystem = Filesystem::new();
        filesystem
            .dump_file(&self.file, content)
            .map_err(io_error)?;
        let _ = filesystem.chmod(&[&self.file], mode, umask);

        if let Some(metadata) = metadata {
            let ser = serialize_resources(metadata)?;
            filesystem
                .dump_file(&self.meta_file, ser.as_bytes())
                .map_err(io_error)?;
            let _ = filesystem.chmod(&[&self.meta_file], mode, umask);

            let json = tracking_json(ser.as_bytes());
            let mut json_file = self.meta_file.clone();
            json_file.extend_from_slice(b".json");
            filesystem.dump_file(&json_file, &json).map_err(io_error)?;
            let _ = filesystem.chmod(&[&json_file], mode, umask);
        }
        Ok(())
    }
}

impl ConfigCacheInterface for ResourceCheckerConfigCache {
    fn get_path(&self) -> &[u8] {
        &self.file
    }

    fn is_fresh(&mut self) -> Result<bool, Error> {
        if !fs::is_file(&self.file) {
            return Ok(false);
        }
        if self.resource_checkers.is_empty() {
            return Ok(true);
        }
        if !fs::is_file(&self.meta_file) {
            return Ok(false);
        }
        let Some(meta) = self.safely_unserialize(&self.meta_file)? else {
            return Ok(false);
        };
        let time = fs::stat(&self.file).map_or(0, |s| s.mtime);

        // foreach ($meta as $resource): an array's values, an object's
        // properties visible from this class (none of a resource's), and
        // nothing for a scalar (PHP warns)
        let values: Vec<Value> = match meta {
            Value::Array(array) => array.values().cloned().collect(),
            Value::Object(object) if object.class().eq_ignore_ascii_case("stdClass") => {
                object.properties().values().cloned().collect()
            }
            _ => Vec::new(),
        };
        let mut resources = Vec::with_capacity(values.len());
        for value in values {
            resources.push(self.restore(value)?);
        }

        for resource in &resources {
            for checker in &self.resource_checkers {
                let resource = match resource {
                    Meta::Resource(r) => r.as_ref(),
                    Meta::Other(value) => {
                        return Err(Error::new(
                            Class::TypeError,
                            format!(
                                "{}::supports(): Argument #1 ($metadata) must be of type {RESOURCE_INTERFACE}, {} given",
                                checker.class(),
                                argument_type(value)
                            ),
                        ));
                    }
                };
                if !checker.supports(resource) {
                    continue;
                }
                if checker.is_fresh(resource, time)? {
                    break;
                }
                return Ok(false);
            }
        }
        Ok(true)
    }

    fn write(
        &mut self,
        content: &[u8],
        metadata: Option<&[Arc<dyn ResourceInterface>]>,
    ) -> Result<(), Error> {
        self.write_files(content, metadata)
    }
}

/// A self-checking resource unserialized without one of its typed
/// properties: PHP creates it, and the `\Error` comes when a checker
/// reads it (`__toString()` or `isFresh()`).
#[derive(Debug)]
struct Uninitialized {
    class: String,
    error: Error,
}

impl ResourceInterface for Uninitialized {
    fn class(&self) -> &str {
        &self.class
    }

    fn to_string(&self) -> Vec<u8> {
        Vec::new()
    }

    fn serialize(&self) -> Result<PhpArray, Error> {
        Err(self.error.clone())
    }

    fn as_self_checking(&self) -> Option<&dyn crate::resource::SelfCheckingResourceInterface> {
        Some(self)
    }
}

impl crate::resource::SelfCheckingResourceInterface for Uninitialized {
    fn is_fresh(&self, _timestamp: i64) -> Result<bool, Error> {
        Err(self.error.clone())
    }
}

/// How PHP names a value's type in an argument `TypeError`.
fn argument_type(value: &Value) -> String {
    match value {
        Value::Bool(true) => "true".into(),
        Value::Bool(false) => "false".into(),
        other => other.debug_type(),
    }
}

/// `new ConfigCache($file, $debug, $metaFile, $skippedResourceTypes)`:
/// in debug mode, resources are checked by a [`SkippingResourceChecker`]
/// (when skipped types are given) then a [`SelfCheckingResourceChecker`];
/// otherwise an existing cache file is always fresh.
#[derive(Debug)]
pub struct ConfigCache {
    inner: ResourceCheckerConfigCache,
    debug: bool,
}

impl ConfigCache {
    pub fn new(
        file: &[u8],
        debug: bool,
        meta_file: Option<&[u8]>,
        skipped_resource_types: Option<&[&str]>,
    ) -> Self {
        let mut checkers: Vec<Arc<dyn ResourceCheckerInterface>> = Vec::new();
        if debug {
            if let Some(types) = skipped_resource_types {
                checkers.push(Arc::new(SkippingResourceChecker::new(types)));
            }
            checkers.push(Arc::new(SelfCheckingResourceChecker::new()));
        }
        Self {
            inner: ResourceCheckerConfigCache::new(file, checkers, meta_file),
            debug,
        }
    }

    /// The classes the meta file may name.
    pub fn with_classes(mut self, classes: ResourceClasses) -> Self {
        self.inner = self.inner.with_classes(classes);
        self
    }
}

impl ConfigCacheInterface for ConfigCache {
    fn get_path(&self) -> &[u8] {
        self.inner.get_path()
    }

    fn is_fresh(&mut self) -> Result<bool, Error> {
        if !self.debug && fs::is_file(self.inner.get_path()) {
            return Ok(true);
        }
        self.inner.is_fresh()
    }

    fn write(
        &mut self,
        content: &[u8],
        metadata: Option<&[Arc<dyn ResourceInterface>]>,
    ) -> Result<(), Error> {
        self.inner.write(content, metadata)
    }
}

fn contains_incomplete(value: &Value) -> bool {
    match value {
        Value::Array(array) => array.values().any(contains_incomplete),
        Value::Object(object) => {
            object.class() == "__PHP_Incomplete_Class"
                || object.properties().values().any(contains_incomplete)
        }
        _ => false,
    }
}

/// `9 + $m[1]` or `$m[1] - $len` as PHP prints it: an int, or a float
/// once the digits overflow.
fn add_to_digits(digits: &[u8], delta: i64) -> Vec<u8> {
    let text = String::from_utf8_lossy(digits);
    if let Some(n) = text.parse::<i64>().ok().and_then(|n| n.checked_add(delta)) {
        n.to_string().into_bytes()
    } else {
        let f = php_value::numeric::string_to_float(digits) + delta as f64;
        php_value::numeric::float_to_string(f).into_bytes()
    }
}

/// `preg_replace_callback('/;O:(\d+):"/', fn ($m) => ';O:'.(9 + $m[1]).':"Tracking\\', $ser)`.
fn prefix_tracking(ser: &[u8]) -> Vec<u8> {
    let mut out = Vec::with_capacity(ser.len());
    let mut i = 0;
    while i < ser.len() {
        if ser[i..].starts_with(b";O:") {
            let digits = ser[i + 3..]
                .iter()
                .take_while(|c| c.is_ascii_digit())
                .count();
            let after = i + 3 + digits;
            if digits > 0 && ser[after..].starts_with(b":\"") {
                out.extend_from_slice(b";O:");
                out.extend_from_slice(&add_to_digits(&ser[i + 3..after], 9));
                out.extend_from_slice(b":\"Tracking\\");
                i = after + 2;
                continue;
            }
        }
        out.push(ser[i]);
        i += 1;
    }
    out
}

/// `preg_replace_callback('/s:(\d+):"(\0[^\0]++\0)/', fn ($m) => 's:'.($m[1] - \strlen($m[2])).':"', $ser)`.
fn cut_mangled_names(ser: &[u8]) -> Vec<u8> {
    let mut out = Vec::with_capacity(ser.len());
    let mut i = 0;
    while i < ser.len() {
        if ser[i..].starts_with(b"s:") {
            let digits = ser[i + 2..]
                .iter()
                .take_while(|c| c.is_ascii_digit())
                .count();
            let quote = i + 2 + digits;
            if digits > 0 && ser[quote..].starts_with(b":\"\0") {
                let name = quote + 3;
                let len = ser[name..].iter().take_while(|&&c| c != 0).count();
                if len > 0 && ser.get(name + len) == Some(&0) {
                    let cut = i64::try_from(len + 2).unwrap_or(i64::MAX);
                    out.extend_from_slice(b"s:");
                    out.extend_from_slice(&add_to_digits(&ser[i + 2..quote], -cut));
                    out.extend_from_slice(b":\"");
                    i = name + len + 1;
                    continue;
                }
            }
        }
        out.push(ser[i]);
        i += 1;
    }
    out
}

/// The `.json` meta file `write()` derives from the serialized resources.
pub fn tracking_json(ser: &[u8]) -> Vec<u8> {
    let ser = cut_mangled_names(&prefix_tracking(ser));
    let value =
        unserialize(&ser, &Classes::new(), &Registry::default()).unwrap_or(Value::Bool(false));
    let flags = JsonFlags {
        unescaped_slashes: true,
        unescaped_unicode: true,
    };
    // @json_encode(...) ?: [], then sprintf('%s') of that array
    let encoded = crate::php::json_encode_with(&value, flags, false)
        .filter(|j| !j.is_empty() && j != b"0")
        .unwrap_or_else(|| b"Array".to_vec());
    let encoded = replace(
        &encoded,
        b"\"__PHP_Incomplete_Class_Name\":\"Tracking\\\\",
        b"\"@type\":\"",
    );
    let mut out = b"{\"resources\":".to_vec();
    out.extend_from_slice(&encoded);
    out.push(b'}');
    out
}

fn replace(subject: &[u8], from: &[u8], to: &[u8]) -> Vec<u8> {
    let mut out = Vec::with_capacity(subject.len());
    let mut i = 0;
    while i < subject.len() {
        if subject[i..].starts_with(from) {
            out.extend_from_slice(to);
            i += from.len();
        } else {
            out.push(subject[i]);
            i += 1;
        }
    }
    out
}

/// The length of the first serialized value in `data`, or `None` when it
/// does not parse: a scan of `serialize()`'s grammar that builds nothing,
/// to tell data after the value (which PHP 8.3+ warns about) from the
/// end. `unserialize()` still decides what the value is.
pub fn serialized_length(data: &[u8]) -> Option<usize> {
    let mut pos = 0;
    scan_value(data, &mut pos, 0)?;
    Some(pos)
}

fn expect(data: &[u8], pos: &mut usize, bytes: &[u8]) -> Option<()> {
    data.get(*pos..)?
        .starts_with(bytes)
        .then(|| *pos += bytes.len())
}

/// `[+-]?\d+` then `end`; the number.
fn scan_int(data: &[u8], pos: &mut usize, end: u8) -> Option<i64> {
    let start = *pos;
    let mut i = start;
    if matches!(data.get(i), Some(b'+' | b'-')) {
        i += 1;
    }
    let digits = data
        .get(i..)?
        .iter()
        .take_while(|b| b.is_ascii_digit())
        .count();
    if digits == 0 || data.get(i + digits) != Some(&end) {
        return None;
    }
    *pos = i + digits + 1;
    Some(php_value::numeric::string_to_int(&data[start..i + digits]))
}

fn scan_len(data: &[u8], pos: &mut usize, end: u8) -> Option<usize> {
    usize::try_from(scan_int(data, pos, end)?).ok()
}

fn scan_quoted(data: &[u8], pos: &mut usize, len: usize) -> Option<()> {
    expect(data, pos, b"\"")?;
    data.get(*pos..*pos + len)?;
    *pos += len;
    expect(data, pos, b"\"")
}

fn scan_escaped(data: &[u8], pos: &mut usize, len: usize) -> Option<()> {
    expect(data, pos, b"\"")?;
    let mut n = 0;
    while n < len {
        if *data.get(*pos)? == b'\\' {
            let hex = data.get(*pos + 1..*pos + 3)?;
            if !hex.iter().all(u8::is_ascii_hexdigit) {
                return None;
            }
            *pos += 3;
        } else {
            *pos += 1;
        }
        n += 1;
    }
    expect(data, pos, b"\"")
}

fn scan_members(data: &[u8], pos: &mut usize, depth: usize) -> Option<()> {
    let count = scan_len(data, pos, b':')?;
    expect(data, pos, b"{")?;
    for _ in 0..count {
        let tag = *data.get(*pos)?;
        *pos += 1;
        expect(data, pos, b":")?;
        match tag {
            b'i' => {
                scan_int(data, pos, b';')?;
            }
            b's' => {
                let len = scan_len(data, pos, b':')?;
                scan_quoted(data, pos, len)?;
                expect(data, pos, b";")?;
            }
            b'S' => {
                let len = scan_len(data, pos, b':')?;
                scan_escaped(data, pos, len)?;
                expect(data, pos, b";")?;
            }
            _ => return None,
        }
        scan_value(data, pos, depth + 1)?;
    }
    expect(data, pos, b"}")
}

fn scan_value(data: &[u8], pos: &mut usize, depth: usize) -> Option<()> {
    if depth > 4096 {
        return None;
    }
    let tag = *data.get(*pos)?;
    *pos += 1;
    if tag == b'N' {
        return expect(data, pos, b";");
    }
    expect(data, pos, b":")?;
    match tag {
        b'b' => {
            if !matches!(data.get(*pos), Some(b'0' | b'1')) {
                return None;
            }
            *pos += 1;
            expect(data, pos, b";")
        }
        b'i' | b'r' | b'R' => scan_int(data, pos, b';').map(drop),
        b'd' => {
            let end = data.get(*pos..)?.iter().position(|&b| b == b';')?;
            *pos += end + 1;
            Some(())
        }
        b's' | b'E' => {
            let len = scan_len(data, pos, b':')?;
            scan_quoted(data, pos, len)?;
            expect(data, pos, b";")
        }
        b'S' => {
            let len = scan_len(data, pos, b':')?;
            scan_escaped(data, pos, len)?;
            expect(data, pos, b";")
        }
        b'a' => scan_members(data, pos, depth),
        b'O' => {
            let len = scan_len(data, pos, b':')?;
            scan_quoted(data, pos, len)?;
            expect(data, pos, b":")?;
            scan_members(data, pos, depth)
        }
        b'C' => {
            let len = scan_len(data, pos, b':')?;
            scan_quoted(data, pos, len)?;
            expect(data, pos, b":")?;
            let len = scan_len(data, pos, b':')?;
            expect(data, pos, b"{")?;
            data.get(*pos..*pos + len)?;
            *pos += len;
            expect(data, pos, b"}")
        }
        _ => None,
    }
}

/// `serialize($metadata)`: the meta file's content. Each resource is
/// `O:<len>:"<class>":<n>:{...}` with its `__serialize()` array, whose
/// keys keep their type (`i:0;` for an int key; php-value's `serialize()`
/// writes an object's property names as strings, as for an object
/// without `__serialize()`).
pub fn serialize_resources(metadata: &[Arc<dyn ResourceInterface>]) -> Result<PhpString, Error> {
    let mut out = format!("a:{}:{{", metadata.len()).into_bytes();
    for (i, resource) in metadata.iter().enumerate() {
        out.extend_from_slice(format!("i:{i};").as_bytes());
        let class = resource.class();
        let data = resource.serialize()?;
        out.extend_from_slice(
            format!("O:{}:\"{class}\":{}:{{", class.len(), data.len()).as_bytes(),
        );
        for (key, value) in data.iter() {
            out.extend_from_slice(serialize(&Value::from(key.clone())).as_bytes());
            out.extend_from_slice(serialize(value).as_bytes());
        }
        out.push(b'}');
    }
    out.push(b'}');
    Ok(PhpString::from(out))
}
