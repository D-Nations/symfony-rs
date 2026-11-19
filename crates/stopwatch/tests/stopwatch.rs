//! StopwatchTest.php at symfony/stopwatch
//! 21c07b026905d596e8379caeb115d87aa479499d. Names and assertions are
//! PHPUnit's. Like upstream (not in the `time-sensitive` group), these run
//! on the real clock.
//!
//! testIsNotStartedEvent injects an unstarted event by reflection; here
//! the same state (a known event with no running period) comes from
//! starting and stopping it.
//!
//! Upstream: symfony/stopwatch Tests/StopwatchTest.php
#![allow(non_snake_case)]

use std::{thread::sleep, time::Duration};

use symfony_stopwatch::Stopwatch;

type TestResult = Result<(), Box<dyn std::error::Error>>;

const DELTA: f64 = 20.0;

#[test]
fn testStart() -> TestResult {
    let stopwatch = Stopwatch::new(false);
    let event = stopwatch.start("foo", Some("cat"));

    assert_eq!("cat", event.get_category());
    assert!(event.same(&stopwatch.get_event("foo")?));
    Ok(())
}

#[test]
fn testStartWithoutCategory() -> TestResult {
    let stopwatch = Stopwatch::new(false);
    let stopwatch_event = stopwatch.start("bar", None);
    assert_eq!("default", stopwatch_event.get_category());
    assert!(stopwatch_event.same(&stopwatch.get_event("bar")?));
    Ok(())
}

#[test]
fn testIsStarted() {
    let stopwatch = Stopwatch::new(false);
    stopwatch.start("foo", Some("cat"));

    assert!(stopwatch.is_started("foo"));
}

#[test]
fn testIsNotStarted() {
    let stopwatch = Stopwatch::new(false);

    assert!(!stopwatch.is_started("foo"));
}

#[test]
fn testIsNotStartedEvent() -> TestResult {
    let stopwatch = Stopwatch::new(false);

    stopwatch.start("foo", None);
    stopwatch.stop("foo")?;

    assert!(!stopwatch.is_started("foo"));
    Ok(())
}

#[test]
fn testStop() -> TestResult {
    let stopwatch = Stopwatch::new(false);
    stopwatch.start("foo", Some("cat"));
    sleep(Duration::from_micros(200000));
    let event = stopwatch.stop("foo")?;

    let duration = event.get_duration().as_f64();
    assert!((200.0 - duration).abs() <= DELTA, "{duration}");
    Ok(())
}

#[test]
fn testUnknownEvent() {
    assert!(Stopwatch::new(false).get_event("foo").is_err());
}

#[test]
fn testStopWithoutStart() {
    assert!(Stopwatch::new(false).stop("foo").is_err());
}

#[test]
fn testMorePrecision() -> TestResult {
    let stopwatch = Stopwatch::new(true);

    stopwatch.start("foo", None);
    let event = stopwatch.stop("foo")?;

    assert!(event.get_start_time().is_float());
    assert!(event.get_end_time().is_float());
    assert!(event.get_duration().is_float());
    Ok(())
}

#[test]
fn testSection() -> TestResult {
    let stopwatch = Stopwatch::new(false);

    stopwatch.open_section(None)?;
    stopwatch.start("foo", Some("cat"));
    stopwatch.stop("foo")?;
    stopwatch.start("bar", Some("cat"));
    stopwatch.stop("bar")?;
    stopwatch.stop_section("1")?;

    stopwatch.open_section(None)?;
    stopwatch.start("foobar", Some("cat"));
    stopwatch.stop("foobar")?;
    stopwatch.stop_section("2")?;

    stopwatch.open_section(None)?;
    stopwatch.start("foobar", Some("cat"));
    stopwatch.stop("foobar")?;
    stopwatch.stop_section("0")?;

    // the section is an event by itself
    assert_eq!(3, stopwatch.get_section_events("1").len());
    assert_eq!(2, stopwatch.get_section_events("2").len());
    assert_eq!(2, stopwatch.get_section_events("0").len());
    Ok(())
}

#[test]
fn testReopenASection() -> TestResult {
    let stopwatch = Stopwatch::new(false);

    stopwatch.open_section(None)?;
    stopwatch.start("foo", Some("cat"));
    stopwatch.stop_section("section")?;

    stopwatch.open_section(Some("section"))?;
    stopwatch.start("bar", Some("cat"));
    stopwatch.stop_section("section")?;

    let events = stopwatch.get_section_events("section");

    assert_eq!(3, events.len());
    let section = events
        .iter()
        .find(|(name, _)| name == "__section__")
        .ok_or("no __section__ event")?;
    assert_eq!(2, section.1.get_periods().len());
    Ok(())
}

#[test]
fn testLap() -> TestResult {
    let stopwatch = Stopwatch::new(false);

    stopwatch.start("foo", None);
    stopwatch.lap("foo")?;
    stopwatch.stop("foo")?;

    let event = stopwatch.get_event("foo")?;

    assert_eq!(2, event.get_periods().len());
    Ok(())
}

#[test]
fn testReopenANewSectionShouldThrowAnException() {
    assert!(Stopwatch::new(false).open_section(Some("section")).is_err());
}

#[test]
fn testReset() -> TestResult {
    let stopwatch = Stopwatch::new(false);

    stopwatch.open_section(None)?;
    stopwatch.start("foo", Some("cat"));

    stopwatch.reset();

    assert_eq!(Stopwatch::new(false), stopwatch);
    Ok(())
}

#[test]
fn testShouldReturnEmptyArrayWhenSectionMissing() {
    assert!(
        Stopwatch::new(false)
            .get_section_events("missing")
            .is_empty()
    );
}
