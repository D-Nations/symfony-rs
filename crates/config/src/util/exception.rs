//! Port of Symfony\Component\Config\Util\Exception\XmlParsingException.php
//! and InvalidXmlException.php.
//!
//! Both are classes of the crate's one [`Error`]: [`Class::XmlParsing`]
//! (extends `\InvalidArgumentException`, so `is_a(&Class::InvalidArgument)`
//! holds) and [`Class::InvalidXml`] (extends `XmlParsingException`), for when
//! XML parsed with a schema file or a callable fails without libxml errors
//! of its own.
//!
//! Symfony source copyright (c) Fabien Potencier; distributed under MIT.
//!
//! Upstream: symfony/config Util/Exception/XmlParsingException.php Util/Exception/InvalidXmlException.php

use crate::{Class, Error};

/// `new XmlParsingException($message, 0, $previous)`.
pub fn xml_parsing(message: impl Into<Vec<u8>>, previous: Option<Error>) -> Error {
    with_previous(Class::XmlParsing, message, previous)
}

/// `new InvalidXmlException($message, 0, $previous)`.
pub fn invalid_xml(message: impl Into<Vec<u8>>, previous: Option<Error>) -> Error {
    with_previous(Class::InvalidXml, message, previous)
}

fn with_previous(class: Class, message: impl Into<Vec<u8>>, previous: Option<Error>) -> Error {
    match previous {
        Some(p) => Error::with_previous(class, message, 0, p),
        None => Error::new(class, message),
    }
}
