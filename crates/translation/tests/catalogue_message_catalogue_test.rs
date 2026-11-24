//! Port of the separate Catalogue/MessageCatalogueTest.php regression.
//! Symfony tests copyright (c) Fabien Potencier; distributed under MIT.
//!
//! Upstream: symfony/translation Tests/Catalogue/MessageCatalogueTest.php
#![allow(non_snake_case)]
mod common;
use common::{catalogue, messages};
use php_value::Value;
#[test]
fn testIcuMetadataKept() {
    let mc = catalogue(&[("messages", messages(&[("a", "new_a")]))]);
    let metadata = Value::Array(messages(&[("metadata", "value")]));
    mc.set_metadata(b"a", metadata.clone(), b"messages+intl-icu");
    assert_eq!(metadata, mc.get_metadata(b"a", b"messages"));
    assert_eq!(metadata, mc.get_metadata(b"a", b"messages+intl-icu"));
}
