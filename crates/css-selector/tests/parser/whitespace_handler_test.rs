//! WhitespaceHandlerTest.php (through AbstractHandlerTestCase.php) at
//! symfony/css-selector v8.1.6, on upstream's provider rows.
//!
//! Upstream: symfony/css-selector Tests/Parser/Handler/WhitespaceHandlerTest.php Tests/Parser/Handler/AbstractHandlerTestCase.php
#[allow(unused_imports)]
use symfony_css_selector::parser::handler::*;

#[allow(unused_imports)]
use crate::{common::provider, escaping, patterns};

const CLASS: &str = "Parser/Handler/WhitespaceHandlerTest";

fn generate_handler() -> Box<dyn HandlerInterface> {
    Box::new(WhitespaceHandler)
}

#[test]
fn testHandleValue() {
    crate::handle_value(
        generate_handler().as_ref(),
        &provider(CLASS, "testHandleValue"),
    );
}

#[test]
fn testDontHandleValue() {
    crate::dont_handle_value(
        generate_handler().as_ref(),
        &provider(CLASS, "testDontHandleValue"),
    );
}
