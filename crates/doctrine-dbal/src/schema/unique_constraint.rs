//! Port of Doctrine\DBAL\Schema\UniqueConstraint.php: a unique constraint
//! (a `UNIQUE (...)` clause of `CREATE TABLE`).
//!
//! Doctrine source copyright (c) Doctrine Project; distributed under MIT.
//!
//! Upstream: doctrine/dbal src/Schema/UniqueConstraint.php

use std::ops::Deref;

use php_value::{Key, PhpArray, Value};

use crate::deprecations;
use crate::exception::Exception;
use crate::platforms::AbstractPlatform;

use super::abstract_asset::{AbstractAsset, AssetKind, NameParser, trim_quotes};
use super::array_map::ArrayMap;
use super::exception::invalid_state;
use super::identifier::Identifier;
use super::name::{Name, UnqualifiedName, parse_unqualified_name};
use super::unique_constraint_editor::UniqueConstraintEditor;

const LINK: &str = "https://github.com/doctrine/dbal/pull/6685";

/// `Doctrine\DBAL\Schema\UniqueConstraint`.
#[derive(Debug, Clone, PartialEq)]
pub struct UniqueConstraint {
    asset: AbstractAsset,
    columns: ArrayMap<Identifier>,
    flags: ArrayMap<bool>,
    column_names: Vec<UnqualifiedName>,
    failed_to_parse_column_names: bool,
    options: PhpArray,
}

fn method_deprecated(method: &str, hint: &str) -> impl FnOnce() -> String {
    let message =
        format!("Doctrine\\DBAL\\Schema\\UniqueConstraint::{method} is deprecated.{hint}");
    move || message
}

impl UniqueConstraint {
    pub const CLASS: &'static str = "Doctrine\\DBAL\\Schema\\UniqueConstraint";

    /// `new UniqueConstraint($name, $columns, $flags, $options)`.
    pub fn new<C, F>(name: &str, columns: C, flags: F, options: PhpArray) -> Self
    where
        C: IntoIterator,
        C::Item: Into<String>,
        F: IntoIterator,
        F::Item: Into<String>,
    {
        let columns: Vec<String> = columns.into_iter().map(Into::into).collect();
        if columns.is_empty() {
            deprecations::trigger(LINK, || {
                "Instantiation of a unique constraint without columns is deprecated.".to_owned()
            });
        }
        if !options.is_empty() {
            deprecations::trigger(LINK, || {
                format!("Using {} options is deprecated.", Self::CLASS)
            });
        }
        let asset = AbstractAsset::new(
            Self::CLASS,
            AssetKind::OptionallyNamed,
            NameParser::Unqualified,
            Some(name),
        );
        let mut constraint = Self {
            asset,
            columns: ArrayMap::new(),
            flags: ArrayMap::new(),
            column_names: Vec::new(),
            failed_to_parse_column_names: false,
            options,
        };
        for column in &columns {
            constraint.add_column(column);
        }
        for flag in flags {
            constraint.add_flag_inner(&flag.into());
        }
        constraint
    }

    /// `getObjectName()`: the name, `None` when not given.
    pub fn get_object_name(&self) -> Result<Option<UnqualifiedName>, Exception> {
        match self.asset.get_object_name()? {
            Some(Name::Unqualified(name)) => Ok(Some(name.clone())),
            _ => Ok(None),
        }
    }

    /// `getColumnNames()`.
    pub fn get_column_names(&self) -> Result<&[UnqualifiedName], Exception> {
        if self.failed_to_parse_column_names {
            return Err(invalid_state::unique_constraint_has_invalid_column_names(
                &self.get_name(),
            ));
        }
        if self.column_names.is_empty() {
            return Err(invalid_state::unique_constraint_has_empty_column_names(
                &self.get_name(),
            ));
        }
        Ok(&self.column_names)
    }

    /// `getColumns()` (deprecated).
    pub fn get_columns(&self) -> Vec<String> {
        deprecations::trigger_if_called_from_outside(
            LINK,
            method_deprecated("getColumns", " Use getColumnNames() instead."),
        );
        self.columns()
    }

    pub(crate) fn columns(&self) -> Vec<String> {
        self.columns.keys().map(str::to_owned).collect()
    }

    /// `getQuotedColumns($platform)` (deprecated).
    pub fn get_quoted_columns(&self, platform: &(impl AbstractPlatform + ?Sized)) -> Vec<String> {
        deprecations::trigger_if_called_from_outside(
            LINK,
            method_deprecated(
                "getQuotedColumns",
                " Use getColumnNames() and UnqualifiedName::toSQL() instead.",
            ),
        );
        self.quoted_columns(platform)
    }

    pub(crate) fn quoted_columns(
        &self,
        platform: &(impl AbstractPlatform + ?Sized),
    ) -> Vec<String> {
        self.columns
            .values()
            .map(|column| column.get_quoted_name(platform))
            .collect()
    }

    /// `getUnquotedColumns()` (deprecated).
    pub fn get_unquoted_columns(&self) -> Vec<String> {
        deprecations::trigger(
            LINK,
            method_deprecated("getUnquotedColumns", " Use getColumnNames() instead."),
        );
        self.columns().iter().map(|c| trim_quotes(c)).collect()
    }

    /// `getFlags()` (deprecated).
    pub fn get_flags(&self) -> Vec<String> {
        deprecations::trigger_if_called_from_outside(
            LINK,
            method_deprecated("getFlags", " Use isClustered() instead."),
        );
        self.flags()
    }

    pub(crate) fn flags(&self) -> Vec<String> {
        self.flags.keys().map(str::to_owned).collect()
    }

    /// `addFlag($flag)` (deprecated).
    pub fn add_flag(&mut self, flag: &str) -> &mut Self {
        deprecations::trigger_if_called_from_outside(LINK, method_deprecated("addFlag", ""));
        self.add_flag_inner(flag);
        self
    }

    fn add_flag_inner(&mut self, flag: &str) {
        self.flags.insert(flag.to_ascii_lowercase(), true);
    }

    /// `hasFlag($flag)` (deprecated).
    pub fn has_flag(&self, flag: &str) -> bool {
        deprecations::trigger_if_called_from_outside(
            LINK,
            method_deprecated("hasFlag", " Use isClustered() instead."),
        );
        self.has_flag_inner(flag)
    }

    pub(crate) fn has_flag_inner(&self, flag: &str) -> bool {
        self.flags.contains_key(&flag.to_ascii_lowercase())
    }

    pub fn is_clustered(&self) -> bool {
        self.has_flag_inner("clustered")
    }

    /// `removeFlag($flag)` (deprecated).
    pub fn remove_flag(&mut self, flag: &str) {
        deprecations::trigger(LINK, method_deprecated("removeFlag", ""));
        self.flags.remove(&flag.to_ascii_lowercase());
    }

    /// `hasOption($name)` (deprecated).
    pub fn has_option(&self, name: &str) -> bool {
        deprecations::trigger(LINK, method_deprecated("hasOption", ""));
        !matches!(
            self.options
                .get(&Key::from(name.to_ascii_lowercase().as_str())),
            None | Some(Value::Null)
        )
    }

    /// `getOption($name)` (deprecated). PHP warns on a missing key and
    /// returns null.
    pub fn get_option(&self, name: &str) -> Value {
        deprecations::trigger(LINK, method_deprecated("getOption", ""));
        self.options
            .get(&Key::from(name.to_ascii_lowercase().as_str()))
            .cloned()
            .unwrap_or(Value::Null)
    }

    /// `getOptions()` (deprecated).
    pub fn get_options(&self) -> &PhpArray {
        deprecations::trigger_if_called_from_outside(LINK, method_deprecated("getOptions", ""));
        &self.options
    }

    pub(crate) fn options(&self) -> &PhpArray {
        &self.options
    }

    fn add_column(&mut self, column: &str) {
        self.columns
            .insert(column.to_owned(), Identifier::new(column, false));
        match parse_unqualified_name(column) {
            Ok(name) => self.column_names.push(name),
            Err(e) => {
                self.failed_to_parse_column_names = true;
                let message = e.message().to_string_lossy().into_owned();
                deprecations::trigger(LINK, || format!("Unable to parse column name: {message}."));
            }
        }
    }

    /// `UniqueConstraint::editor()`.
    pub fn editor() -> UniqueConstraintEditor {
        UniqueConstraintEditor::new()
    }

    /// `edit()`.
    pub fn edit(&self) -> Result<UniqueConstraintEditor, Exception> {
        Ok(Self::editor()
            .set_name(self.get_object_name()?)
            .set_column_names(self.get_column_names()?.to_vec())
            .set_is_clustered(self.is_clustered()))
    }
}

impl Deref for UniqueConstraint {
    type Target = AbstractAsset;

    fn deref(&self) -> &AbstractAsset {
        &self.asset
    }
}
