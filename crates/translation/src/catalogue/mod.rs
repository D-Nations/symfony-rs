//! Catalogue operations and their shared lazy state.
//!
//! Upstream: none

mod abstract_operation;
mod merge_operation;
mod operation_interface;
mod target_operation;

pub use abstract_operation::{AbstractOperation, Batch};
pub use merge_operation::MergeOperation;
pub use operation_interface::OperationInterface;
pub use target_operation::TargetOperation;
