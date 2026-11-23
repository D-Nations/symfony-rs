//! Port of Symfony\Component\Translation\MessageCatalogue.php, with
//! MessageCatalogueInterface.php (`INTL_DOMAIN_SUFFIX`),
//! MetadataAwareInterface.php and CatalogueMetadataAwareInterface.php.
//!
//! Messages, metadata and catalogue metadata are PHP arrays
//! ([`PhpArray`]): a domain or id that is a decimal integer string is an
//! integer key, and the order of keys is PHP's. A catalogue is a shared
//! handle, as a PHP object is: a fallback catalogue knows its parent and
//! adds its resources to it. The parent link is weak (PHP's is a strong
//! reference); a parent nobody holds any more can no longer be seen.
//!
//! Resources are symfony/config's [`ResourceInterface`] objects, keyed by
//! their `__toString()`.
//!
//! Symfony source copyright (c) Fabien Potencier; distributed under MIT.
//!
//! Upstream: symfony/translation MessageCatalogue.php MessageCatalogueInterface.php MetadataAwareInterface.php CatalogueMetadataAwareInterface.php

use std::cell::RefCell;
use std::fmt;
use std::rc::{Rc, Weak};

use php_value::{Key, PhpArray, PhpString, Value};

use symfony_config::resource::ResourceInterface;

use crate::exception::{Class, Error};

/// `MessageCatalogueInterface::INTL_DOMAIN_SUFFIX`.
pub const INTL_DOMAIN_SUFFIX: &str = "+intl-icu";

/// `MessageCatalogue`.
#[derive(Clone)]
pub struct MessageCatalogue(Rc<RefCell<Inner>>);

struct Inner {
    locale: PhpString,
    messages: PhpArray,
    metadata: PhpArray,
    catalogue_metadata: PhpArray,
    resources: Vec<(Vec<u8>, Rc<dyn ResourceInterface>)>,
    fallback: Option<MessageCatalogue>,
    parent: Option<Weak<RefCell<Inner>>>,
}

impl fmt::Debug for MessageCatalogue {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let inner = self.0.borrow();
        f.debug_struct("MessageCatalogue")
            .field("locale", &inner.locale)
            .field("messages", &inner.messages)
            .field("metadata", &inner.metadata)
            .field("catalogue_metadata", &inner.catalogue_metadata)
            .field("resources", &inner.resources)
            .finish_non_exhaustive()
    }
}

/// A key as the string PHP's `foreach` gives to string functions.
fn key_bytes(key: &Key) -> Vec<u8> {
    match key {
        Key::Int(i) => i.to_string().into_bytes(),
        Key::Str(s) => s.as_bytes().to_vec(),
    }
}

fn with_suffix(domain: &[u8]) -> Vec<u8> {
    let mut d = domain.to_vec();
    d.extend_from_slice(INTL_DOMAIN_SUFFIX.as_bytes());
    d
}

fn strip_suffix(domain: &[u8]) -> Option<&[u8]> {
    domain.strip_suffix(INTL_DOMAIN_SUFFIX.as_bytes())
}

fn array_at(array: &PhpArray, key: &[u8]) -> Option<PhpArray> {
    array
        .get(&Key::from_bytes(key))
        .and_then(Value::as_array)
        .cloned()
}

/// `isset($array[$a][$b])`.
fn isset2<'a>(array: &'a PhpArray, a: &[u8], b: &[u8]) -> Option<&'a Value> {
    array
        .get(&Key::from_bytes(a))
        .and_then(Value::as_array)
        .and_then(|inner| inner.get(&Key::from_bytes(b)))
        .filter(|v| !v.is_null())
}

/// `$array[$a][$b] = $value`.
fn set2(array: &mut PhpArray, a: Key, b: Key, value: Value) {
    let mut inner = array
        .get(&a)
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default();
    inner.insert(b, value);
    array.insert(a, Value::Array(inner));
}

/// `!$value` for a string.
fn falsy(s: &[u8]) -> bool {
    s.is_empty() || s == b"0"
}

/// A message as a string (`get()`'s `string` return type).
fn message_string(value: &Value) -> PhpString {
    value
        .to_php_string()
        .unwrap_or_else(|| PhpString::from(&b""[..]))
}

impl MessageCatalogue {
    /// `new MessageCatalogue($locale, $messages)`: messages by domain.
    pub fn new(locale: impl Into<PhpString>, messages: PhpArray) -> Self {
        Self(Rc::new(RefCell::new(Inner {
            locale: locale.into(),
            messages,
            metadata: PhpArray::new(),
            catalogue_metadata: PhpArray::new(),
            resources: Vec::new(),
            fallback: None,
            parent: None,
        })))
    }

    pub fn get_locale(&self) -> PhpString {
        self.0.borrow().locale.clone()
    }

    /// `getDomains()`: the domains, `+intl-icu` removed, once each.
    pub fn get_domains(&self) -> PhpArray {
        let inner = self.0.borrow();
        let mut domains = PhpArray::new();
        for (domain, _) in inner.messages.iter() {
            let bytes = key_bytes(domain);
            match strip_suffix(&bytes) {
                Some(base) => domains.insert(Key::from_bytes(base), Value::string(base)),
                None => domains.insert(domain.clone(), domain.to_value()),
            }
        }
        PhpArray::list(domains.values().cloned())
    }

    /// `all($domain)`: one domain's messages (its `+intl-icu` messages
    /// first), or every domain's.
    pub fn all(&self, domain: Option<&[u8]>) -> PhpArray {
        let inner = self.0.borrow();
        if let Some(domain) = domain {
            if strip_suffix(domain).is_some() {
                return array_at(&inner.messages, domain).unwrap_or_default();
            }
            let mut all = array_at(&inner.messages, &with_suffix(domain)).unwrap_or_default();
            all.union(&array_at(&inner.messages, domain).unwrap_or_default());
            return all;
        }

        let mut all = PhpArray::new();
        for (domain, messages) in inner.messages.iter() {
            let messages = messages.as_array().cloned().unwrap_or_default();
            let bytes = key_bytes(domain);
            match strip_suffix(&bytes) {
                Some(base) => {
                    let key = Key::from_bytes(base);
                    let mut merged = messages;
                    if let Some(existing) = all.get(&key).and_then(Value::as_array) {
                        merged.union(existing);
                    }
                    all.insert(key, Value::Array(merged));
                }
                None => {
                    let mut merged = all
                        .get(domain)
                        .and_then(Value::as_array)
                        .cloned()
                        .unwrap_or_default();
                    merged.union(&messages);
                    all.insert(domain.clone(), Value::Array(merged));
                }
            }
        }
        all
    }

    pub fn set(&self, id: &[u8], translation: &[u8], domain: &[u8]) {
        let mut messages = PhpArray::new();
        messages.insert(Key::from_bytes(id), Value::string(translation));
        self.add(&messages, domain);
    }

    pub fn has(&self, id: &[u8], domain: &[u8]) -> bool {
        if self.defines(id, domain) {
            return true;
        }
        let fallback = self.0.borrow().fallback.clone();
        match fallback {
            Some(fallback) => fallback.has(id, domain),
            None => false,
        }
    }

    pub fn defines(&self, id: &[u8], domain: &[u8]) -> bool {
        let inner = self.0.borrow();
        isset2(&inner.messages, domain, id).is_some()
            || isset2(&inner.messages, &with_suffix(domain), id).is_some()
    }

    /// `get($id, $domain)`: the translation, the fallback catalogue's, or
    /// the id.
    pub fn get(&self, id: &[u8], domain: &[u8]) -> PhpString {
        let fallback = {
            let inner = self.0.borrow();
            if let Some(m) = isset2(&inner.messages, &with_suffix(domain), id) {
                return message_string(m);
            }
            if let Some(m) = isset2(&inner.messages, domain, id) {
                return message_string(m);
            }
            inner.fallback.clone()
        };
        match fallback {
            Some(fallback) => fallback.get(id, domain),
            None => PhpString::from(id),
        }
    }

    pub fn replace(&self, messages: &PhpArray, domain: &[u8]) {
        {
            let mut inner = self.0.borrow_mut();
            inner.messages.remove(&Key::from_bytes(domain));
            inner
                .messages
                .remove(&Key::from_bytes(&with_suffix(domain)));
        }
        self.add(messages, domain);
    }

    pub fn add(&self, messages: &PhpArray, domain: &[u8]) {
        let alt = match strip_suffix(domain) {
            Some(base) => base.to_vec(),
            None => with_suffix(domain),
        };
        let alt_key = Key::from_bytes(&alt);
        let domain_key = Key::from_bytes(domain);
        let mut inner = self.0.borrow_mut();
        for (id, message) in messages.iter() {
            if let Some(mut alt_messages) = inner
                .messages
                .get(&alt_key)
                .and_then(Value::as_array)
                .cloned()
            {
                alt_messages.remove(id);
                inner
                    .messages
                    .insert(alt_key.clone(), Value::Array(alt_messages));
            }
            set2(
                &mut inner.messages,
                domain_key.clone(),
                id.clone(),
                message.clone(),
            );
        }
        if inner
            .messages
            .get(&alt_key)
            .and_then(Value::as_array)
            .is_some_and(PhpArray::is_empty)
        {
            inner.messages.remove(&alt_key);
        }
    }

    pub fn add_catalogue(&self, catalogue: &MessageCatalogue) -> Result<(), Error> {
        let (locale, other) = (self.get_locale(), catalogue.get_locale());
        if other != locale {
            let mut message = b"Cannot add a catalogue for locale \"".to_vec();
            message.extend_from_slice(other.as_bytes());
            message.extend_from_slice(b"\" as the current locale for this catalogue is \"");
            message.extend_from_slice(locale.as_bytes());
            message.extend_from_slice(b"\".");
            return Err(Error::new(Class::Logic, message));
        }

        for (domain, messages) in catalogue.all(None).iter() {
            let domain = key_bytes(domain);
            let mut messages = messages.as_array().cloned().unwrap_or_default();
            let intl_domain = with_suffix(&domain);
            let intl = catalogue.all(Some(&intl_domain));
            if !intl.is_empty() {
                self.add(&intl, &intl_domain);
                let keys: Vec<Key> = intl.keys().cloned().collect();
                for key in &keys {
                    messages.remove(key);
                }
            }
            self.add(&messages, &domain);
        }

        for resource in catalogue.get_resources() {
            self.add_resource(resource);
        }

        let metadata = catalogue.0.borrow().metadata.clone();
        self.merge(&metadata, false);
        let catalogue_metadata = catalogue.0.borrow().catalogue_metadata.clone();
        self.merge(&catalogue_metadata, true);
        Ok(())
    }

    /// `addMetadata()` / `addCatalogueMetadata()`.
    fn merge(&self, values: &PhpArray, catalogue_metadata: bool) {
        let mut inner = self.0.borrow_mut();
        let target = if catalogue_metadata {
            &mut inner.catalogue_metadata
        } else {
            &mut inner.metadata
        };
        for (domain, keys) in values.iter() {
            let Some(keys) = keys.as_array() else {
                continue;
            };
            for (key, value) in keys.iter() {
                set2(target, domain.clone(), key.clone(), value.clone());
            }
        }
    }

    pub fn add_fallback_catalogue(&self, catalogue: &MessageCatalogue) -> Result<(), Error> {
        let circular = || {
            let mut message =
                b"Circular reference detected when adding a fallback catalogue for locale \""
                    .to_vec();
            message.extend_from_slice(catalogue.get_locale().as_bytes());
            message.extend_from_slice(b"\".");
            Error::new(Class::Logic, message)
        };

        let locale = self.get_locale();
        let mut c = catalogue.get_fallback_catalogue();
        while let Some(current) = c {
            if current.get_locale() == locale {
                return Err(circular());
            }
            c = current.get_fallback_catalogue();
        }

        let mut c = Some(self.clone());
        while let Some(current) = c {
            if current.get_locale() == catalogue.get_locale() {
                return Err(circular());
            }
            for resource in catalogue.get_resources() {
                current.add_resource(resource);
            }
            let parent = current.0.borrow().parent.as_ref().and_then(Weak::upgrade);
            c = parent.map(MessageCatalogue);
        }

        catalogue.0.borrow_mut().parent = Some(Rc::downgrade(&self.0));
        self.0.borrow_mut().fallback = Some(catalogue.clone());

        for resource in catalogue.get_resources() {
            self.add_resource(resource);
        }
        Ok(())
    }

    pub fn get_fallback_catalogue(&self) -> Option<MessageCatalogue> {
        self.0.borrow().fallback.clone()
    }

    pub fn get_resources(&self) -> Vec<Rc<dyn ResourceInterface>> {
        self.0
            .borrow()
            .resources
            .iter()
            .map(|(_, r)| Rc::clone(r))
            .collect()
    }

    /// `addResource()`: keyed by `__toString()`; adding one with the same
    /// text replaces it in place.
    pub fn add_resource(&self, resource: Rc<dyn ResourceInterface>) {
        let key = resource.to_string();
        let mut inner = self.0.borrow_mut();
        match inner.resources.iter_mut().find(|(k, _)| *k == key) {
            Some(entry) => entry.1 = resource,
            None => inner.resources.push((key, resource)),
        }
    }

    /// `getMetadata($key, $domain)`: everything for an empty domain, a
    /// domain's metadata for an empty key (its `+intl-icu` domain's first),
    /// else one value; `null` when absent.
    pub fn get_metadata(&self, key: &[u8], domain: &[u8]) -> Value {
        let inner = self.0.borrow();
        if domain.is_empty() {
            return Value::Array(inner.metadata.clone());
        }
        for d in [with_suffix(domain), domain.to_vec()] {
            if let Some(values) = inner
                .metadata
                .get(&Key::from_bytes(&d))
                .filter(|v| !v.is_null())
            {
                if key.is_empty() {
                    return values.clone();
                }
                if let Some(value) = isset2(&inner.metadata, &d, key) {
                    return value.clone();
                }
            }
        }
        Value::Null
    }

    pub fn set_metadata(&self, key: &[u8], value: Value, domain: &[u8]) {
        let mut inner = self.0.borrow_mut();
        set2(
            &mut inner.metadata,
            Key::from_bytes(domain),
            Key::from_bytes(key),
            value,
        );
    }

    pub fn delete_metadata(&self, key: &[u8], domain: &[u8]) {
        let mut inner = self.0.borrow_mut();
        delete(&mut inner.metadata, key, domain, |s| s.is_empty());
    }

    /// `getCatalogueMetadata($key, $domain)`; a domain or key of `"0"` counts
    /// as empty (`!$domain`).
    pub fn get_catalogue_metadata(&self, key: &[u8], domain: &[u8]) -> Value {
        let inner = self.0.borrow();
        if falsy(domain) {
            return Value::Array(inner.catalogue_metadata.clone());
        }
        if let Some(values) = inner
            .catalogue_metadata
            .get(&Key::from_bytes(domain))
            .filter(|v| !v.is_null())
        {
            if falsy(key) {
                return values.clone();
            }
            if let Some(value) = isset2(&inner.catalogue_metadata, domain, key) {
                return value.clone();
            }
        }
        Value::Null
    }

    pub fn set_catalogue_metadata(&self, key: &[u8], value: Value, domain: &[u8]) {
        let mut inner = self.0.borrow_mut();
        set2(
            &mut inner.catalogue_metadata,
            Key::from_bytes(domain),
            Key::from_bytes(key),
            value,
        );
    }

    pub fn delete_catalogue_metadata(&self, key: &[u8], domain: &[u8]) {
        let mut inner = self.0.borrow_mut();
        delete(&mut inner.catalogue_metadata, key, domain, falsy);
    }

    /// The raw arrays, as the private properties hold them (messages by
    /// domain with `+intl-icu` domains apart, metadata, catalogue
    /// metadata).
    pub fn raw(&self) -> (PhpArray, PhpArray, PhpArray) {
        let inner = self.0.borrow();
        (
            inner.messages.clone(),
            inner.metadata.clone(),
            inner.catalogue_metadata.clone(),
        )
    }
}

fn delete(array: &mut PhpArray, key: &[u8], domain: &[u8], empty: fn(&[u8]) -> bool) {
    if empty(domain) {
        *array = PhpArray::new();
    } else if empty(key) {
        array.remove(&Key::from_bytes(domain));
    } else {
        let domain_key = Key::from_bytes(domain);
        if let Some(mut inner) = array.get(&domain_key).and_then(Value::as_array).cloned() {
            inner.remove(&Key::from_bytes(key));
            array.insert(domain_key, Value::Array(inner));
        }
    }
}
