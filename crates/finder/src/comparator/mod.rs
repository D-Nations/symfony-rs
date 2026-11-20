//! Port of Symfony\Component\Finder\Comparator\Comparator.php.
//!
//! Symfony source copyright (c) Fabien Potencier; distributed under MIT.
//!
//! Upstream: symfony/finder Comparator/Comparator.php

mod date_comparator;
mod number_comparator;

use std::cmp::Ordering;

pub use date_comparator::DateComparator;
pub use number_comparator::NumberComparator;

use crate::{
    exception::Error,
    php::{Scalar, compare},
};

/// The comparison operators (`>`, `<`, `>=`, `<=`, `==`, `!=`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Operator {
    Greater,
    Less,
    GreaterOrEqual,
    LessOrEqual,
    Equal,
    NotEqual,
}

impl Operator {
    pub fn parse(s: &str) -> Option<Self> {
        Some(match s {
            ">" => Self::Greater,
            "<" => Self::Less,
            ">=" => Self::GreaterOrEqual,
            "<=" => Self::LessOrEqual,
            "==" => Self::Equal,
            "!=" => Self::NotEqual,
            _ => return None,
        })
    }

    pub fn as_str(self) -> &'static str {
        match self {
            Self::Greater => ">",
            Self::Less => "<",
            Self::GreaterOrEqual => ">=",
            Self::LessOrEqual => "<=",
            Self::Equal => "==",
            Self::NotEqual => "!=",
        }
    }
}

/// A target and an operator; `test()` compares a value with PHP's rules.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Comparator {
    target: Vec<u8>,
    operator: Operator,
}

impl Comparator {
    /// `new Comparator($target, $operator)`.
    pub fn new(target: impl Into<Vec<u8>>, operator: &str) -> Result<Self, Error> {
        let Some(operator) = Operator::parse(operator) else {
            return Err(Error::InvalidArgument(format!(
                "Invalid operator \"{operator}\"."
            )));
        };
        Ok(Self {
            target: target.into(),
            operator,
        })
    }

    /// `getTarget()`.
    pub fn get_target(&self) -> &[u8] {
        &self.target
    }

    /// `getOperator()`.
    pub fn get_operator(&self) -> &'static str {
        self.operator.as_str()
    }

    pub fn operator(&self) -> Operator {
        self.operator
    }

    /// `test($test)`.
    pub fn test(&self, test: impl Into<Scalar>) -> bool {
        let target = Scalar::String(self.target.clone());
        let o = compare(&test.into(), &target);
        match self.operator {
            Operator::Greater => o == Ordering::Greater,
            Operator::GreaterOrEqual => o != Ordering::Less,
            Operator::Less => o == Ordering::Less,
            Operator::LessOrEqual => o != Ordering::Greater,
            Operator::NotEqual => o != Ordering::Equal,
            Operator::Equal => o == Ordering::Equal,
        }
    }
}
