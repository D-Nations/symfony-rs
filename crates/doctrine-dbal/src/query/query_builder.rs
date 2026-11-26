//! Port of Doctrine\DBAL\Query\QueryBuilder.php: SQL building, parameters
//! and state. The execution methods (`executeQuery()`,
//! `executeStatement()`, `fetch*()`) are those of a builder over a
//! `Connection` (Slice Connection, at the end); the result cache
//! (`enableResultCache()`, `disableResultCache()`) needs
//! `QueryCacheProfile`, which is not ported (README.md).
//!
//! Doctrine source copyright (c) Doctrine Project; distributed under MIT.
//!
//! Upstream: doctrine/dbal src/Query/QueryBuilder.php

use std::fmt;
use std::sync::{Arc, OnceLock};

use php_value::object::{ArrayObject, GenericObject, StdClass};
use php_value::{Key, PhpArray, Value};

use super::expression::{CompositeExpression, CompositeType, ExpressionBuilder, ExpressionPart};
use super::{
    CommonTableExpression, ConflictResolutionMode, ForUpdate, FromTable, Join, Limit,
    QueryException, QueryType, SelectQuery, Union, UnionQuery, UnionType,
};
use crate::platforms::AbstractPlatform;
use crate::{ConnectionStandIn, Exception, ParameterType, WrapperParameterType};

/// The type a parameter gets when none is given: the default argument of
/// `setParameter()`, `createNamedParameter()` and
/// `createPositionalParameter()`, and what `getParameterType()` gives for
/// a parameter without one.
pub const DEFAULT_PARAMETER_TYPE: ParameterType = ParameterType::String;

/// A query whose SQL can be built: what `(string) $query` does for a
/// `QueryBuilder` given as a CTE or UNION part.
pub trait QuerySql {
    /// `getSQL()`.
    fn get_sql(&self) -> Result<String, Exception>;
}

/// A query given to `with()`, `union()` or `addUnion()`: PHP's
/// `string|QueryBuilder`. A builder is shared, as PHP shares the object;
/// Rust takes it, so it cannot change once given (README.md).
#[derive(Clone)]
pub enum QueryPart<'c> {
    Sql(String),
    Builder(Arc<dyn QuerySql + Send + Sync + 'c>),
}

impl QueryPart<'_> {
    /// `(string) $query`: the SQL, or the builder's `getSQL()`.
    pub fn to_sql(&self) -> Result<String, Exception> {
        match self {
            Self::Sql(sql) => Ok(sql.clone()),
            Self::Builder(builder) => builder.get_sql(),
        }
    }
}

impl fmt::Debug for QueryPart<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Sql(sql) => f.debug_tuple("Sql").field(sql).finish(),
            Self::Builder(_) => f.write_str("Builder(QueryBuilder)"),
        }
    }
}

impl From<&str> for QueryPart<'_> {
    fn from(value: &str) -> Self {
        Self::Sql(value.to_owned())
    }
}

impl From<String> for QueryPart<'_> {
    fn from(value: String) -> Self {
        Self::Sql(value)
    }
}

impl<'c, C: ConnectionStandIn + Sync + ?Sized + 'c> From<QueryBuilder<'c, C>> for QueryPart<'c> {
    fn from(value: QueryBuilder<'c, C>) -> Self {
        Self::Builder(Arc::new(value))
    }
}

/// `QueryBuilder` class is responsible to dynamically create SQL queries.
///
/// Important: Verify that every feature you use will work with your
/// database vendor. SQL Query Builder does not attempt to validate the
/// generated SQL at all.
///
/// The query builder does no validation whatsoever if certain features
/// even work with the underlying database vendor. Limit queries and joins
/// are NOT applied to UPDATE and DELETE statements even if some vendors
/// such as `MySQL` support it.
pub struct QueryBuilder<'c, C: ?Sized> {
    connection: &'c C,
    /// The complete SQL string for this query (cached by `get_sql()`).
    sql: OnceLock<String>,
    /// The query parameters.
    params: PhpArray,
    /// The parameter type map of this query, in insertion order.
    types: Vec<(Key, WrapperParameterType)>,
    /// The type of query this is.
    r#type: QueryType,
    /// The index of the first result to retrieve.
    first_result: i64,
    /// The maximum number of results to retrieve or NULL to retrieve all
    /// results.
    max_results: Option<i64>,
    /// The counter of bound parameters used with `createNamedParameter()`
    /// and `createPositionalParameter()`.
    bound_counter: i64,
    /// The SELECT parts of the query.
    select: Vec<String>,
    /// Whether this is a SELECT DISTINCT query.
    distinct: bool,
    /// The FROM parts of a SELECT query.
    from: Vec<FromTable>,
    /// The table name for an INSERT, UPDATE or DELETE query.
    table: Option<String>,
    /// The list of joins, indexed by from alias, in insertion order.
    join: Vec<(String, Vec<Join>)>,
    /// The SET parts of an UPDATE query.
    set: Vec<String>,
    /// The WHERE part of a SELECT, UPDATE or DELETE query.
    r#where: Option<ExpressionPart>,
    /// The GROUP BY part of a SELECT query.
    group_by: Vec<String>,
    /// The HAVING part of a SELECT query.
    having: Option<ExpressionPart>,
    /// The ORDER BY parts of a SELECT query.
    order_by: Vec<String>,
    for_update: Option<ForUpdate>,
    /// The values of an INSERT query, by column, in insertion order.
    values: Vec<(String, String)>,
    /// The `QueryBuilder` for the union parts.
    union_parts: Vec<Union<'c>>,
    /// The common table expressions of the query.
    common_table_expressions: Vec<CommonTableExpression<'c>>,
}

impl<C: ?Sized> fmt::Debug for QueryBuilder<'_, C> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("QueryBuilder")
            .field("type", &self.r#type)
            .field("params", &self.params)
            .field("types", &self.types)
            .field("select", &self.select)
            .field("from", &self.from)
            .field("table", &self.table)
            .field("where", &self.r#where)
            .finish_non_exhaustive()
    }
}

impl<C: ?Sized> Clone for QueryBuilder<'_, C> {
    /// `clone $qb`: `__clone()` also clones each object parameter, so the
    /// copy's parameters are not identical (`!==`) to the original's.
    /// Objects of the plain classes (`stdClass`, `ArrayObject`, any
    /// recorded class) get a new identity with the same properties; other
    /// objects are shared (README.md).
    fn clone(&self) -> Self {
        let params = self
            .params
            .iter()
            .map(|(key, value)| (key.clone(), clone_parameter(value)))
            .collect();

        Self {
            connection: self.connection,
            sql: self.sql.clone(),
            params,
            types: self.types.clone(),
            r#type: self.r#type,
            first_result: self.first_result,
            max_results: self.max_results,
            bound_counter: self.bound_counter,
            select: self.select.clone(),
            distinct: self.distinct,
            from: self.from.clone(),
            table: self.table.clone(),
            join: self.join.clone(),
            set: self.set.clone(),
            r#where: self.r#where.clone(),
            group_by: self.group_by.clone(),
            having: self.having.clone(),
            order_by: self.order_by.clone(),
            for_update: self.for_update,
            values: self.values.clone(),
            union_parts: self.union_parts.clone(),
            common_table_expressions: self.common_table_expressions.clone(),
        }
    }
}

/// `clone $param` for an object parameter.
fn clone_parameter(value: &Value) -> Value {
    let Value::Object(object) = value else {
        return value.clone();
    };

    if object.downcast_ref::<StdClass>().is_some() {
        StdClass::object(object.properties())
    } else if let Some(array_object) = object.downcast_ref::<ArrayObject>() {
        Value::Object(Arc::new(ArrayObject::new(array_object.storage().clone())))
    } else if object.downcast_ref::<GenericObject>().is_some() {
        Value::Object(Arc::new(GenericObject::new(
            object.class(),
            object.properties(),
        )))
    } else {
        value.clone()
    }
}

/// `$array[$key] = $value` on an ordered list of pairs: replaces the value
/// in place, or appends.
fn set_entry<K: PartialEq, V>(entries: &mut Vec<(K, V)>, key: K, value: V) {
    match entries.iter_mut().find(|(k, _)| *k == key) {
        Some(entry) => entry.1 = value,
        None => entries.push((key, value)),
    }
}

fn strings(items: &[&str]) -> Vec<String> {
    items.iter().map(|item| (*item).to_owned()).collect()
}

impl<'c, C: ?Sized> QueryBuilder<'c, C> {
    /// Initializes a new `QueryBuilder`.
    pub fn new(connection: &'c C) -> Self {
        Self {
            connection,
            sql: OnceLock::new(),
            params: PhpArray::new(),
            types: Vec::new(),
            r#type: QueryType::Select,
            first_result: 0,
            max_results: None,
            bound_counter: 0,
            select: Vec::new(),
            distinct: false,
            from: Vec::new(),
            table: None,
            join: Vec::new(),
            set: Vec::new(),
            r#where: None,
            group_by: Vec::new(),
            having: None,
            order_by: Vec::new(),
            for_update: None,
            values: Vec::new(),
            union_parts: Vec::new(),
            common_table_expressions: Vec::new(),
        }
    }

    /// `$this->sql = null`.
    fn reset_sql(&mut self) {
        self.sql = OnceLock::new();
    }

    /// Sets a query parameter for the query being constructed.
    ///
    /// Like PHP, this does not reset the cached SQL.
    pub fn set_parameter(
        &mut self,
        key: impl Into<Key>,
        value: impl Into<Value>,
        r#type: impl Into<WrapperParameterType>,
    ) -> &mut Self {
        let key = key.into();
        self.params.insert(key.clone(), value.into());
        set_entry(&mut self.types, key, r#type.into());

        self
    }

    /// Sets a collection of query parameters for the query being
    /// constructed, replacing the ones set before. A key given twice in
    /// `types` keeps its first place and its last type, as in a PHP array
    /// literal.
    pub fn set_parameters(
        &mut self,
        params: PhpArray,
        types: impl IntoIterator<Item = (Key, WrapperParameterType)>,
    ) -> &mut Self {
        self.params = params;
        self.types = Vec::new();
        for (key, r#type) in types {
            set_entry(&mut self.types, key, r#type);
        }

        self
    }

    /// Gets all defined query parameters for the query being constructed
    /// indexed by parameter index or name.
    pub fn get_parameters(&self) -> &PhpArray {
        &self.params
    }

    /// Gets a (previously set) query parameter of the query being
    /// constructed: null when it is not set.
    pub fn get_parameter(&self, key: impl Into<Key>) -> Value {
        self.params.get(&key.into()).cloned().unwrap_or(Value::Null)
    }

    /// Gets all defined query parameter types for the query being
    /// constructed indexed by parameter index or name, in insertion
    /// order.
    pub fn get_parameter_types(&self) -> &[(Key, WrapperParameterType)] {
        &self.types
    }

    /// Gets a (previously set) query parameter type of the query being
    /// constructed: [`DEFAULT_PARAMETER_TYPE`] when it is not set.
    pub fn get_parameter_type(&self, key: impl Into<Key>) -> WrapperParameterType {
        let key = key.into();
        self.types.iter().find(|(k, _)| *k == key).map_or(
            WrapperParameterType::Parameter(DEFAULT_PARAMETER_TYPE),
            |(_, r#type)| r#type.clone(),
        )
    }

    /// Sets the position of the first result to retrieve (the "offset").
    pub fn set_first_result(&mut self, first_result: i64) -> &mut Self {
        self.first_result = first_result;
        self.reset_sql();

        self
    }

    /// Gets the position of the first result the query object was set to
    /// retrieve (the "offset").
    pub fn get_first_result(&self) -> i64 {
        self.first_result
    }

    /// Sets the maximum number of results to retrieve (the "limit").
    pub fn set_max_results(&mut self, max_results: Option<i64>) -> &mut Self {
        self.max_results = max_results;
        self.reset_sql();

        self
    }

    /// Gets the maximum number of results the query object was set to
    /// retrieve (the "limit"). Returns NULL if all results will be
    /// returned.
    pub fn get_max_results(&self) -> Option<i64> {
        self.max_results
    }

    /// Locks the queried rows for a subsequent update (PHP's default mode
    /// is `ConflictResolutionMode::default()`).
    pub fn for_update(&mut self, conflict_resolution_mode: ConflictResolutionMode) -> &mut Self {
        self.for_update = Some(ForUpdate::new(conflict_resolution_mode));
        self.reset_sql();

        self
    }

    /// Specifies union parts to be used to build a UNION query.
    /// Replaces any previously specified parts.
    pub fn union(&mut self, part: impl Into<QueryPart<'c>>) -> &mut Self {
        self.r#type = QueryType::Union;
        self.union_parts = vec![Union::new(part.into(), None)];
        self.reset_sql();

        self
    }

    /// Add parts to be used to build a UNION query (PHP's default type is
    /// `UnionType::default()`).
    ///
    /// As in PHP, the query becomes a UNION query even when this fails,
    /// and the cached SQL is kept.
    pub fn add_union(
        &mut self,
        part: impl Into<QueryPart<'c>>,
        r#type: UnionType,
    ) -> Result<&mut Self, QueryException> {
        self.r#type = QueryType::Union;

        if self.union_parts.is_empty() {
            return Err(QueryException::new(
                "No initial UNION part set, use union() to set one first.",
            ));
        }

        self.union_parts.push(Union::new(part.into(), Some(r#type)));
        self.reset_sql();

        Ok(self)
    }

    /// Add a Common Table Expression to be used for a select query.
    pub fn with(
        &mut self,
        name: &str,
        part: impl Into<QueryPart<'c>>,
        columns: Option<&[&str]>,
    ) -> Result<&mut Self, QueryException> {
        self.common_table_expressions
            .push(CommonTableExpression::new(
                name,
                part.into(),
                columns.map(strings),
            )?);
        self.reset_sql();

        Ok(self)
    }

    /// Specifies an item that is to be returned in the query result.
    /// Replaces any previously specified selections, if any.
    pub fn select(&mut self, expressions: &[&str]) -> &mut Self {
        self.r#type = QueryType::Select;
        self.select = strings(expressions);
        self.reset_sql();

        self
    }

    /// Adds or removes DISTINCT to/from the query (PHP's default argument
    /// is `true`).
    pub fn distinct(&mut self, distinct: bool) -> &mut Self {
        self.distinct = distinct;
        self.reset_sql();

        self
    }

    /// Adds an item that is to be returned in the query result.
    pub fn add_select(&mut self, expression: &str, expressions: &[&str]) -> &mut Self {
        self.r#type = QueryType::Select;
        self.select.push(expression.to_owned());
        self.select.extend(strings(expressions));
        self.reset_sql();

        self
    }

    /// Turns the query being built into a bulk delete query that ranges
    /// over a certain table.
    pub fn delete(&mut self, table: &str) -> &mut Self {
        self.r#type = QueryType::Delete;
        self.table = Some(table.to_owned());
        self.reset_sql();

        self
    }

    /// Turns the query being built into a bulk update query that ranges
    /// over a certain table.
    pub fn update(&mut self, table: &str) -> &mut Self {
        self.r#type = QueryType::Update;
        self.table = Some(table.to_owned());
        self.reset_sql();

        self
    }

    /// Turns the query being built into an insert query that inserts into
    /// a certain table.
    pub fn insert(&mut self, table: &str) -> &mut Self {
        self.r#type = QueryType::Insert;
        self.table = Some(table.to_owned());
        self.reset_sql();

        self
    }

    /// Creates and adds a query root corresponding to the table identified
    /// by the given alias, forming a cartesian product with any existing
    /// query roots.
    pub fn from(&mut self, table: &str, alias: Option<&str>) -> &mut Self {
        self.from
            .push(FromTable::new(table, alias.map(str::to_owned)));
        self.reset_sql();

        self
    }

    /// Creates and adds a join to the query (an inner join).
    pub fn join(
        &mut self,
        from_alias: &str,
        join: &str,
        alias: &str,
        condition: Option<&str>,
    ) -> &mut Self {
        self.inner_join(from_alias, join, alias, condition)
    }

    fn add_join(&mut self, from_alias: &str, join: Join) -> &mut Self {
        match self.join.iter_mut().find(|(a, _)| a == from_alias) {
            Some((_, joins)) => joins.push(join),
            None => self.join.push((from_alias.to_owned(), vec![join])),
        }
        self.reset_sql();

        self
    }

    /// Creates and adds a join to the query.
    pub fn inner_join(
        &mut self,
        from_alias: &str,
        join: &str,
        alias: &str,
        condition: Option<&str>,
    ) -> &mut Self {
        self.add_join(from_alias, Join::inner(join, alias, condition))
    }

    /// Creates and adds a left join to the query.
    pub fn left_join(
        &mut self,
        from_alias: &str,
        join: &str,
        alias: &str,
        condition: Option<&str>,
    ) -> &mut Self {
        self.add_join(from_alias, Join::left(join, alias, condition))
    }

    /// Creates and adds a right join to the query.
    pub fn right_join(
        &mut self,
        from_alias: &str,
        join: &str,
        alias: &str,
        condition: Option<&str>,
    ) -> &mut Self {
        self.add_join(from_alias, Join::right(join, alias, condition))
    }

    /// Creates and adds a full join to the query.
    pub fn full_join(
        &mut self,
        from_alias: &str,
        join: &str,
        alias: &str,
        condition: Option<&str>,
    ) -> &mut Self {
        self.add_join(from_alias, Join::full(join, alias, condition))
    }

    /// Sets a new value for a column in a bulk update query.
    pub fn set(&mut self, key: &str, value: &str) -> &mut Self {
        self.set.push(format!("{key} = {value}"));
        self.reset_sql();

        self
    }

    /// Specifies one or more restrictions to the query result. Replaces
    /// any previously specified restrictions, if any.
    pub fn r#where(
        &mut self,
        predicate: impl Into<ExpressionPart>,
        predicates: impl IntoIterator<Item = ExpressionPart>,
    ) -> &mut Self {
        self.r#where = Some(create_predicate(predicate.into(), predicates));
        self.reset_sql();

        self
    }

    /// Adds one or more restrictions to the query results, forming a
    /// logical conjunction with any previously specified restrictions.
    pub fn and_where(
        &mut self,
        predicate: impl Into<ExpressionPart>,
        predicates: impl IntoIterator<Item = ExpressionPart>,
    ) -> &mut Self {
        self.r#where = Some(append_to_predicate(
            self.r#where.take(),
            CompositeType::And,
            predicate.into(),
            predicates,
        ));
        self.reset_sql();

        self
    }

    /// Adds one or more restrictions to the query results, forming a
    /// logical disjunction with any previously specified restrictions.
    pub fn or_where(
        &mut self,
        predicate: impl Into<ExpressionPart>,
        predicates: impl IntoIterator<Item = ExpressionPart>,
    ) -> &mut Self {
        self.r#where = Some(append_to_predicate(
            self.r#where.take(),
            CompositeType::Or,
            predicate.into(),
            predicates,
        ));
        self.reset_sql();

        self
    }

    /// Specifies one or more grouping expressions over the results of the
    /// query. Replaces any previously specified groupings, if any.
    pub fn group_by(&mut self, expression: &str, expressions: &[&str]) -> &mut Self {
        self.group_by = vec![expression.to_owned()];
        self.group_by.extend(strings(expressions));
        self.reset_sql();

        self
    }

    /// Adds one or more grouping expressions to the query.
    pub fn add_group_by(&mut self, expression: &str, expressions: &[&str]) -> &mut Self {
        self.group_by.push(expression.to_owned());
        self.group_by.extend(strings(expressions));
        self.reset_sql();

        self
    }

    /// Sets a value for a column in an insert query.
    ///
    /// Like PHP, this does not reset the cached SQL.
    pub fn set_value(&mut self, column: &str, value: &str) -> &mut Self {
        set_entry(&mut self.values, column.to_owned(), value.to_owned());

        self
    }

    /// Specifies values for an insert query indexed by column names.
    /// Replaces any previous values, if any. A column given twice keeps
    /// its first place and its last value, as in a PHP array literal.
    pub fn values(&mut self, values: &[(&str, &str)]) -> &mut Self {
        self.values = Vec::new();
        for (column, value) in values {
            set_entry(&mut self.values, (*column).to_owned(), (*value).to_owned());
        }
        self.reset_sql();

        self
    }

    /// Specifies a restriction over the groups of the query. Replaces any
    /// previous having restrictions, if any.
    pub fn having(
        &mut self,
        predicate: impl Into<ExpressionPart>,
        predicates: impl IntoIterator<Item = ExpressionPart>,
    ) -> &mut Self {
        self.having = Some(create_predicate(predicate.into(), predicates));
        self.reset_sql();

        self
    }

    /// Adds a restriction over the groups of the query, forming a logical
    /// conjunction with any existing having restrictions.
    pub fn and_having(
        &mut self,
        predicate: impl Into<ExpressionPart>,
        predicates: impl IntoIterator<Item = ExpressionPart>,
    ) -> &mut Self {
        self.having = Some(append_to_predicate(
            self.having.take(),
            CompositeType::And,
            predicate.into(),
            predicates,
        ));
        self.reset_sql();

        self
    }

    /// Adds a restriction over the groups of the query, forming a logical
    /// disjunction with any existing having restrictions.
    pub fn or_having(
        &mut self,
        predicate: impl Into<ExpressionPart>,
        predicates: impl IntoIterator<Item = ExpressionPart>,
    ) -> &mut Self {
        self.having = Some(append_to_predicate(
            self.having.take(),
            CompositeType::Or,
            predicate.into(),
            predicates,
        ));
        self.reset_sql();

        self
    }

    /// Specifies an ordering for the query results. Replaces any
    /// previously specified orderings, if any.
    pub fn order_by(&mut self, sort: &str, order: Option<&str>) -> &mut Self {
        self.order_by = vec![order_by(sort, order)];
        self.reset_sql();

        self
    }

    /// Adds an ordering to the query results.
    pub fn add_order_by(&mut self, sort: &str, order: Option<&str>) -> &mut Self {
        self.order_by.push(order_by(sort, order));
        self.reset_sql();

        self
    }

    /// Resets the WHERE conditions for the query.
    pub fn reset_where(&mut self) -> &mut Self {
        self.r#where = None;
        self.reset_sql();

        self
    }

    /// Resets the grouping for the query.
    pub fn reset_group_by(&mut self) -> &mut Self {
        self.group_by = Vec::new();
        self.reset_sql();

        self
    }

    /// Resets the HAVING conditions for the query.
    pub fn reset_having(&mut self) -> &mut Self {
        self.having = None;
        self.reset_sql();

        self
    }

    /// Resets the ordering for the query.
    pub fn reset_order_by(&mut self) -> &mut Self {
        self.order_by = Vec::new();
        self.reset_sql();

        self
    }

    /// Creates a new named parameter and bind the value `value` to it
    /// (`:dcValue1`, `:dcValue2`, ... unless a placeholder is given; the
    /// parameter's name is the placeholder without its first byte).
    pub fn create_named_parameter(
        &mut self,
        value: impl Into<Value>,
        r#type: impl Into<WrapperParameterType>,
        place_holder: Option<&str>,
    ) -> String {
        let place_holder = if let Some(place_holder) = place_holder {
            place_holder.to_owned()
        } else {
            self.bound_counter += 1;
            format!(":dcValue{}", self.bound_counter)
        };

        let name = place_holder.as_bytes().get(1..).unwrap_or_default();
        self.set_parameter(Key::from_bytes(name), value, r#type);

        place_holder
    }

    /// Creates a new positional parameter and bind the given value to it.
    pub fn create_positional_parameter(
        &mut self,
        value: impl Into<Value>,
        r#type: impl Into<WrapperParameterType>,
    ) -> String {
        self.set_parameter(self.bound_counter, value, r#type);
        self.bound_counter += 1;

        "?".to_owned()
    }

    fn get_sql_for_insert(&self) -> String {
        let columns: Vec<&str> = self.values.iter().map(|(c, _)| c.as_str()).collect();
        let values: Vec<&str> = self.values.iter().map(|(_, v)| v.as_str()).collect();

        format!(
            "INSERT INTO {} ({}) VALUES({})",
            self.table.as_deref().unwrap_or_default(),
            columns.join(", "),
            values.join(", ")
        )
    }

    fn get_sql_for_update(&self) -> String {
        let mut query = format!(
            "UPDATE {} SET {}",
            self.table.as_deref().unwrap_or_default(),
            self.set.join(", ")
        );

        if let Some(r#where) = &self.r#where {
            query = format!("{query} WHERE {where}");
        }

        query
    }

    fn get_sql_for_delete(&self) -> String {
        let mut query = format!("DELETE FROM {}", self.table.as_deref().unwrap_or_default());

        if let Some(r#where) = &self.r#where {
            query = format!("{query} WHERE {where}");
        }

        query
    }

    /// The FROM clauses by table reference (alias, or table without one),
    /// in order; a reference given twice keeps its first place and its
    /// last clause, as PHP's array does.
    fn get_from_clauses(&self) -> Result<Vec<String>, QueryException> {
        let mut from_clauses: Vec<(String, String)> = Vec::new();
        let mut known_aliases: Vec<String> = Vec::new();

        for from in &self.from {
            let (table_sql, table_reference) = match &from.alias {
                Some(alias) if *alias != from.table => {
                    (format!("{} {alias}", from.table), alias.clone())
                }
                _ => (from.table.clone(), from.table.clone()),
            };

            if !known_aliases.contains(&table_reference) {
                known_aliases.push(table_reference.clone());
            }

            let joins = self.get_sql_for_joins(&table_reference, &mut known_aliases)?;
            set_entry(&mut from_clauses, table_reference, table_sql + &joins);
        }

        self.verify_all_aliases_are_known(&known_aliases)?;

        Ok(from_clauses.into_iter().map(|(_, sql)| sql).collect())
    }

    fn verify_all_aliases_are_known(&self, known_aliases: &[String]) -> Result<(), QueryException> {
        for (from_alias, _) in &self.join {
            if !known_aliases.contains(from_alias) {
                return Err(QueryException::unknown_alias(from_alias, known_aliases));
            }
        }

        Ok(())
    }

    fn get_sql_for_joins(
        &self,
        from_alias: &str,
        known_aliases: &mut Vec<String>,
    ) -> Result<String, QueryException> {
        let mut sql = String::new();

        let Some((_, joins)) = self.join.iter().find(|(a, _)| a == from_alias) else {
            return Ok(sql);
        };

        for join in joins {
            if known_aliases.contains(&join.alias) {
                return Err(QueryException::non_unique_alias(&join.alias, known_aliases));
            }

            sql = format!(
                "{sql} {} JOIN {} {}",
                join.r#type.as_str(),
                join.table,
                join.alias
            );

            if let Some(condition) = &join.condition {
                sql = format!("{sql} ON {condition}");
            }

            known_aliases.push(join.alias.clone());
        }

        for join in joins {
            sql.push_str(&self.get_sql_for_joins(&join.alias, known_aliases)?);
        }

        Ok(sql)
    }
}

impl<'c, C: ConnectionStandIn + ?Sized> QueryBuilder<'c, C> {
    /// Gets an `ExpressionBuilder` used for object-oriented construction
    /// of query expressions.
    pub fn expr(&self) -> ExpressionBuilder<'c, C> {
        self.connection.create_expression_builder()
    }

    /// Creates a new query builder on the same connection, for a sub
    /// query (`sub()`).
    #[must_use]
    pub fn sub(&self) -> QueryBuilder<'c, C> {
        self.connection.create_query_builder()
    }

    /// Gets the complete SQL string formed by the current specifications
    /// of this `QueryBuilder` (`getSQL()`, and `(string) $qb`). The SQL is
    /// cached until a method that changes it is called.
    pub fn get_sql(&self) -> Result<String, Exception> {
        if let Some(sql) = self.sql.get() {
            return Ok(sql.clone());
        }

        let sql = match self.r#type {
            QueryType::Insert => self.get_sql_for_insert(),
            QueryType::Delete => self.get_sql_for_delete(),
            QueryType::Update => self.get_sql_for_update(),
            QueryType::Select => self.get_sql_for_select()?,
            QueryType::Union => self.get_sql_for_union()?,
        };

        Ok(self.sql.get_or_init(|| sql).clone())
    }

    fn get_sql_for_select(&self) -> Result<String, Exception> {
        if self.select.is_empty() {
            return Err(QueryException::new(
                "No SELECT expressions given. Please use select() or addSelect().",
            )
            .into());
        }

        let database_platform = self.connection.get_database_platform()?;
        let mut select_parts = Vec::new();

        if let Some((first, others)) = self.common_table_expressions.split_first() {
            select_parts.push(
                database_platform
                    .create_with_sql_builder()
                    .build_sql(first, others)?,
            );
        }

        let builder = database_platform.create_select_sql_builder();
        select_parts.push(builder.build_sql(&SelectQuery::new(
            self.distinct,
            self.select.clone(),
            self.get_from_clauses()?,
            self.r#where.as_ref().map(ToString::to_string),
            self.group_by.clone(),
            self.having.as_ref().map(ToString::to_string),
            self.order_by.clone(),
            Limit::new(self.max_results, self.first_result),
            self.for_update,
        ))?);

        Ok(select_parts.join(" "))
    }

    fn get_sql_for_union(&self) -> Result<String, Exception> {
        if self.union_parts.len() < 2 {
            return Err(QueryException::new(
                "Insufficient UNION parts give, need at least 2. \
                 Please use union() and addUnion() to set enough UNION parts.",
            )
            .into());
        }

        let database_platform = self.connection.get_database_platform()?;
        let mut union_parts = Vec::new();

        if let Some((first, others)) = self.common_table_expressions.split_first() {
            union_parts.push(
                database_platform
                    .create_with_sql_builder()
                    .build_sql(first, others)?,
            );
        }

        union_parts.push(database_platform.create_union_sql_builder().build_sql(
            &UnionQuery::new(
                self.union_parts.clone(),
                self.order_by.clone(),
                Limit::new(self.max_results, self.first_result),
            ),
        )?);

        Ok(union_parts.join(" "))
    }
}

impl<C: ConnectionStandIn + ?Sized> QuerySql for QueryBuilder<'_, C> {
    fn get_sql(&self) -> Result<String, Exception> {
        QueryBuilder::get_sql(self)
    }
}

fn order_by(sort: &str, order: Option<&str>) -> String {
    match order {
        Some(order) => format!("{sort} {order}"),
        None => sort.to_owned(),
    }
}

/// Creates a `CompositeExpression` from one or more predicates combined by
/// the AND logic.
fn create_predicate(
    predicate: ExpressionPart,
    predicates: impl IntoIterator<Item = ExpressionPart>,
) -> ExpressionPart {
    let predicates: Vec<ExpressionPart> = predicates.into_iter().collect();

    if predicates.is_empty() {
        return predicate;
    }

    ExpressionPart::Composite(CompositeExpression::new(
        CompositeType::And,
        predicate,
        predicates,
    ))
}

/// Appends the given predicates combined by the given type of logic to
/// the current predicate.
fn append_to_predicate(
    current_predicate: Option<ExpressionPart>,
    r#type: CompositeType,
    predicate: ExpressionPart,
    predicates: impl IntoIterator<Item = ExpressionPart>,
) -> ExpressionPart {
    let mut predicates: Vec<ExpressionPart> = predicates.into_iter().collect();

    if let Some(ExpressionPart::Composite(current)) = &current_predicate
        && current.get_type() == r#type
    {
        return ExpressionPart::Composite(current.with(predicate, predicates));
    }

    let first = match current_predicate {
        Some(current) => {
            predicates.insert(0, predicate);
            current
        }
        None if predicates.is_empty() => return predicate,
        None => predicate,
    };

    ExpressionPart::Composite(CompositeExpression::new(r#type, first, predicates))
}

// Slice Connection: the execution methods, for a builder over a
// `Connection`.
impl<'c> QueryBuilder<'c, crate::Connection> {
    /// The parameter types as the connection takes them; a key typed twice
    /// keeps its last type, as PHP's array does.
    fn connection_types(&self) -> crate::ParameterTypes {
        self.types.iter().cloned().collect()
    }

    /// Executes an SQL query (SELECT) and returns its result.
    pub async fn execute_query(&self) -> Result<crate::Result<'c>, Exception> {
        let sql = self.get_sql()?;
        self.connection
            .execute_query(&sql, &self.params, &self.connection_types())
            .await
    }

    /// Executes an SQL statement and returns the number of affected rows.
    pub async fn execute_statement(&self) -> Result<i64, Exception> {
        let sql = self.get_sql()?;
        self.connection
            .execute_statement(&sql, &self.params, &self.connection_types())
            .await
    }

    /// The first row of the result as an associative array.
    pub async fn fetch_associative(&self) -> Result<Option<PhpArray>, Exception> {
        self.execute_query().await?.fetch_associative()
    }

    /// The first row of the result as a list.
    pub async fn fetch_numeric(&self) -> Result<Option<PhpArray>, Exception> {
        self.execute_query().await?.fetch_numeric()
    }

    /// The value of the first column of the first row.
    pub async fn fetch_one(&self) -> Result<Option<Value>, Exception> {
        self.execute_query().await?.fetch_one()
    }

    /// Every row as a list.
    pub async fn fetch_all_numeric(&self) -> Result<Vec<PhpArray>, Exception> {
        self.execute_query().await?.fetch_all_numeric()
    }

    /// Every row as an associative array.
    pub async fn fetch_all_associative(&self) -> Result<Vec<PhpArray>, Exception> {
        self.execute_query().await?.fetch_all_associative()
    }

    /// The first column as keys and the second as values.
    pub async fn fetch_all_key_value(&self) -> Result<PhpArray, Exception> {
        self.execute_query().await?.fetch_all_key_value()
    }

    /// The rows by their first column.
    pub async fn fetch_all_associative_indexed(&self) -> Result<PhpArray, Exception> {
        self.execute_query().await?.fetch_all_associative_indexed()
    }

    /// The values of the first column.
    pub async fn fetch_first_column(&self) -> Result<Vec<Value>, Exception> {
        self.execute_query().await?.fetch_first_column()
    }
}
