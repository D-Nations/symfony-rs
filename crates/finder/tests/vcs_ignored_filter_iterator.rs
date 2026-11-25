//! VcsIgnoredFilterIteratorTest.php at symfony/finder
//! 2f4c0f26106520d82e1b71759d2fb8d32d8b7a43. Names and assertions are
//! PHPUnit's; testAccept replays the recorded `getAcceptData` rows. Each
//! test gets its own directory (`symfony_finder_vcs_ignored_<test>`),
//! since Rust runs the tests in parallel where PHPUnit runs them one by
//! one in a shared one. The symlink tests do not run where the system
//! refuses symbolic links (Windows without the privilege), as upstream
//! skips them on Windows.
//!
//! Upstream: symfony/finder Tests/Iterator/VcsIgnoredFilterIteratorTest.php
#![allow(non_snake_case)]
#![allow(
    clippy::unused_self,
    reason = "test code ports PHPUnit tests and helpers as they are"
)]

mod common;

use std::fs;

use common::{
    TestResult,
    fixture::{
        assert_iterator, can_symlink, infos, remove_dir, symlink, temp_root, touch, upstream_tests,
    },
    provider, string, strings,
};
use php_value::Value;
use symfony_finder::{Finder, iterator::VcsIgnoredFilterIterator};

struct TmpDir(String);

impl TmpDir {
    fn new(test: &str) -> Self {
        let dir = format!("{}/symfony_finder_vcs_ignored_{test}", temp_root());
        remove_dir(&dir);
        fs::create_dir(&dir).unwrap_or_else(|e| panic!("mkdir {dir}: {e}"));
        Self(dir)
    }

    fn path(&self, file: &str) -> String {
        format!("{}/{file}", self.0)
    }

    fn to_absolute(&self, files: &[&str]) -> Vec<String> {
        files.iter().map(|f| self.path(f)).collect()
    }

    fn mkdir(&self, dir: &str) {
        fs::create_dir_all(self.path(dir)).unwrap_or_else(|e| panic!("mkdir {dir}: {e}"));
    }

    fn put(&self, file: &str, content: &str) {
        fs::write(self.path(file), content).unwrap_or_else(|e| panic!("write {file}: {e}"));
    }

    fn touch(&self, file: &str) {
        touch(&self.path(file), None);
    }

    fn finder(&self, dir: &str) -> Result<Finder, symfony_finder::Error> {
        let mut finder = Finder::create();
        finder.files().r#in(dir)?.ignore_vcs_ignored(true);
        Ok(finder)
    }
}

impl Drop for TmpDir {
    fn drop(&mut self) {
        remove_dir(&self.0);
    }
}

#[test]
fn testAccept() -> TestResult {
    let rows = provider("VcsIgnoredFilterIteratorTest", "testAccept");
    assert_eq!(rows.len(), 24);
    for (i, (name, args)) in rows.into_iter().enumerate() {
        let tmp = TmpDir::new(&format!("accept_{i}"));
        let other: Vec<String> = strings(&args[1]).iter().map(|f| tmp.path(f)).collect();
        for path in &other {
            if path.ends_with('/') {
                fs::create_dir(path).unwrap_or_else(|e| panic!("{name}: mkdir {path}: {e}"));
            } else {
                touch(path, None);
            }
        }
        if let Value::Array(files) = &args[0] {
            for (path, content) in files.iter() {
                let path = path.to_value();
                tmp.put(&string(&path), &string(content));
            }
        }
        let base_dir = match args.get(3) {
            Some(v) if !string(v).is_empty() => tmp.path(&string(v)),
            _ => tmp.0.clone(),
        };
        let iterator = VcsIgnoredFilterIterator::new(infos(&other), base_dir.as_bytes())?;
        let expected: Vec<String> = strings(&args[2]).iter().map(|f| tmp.path(f)).collect();
        let mut actual: Vec<String> = iterator
            .map(|f| f.map(|f| String::from_utf8_lossy(f.get_pathname()).into_owned()))
            .collect::<Result<_, _>>()?;
        let mut expected = expected;
        actual.sort();
        expected.sort();
        assert_eq!(expected, actual, "{name}");
    }
    Ok(())
}

#[test]
fn testAcceptAtRootDirectory() -> TestResult {
    // __FILE__: this test's own file (upstream/ is ignored by this
    // repository's .gitignore, so an upstream file would be filtered)
    let file = symfony_finder::fs::realpath(
        concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/tests/vcs_ignored_filter_iterator.rs"
        )
        .as_bytes(),
    )
    .map(|p| String::from_utf8_lossy(&p).into_owned())
    .ok_or("realpath")?;
    let iterator = VcsIgnoredFilterIterator::new(infos(std::slice::from_ref(&file)), b"/")?;
    assert_iterator(vec![file], iterator);
    Ok(())
}

#[test]
fn testAcceptWithSymlinkedBaseDirectory() -> TestResult {
    if !can_symlink() {
        return Ok(());
    }
    let tmp = TmpDir::new("symlinked_base");
    tmp.mkdir("a");
    tmp.touch("a/file.txt");
    tmp.touch("b.txt");
    tmp.put(".gitignore", "a/\n");
    let link = format!("{}/symfony_finder_vcs_link", temp_root());
    remove_dir(&link);
    let _ = fs::remove_file(&link);
    symlink(&tmp.0, &link).ok_or("symlink")?;
    let inner = infos(&[format!("{link}/a/file.txt"), format!("{link}/b.txt")]);
    let iterator = VcsIgnoredFilterIterator::new(inner, link.as_bytes())?;
    let result: Vec<String> = iterator
        .map(|f| f.map(|f| String::from_utf8_lossy(f.get_pathname()).into_owned()))
        .collect::<Result<_, _>>()?;
    let _ = fs::remove_file(&link);
    assert_eq!(vec![format!("{link}/b.txt")], result);
    Ok(())
}

#[test]
fn testIgnoreGitInfoExclude() -> TestResult {
    let tmp = TmpDir::new("info_exclude");
    tmp.mkdir(".git/info");
    tmp.put(".git/info/exclude", "excluded.txt\n");
    tmp.touch("excluded.txt");
    tmp.touch("visible.txt");
    assert_iterator(
        tmp.to_absolute(&["visible.txt"]),
        tmp.finder(&tmp.0)?.get_iterator()?,
    );
    Ok(())
}

#[test]
fn testGitignoreRulesTakePrecedenceOverGitInfoExclude() -> TestResult {
    let tmp = TmpDir::new("precedence");
    tmp.mkdir(".git/info");
    tmp.put(".git/info/exclude", "excluded.txt\n");
    tmp.put(".gitignore", "!excluded.txt\n");
    tmp.touch("excluded.txt");
    tmp.touch("visible.txt");
    assert_iterator(
        tmp.to_absolute(&["excluded.txt", "visible.txt"]),
        tmp.finder(&tmp.0)?.get_iterator()?,
    );
    Ok(())
}

#[test]
fn testGitignoreRulesCanOverrideNegatedGitInfoExcludeRules() -> TestResult {
    let tmp = TmpDir::new("override_negated");
    tmp.mkdir(".git/info");
    tmp.put(".git/info/exclude", "!excluded.txt\n");
    tmp.put(".gitignore", "excluded.txt\n");
    tmp.touch("excluded.txt");
    tmp.touch("visible.txt");
    assert_iterator(
        tmp.to_absolute(&["visible.txt"]),
        tmp.finder(&tmp.0)?.get_iterator()?,
    );
    Ok(())
}

#[test]
fn testGitInfoExcludePatternsAreRelativeToRepositoryRoot() -> TestResult {
    let tmp = TmpDir::new("relative_to_root");
    tmp.mkdir(".git/info");
    tmp.mkdir("nested");
    tmp.put(".git/info/exclude", "nested/excluded.txt\n");
    tmp.touch("nested/excluded.txt");
    tmp.touch("nested/visible.txt");
    tmp.touch("visible.txt");
    assert_iterator(
        tmp.to_absolute(&["nested/visible.txt", "visible.txt"]),
        tmp.finder(&tmp.0)?.get_iterator()?,
    );
    Ok(())
}

#[test]
fn testGitInfoExcludeLastMatchingRuleWins() -> TestResult {
    let tmp = TmpDir::new("last_rule");
    tmp.mkdir(".git/info");
    tmp.put(".git/info/exclude", "*.txt\n!visible.txt\nvisible.txt\n");
    tmp.touch("excluded.txt");
    tmp.touch("visible.txt");
    tmp.touch("other.log");
    assert_iterator(
        tmp.to_absolute(&["other.log"]),
        tmp.finder(&tmp.0)?.get_iterator()?,
    );
    Ok(())
}

#[test]
fn testGitInfoExcludeDirectoryPatternsAreAppliedToContents() -> TestResult {
    let tmp = TmpDir::new("directory_patterns");
    tmp.mkdir(".git/info");
    tmp.mkdir("build");
    tmp.put(".git/info/exclude", "build/\n");
    tmp.touch("build/generated.php");
    tmp.touch("build.txt");
    tmp.touch("visible.txt");
    assert_iterator(
        tmp.to_absolute(&["build.txt", "visible.txt"]),
        tmp.finder(&tmp.0)?.get_iterator()?,
    );
    Ok(())
}

#[test]
fn testNestedGitignoreRulesTakePrecedenceOverGitInfoExclude() -> TestResult {
    let tmp = TmpDir::new("nested_precedence");
    tmp.mkdir(".git/info");
    tmp.mkdir("nested");
    tmp.put(".git/info/exclude", "nested/excluded.txt\n");
    tmp.put("nested/.gitignore", "!excluded.txt\n");
    tmp.touch("nested/excluded.txt");
    tmp.touch("nested/visible.txt");
    assert_iterator(
        tmp.to_absolute(&["nested/excluded.txt", "nested/visible.txt"]),
        tmp.finder(&tmp.0)?.get_iterator()?,
    );
    Ok(())
}

#[test]
fn testGitInfoExcludeSupportsWorktrees() -> TestResult {
    let tmp = TmpDir::new("worktrees");
    tmp.mkdir(".git-data/worktrees/main");
    tmp.mkdir(".git-data/info");
    tmp.put(".git", "gitdir: .git-data/worktrees/main\n");
    tmp.put(".git-data/worktrees/main/commondir", "../..\n");
    tmp.put(".git-data/info/exclude", "excluded.txt\n");
    tmp.touch("excluded.txt");
    tmp.touch("visible.txt");
    assert_iterator(
        tmp.to_absolute(&["visible.txt"]),
        tmp.finder(&tmp.0)?.get_iterator()?,
    );
    Ok(())
}

#[test]
fn testGitInfoExcludeIsAppliedWhenFinderStartsInSubdirectory() -> TestResult {
    let tmp = TmpDir::new("subdirectory");
    tmp.mkdir(".git/info");
    tmp.mkdir("nested");
    tmp.put(".git/info/exclude", "nested/excluded.txt\n");
    tmp.touch("nested/excluded.txt");
    tmp.touch("nested/visible.txt");
    assert_iterator(
        tmp.to_absolute(&["nested/visible.txt"]),
        tmp.finder(&tmp.path("nested"))?.get_iterator()?,
    );
    Ok(())
}

#[test]
fn testGitInfoExcludeSupportsSubmodules() -> TestResult {
    let tmp = TmpDir::new("submodules");
    let git_dir = format!("{}_git", tmp.0);
    remove_dir(&git_dir);
    fs::create_dir_all(format!("{git_dir}/modules/submodule/info"))?;
    tmp.put(".git", &format!("gitdir: {git_dir}/modules/submodule\n"));
    fs::write(
        format!("{git_dir}/modules/submodule/info/exclude"),
        "excluded.txt\n",
    )?;
    tmp.touch("excluded.txt");
    tmp.touch("visible.txt");
    let result = tmp.finder(&tmp.0)?.get_iterator();
    let files: Result<Vec<_>, _> = result?.collect();
    remove_dir(&git_dir);
    assert_iterator(
        tmp.to_absolute(&["visible.txt"]),
        files?.into_iter().map(Ok),
    );
    Ok(())
}

#[test]
fn testGitInfoExcludeIsNotMatchedAgainstSymlinkTargetsOutsideTheRepository() -> TestResult {
    if !can_symlink() {
        return Ok(());
    }
    let tmp = TmpDir::new("symlink_targets");
    let outside = format!("{}/symfony_finder_vcs_outside", temp_root());
    remove_dir(&outside);
    fs::create_dir(&outside)?;
    touch(&format!("{outside}/target.txt"), None);
    tmp.mkdir(".git/info");
    tmp.put(".git/info/exclude", "target.txt\n");
    symlink(&format!("{outside}/target.txt"), &tmp.path("link.txt")).ok_or("symlink")?;
    tmp.touch("target.txt");
    let files: Result<Vec<_>, _> = tmp.finder(&tmp.0)?.get_iterator()?.collect();
    remove_dir(&outside);
    assert_iterator(tmp.to_absolute(&["link.txt"]), files?.into_iter().map(Ok));
    Ok(())
}

#[test]
fn testAcceptMatchesSymlinksAgainstTheirOwnPath() -> TestResult {
    if !can_symlink() {
        return Ok(());
    }
    let tmp = TmpDir::new("own_path");
    tmp.put(".gitignore", "/build/\n/vendor/\n/dir-link/\n*.lnk\n");
    tmp.mkdir("build");
    tmp.mkdir("src");
    tmp.mkdir("vendor");
    tmp.touch("build/generated.php");
    tmp.touch("src/app.php");
    symlink("../src/app.php", &tmp.path("build/app.php")).ok_or("symlink")?;
    symlink("../build/generated.php", &tmp.path("src/generated.php")).ok_or("symlink")?;
    symlink("../build", &tmp.path("src/build")).ok_or("symlink")?;
    symlink(
        &format!("{}/Iterator", upstream_tests()),
        &tmp.path("vendor/outside"),
    )
    .ok_or("symlink")?;
    symlink("src", &tmp.path("dir-link")).ok_or("symlink")?;
    symlink("src/app.php", &tmp.path("app.lnk")).ok_or("symlink")?;
    let inner = infos(&tmp.to_absolute(&[
        "build/app.php",
        "src/generated.php",
        "src/build",
        "src/build/generated.php",
        "vendor/outside",
        "vendor/outside/InnerNameIterator.php",
        "dir-link",
        "dir-link/app.php",
        "app.lnk",
    ]));
    let iterator = VcsIgnoredFilterIterator::new(inner, tmp.0.as_bytes())?;
    assert_iterator(
        tmp.to_absolute(&[
            "src/generated.php",
            "src/build",
            "src/build/generated.php",
            "dir-link",
            "dir-link/app.php",
        ]),
        iterator,
    );
    Ok(())
}
