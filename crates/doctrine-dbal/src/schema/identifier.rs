//! Port of Doctrine\DBAL\Schema\Identifier.php: an identifier in the
//! context of database objects (`new Identifier($name)`), used to quote
//! names given as strings.
//!
//! Doctrine source copyright (c) Doctrine Project; distributed under MIT.
//!
//! Upstream: doctrine/dbal src/Schema/Identifier.php

use std::ops::Deref;

use crate::exception::Exception;

use super::abstract_asset::{AbstractAsset, AssetKind, NameParser};
use super::name::{GenericName, Name};

/// `Doctrine\DBAL\Schema\Identifier`.
#[derive(Debug, Clone, PartialEq)]
pub struct Identifier {
    asset: AbstractAsset,
}

impl Identifier {
    pub const CLASS: &'static str = "Doctrine\\DBAL\\Schema\\Identifier";

    /// `new Identifier($identifier, $quote)`.
    pub fn new(identifier: &str, quote: bool) -> Self {
        let mut asset = AbstractAsset::new(
            Self::CLASS,
            AssetKind::Named,
            NameParser::Generic,
            Some(identifier),
        );
        if quote && !asset.quoted {
            let quoted = format!("\"{}\"", asset.get_name());
            asset.set_name_string(&quoted);
        }
        Self { asset }
    }

    /// `getObjectName()`.
    pub fn get_object_name(&self) -> Result<GenericName, Exception> {
        match self.asset.get_object_name()? {
            Some(Name::Generic(name)) => Ok(name.clone()),
            _ => Err(super::exception::invalid_state::object_name_not_initialized()),
        }
    }
}

impl Deref for Identifier {
    type Target = AbstractAsset;

    fn deref(&self) -> &AbstractAsset {
        &self.asset
    }
}
