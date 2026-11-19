#!/usr/bin/env python3
"""Report upstream changes that may concern the ported code.

    python scripts/upstream_watch.py                 print the report
    python scripts/upstream_watch.py --issue         also open or update the
                                                     "Upstream watch" issue

For every package in parity/packages.tsv:

- **Advisories.** GitHub's advisory database (ecosystem composer) is asked
  for the package; an advisory is reported when the pinned version lies in
  one of its vulnerable ranges. For packages that Rust code ports
  (parity/sources.tsv), *every* advisory published in the last 400 days is
  listed as well, with whether the pin is affected: a fix released before
  the pin may still describe behaviour the Rust code reproduces.
- **Newer patch releases.** The newest tag of the same minor line is
  compared with the pin. For ported packages, the mapped upstream files
  that changed between the pin and that tag are listed (the checkout in
  upstream/<package> is deepened to fetch the tag).

Needs git, and a GitHub token in GH_TOKEN or GITHUB_TOKEN (or `gh auth
token`) for the advisory API. Standard library only.
"""

from __future__ import annotations

import datetime as dt
import json
import os
import re
import subprocess
import sys
import urllib.parse
import urllib.request
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
ISSUE_TITLE = "Upstream watch"
RECENT_DAYS = 400


def read_tsv(path: Path) -> list[list[str]]:
    rows = []
    for line in path.read_text(encoding="utf-8").splitlines():
        if line and not line.startswith("#"):
            rows.append(line.split("\t"))
    return rows


def version_tuple(version: str) -> tuple[int, ...] | None:
    match = re.fullmatch(r"v?(\d+)\.(\d+)(?:\.(\d+))?(?:\.(\d+))?", version.strip())
    if not match:
        return None
    return tuple(int(part or 0) for part in match.groups())


def in_range(version: tuple[int, ...], spec: str) -> bool:
    """GitHub's vulnerable_version_range: comma-separated comparators."""
    for clause in spec.split(","):
        clause = clause.strip()
        match = re.fullmatch(r"(>=|<=|>|<|=)?\s*(\S+)", clause)
        if not match:
            return False
        op, bound_text = match.group(1) or "=", match.group(2)
        bound = version_tuple(bound_text)
        if bound is None:
            return False
        ok = {
            ">=": version >= bound, "<=": version <= bound, ">": version > bound,
            "<": version < bound, "=": version == bound,
        }[op]
        if not ok:
            return False
    return True


def token() -> str | None:
    for name in ("GH_TOKEN", "GITHUB_TOKEN"):
        if os.environ.get(name):
            return os.environ[name]
    try:
        out = subprocess.run(["gh", "auth", "token"], capture_output=True, text=True, check=True)
        return out.stdout.strip() or None
    except (OSError, subprocess.CalledProcessError):
        return None


def advisories(package: str, auth: str | None) -> list[dict]:
    query = urllib.parse.urlencode({"ecosystem": "composer", "affects": package, "per_page": 100})
    request = urllib.request.Request(f"https://api.github.com/advisories?{query}")
    request.add_header("Accept", "application/vnd.github+json")
    request.add_header("User-Agent", "symfony-rs-upstream-watch")
    if auth:
        request.add_header("Authorization", f"Bearer {auth}")
    with urllib.request.urlopen(request, timeout=30) as response:
        return json.load(response)


def tags(repository: str) -> list[str]:
    out = subprocess.run(["git", "ls-remote", "--tags", "--refs", repository],
                         capture_output=True, text=True, check=True).stdout
    return [line.split("refs/tags/", 1)[1] for line in out.splitlines() if "refs/tags/" in line]


def newest_patch(pinned: str, available: list[str]) -> str | None:
    base = version_tuple(pinned)
    if base is None:
        return None
    best, best_tag = base, None
    for tag in available:
        v = version_tuple(tag)
        if v and v[:2] == base[:2] and v > best:
            best, best_tag = v, tag
    return best_tag


def changed_files(package: str, repository: str, reference: str, tag: str,
                  paths: list[str]) -> list[str] | None:
    checkout = ROOT / "upstream" / package
    if not (checkout / ".git").is_dir():
        return None
    git = ["git", "-c", "core.autocrlf=false", "-C", str(checkout)]
    subprocess.run(git + ["fetch", "--quiet", "--depth", "1", repository,
                          f"refs/tags/{tag}:refs/tags/{tag}"],
                   capture_output=True, check=False)
    out = subprocess.run(git + ["diff", "--name-only", reference, f"refs/tags/{tag}", "--", *paths],
                         capture_output=True, text=True, check=False)
    if out.returncode != 0:
        return None
    return [line for line in out.stdout.splitlines() if line]


def main() -> int:
    packages = read_tsv(ROOT / "parity" / "packages.tsv")
    sources = read_tsv(ROOT / "parity" / "sources.tsv")
    mapped: dict[str, set[str]] = {}
    rust_files: dict[tuple[str, str], list[str]] = {}
    for rust, package, path, _ref in sources:
        mapped.setdefault(package, set()).add(path)
        rust_files.setdefault((package, path), []).append(rust)
    triaged = {row[0]: row for row in read_tsv(ROOT / "parity" / "advisories.tsv")}
    auth = token()
    today = dt.datetime.now(dt.timezone.utc)

    affected, recent, done, releases, errors = [], [], [], [], []
    for package, version, reference, repository in packages:
        pin = version_tuple(version)
        try:
            found = advisories(package, auth)
        except Exception as error:  # report and carry on with the others
            errors.append(f"{package}: advisory query failed ({error})")
            found = []
        for advisory in found:
            ranges = [v.get("vulnerable_version_range") or "" for v in advisory.get("vulnerabilities", [])
                      if (v.get("package") or {}).get("name") == package]
            hit = pin is not None and any(in_range(pin, r) for r in ranges if r)
            line = (f"[{advisory['ghsa_id']}]({advisory['html_url']}) {advisory.get('cve_id') or ''} "
                    f"{advisory['summary']} (`{' / '.join(ranges)}`)")
            if hit:
                affected.append(f"- **{package} {version} is affected:** {line}")
            elif package in mapped:
                published = dt.datetime.fromisoformat(advisory["published_at"].replace("Z", "+00:00"))
                if advisory["ghsa_id"] in triaged:
                    done.append(f"- {package}: {line}: **{triaged[advisory['ghsa_id']][2]}**")
                elif (today - published).days <= RECENT_DAYS:
                    recent.append(f"- {package} (ported; pin {version} not in range): {line}")
        if package in mapped:
            try:
                newer = newest_patch(version, tags(repository))
            except subprocess.CalledProcessError as error:
                errors.append(f"{package}: tag listing failed ({error})")
                continue
            if newer:
                files = changed_files(package, repository, reference, newer, sorted(mapped[package]))
                if files is None:
                    detail = "mapped files not compared (run scripts/fetch-upstream.sh first)"
                elif files:
                    detail = "changed mapped files: " + ", ".join(
                        f"`{f}` (ported in {', '.join(rust_files[(package, f)])})" for f in files)
                else:
                    detail = "no mapped file changed"
                releases.append(f"- {package}: pinned {version}, newest {newer}; {detail}")

    report = ["# Upstream watch", "",
              f"Run {today:%Y-%m-%d}. Pins: `parity/packages.tsv`; ported files: `parity/sources.tsv`.", ""]
    report += ["## Advisories affecting a pinned version", ""] + (affected or ["None."]) + [""]
    report += [f"## Recent advisories for ported packages (last {RECENT_DAYS} days)", "",
               "Fixed before the pin, but the Rust port may reproduce the old behaviour: "
               "each needs a triage row in `parity/advisories.tsv`.", ""] + (recent or ["None."]) + [""]
    report += ["## Triaged advisories", ""] + (done or ["None."]) + [""]
    report += ["## Newer patch releases of ported packages", ""] + (releases or ["None."]) + [""]
    if errors:
        report += ["## Errors", ""] + [f"- {e}" for e in errors] + [""]
    text = "\n".join(report)
    print(text)

    if "--issue" in sys.argv[1:]:
        number = subprocess.run(["gh", "issue", "list", "--state", "open", "--search",
                                 f'"{ISSUE_TITLE}" in:title', "--json", "number", "--jq", ".[0].number"],
                                capture_output=True, text=True, check=True).stdout.strip()
        if number:
            subprocess.run(["gh", "issue", "edit", number, "--body", text], check=True)
        else:
            subprocess.run(["gh", "issue", "create", "--title", ISSUE_TITLE, "--body", text], check=True)
    return 1 if errors else 0


if __name__ == "__main__":
    sys.exit(main())
