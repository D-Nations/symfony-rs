//! FinderTest.php at symfony/finder
//! 2f4c0f26106520d82e1b71759d2fb8d32d8b7a43, and the four GlobTest tests
//! that walk directories. Names and assertions are PHPUnit's;
//! testContains, testPath and testRegexName replay their recorded
//! providers. RealIteratorTestCase's tree is built once in
//! `<temp>/symfony_finder_FinderTest`; the tests that change it (symbolic
//! links, permissions) work on a copy of their own, since Rust runs tests
//! in parallel. `__DIR__` is the upstream Tests directory, read only.
//! VfsIteratorTestTrait's stream wrapper is `Vfs` below, registered with
//! `fs::stream_wrapper_register()`.
//!
//! Tests that need symbolic links do not run where the system refuses
//! them; the permission tests need Unix permissions. Upstream skips both
//! kinds on Windows.
//!
//! Upstream: symfony/finder Tests/FinderTest.php Tests/Iterator/VfsIteratorTestTrait.php
#![allow(non_snake_case)]

mod common;

use std::sync::{
    Arc, Mutex, OnceLock,
    atomic::{AtomicUsize, Ordering as AtomicOrdering},
};

use common::{
    TestResult,
    fixture::{
        assert_iterator, assert_ordered_iterator, can_symlink, real_fixture, remove_dir, symlink,
        temp_root, touch, upstream_tests,
    },
    provider, strings,
};
use symfony_finder::{
    Appended, Error, Finder, SplFileInfo,
    fs::{Stat, StreamWrapper, stream_wrapper_register, stream_wrapper_unregister},
    php::strcmp,
};

fn tmp() -> &'static str {
    static TMP: OnceLock<String> = OnceLock::new();
    TMP.get_or_init(|| real_fixture("FinderTest", true))
}

fn abs(files: &[&str]) -> Vec<String> {
    files.iter().map(|f| format!("{}/{f}", tmp())).collect()
}

fn abs_in(dir: &str, files: &[&str]) -> Vec<String> {
    files.iter().map(|f| format!("{dir}/{f}")).collect()
}

fn fixtures(files: &[&str]) -> Vec<String> {
    files
        .iter()
        .map(|f| {
            let path = format!("{}/Fixtures/{f}", upstream_tests());
            symfony_finder::fs::realpath(path.as_bytes())
                .map(|p| String::from_utf8_lossy(&p).into_owned())
                .unwrap_or(path)
        })
        .collect()
}

/// buildFinder(): `Finder::create()->exclude('gitignore')`.
fn build_finder() -> Finder {
    let mut finder = Finder::create();
    finder.exclude("gitignore");
    finder
}

fn same(finder: *const Finder, returned: &mut Finder) {
    assert!(std::ptr::eq(finder, returned), "the method returns $this");
}

fn pathnames(finder: &Finder) -> Result<Vec<String>, Error> {
    finder
        .get_iterator()?
        .map(|f| f.map(|f| String::from_utf8_lossy(f.get_pathname()).into_owned()))
        .collect()
}

const ALL: [&str; 17] = [
    "foo",
    "foo/bar.tmp",
    "test.php",
    "test.py",
    "toto",
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
    "Zephire.php",
];

const FILES_ONLY: [&str; 14] = [
    "foo/bar.tmp",
    "test.php",
    "test.py",
    "foo bar",
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

const DOTS: [&str; 4] = [".bar", ".foo", ".foo/.bar", ".foo/bar"];
const VCS: [&str; 2] = [".git", "toto/.git"];

fn with(base: &[&str], more: &[&str]) -> Vec<String> {
    let mut all: Vec<&str> = base.to_vec();
    all.extend_from_slice(more);
    abs(&all)
}

const PHP_FILES: [&str; 9] = [
    "test.php",
    "qux_0_1.php",
    "qux_1000_1.php",
    "qux_1002_0.php",
    "qux_10_2.php",
    "qux_12_0.php",
    "qux_2_0.php",
    "zebulon.php",
    "Zephire.php",
];

#[test]
fn testCreate() {
    let _: Finder = Finder::create();
}

#[test]
fn testDirectories() -> TestResult {
    let mut finder = build_finder();
    let p: *const Finder = &finder;
    same(p, finder.directories());
    assert_iterator(
        abs(&["foo", "qux", "toto"]),
        finder.r#in(tmp())?.get_iterator()?,
    );

    let mut finder = build_finder();
    finder.directories();
    finder.files();
    finder.directories();
    assert_iterator(
        abs(&["foo", "qux", "toto"]),
        finder.r#in(tmp())?.get_iterator()?,
    );
    Ok(())
}

#[test]
fn testFiles() -> TestResult {
    let mut finder = build_finder();
    let p: *const Finder = &finder;
    same(p, finder.files());
    assert_iterator(abs(&FILES_ONLY), finder.r#in(tmp())?.get_iterator()?);

    let mut finder = build_finder();
    finder.files();
    finder.directories();
    finder.files();
    assert_iterator(abs(&FILES_ONLY), finder.r#in(tmp())?.get_iterator()?);
    Ok(())
}

#[test]
fn testRemoveTrailingSlash() -> TestResult {
    let mut finder = build_finder();
    let r#in = format!("{}//", tmp());
    assert_iterator(
        abs(&FILES_ONLY),
        finder.r#in(r#in.as_str())?.files().get_iterator()?,
    );
    Ok(())
}

#[test]
fn testSymlinksNotResolved() -> TestResult {
    if !can_symlink() {
        return Ok(());
    }
    let dir = real_fixture("FinderTest_symlinks", true);
    let mut finder = build_finder();
    symlink(&format!("{dir}/foo"), &format!("{dir}/baz")).ok_or("symlink")?;
    let expected = abs_in(&dir, &["baz/bar.tmp"]);
    let r#in = format!("{dir}/baz/");
    let result: Result<Vec<SplFileInfo>, Error> = finder
        .r#in(r#in.as_str())?
        .files()
        .get_iterator()?
        .collect();
    remove_dir(&dir);
    assert_iterator(expected, result?.into_iter().map(Ok));
    Ok(())
}

#[test]
fn testBackPathNotNormalized() -> TestResult {
    let mut finder = build_finder();
    let expected = abs(&["foo/../foo/bar.tmp"]);
    let r#in = format!("{}/foo/../foo/", tmp());
    assert_iterator(
        expected,
        finder.r#in(r#in.as_str())?.files().get_iterator()?,
    );
    Ok(())
}

#[test]
fn testDepth() -> TestResult {
    let depth0: Vec<&str> = ALL.iter().copied().filter(|f| !f.contains('/')).collect();
    let mut finder = build_finder();
    let p: *const Finder = &finder;
    same(p, finder.depth("< 1")?);
    assert_iterator(abs(&depth0), finder.r#in(tmp())?.get_iterator()?);

    let mut finder = build_finder();
    let p: *const Finder = &finder;
    same(p, finder.depth("<= 0")?);
    assert_iterator(abs(&depth0), finder.r#in(tmp())?.get_iterator()?);

    let mut finder = build_finder();
    let p: *const Finder = &finder;
    same(p, finder.depth(">= 1")?);
    assert_iterator(
        abs(&["foo/bar.tmp", "qux/baz_100_1.py", "qux/baz_1_2.py"]),
        finder.r#in(tmp())?.get_iterator()?,
    );

    let mut finder = build_finder();
    finder.depth("< 1")?.depth(">= 1")?;
    assert_iterator(Vec::new(), finder.r#in(tmp())?.get_iterator()?);
    Ok(())
}

#[test]
fn testDepthWithArrayParam() -> TestResult {
    let mut finder = build_finder();
    finder.depth([">= 1", "< 2"])?;
    assert_iterator(
        abs(&["foo/bar.tmp", "qux/baz_100_1.py", "qux/baz_1_2.py"]),
        finder.r#in(tmp())?.get_iterator()?,
    );
    Ok(())
}

#[test]
fn testName() -> TestResult {
    let mut finder = build_finder();
    let p: *const Finder = &finder;
    same(p, finder.name("*.php"));
    assert_iterator(abs(&PHP_FILES), finder.r#in(tmp())?.get_iterator()?);

    let mut finder = build_finder();
    finder.name("test.ph*");
    finder.name("test.py");
    assert_iterator(
        abs(&["test.php", "test.py"]),
        finder.r#in(tmp())?.get_iterator()?,
    );

    let mut finder = build_finder();
    finder.name("~^test~i");
    assert_iterator(
        abs(&["test.php", "test.py"]),
        finder.r#in(tmp())?.get_iterator()?,
    );

    let mut finder = build_finder();
    finder.name(r"~\.php$~i");
    assert_iterator(abs(&PHP_FILES), finder.r#in(tmp())?.get_iterator()?);

    let mut finder = build_finder();
    finder.name("test.p{hp,y}");
    assert_iterator(
        abs(&["test.php", "test.py"]),
        finder.r#in(tmp())?.get_iterator()?,
    );
    Ok(())
}

#[test]
fn testNameWithArrayParam() -> TestResult {
    let mut finder = build_finder();
    finder.name(["test.php", "test.py"]);
    assert_iterator(
        abs(&["test.php", "test.py"]),
        finder.r#in(tmp())?.get_iterator()?,
    );
    Ok(())
}

#[test]
fn testNotName() -> TestResult {
    let mut finder = build_finder();
    let p: *const Finder = &finder;
    same(p, finder.not_name("*.php"));
    assert_iterator(
        abs(&[
            "foo",
            "foo/bar.tmp",
            "test.py",
            "toto",
            "foo bar",
            "qux",
            "qux/baz_100_1.py",
            "qux/baz_1_2.py",
        ]),
        finder.r#in(tmp())?.get_iterator()?,
    );

    let mut finder = build_finder();
    finder.not_name("*.php");
    finder.not_name("*.py");
    assert_iterator(
        abs(&["foo", "foo/bar.tmp", "toto", "foo bar", "qux"]),
        finder.r#in(tmp())?.get_iterator()?,
    );

    let mut finder = build_finder();
    finder.name("test.ph*");
    finder.name("test.py");
    finder.not_name("*.php");
    finder.not_name("*.py");
    assert_iterator(Vec::new(), finder.r#in(tmp())?.get_iterator()?);

    let mut finder = build_finder();
    finder.name("test.ph*");
    finder.name("test.py");
    finder.not_name("*.p{hp,y}");
    assert_iterator(Vec::new(), finder.r#in(tmp())?.get_iterator()?);
    Ok(())
}

#[test]
fn testNotNameWithArrayParam() -> TestResult {
    let mut finder = build_finder();
    finder.not_name(["*.php", "*.py"]);
    assert_iterator(
        abs(&["foo", "foo/bar.tmp", "toto", "foo bar", "qux"]),
        finder.r#in(tmp())?.get_iterator()?,
    );
    Ok(())
}

#[test]
fn testRegexName() -> TestResult {
    let rows = provider("FinderTest", "testRegexName");
    assert_eq!(rows.len(), 2);
    for (_, args) in rows {
        let mut finder = build_finder();
        finder.name(strings(&args[0]));
        assert_iterator(
            abs(&["test.py", "test.php"]),
            finder.r#in(tmp())?.get_iterator()?,
        );
    }
    Ok(())
}

#[test]
fn testSize() -> TestResult {
    let mut finder = build_finder();
    let p: *const Finder = &finder;
    same(p, finder.files().size("< 1K")?.size("> 500")?);
    assert_iterator(abs(&["test.php"]), finder.r#in(tmp())?.get_iterator()?);
    Ok(())
}

#[test]
fn testSizeWithArrayParam() -> TestResult {
    let mut finder = build_finder();
    let p: *const Finder = &finder;
    same(p, finder.files().size(["< 1K", "> 500"])?);
    assert_iterator(abs(&["test.php"]), finder.r#in(tmp())?.get_iterator()?);
    Ok(())
}

#[test]
fn testDate() -> TestResult {
    let mut finder = build_finder();
    let p: *const Finder = &finder;
    same(p, finder.files().date("until last month")?);
    assert_iterator(
        abs(&["foo/bar.tmp", "test.php"]),
        finder.r#in(tmp())?.get_iterator()?,
    );
    Ok(())
}

#[test]
fn testDateWithArrayParam() -> TestResult {
    let mut finder = build_finder();
    let p: *const Finder = &finder;
    same(
        p,
        finder.files().date([">= 2005-10-15", "until last month"])?,
    );
    assert_iterator(
        abs(&["foo/bar.tmp", "test.php"]),
        finder.r#in(tmp())?.get_iterator()?,
    );
    Ok(())
}

#[test]
fn testExclude() -> TestResult {
    let mut finder = build_finder();
    let p: *const Finder = &finder;
    same(p, finder.exclude("foo"));
    let expected: Vec<&str> = ALL
        .iter()
        .copied()
        .filter(|f| !f.starts_with("foo/") && *f != "foo")
        .collect();
    assert_iterator(abs(&expected), finder.r#in(tmp())?.get_iterator()?);
    Ok(())
}

#[test]
fn testIgnoreVCS() -> TestResult {
    let mut finder = build_finder();
    let p: *const Finder = &finder;
    same(p, finder.ignore_vcs(false).ignore_dot_files(false));
    let everything: Vec<&str> = ALL
        .iter()
        .chain(DOTS.iter())
        .chain(VCS.iter())
        .copied()
        .collect();
    assert_iterator(abs(&everything), finder.r#in(tmp())?.get_iterator()?);

    let mut finder = build_finder();
    finder
        .ignore_vcs(false)
        .ignore_vcs(false)
        .ignore_dot_files(false);
    assert_iterator(abs(&everything), finder.r#in(tmp())?.get_iterator()?);

    let mut finder = build_finder();
    let p: *const Finder = &finder;
    same(p, finder.ignore_vcs(true).ignore_dot_files(false));
    assert_iterator(with(&ALL, &DOTS), finder.r#in(tmp())?.get_iterator()?);
    Ok(())
}

#[test]
fn testIgnoreVCSIgnored() -> TestResult {
    let mut finder = build_finder();
    let p: *const Finder = &finder;
    same(
        p,
        finder
            .ignore_vcs(true)
            .ignore_dot_files(true)
            .ignore_vcs_ignored(true),
    );
    assert_iterator(
        abs(&[
            "gitignore/search_root/b.txt",
            "gitignore/search_root/dir",
            "gitignore/search_root/dir/a.txt",
        ]),
        finder
            .r#in(format!("{}/gitignore/search_root", tmp()))?
            .get_iterator()?,
    );
    Ok(())
}

#[test]
fn testIgnoreVCSIgnoredUpToFirstGitRepositoryRoot() -> TestResult {
    let mut finder = build_finder();
    let p: *const Finder = &finder;
    same(
        p,
        finder
            .ignore_vcs(true)
            .ignore_dot_files(true)
            .ignore_vcs_ignored(true),
    );
    assert_iterator(
        abs(&[
            "gitignore/git_root/search_root/b.txt",
            "gitignore/git_root/search_root/c.txt",
            "gitignore/git_root/search_root/dir",
            "gitignore/git_root/search_root/dir/a.txt",
            "gitignore/git_root/search_root/dir/c.txt",
        ]),
        finder
            .r#in(format!("{}/gitignore/git_root/search_root", tmp()))?
            .get_iterator()?,
    );
    Ok(())
}

#[test]
fn testIgnoreVCSCanBeDisabledAfterFirstIteration() -> TestResult {
    let mut finder = build_finder();
    finder.r#in(tmp())?;
    finder.ignore_dot_files(false);
    assert_iterator(with(&ALL, &DOTS), finder.get_iterator()?);

    finder.ignore_vcs(false);
    let everything: Vec<&str> = ALL
        .iter()
        .chain(DOTS.iter())
        .chain(VCS.iter())
        .copied()
        .collect();
    assert_iterator(abs(&everything), finder.get_iterator()?);
    Ok(())
}

#[test]
fn testIgnoreDotFiles() -> TestResult {
    let everything: Vec<&str> = ALL
        .iter()
        .chain(DOTS.iter())
        .chain(VCS.iter())
        .copied()
        .collect();
    let mut finder = build_finder();
    let p: *const Finder = &finder;
    same(p, finder.ignore_dot_files(false).ignore_vcs(false));
    assert_iterator(abs(&everything), finder.r#in(tmp())?.get_iterator()?);

    let mut finder = build_finder();
    finder
        .ignore_dot_files(false)
        .ignore_dot_files(false)
        .ignore_vcs(false);
    assert_iterator(abs(&everything), finder.r#in(tmp())?.get_iterator()?);

    let mut finder = build_finder();
    let p: *const Finder = &finder;
    same(p, finder.ignore_dot_files(true).ignore_vcs(false));
    assert_iterator(abs(&ALL), finder.r#in(tmp())?.get_iterator()?);
    Ok(())
}

#[test]
fn testIgnoreDotFilesCanBeDisabledAfterFirstIteration() -> TestResult {
    let mut finder = build_finder();
    finder.r#in(tmp())?;
    assert_iterator(abs(&ALL), finder.get_iterator()?);

    finder.ignore_dot_files(false);
    assert_iterator(with(&ALL, &DOTS), finder.get_iterator()?);
    Ok(())
}

const BY_NAME: [&str; 17] = [
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
    "zebulon.php",
];

#[test]
fn testSortByName() -> TestResult {
    let mut finder = build_finder();
    let p: *const Finder = &finder;
    same(p, finder.sort_by_name(false));
    assert_ordered_iterator(abs(&BY_NAME), finder.r#in(tmp())?.get_iterator()?);
    Ok(())
}

#[test]
fn testSortByType() -> TestResult {
    let mut finder = build_finder();
    let p: *const Finder = &finder;
    same(p, finder.sort_by_type());
    assert_ordered_iterator(
        abs(&[
            "foo",
            "qux",
            "toto",
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
        ]),
        finder.r#in(tmp())?.get_iterator()?,
    );
    Ok(())
}

#[test]
fn testSortByAccessedTime() -> TestResult {
    let mut finder = build_finder();
    let p: *const Finder = &finder;
    same(p, finder.sort_by_accessed_time());
    assert_iterator(abs(&ALL), finder.r#in(tmp())?.get_iterator()?);
    Ok(())
}

#[test]
fn testSortByChangedTime() -> TestResult {
    let mut finder = build_finder();
    let p: *const Finder = &finder;
    same(p, finder.sort_by_changed_time());
    assert_iterator(abs(&ALL), finder.r#in(tmp())?.get_iterator()?);
    Ok(())
}

#[test]
fn testSortByModifiedTime() -> TestResult {
    let mut finder = build_finder();
    let p: *const Finder = &finder;
    same(p, finder.sort_by_modified_time());
    assert_iterator(abs(&ALL), finder.r#in(tmp())?.get_iterator()?);
    Ok(())
}

#[test]
fn testReverseSorting() -> TestResult {
    let mut finder = build_finder();
    let p: *const Finder = &finder;
    same(p, finder.sort_by_name(false));
    same(p, finder.reverse_sorting());
    let mut expected = BY_NAME.to_vec();
    expected.reverse();
    // assertOrderedIteratorInForeach
    assert_eq!(abs(&expected), pathnames(finder.r#in(tmp())?)?);
    Ok(())
}

#[test]
fn testSortByNameNatural() -> TestResult {
    let mut finder = build_finder();
    let p: *const Finder = &finder;
    same(p, finder.sort_by_name(true));
    assert_ordered_iterator(
        abs(&[
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
            "zebulon.php",
        ]),
        finder.r#in(tmp())?.get_iterator()?,
    );

    let mut finder = build_finder();
    let p: *const Finder = &finder;
    same(p, finder.sort_by_name(false));
    assert_ordered_iterator(abs(&BY_NAME), finder.r#in(tmp())?.get_iterator()?);
    Ok(())
}

#[test]
fn testSortByNameCaseInsensitive() -> TestResult {
    let mut finder = build_finder();
    let p: *const Finder = &finder;
    same(p, finder.sort_by_case_insensitive_name(true));
    // the separator is "/" (Linux behaviour is canonical)
    assert_ordered_iterator(
        abs(&[
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
            "zebulon.php",
            "Zephire.php",
        ]),
        finder.r#in(tmp())?.get_iterator()?,
    );

    let mut finder = build_finder();
    let p: *const Finder = &finder;
    same(p, finder.sort_by_case_insensitive_name(false));
    assert_ordered_iterator(
        abs(&[
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
            "zebulon.php",
            "Zephire.php",
        ]),
        finder.r#in(tmp())?.get_iterator()?,
    );
    Ok(())
}

fn real(f: &SplFileInfo) -> Vec<u8> {
    f.get_real_path().unwrap_or_default()
}

#[test]
fn testSort() -> TestResult {
    let mut finder = build_finder();
    let p: *const Finder = &finder;
    same(p, finder.sort(|a, b| strcmp(&real(a), &real(b))));
    assert_ordered_iterator(abs(&BY_NAME), finder.r#in(tmp())?.get_iterator()?);
    Ok(())
}

#[test]
fn testSortAcrossDirectories() -> TestResult {
    let mut finder = build_finder();
    finder
        .r#in(vec![
            tmp().to_string(),
            format!("{}/qux", tmp()),
            format!("{}/foo", tmp()),
        ])?
        .depth(0)?
        .files()
        .filter(|file| !file.get_extension().is_empty(), false)
        .sort(|a, b| {
            strcmp(a.get_extension(), b.get_extension())
                .then_with(|| strcmp(a.get_filename(), b.get_filename()))
        });
    assert_ordered_iterator(
        abs(&[
            "Zephire.php",
            "qux_0_1.php",
            "qux_1000_1.php",
            "qux_1002_0.php",
            "qux_10_2.php",
            "qux_12_0.php",
            "qux_2_0.php",
            "test.php",
            "zebulon.php",
            "qux/baz_100_1.py",
            "qux/baz_1_2.py",
            "test.py",
            "foo/bar.tmp",
        ]),
        finder.get_iterator()?,
    );
    Ok(())
}

#[test]
fn testFilter() -> TestResult {
    let mut finder = build_finder();
    let p: *const Finder = &finder;
    same(
        p,
        finder.filter(|f| f.get_pathname().windows(4).any(|w| w == b"test"), false),
    );
    assert_iterator(
        abs(&["test.php", "test.py"]),
        finder.r#in(tmp())?.get_iterator()?,
    );
    Ok(())
}

// --- VfsIteratorTestTrait -------------------------------------------------

#[derive(Clone)]
enum Node {
    File,
    Dir(Vec<(&'static str, Node)>),
}

fn dir(entries: Vec<(&'static str, Node)>) -> Node {
    Node::Dir(entries)
}

#[derive(Debug, Clone, PartialEq)]
enum Res {
    List(Vec<String>),
    Bool(bool),
}

type Log = Arc<Mutex<Vec<(String, String, Res)>>>;

/// The anonymous stream wrapper class of VfsIteratorTestTrait, with the
/// provider setupVfsProvider() installs.
struct Vfs {
    data: Node,
    log: Log,
}

impl Vfs {
    fn parse(url: &[u8]) -> String {
        let url = String::from_utf8_lossy(url);
        let rest = url.split_once("://").map_or(&*url, |(_, r)| r);
        rest.trim_end_matches('/').to_string()
    }

    fn entry(&self, path: &str) -> Option<&Node> {
        let mut node = &self.data;
        for name in path.split('/') {
            match node {
                Node::Dir(entries) => node = &entries.iter().find(|(n, _)| *n == name)?.1,
                Node::File => return None,
            }
        }
        Some(node)
    }

    fn log(&self, path: &str, op: &str, res: Res) {
        self.log.lock().unwrap_or_else(|e| e.into_inner()).push((
            path.to_string(),
            op.to_string(),
            res,
        ));
    }
}

impl StreamWrapper for Vfs {
    fn dir_opendir(&self, url: &[u8]) -> Option<Vec<Vec<u8>>> {
        let path = Self::parse(url);
        let names: Vec<String> = match self.entry(&path) {
            Some(Node::Dir(entries)) => entries.iter().map(|(n, _)| n.to_string()).collect(),
            _ => Vec::new(),
        };
        self.log(&path, "list_dir_open", Res::List(names.clone()));
        Some(names.into_iter().map(String::into_bytes).collect())
    }

    fn url_stat(&self, url: &[u8]) -> Option<Stat> {
        let path = Self::parse(url);
        let is_dir = matches!(self.entry(&path), Some(Node::Dir(_)));
        self.log(&path, "is_dir", Res::Bool(is_dir));
        Some(Stat {
            mode: if is_dir { 0o040_755 } else { 0o100_644 },
            ..Stat::default()
        })
    }
}

struct VfsScheme {
    scheme: String,
    log: Log,
}

impl VfsScheme {
    fn setup(data: Node) -> Self {
        static NEXT: AtomicUsize = AtomicUsize::new(0);
        let scheme = format!(
            "symfony-finder-vfs-test-{}",
            NEXT.fetch_add(1, AtomicOrdering::SeqCst) + 1
        );
        let log: Log = Arc::default();
        stream_wrapper_register(
            &scheme,
            Arc::new(Vfs {
                data,
                log: Arc::clone(&log),
            }),
        );
        Self { scheme, log }
    }

    fn dir(&self) -> String {
        format!("{}://", self.scheme)
    }

    fn strip(&self, url: &[u8]) -> String {
        Vfs::parse(url)
    }

    fn log(&self) -> Vec<(String, String, Res)> {
        self.log.lock().unwrap_or_else(|e| e.into_inner()).clone()
    }

    fn assert_same_vfs_iterator(&self, expected: &[&str], finder: &Finder) -> TestResult {
        let values: Vec<String> = finder
            .get_iterator()?
            .map(|f| f.map(|f| self.strip(f.get_pathname())))
            .collect::<Result<_, _>>()?;
        assert_eq!(expected, values);
        Ok(())
    }
}

impl Drop for VfsScheme {
    fn drop(&mut self) {
        stream_wrapper_unregister(&self.scheme);
    }
}

fn log_entry(path: &str, op: &str, res: Res) -> (String, String, Res) {
    (path.to_string(), op.to_string(), res)
}

fn list(names: &[&str]) -> Res {
    Res::List(names.iter().map(|s| s.to_string()).collect())
}

#[test]
fn testFilterPrune() -> TestResult {
    let vfs = VfsScheme::setup(dir(vec![
        (
            "x",
            dir(vec![
                ("a.php", Node::File),
                ("b.php", Node::File),
                ("d", dir(vec![("u.php", Node::File)])),
                ("x", dir(vec![("d", dir(vec![("u2.php", Node::File)]))])),
            ]),
        ),
        ("y", dir(vec![("c.php", Node::File)])),
    ]));
    let log = Arc::clone(&vfs.log);
    let mut finder = build_finder();
    finder
        .r#in(format!("{}x", vfs.dir()))?
        .filter(|_| true, true) // does nothing
        .filter(
            move |file| {
                let path = Vfs::parse(file.get_pathname());
                let res = path != "x/d";
                log.lock().unwrap_or_else(|e| e.into_inner()).push((
                    path,
                    "exclude_filter".into(),
                    Res::Bool(res),
                ));
                res
            },
            true,
        )
        .filter(|_| true, true); // does nothing

    vfs.assert_same_vfs_iterator(
        &["x/a.php", "x/b.php", "x/x", "x/x/d", "x/x/d/u2.php"],
        &finder,
    )?;

    // "x/d" directory must be pruned early
    // "x/x/d" directory must not be pruned
    assert_eq!(
        vec![
            log_entry("x", "is_dir", Res::Bool(true)),
            log_entry("x", "list_dir_open", list(&["a.php", "b.php", "d", "x"])),
            log_entry("x/a.php", "is_dir", Res::Bool(false)),
            log_entry("x/a.php", "exclude_filter", Res::Bool(true)),
            log_entry("x/b.php", "is_dir", Res::Bool(false)),
            log_entry("x/b.php", "exclude_filter", Res::Bool(true)),
            log_entry("x/d", "is_dir", Res::Bool(true)),
            log_entry("x/d", "exclude_filter", Res::Bool(false)),
            log_entry("x/x", "is_dir", Res::Bool(true)),
            log_entry("x/x", "exclude_filter", Res::Bool(true)), // from ExcludeDirectoryFilterIterator::accept() (prune directory filter)
            log_entry("x/x", "exclude_filter", Res::Bool(true)), // from CustomFilterIterator::accept() (regular filter)
            log_entry("x/x", "list_dir_open", list(&["d"])),
            log_entry("x/x/d", "is_dir", Res::Bool(true)),
            log_entry("x/x/d", "exclude_filter", Res::Bool(true)), // from ExcludeDirectoryFilterIterator::accept() (prune directory filter)
            log_entry("x/x/d", "exclude_filter", Res::Bool(true)), // from CustomFilterIterator::accept() (regular filter)
            log_entry("x/x/d", "list_dir_open", list(&["u2.php"])),
            log_entry("x/x/d/u2.php", "is_dir", Res::Bool(false)),
            log_entry("x/x/d/u2.php", "exclude_filter", Res::Bool(true)),
        ],
        vfs.log()
    );
    Ok(())
}

#[test]
fn testFilterPruneNestedDirectory() -> TestResult {
    let vfs = VfsScheme::setup(dir(vec![(
        "x",
        dir(vec![
            ("a.php", Node::File),
            (
                "n",
                dir(vec![
                    ("d", dir(vec![("u.php", Node::File)])),
                    ("z.php", Node::File),
                ]),
            ),
        ]),
    )]));
    let mut finder = build_finder();
    finder
        .r#in(format!("{}x", vfs.dir()))?
        .filter(|file| file.get_filename() != b"d", true);
    vfs.assert_same_vfs_iterator(&["x/a.php", "x/n", "x/n/z.php"], &finder)
}

#[test]
fn testFilterPruneWithExcludedPath() -> TestResult {
    let vfs = VfsScheme::setup(dir(vec![(
        "x",
        dir(vec![
            ("a.php", Node::File),
            ("d", dir(vec![("u.php", Node::File)])),
            (
                "g",
                dir(vec![
                    ("h", dir(vec![("v.php", Node::File)])),
                    ("w.php", Node::File),
                ]),
            ),
        ]),
    )]));
    let mut finder = build_finder();
    finder
        .r#in(format!("{}x", vfs.dir()))?
        .exclude("g/h")
        .filter(|file| file.get_filename() != b"d", true);
    vfs.assert_same_vfs_iterator(&["x/a.php", "x/g", "x/g/w.php"], &finder)
}

#[test]
fn testFollowLinks() -> TestResult {
    let mut finder = build_finder();
    let p: *const Finder = &finder;
    same(p, finder.follow_links());
    assert_iterator(abs(&ALL), finder.r#in(tmp())?.get_iterator()?);
    Ok(())
}

#[test]
fn testUseUnixPaths() -> TestResult {
    let fixtures_directory = format!("{}/Fixtures", upstream_tests()).replace('\\', "/");
    let mut finder = build_finder();
    let p: *const Finder = &finder;
    same(p, finder.use_unix_paths());
    let mut saw_subdirectory_entry = false;
    for file in finder.r#in(fixtures_directory.as_str())?.get_iterator()? {
        let file = file?;
        assert!(
            !file.get_pathname().contains(&b'\\'),
            "getPathname() should use forward slashes."
        );
        assert!(
            !file.get_path().contains(&b'\\'),
            "getPath() should use forward slashes."
        );
        assert!(
            !file.get_relative_path().contains(&b'\\'),
            "getRelativePath() should use forward slashes."
        );
        assert!(
            !file.get_relative_pathname().contains(&b'\\'),
            "getRelativePathname() should use forward slashes."
        );
        if !file.get_relative_path().is_empty() {
            saw_subdirectory_entry = true;
        }
    }
    assert!(
        saw_subdirectory_entry,
        "The Fixtures directory should yield at least one entry inside a subdirectory."
    );
    Ok(())
}

#[test]
fn testFollowLinksDoesNotLoopOnCircularSymlinks() -> TestResult {
    if !can_symlink() {
        return Ok(());
    }
    let tmp_dir = format!("{}/symfony_finder_symlink_loop_circular", temp_root());
    remove_dir(&tmp_dir);
    std::fs::create_dir_all(format!("{tmp_dir}/a"))?;
    std::fs::create_dir_all(format!("{tmp_dir}/b"))?;
    touch(&format!("{tmp_dir}/a/f.txt"), None);
    touch(&format!("{tmp_dir}/b/g.txt"), None);
    symlink("../b", &format!("{tmp_dir}/a/b")).ok_or("symlink")?;
    symlink("../a", &format!("{tmp_dir}/b/a")).ok_or("symlink")?;
    let mut finder = build_finder();
    finder.follow_links().r#in(format!("{tmp_dir}/a"))?;
    let result: Result<Vec<SplFileInfo>, Error> = finder.get_iterator()?.collect();
    remove_dir(&tmp_dir);
    assert_iterator(
        abs_in(&tmp_dir, &["a/f.txt", "a/b", "a/b/g.txt", "a/b/a"]),
        result?.into_iter().map(Ok),
    );
    Ok(())
}

#[test]
fn testFollowLinksDoesNotLoopOnSymlinkToAParentOfTheSearchedDirectory() -> TestResult {
    if !can_symlink() {
        return Ok(());
    }
    let tmp_dir = format!("{}/symfony_finder_symlink_loop_parent", temp_root());
    remove_dir(&tmp_dir);
    std::fs::create_dir_all(format!("{tmp_dir}/x/y"))?;
    touch(&format!("{tmp_dir}/x/y/f.txt"), None);
    symlink("..", &format!("{tmp_dir}/x/y/up")).ok_or("symlink")?;
    let mut finder = build_finder();
    finder.follow_links().r#in(format!("{tmp_dir}/x/y"))?;
    let result: Result<Vec<SplFileInfo>, Error> = finder.get_iterator()?.collect();
    remove_dir(&tmp_dir);
    assert_iterator(
        abs_in(&tmp_dir, &["x/y/f.txt", "x/y/up", "x/y/up/y"]),
        result?.into_iter().map(Ok),
    );
    Ok(())
}

#[test]
fn testIn() -> TestResult {
    let dir = upstream_tests();
    let mut finder = build_finder();
    let iterator = finder
        .files()
        .name("*.php")
        .depth("< 1")?
        .r#in(vec![tmp().to_string(), dir.clone()])?
        .get_iterator()?;
    let mut expected = abs(&[
        "Zephire.php",
        "test.php",
        "qux_0_1.php",
        "qux_1000_1.php",
        "qux_1002_0.php",
        "qux_10_2.php",
        "qux_12_0.php",
        "qux_2_0.php",
        "zebulon.php",
    ]);
    expected.extend(abs_in(
        &dir,
        &[
            "GitignoreTest.php",
            "FinderOpenBasedirTest.php",
            "FinderTest.php",
            "GlobTest.php",
        ],
    ));
    assert_iterator(expected, iterator);
    Ok(())
}

#[test]
fn testInWithNonExistentDirectory() {
    let mut finder = Finder::new();
    let e = finder.r#in("foobar").err();
    assert!(e.is_some_and(|e| {
        e.is_instance_of(r"Symfony\Component\Finder\Exception\DirectoryNotFoundException")
    }));
}

#[test]
fn testInWithNonExistentDirectoryLegacyException() {
    let mut finder = Finder::new();
    let e = finder.r#in("foobar").err();
    assert!(e.is_some_and(|e| e.is_instance_of("InvalidArgumentException")));
}

#[test]
fn testInWithGlob() -> TestResult {
    let dir = upstream_tests();
    let mut finder = build_finder();
    finder.r#in(vec![
        format!("{dir}/Fixtures/*/B/C/"),
        format!("{dir}/Fixtures/*/*/B/C/"),
    ])?;
    assert_iterator(
        fixtures(&["A/B/C/abc.dat", "copy/A/B/C/abc.dat.copy"]),
        finder.get_iterator()?,
    );
    Ok(())
}

#[test]
fn testInWithNonDirectoryGlob() {
    let mut finder = Finder::new();
    let e = finder
        .r#in(format!("{}/Fixtures/A/a*", upstream_tests()))
        .err();
    assert!(e.is_some_and(|e| e.is_instance_of("InvalidArgumentException")));
}

#[test]
fn testInWithGlobBrace() -> TestResult {
    let mut finder = build_finder();
    finder.r#in(vec![format!(
        "{}/Fixtures/{{A,copy/A}}/B/C",
        upstream_tests()
    )])?;
    assert_iterator(
        fixtures(&["A/B/C/abc.dat", "copy/A/B/C/abc.dat.copy"]),
        finder.get_iterator()?,
    );
    Ok(())
}

#[test]
fn testGetIteratorWithoutIn() {
    let finder = Finder::create();
    let e = finder.get_iterator().err();
    assert!(e.is_some_and(|e| e.is_instance_of("LogicException")));
}

#[test]
fn testGetIterator() -> TestResult {
    let mut finder = build_finder();
    let mut dirs = Vec::new();
    for dir in finder.directories().r#in(tmp())?.get_iterator()? {
        dirs.push(dir?.to_string());
    }
    let mut expected = abs(&["foo", "qux", "toto"]);
    dirs.sort();
    expected.sort();
    assert_eq!(
        expected, dirs,
        "implements the \\IteratorAggregate interface"
    );

    let mut finder = build_finder();
    assert_eq!(
        3,
        finder.directories().r#in(tmp())?.get_iterator()?.count(),
        "implements the \\IteratorAggregate interface"
    );

    let mut finder = build_finder();
    let mut a: Vec<String> = finder
        .directories()
        .r#in(tmp())?
        .get_iterator()?
        .map(|f| f.map(|f| f.to_string()))
        .collect::<Result<_, _>>()?;
    a.sort();
    assert_eq!(expected, a, "implements the \\IteratorAggregate interface");
    Ok(())
}

#[test]
fn testRelativePath() -> TestResult {
    let mut finder = build_finder();
    finder.r#in(tmp())?;
    let mut paths: Vec<String> = finder
        .get_iterator()?
        .map(|f| f.map(|f| String::from_utf8_lossy(f.get_relative_path()).into_owned()))
        .collect::<Result<_, _>>()?;
    let mut r#ref: Vec<String> = [
        "", "", "", "", "", "", "", "", "", "", "", "", "", "foo", "qux", "qux", "",
    ]
    .iter()
    .map(|s| s.to_string())
    .collect();
    r#ref.sort();
    paths.sort();
    assert_eq!(r#ref, paths);
    Ok(())
}

#[test]
fn testRelativePathname() -> TestResult {
    let mut finder = build_finder();
    finder.r#in(tmp())?.sort_by_name(false);
    let mut paths: Vec<String> = finder
        .get_iterator()?
        .map(|f| f.map(|f| String::from_utf8_lossy(f.get_relative_pathname()).into_owned()))
        .collect::<Result<_, _>>()?;
    let mut r#ref: Vec<String> = ALL.iter().map(|s| s.to_string()).collect();
    paths.sort();
    r#ref.sort();
    assert_eq!(r#ref, paths);
    Ok(())
}

#[test]
fn testGetFilenameWithoutExtension() -> TestResult {
    let mut finder = build_finder();
    finder.r#in(tmp())?.sort_by_name(false);
    let mut file_names: Vec<String> = finder
        .get_iterator()?
        .map(|f| {
            f.map(|f| String::from_utf8_lossy(f.get_filename_without_extension()).into_owned())
        })
        .collect::<Result<_, _>>()?;
    let mut r#ref: Vec<String> = [
        "Zephire",
        "test",
        "toto",
        "test",
        "foo",
        "bar",
        "foo bar",
        "qux",
        "baz_100_1",
        "baz_1_2",
        "qux_0_1",
        "qux_1000_1",
        "qux_1002_0",
        "qux_10_2",
        "qux_12_0",
        "qux_2_0",
        "zebulon",
    ]
    .iter()
    .map(|s| s.to_string())
    .collect();
    file_names.sort();
    r#ref.sort();
    assert_eq!(r#ref, file_names);
    Ok(())
}

#[test]
fn testAppendWithAFinder() -> TestResult {
    let mut finder = build_finder();
    finder.files().r#in(format!("{}/foo", tmp()))?;
    let mut finder1 = build_finder();
    finder1.directories().r#in(tmp())?;
    finder.append(finder1);
    assert_iterator(
        abs(&["foo", "foo/bar.tmp", "qux", "toto"]),
        finder.get_iterator()?,
    );
    Ok(())
}

#[test]
fn testAppendWithAnArray() -> TestResult {
    let mut finder = build_finder();
    finder.files().r#in(format!("{}/foo", tmp()))?;
    finder.append(abs(&["foo", "toto"]));
    assert_iterator(abs(&["foo", "foo/bar.tmp", "toto"]), finder.get_iterator()?);
    Ok(())
}

#[test]
fn testAppendStandardizesItemsToBeSymfonySplFileInfo() -> TestResult {
    let mut finder1 = build_finder();
    finder1.files().r#in(format!("{}/foo", tmp()))?;
    let mut finder2 = build_finder();
    finder2.directories().r#in(tmp())?;
    finder1.append(finder2);
    finder1.append(abs(&["foo"]));
    finder1.append(Appended::Paths(
        abs(&["toto"]).into_iter().map(String::into_bytes).collect(),
    ));
    // every item is a Symfony SplFileInfo: the iterator's item type
    for item in finder1.get_iterator()? {
        let _: SplFileInfo = item?;
    }
    Ok(())
}

fn format_for_assert(finder: &Finder) -> Result<Vec<(String, String)>, Error> {
    finder
        .get_iterator()?
        .map(|f| {
            f.map(|f| {
                (
                    String::from_utf8_lossy(f.get_pathname()).into_owned(),
                    String::from_utf8_lossy(f.get_relative_pathname()).into_owned(),
                )
            })
        })
        .collect()
}

fn abc_tree() -> Node {
    dir(vec![(
        "a",
        dir(vec![
            ("a1", Node::File),
            ("a2", Node::File),
            (
                "b",
                dir(vec![
                    ("b1", Node::File),
                    ("b2", Node::File),
                    ("c", dir(vec![("c1", Node::File), ("c2", Node::File)])),
                ]),
            ),
        ]),
    )])
}

fn pairs(dir: &str, rows: &[(&str, &str)]) -> Vec<(String, String)> {
    rows.iter()
        .map(|(k, r)| (format!("{dir}{k}"), r.replace("{dir}", dir)))
        .collect()
}

fn canonical(mut v: Vec<(String, String)>) -> Vec<(String, String)> {
    v.sort();
    v
}

#[test]
fn testRelativePathWithoutAppend() -> TestResult {
    let vfs = VfsScheme::setup(abc_tree());
    let dir = vfs.dir();
    let mut finder = Finder::create();
    finder.sort_by_name(false).r#in(format!("{dir}a/b"))?;
    assert_eq!(
        pairs(
            &dir,
            &[
                ("a/b/b1", "b1"),
                ("a/b/b2", "b2"),
                ("a/b/c", "c"),
                ("a/b/c/c1", "c/c1"),
                ("a/b/c/c2", "c/c2")
            ]
        ),
        format_for_assert(&finder)?
    );
    Ok(())
}

#[test]
fn testRelativePathWithAppendedFinderForParentDirectory() -> TestResult {
    let vfs = VfsScheme::setup(abc_tree());
    let dir = vfs.dir();
    let mut finder = Finder::create();
    finder.sort_by_name(true).r#in(format!("{dir}a/b"))?;
    let mut appended = Finder::create();
    appended.r#in(format!("{dir}a"))?;
    finder.append(appended);
    let expected = pairs(
        &dir,
        &[
            ("a/a1", "a1"),
            ("a/a2", "a2"),
            ("a/b", "b"),
            ("a/b/b1", "b1"),
            ("a/b/b1", "b/b1"),
            ("a/b/b2", "b2"),
            ("a/b/b2", "b/b2"),
            ("a/b/c", "c"),
            ("a/b/c", "b/c"),
            ("a/b/c/c1", "c/c1"),
            ("a/b/c/c1", "b/c/c1"),
            ("a/b/c/c2", "c/c2"),
            ("a/b/c/c2", "b/c/c2"),
        ],
    );
    assert_eq!(canonical(expected), canonical(format_for_assert(&finder)?));
    Ok(())
}

#[test]
fn testRelativePathWithAppendedFinderForChildDirectory() -> TestResult {
    let vfs = VfsScheme::setup(abc_tree());
    let dir = vfs.dir();
    let mut finder = Finder::create();
    finder.sort_by_name(true).r#in(format!("{dir}a/b"))?;
    let mut appended = Finder::create();
    appended.r#in(format!("{dir}a/b/c"))?;
    finder.append(appended);
    let expected = pairs(
        &dir,
        &[
            ("a/b/b1", "b1"),
            ("a/b/b2", "b2"),
            ("a/b/c", "c"),
            ("a/b/c/c1", "c/c1"),
            ("a/b/c/c1", "c1"),
            ("a/b/c/c2", "c/c2"),
            ("a/b/c/c2", "c2"),
        ],
    );
    assert_eq!(canonical(expected), canonical(format_for_assert(&finder)?));
    Ok(())
}

#[test]
fn testRelativePathWithAppendedPaths() -> TestResult {
    let vfs = VfsScheme::setup(abc_tree());
    let dir = vfs.dir();
    let mut finder = Finder::create();
    finder.sort_by_name(true).r#in(format!("{dir}a/b"))?;
    finder.append(vec![format!("{dir}a/a1"), format!("{dir}a/b/c/c1")]);
    let expected = pairs(
        &dir,
        &[
            ("a/a1", "{dir}a/a1"),
            ("a/b/b1", "b1"),
            ("a/b/b2", "b2"),
            ("a/b/c", "c"),
            ("a/b/c/c1", "c/c1"),
            ("a/b/c/c1", "{dir}a/b/c/c1"),
            ("a/b/c/c2", "c/c2"),
        ],
    );
    assert_eq!(canonical(expected), canonical(format_for_assert(&finder)?));
    Ok(())
}

#[test]
fn testRelativePathWithAppendedOnEmptyFinder() -> TestResult {
    let vfs = VfsScheme::setup(abc_tree());
    let dir = vfs.dir();
    let mut finder = Finder::create();
    finder.sort_by_name(false);
    finder.append(vec![format!("{dir}a/a1")]);
    assert_eq!(
        pairs(&dir, &[("a/a1", "{dir}a/a1")]),
        format_for_assert(&finder)?
    );
    Ok(())
}

#[test]
fn testAppendReturnsAFinder() {
    let mut finder = Finder::create();
    let _: &mut Finder = finder.append(Vec::<String>::new());
}

#[test]
fn testAppendEmptyIterableAllowsIteration() -> TestResult {
    let mut finder = Finder::create();
    finder.files().name("*.php").append(Vec::<String>::new());
    assert_eq!(0, finder.get_iterator()?.count());
    Ok(())
}

#[test]
fn testAppendDoesNotRequireIn() -> TestResult {
    let mut finder = build_finder();
    finder.r#in(format!("{}/foo", tmp()))?;
    let expected = pathnames(&finder)?;
    let mut finder1 = Finder::create();
    finder1.append(finder);
    assert_iterator(expected, finder1.get_iterator()?);
    Ok(())
}

#[test]
fn testMultipleAppendCallsWithSorting() -> TestResult {
    let mut finder = build_finder();
    finder
        .sort_by_name(false)
        .append(abs(&["qux_1000_1.php"]))
        .append(abs(&["qux_1002_0.php"]));
    assert_ordered_iterator(
        abs(&["qux_1000_1.php", "qux_1002_0.php"]),
        finder.get_iterator()?,
    );
    Ok(())
}

#[test]
fn testCountDirectories() -> TestResult {
    let mut directory = Finder::create();
    directory.directories().r#in(tmp())?;
    let mut i = 0;
    for dir in directory.get_iterator()? {
        dir?;
        i += 1;
    }
    assert_eq!(i, directory.count()?);
    Ok(())
}

#[test]
fn testCountFiles() -> TestResult {
    let mut files = Finder::create();
    files
        .files()
        .r#in(format!("{}/Fixtures", upstream_tests()))?;
    let mut i = 0;
    for file in files.get_iterator()? {
        file?;
        i += 1;
    }
    assert_eq!(i, files.count()?);
    Ok(())
}

#[test]
fn testCountWithoutIn() {
    let mut finder = Finder::create();
    finder.files();
    let e = finder.count().err();
    assert!(e.is_some_and(|e| e.is_instance_of("LogicException")));
}

#[test]
fn testHasResults() -> TestResult {
    let mut finder = build_finder();
    finder.r#in(upstream_tests())?;
    assert!(finder.has_results()?);
    Ok(())
}

#[test]
fn testNoResults() -> TestResult {
    let mut finder = build_finder();
    finder.r#in(upstream_tests())?.name("DoesNotExist");
    assert!(!finder.has_results()?);
    Ok(())
}

#[test]
fn testContains() -> TestResult {
    let rows = provider("FinderTest", "testContains");
    assert_eq!(rows.len(), 12);
    for (_, args) in rows {
        let mut finder = build_finder();
        finder
            .r#in(format!("{}/Fixtures", upstream_tests()))?
            .name("*.txt")
            .sort_by_name(false)
            .contains(strings(&args[0]))
            .not_contains(strings(&args[1]));
        let expected: Vec<String> = strings(&args[2]);
        let expected: Vec<&str> = expected.iter().map(String::as_str).collect();
        assert_iterator(fixtures(&expected), finder.get_iterator()?);
    }
    Ok(())
}

#[test]
fn testContainsOnDirectory() -> TestResult {
    let mut finder = build_finder();
    finder
        .r#in(upstream_tests())?
        .directories()
        .name("Fixtures")
        .contains("abc");
    assert_iterator(Vec::new(), finder.get_iterator()?);
    Ok(())
}

#[test]
fn testNotContainsOnDirectory() -> TestResult {
    let mut finder = build_finder();
    finder
        .r#in(upstream_tests())?
        .directories()
        .name("Fixtures")
        .not_contains("abc");
    assert_iterator(Vec::new(), finder.get_iterator()?);
    Ok(())
}

#[test]
fn testMultipleLocations() -> TestResult {
    let locations = vec![format!("{}/", tmp()), format!("{}/toto/", tmp())];
    // it is expected that there are test.py test.php in the tmpDir
    let mut finder = Finder::new();
    finder
        .r#in(locations)?
        // the default flag IGNORE_DOT_FILES fixes the problem indirectly
        // so we set it to false for better isolation
        .ignore_dot_files(false)
        .depth("< 1")?
        .name("test.php");
    assert_eq!(1, finder.count()?);
    Ok(())
}

#[test]
fn testMultipleLocationsWithSubDirectories() -> TestResult {
    let dir = upstream_tests();
    let locations = vec![format!("{dir}/Fixtures/one"), format!("{}/toto", tmp())];
    let mut finder = build_finder();
    finder.r#in(locations)?.depth("< 10")?.name("*.neon");
    let expected = vec![
        format!("{dir}/Fixtures/one/b/c.neon"),
        format!("{dir}/Fixtures/one/b/d.neon"),
    ];
    assert_iterator(expected.clone(), finder.get_iterator()?);
    // assertIteratorInForeach
    let mut values = pathnames(&finder)?;
    values.sort();
    assert_eq!(expected, values);
    Ok(())
}

#[test]
fn testIteratorKeys() -> TestResult {
    // the key of each file is its pathname: the iterator yields the files,
    // whose getPathname() is the key PHP gives
    let mut finder = build_finder();
    finder.r#in(tmp())?;
    for file in finder.get_iterator()? {
        let file = file?;
        assert_eq!(file.to_string().as_bytes(), file.get_pathname());
    }
    Ok(())
}

#[test]
fn testRegexSpecialCharsLocationWithPathRestrictionContainingStartFlag() -> TestResult {
    let mut finder = build_finder();
    finder
        .r#in(format!("{}/Fixtures/r+e.gex[c]a(r)s", upstream_tests()))?
        .path("/^dir/");
    assert_iterator(
        fixtures(&["r+e.gex[c]a(r)s/dir", "r+e.gex[c]a(r)s/dir/bar.dat"]),
        finder.get_iterator()?,
    );
    Ok(())
}

#[test]
fn testPath() -> TestResult {
    let rows = provider("FinderTest", "testPath");
    assert_eq!(rows.len(), 8);
    for (_, args) in rows {
        let mut finder = build_finder();
        finder
            .r#in(format!("{}/Fixtures", upstream_tests()))?
            .path(strings(&args[0]))
            .not_path(strings(&args[1]));
        let expected = strings(&args[2]);
        let expected: Vec<&str> = expected.iter().map(String::as_str).collect();
        assert_iterator(fixtures(&expected), finder.get_iterator()?);
    }
    Ok(())
}

#[cfg(unix)]
fn chmod(path: &str, mode: u32) {
    use std::os::unix::fs::PermissionsExt;
    std::fs::set_permissions(path, std::fs::Permissions::from_mode(mode))
        .unwrap_or_else(|e| panic!("chmod {path}: {e}"));
}

#[test]
fn testAccessDeniedException() -> TestResult {
    #[cfg(unix)]
    {
        let dir = real_fixture("FinderTest_access_denied", true);
        let mut finder = build_finder();
        finder.files().r#in(dir.as_str())?;
        // make 'foo' directory non-readable
        let test_dir = format!("{dir}/foo");
        chmod(&test_dir, 0o333);
        let could_read = std::fs::read_dir(&test_dir).is_ok();
        if !could_read {
            let result: Result<Vec<SplFileInfo>, Error> = finder.get_iterator()?.collect();
            chmod(&test_dir, 0o777);
            remove_dir(&dir);
            let e = result
                .err()
                .ok_or("Finder should throw an exception when opening a non-readable directory.")?;
            assert!(e.is_instance_of(r"Symfony\Component\Finder\Exception\AccessDeniedException"));
        } else {
            // could read test files while test requires unreadable (root)
            chmod(&test_dir, 0o777);
            remove_dir(&dir);
        }
    }
    Ok(())
}

#[test]
fn testIgnoredAccessDeniedException() -> TestResult {
    #[cfg(unix)]
    {
        let dir = real_fixture("FinderTest_ignored_access_denied", true);
        let mut finder = build_finder();
        finder
            .files()
            .ignore_unreadable_dirs(true)
            .r#in(dir.as_str())?;
        let test_dir = format!("{dir}/foo");
        chmod(&test_dir, 0o333);
        if std::fs::read_dir(&test_dir).is_err() {
            let result: Result<Vec<SplFileInfo>, Error> = finder.get_iterator()?.collect();
            chmod(&test_dir, 0o777);
            let expected: Vec<&str> = FILES_ONLY
                .iter()
                .copied()
                .filter(|f| *f != "foo/bar.tmp")
                .collect();
            assert_iterator(abs_in(&dir, &expected), result?.into_iter().map(Ok));
        }
        chmod(&test_dir, 0o777);
        remove_dir(&dir);
    }
    Ok(())
}

#[test]
fn testNormalizeDirKeepsTrailingSlashForStreamWrappers() {
    assert_eq!(
        b"s3://bucket/test_dir/".to_vec(),
        Finder::normalize_dir(b"s3://bucket/test_dir")
    );
    assert_eq!(
        b"sftp://host/test_dir/".to_vec(),
        Finder::normalize_dir(b"sftp://host/test_dir")
    );
    let dir = upstream_tests();
    assert_eq!(
        dir.as_bytes().to_vec(),
        Finder::normalize_dir(format!("{dir}/").as_bytes())
    );
}

// --- GlobTest: the tests that walk the Tests directory --------------------

fn glob_matches(regex: &[u8]) -> Result<Vec<String>, Box<dyn std::error::Error + Send + Sync>> {
    let dir = upstream_tests();
    let regex = symfony_finder::pcre::Pcre::new(regex)?;
    let mut finder = Finder::new();
    finder.ignore_dot_files(false);
    let mut matches = Vec::new();
    for file in finder.r#in(dir.as_str())?.get_iterator()? {
        let k = String::from_utf8_lossy(file?.get_pathname()).into_owned();
        if regex.is_match(&k.as_bytes()[dir.len()..])? {
            matches.push(k[10 + dir.len()..].to_string());
        }
    }
    matches.sort();
    Ok(matches)
}

#[test]
fn GlobTest_testGlobToRegexDoubleStarStrictDots() -> TestResult {
    let regex = symfony_finder::Glob::to_regex(b"/**/*.neon", true, true, Some(b'#'));
    assert_eq!(vec!["one/b/c.neon", "one/b/d.neon"], glob_matches(&regex)?);
    Ok(())
}

#[test]
fn GlobTest_testGlobToRegexDoubleStarNonStrictDots() -> TestResult {
    let regex = symfony_finder::Glob::to_regex(b"/**/*.neon", false, true, Some(b'#'));
    assert_eq!(
        vec![
            ".dot/b/c.neon",
            ".dot/b/d.neon",
            "one/b/c.neon",
            "one/b/d.neon"
        ],
        glob_matches(&regex)?
    );
    Ok(())
}

#[test]
fn GlobTest_testGlobToRegexDoubleStarWithoutLeadingSlash() -> TestResult {
    let regex = symfony_finder::Glob::to_regex(b"/Fixtures/one/**", true, true, Some(b'#'));
    assert_eq!(
        vec!["one/a", "one/b", "one/b/c.neon", "one/b/d.neon"],
        glob_matches(&regex)?
    );
    Ok(())
}

#[test]
fn GlobTest_testGlobToRegexDoubleStarWithoutLeadingSlashNotStrictLeadingDot() -> TestResult {
    let regex = symfony_finder::Glob::to_regex(b"/Fixtures/one/**", false, true, Some(b'#'));
    assert_eq!(
        vec!["one/.dot", "one/a", "one/b", "one/b/c.neon", "one/b/d.neon"],
        glob_matches(&regex)?
    );
    Ok(())
}
