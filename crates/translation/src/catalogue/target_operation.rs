//! Port of Symfony\Component\Translation\Catalogue\TargetOperation.php.
//!
//! Symfony source copyright (c) Fabien Potencier; distributed under MIT.
//!
//! Upstream: symfony/translation Catalogue/TargetOperation.php

use super::AbstractOperation;
use super::abstract_operation::{
    Messages, add_message, copy_catalogue_metadata, intl_domain, key_bytes,
};
use super::operation_interface::implement_operation;
use crate::{Error, MessageCatalogue};
use std::ops::{Deref, DerefMut};

#[derive(Debug)]
pub struct TargetOperation(AbstractOperation);

impl TargetOperation {
    pub fn new(source: MessageCatalogue, target: MessageCatalogue) -> Result<Self, Error> {
        AbstractOperation::new(source, target, process_domain).map(Self)
    }
}

impl Deref for TargetOperation {
    type Target = AbstractOperation;
    fn deref(&self) -> &Self::Target {
        &self.0
    }
}
impl DerefMut for TargetOperation {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.0
    }
}
implement_operation!(TargetOperation);

fn process_domain(
    source: &MessageCatalogue,
    target: &MessageCatalogue,
    result: &MessageCatalogue,
    domain: &[u8],
) -> Messages {
    let mut messages = Messages::default();
    let intl = intl_domain(domain);
    copy_catalogue_metadata(target, result, domain, domain);
    copy_catalogue_metadata(target, result, &intl, &intl);
    for (id, message) in source.all(Some(domain)).iter() {
        if target.has(&key_bytes(id), domain) {
            messages.all.insert(id.clone(), message.clone());
            add_message(source, result, domain, &intl, id, message);
        } else {
            messages.obsolete.insert(id.clone(), message.clone());
        }
    }
    for (id, message) in target.all(Some(domain)).iter() {
        if !source.has(&key_bytes(id), domain) {
            messages.all.insert(id.clone(), message.clone());
            messages.new.insert(id.clone(), message.clone());
            add_message(target, result, domain, &intl, id, message);
        }
    }
    messages
}
