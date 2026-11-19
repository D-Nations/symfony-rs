//! Port of Symfony\Component\Config\Definition\ConfigurationInterface.php:
//! a configuration gives the TreeBuilder of its tree.
//!
//! Symfony source copyright (c) Fabien Potencier; distributed under MIT.
//!
//! Upstream: symfony/config Definition/ConfigurationInterface.php

use crate::definition::{builder::TreeBuilder, exception::Error};

/// ConfigurationInterface.
pub trait ConfigurationInterface {
    /// `getConfigTreeBuilder()`. Building the tree can fail where PHP
    /// throws, so it returns a `Result`.
    fn get_config_tree_builder(&self) -> Result<TreeBuilder, Error>;
}
