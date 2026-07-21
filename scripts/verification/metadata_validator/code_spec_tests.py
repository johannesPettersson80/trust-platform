"""Tests for Phase 17 ownership in the primary metadata validator."""

from __future__ import annotations

import shutil
import tempfile
import unittest
from pathlib import Path

from scripts.verification.metadata_validator.code_spec import (
    validate_code_spec_metadata_instances,
)


ROOT = Path(__file__).resolve().parents[3]


class CodeSpecMetadataTests(unittest.TestCase):
    def test_invalid_committed_map_is_rejected_against_its_schema(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            schemas = root / "verification/schemas"
            schemas.mkdir(parents=True)
            for name in (
                "code-spec-map.schema.json",
                "code-spec-debt-baseline.schema.json",
            ):
                shutil.copy2(ROOT / "verification/schemas" / name, schemas / name)
            (root / "verification/code-spec-map.toml").write_text(
                "schema_version = 1\n",
                encoding="utf-8",
            )
            (root / "verification/code-spec-debt-baseline.toml").write_text(
                'schema_version = 1\ncreated_from_commit = "' + "a" * 40 + '"\n'
                'captured_on = "2026-07-21"\nenforcement_mode = "changed"\n'
                'append_policy = "delete_only"\nentries = []\n',
                encoding="utf-8",
            )

            failures = validate_code_spec_metadata_instances(root)

        self.assertTrue(any("code-spec-map.toml" in str(path) for path, _ in failures))
        self.assertFalse(
            any("code-spec-debt-baseline.toml" in str(path) for path, _ in failures)
        )


if __name__ == "__main__":
    unittest.main()
