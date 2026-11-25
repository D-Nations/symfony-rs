#!/usr/bin/env python3
"""Local development checks. Standard library only; Windows and Linux.

Affected mode is deliberately conservative and is not a merge gate.
Run from any directory. Git changes include committed, staged, unstaged
and untracked (non-ignored) files. Cargo metadata supplies all workspace
path dependencies, including optional, build, dev and target-specific ones.
"""
from __future__ import annotations

import argparse
import json
import os
from pathlib import Path
import shlex
import shutil
import subprocess
import sys
import time

ROOT = Path(__file__).resolve().parents[1]


def capture(*args: str) -> bytes:
    return subprocess.check_output(args, cwd=ROOT)


def run(*args: str) -> None:
    print("+", shlex.join(args), flush=True)
    started = time.perf_counter()
    try:
        subprocess.run(
            args, cwd=ROOT, check=True,
            env={**os.environ, "SYMFONY_RS_PYTHON": sys.executable},
        )
    finally:
        print(f"  elapsed: {time.perf_counter() - started:.2f}s", flush=True)


def run_checks(commands: list[tuple[str, ...]]) -> int:
    """Report every failure, including doctests after a nextest failure."""
    failed = []
    for command in commands:
        try:
            run(*command)
        except (subprocess.CalledProcessError, OSError) as error:
            failed.append(shlex.join(command))
            print(error, file=sys.stderr)
    if failed:
        print("Failed checks: " + "; ".join(failed), file=sys.stderr)
    return int(bool(failed))


def bash_executable(requested: str | None) -> str:
    if requested:
        return requested
    # Windows' system32/bash.exe launches WSL, not the host Git checkout.
    # Prefer Bash from the same Git installation used for change detection.
    if sys.platform == "win32":
        git = shutil.which("git")
        if git:
            install = Path(git).resolve().parent.parent
            for candidate in (install / "bin/bash.exe", install / "usr/bin/bash.exe"):
                if candidate.is_file():
                    return str(candidate)
    return shutil.which("bash") or "bash"


def changed_paths(base: str) -> set[str]:
    # Resolve the ref before using it in another command.
    ancestor = capture("git", "merge-base", "HEAD", base).decode().strip()
    if not ancestor:
        raise ValueError("no merge base")
    commands = [
        ("git", "diff", "--name-only", "--no-renames", "-z", ancestor, "HEAD", "--"),
        ("git", "diff", "--cached", "--name-only", "--no-renames", "-z", "--"),
        ("git", "diff", "--name-only", "--no-renames", "-z", "--"),
        ("git", "ls-files", "--others", "--exclude-standard", "-z", "--"),
    ]
    return {
        os.fsdecode(path)
        for command in commands
        for path in capture(*command).split(b"\0")
        if path
    }


def affected(metadata: dict, changes: set[str], root: Path) -> tuple[list[str], str]:
    """Return selected names; empty selection means full workspace."""
    members = set(metadata["workspace_members"])
    packages = [p for p in metadata["packages"] if p["id"] in members]
    directories = {
        Path(p["manifest_path"]).resolve().parent: p["name"] for p in packages
    }
    reverse: dict[str, set[str]] = {name: set() for name in directories.values()}
    if len(reverse) != len(packages):
        return [], "ambiguous workspace package names"
    for package in packages:
        for dependency in package["dependencies"]:
            if dependency.get("path"):
                provider = directories.get(Path(dependency["path"]).resolve())
                if provider is None:
                    return [], "path dependency outside workspace"
                if provider != package["name"]:
                    reverse[provider].add(package["name"])
    selected: set[str] = set()
    for change in changes:
        # Root files, manifests, scripts, oracle, parity, deleted/moved
        # packages, and anything unknown trigger a full check.
        path = root / change
        if path.name == "Cargo.toml" or path.name == "Cargo.lock":
            return [], f"manifest or lock change: {change}"
        owners = [
            (directory, name)
            for directory, name in directories.items()
            if path.is_relative_to(directory)
        ]
        if not owners:
            return [], f"workspace-level or unknown change: {change}"
        directory, name = max(owners, key=lambda item: len(item[0].parts))
        # Only ordinary Rust sources/tests are safe to scope. Build scripts,
        # generated data and arbitrary include paths may affect other crates.
        relative = path.relative_to(directory)
        if not relative.parts or relative.parts[0] not in {"src", "tests", "benches", "examples"}:
            return [], f"non-source change: {change}"
        if path.suffix != ".rs":
            return [], f"non-Rust input: {change}"
        selected.add(name)
    if not selected:
        return [], "no changes detected; default to full workspace"
    pending = list(selected)
    while pending:
        for consumer in reverse[pending.pop()] - selected:
            selected.add(consumer)
            pending.append(consumer)
    return sorted(selected), "changed crates and transitive reverse dependencies"


def selection(base: str) -> tuple[list[str], str]:
    try:
        metadata = json.loads(capture("cargo", "metadata", "--locked", "--no-deps", "--format-version", "1"))
        return affected(metadata, changed_paths(base), ROOT)
    except (subprocess.CalledProcessError, OSError, ValueError, KeyError) as error:
        return [], f"selection failed ({error}); checking full workspace"


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("mode", choices=["list", "check", "test", "full", "parity", "audit"])
    parser.add_argument("--bash", help="Bash executable for source/parity checks (auto-detect Git Bash on Windows)")
    parser.add_argument("--base", default="origin/main", help="Git comparison ref")
    parser.add_argument("--workspace", action="store_true", help="force all packages")
    parser.add_argument("-p", "--package", action="append", default=[], help="explicit package (development only)")
    parser.add_argument("--nextest", action="store_true", help="use installed nextest for development tests, then Cargo doctests")
    parser.add_argument("--timings", action="store_true", help="write Cargo HTML compilation timings")
    options = parser.parse_args()
    if options.package and (options.workspace or options.mode in {"full", "parity", "audit"}):
        parser.error("explicit packages are only for scoped development checks")
    if options.nextest and options.mode != "test":
        parser.error("--nextest is only for development test mode")
    if options.timings and options.mode not in {"check", "test", "audit"}:
        parser.error("--timings is only for check, test or audit")
    if options.nextest and options.timings:
        parser.error("use Cargo test mode for compilation timings")

    if options.mode in {"full", "parity", "audit"} or options.workspace:
        packages, reason = [], "explicit full workspace"
    elif options.package:
        packages, reason = options.package, "explicit packages (no reverse closure)"
    else:
        packages, reason = selection(options.base)
    scope = [item for package in packages for item in ("-p", package)] if packages else ["--workspace"]
    print(f"Scope: {', '.join(packages) or 'whole workspace'} ({reason})", flush=True)
    timing = ["--timings"] if options.timings else []
    if options.mode == "list":
        return 0
    if options.mode == "check":
        run("cargo", "check", "--locked", "--all-targets", *scope, *timing)
    elif options.mode == "test":
        if options.nextest:
            return run_checks([
                ("cargo", "nextest", "run", "--locked", *scope, "--no-fail-fast"),
                ("cargo", "test", "--locked", *scope, "--doc"),
            ])
        else:
            run("cargo", "test", "--locked", *scope, "--no-fail-fast", *timing)
    elif options.mode in {"full", "parity"}:
        bash = bash_executable(options.bash)
        commands = []
        if options.mode == "full":
            commands.extend([
                (sys.executable, "-m", "unittest", "discover", "-s", "scripts", "-p", "test_*.py"),
                ("cargo", "fmt", "--all", "--check"),
                ("cargo", "clippy", "--locked", "--workspace", "--all-targets", "--", "-D", "warnings"),
                ("cargo", "test", "--locked", "--workspace", "--no-fail-fast"),
            ])
        commands.extend([
            (bash, "scripts/sources.sh", "--check"),
            (bash, "scripts/parity-check.sh"),
            (sys.executable, "scripts/classes.py", "check"),
        ])
        return run_checks(commands)
    else:
        run("cargo", "tree", "--locked", "--workspace", "--duplicates")
        run("cargo", "tree", "--locked", "--workspace", "-e", "features")
        if options.timings:
            run("cargo", "test", "--locked", "--workspace", "--no-run", "--timings")
    return 0


if __name__ == "__main__":
    try:
        sys.exit(main())
    except subprocess.CalledProcessError as error:
        sys.exit(error.returncode or 1)
    except OSError as error:
        sys.exit(str(error))
