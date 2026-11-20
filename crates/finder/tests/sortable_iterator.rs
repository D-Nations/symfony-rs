//! SortableIteratorTest.php at symfony/finder
//! 2f4c0f26106520d82e1b71759d2fb8d32d8b7a43. Names and assertions are
//! PHPUnit's; the provider's rows (sort constants and a closure) are
//! written out and run in upstream's order on one fixture tree, since the
//! time-based rows change the files the later rows read. As upstream, the
//! accessed- and changed-time rows are not checked on Windows (Windows
//! has no inode change time, and its access times are not updated the
//! same way).
//!
//! Upstream: symfony/finder Tests/Iterator/SortableIteratorTest.php
#![allow(non_snake_case)]

mod common;

use std::{fs, sync::Arc, thread::sleep, time::Duration};

use common::fixture::{
    FILES, assert_ordered_iterator, assert_ordered_iterator_for_groups, infos, real_fixture,
    to_absolute, touch,
};
use symfony_finder::{
    SplFileInfo,
    iterator::{Sort, SortableIterator},
    php::strcmp,
};

#[test]
fn testConstructor() {
    let e = Sort::from_int(-255).err();
    assert!(
        e.is_some_and(|e| e.is_instance_of("InvalidArgumentException")),
        "__construct() throws an \\InvalidArgumentException exception if the mode is not valid"
    );
}

fn abs(tmp: &str, files: &[&str]) -> Vec<String> {
    to_absolute(tmp, files)
}

#[test]
fn testAccept() {
    let tmp = real_fixture("SortableIteratorTest", false);
    let sort_by_name = [
        ".bar",
        ".foo",
        ".foo/.bar",
        ".foo/bar",
        ".git",
        "Zephire.php",
        "foo",
        "foo bar",
        "foo/bar.tmp",
        "qux",
        "qux/baz_100_1.py",
        "qux/baz_1_2.py",
        "qux_0_1.php",
        "qux_1000_1.php",
        "qux_1002_0.php",
        "qux_10_2.php",
        "qux_12_0.php",
        "qux_2_0.php",
        "test.php",
        "test.py",
        "toto",
        "toto/.git",
        "zebulon.php",
    ];
    let sort_by_type = [
        ".foo",
        ".git",
        "foo",
        "qux",
        "toto",
        "toto/.git",
        ".bar",
        ".foo/.bar",
        ".foo/bar",
        "Zephire.php",
        "foo bar",
        "foo/bar.tmp",
        "qux/baz_100_1.py",
        "qux/baz_1_2.py",
        "qux_0_1.php",
        "qux_1000_1.php",
        "qux_1002_0.php",
        "qux_10_2.php",
        "qux_12_0.php",
        "qux_2_0.php",
        "test.php",
        "test.py",
        "zebulon.php",
    ];
    let sort_by_accessed_time: Vec<Vec<&str>> = vec![
        // For these two files the access time was set to 2005-10-15
        vec!["foo/bar.tmp", "test.php"],
        // These files were created more or less at the same time
        vec![
            ".git",
            ".foo",
            ".foo/.bar",
            ".foo/bar",
            "test.py",
            "Zephire.php",
            "foo",
            "toto",
            "toto/.git",
            "foo bar",
            "qux",
            "qux/baz_100_1.py",
            "qux/baz_1_2.py",
            "qux_0_1.php",
            "qux_1000_1.php",
            "qux_1002_0.php",
            "qux_10_2.php",
            "qux_12_0.php",
            "qux_2_0.php",
            "zebulon.php",
        ],
        // This file was accessed after sleeping for 1 sec
        vec![".bar"],
    ];
    let same_time = vec![
        ".git",
        ".foo",
        ".foo/.bar",
        ".foo/bar",
        ".bar",
        "Zephire.php",
        "foo",
        "foo/bar.tmp",
        "toto",
        "toto/.git",
        "foo bar",
        "qux",
        "qux/baz_100_1.py",
        "qux/baz_1_2.py",
        "qux_0_1.php",
        "qux_1000_1.php",
        "qux_1002_0.php",
        "qux_10_2.php",
        "qux_12_0.php",
        "qux_2_0.php",
        "zebulon.php",
    ];
    let sort_by_changed_time = vec![same_time.clone(), vec!["test.php"], vec!["test.py"]];
    let sort_by_modified_time = vec![same_time, vec!["test.php"], vec!["test.py"]];
    let sort_by_name_natural = [
        ".bar",
        ".foo",
        ".foo/.bar",
        ".foo/bar",
        ".git",
        "Zephire.php",
        "foo",
        "foo/bar.tmp",
        "foo bar",
        "qux",
        "qux/baz_1_2.py",
        "qux/baz_100_1.py",
        "qux_0_1.php",
        "qux_2_0.php",
        "qux_10_2.php",
        "qux_12_0.php",
        "qux_1000_1.php",
        "qux_1002_0.php",
        "test.php",
        "test.py",
        "toto",
        "toto/.git",
        "zebulon.php",
    ];
    let custom_comparison = sort_by_name;

    let inner = || infos(&to_absolute(&tmp, &FILES));
    let groups = |g: &[Vec<&str>]| g.iter().map(|g| abs(&tmp, g)).collect::<Vec<_>>();
    let write =
        |file: &str| fs::write(format!("{tmp}/{file}"), "foo").unwrap_or_else(|e| panic!("{e}"));

    // SORT_BY_NAME
    assert_ordered_iterator(
        abs(&tmp, &sort_by_name),
        SortableIterator::new(inner(), Sort::Name, false).get_iterator(),
    );
    // SORT_BY_TYPE
    assert_ordered_iterator(
        abs(&tmp, &sort_by_type),
        SortableIterator::new(inner(), Sort::Type, false).get_iterator(),
    );
    // SORT_BY_ACCESSED_TIME
    touch(&format!("{tmp}/.git"), Some(std::time::SystemTime::now()));
    sleep(Duration::from_secs(1));
    touch(&format!("{tmp}/.bar"), Some(std::time::SystemTime::now()));
    if !cfg!(windows) {
        assert_ordered_iterator_for_groups(
            groups(&sort_by_accessed_time),
            SortableIterator::new(inner(), Sort::AccessedTime, false).get_iterator(),
        );
    }
    // SORT_BY_CHANGED_TIME
    sleep(Duration::from_secs(1));
    write("test.php");
    sleep(Duration::from_secs(1));
    write("test.py");
    if !cfg!(windows) {
        assert_ordered_iterator_for_groups(
            groups(&sort_by_changed_time),
            SortableIterator::new(inner(), Sort::ChangedTime, false).get_iterator(),
        );
    }
    // SORT_BY_MODIFIED_TIME
    write("test.php");
    sleep(Duration::from_secs(1));
    write("test.py");
    assert_ordered_iterator_for_groups(
        groups(&sort_by_modified_time),
        SortableIterator::new(inner(), Sort::ModifiedTime, false).get_iterator(),
    );
    // SORT_BY_NAME_NATURAL
    assert_ordered_iterator(
        abs(&tmp, &sort_by_name_natural),
        SortableIterator::new(inner(), Sort::NameNatural, false).get_iterator(),
    );
    // a callback
    let callback = Sort::Callback(Arc::new(|a: &SplFileInfo, b: &SplFileInfo| {
        strcmp(
            &a.get_real_path().unwrap_or_default(),
            &b.get_real_path().unwrap_or_default(),
        )
    }));
    assert_ordered_iterator(
        abs(&tmp, &custom_comparison),
        SortableIterator::new(inner(), callback, false).get_iterator(),
    );
    common::fixture::remove_dir(&tmp);
}
