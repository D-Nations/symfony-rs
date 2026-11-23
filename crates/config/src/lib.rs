//! Port of the symfony/config component. See README.md for the contract
//! and what is ported so far.
//!
//! Symfony source copyright (c) Fabien Potencier; distributed under MIT.
//!
//! Upstream: none

#![forbid(unsafe_code)]

pub mod composer;
pub mod config_cache;
pub mod config_cache_factory;
pub mod definition;
pub mod exception;
pub mod file_locator;
pub mod loader;
pub mod php;
pub mod resource;
pub mod util;

pub use config_cache::{
    ConfigCache, ConfigCacheInterface, ResourceCheckerConfigCache, ResourceCheckerInterface,
    ResourceClasses,
};
pub use config_cache_factory::{
    ConfigCacheFactory, ConfigCacheFactoryInterface, ResourceCheckerConfigCacheFactory,
};
pub use definition::{Class, ConfigurationInterface, Error, Node, Processor};
pub use file_locator::{FileLocator, FileLocatorInterface, Located};
pub use util::XmlUtils;
