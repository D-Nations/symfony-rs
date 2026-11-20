//! CommentHandlerTest.php (through AbstractHandlerTestCase.php) at
//! symfony/css-selector v8.1.6, on upstream's provider rows.
//!
//! Upstream: symfony/css-selector Tests/Parser/Handler/CommentHandlerTest.php Tests/Parser/Handler/AbstractHandlerTestCase.php
#[allow(unused_imports)]
use symfony_css_selector::parser::handler::*;

#[allow(unused_imports)]
use crate::{common::provider, escaping, patterns};

const CLASS: &str = "Parser/Handler/CommentHandlerTest";

fn generate_handler() -> Box<dyn HandlerInterface> {
    Box::new(CommentHandler)
}

#[test]
fn testHandleValue() {
    // comments are ignored (not pushed as token in stream); the 2nd
    // argument only exists for inherited method compatibility
    crate::handle_value_without_token(
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
