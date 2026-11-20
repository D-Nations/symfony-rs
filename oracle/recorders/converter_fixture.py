#!/usr/bin/env python3
"""Records the M1-D converter fixture app (oracle/apps/converter-fixture)
from PHP: oracle/apps/converter-fixture/responses.json.

    scripts/record.sh converter-fixture

The app is served with APP_ENV=prod by PHP's built-in server in the oracle
image, on 127.0.0.1 only, with this repository and symfony/demo (whose
vendor/ holds the pinned packages) mounted read-only. Each case is one
request; the server runs every request with fresh PHP state, as PHP-FPM
does, so repeating a request shows that no service state is shared.
Every response header is kept, in order; `Date` is replaced by a
placeholder. Standard library only.
"""

from __future__ import annotations

import http.client
import json
import subprocess
import sys
import time
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]
DEMO = ROOT.parent / "symfony-demo"
APP = "/work/oracle/apps/converter-fixture"
OUT = ROOT / "oracle" / "apps" / "converter-fixture" / "responses.json"
IMAGE = "symfony-rs-oracle:8.4.27"
CONTAINER = "symfony-rs-converter-fixture"
PORT = 8021

# (name, method, path, extra request headers). Host is always sent.
CASES: list[tuple[str, str, str, dict[str, str]]] = [
    ("greet", "GET", "/greet/world", {}),
    ("greet-again", "GET", "/greet/world", {}),
    ("greet-query", "GET", "/greet/world?punctuation=%3F", {}),
    ("greet-query-empty", "GET", "/greet/world?punctuation=", {}),
    ("greet-quarter", "GET", "/greet/abc", {}),
    ("greet-max-length", "GET", "/greet/abcdefghijkl", {}),
    ("greet-head", "HEAD", "/greet/world", {}),
    ("too-long", "GET", "/greet/abcdefghijklm", {}),
    ("requirement-uppercase", "GET", "/greet/World", {}),
    ("requirement-encoded", "GET", "/greet/%C3%A9t%C3%A9", {}),
    ("no-route-referer", "GET", "/nope", {"Referer": "http://localhost/greet/world"}),
    ("method-not-allowed", "POST", "/greet/world", {}),
    ("query-non-scalar", "GET", "/greet/world?punctuation[]=x", {}),
    ("trailing-slash", "GET", "/greet/world/", {}),
    ("trailing-slash-query", "GET", "/greet/world/?punctuation=%3F", {}),
    ("trailing-slash-port", "GET", "/greet/world/", {"Host": "example.com:8080"}),
]


def host_path(path: Path) -> str:
    try:
        return subprocess.run(["cygpath", "-w", str(path)], capture_output=True,
                              text=True, check=True).stdout.strip()
    except (FileNotFoundError, subprocess.CalledProcessError):
        return str(path)


def run(*args: str, check: bool = True) -> subprocess.CompletedProcess:
    return subprocess.run(args, capture_output=True, text=True, check=check)


def start() -> None:
    run("docker", "rm", "-f", CONTAINER, check=False)
    run("docker", "run", "-d", "--rm", "--name", CONTAINER,
        "-v", f"{host_path(ROOT)}:/work:ro", "-v", f"{host_path(DEMO)}:/demo:ro",
        "-p", f"127.0.0.1:{PORT}:8000", "--entrypoint", "php", IMAGE,
        "-d", "display_errors=0", "-S", "0.0.0.0:8000", "-t", f"{APP}/public")
    for _ in range(120):
        try:
            connection = http.client.HTTPConnection("127.0.0.1", PORT, timeout=30)
            connection.request("GET", "/", headers={"Host": "localhost"})
            connection.getresponse().read()
            return
        except OSError:
            time.sleep(0.5)
    sys.exit("the converter fixture app did not start")


def stop() -> None:
    run("docker", "rm", "-f", CONTAINER, check=False)


def record(name: str, method: str, path: str, extra: dict[str, str]) -> dict:
    headers = {"Host": "localhost", **extra}
    connection = http.client.HTTPConnection("127.0.0.1", PORT, timeout=60)
    connection.request(method, path, headers=headers)
    response = connection.getresponse()
    body = response.read()
    return {
        "name": name,
        "request": {"method": method, "path": path, "headers": headers},
        "status": response.status,
        "headers": [[key, "<date>" if key.lower() == "date" else value]
                    for key, value in response.getheaders()],
        "body": body.decode("utf-8"),
    }


def main() -> int:
    if not (DEMO / "vendor" / "autoload.php").is_file():
        sys.exit("expected symfony/demo with its vendor/ at ../symfony-demo")
    recording = {
        "app": "oracle/apps/converter-fixture",
        "packages": "symfony/demo's vendor/ (parity/packages.tsv)",
        "php": IMAGE,
        "env": {"APP_ENV": "prod", "APP_DEBUG": "0"},
        "server": "PHP built-in server (php -S), docroot public/",
        "kind": "recorded from PHP",
        "server_headers": ["Host", "Connection", "X-Powered-By"],
        "cases": [],
    }
    try:
        start()
        recording["cases"] = [record(*case) for case in CASES]
    finally:
        stop()
    OUT.parent.mkdir(parents=True, exist_ok=True)
    with open(OUT, "w", encoding="utf-8", newline="\n") as out:
        json.dump(recording, out, indent=1, ensure_ascii=False)
        out.write("\n")
    print(f"{len(recording['cases'])} responses written to {OUT.relative_to(ROOT)}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
