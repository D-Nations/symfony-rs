//! Port of the marker interfaces Doctrine\DBAL\Types\PhpDateMappingType.php,
//! PhpDateTimeMappingType.php, PhpFloatMappingType.php,
//! PhpIntegerMappingType.php and PhpTimeMappingType.php: which PHP type a
//! Doctrine type converts to (`Type::php_mapping_type()`).
//!
//! Doctrine source copyright (c) Doctrine Project; distributed under MIT.
//!
//! Upstream: doctrine/dbal src/Types/PhpDateMappingType.php src/Types/PhpDateTimeMappingType.php
//! Upstream: doctrine/dbal src/Types/PhpFloatMappingType.php src/Types/PhpIntegerMappingType.php
//! Upstream: doctrine/dbal src/Types/PhpTimeMappingType.php

/// The `Php*MappingType` interface a type implements.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum PhpMappingType {
    /// `PhpDateMappingType`: implemented by date types.
    Date,
    /// `PhpDateTimeMappingType`: implemented by date-time types.
    DateTime,
    /// `PhpFloatMappingType`: implemented by float types.
    Float,
    /// `PhpIntegerMappingType`: implemented by integer types.
    Integer,
    /// `PhpTimeMappingType`: implemented by time types.
    Time,
}

impl PhpMappingType {
    /// The interface's name.
    pub fn interface(self) -> &'static str {
        match self {
            Self::Date => "Doctrine\\DBAL\\Types\\PhpDateMappingType",
            Self::DateTime => "Doctrine\\DBAL\\Types\\PhpDateTimeMappingType",
            Self::Float => "Doctrine\\DBAL\\Types\\PhpFloatMappingType",
            Self::Integer => "Doctrine\\DBAL\\Types\\PhpIntegerMappingType",
            Self::Time => "Doctrine\\DBAL\\Types\\PhpTimeMappingType",
        }
    }
}
