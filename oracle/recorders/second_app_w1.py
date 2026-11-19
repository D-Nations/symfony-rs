#!/usr/bin/env python3
"""Records W1, the first workflow of the M1-D second application
(docs/acceptance/second-app.md), from the PHP original:
oracle/apps/easyadmin-demo/w1.json.

    scripts/record.sh second-app-w1

W1 is `GET /` with one Accept-Language value, then a GET of the
`Location` it redirects to, with the cookies the first response set, as a
browser does. Redirects are recorded, not followed further. It runs once
on the database the pin ships (`pinned`) and once on an empty schema
(`empty`), each on a fresh copy of the app's state.

The app (built by scripts/second-app.sh) is served with APP_ENV=prod by
PHP's built-in server in the oracle image, on 127.0.0.1 only. Every
response header is kept, in order; `Date` and the cookies' values and
expiry dates are replaced by placeholders, so the file is stable and holds
no session identifier. Standard library only.
"""

from __future__ import annotations

import http.client
import json
import os
import re
import subprocess
import sys
import time
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]
APP = ROOT / "oracle" / "out" / "apps" / "easyadmin-demo"
OUT = ROOT / "oracle" / "apps" / "easyadmin-demo" / "w1.json"
IMAGE = "symfony-rs-oracle:8.4.27"
CONTAINER = "symfony-rs-second-app"
PORT = 8020
# Recording only. The pin's .env leaves APP_SECRET empty.
SECRET = "symfony-rs-oracle-recording"
# scripts/record.sh passes the bash it runs in; plain "bash" may be another.
BASH = os.environ.get("SECOND_APP_BASH", "bash")

ACCEPT_LANGUAGES = [
    None,
    "en",
    "fr",
    "de-AT",
    "es-ES,es;q=0.9,en;q=0.8",
    "fr;q=0.5, de;q=0.8",
    "ja",
    "*",
    "EN",
    "pt-BR,de;q=0.1",
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
        "-e", "APP_ENV=prod", "-e", "APP_DEBUG=0", "-e", f"APP_SECRET={SECRET}",
        "-v", f"{host_path(APP)}:/app", "-w", "/app", "-p", f"127.0.0.1:{PORT}:8000",
        "--entrypoint", "php", IMAGE,
        "-d", "display_errors=0", "-S", "0.0.0.0:8000", "-t", "public")
    for _ in range(120):
        try:
            connection = http.client.HTTPConnection("127.0.0.1", PORT, timeout=30)
            connection.request("GET", "/favicon.png")
            connection.getresponse().read()
            return
        except OSError:
            time.sleep(0.5)
    sys.exit("the second app did not start")


def stop() -> None:
    run("docker", "rm", "-f", CONTAINER, check=False)


def masked(name: str, value: str) -> str:
    lower = name.lower()
    if lower == "date":
        return "<date>"
    if lower == "set-cookie":
        value = re.sub(r"^([^=]*)=[^;]*", r"\1=<value>", value)
        value = re.sub(r"(?i)(;\s*expires=)[^;]*", r"\1<date>", value)
    return value


def get(path: str, headers: dict[str, str]) -> dict:
    connection = http.client.HTTPConnection("127.0.0.1", PORT, timeout=60)
    connection.request("GET", path, headers=headers)
    response = connection.getresponse()
    body = response.read()
    return {
        "request": {"method": "GET", "path": path, "headers": headers},
        "status": response.status,
        "headers": [[name, masked(name, value)] for name, value in response.getheaders()],
        "body": body.decode("utf-8", errors="replace"),
        "_cookies": [value for name, value in response.getheaders() if name.lower() == "set-cookie"],
        "_location": response.getheader("Location"),
    }


def workflow(accept_language: str | None) -> list[dict]:
    headers = {"Host": "localhost"}
    if accept_language is not None:
        headers["Accept-Language"] = accept_language
    first = get("/", headers)
    steps = [first]
    location = first["_location"]
    if location and location.startswith("/"):
        follow = dict(headers)
        jar = "; ".join(cookie.split(";", 1)[0] for cookie in first["_cookies"])
        if jar:
            follow["Cookie"] = jar
        steps.append(get(location, follow))
    for step in steps:
        cookie = step["request"]["headers"].get("Cookie")
        if cookie:
            step["request"]["headers"]["Cookie"] = re.sub(r"=[^;]*", "=<value>", cookie)
        del step["_cookies"], step["_location"]
    return steps


def main() -> int:
    if not (APP / "vendor").is_dir():
        sys.exit("run scripts/second-app.sh install first")
    pin = run("git", "-C", str(ROOT / "upstream" / "apps" / "easyadmin-demo"),
              "rev-parse", "HEAD").stdout.strip()
    recording = {
        "app": "EasyCorp/easyadmin-demo",
        "commit": pin,
        "patch": "oracle/apps/easyadmin-demo/prod-boot.patch",
        "lock": "the pin's composer.lock (PLAN.md decision 10)",
        "php": IMAGE,
        "env": {"APP_ENV": "prod", "APP_DEBUG": "0", "APP_SECRET": SECRET},
        "server": "PHP built-in server (php -S), docroot public/",
        "kind": "recorded from PHP",
        "databases": {},
    }
    try:
        for database in ("pinned", "empty"):
            stop()
            subprocess.run([BASH, str(ROOT / "scripts" / "second-app.sh"), "database", database],
                           check=True, capture_output=True)
            start()
            recording["databases"][database] = [
                {"accept_language": value, "steps": workflow(value)} for value in ACCEPT_LANGUAGES
            ]
    finally:
        stop()
        subprocess.run([BASH, str(ROOT / "scripts" / "second-app.sh"), "database", "pinned"],
                       check=False, capture_output=True)
    OUT.parent.mkdir(parents=True, exist_ok=True)
    with open(OUT, "w", encoding="utf-8", newline="\n") as out:
        json.dump(recording, out, indent=1, ensure_ascii=False)
        out.write("\n")
    total = sum(len(case["steps"]) for cases in recording["databases"].values() for case in cases)
    print(f"{total} responses written to {OUT.relative_to(ROOT)}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
