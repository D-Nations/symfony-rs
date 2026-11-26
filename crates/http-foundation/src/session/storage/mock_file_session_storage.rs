//! Port of Symfony\Component\HttpFoundation\Session\Storage\MockFileSessionStorage.php:
//! a `MockArraySessionStorage` that keeps each session in a file,
//! `<savePath>/<id>.mocksess`, as `serialize()` writes it.
//!
//! `save()` leaves out empty bags, and the metadata when it is all that is
//! left; with nothing left, it deletes the file. An id that is not a usable
//! file name (`[a-zA-Z0-9,-]{1,246}`) is ignored, which starts a new
//! session. Reading a file that `unserialize()` makes into something other
//! than an array fails as PHP's `array` property does.
//!
//! `unserialize($data, ['allowed_classes' => true])` creates `stdClass`
//! objects; objects of other classes come back as
//! `__PHP_Incomplete_Class` (php-value creates only the classes it is
//! told of).
//!
//! Symfony source copyright (c) Fabien Potencier; distributed under MIT.
//!
//! Upstream: symfony/http-foundation Session/Storage/MockFileSessionStorage.php

use std::sync::Arc;

use php_value::{
    Key, Value,
    constants::Registry,
    serialize::{Classes, serialize, unserialize},
};
use symfony_filesystem::php as fs_php;
use symfony_finder::fs;

use super::{
    metadata_bag::MetadataBag,
    mock_array_session_storage::{MockArraySessionStorage, random_hex},
    session_storage_interface::SessionStorageInterface,
};
use crate::{
    exception::{Class, Error, Result},
    session::{Data, session_bag_interface::SessionBagInterface},
};

/// `new MockFileSessionStorage($savePath, $name, $metaBag)`.
#[derive(Debug)]
pub struct MockFileSessionStorage {
    base: MockArraySessionStorage,
    save_path: Vec<u8>,
}

/// `preg_match('/^[a-zA-Z0-9,-]{1,246}$/D', $id)`.
fn usable_id(id: &str) -> bool {
    (1..=246).contains(&id.len())
        && id
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b == b',' || b == b'-')
}

impl MockFileSessionStorage {
    /// Fails with `\RuntimeException` when the directory cannot be
    /// created.
    pub fn new(
        save_path: Option<Vec<u8>>,
        name: impl Into<String>,
        meta_bag: Option<Arc<MetadataBag>>,
    ) -> Result<Self> {
        let save_path = save_path.unwrap_or_else(fs_php::sys_get_temp_dir);
        if !fs::is_dir(&save_path)
            && !fs_php::mkdir_recursive(&save_path, 0o777).value
            && !fs::is_dir(&save_path)
        {
            return Err(Error::new(
                Class::PhpRuntime,
                format!(
                    "Session Storage was not able to create directory \"{}\".",
                    String::from_utf8_lossy(&save_path)
                ),
            ));
        }
        Ok(Self {
            base: MockArraySessionStorage::new(name, meta_bag),
            save_path,
        })
    }

    /// `setSessionData($array)`.
    pub fn set_session_data(&self, array: &php_value::PhpArray) {
        self.base.set_session_data(array);
    }

    /// `setMetadataBag($bag)`.
    pub fn set_metadata_bag(&self, bag: Option<Arc<MetadataBag>>) {
        self.base.set_metadata_bag(bag);
    }

    /// `getFilePath()`.
    fn get_file_path(&self) -> Vec<u8> {
        let id = self.base.state().id.clone();
        [&self.save_path[..], b"/", id.as_bytes(), b".mocksess"].concat()
    }

    /// `destroy()`: the file removed, failures ignored.
    fn destroy(&self) {
        fs_php::unlink(&self.get_file_path());
    }

    /// `read()`: the file's data (none when it cannot be read or is
    /// empty), then the bags loaded.
    fn read(&self) -> Result<()> {
        let contents = fs_php::file_get_contents(&self.get_file_path()).unwrap_or_default();
        let data = if Value::String(contents.clone().into()).is_truthy() {
            let mut classes = Classes::new();
            classes.allow("stdClass");
            match unserialize(&contents, &classes, &Registry::default()) {
                Some(Value::Array(array)) => Data::from_array(&array),
                other => {
                    let given = other.map_or_else(|| "bool".to_owned(), |v| v.debug_type());
                    return Err(Error::new(
                        Class::PhpTypeError,
                        format!(
                            "Cannot assign {given} to property {}::$data of type array",
                            super::mock_array_session_storage::CLASS
                        ),
                    ));
                }
            }
        } else {
            Data::default()
        };
        self.base.state().data = data;
        self.base.load_session()
    }
}

impl SessionStorageInterface for MockFileSessionStorage {
    fn start(&self) -> Result<bool> {
        if self.base.state().started {
            return Ok(true);
        }
        self.base.ensure_id()?;
        self.read()?;
        self.base.state().started = true;
        Ok(true)
    }

    fn is_started(&self) -> bool {
        self.base.is_started()
    }

    fn get_id(&self) -> String {
        self.base.get_id()
    }

    fn set_id(&self, id: &str) -> Result<()> {
        // the id is turned into a file name, so keep it to the charset PHP allows for session ids
        // and to the 255 bytes a file name can hold once the ".mocksess" suffix is added
        let id = if !id.is_empty() && !usable_id(id) {
            ""
        } else {
            id
        };
        self.base.set_id_array(id)
    }

    fn get_name(&self) -> String {
        self.base.get_name()
    }

    fn set_name(&self, name: &str) -> Result<()> {
        self.base.set_name(name)
    }

    fn regenerate(&self, destroy: bool, lifetime: Option<i64>) -> Result<bool> {
        if !self.base.state().started {
            self.start()?;
        }
        if destroy {
            self.destroy();
        }
        self.base.regenerate_with(lifetime, || self.start())
    }

    fn save(&self) -> Result<()> {
        if !self.base.state().started {
            return Err(Error::new(
                Class::PhpRuntime,
                "Trying to save a session that was not started yet or was already closed.",
            ));
        }
        let (mut data, bag_keys, meta_key) = {
            let state = self.base.state();
            let bag_keys: Vec<String> = state
                .bags
                .iter()
                .map(|(_, b)| b.get_storage_key())
                .collect();
            (
                state.data.clone(),
                bag_keys,
                state.metadata_bag.get_storage_key(),
            )
        };
        for key in bag_keys {
            if !data.get(&key).is_some_and(|v| v.is_truthy()) {
                data.remove(&key);
            }
        }
        let meta = Key::from_bytes(meta_key.as_bytes());
        if data.keys() == [meta.clone()] {
            data.remove(&meta_key);
        }

        let written = if data.is_empty() {
            self.destroy();
            Ok(())
        } else {
            let path = self.get_file_path();
            random_hex(6).map(|suffix| {
                let tmp = [&path[..], suffix.as_bytes()].concat();
                fs_php::file_put_contents(
                    &tmp,
                    serialize(&Value::Array(data.to_array())).as_bytes(),
                );
                fs_php::rename(&tmp, &path);
            })
        };
        self.base.state().data = data;
        written?;

        // this is needed when the session object is reused across multiple requests
        // in functional tests.
        self.base.state().started = false;
        Ok(())
    }

    fn clear(&self) -> Result<()> {
        self.base.clear()
    }

    fn get_bag(&self, name: &str) -> Result<Arc<dyn SessionBagInterface>> {
        self.base.get_bag_with(name, || self.start())
    }

    fn register_bag(&self, bag: Arc<dyn SessionBagInterface>) -> Result<()> {
        self.base.register_bag(bag)
    }

    fn get_metadata_bag(&self) -> Arc<MetadataBag> {
        self.base.get_metadata_bag()
    }
}
