//! The filter iterator tests of symfony/finder
//! 2f4c0f26106520d82e1b71759d2fb8d32d8b7a43 (Tests/Iterator/*Test.php):
//! `CustomFilter`, `DateRange`, `DepthRange`, `ExcludeDirectory`, `FileType`,
//! Filecontent, Filename, Lazy, `MultiplePcre`, Path and `SizeRange`. Names
//! and assertions are PHPUnit's; each test is named
//! `<Class>_<method>`. `FilenameFilterIteratorTest` and
//! `MultiplePcreFilterIteratorTest` replay their recorded providers; the
//! others' providers hold closures, comparators or mock objects
//! (`MockSplFileInfo`, below) and are written out here. The real-file
//! tests run on `RealIteratorTestCase`'s tree, one per test class.
//!
//! Upstream: symfony/finder Tests/Iterator/CustomFilterIteratorTest.php Tests/Iterator/DateRangeFilterIteratorTest.php Tests/Iterator/DepthRangeFilterIteratorTest.php Tests/Iterator/ExcludeDirectoryFilterIteratorTest.php Tests/Iterator/FileTypeFilterIteratorTest.php Tests/Iterator/FilecontentFilterIteratorTest.php Tests/Iterator/FilenameFilterIteratorTest.php Tests/Iterator/LazyIteratorTest.php Tests/Iterator/MultiplePcreFilterIteratorTest.php Tests/Iterator/PathFilterIteratorTest.php Tests/Iterator/SizeRangeFilterIteratorTest.php Tests/Iterator/MockSplFileInfo.php Tests/Iterator/RealIteratorTestCase.php Tests/Iterator/IteratorTestCase.php
#![allow(non_snake_case)]

mod common;

use std::sync::{
    Arc, OnceLock,
    atomic::{AtomicUsize, Ordering},
};

use common::{
    TestResult,
    fixture::{assert_iterator, infos, real_fixture, symlink, temp_root, to_absolute},
    provider, strings,
};
use php_value::Value;
use symfony_finder::{
    DateComparator, Error, FileInfo, NumberComparator, SplFileInfo,
    iterator::{
        CustomFilterIterator, DateRangeFilterIterator, DepthRangeFilterIterator,
        ExcludeDirectoryFilterIterator, Exclusion, FileTypeFilterIterator,
        FilecontentFilterIterator, FilenameFilterIterator, Filter, Flags, LazyIterator, Mode,
        MultiplePcreFilter, PathFilterIterator, RecursiveDirectoryIterator,
        RecursiveIteratorIterator, SizeRangeFilterIterator, files,
    },
};

fn tmp() -> &'static str {
    static TMP: OnceLock<String> = OnceLock::new();
    TMP.get_or_init(|| real_fixture("IteratorFilters", false))
}

fn all_files() -> Vec<String> {
    common::fixture::FILES
        .iter()
        .map(|f| format!("{}/{}", tmp(), f.trim_end_matches('/')))
        .collect()
}

fn bytes_list(v: &[&str]) -> Vec<Vec<u8>> {
    v.iter().map(|s| s.as_bytes().to_vec()).collect()
}

// --- CustomFilterIteratorTest ---------------------------------------------

#[test]
fn CustomFilterIteratorTest_testAccept() {
    let rows: Vec<(Vec<Filter<SplFileInfo>>, Vec<&str>)> = vec![
        (vec![Arc::new(|_: &SplFileInfo| false)], vec![]),
        (
            vec![Arc::new(|f: &SplFileInfo| {
                f.get_pathname().starts_with(b"test")
            })],
            vec!["test.php", "test.py"],
        ),
        (vec![Arc::new(|f: &SplFileInfo| f.is_dir())], vec![]),
    ];
    for (filters, expected) in rows {
        let inner = infos(&["test.php", "test.py", "foo.php"].map(String::from));
        let iterator = CustomFilterIterator::new(inner, filters);
        assert_iterator(
            expected
                .iter()
                .map(std::string::ToString::to_string)
                .collect(),
            iterator,
        );
    }
}

// --- DateRangeFilterIteratorTest ------------------------------------------

#[test]
fn DateRangeFilterIteratorTest_testAccept() -> TestResult {
    let since_20_years_ago = [
        ".git",
        "test.py",
        "foo",
        "foo/bar.tmp",
        "test.php",
        "toto",
        "toto/.git",
        ".bar",
        ".foo",
        ".foo/.bar",
        "foo bar",
        ".foo/bar",
        "qux",
        "qux/baz_100_1.py",
        "zebulon.php",
        "Zephire.php",
        "qux/baz_1_2.py",
        "qux_0_1.php",
        "qux_1000_1.php",
        "qux_1002_0.php",
        "qux_10_2.php",
        "qux_12_0.php",
        "qux_2_0.php",
    ];
    let since_2_months_ago = [
        ".git",
        "test.py",
        "foo",
        "toto",
        "toto/.git",
        ".bar",
        ".foo",
        ".foo/.bar",
        "foo bar",
        ".foo/bar",
        "qux",
        "qux/baz_100_1.py",
        "zebulon.php",
        "Zephire.php",
        "qux/baz_1_2.py",
        "qux_0_1.php",
        "qux_1000_1.php",
        "qux_1002_0.php",
        "qux_10_2.php",
        "qux_12_0.php",
        "qux_2_0.php",
    ];
    let until_last_month = ["foo/bar.tmp", "test.php"];
    let rows: Vec<(&[u8], &[&str])> = vec![
        (b"since 20 years ago", &since_20_years_ago),
        (b"since 2 months ago", &since_2_months_ago),
        (b"until last month", &until_last_month),
    ];
    for (test, expected) in rows {
        let mut files = all_files();
        files.push(format!("{}/doesnotexist", tmp()));
        let iterator =
            DateRangeFilterIterator::new(infos(&files), vec![DateComparator::new(test)?]);
        assert_iterator(to_absolute(tmp(), expected), iterator);
    }
    Ok(())
}

// --- DepthRangeFilterIteratorTest -----------------------------------------

#[test]
fn DepthRangeFilterIteratorTest_testAccept() -> TestResult {
    let less_than_1 = [
        ".git",
        "test.py",
        "foo",
        "test.php",
        "toto",
        ".foo",
        ".bar",
        "foo bar",
        "qux",
        "qux_0_1.php",
        "qux_1000_1.php",
        "qux_1002_0.php",
        "qux_10_2.php",
        "qux_12_0.php",
        "qux_2_0.php",
        "zebulon.php",
        "Zephire.php",
    ];
    let less_than_or_equal_to_1 = [
        ".git",
        "test.py",
        "foo",
        "foo/bar.tmp",
        "test.php",
        "toto",
        "toto/.git",
        ".foo",
        ".foo/.bar",
        ".bar",
        "foo bar",
        ".foo/bar",
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
        "Zephire.php",
    ];
    let grater_than_or_equal_to_1 = [
        "toto/.git",
        "foo/bar.tmp",
        ".foo/.bar",
        ".foo/bar",
        "qux/baz_100_1.py",
        "qux/baz_1_2.py",
    ];
    let equal_to_1 = grater_than_or_equal_to_1;
    let rows: Vec<(i64, i64, &[&str])> = vec![
        (0, 0, &less_than_1),
        (0, 1, &less_than_or_equal_to_1),
        (2, i64::MAX, &[]),
        (1, i64::MAX, &grater_than_or_equal_to_1),
        (1, 1, &equal_to_1),
    ];
    for (min, max, expected) in rows {
        let inner = RecursiveIteratorIterator::new(RecursiveDirectoryIterator::new(
            tmp().as_bytes(),
            Flags::default(),
            false,
        )?);
        let iterator = DepthRangeFilterIterator::new(inner, min, max)?;
        // array_keys(iterator_to_array($iterator)): the pathnames
        assert_iterator(to_absolute(tmp(), expected), iterator);
    }
    Ok(())
}

// --- ExcludeDirectoryFilterIteratorTest -----------------------------------

#[test]
fn ExcludeDirectoryFilterIteratorTest_testAccept() -> TestResult {
    let foo = [
        ".bar",
        ".foo",
        ".foo/.bar",
        ".foo/bar",
        ".git",
        "test.py",
        "test.php",
        "toto",
        "toto/.git",
        "foo bar",
        "qux",
        "qux/baz_100_1.py",
        "zebulon.php",
        "Zephire.php",
        "qux/baz_1_2.py",
        "qux_0_1.php",
        "qux_1000_1.php",
        "qux_1002_0.php",
        "qux_10_2.php",
        "qux_12_0.php",
        "qux_2_0.php",
    ];
    let fo = [
        ".bar",
        ".foo",
        ".foo/.bar",
        ".foo/bar",
        ".git",
        "test.py",
        "foo",
        "foo/bar.tmp",
        "test.php",
        "toto",
        "toto/.git",
        "foo bar",
        "qux",
        "qux/baz_100_1.py",
        "zebulon.php",
        "Zephire.php",
        "qux/baz_1_2.py",
        "qux_0_1.php",
        "qux_1000_1.php",
        "qux_1002_0.php",
        "qux_10_2.php",
        "qux_12_0.php",
        "qux_2_0.php",
    ];
    let toto = [
        ".bar",
        ".foo",
        ".foo/.bar",
        ".foo/bar",
        ".git",
        "test.py",
        "foo",
        "foo/bar.tmp",
        "test.php",
        "foo bar",
        "qux",
        "qux/baz_100_1.py",
        "zebulon.php",
        "Zephire.php",
        "qux/baz_1_2.py",
        "qux_0_1.php",
        "qux_1000_1.php",
        "qux_1002_0.php",
        "qux_10_2.php",
        "qux_12_0.php",
        "qux_2_0.php",
    ];
    let rows: Vec<(&str, &[&str])> = vec![("foo", &foo), ("fo", &fo), ("toto/", &toto)];
    for (directory, expected) in rows {
        let inner = ExcludeDirectoryFilterIterator::new(
            RecursiveDirectoryIterator::new(tmp().as_bytes(), Flags::default(), false)?,
            vec![Exclusion::Directory(directory.as_bytes().to_vec())],
        )?;
        let iterator = files(RecursiveIteratorIterator::new(inner));
        assert_iterator(to_absolute(tmp(), expected), iterator);
    }
    Ok(())
}

// --- FileTypeFilterIteratorTest -------------------------------------------

#[test]
fn FileTypeFilterIteratorTest_testAccept() {
    let only_files = [
        "test.py",
        "foo/bar.tmp",
        "test.php",
        ".bar",
        ".foo/.bar",
        ".foo/bar",
        "foo bar",
        "qux/baz_100_1.py",
        "zebulon.php",
        "Zephire.php",
        "qux/baz_1_2.py",
        "qux_0_1.php",
        "qux_1000_1.php",
        "qux_1002_0.php",
        "qux_10_2.php",
        "qux_12_0.php",
        "qux_2_0.php",
    ];
    let only_directories = [".git", "foo", "qux", "toto", "toto/.git", ".foo"];
    for (mode, expected) in [
        (Mode::OnlyFiles, &only_files[..]),
        (Mode::OnlyDirectories, &only_directories[..]),
    ] {
        let iterator = FileTypeFilterIterator::new(infos(&all_files()), mode);
        assert_iterator(to_absolute(tmp(), expected), iterator);
    }
}

#[test]
fn FileTypeFilterIteratorTest_testDanglingSymlinkIsNeitherAFileNorADirectory() {
    let dir = format!("{}/symfony_finder_dangling_symlink", temp_root());
    common::fixture::remove_dir(&dir);
    std::fs::create_dir(&dir).unwrap_or_else(|e| panic!("{e}"));
    common::fixture::touch(&format!("{dir}/file.txt"), None);
    std::fs::create_dir(format!("{dir}/dir")).unwrap_or_else(|e| panic!("{e}"));
    if symlink(&format!("{dir}/missing"), &format!("{dir}/dangling")).is_none() {
        eprintln!("symbolic links cannot be created on this system: test not run");
        common::fixture::remove_dir(&dir);
        return;
    }
    let inner = [
        format!("{dir}/file.txt"),
        format!("{dir}/dir"),
        format!("{dir}/dangling"),
    ];
    assert_iterator(
        vec![format!("{dir}/file.txt")],
        FileTypeFilterIterator::new(infos(&inner), Mode::OnlyFiles),
    );
    assert_iterator(
        vec![format!("{dir}/dir")],
        FileTypeFilterIterator::new(infos(&inner), Mode::OnlyDirectories),
    );
    common::fixture::remove_dir(&dir);
}

// --- MockSplFileInfo ------------------------------------------------------

/// Tests/Iterator/MockSplFileInfo.php: a file whose type, mode, contents
/// and relative paths are given, not read.
#[derive(Default, Clone)]
struct MockSplFileInfo {
    name: String,
    contents: Option<String>,
    mode: Option<String>,
    is_dir: Option<bool>,
    relative_pathname: Option<String>,
}

impl MockSplFileInfo {
    fn named(name: &str) -> Self {
        Self {
            name: name.to_string(),
            ..Self::default()
        }
    }

    fn filename(&self) -> &str {
        self.name.rsplit('/').next().unwrap_or(&self.name)
    }
}

impl FileInfo for MockSplFileInfo {
    fn get_pathname(&self) -> &[u8] {
        self.name.as_bytes()
    }
    fn get_filename(&self) -> &[u8] {
        self.filename().as_bytes()
    }
    fn get_relative_path(&self) -> &[u8] {
        b""
    }
    fn get_relative_pathname(&self) -> &[u8] {
        self.relative_pathname
            .as_deref()
            .unwrap_or_default()
            .as_bytes()
    }
    fn is_dir(&self) -> bool {
        self.is_dir
            .unwrap_or_else(|| self.filename().contains("directory"))
    }
    fn is_file(&self) -> bool {
        self.is_dir
            .map_or_else(|| self.filename().contains("file"), |d| !d)
    }
    fn is_link(&self) -> bool {
        false
    }
    fn is_readable(&self) -> bool {
        self.mode.as_deref().unwrap_or(&self.name).contains("r+")
    }
    fn exists(&self) -> bool {
        true
    }
    fn get_size(&self) -> Result<i64, Error> {
        Ok(0)
    }
    fn get_m_time(&self) -> Result<i64, Error> {
        Ok(0)
    }
    fn get_contents(&self) -> Result<Option<Vec<u8>>, Error> {
        Ok(self.contents.clone().map(String::into_bytes))
    }
}

fn mocks(names: &[&str]) -> std::vec::IntoIter<Result<MockSplFileInfo, Error>> {
    names
        .iter()
        .map(|n| Ok(MockSplFileInfo::named(n)))
        .collect::<Vec<_>>()
        .into_iter()
}

fn strs(v: &[&str]) -> Vec<String> {
    v.iter().map(std::string::ToString::to_string).collect()
}

// --- FilecontentFilterIteratorTest ----------------------------------------

#[test]
fn FilecontentFilterIteratorTest_testAccept() -> TestResult {
    let iterator = FilecontentFilterIterator::new(mocks(&["test.txt"]), &[], &[])?;
    assert_iterator(strs(&["test.txt"]), iterator);
    Ok(())
}

#[test]
fn FilecontentFilterIteratorTest_testDirectory() -> TestResult {
    let iterator =
        FilecontentFilterIterator::new(mocks(&["directory"]), &bytes_list(&["directory"]), &[])?;
    assert_iterator(Vec::new(), iterator);
    Ok(())
}

#[test]
fn FilecontentFilterIteratorTest_testUnreadableFile() -> TestResult {
    let iterator =
        FilecontentFilterIterator::new(mocks(&["file r-"]), &bytes_list(&["file r-"]), &[])?;
    assert_iterator(Vec::new(), iterator);
    Ok(())
}

#[test]
fn FilecontentFilterIteratorTest_testFilter() -> TestResult {
    let file = |name: &str, contents: &str| MockSplFileInfo {
        name: name.into(),
        contents: Some(contents.into()),
        is_dir: Some(false),
        mode: Some("r+".into()),
        ..MockSplFileInfo::default()
    };
    let inner = vec![
        file("a.txt", "Lorem ipsum..."),
        file("b.yml", "dolor sit..."),
        file("some/other/dir/third.php", "amet..."),
        file("unreadable-file.txt", ""),
    ];
    let rows: Vec<(&[&str], &[&str], &[&str])> = vec![
        (&["."], &[], &["a.txt", "b.yml", "some/other/dir/third.php"]),
        (&["ipsum"], &[], &["a.txt"]),
        (&["i", "amet"], &["Lorem", "amet"], &["b.yml"]),
        (
            &["..."],
            &[],
            &["a.txt", "b.yml", "some/other/dir/third.php"],
        ),
    ];
    for (matches, no_matches, expected) in rows {
        let iterator = FilecontentFilterIterator::new(
            inner.clone().into_iter().map(Ok),
            &bytes_list(matches),
            &bytes_list(no_matches),
        )?;
        assert_iterator(strs(expected), iterator);
    }
    Ok(())
}

// --- FilenameFilterIteratorTest -------------------------------------------

fn byte_args(v: &Value) -> Vec<Vec<u8>> {
    strings(v).into_iter().map(String::into_bytes).collect()
}

#[test]
fn FilenameFilterIteratorTest_testAccept() -> TestResult {
    let rows = provider("FilenameFilterIteratorTest", "testAccept");
    assert_eq!(rows.len(), 7);
    for (_, args) in rows {
        let inner = infos(&strs(&["test.php", "test.py", "foo.php"]));
        let iterator =
            FilenameFilterIterator::new(inner, &byte_args(&args[0]), &byte_args(&args[1]))?;
        assert_iterator(strings(&args[2]), iterator);
    }
    Ok(())
}

// --- LazyIteratorTest -----------------------------------------------------

#[test]
fn LazyIteratorTest_testLazy() {
    let _ = LazyIterator::new(|| -> std::vec::IntoIter<()> {
        panic!("lazyIterator should not be called")
    });
}

#[test]
fn LazyIteratorTest_testDelegate() {
    let iterator = LazyIterator::new(|| infos(&strs(&["foo", "bar"])));
    assert_eq!(2, iterator.get_iterator().count());
}

#[test]
fn LazyIteratorTest_testInnerDestructedAtTheEnd() {
    let count = Arc::new(AtomicUsize::new(0));
    let counter = Arc::clone(&count);
    let iterator = LazyIterator::new(move || {
        counter.fetch_add(1, Ordering::SeqCst);
        infos(&strs(&["foo", "bar"]))
    });
    for _ in iterator.get_iterator() {}
    assert_eq!(1, count.load(Ordering::SeqCst));
    for _ in iterator.get_iterator() {}
    assert_eq!(2, count.load(Ordering::SeqCst));
}

// --- MultiplePcreFilterIteratorTest ---------------------------------------

#[test]
fn MultiplePcreFilterIteratorTest_testIsRegex() -> TestResult {
    let rows = provider("MultiplePcreFilterIteratorTest", "testIsRegex");
    assert_eq!(rows.len(), 19);
    for (_, args) in rows {
        let string = common::bytes(&args[0]);
        let expected = matches!(args[1], Value::Bool(true));
        assert_eq!(
            expected,
            MultiplePcreFilter::is_regex(&string)?,
            "{}",
            common::string(&args[2])
        );
    }
    Ok(())
}

// --- PathFilterIteratorTest -----------------------------------------------

#[test]
fn PathFilterIteratorTest_testFilter() -> TestResult {
    let file = |name: &str, relative: &str| MockSplFileInfo {
        name: name.into(),
        relative_pathname: Some(relative.into()),
        ..MockSplFileInfo::default()
    };
    let inner = vec![
        file("abc.dat", "A/B/C/abc.dat"),
        file("ab.dat", "A/B/ab.dat"),
        file("a.dat", "A/a.dat"),
        file("abc.dat.copy", "copy/A/B/C/abc.dat"),
        file("ab.dat.copy", "copy/A/B/ab.dat"),
        file("a.dat.copy", "copy/A/a.dat"),
    ];
    let all: &[&str] = &[
        "abc.dat",
        "ab.dat",
        "a.dat",
        "abc.dat.copy",
        "ab.dat.copy",
        "a.dat.copy",
    ];
    let rows: Vec<(&[&str], &[&str], &[&str])> = vec![
        (&["/^A/"], &[], &["abc.dat", "ab.dat", "a.dat"]),
        (&[r"/^A\/B/"], &[], &["abc.dat", "ab.dat"]),
        (&[r"/^A\/B\/C/"], &[], &["abc.dat"]),
        (&[r"/A\/B\/C/"], &[], &["abc.dat", "abc.dat.copy"]),
        (&["A"], &[], all),
        (
            &["A/B"],
            &[],
            &["abc.dat", "ab.dat", "abc.dat.copy", "ab.dat.copy"],
        ),
        (&["A/B/C"], &[], &["abc.dat", "abc.dat.copy"]),
        (
            &["copy/A"],
            &[],
            &["abc.dat.copy", "ab.dat.copy", "a.dat.copy"],
        ),
        (&["copy/A/B"], &[], &["abc.dat.copy", "ab.dat.copy"]),
        (&["copy/A/B/C"], &[], &["abc.dat.copy"]),
        (
            &[],
            &["/A/B/C/"],
            &["abc.dat", "ab.dat", "a.dat", "ab.dat.copy", "a.dat.copy"],
        ),
        (&["/A/B/"], &[], &["abc.dat.copy", "ab.dat.copy"]),
        (&["/a/i"], &[], all),
        (
            &[r"/A\/B\//"],
            &[],
            &["abc.dat", "ab.dat", "abc.dat.copy", "ab.dat.copy"],
        ),
        (&["{^A/B/}"], &[], &["abc.dat", "ab.dat"]),
        (&["/[unterminated/"], &[], &[]),
    ];
    for (matches, no_matches, expected) in rows {
        let iterator = PathFilterIterator::new(
            inner.clone().into_iter().map(Ok),
            &bytes_list(matches),
            &bytes_list(no_matches),
        )?;
        assert_iterator(strs(expected), iterator);
    }
    Ok(())
}

// --- SizeRangeFilterIteratorTest ------------------------------------------

#[test]
fn SizeRangeFilterIteratorTest_testAccept() -> TestResult {
    let less_than_1k_greater_than_05k = [
        ".foo",
        ".git",
        "foo",
        "qux",
        "test.php",
        "toto",
        "toto/.git",
    ];
    let iterator = SizeRangeFilterIterator::new(
        infos(&all_files()),
        vec![
            NumberComparator::new(Some(b"< 1K"))?,
            NumberComparator::new(Some(b"> 0.5K"))?,
        ],
    );
    assert_iterator(to_absolute(tmp(), &less_than_1k_greater_than_05k), iterator);
    Ok(())
}
