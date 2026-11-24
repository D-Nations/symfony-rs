//! Unsupported or invalid input is refused at build time with the
//! template name and line, never approximated (AGENTS.md).
//!
//! Upstream: none

use twig_trial_compiler::{Callable, Environment, compile};

fn env() -> Environment {
    Environment {
        functions: vec![Callable::new("path", "crate::ext::path", false)],
        filters: Vec::new(),
        component_pre_lexer: true,
    }
}

fn refusal(source: &str) -> String {
    match compile(&env(), &[("t.html.twig".to_owned(), source.to_owned())]) {
        Ok(_) => panic!("compiled: {source:?}"),
        Err(e) => e.to_string(),
    }
}

#[test]
fn unsupported_tags_are_refused_with_their_line() {
    assert_eq!(
        refusal("a\n{% block b %}{% endblock %}"),
        "The Twig trial compiler does not support the \"block\" tag in \"t.html.twig\" at line 2."
    );
    assert_eq!(
        refusal("\n\n{% extends 'base.html.twig' %}"),
        "The Twig trial compiler does not support the \"extends\" tag in \"t.html.twig\" at line 3."
    );
    assert_eq!(
        refusal("{% verbatim %}x{% endverbatim %}"),
        "The Twig trial compiler does not support the \"verbatim\" tag in \"t.html.twig\" at line 1."
    );
}

#[test]
fn unsupported_expressions_are_refused() {
    assert_eq!(
        refusal("{{ a is defined }}"),
        "The Twig trial compiler does not support the \"is\" operator in \"t.html.twig\" at line 1."
    );
    assert_eq!(
        refusal("{{ \"#{a}\" }}"),
        "The Twig trial compiler does not support string interpolation in \"t.html.twig\" at line 1."
    );
    assert_eq!(
        refusal("{{ path('a', name: 'b') }}"),
        "The Twig trial compiler does not support named arguments in \"t.html.twig\" at line 1."
    );
}

#[test]
fn unknown_callables_use_twigs_message() {
    assert_eq!(
        refusal("\n{{ url('x') }}"),
        "Unknown \"url\" function in \"t.html.twig\" at line 2."
    );
    assert_eq!(
        refusal("{{ 'x'|trans }}"),
        "Unknown \"trans\" filter in \"t.html.twig\" at line 1."
    );
}

#[test]
fn syntax_errors_are_located() {
    assert_eq!(
        refusal("{% if a %}x"),
        "Unexpected end of template. Twig was looking for the following tags \"elseif\", \"else\", \"endif\" in \"t.html.twig\" at line 1."
    );
    assert_eq!(
        refusal("{{ (a }}"),
        "Unclosed \"(\" in \"t.html.twig\" at line 1."
    );
}

#[test]
fn components_with_content_are_refused() {
    assert_eq!(
        refusal("<twig:Alert>hi</twig:Alert>"),
        "The Twig trial compiler does not support components with content (\"<twig:Alert>\") in \"t.html.twig\" at line 1."
    );
}

#[test]
fn other_escaping_strategies_are_refused() {
    let err = compile(&env(), &[("app.js.twig".to_owned(), "x".to_owned())]).expect_err("js");
    assert_eq!(
        err.to_string(),
        "The Twig trial compiler supports the \"html\" escaping strategy only in \"app.js.twig\" at line 1."
    );
}
