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
import subprocess
import sys

ROOT = Path(__file__).resolve().parents[1]


def capture(*args: str) -> bytes:
    return subprocess.check_output(args, cwd=ROOT)


def run(*args: str) -> None:
    print("+", shlex.join(args), flush=True)
    subprocess.run(args, cwd=ROOT, check=True)


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
    parser.add_argument("mode", choices=["list", "check", "test", "full", "audit"])
    parser.add_argument("--base", default="origin/main", help="Git comparison ref")
    parser.add_argument("--workspace", action="store_true", help="force all packages")
    parser.add_argument("-p", "--package", action="append", default=[], help="explicit package (development only)")
    parser.add_argument("--nextest", action="store_true", help="use installed nextest for development tests, then Cargo doctests")
    parser.add_argument("--timings", action="store_true", help="write Cargo HTML compilation timings")
    options = parser.parse_args()
    if options.package and (options.workspace or options.mode in {"full", "audit"}):
        parser.error("explicit packages are only for scoped development checks")
    if options.nextest and options.mode != "test":
        parser.error("--nextest is only for development test mode")
    if options.timings and options.mode not in {"check", "test", "audit"}:
        parser.error("--timings is only for check, test or audit")
    if options.nextest and options.timings:
        parser.error("use Cargo test mode for compilation timings")

    if options.mode in {"full", "audit"} or options.workspace:
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
            run("cargo", "nextest", "run", "--locked", *scope, "--no-fail-fast")
            run("cargo", "test", "--locked", *scope, "--doc")
        else:
            run("cargo", "test", "--locked", *scope, "--no-fail-fast", *timing)
    elif options.mode == "full":
        run("cargo", "fmt", "--all", "--check")
        run("cargo", "clippy", "--locked", "--workspace", "--all-targets", "--", "-D", "warnings")
        run("cargo", "test", "--locked", "--workspace", "--no-fail-fast")
        run("bash", "scripts/sources.sh", "--check")
        run("bash", "scripts/parity-check.sh")
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
