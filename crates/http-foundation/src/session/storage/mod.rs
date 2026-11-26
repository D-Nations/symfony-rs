//! `Symfony\Component\HttpFoundation\Session\Storage`: the metadata bag
//! and the storages: the in-memory and file mocks, and PHP's session
//! (native, or bridged to one PHP started).
//!
//! Upstream: none

pub mod handler;
pub mod metadata_bag;
pub mod mock_array_session_storage;
pub mod mock_file_session_storage;
pub mod mock_file_session_storage_factory;
pub mod native_session_storage;
pub mod native_session_storage_factory;
pub mod php_bridge_session_storage;
pub mod php_bridge_session_storage_factory;
pub mod proxy;
pub mod session_storage_factory_interface;
pub mod session_storage_interface;
