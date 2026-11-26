//! Port of Doctrine\DBAL\Schema\ForeignKeyConstraint.php and its enums
//! ForeignKeyConstraint\Deferrability.php, MatchType.php and
//! ReferentialAction.php.
//!
//! Doctrine source copyright (c) Doctrine Project; distributed under MIT.
//!
//! Upstream: doctrine/dbal src/Schema/ForeignKeyConstraint.php
//! Upstream: doctrine/dbal src/Schema/ForeignKeyConstraint/Deferrability.php
//! Upstream: doctrine/dbal src/Schema/ForeignKeyConstraint/MatchType.php
//! Upstream: doctrine/dbal src/Schema/ForeignKeyConstraint/ReferentialAction.php

use std::ops::Deref;

use php_value::{Key, PhpArray, Value};

use crate::deprecations;
use crate::exception::Exception;
use crate::platforms::AbstractPlatform;

use super::abstract_asset::{
    AbstractAsset, AssetKind, NameParser, is_identifier_quoted, trim_quotes,
};
use super::array_map::ArrayMap;
use super::exception::invalid_state;
use super::foreign_key_constraint_editor::ForeignKeyConstraintEditor;
use super::identifier::Identifier;
use super::index::Index;
use super::name::{
    Name, OptionallyQualifiedName, UnqualifiedName, parse_optionally_qualified_name,
    parse_unqualified_name,
};

const LINK: &str = "https://github.com/doctrine/dbal/pull/6728";

/// `ForeignKeyConstraint\Deferrability`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Deferrability {
    NotDeferrable,
    Deferrable,
    Deferred,
}

impl Deferrability {
    pub fn value(self) -> &'static str {
        match self {
            Self::NotDeferrable => "NOT DEFERRABLE",
            Self::Deferrable => "DEFERRABLE",
            Self::Deferred => "INITIALLY DEFERRED",
        }
    }

    pub fn to_sql(self) -> &'static str {
        self.value()
    }
}

/// `ForeignKeyConstraint\MatchType`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MatchType {
    Full,
    Partial,
    Simple,
}

impl MatchType {
    pub const CLASS: &'static str = "Doctrine\\DBAL\\Schema\\ForeignKeyConstraint\\MatchType";

    pub fn value(self) -> &'static str {
        match self {
            Self::Full => "FULL",
            Self::Partial => "PARTIAL",
            Self::Simple => "SIMPLE",
        }
    }

    pub fn to_sql(self) -> &'static str {
        self.value()
    }

    /// `MatchType::tryFrom($value)`.
    pub fn try_from_value(value: &str) -> Option<Self> {
        [Self::Full, Self::Partial, Self::Simple]
            .into_iter()
            .find(|case| case.value() == value)
    }
}

/// `ForeignKeyConstraint\ReferentialAction`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ReferentialAction {
    Cascade,
    NoAction,
    SetDefault,
    SetNull,
    Restrict,
}

impl ReferentialAction {
    pub const CLASS: &'static str =
        "Doctrine\\DBAL\\Schema\\ForeignKeyConstraint\\ReferentialAction";

    pub fn value(self) -> &'static str {
        match self {
            Self::Cascade => "CASCADE",
            Self::NoAction => "NO ACTION",
            Self::SetDefault => "SET DEFAULT",
            Self::SetNull => "SET NULL",
            Self::Restrict => "RESTRICT",
        }
    }

    pub fn to_sql(self) -> &'static str {
        self.value()
    }

    /// `ReferentialAction::tryFrom($value)`.
    pub fn try_from_value(value: &str) -> Option<Self> {
        [
            Self::Cascade,
            Self::NoAction,
            Self::SetDefault,
            Self::SetNull,
            Self::Restrict,
        ]
        .into_iter()
        .find(|case| case.value() == value)
    }
}

/// A foreign key constraint.
#[derive(Debug, Clone, PartialEq)]
pub struct ForeignKeyConstraint {
    asset: AbstractAsset,
    local_column_names: ArrayMap<Identifier>,
    foreign_table_name: Identifier,
    foreign_column_names: ArrayMap<Identifier>,
    options: PhpArray,
    referencing_column_names: Vec<UnqualifiedName>,
    referenced_table_name: Option<OptionallyQualifiedName>,
    referenced_column_names: Vec<UnqualifiedName>,
    match_type: Option<MatchType>,
    on_update_action: Option<ReferentialAction>,
    on_delete_action: Option<ReferentialAction>,
    deferrability: Option<Deferrability>,
}

/// `strtoupper($options[$name])` under `strict_types` (a non-string
/// value is a `TypeError`).
fn upper_option(options: &PhpArray, name: &str) -> Result<Option<String>, Exception> {
    match options.get(&Key::from(name)) {
        None | Some(Value::Null) => Ok(None),
        Some(Value::String(s)) => Ok(Some(s.to_string_lossy().to_ascii_uppercase())),
        Some(other) => Err(Exception::new(
            Exception::TYPE_ERROR,
            format!(
                "strtoupper(): Argument #1 ($string) must be of type string, {} given",
                other.value_name()
            ),
        )),
    }
}

/// `isset($options[$name]) && $options[$name] !== false`, or `None`
/// when not set.
fn flag_option(options: &PhpArray, name: &str) -> Option<bool> {
    match options.get(&Key::from(name)) {
        None | Some(Value::Null) => None,
        Some(value) => Some(!matches!(value, Value::Bool(false))),
    }
}

impl ForeignKeyConstraint {
    pub const CLASS: &'static str = "Doctrine\\DBAL\\Schema\\ForeignKeyConstraint";

    /// `new ForeignKeyConstraint($localColumnNames, $foreignTableName,
    /// $foreignColumnNames, $name, $options)`.
    pub fn new<L, F>(
        local_column_names: L,
        foreign_table_name: &str,
        foreign_column_names: F,
        name: &str,
        options: PhpArray,
    ) -> Result<Self, Exception>
    where
        L: IntoIterator,
        L::Item: Into<String>,
        F: IntoIterator,
        F::Item: Into<String>,
    {
        let local: Vec<String> = local_column_names.into_iter().map(Into::into).collect();
        let foreign: Vec<String> = foreign_column_names.into_iter().map(Into::into).collect();

        if local.is_empty() {
            deprecations::trigger(LINK, || {
                "Instantiation of a foreign key constraint without local column names is deprecated."
                    .to_owned()
            });
        }
        if foreign.is_empty() {
            deprecations::trigger(LINK, || {
                "Instantiation of a foreign key constraint without foreign column names is deprecated."
                    .to_owned()
            });
        }
        if foreign.len() != local.len() {
            deprecations::trigger(LINK, || {
                "Instantiation of a foreign key constraint with a different number of local and foreign \
                 column names is deprecated."
                    .to_owned()
            });
        }

        let asset = AbstractAsset::new(
            Self::CLASS,
            AssetKind::OptionallyNamed,
            NameParser::Unqualified,
            Some(name),
        );

        let match_type = Self::parse_match_type(&options)?;
        let on_update_action = Self::parse_referential_action(&options, "onUpdate")?;
        let on_delete_action = Self::parse_referential_action(&options, "onDelete")?;
        let deferrability = Self::parse_deferrability(&options);

        Ok(Self {
            asset,
            local_column_names: create_identifier_map(&local),
            foreign_table_name: Identifier::new(foreign_table_name, false),
            foreign_column_names: create_identifier_map(&foreign),
            referencing_column_names: parse_column_names(&local),
            referenced_table_name: parse_referenced_table_name(foreign_table_name),
            referenced_column_names: parse_column_names(&foreign),
            match_type,
            on_update_action,
            on_delete_action,
            deferrability,
            options,
        })
    }

    /// `getObjectName()`: the name, `None` when not given.
    pub fn get_object_name(&self) -> Result<Option<UnqualifiedName>, Exception> {
        match self.asset.get_object_name()? {
            Some(Name::Unqualified(name)) => Ok(Some(name.clone())),
            _ => Ok(None),
        }
    }

    pub fn get_referencing_column_names(&self) -> Result<&[UnqualifiedName], Exception> {
        if self.referencing_column_names.is_empty() {
            return Err(
                invalid_state::foreign_key_constraint_has_invalid_referencing_column_names(
                    &self.get_name(),
                ),
            );
        }
        Ok(&self.referencing_column_names)
    }

    pub fn get_referenced_table_name(&self) -> Result<&OptionallyQualifiedName, Exception> {
        self.referenced_table_name.as_ref().ok_or_else(|| {
            invalid_state::foreign_key_constraint_has_invalid_referenced_table_name(
                &self.get_name(),
            )
        })
    }

    pub fn get_referenced_column_names(&self) -> Result<&[UnqualifiedName], Exception> {
        if self.referenced_column_names.is_empty() {
            return Err(
                invalid_state::foreign_key_constraint_has_invalid_referenced_column_names(
                    &self.get_name(),
                ),
            );
        }
        Ok(&self.referenced_column_names)
    }

    pub fn get_match_type(&self) -> Result<MatchType, Exception> {
        self.match_type.ok_or_else(|| {
            invalid_state::foreign_key_constraint_has_invalid_match_type(&self.get_name())
        })
    }

    pub fn get_on_update_action(&self) -> Result<ReferentialAction, Exception> {
        self.on_update_action.ok_or_else(|| {
            invalid_state::foreign_key_constraint_has_invalid_on_update_action(&self.get_name())
        })
    }

    pub fn get_on_delete_action(&self) -> Result<ReferentialAction, Exception> {
        self.on_delete_action.ok_or_else(|| {
            invalid_state::foreign_key_constraint_has_invalid_on_delete_action(&self.get_name())
        })
    }

    pub fn get_deferrability(&self) -> Result<Deferrability, Exception> {
        self.deferrability.ok_or_else(|| {
            invalid_state::foreign_key_constraint_has_invalid_deferrability(&self.get_name())
        })
    }

    fn outside(method: &str, message: &str) {
        deprecations::trigger_if_called_from_outside(LINK, || {
            format!(
                "Doctrine\\DBAL\\Schema\\ForeignKeyConstraint::{method} is deprecated.{message}"
            )
        });
    }

    /// `getLocalColumns()` (deprecated).
    pub fn get_local_columns(&self) -> Vec<String> {
        Self::outside(
            "getLocalColumns",
            " Use getReferencingColumnNames() instead.",
        );
        self.local_columns()
    }

    pub(crate) fn local_columns(&self) -> Vec<String> {
        self.local_column_names.keys().map(str::to_owned).collect()
    }

    /// `getQuotedLocalColumns($platform)` (deprecated).
    pub fn get_quoted_local_columns(
        &self,
        platform: &(impl AbstractPlatform + ?Sized),
    ) -> Vec<String> {
        Self::outside(
            "getQuotedLocalColumns",
            " Use getReferencingColumnNames() and UnqualifiedName::toSQL() instead.",
        );
        self.quoted_local_columns(platform)
    }

    pub(crate) fn quoted_local_columns(
        &self,
        platform: &(impl AbstractPlatform + ?Sized),
    ) -> Vec<String> {
        self.local_column_names
            .values()
            .map(|column| column.get_quoted_name(platform))
            .collect()
    }

    /// `getUnquotedLocalColumns()` (deprecated).
    pub fn get_unquoted_local_columns(&self) -> Vec<String> {
        Self::outside(
            "getUnquotedLocalColumns",
            " Use getReferencingColumnNames() instead.",
        );
        self.local_columns()
            .iter()
            .map(|c| trim_quotes(c))
            .collect()
    }

    pub(crate) fn unquoted_local_columns(&self) -> Vec<String> {
        self.local_columns()
            .iter()
            .map(|c| trim_quotes(c))
            .collect()
    }

    /// `getUnquotedForeignColumns()` (deprecated).
    pub fn get_unquoted_foreign_columns(&self) -> Vec<String> {
        Self::outside(
            "getUnquotedForeignColumns",
            " Use getReferencedColumnNames() instead.",
        );
        self.foreign_columns()
            .iter()
            .map(|c| trim_quotes(c))
            .collect()
    }

    /// `getForeignTableName()` (deprecated).
    pub fn get_foreign_table_name(&self) -> String {
        Self::outside(
            "getForeignTableName",
            " Use getReferencedTableName() instead.",
        );
        self.foreign_table_name()
    }

    pub(crate) fn foreign_table_name(&self) -> String {
        self.foreign_table_name.get_name()
    }

    /// `getUnqualifiedForeignTableName()` (deprecated).
    pub fn get_unqualified_foreign_table_name(&self) -> String {
        Self::outside(
            "getUnqualifiedForeignTableName",
            " Use getReferencedTableName() instead.",
        );
        let full = self.foreign_table_name.get_name();
        let mut name = match full.rfind('.') {
            Some(position) => full[position + 1..].to_owned(),
            None => full,
        };
        if is_identifier_quoted(&name) {
            name = trim_quotes(&name);
        }
        name.to_ascii_lowercase()
    }

    /// `getQuotedForeignTableName($platform)` (deprecated).
    pub fn get_quoted_foreign_table_name(
        &self,
        platform: &(impl AbstractPlatform + ?Sized),
    ) -> String {
        Self::outside(
            "getQuotedForeignTableName",
            " Use getReferencedTableName() and OptionallyQualifiedName::toSQL() instead.",
        );
        self.quoted_foreign_table_name(platform)
    }

    pub(crate) fn quoted_foreign_table_name(
        &self,
        platform: &(impl AbstractPlatform + ?Sized),
    ) -> String {
        self.foreign_table_name.get_quoted_name(platform)
    }

    /// `getForeignColumns()` (deprecated).
    pub fn get_foreign_columns(&self) -> Vec<String> {
        Self::outside(
            "getForeignColumns",
            " Use getReferencedColumnNames() instead.",
        );
        self.foreign_columns()
    }

    pub(crate) fn foreign_columns(&self) -> Vec<String> {
        self.foreign_column_names
            .keys()
            .map(str::to_owned)
            .collect()
    }

    /// `getQuotedForeignColumns($platform)` (deprecated).
    pub fn get_quoted_foreign_columns(
        &self,
        platform: &(impl AbstractPlatform + ?Sized),
    ) -> Vec<String> {
        Self::outside(
            "getQuotedForeignColumns",
            " Use getReferencedColumnNames() and UnqualifiedName::toSQL() instead.",
        );
        self.quoted_foreign_columns(platform)
    }

    pub(crate) fn quoted_foreign_columns(
        &self,
        platform: &(impl AbstractPlatform + ?Sized),
    ) -> Vec<String> {
        self.foreign_column_names
            .values()
            .map(|column| column.get_quoted_name(platform))
            .collect()
    }

    const OPTIONS_HINT: &'static str = " Use getMatchType(), getOnDeleteAction(), getOnUpdateAction() or \
                                         getDeferrability() instead.";

    /// `hasOption($name)` (deprecated): `isset()`.
    pub fn has_option(&self, name: &str) -> bool {
        Self::outside("hasOption", Self::OPTIONS_HINT);
        self.has_option_inner(name)
    }

    pub(crate) fn has_option_inner(&self, name: &str) -> bool {
        !matches!(self.options.get(&Key::from(name)), None | Some(Value::Null))
    }

    /// `getOption($name)` (deprecated). PHP warns on a missing key and
    /// returns null.
    pub fn get_option(&self, name: &str) -> Value {
        Self::outside("getOption", Self::OPTIONS_HINT);
        self.option(name)
    }

    pub(crate) fn option(&self, name: &str) -> Value {
        self.options
            .get(&Key::from(name))
            .cloned()
            .unwrap_or(Value::Null)
    }

    /// `getOptions()` (deprecated).
    pub fn get_options(&self) -> &PhpArray {
        Self::outside("getOptions", Self::OPTIONS_HINT);
        &self.options
    }

    pub(crate) fn options(&self) -> &PhpArray {
        &self.options
    }

    /// `onUpdate()` (deprecated).
    pub fn on_update(&self) -> Option<String> {
        Self::outside("onUpdate", " Use getOnUpdateAction() instead.");
        self.on_event("onUpdate")
    }

    /// `onDelete()` (deprecated).
    pub fn on_delete(&self) -> Option<String> {
        Self::outside("onDelete", " Use getOnDeleteAction() instead.");
        self.on_event("onDelete")
    }

    pub(crate) fn on_event(&self, event: &str) -> Option<String> {
        match self.options.get(&Key::from(event)) {
            Some(Value::String(s)) => {
                let on_event = s.to_string_lossy().to_ascii_uppercase();
                (on_event != "NO ACTION" && on_event != "RESTRICT").then_some(on_event)
            }
            _ => None,
        }
    }

    fn parse_match_type(options: &PhpArray) -> Result<Option<MatchType>, Exception> {
        let Some(value) = upper_option(options, "match")? else {
            return Ok(Some(MatchType::Simple));
        };
        Ok(MatchType::try_from_value(&value).or_else(|| {
            deprecations::trigger(LINK, || {
                format!(
                    "Unable to parse match type: \"{value}\" is not a valid backing value for enum {}.",
                    MatchType::CLASS
                )
            });
            None
        }))
    }

    fn parse_referential_action(
        options: &PhpArray,
        option: &str,
    ) -> Result<Option<ReferentialAction>, Exception> {
        let Some(value) = upper_option(options, option)? else {
            return Ok(Some(ReferentialAction::NoAction));
        };
        Ok(ReferentialAction::try_from_value(&value).or_else(|| {
            deprecations::trigger(LINK, || {
                format!(
                    "Unable to parse referential action: \"{value}\" is not a valid backing value for enum {}.",
                    ReferentialAction::CLASS
                )
            });
            None
        }))
    }

    fn parse_deferrability(options: &PhpArray) -> Option<Deferrability> {
        // a constraint is INITIALLY IMMEDIATE unless explicitly declared
        // as INITIALLY DEFERRED
        let is_deferred = flag_option(options, "deferred").unwrap_or(false);
        // a constraint is NOT DEFERRABLE unless explicitly declared as
        // DEFERRABLE or is explicitly or implicitly INITIALLY DEFERRED
        let is_deferrable = flag_option(options, "deferrable").unwrap_or(is_deferred);

        if is_deferred {
            if !is_deferrable {
                deprecations::trigger(LINK, || {
                    "Declaring a constraint as NOT DEFERRABLE INITIALLY DEFERRED is deprecated"
                        .to_owned()
                });
                return None;
            }
            return Some(Deferrability::Deferred);
        }
        Some(if is_deferrable {
            Deferrability::Deferrable
        } else {
            Deferrability::NotDeferrable
        })
    }

    /// `intersectsIndexColumns($index)` (deprecated).
    pub fn intersects_index_columns(&self, index: &Index) -> bool {
        Self::outside("intersectsIndexColumns", "");
        self.intersects_index_columns_inner(index)
    }

    pub(crate) fn intersects_index_columns_inner(&self, index: &Index) -> bool {
        index.get_columns().iter().any(|index_column| {
            self.local_column_names
                .values()
                .any(|local| index_column.eq_ignore_ascii_case(&local.get_name()))
        })
    }

    /// `ForeignKeyConstraint::editor()`.
    pub fn editor() -> ForeignKeyConstraintEditor {
        ForeignKeyConstraintEditor::new()
    }

    /// `edit()`.
    pub fn edit(&self) -> Result<ForeignKeyConstraintEditor, Exception> {
        Ok(Self::editor()
            .set_name(self.get_object_name()?)
            .set_referenced_table_name(self.get_referenced_table_name()?.clone())
            .set_referencing_column_names(self.get_referencing_column_names()?.to_vec())
            .set_referenced_column_names(self.get_referenced_column_names()?.to_vec())
            .set_match_type(self.get_match_type()?)
            .set_on_delete_action(self.get_on_delete_action()?)
            .set_on_update_action(self.get_on_update_action()?)
            .set_deferrability(self.get_deferrability()?))
    }
}

impl Deref for ForeignKeyConstraint {
    type Target = AbstractAsset;

    fn deref(&self) -> &AbstractAsset {
        &self.asset
    }
}

fn create_identifier_map(names: &[String]) -> ArrayMap<Identifier> {
    let mut identifiers = ArrayMap::new();
    for name in names {
        identifiers.insert(name.clone(), Identifier::new(name, false));
    }
    identifiers
}

fn parse_referenced_table_name(name: &str) -> Option<OptionallyQualifiedName> {
    match parse_optionally_qualified_name(name) {
        Ok(name) => Some(name),
        Err(e) => {
            let message = e.message().to_string_lossy().into_owned();
            deprecations::trigger(LINK, || {
                format!("Unable to parse referenced table name: {message}.")
            });
            None
        }
    }
}

fn parse_column_names(names: &[String]) -> Vec<UnqualifiedName> {
    let parsed: Result<Vec<_>, _> = names
        .iter()
        .map(|name| parse_unqualified_name(name))
        .collect();
    match parsed {
        Ok(names) => names,
        Err(e) => {
            let message = e.message().to_string_lossy().into_owned();
            deprecations::trigger(LINK, || format!("Unable to parse column name: {message}."));
            Vec::new()
        }
    }
}
