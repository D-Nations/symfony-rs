#!/usr/bin/env python3
"""Records what PHP itself makes of multipart/form-data bodies, as the test
vectors of symfony-http-foundation's multipart parser:
crates/http-foundation/tests/data/multipart_vectors.json.

    scripts/record.sh multipart

Each body of multipart_cases.py is sent as POST and as PUT to PHP's
built-in server in the oracle image (multipart.php). Arrays are written as
ordered lists of [key, value] pairs and strings in base64, so order and
bytes are kept. Standard library only; the PHP side runs in the container.
Moved from symfony-demo-rs (compare/multipart_vectors.py); same output.
"""

from __future__ import annotations

import base64
import json
import subprocess
import sys
from pathlib import Path

sys.path.insert(0, str(Path(__file__).parent))
from multipart_cases import all_cases  # noqa: E402

ROOT = Path(__file__).resolve().parents[2]
OUT = ROOT / "crates" / "http-foundation" / "tests" / "data" / "multipart_vectors.json"


def main() -> int:
    cases = all_cases()
    payload = json.dumps([
        {"name": name, "content_type": content_type, "body": base64.b64encode(body).decode()}
        for name, content_type, body in cases
    ]).encode()
    result = subprocess.run(
        ["docker", "compose", "-f", str(ROOT / "oracle" / "compose.yaml"), "run", "--rm", "-T",
         "php", "oracle/recorders/multipart.php"],
        input=payload, capture_output=True, check=False,
    )
    if result.returncode != 0:
        sys.exit(f"oracle failed: {result.stderr.decode(errors='replace')}")
    answers = json.loads(result.stdout)
    if len(answers) != 2 * len(cases):
        sys.exit(f"expected {2 * len(cases)} answers, got {len(answers)}")
    vectors = []
    answer = iter(answers)
    for name, content_type, body in cases:
        for method in ("POST", "PUT"):
            parsed = next(answer)
            vectors.append({
                "name": name,
                "method": method,
                "content_type": content_type,
                "body": base64.b64encode(body).decode(),
                "fields": parsed["fields"],
                "failed": parsed["failed"],
            })
    OUT.parent.mkdir(parents=True, exist_ok=True)
    # LF on every platform: the file is compared byte for byte.
    with OUT.open("w", encoding="utf-8", newline="\n") as out:
        out.write(json.dumps(vectors, indent=0) + "\n")
    print(f"{len(vectors)} vectors written to {OUT.relative_to(ROOT)}")
    return 0


if __name__ == "__main__":
    sys.exit(main())
