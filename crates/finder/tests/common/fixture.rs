//! RealIteratorTestCase and IteratorTestCase helpers: the fixture tree in
//! a temporary directory, and the iterator assertions.
//!
//! Upstream: none

use std::{
    fs,
    time::{Duration, SystemTime},
};

use symfony_finder::{Error, FileInfo, SplFileInfo};

/// `realpath(sys_get_temp_dir())` with `/` separators.
pub fn temp_root() -> String {
    let dir = std::env::temp_dir();
    let real = symfony_finder::fs::realpath(dir.to_string_lossy().as_bytes())
        .unwrap_or_else(|| panic!("no temp dir"));
    String::from_utf8_lossy(&real).into_owned()
}

/// The upstream Tests directory (`__DIR__` of the upstream tests).
pub fn upstream_tests() -> String {
    let dir = concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../upstream/symfony/finder/Tests"
    );
    let real = symfony_finder::fs::realpath(dir.as_bytes())
        .unwrap_or_else(|| panic!("upstream/ is not fetched (scripts/fetch-upstream.sh)"));
    String::from_utf8_lossy(&real).into_owned()
}

/// The files of RealIteratorTestCase, relative to its directory.
pub const FILES: [&str; 23] = [
    ".git/",
    ".foo/",
    ".foo/.bar",
    ".foo/bar",
    ".bar",
    "test.py",
    "foo/",
    "foo/bar.tmp",
    "test.php",
    "toto/",
    "toto/.git/",
    "foo bar",
    "qux_0_1.php",
    "qux_2_0.php",
    "qux_10_2.php",
    "qux_12_0.php",
    "qux_1000_1.php",
    "qux_1002_0.php",
    "qux/",
    "qux/baz_1_2.py",
    "qux/baz_100_1.py",
    "zebulon.php",
    "Zephire.php",
];

/// `touch($path, $time)`.
pub fn touch(path: &str, time: Option<SystemTime>) {
    let is_dir = fs::metadata(path).is_ok_and(|m| m.is_dir());
    let mut options = fs::OpenOptions::new();
    if is_dir {
        #[cfg(windows)]
        {
            use std::os::windows::fs::OpenOptionsExt;
            // FILE_WRITE_ATTRIBUTES, FILE_FLAG_BACKUP_SEMANTICS
            options.access_mode(0x100).custom_flags(0x0200_0000);
        }
        #[cfg(not(windows))]
        options.read(true);
    } else {
        options.create(true).append(true);
    }
    let file = options
        .open(path)
        .unwrap_or_else(|e| panic!("touch {path}: {e}"));
    let time = time.or_else(|| is_dir.then(SystemTime::now));
    if let Some(t) = time {
        file.set_times(fs::FileTimes::new().set_accessed(t).set_modified(t))
            .unwrap_or_else(|e| panic!("touch {path}: {e}"));
    }
}

/// Filesystem::remove() of a tree.
pub fn remove_dir(path: &str) {
    if fs::symlink_metadata(path).is_ok() {
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            for entry in walk(path) {
                let _ = fs::set_permissions(&entry, fs::Permissions::from_mode(0o777));
            }
        }
        let _ = fs::remove_dir_all(path);
    }
}

#[cfg(unix)]
fn walk(path: &str) -> Vec<String> {
    let mut out = vec![path.to_string()];
    if let Ok(entries) = fs::read_dir(path) {
        for e in entries.flatten() {
            if e.file_type().is_ok_and(|t| t.is_dir()) {
                out.extend(walk(&e.path().to_string_lossy()));
            }
        }
    }
    out
}

/// setUpBeforeClass(): the fixture tree in `<temp>/symfony_finder_<name>`;
/// FinderTest adds the gitignore tree.
pub fn real_fixture(name: &str, with_gitignore: bool) -> String {
    let tmp = format!("{}/symfony_finder_{name}", temp_root());
    remove_dir(&tmp);
    fs::create_dir(&tmp).unwrap_or_else(|e| panic!("mkdir {tmp}: {e}"));
    let mut files: Vec<&str> = FILES.to_vec();
    if with_gitignore {
        files.push("gitignore/");
    }
    for file in files {
        let path = format!("{tmp}/{file}");
        if file.ends_with('/') {
            fs::create_dir(&path).unwrap_or_else(|e| panic!("mkdir {path}: {e}"));
        } else {
            touch(&path, None);
        }
    }
    fs::write(format!("{tmp}/test.php"), " ".repeat(800)).unwrap_or_else(|e| panic!("{e}"));
    fs::write(format!("{tmp}/test.py"), " ".repeat(2000)).unwrap_or_else(|e| panic!("{e}"));
    let one_year_ago = SystemTime::now() - Duration::from_secs(365 * 86_400);
    touch(&format!("{tmp}/foo/bar.tmp"), Some(one_year_ago));
    touch(&format!("{tmp}/test.php"), Some(one_year_ago));
    if with_gitignore {
        mirror(
            &format!("{}/Fixtures/gitignore", upstream_tests()),
            &format!("{tmp}/gitignore"),
        );
        for file in [
            "gitignore/search_root/a.txt",
            "gitignore/search_root/c.txt",
            "gitignore/search_root/dir/b.txt",
            "gitignore/search_root/dir/c.txt",
            "gitignore/git_root/search_root/a.txt",
            "gitignore/git_root/search_root/c.txt",
            "gitignore/git_root/search_root/dir/b.txt",
            "gitignore/git_root/search_root/dir/c.txt",
        ] {
            touch(&format!("{tmp}/{file}"), None);
        }
        fs::create_dir_all(format!("{tmp}/gitignore/git_root/.git"))
            .unwrap_or_else(|e| panic!("{e}"));
    }
    tmp
}

/// Filesystem::mirror() of a plain tree.
pub fn mirror(from: &str, to: &str) {
    fs::create_dir_all(to).unwrap_or_else(|e| panic!("mkdir {to}: {e}"));
    for entry in fs::read_dir(from)
        .unwrap_or_else(|e| panic!("{from}: {e}"))
        .flatten()
    {
        let name = entry.file_name().to_string_lossy().into_owned();
        let (src, dst) = (format!("{from}/{name}"), format!("{to}/{name}"));
        if entry.file_type().is_ok_and(|t| t.is_dir()) {
            mirror(&src, &dst);
        } else {
            fs::copy(&src, &dst).unwrap_or_else(|e| panic!("copy {src}: {e}"));
        }
    }
}

/// `toAbsolute($files)`.
pub fn to_absolute(tmp: &str, files: &[&str]) -> Vec<String> {
    files.iter().map(|f| format!("{tmp}/{f}")).collect()
}

fn pathnames<I, F>(iterator: I) -> Vec<String>
where
    I: IntoIterator<Item = Result<F, Error>>,
    F: FileInfo,
{
    iterator
        .into_iter()
        .map(|f| match f {
            Ok(f) => String::from_utf8_lossy(f.get_pathname()).into_owned(),
            Err(e) => panic!("iteration threw {e}"),
        })
        .collect()
}

/// IteratorTestCase::assertIterator(): the pathnames, in any order.
pub fn assert_iterator<I, F>(expected: Vec<String>, iterator: I)
where
    I: IntoIterator<Item = Result<F, Error>>,
    F: FileInfo,
{
    let mut values = pathnames(iterator);
    let mut expected = expected;
    values.sort();
    expected.sort();
    assert_eq!(expected, values);
}

/// IteratorTestCase::assertOrderedIterator().
pub fn assert_ordered_iterator<I, F>(expected: Vec<String>, iterator: I)
where
    I: IntoIterator<Item = Result<F, Error>>,
    F: FileInfo,
{
    assert_eq!(expected, pathnames(iterator));
}

/// IteratorTestCase::assertOrderedIteratorForGroups().
pub fn assert_ordered_iterator_for_groups<I, F>(expected: Vec<Vec<String>>, iterator: I)
where
    I: IntoIterator<Item = Result<F, Error>>,
    F: FileInfo,
{
    let mut values = pathnames(iterator);
    for mut group in expected {
        let n = group.len().min(values.len());
        let mut temp: Vec<String> = values.drain(..n).collect();
        temp.sort();
        group.sort();
        assert_eq!(group, temp);
    }
}

/// Plain `\SplFileInfo` objects of paths (the tests' Iterator class).
pub fn infos(paths: &[String]) -> std::vec::IntoIter<Result<SplFileInfo, Error>> {
    paths
        .iter()
        .map(|p| Ok(SplFileInfo::from_path(p.as_bytes().to_vec())))
        .collect::<Vec<_>>()
        .into_iter()
}

/// A symbolic link, or None where the system refuses one (Windows
/// without the privilege): those tests cannot run there.
pub fn symlink(target: &str, link: &str) -> Option<()> {
    #[cfg(unix)]
    {
        std::os::unix::fs::symlink(target, link).ok()
    }
    #[cfg(windows)]
    {
        let resolved = std::path::Path::new(link)
            .parent()
            .map(|p| p.join(target))
            .unwrap_or_else(|| std::path::Path::new(target).to_path_buf());
        if resolved.is_dir() {
            std::os::windows::fs::symlink_dir(target, link).ok()
        } else {
            std::os::windows::fs::symlink_file(target, link).ok()
        }
    }
}

/// Whether the system lets this process create symbolic links.
pub fn can_symlink() -> bool {
    let dir = format!(
        "{}/symfony_finder_symlink_probe_{}",
        temp_root(),
        std::process::id()
    );
    remove_dir(&dir);
    let ok = fs::create_dir(&dir).is_ok() && symlink(".", &format!("{dir}/link")).is_some();
    remove_dir(&dir);
    if !ok {
        eprintln!("symbolic links cannot be created on this system: test not run");
    }
    ok
}
