//! Port of Symfony\Component\Config\Definition\Builder\ExprBuilder.php:
//! an "if ... then ..." rule for normalization or validation.
//!
//! An if part is a closure whose result PHP tests for truth: here it
//! returns any [`Truthy`] (a `bool`, a [`Value`], an `i64`, or a `Result`
//! of one). `always(?\Closure $then)`, `ifTrue(?\Closure)` and
//! `ifFalse(?\Closure)` are each two methods, without and with the
//! closure (`always_then`, `if_true_with`, `if_false_with`).
//!
//! Symfony source copyright (c) Fabien Potencier; distributed under MIT.
//!
//! Upstream: symfony/config Definition/Builder/ExprBuilder.php

use std::{
    fmt,
    sync::{Arc, Mutex, MutexGuard, PoisonError},
};

use php_value::{PhpArray, Value};

use crate::{
    definition::{
        base_node::Closure,
        exception::{Class, Error},
    },
    php,
};

pub const CLASS: &str = "Symfony\\Component\\Config\\Definition\\Builder\\ExprBuilder";

/// The `TYPE_*` constants: what an if part accepts.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ExprType {
    Any,
    String,
    Null,
    Array,
    Bool,
    Int,
    BackedEnum,
}

impl ExprType {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Any => "any",
            Self::String => "string",
            Self::Null => "null",
            Self::Array => "array",
            Self::Bool => "bool",
            Self::Int => "int",
            Self::BackedEnum => "backed-enum",
        }
    }
}

/// An if part.
pub type Condition = Arc<dyn Fn(&Value) -> Result<bool, Error> + Send + Sync>;

/// A closure result PHP tests for truth.
pub trait Truthy {
    fn truthy(self) -> Result<bool, Error>;
}

impl Truthy for bool {
    fn truthy(self) -> Result<bool, Error> {
        Ok(self)
    }
}

impl Truthy for i64 {
    fn truthy(self) -> Result<bool, Error> {
        Ok(self != 0)
    }
}

impl Truthy for Value {
    fn truthy(self) -> Result<bool, Error> {
        Ok(php::truthy(&self))
    }
}

impl<T: Truthy> Truthy for Result<T, Error> {
    fn truthy(self) -> Result<bool, Error> {
        self?.truthy()
    }
}

fn condition<F, R>(f: F) -> Condition
where
    F: Fn(&Value) -> R + Send + Sync + 'static,
    R: Truthy,
{
    Arc::new(move |v| f(v).truthy())
}

#[derive(Default)]
pub(crate) struct ExprState {
    /// Uninitialized until an if part is chosen.
    allowed_types: Option<ExprType>,
    if_part: Option<Condition>,
    then_part: Option<Closure>,
}

/// A shared expression, as the normalization and validation builders
/// keep it.
#[derive(Clone, Default)]
pub(crate) struct ExprRef(Arc<Mutex<ExprState>>);

impl ExprRef {
    fn lock(&self) -> MutexGuard<'_, ExprState> {
        self.0.lock().unwrap_or_else(PoisonError::into_inner)
    }

    pub(crate) fn allowed_types(&self) -> Option<ExprType> {
        self.lock().allowed_types
    }
}

/// A rule: an expression being built, or a closure.
#[derive(Clone)]
pub(crate) enum Expr {
    Builder(ExprRef),
    Closure(Closure),
}

/// An expression whose `end()` gives `T`, its node definition.
pub struct ExprBuilder<T> {
    expr: ExprRef,
    node: T,
}

impl<T: Clone> Clone for ExprBuilder<T> {
    fn clone(&self) -> Self {
        Self {
            expr: self.expr.clone(),
            node: self.node.clone(),
        }
    }
}

impl<T> fmt::Debug for ExprBuilder<T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "ExprBuilder({:?})", self.expr.allowed_types())
    }
}

impl<T: Clone> ExprBuilder<T> {
    pub(crate) fn new(expr: ExprRef, node: T) -> Self {
        Self { expr, node }
    }

    fn set_if(&self, if_part: Condition, allowed: ExprType) -> Self {
        let mut e = self.expr.lock();
        e.if_part = Some(if_part);
        e.allowed_types = Some(allowed);
        drop(e);
        self.clone()
    }

    fn set_then(&self, then: Closure) -> Self {
        self.expr.lock().then_part = Some(then);
        self.clone()
    }

    /// `always()`: the expression is always used.
    pub fn always(&self) -> Self {
        self.set_if(Arc::new(|_| Ok(true)), ExprType::Any)
    }

    /// `always($then)`.
    pub fn always_then<F>(&self, then: F) -> Self
    where
        F: Fn(Value) -> Result<Value, Error> + Send + Sync + 'static,
    {
        self.always();
        self.set_then(Arc::new(then))
    }

    /// `ifTrue()`: the value is `true`.
    pub fn if_true(&self) -> Self {
        self.set_if(
            Arc::new(|v| Ok(matches!(v, Value::Bool(true)))),
            ExprType::Bool,
        )
    }

    /// `ifTrue($closure)`: the closure's result is truthy.
    pub fn if_true_with<F, R>(&self, closure: F) -> Self
    where
        F: Fn(&Value) -> R + Send + Sync + 'static,
        R: Truthy,
    {
        self.set_if(condition(closure), ExprType::Any)
    }

    /// `ifFalse()`: the value is `false`.
    pub fn if_false(&self) -> Self {
        self.set_if(
            Arc::new(|v| Ok(matches!(v, Value::Bool(false)))),
            ExprType::Bool,
        )
    }

    /// `ifFalse($closure)`: the closure's result is falsy.
    pub fn if_false_with<F, R>(&self, closure: F) -> Self
    where
        F: Fn(&Value) -> R + Send + Sync + 'static,
        R: Truthy,
    {
        let closure = condition(closure);
        self.set_if(Arc::new(move |v| Ok(!closure(v)?)), ExprType::Any)
    }

    pub fn if_string(&self) -> Self {
        self.set_if(
            Arc::new(|v| Ok(matches!(v, Value::String(_)))),
            ExprType::String,
        )
    }

    pub fn if_null(&self) -> Self {
        self.set_if(Arc::new(|v| Ok(v.is_null())), ExprType::Null)
    }

    /// `ifEmpty()`: `!$v`.
    pub fn if_empty(&self) -> Self {
        self.set_if(Arc::new(|v| Ok(!php::truthy(v))), ExprType::Any)
    }

    pub fn if_array(&self) -> Self {
        self.set_if(
            Arc::new(|v| Ok(matches!(v, Value::Array(_)))),
            ExprType::Array,
        )
    }

    /// `ifInArray($array)`: `in_array($v, $array, true)`.
    pub fn if_in_array(&self, values: Vec<Value>) -> Self {
        self.set_if(
            Arc::new(move |v| Ok(values.iter().any(|x| x.identical(v)))),
            ExprType::Any,
        )
    }

    /// `ifNotInArray($array)`.
    pub fn if_not_in_array(&self, values: Vec<Value>) -> Self {
        self.set_if(
            Arc::new(move |v| Ok(!values.iter().any(|x| x.identical(v)))),
            ExprType::Any,
        )
    }

    /// `castToArray()`: a value that is not an array becomes `[$v]`.
    pub fn cast_to_array(&self) -> Self {
        self.set_if(
            Arc::new(|v| Ok(!matches!(v, Value::Array(_)))),
            ExprType::Any,
        );
        self.set_then(Arc::new(|v| Ok(Value::Array(PhpArray::list([v])))))
    }

    /// `then($closure)`.
    pub fn then<F>(&self, closure: F) -> Self
    where
        F: Fn(Value) -> Result<Value, Error> + Send + Sync + 'static,
    {
        self.set_then(Arc::new(closure))
    }

    pub fn then_empty_array(&self) -> Self {
        self.set_then(Arc::new(|_| Ok(Value::Array(PhpArray::new()))))
    }

    /// `thenInvalid($message)`: an InvalidArgumentException whose message
    /// is `sprintf($message, json_encode($v))`.
    pub fn then_invalid(&self, message: impl AsRef<[u8]>) -> Self {
        let message = message.as_ref().to_vec();
        self.set_then(Arc::new(move |v| {
            let json = match php::json_encode(&v) {
                Some(json) => Value::from(php_value::PhpString::from(json)),
                None => Value::Bool(false),
            };
            let text = php::sprintf(&message, &[json])?;
            Err(Error::new(Class::InvalidArgument, text))
        }))
    }

    /// `thenUnset()`: an UnsetKeyException removes the key.
    pub fn then_unset(&self) -> Self {
        self.set_then(Arc::new(|_| {
            Err(Error::new(Class::UnsetKey, "Unsetting key."))
        }))
    }

    /// `end()`: the node definition, once both parts are set.
    pub fn end(&self) -> Result<T, Error> {
        let (has_if, has_then) = {
            let e = self.expr.lock();
            (e.if_part.is_some(), e.then_part.is_some())
        };
        if !has_if {
            return Err(Error::new(Class::Runtime, "You must specify an if part."));
        }
        if !has_then {
            return Err(Error::new(Class::Runtime, "You must specify a then part."));
        }
        Ok(self.node.clone())
    }

    /// Sets the public `$ifPart`.
    pub fn set_if_part<F, R>(&self, closure: F)
    where
        F: Fn(&Value) -> R + Send + Sync + 'static,
        R: Truthy,
    {
        self.expr.lock().if_part = Some(condition(closure));
    }

    /// The public `$allowedTypes`, unset until an if part is chosen.
    pub fn allowed_types(&self) -> Option<ExprType> {
        self.expr.allowed_types()
    }
}

/// `ExprBuilder::buildExpressions()`: each expression becomes
/// `fn ($v) => $if($v) ? $then($v) : $v`, with the parts it has now.
pub(crate) fn build_expressions(exprs: Vec<Expr>) -> Vec<Expr> {
    exprs
        .into_iter()
        .map(|expr| match expr {
            Expr::Closure(c) => Expr::Closure(c),
            Expr::Builder(e) => {
                let (if_part, then_part) = {
                    let s = e.lock();
                    (s.if_part.clone(), s.then_part.clone())
                };
                Expr::Closure(Arc::new(move |v| {
                    let not_callable =
                        || Error::new(Class::Error, "Value of type null is not callable");
                    let if_part = if_part.as_ref().ok_or_else(not_callable)?;
                    if if_part(&v)? {
                        let then_part = then_part.as_ref().ok_or_else(not_callable)?;
                        then_part(v)
                    } else {
                        Ok(v)
                    }
                }))
            }
        })
        .collect()
}
