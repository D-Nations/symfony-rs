//! Compiles templates/**/*.twig and tests/snippets/*.twig to Rust in
//! OUT_DIR/templates.rs.
//!
//! Upstream: none

use std::{env, fs, path::Path};

use twig_trial_compiler::{Callable, Environment, compile};

fn collect(dir: &Path, root: &Path, out: &mut Vec<(String, String)>) -> std::io::Result<()> {
    let mut entries: Vec<_> = fs::read_dir(dir)?.collect::<Result<_, _>>()?;
    entries.sort_by_key(|e| e.path());
    for entry in entries {
        let path = entry.path();
        if path.is_dir() {
            collect(&path, root, out)?;
        } else if path.extension().is_some_and(|e| e == "twig") {
            let name = path
                .strip_prefix(root)
                .unwrap_or(&path)
                .to_string_lossy()
                .replace('\\', "/");
            out.push((name, fs::read_to_string(&path)?));
        }
    }
    Ok(())
}

fn main() {
    let root = Path::new("templates");
    println!("cargo::rerun-if-changed=templates");
    println!("cargo::rerun-if-changed=tests/snippets");
    let mut templates = Vec::new();
    // The application's templates, and the language snippets the tests
    // compare with PHP (found as "snippets/<file>", as the recorder's
    // second loader path gives them).
    for (dir, base) in [
        (root, root),
        (Path::new("tests/snippets"), Path::new("tests")),
    ] {
        if let Err(e) = collect(dir, base, &mut templates) {
            panic!("reading {}: {e}", dir.display());
        }
    }
    let env = Environment {
        functions: vec![
            Callable::new("path", "crate::ext::path", false),
            Callable::new("component", "crate::ext::component", true),
        ],
        filters: vec![Callable::new(
            "format_datetime",
            "crate::ext::format_datetime",
            false,
        )],
        component_pre_lexer: true,
    };
    let code = match compile(&env, &templates) {
        Ok(code) => code,
        // A located refusal fails the build, as AGENTS.md requires.
        Err(e) => panic!("{e}"),
    };
    let out = Path::new(&env::var_os("OUT_DIR").unwrap_or_default()).join("templates.rs");
    if let Err(e) = fs::write(&out, code) {
        panic!("writing {}: {e}", out.display());
    }
}
