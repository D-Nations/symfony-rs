//! ComparatorTest.php at symfony/finder
//! 2f4c0f26106520d82e1b71759d2fb8d32d8b7a43. Names and assertions are
//! PHPUnit's; provider rows are replayed from tests/data/providers.json.
//!
//! Upstream: symfony/finder Tests/Comparator/ComparatorTest.php
#![allow(non_snake_case)]

mod common;

use common::{TestResult, provider, string};
use symfony_finder::Comparator;

#[test]
fn testInvalidOperator() {
    let e = Comparator::new("some target", "foo").err();
    assert!(
        e.as_ref()
            .is_some_and(|e| e.is_instance_of("InvalidArgumentException"))
    );
    assert_eq!(
        e.map(|e| e.message().to_string()).as_deref(),
        Some("Invalid operator \"foo\".")
    );
}

#[test]
fn testTestSucceeds() -> TestResult {
    let rows = provider("ComparatorTest", "testTestSucceeds");
    assert_eq!(rows.len(), 10);
    for (_, args) in rows {
        let (operator, target, tested) = (string(&args[0]), string(&args[1]), string(&args[2]));
        let c = Comparator::new(target.as_str(), &operator)?;
        assert_eq!(c.get_target(), target.as_bytes());
        assert_eq!(c.get_operator(), operator);
        assert!(c.test(tested.as_str()), "{operator} {target} {tested}");
    }
    Ok(())
}

#[test]
fn testTestFails() -> TestResult {
    let rows = provider("ComparatorTest", "testTestFails");
    assert_eq!(rows.len(), 7);
    for (_, args) in rows {
        let (operator, target, tested) = (string(&args[0]), string(&args[1]), string(&args[2]));
        let c = Comparator::new(target.as_str(), &operator)?;
        assert!(!c.test(tested.as_str()), "{operator} {target} {tested}");
    }
    Ok(())
}
