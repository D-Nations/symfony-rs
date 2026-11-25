//! Port of Symfony\Component\DependencyInjection\Tests\Argument\RewindableGeneratorTest
//! from Tests/Argument/RewindableGeneratorTest.php at
//! symfony/dependency-injection 9b34cad8. Names and assertions are
//! PHPUnit's; `assertCount()` is `count()`.
//!
//! Upstream: symfony/dependency-injection Tests/Argument/RewindableGeneratorTest.php
#![allow(non_snake_case, clippy::unwrap_used)]

use std::sync::Arc;
use std::sync::atomic::{AtomicI64, Ordering};

use php_value::Value;
use symfony_dependency_injection::argument::RewindableGenerator;
use symfony_dependency_injection::argument::rewindable_generator::Generator;

fn yield_one() -> Generator {
    Box::new(std::iter::once(Ok((Value::Int(0), Value::Int(1)))))
}

#[test]
fn testImplementsCountable() {
    let generator = RewindableGenerator::new(yield_one, 1);
    assert!(RewindableGenerator::instance_of("Countable"));
    assert_eq!(1, generator.count().unwrap());
}

#[test]
fn testCountUsesProvidedValue() {
    let generator = RewindableGenerator::new(yield_one, 3);

    assert_eq!(3, generator.count().unwrap());
}

#[test]
fn testCountUsesProvidedValueAsCallback() {
    let called = Arc::new(AtomicI64::new(0));
    let generator = RewindableGenerator::with_count_callback(yield_one, {
        let called = called.clone();
        move || {
            called.fetch_add(1, Ordering::SeqCst);
            Ok(3)
        }
    });

    assert_eq!(
        0,
        called.load(Ordering::SeqCst),
        "Count callback is called lazily"
    );
    assert_eq!(3, generator.count().unwrap());

    generator.count().unwrap();

    assert_eq!(
        1,
        called.load(Ordering::SeqCst),
        "Count callback is called only once"
    );
}
