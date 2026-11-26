//! `Symfony\Component\HttpFoundation\Session\Storage`: the metadata bag
//! and the storages ported so far (the in-memory and file mocks).
//!
//! Upstream: none

pub mod metadata_bag;
pub mod mock_array_session_storage;
pub mod mock_file_session_storage;
pub mod mock_file_session_storage_factory;
pub mod session_storage_factory_interface;
pub mod session_storage_interface;
