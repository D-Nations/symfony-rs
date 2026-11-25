//! Compiles symfony/demo's container (data/container.xml, recorded from
//! PHP by `scripts/record.sh di-trial`) to Rust in `OUT_DIR/container.rs`,
//! and writes what the compiler found to `OUT_DIR/report.txt`.
//!
//! Upstream: none

use std::{env, fmt::Write as _, fs, path::Path, process::ExitCode};

use di_trial_compiler::{Config, compile};

fn main() -> ExitCode {
    let input = "data/container.xml";
    println!("cargo::rerun-if-changed={input}");
    let xml = match fs::read(input) {
        Ok(xml) => xml,
        Err(e) => {
            println!("cargo::error={input}: {e}");
            return ExitCode::FAILURE;
        }
    };
    let config = Config {
        // Symfony resets these between requests in long-running workers:
        // they keep state from one use to the next.
        request_scoped_tags: vec!["kernel.reset".into()],
        request_scoped_ids: Vec::new(),
    };
    let output = match compile(&xml, &config) {
        Ok(output) => output,
        Err(e) => {
            println!("cargo::error={input}: {e}");
            return ExitCode::FAILURE;
        }
    };
    let Some(out_dir) = env::var_os("OUT_DIR") else {
        println!("cargo::error=OUT_DIR is not set");
        return ExitCode::FAILURE;
    };
    let out = Path::new(&out_dir);
    let r = &output.report;
    let mut report = String::new();
    let _ = writeln!(report, "services: {}", r.services);
    let _ = writeln!(report, "inline definitions: {}", r.inline);
    let _ = writeln!(report, "aliases: {}", r.aliases);
    let _ = writeln!(report, "classes: {}", r.classes);
    let _ = writeln!(report, "request-scoped seeds: {}", r.request_seeds);
    let _ = writeln!(report, "application-scoped: {}", r.app_scoped);
    let _ = writeln!(report, "request-scoped: {}", r.request_scoped);
    for c in &r.call_cycles {
        let _ = writeln!(report, "cycle through calls: {}", c.join(" -> "));
    }
    for e in &r.env {
        let _ = writeln!(report, "env: {e}");
    }
    for m in &r.missing_optional {
        let _ = writeln!(report, "missing, optional: {m}");
    }
    if let Err(e) = fs::write(out.join("container.rs"), &output.code)
        .and_then(|()| fs::write(out.join("report.txt"), report))
    {
        println!("cargo::error=cannot write the container: {e}");
        return ExitCode::FAILURE;
    }
    ExitCode::SUCCESS
}
