//! Port of Symfony\Component\Config\Definition\Builder\NumericNodeDefinition.php:
//! `min()` and `max()` for integer and float definitions, which refuse a
//! bound that contradicts the other. `cannotBeEmpty()`, which it refuses,
//! is in [`crate::definition::builder::node_definition`].
//!
//! Symfony source copyright (c) Fabien Potencier; distributed under MIT.
//!
//! Upstream: symfony/config Definition/Builder/NumericNodeDefinition.php

use php_value::{Number, Value};

use crate::{
    definition::{
        builder::node_definition::{Definition, class},
        exception::{Class, Error},
    },
    php::{self, Msg},
};

fn value(n: Number) -> Value {
    match n {
        Number::Int(i) => Value::Int(i),
        Number::Float(f) => Value::Float(f),
    }
}

/// `int|float`.
pub trait IntoNumber {
    fn into_number(self) -> Number;
}

impl IntoNumber for Number {
    fn into_number(self) -> Number {
        self
    }
}

impl IntoNumber for i64 {
    fn into_number(self) -> Number {
        Number::Int(self)
    }
}

impl IntoNumber for i32 {
    fn into_number(self) -> Number {
        Number::Int(i64::from(self))
    }
}

impl IntoNumber for f64 {
    fn into_number(self) -> Number {
        Number::Float(self)
    }
}

impl<C: class::Numeric, P: Clone> Definition<C, P> {
    /// `max($max)`: the value must be at most `max`.
    pub fn max(&self, max: impl IntoNumber) -> Result<Self, Error> {
        let max = max.into_number();
        if let Some(min) = self.def.read(|s| s.numeric.min)
            && php::greater(&value(min), &value(max))
        {
            return Err(Error::new(
                Class::InvalidArgument,
                Msg::new("You cannot define a max(")
                    .b(&php::to_bytes(&value(max)))
                    .s(") as you already have a min(")
                    .b(&php::to_bytes(&value(min)))
                    .s(")."),
            ));
        }
        self.def.write(|s| s.numeric.max = Some(max));
        Ok(self.clone())
    }

    /// `min($min)`: the value must be at least `min`.
    pub fn min(&self, min: impl IntoNumber) -> Result<Self, Error> {
        let min = min.into_number();
        if let Some(max) = self.def.read(|s| s.numeric.max)
            && php::less(&value(max), &value(min))
        {
            return Err(Error::new(
                Class::InvalidArgument,
                Msg::new("You cannot define a min(")
                    .b(&php::to_bytes(&value(min)))
                    .s(") as you already have a max(")
                    .b(&php::to_bytes(&value(max)))
                    .s(")."),
            ));
        }
        self.def.write(|s| s.numeric.min = Some(min));
        Ok(self.clone())
    }
}
