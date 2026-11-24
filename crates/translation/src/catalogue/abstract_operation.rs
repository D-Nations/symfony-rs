//! Port of Symfony\Component\Translation\Catalogue\AbstractOperation.php.
//!
//! Symfony source copyright (c) Fabien Potencier; distributed under MIT.
//!
//! Upstream: symfony/translation Catalogue/AbstractOperation.php

use php_value::{Key, PhpArray, Value};

use crate::{Class, Error, INTL_DOMAIN_SUFFIX, MessageCatalogue};

/// The three upstream *_BATCH constants.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Batch {
    #[default]
    All,
    New,
    Obsolete,
}

impl Batch {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::All => "all",
            Self::New => "new",
            Self::Obsolete => "obsolete",
        }
    }
}

impl TryFrom<&[u8]> for Batch {
    type Error = Error;

    fn try_from(value: &[u8]) -> Result<Self, Error> {
        [Self::All, Self::New, Self::Obsolete]
            .into_iter()
            .find(|batch| batch.as_str().as_bytes() == value)
            .ok_or_else(|| {
                Error::new(
                    Class::NativeInvalidArgument,
                    format!(
                        "$batch argument must be one of [\"{}\", \"{}\", \"{}\"].",
                        Self::All.as_str(),
                        Self::New.as_str(),
                        Self::Obsolete.as_str()
                    )
                    .into_bytes(),
                )
            })
    }
}

#[derive(Debug, Default)]
pub(super) struct Messages {
    pub all: PhpArray,
    pub new: PhpArray,
    pub obsolete: PhpArray,
}

impl Messages {
    fn batch(&self, batch: Batch) -> PhpArray {
        match batch {
            Batch::All => self.all.clone(),
            Batch::New => self.new.clone(),
            Batch::Obsolete => self.obsolete.clone(),
        }
    }
}

pub(super) type Processor =
    fn(&MessageCatalogue, &MessageCatalogue, &MessageCatalogue, &[u8]) -> Messages;

/// Shared state. Construct the concrete MergeOperation or TargetOperation.
/// Inputs and result are shared handles; returned arrays are value copies.
#[derive(Debug)]
pub struct AbstractOperation {
    source: MessageCatalogue,
    target: MessageCatalogue,
    result: MessageCatalogue,
    domains: Option<PhpArray>,
    messages: Vec<(Vec<u8>, Messages)>,
    process: Processor,
}

pub(super) fn key_bytes(key: &Key) -> Vec<u8> {
    match key {
        Key::Int(i) => i.to_string().into_bytes(),
        Key::Str(s) => s.as_bytes().to_vec(),
    }
}

pub(super) fn intl_domain(domain: &[u8]) -> Vec<u8> {
    [domain, INTL_DOMAIN_SUFFIX.as_bytes()].concat()
}

impl AbstractOperation {
    pub(super) fn new(
        source: MessageCatalogue,
        target: MessageCatalogue,
        process: Processor,
    ) -> Result<Self, Error> {
        if source.get_locale() != target.get_locale() {
            return Err(Error::new(
                Class::Logic,
                b"Operated catalogues must belong to the same locale.",
            ));
        }
        let result = MessageCatalogue::new(source.get_locale(), PhpArray::new());
        Ok(Self {
            source,
            target,
            result,
            domains: None,
            messages: Vec::new(),
            process,
        })
    }

    pub fn get_domains(&mut self) -> PhpArray {
        if let Some(domains) = &self.domains {
            return domains.clone();
        }
        let mut domains = PhpArray::new();
        for catalogue in [&self.source, &self.target] {
            for domain in catalogue.get_domains().values() {
                // getDomains() can return integer values for numeric domains.
                let key = match domain {
                    Value::Int(i) => Key::Int(*i),
                    Value::String(s) => Key::from_bytes(s.as_bytes()),
                    _ => continue,
                };
                let intl = intl_domain(&key_bytes(&key));
                domains.insert(key, domain.clone());
                if !catalogue.all(Some(&intl)).is_empty() {
                    domains.insert(Key::from_bytes(&intl), Value::string(intl));
                }
            }
        }
        let domains = PhpArray::list(domains.values().cloned());
        self.domains = Some(domains.clone());
        domains
    }

    fn messages(&mut self, domain: &[u8], batch: Batch) -> Result<PhpArray, Error> {
        // Upstream's in_array(..., true) deliberately rejects a numeric domain
        // whose getDomains() value is an integer rather than a string.
        if !self
            .get_domains()
            .values()
            .any(|v| matches!(v, Value::String(s) if s.as_bytes() == domain))
        {
            let message = [b"Invalid domain: \"", domain, b"\"."].concat();
            return Err(Error::new(Class::InvalidArgument, message));
        }
        Ok(self.process_domain(domain).batch(batch))
    }

    fn process_domain(&mut self, domain: &[u8]) -> &Messages {
        let index = match self.messages.iter().position(|(d, _)| d == domain) {
            Some(index) => index,
            None => {
                let messages = (self.process)(&self.source, &self.target, &self.result, domain);
                self.messages.push((domain.to_vec(), messages));
                self.messages.len() - 1
            }
        };
        &self.messages[index].1
    }

    pub fn get_messages(&mut self, domain: &[u8]) -> Result<PhpArray, Error> {
        self.messages(domain, Batch::All)
    }

    pub fn get_new_messages(&mut self, domain: &[u8]) -> Result<PhpArray, Error> {
        self.messages(domain, Batch::New)
    }

    pub fn get_obsolete_messages(&mut self, domain: &[u8]) -> Result<PhpArray, Error> {
        self.messages(domain, Batch::Obsolete)
    }

    pub fn get_result(&mut self) -> MessageCatalogue {
        for domain in self.get_domains().values() {
            match domain {
                Value::String(s) => {
                    self.process_domain(s.as_bytes());
                }
                Value::Int(i) => {
                    self.process_domain(i.to_string().as_bytes());
                }
                _ => {}
            }
        }
        self.result.clone()
    }

    /// `intl_available` is the build-time equivalent of PHP's
    /// class_exists(MessageFormatter::class); Batch::All is PHP's default.
    pub fn move_messages_to_intl_domains_if_possible(
        &mut self,
        batch: Batch,
        intl_available: bool,
    ) -> Result<(), Error> {
        self.move_messages_to_intl_domains_with_batch(batch.as_str().as_bytes(), intl_available)
    }

    /// Dynamic converter boundary. Upstream validates the batch only inside
    /// the domain loop, after checking whether intl is available.
    pub fn move_messages_to_intl_domains_with_batch(
        &mut self,
        batch: &[u8],
        intl_available: bool,
    ) -> Result<(), Error> {
        if !intl_available {
            return Ok(());
        }
        for domain in self.get_domains().values() {
            let domain = match domain {
                Value::String(s) => s.as_bytes().to_vec(),
                Value::Int(i) => i.to_string().into_bytes(),
                _ => continue,
            };
            let intl = intl_domain(&domain);
            let messages = self.messages(&domain, Batch::try_from(batch)?)?;
            if messages.is_empty()
                || (self.source.all(Some(&intl)).is_empty()
                    && !self.source.all(Some(&domain)).is_empty())
            {
                continue;
            }
            let result = self.get_result();
            let mut all_intl = result.all(Some(&intl));
            let mut current = messages.clone();
            for key in result.all(Some(&domain)).keys() {
                current.remove(key);
            }
            result.replace(&current, &domain);
            all_intl.union(&messages);
            result.replace(&all_intl, &intl);
            copy_catalogue_metadata(&result, &result, &domain, &intl);
        }
        Ok(())
    }
}

pub(super) fn copy_catalogue_metadata(
    from: &MessageCatalogue,
    result: &MessageCatalogue,
    domain: &[u8],
    to: &[u8],
) {
    if let Value::Array(metadata) = from.get_catalogue_metadata(b"", domain) {
        for (key, value) in metadata.iter() {
            let key = key_bytes(key);
            if result.get_catalogue_metadata(&key, to).is_null() {
                result.set_catalogue_metadata(&key, value.clone(), to);
            }
        }
    }
}

pub(super) fn add_message(
    from: &MessageCatalogue,
    result: &MessageCatalogue,
    domain: &[u8],
    intl: &[u8],
    id: &Key,
    message: &Value,
) {
    let id_bytes = key_bytes(id);
    let domain = if from.defines(&id_bytes, intl) {
        intl
    } else {
        domain
    };
    let mut messages = PhpArray::new();
    messages.insert(id.clone(), message.clone());
    result.add(&messages, domain);
    let metadata = from.get_metadata(&id_bytes, domain);
    if !metadata.is_null() {
        result.set_metadata(&id_bytes, metadata, domain);
    }
}
