#!/usr/bin/env python3
"""Records what PHP makes of query strings and Cookie headers, as the test
vectors of symfony-http-foundation's query and cookie parsers:
crates/http-foundation/tests/data/query_vectors.json,
cookie_vectors.json and content_type_vectors.json.

    scripts/record.sh request-vars

Runs request_vars.php in the oracle image. Array keys are JSON integers
for PHP int keys and base64 for string keys; strings are base64.
Standard library only; the PHP side runs in the container.
"""

from __future__ import annotations

import base64
import json
import subprocess
import sys
from pathlib import Path

sys.path.insert(0, str(Path(__file__).parent))
from request_vars_cases import content_type_cases, cookie_cases, query_cases  # noqa: E402

ROOT = Path(__file__).resolve().parents[2]
DATA = ROOT / "crates" / "http-foundation" / "tests" / "data"


def valid_in_request_line(raw: bytes) -> bool:
    return all(0x21 <= byte <= 0x7E and byte != ord("#") for byte in raw)


def main() -> int:
    queries = query_cases()
    cookies = cookie_cases()
    types = content_type_cases()
    cases = [
        {"kind": "query", "raw": base64.b64encode(raw).decode(), "get": valid_in_request_line(raw)}
        for _, raw in queries
    ] + [{"kind": "cookie", "raw": base64.b64encode(raw).decode(), "get": False} for _, raw in cookies] + [
        {"kind": "type", "raw": base64.b64encode(raw).decode(), "get": False} for _, raw in types
    ]
    result = subprocess.run(
        ["docker", "compose", "-f", str(ROOT / "oracle" / "compose.yaml"), "run", "--rm", "-T",
         "php", "oracle/recorders/request_vars.php"],
        input=json.dumps(cases).encode(), capture_output=True, check=False,
    )
    if result.returncode != 0:
        sys.exit(f"oracle failed: {result.stderr.decode(errors='replace')}")
    answers = json.loads(result.stdout)
    if len(answers) != len(cases):
        sys.exit(f"expected {len(cases)} answers, got {len(answers)}")
    query_vectors = []
    for (name, raw), answer in zip(queries, answers[: len(queries)]):
        query_vectors.append({
            "name": name, "raw": base64.b64encode(raw).decode(),
            "get": answer["get"], "post": answer["post"],
        })
    cookie_vectors = [
        {"name": name, "raw": base64.b64encode(raw).decode(), "cookie": answer["cookie"]}
        for (name, raw), answer in zip(cookies, answers[len(queries):len(queries) + len(cookies)])
    ]
    type_vectors = [
        {"name": name, "content_type": raw.decode("latin-1"), "post": answer["post"]}
        for (name, raw), answer in zip(types, answers[len(queries) + len(cookies):])
    ]
    DATA.mkdir(parents=True, exist_ok=True)
    for file, vectors in (
        ("query_vectors.json", query_vectors),
        ("cookie_vectors.json", cookie_vectors),
        ("content_type_vectors.json", type_vectors),
    ):
        # LF on every platform: the files are compared byte for byte.
        with open(DATA / file, "w", encoding="utf-8", newline="\n") as out:
            json.dump(vectors, out, indent=1)
            out.write("\n")
    print(f"{len(query_vectors)} query, {len(cookie_vectors)} cookie and {len(type_vectors)} content type vectors")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
