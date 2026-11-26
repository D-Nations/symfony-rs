//! Doctrine\DBAL\Schema: the schema value objects the platforms' SQL
//! takes.
//!
//! Upstream: none

mod abstract_asset;
mod array_map;
mod collections;
mod column;
mod column_diff;
mod column_editor;
mod default_expression;
pub mod exception;
mod foreign_key_constraint;
mod foreign_key_constraint_editor;
mod identifier;
mod index;
mod index_editor;
pub mod name;
mod primary_key_constraint;
mod schema_config;
mod table;
mod table_diff;
mod table_editor;
mod unique_constraint;
mod unique_constraint_editor;

pub use abstract_asset::{AbstractAsset, AssetKind, NameParser};
pub use array_map::ArrayMap;
pub use collections::{
    CollectionError, NamedObject, OptionallyNamedObject, OptionallyUnqualifiedNamedObjectSet,
    UnqualifiedNamedObjectSet,
};
pub(crate) use column::array_merge_into as column_array_merge;
pub use column::{Column, ColumnType, TypeObject};
pub use column_diff::ColumnDiff;
pub use column_editor::ColumnEditor;
pub use default_expression::{
    CurrentDate, CurrentTime, CurrentTimestamp, DefaultExpression, DefaultExpressionObject,
};
pub use foreign_key_constraint::{
    Deferrability, ForeignKeyConstraint, MatchType, ReferentialAction,
};
pub use foreign_key_constraint_editor::ForeignKeyConstraintEditor;
pub use identifier::Identifier;
pub use index::{Index, IndexType, IndexedColumn};
pub use index_editor::IndexEditor;
pub use primary_key_constraint::{PrimaryKeyConstraint, PrimaryKeyConstraintEditor};
pub use schema_config::{SchemaConfig, TableConfiguration};
pub use table::{Table, TableParts};
pub use table_diff::{IndexRename, TableDiff, TableDiffParts};
pub use table_editor::{TableEditor, TableIndex};
pub use unique_constraint::UniqueConstraint;
pub use unique_constraint_editor::UniqueConstraintEditor;
