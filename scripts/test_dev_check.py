"""Selection unit tests: python scripts/test_dev_check.py."""
import unittest
from pathlib import Path
from tempfile import TemporaryDirectory

from dev_check import affected


class SelectionTests(unittest.TestCase):
    def setUp(self):
        self.directory = TemporaryDirectory()
        self.addCleanup(self.directory.cleanup)
        self.root = Path(self.directory.name).resolve()

    def package(self, name, dependencies=()):
        return {
            "id": name,
            "name": name,
            "manifest_path": str(self.root / "crates" / name / "Cargo.toml"),
            "dependencies": [
                {"path": str(self.root / "crates" / dep)} for dep in dependencies
            ],
        }

    def metadata(self):
        packages = [
            self.package("primitive"),
            self.package("middle", ["primitive"]),
            self.package("app", ["middle"]),
            self.package("independent"),
        ]
        return {"workspace_members": [p["id"] for p in packages], "packages": packages}

    def test_reverse_transitive_closure(self):
        self.assertEqual(
            affected(self.metadata(), {"crates/primitive/src/lib.rs"}, self.root)[0],
            ["app", "middle", "primitive"],
        )

    def test_independent(self):
        self.assertEqual(
            affected(self.metadata(), {"crates/independent/tests/test.rs"}, self.root)[0],
            ["independent"],
        )

    def test_global_fallback(self):
        for path in (
            "Cargo.toml", "Cargo.lock", "scripts/record.sh", "parity/done.txt",
            "crates/deleted/src/lib.rs", "crates/app/Cargo.toml",
            "crates/app/build.rs", "crates/app/tests/data.json",
        ):
            with self.subTest(path=path):
                self.assertEqual(affected(self.metadata(), {path}, self.root)[0], [])

    def test_no_changes_is_full(self):
        self.assertEqual(affected(self.metadata(), set(), self.root)[0], [])

    def test_external_path_dependency_is_full(self):
        metadata = self.metadata()
        metadata["packages"][0]["dependencies"].append(
            {"path": str(self.root / "outside")}
        )
        self.assertEqual(
            affected(metadata, {"crates/app/src/lib.rs"}, self.root)[0], []
        )


if __name__ == "__main__":
    unittest.main()
