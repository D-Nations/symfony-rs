//! Port of Symfony\Component\Translation\Loader\QtFileLoader.php: Qt
//! Linguist `.ts` files.
//!
//! As upstream (recorded on PHP 8.4.27, libxml 2.9.14):
//! - the resource is checked as the XLIFF loader checks a file name
//!   (`stream_is_local()`, `file_exists()`), then read with XmlUtils's
//!   `loadFile()`; its InvalidArgumentException becomes
//!   `Unable to load "%s".` with that code and the exception as previous;
//! - the domain is pasted into the XPath `//TS/context/name[text()="%s"]`
//!   without escaping, so a domain with `"` changes the expression (an
//!   invalid one finds nothing; a valid one is evaluated);
//! - only when exactly one node is found: from its next sibling's
//!   grandparent (the TS), every `message` element by local name, in all
//!   contexts; the text content of the first `translation`, when truthy (not
//!   `''` or `'0'`), is set for the text content of the first `source` (`''`
//!   when there is none); then the catalogue gets a FileResource;
//! - a found node with no next sibling is PHP's `\Error` (a method called
//!   on `null`), and `libxml_use_internal_errors()` stays `true`.
//!
//! An expression outside php-dom's XPath subset is [`Class::Refused`].
//!
//! Symfony source copyright (c) Fabien Potencier; distributed under MIT.
//!
//! Upstream: symfony/translation Loader/QtFileLoader.php

use std::rc::Rc;

use php_dom::{XPathError, libxml_use_internal_errors, xpath};
use php_value::PhpArray;
use symfony_config::XmlUtils;
use symfony_config::resource::FileResource;

use super::local::{Location, locate, quoted};
use super::xliff_file_loader::LoaderInterface;
use crate::exception::{Class, Error, Previous};
use crate::message_catalogue::MessageCatalogue;

/// `QtFileLoader`.
#[derive(Clone, Copy, Debug, Default)]
pub struct QtFileLoader;

impl LoaderInterface for QtFileLoader {
    fn load(
        &self,
        resource: &[u8],
        locale: &[u8],
        domain: &[u8],
    ) -> Result<MessageCatalogue, Error> {
        let path = match locate(resource) {
            Location::Remote => {
                return Err(Error::new(
                    Class::InvalidResource,
                    quoted("This is not a local file \"", resource, "\"."),
                ));
            }
            Location::Absent => {
                return Err(Error::new(
                    Class::NotFoundResource,
                    quoted("File \"", resource, "\" not found."),
                ));
            }
            Location::Phar => {
                return Err(Error::new(
                    Class::Refused,
                    quoted("phar archive \"", resource, "\" is not read"),
                ));
            }
            Location::Path(p) => {
                if std::fs::metadata(&p).is_err() {
                    return Err(Error::new(
                        Class::NotFoundResource,
                        quoted("File \"", resource, "\" not found."),
                    ));
                }
                p
            }
        };

        let dom = match XmlUtils::load_file_named(&path, resource, None) {
            Ok(dom) => dom,
            Err(e) if e.class().is_a(&symfony_config::Class::InvalidArgument) => {
                return Err(Error::with_previous(
                    Class::InvalidResource,
                    quoted("Unable to load \"", resource, "\"."),
                    e.get_code(),
                    Previous::Config(e),
                ));
            }
            Err(e) => {
                return Err(Error::new(
                    Class::Config(Box::new(e.clone())),
                    e.get_message(),
                ));
            }
        };

        let internal_errors = libxml_use_internal_errors(Some(true));

        let mut expression = b"//TS/context/name[text()=\"".to_vec();
        expression.extend_from_slice(domain);
        expression.extend_from_slice(b"\"]");
        let nodes = match xpath::evaluate(&dom, &expression) {
            Ok(nodes) => nodes,
            // evaluate() returns false: `1 == false->length` is false
            Err(XPathError::Invalid) => Vec::new(),
            Err(refused @ XPathError::Refused(_)) => {
                libxml_use_internal_errors(Some(internal_errors));
                return Err(Error::new(Class::Refused, refused.to_string()));
            }
        };

        let catalogue = MessageCatalogue::new(locale, PhpArray::new());
        if nodes.len() == 1 {
            let Some(sibling) = dom.next_sibling(nodes[0]) else {
                // `null->parentNode->parentNode->getElementsByTagName()`
                return Err(Error::new(
                    Class::PhpError,
                    "Call to a member function getElementsByTagName() on null",
                ));
            };
            let ts = dom
                .parent(sibling)
                .and_then(|context| dom.parent(context))
                .expect("a node the path found has a grandparent");
            for message in dom.get_elements_by_tag_name(ts, "message") {
                let first = |name: &str| {
                    dom.get_elements_by_tag_name(message, name)
                        .first()
                        .map(|&n| dom.text_content(n))
                        .unwrap_or_default()
                };
                let translation = first("translation");
                if !translation.is_empty() && translation != "0" {
                    catalogue.set(first("source").as_bytes(), translation.as_bytes(), domain);
                }
            }

            let file = FileResource::new(resource)
                .map_err(|e| Error::new(Class::Config(Box::new(e.clone())), e.get_message()))?;
            catalogue.add_resource(Rc::new(file));
        }

        libxml_use_internal_errors(Some(internal_errors));

        Ok(catalogue)
    }
}
