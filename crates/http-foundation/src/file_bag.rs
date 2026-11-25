//! Port of Symfony\Component\HttpFoundation\FileBag.php: a `ParameterBag` of
//! uploaded files, converting `$_FILES` arrays into [`UploadedFile`]
//! objects (and the array-named fields' "PHP bug" layout into nested
//! arrays).
//!
//! Values PHP coerces into `new UploadedFile()`'s parameters are coerced
//! the same way ([`crate::php`]); one PHP rejects is its `TypeError`
//! (without the "called in" suffix). Not reproduced, so refused
//! (`Class::NotPorted`): a sibling field read as a string or an object
//! where the `name` field is an array (PHP's string offsets), and arrays
//! nested deeper than 512 levels.
//!
//! Symfony source copyright (c) Fabien Potencier; distributed under MIT.
//!
//! Upstream: symfony/http-foundation FileBag.php

use std::{ops::Deref, sync::Arc};

use php_value::{Key, PhpArray, Value};

use crate::{
    exception::{Class, Error, Result},
    parameter_bag::ParameterBag,
    php::{self, UPLOAD_ERR_NO_FILE},
    uploaded_file::UploadedFile,
};

/// `FileBag::FILE_KEYS`.
const FILE_KEYS: [&str; 6] = ["error", "full_path", "name", "size", "tmp_name", "type"];

const MAX_DEPTH: usize = 512;

/// `new FileBag($parameters)`. It reads as a `ParameterBag`; changes go
/// through its own `set()`, `add()` and `replace()`.
#[derive(Debug, Clone, Default)]
pub struct FileBag(ParameterBag);

impl Deref for FileBag {
    type Target = ParameterBag;

    fn deref(&self) -> &ParameterBag {
        &self.0
    }
}

/// The `UploadedFile` a value holds, if any.
pub fn uploaded_file(value: &Value) -> Option<&UploadedFile> {
    match value {
        Value::Object(object) => object.downcast_ref::<UploadedFile>(),
        _ => None,
    }
}

/// `array_keys($data + ['full_path' => null])`, sorted, is `FILE_KEYS`.
fn has_file_keys(data: &PhpArray) -> bool {
    let all = FILE_KEYS
        .iter()
        .all(|k| *k == "full_path" || data.contains_key(&Key::from(*k)));
    let only = data.keys().all(|k| match k {
        Key::Str(s) => FILE_KEYS.iter().any(|f| f.as_bytes() == s.as_bytes()),
        Key::Int(_) => false,
    });
    all && only
}

fn field<'a>(data: &'a PhpArray, key: &str) -> Option<&'a Value> {
    data.get(&Key::from(key))
}

/// PHP's boolean conversion.
fn truthy(value: &Value) -> bool {
    match value {
        Value::Null => false,
        Value::Bool(b) => *b,
        Value::Int(i) => *i != 0,
        Value::Float(f) => *f != 0.0,
        Value::String(s) => !(s.is_empty() || s.as_bytes() == b"0"),
        Value::Array(a) => !a.is_empty(),
        Value::Object(_) => true,
    }
}

/// `$data[$field][$key]` where `$data['name']` is an array: a missing
/// key, or a null or scalar non-string field, reads as null (PHP warns).
fn sibling(data: &PhpArray, name: &str, key: &Key) -> Result<Value> {
    match field(data, name) {
        Some(Value::Array(values)) => Ok(values.get(key).cloned().unwrap_or(Value::Null)),
        None | Some(Value::Null | Value::Bool(_) | Value::Int(_) | Value::Float(_)) => {
            Ok(Value::Null)
        }
        Some(other) => Err(Error::new(
            Class::NotPorted,
            format!("FileBag: \"{name}\" offset of a {}", other.debug_type()),
        )),
    }
}

/// `isset($data['full_path'][$key])`: the value, if set and not null.
fn full_path_of(data: &PhpArray, key: &Key) -> Result<Option<Value>> {
    match sibling(data, "full_path", key)? {
        Value::Null => Ok(None),
        value => Ok(Some(value)),
    }
}

/// `fixPhpFilesArray($data)`.
fn fix_php_files_array(data: &PhpArray, depth: usize) -> Result<PhpArray> {
    if depth > MAX_DEPTH {
        return Err(Error::new(
            Class::NotPorted,
            "FileBag: nesting deeper than 512",
        ));
    }
    let Some(Value::Array(names)) = field(data, "name") else {
        return Ok(data.clone());
    };
    if !has_file_keys(data) {
        return Ok(data.clone());
    }
    // every key is one of FILE_KEYS, so unsetting them leaves nothing
    let mut files = PhpArray::new();
    for (key, name) in names.iter() {
        let mut entry = PhpArray::new();
        entry.insert("error", sibling(data, "error", key)?);
        entry.insert("name", name.clone());
        entry.insert("type", sibling(data, "type", key)?);
        entry.insert("tmp_name", sibling(data, "tmp_name", key)?);
        entry.insert("size", sibling(data, "size", key)?);
        if let Some(full_path) = full_path_of(data, key)? {
            entry.insert("full_path", full_path);
        }
        files.insert(
            key.clone(),
            Value::Array(fix_php_files_array(&entry, depth + 1)?),
        );
    }
    Ok(files)
}

/// `convertFileInformation($file)` of an array.
fn convert_array(file: &PhpArray, depth: usize) -> Result<Value> {
    if depth > MAX_DEPTH {
        return Err(Error::new(
            Class::NotPorted,
            "FileBag: nesting deeper than 512",
        ));
    }
    let file = fix_php_files_array(file, depth)?;
    if has_file_keys(&file) {
        let error = field(&file, "error").cloned().unwrap_or(Value::Null);
        if error.identical(&Value::Int(UPLOAD_ERR_NO_FILE)) {
            return Ok(Value::Null);
        }
        let tmp_name = field(&file, "tmp_name").cloned().unwrap_or(Value::Null);
        let original_name = match field(&file, "full_path") {
            Some(v) if !v.is_null() => v.clone(),
            _ => field(&file, "name").cloned().unwrap_or(Value::Null),
        };
        let mime_type = field(&file, "type").cloned().unwrap_or(Value::Null);
        let path = php::string_argument(1, "path", &tmp_name)?;
        let original_name = php::string_argument(2, "originalName", &original_name)?;
        let mime_type = php::nullable_string_argument(3, "mimeType", &mime_type)?;
        let error = php::nullable_int_argument(4, "error", &error)?;
        let uploaded = UploadedFile::new(
            path.as_bytes().to_vec(),
            original_name,
            mime_type,
            error,
            false,
        )?;
        return Ok(Value::Object(Arc::new(uploaded)));
    }
    let mut mapped = PhpArray::new();
    for (key, value) in file.iter() {
        let value = match value {
            Value::Array(inner) => convert_array(inner, depth + 1)?,
            other => other.clone(),
        };
        mapped.insert(key.clone(), value);
    }
    if mapped.is_list() {
        let mut filtered = PhpArray::new();
        for (key, value) in mapped.iter() {
            if truthy(value) {
                filtered.insert(key.clone(), value.clone());
            }
        }
        mapped = filtered;
    }
    Ok(Value::Array(mapped))
}

impl FileBag {
    /// `new FileBag($parameters)`.
    pub fn new(parameters: PhpArray) -> Result<Self> {
        let mut bag = Self::default();
        bag.replace(parameters)?;
        Ok(bag)
    }

    /// `replace($files)`: empties the bag, then adds the files.
    pub fn replace(&mut self, files: PhpArray) -> Result<()> {
        self.0 = ParameterBag::default();
        self.add(files)
    }

    /// `set($key, $value)`: an array (converted) or an `UploadedFile`; any
    /// other value is PHP's `InvalidArgumentException`.
    pub fn set(&mut self, key: &str, value: Value) -> Result<()> {
        let converted = match value {
            Value::Array(array) => convert_array(&array, 0)?,
            value if uploaded_file(&value).is_some() => value,
            _ => {
                return Err(Error::new(
                    Class::PhpInvalidArgument,
                    "An uploaded file must be an array or an instance of UploadedFile.",
                ));
            }
        };
        self.0.set(key, converted);
        Ok(())
    }

    /// `add($files)`.
    #[expect(clippy::needless_pass_by_value, reason = "upstream's signature")]
    pub fn add(&mut self, files: PhpArray) -> Result<()> {
        for (key, file) in files.iter() {
            let key = match key {
                Key::Int(i) => i.to_string(),
                Key::Str(s) => s
                    .to_str()
                    .ok_or_else(|| {
                        Error::new(Class::NotPorted, "FileBag: a key that is not UTF-8")
                    })?
                    .to_owned(),
            };
            self.set(&key, file.clone())?;
        }
        Ok(())
    }

    /// `remove($key)`.
    pub fn remove(&mut self, key: &str) {
        self.0.remove(key);
    }
}
