//! Port of Doctrine\DBAL\Statement.php: a prepared statement, over a
//! driver statement, that converts typed values before binding them.
//!
//! Doctrine source copyright (c) Doctrine Project; distributed under MIT.
//!
//! Upstream: doctrine/dbal src/Statement.php

use php_value::{Key, PhpArray, Value};

use crate::driver::Statement as DriverStatement;
use crate::platforms::AbstractPlatform;
use crate::types;
use crate::{Connection, Exception, ParameterType, ParameterTypes, Result, WrapperParameterType};

type StdResult<T> = std::result::Result<T, Exception>;

/// A prepared statement (`Doctrine\DBAL\Statement`).
pub struct Statement<'c> {
    /// The bound parameters.
    params: PhpArray,
    /// The parameter types.
    types: ParameterTypes,
    platform: &'c (dyn AbstractPlatform + Send + Sync + 'static),
    conn: &'c Connection,
    stmt: Box<dyn DriverStatement>,
    /// The SQL statement.
    sql: String,
}

impl std::fmt::Debug for Statement<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Statement")
            .field("sql", &self.sql)
            .field("params", &self.params)
            .finish_non_exhaustive()
    }
}

impl<'c> Statement<'c> {
    /// `@internal The statement can be only instantiated by Connection`;
    /// it reads the connection's platform, as PHP's constructor does.
    pub async fn new(
        conn: &'c Connection,
        stmt: Box<dyn DriverStatement>,
        sql: String,
    ) -> StdResult<Self> {
        let platform = conn.get_database_platform().await?;
        Ok(Self {
            params: PhpArray::new(),
            types: ParameterTypes::new(),
            platform,
            conn,
            stmt,
            sql,
        })
    }

    /// Binds a parameter value to the statement: by name (without or with
    /// the colon, as the driver takes it) or by its 1-based position. A
    /// DBAL type converts the value first and gives the binding type.
    pub fn bind_value(
        &mut self,
        param: impl Into<Key>,
        value: Value,
        r#type: impl Into<WrapperParameterType>,
    ) -> StdResult<()> {
        let param = param.into();
        let r#type = r#type.into();
        self.params.insert(param.clone(), value.clone());
        self.types.insert(param.clone(), r#type.clone());
        let (value, binding_type) = match r#type {
            WrapperParameterType::Parameter(binding_type) => (value, binding_type),
            WrapperParameterType::Array(_) => {
                return Err(Exception::new(
                    Exception::TYPE_ERROR,
                    "Doctrine\\DBAL\\Statement::bindValue(): Argument #3 ($type) must be of type \
                     Doctrine\\DBAL\\Types\\Type|Doctrine\\DBAL\\ParameterType|string, \
                     Doctrine\\DBAL\\ArrayParameterType given",
                ));
            }
            WrapperParameterType::Name(name) => {
                let type_ = types::get_type(&name.to_string_lossy())?;
                let value = type_.convert_to_database_value(&value, self.platform)?;
                (value, type_.get_binding_type())
            }
            WrapperParameterType::Type(type_) => {
                let value = type_.convert_to_database_value(&value, self.platform)?;
                (value, type_.get_binding_type())
            }
        };
        self.stmt
            .bind_value(&param, value, binding_type)
            .map_err(|e| self.conn.convert_if_driver(e, None))
    }

    /// `bindValue($param, $value)`: the default type, `ParameterType::STRING`.
    pub fn bind_string(&mut self, param: impl Into<Key>, value: Value) -> StdResult<()> {
        self.bind_value(param, value, ParameterType::String)
    }

    async fn execute(&mut self) -> StdResult<Result<'c>> {
        match self.stmt.execute().await {
            Ok(result) => Ok(Result::new(result, self.conn)),
            Err(e) => Err(self.conn.convert_if_driver(
                e,
                Some(crate::Query::new(
                    self.sql.clone(),
                    self.params.clone(),
                    self.types.clone(),
                )),
            )),
        }
    }

    /// Executes the statement with the currently bound parameters and
    /// returns the result.
    pub async fn execute_query(&mut self) -> StdResult<Result<'c>> {
        self.execute().await
    }

    /// Executes the statement and returns the number of affected rows.
    pub async fn execute_statement(&mut self) -> StdResult<i64> {
        self.execute().await?.row_count()
    }

    /// The wrapped driver statement.
    pub fn get_wrapped_statement(&mut self) -> &mut dyn DriverStatement {
        &mut *self.stmt
    }
}
