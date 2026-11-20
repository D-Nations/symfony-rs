//! Port of Symfony\Component\Finder\Iterator\VcsIgnoredFilterIterator.php.
//!
//! Symfony source copyright (c) Fabien Potencier; distributed under MIT.
//!
//! Upstream: symfony/finder Iterator/VcsIgnoredFilterIterator.php

use std::{
    collections::HashMap,
    sync::{Arc, LazyLock},
};

use crate::{
    exception::Error,
    fs,
    gitignore::{Gitignore, patterns, preg_split},
    pcre::Pcre,
    spl_file_info::FileInfo,
};

static TRAILING_SPACES: LazyLock<Option<Pcre>> = LazyLock::new(|| Pcre::new(br"~(?<!\\) +$~").ok());
static GITDIR: LazyLock<Option<Pcre>> = LazyLock::new(|| Pcre::new(br"~^gitdir:\s*(.+)~i").ok());
static ABSOLUTE: LazyLock<Option<Pcre>> =
    LazyLock::new(|| Pcre::new(br"~^(?:[a-zA-Z]:)?[/\\]~").ok());

fn internal(e: impl std::fmt::Display) -> Error {
    Error::Runtime(e.to_string())
}

fn pattern(p: &'static LazyLock<Option<Pcre>>) -> Result<&'static Pcre, Error> {
    p.as_ref()
        .ok_or_else(|| internal("VcsIgnoredFilterIterator pattern"))
}

/// One rule: (regex, is negated, is for directories only).
type Rule = (Arc<Pcre>, bool, bool);

/// Removes the files git ignores.
pub struct VcsIgnoredFilterIterator<I> {
    iterator: I,
    search_dir: Vec<u8>,
    real_search_dir: Vec<u8>,
    base_dir: Vec<u8>,
    git_info_exclude_file: Option<Vec<u8>>,
    gitignore_files_cache: HashMap<Vec<u8>, Option<Vec<Rule>>>,
    ignored_paths_cache: HashMap<Vec<u8>, bool>,
}

fn rtrim_slashes(s: &[u8]) -> &[u8] {
    let mut end = s.len();
    while end > 0 && s[end - 1] == b'/' {
        end -= 1;
    }
    &s[..end]
}

fn concat(parts: &[&[u8]]) -> Vec<u8> {
    parts.concat()
}

impl<I> VcsIgnoredFilterIterator<I> {
    pub fn new(iterator: I, base_dir: &[u8]) -> Result<Self, Error> {
        let search_dir = concat(&[rtrim_slashes(&normalize_path(base_dir)), b"/"]);
        let base = normalize_path(&fs::realpath(base_dir).unwrap_or_else(|| base_dir.to_vec()));
        let real_search_dir = concat(&[rtrim_slashes(&base), b"/"]);
        let mut this = Self {
            iterator,
            search_dir,
            real_search_dir,
            base_dir: base.clone(),
            git_info_exclude_file: None,
            gitignore_files_cache: HashMap::new(),
            ignored_paths_cache: HashMap::new(),
        };
        let mut candidates = vec![base.clone()];
        candidates.extend(parent_directories_upwards(&base));
        for directory in candidates {
            if fs::file_exists(&concat(&[&directory, b"/.git"])) {
                this.base_dir = directory;
                this.git_info_exclude_file = this.get_git_info_exclude_path()?;
                break;
            }
        }
        Ok(this)
    }

    fn accept_path(&mut self, pathname: &[u8]) -> Result<bool, Error> {
        let mut path = normalize_path(pathname);
        // git matches a symlink by its own path, so only the search directory is resolved
        if path.starts_with(&self.search_dir) {
            path = concat(&[&self.real_search_dir, &path[self.search_dir.len()..]]);
        }
        Ok(!self.is_ignored(&path)?)
    }

    fn is_ignored(&mut self, path: &[u8]) -> Result<bool, Error> {
        if let Some(&ignored) = self.ignored_paths_cache.get(path) {
            return Ok(ignored);
        }
        let mut ignored = false;
        let parent_directories = self.parent_directories_downwards(path);
        if let Some(parent) = parent_directories.last()
            && *parent != self.base_dir
        {
            // paths inside an ignored directory are ignored and cannot be re-included
            ignored = self.is_ignored(&parent.clone())?;
        }
        if !ignored {
            let is_dir = !fs::is_link(path) && fs::is_dir(path);
            let mut gitignore_files: Vec<(Vec<u8>, Vec<u8>)> = Vec::new();
            let mut put = |file: Vec<u8>, dir: Vec<u8>| {
                // a PHP array: a repeated key keeps its first place
                match gitignore_files.iter_mut().find(|(f, _)| *f == file) {
                    Some(entry) => entry.1 = dir,
                    None => gitignore_files.push((file, dir)),
                }
            };
            for parent in parent_directories.iter().rev() {
                put(concat(&[parent, b"/.gitignore"]), parent.clone());
            }
            if let Some(exclude) = &self.git_info_exclude_file
                && parent_directories.first() == Some(&self.base_dir)
            {
                put(exclude.clone(), self.base_dir.clone());
            }
            // the last matching rule of the deepest .gitignore file decides, and the exclude file of the repository comes after the root one
            'files: for (file, parent) in gitignore_files {
                let Some(rules) = self.read_gitignore_file(&file)? else {
                    continue;
                };
                let relative = path.get(parent.len() + 1..).unwrap_or_default().to_vec();
                for (regex, is_negated, is_dir_only) in rules {
                    if is_dir_only && !is_dir {
                        continue;
                    }
                    if let Some(groups) = regex.captures_from(&relative, 0).map_err(internal)?
                        && let Some((s, e)) = groups[0]
                        && relative[s..e] == relative[..]
                    {
                        ignored = !is_negated;
                        break 'files;
                    }
                }
            }
        }
        self.ignored_paths_cache.insert(path.to_vec(), ignored);
        Ok(ignored)
    }

    fn parent_directories_up_to(&self, from: &[u8]) -> Vec<Vec<u8>> {
        parent_directories_upwards(from)
            .into_iter()
            .filter(|d| d.starts_with(&self.base_dir))
            .collect()
    }

    fn parent_directories_downwards(&self, file_real_path: &[u8]) -> Vec<Vec<u8>> {
        let mut dirs = self.parent_directories_up_to(file_real_path);
        dirs.reverse();
        dirs
    }

    fn get_git_info_exclude_path(&self) -> Result<Option<Vec<u8>>, Error> {
        let mut git_dir = concat(&[&self.base_dir, b"/.git"]);
        // worktrees and submodules have a .git file that points to their git directory, which can be outside of open_basedir
        if !fs::is_dir(&git_dir) {
            let content = fs::read(&git_dir).unwrap_or_default();
            let Some(groups) = pattern(&GITDIR)?
                .captures_from(&content, 0)
                .map_err(internal)?
            else {
                return Ok(None);
            };
            let Some((s, e)) = groups.get(1).copied().flatten() else {
                return Ok(None);
            };
            git_dir = self.resolve_git_path(trim(&content[s..e]), &self.base_dir)?;
            // the git directory of a worktree points to the common directory, where the exclude file is
            if let Ok(common_dir) = fs::read(&concat(&[&git_dir, b"/commondir"])) {
                git_dir = self.resolve_git_path(trim(&common_dir), &git_dir)?;
            }
        }
        let exclude = concat(&[&git_dir, b"/info/exclude"]);
        Ok(fs::is_file(&exclude).then_some(exclude))
    }

    fn resolve_git_path(&self, path: &[u8], from: &[u8]) -> Result<Vec<u8>, Error> {
        Ok(if pattern(&ABSOLUTE)?.is_match(path).map_err(internal)? {
            path.to_vec()
        } else {
            concat(&[from, b"/", path])
        })
    }

    /// The rules of a .gitignore file, last one first.
    fn read_gitignore_file(&mut self, path: &[u8]) -> Result<Option<Vec<Rule>>, Error> {
        if let Some(cached) = self.gitignore_files_cache.get(path) {
            return Ok(cached.clone());
        }
        if !fs::file_exists(path) {
            self.gitignore_files_cache.insert(path.to_vec(), None);
            return Ok(None);
        }
        if !fs::is_file(path) || !fs::is_readable(path) {
            return Err(Error::Runtime(format!(
                "The \"ignoreVCSIgnored\" option cannot be used by the Finder as the \"{}\" file is not readable.",
                String::from_utf8_lossy(path)
            )));
        }
        let content = fs::read(path).map_err(Error::Runtime)?;
        let mut rules = Vec::new();
        for line in preg_split(&patterns()?.newline, &content, false)? {
            // only a line starting with "#" is a comment, and only trailing spaces are stripped
            if line.starts_with(b"#") {
                continue;
            }
            let mut line = pattern(&TRAILING_SPACES)?
                .replace(&line, b"")
                .map_err(internal)?;
            let is_negated = line.starts_with(b"!");
            if is_negated {
                line.remove(0);
            }
            let is_dir_only = line.ends_with(b"/");
            if is_dir_only {
                line.pop();
            }
            if line.is_empty() {
                continue;
            }
            if line.starts_with(b"#") {
                // the leading "!" is already stripped, so a "#" here starts a pattern, not a comment
                line.insert(0, b'\\');
            }
            rules.push((
                Arc::new(Pcre::new(&Gitignore::to_regex(&line)?)?),
                is_negated,
                is_dir_only,
            ));
        }
        rules.reverse();
        self.gitignore_files_cache
            .insert(path.to_vec(), Some(rules.clone()));
        Ok(Some(rules))
    }
}

/// `trim()`'s default characters.
fn trim(s: &[u8]) -> &[u8] {
    let is = |c: &u8| matches!(c, b' ' | b'\t' | b'\n' | b'\r' | 0 | 0x0b);
    let start = s.iter().position(|c| !is(c)).unwrap_or(s.len());
    let end = s.iter().rposition(|c| !is(c)).map_or(start, |e| e + 1);
    &s[start..end.max(start)]
}

fn parent_directories_upwards(from: &[u8]) -> Vec<Vec<u8>> {
    let mut parents = Vec::new();
    let mut parent = from.to_vec();
    loop {
        let next = fs::dirname(&parent);
        // dirname('/') = '/'
        if next == parent {
            break;
        }
        parents.push(next.clone());
        parent = next;
    }
    parents
}

/// Linux behaviour is canonical: paths already use `/`.
fn normalize_path(path: &[u8]) -> Vec<u8> {
    path.to_vec()
}

impl<I: Iterator<Item = Result<F, Error>>, F: FileInfo> Iterator for VcsIgnoredFilterIterator<I> {
    type Item = Result<F, Error>;

    fn next(&mut self) -> Option<Self::Item> {
        loop {
            match self.iterator.next()? {
                Ok(f) => match self.accept_path(f.get_pathname()) {
                    Ok(true) => return Some(Ok(f)),
                    Ok(false) => {}
                    Err(e) => return Some(Err(e)),
                },
                Err(e) => return Some(Err(e)),
            }
        }
    }
}
