//! Port of Symfony\Component\Translation\Catalogue\OperationInterface.php.
//!
//! Symfony source copyright (c) Fabien Potencier; distributed under MIT.
//!
//! Upstream: symfony/translation Catalogue/OperationInterface.php

use crate::{Error, MessageCatalogue};
use php_value::PhpArray;

pub trait OperationInterface {
    fn get_domains(&mut self) -> PhpArray;
    fn get_messages(&mut self, domain: &[u8]) -> Result<PhpArray, Error>;
    fn get_new_messages(&mut self, domain: &[u8]) -> Result<PhpArray, Error>;
    fn get_obsolete_messages(&mut self, domain: &[u8]) -> Result<PhpArray, Error>;
    fn get_result(&mut self) -> MessageCatalogue;
}

macro_rules! implement_operation {
    ($name:ty) => {
        impl super::OperationInterface for $name {
            fn get_domains(&mut self) -> php_value::PhpArray {
                self.0.get_domains()
            }
            fn get_messages(&mut self, domain: &[u8]) -> Result<php_value::PhpArray, crate::Error> {
                self.0.get_messages(domain)
            }
            fn get_new_messages(
                &mut self,
                domain: &[u8],
            ) -> Result<php_value::PhpArray, crate::Error> {
                self.0.get_new_messages(domain)
            }
            fn get_obsolete_messages(
                &mut self,
                domain: &[u8],
            ) -> Result<php_value::PhpArray, crate::Error> {
                self.0.get_obsolete_messages(domain)
            }
            fn get_result(&mut self) -> crate::MessageCatalogue {
                self.0.get_result()
            }
        }
    };
}
pub(super) use implement_operation;
