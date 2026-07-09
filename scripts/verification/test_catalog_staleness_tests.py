"""Tests for committed catalog path and generated-test identity checks."""

from __future__ import annotations

import tempfile
import unittest
from pathlib import Path

from scripts.verification.test_catalog_common import make_fact
from scripts.verification.test_catalog_rust import scan_rust_tests
from scripts.verification.test_catalog_staleness import validate_catalog_staleness


class TestCatalogStalenessTests(unittest.TestCase):
    def test_generated_binding_accepts_line_only_movement(self) -> None:
        with catalog_root() as root:
            fact = generated_fact(line=99)
            failures = validate_catalog_staleness(
                root=root,
                tests={"TEST_INVALID_MAGIC": generated_record()},
                facts=[fact],
            )

        self.assertEqual(failures, [])

    def test_rename_inside_surviving_file_fails(self) -> None:
        with catalog_root() as root:
            renamed = generated_fact(name="renamed_header_validation")
            failures = validate_catalog_staleness(
                root=root,
                tests={"TEST_INVALID_MAGIC": generated_record()},
                facts=[renamed],
            )

        self.assertTrue(any("discovery_id is absent" in item for item in failures))

    def test_deleted_or_moved_test_and_source_kind_drift_fail(self) -> None:
        with catalog_root() as root:
            deleted = validate_catalog_staleness(
                root=root,
                tests={"TEST_INVALID_MAGIC": generated_record()},
                facts=[],
            )
            moved = generated_fact(path="crates/trust-runtime/tests/moved.rs")
            moved_failure = validate_catalog_staleness(
                root=root,
                tests={"TEST_INVALID_MAGIC": generated_record()},
                facts=[moved],
            )
            wrong_kind = generated_fact(source_kind="rust_unit_test")
            kind_failure = validate_catalog_staleness(
                root=root,
                tests={"TEST_INVALID_MAGIC": generated_record()},
                facts=[wrong_kind],
            )

        self.assertTrue(any("discovery_id is absent" in item for item in deleted))
        self.assertTrue(any("discovery_id is absent" in item for item in moved_failure))
        self.assertTrue(any("discovery_id is absent" in item for item in kind_failure))

    def test_duplicate_generated_identity_fails_closed(self) -> None:
        with catalog_root() as root:
            fact = generated_fact()
            failures = validate_catalog_staleness(
                root=root,
                tests={"TEST_INVALID_MAGIC": generated_record()},
                facts=[fact, fact],
            )

        self.assertTrue(any("resolved to 2 scanner facts" in item for item in failures))

    def test_every_subject_requires_an_existing_safe_path(self) -> None:
        with catalog_root() as root:
            missing = generated_record()
            missing["path"] = "crates/trust-runtime/tests/missing.rs"
            failures = validate_catalog_staleness(root=root, tests={missing["id"]: missing}, facts=[])

        self.assertTrue(any("path does not exist" in item for item in failures))

    def test_special_subjects_pass_without_scanner_facts_and_forbid_bindings(self) -> None:
        with catalog_root() as root:
            case_table = {
                "id": "TEST_CASE",
                "subject_kind": "case_table_artifact",
                "path": "verification/cases/bytecode_vm/VM_SEAM_VALID_001.toml",
            }
            mutation = {
                "id": "TEST_MUTATION",
                "subject_kind": "mutation_shard_runner",
                "path": "scripts/bytecode_validator_mutation.py",
            }
            self.assertEqual(
                validate_catalog_staleness(
                    root=root,
                    tests={case_table["id"]: case_table, mutation["id"]: mutation},
                    facts=[],
                ),
                [],
            )

            mutation["discovery_id"] = "DISC_" + "A" * 20
            failures = validate_catalog_staleness(
                root=root,
                tests={mutation["id"]: mutation},
                facts=[],
            )
        self.assertTrue(any("must not carry scanner identity" in item for item in failures))

    def test_real_rust_scan_detects_rename_inside_surviving_file(self) -> None:
        with tempfile.TemporaryDirectory() as temp:
            root = Path(temp)
            manifest = root / "crates/fixture/Cargo.toml"
            source = root / "crates/fixture/tests/identity.rs"
            manifest.parent.mkdir(parents=True)
            source.parent.mkdir(parents=True)
            manifest.write_text('[package]\nname = "fixture"\nversion = "0.1.0"\n')
            source.write_text("#[test]\nfn original_name() {}\n")
            original = scan_rust_tests(root).facts[0]
            record = {
                "id": "TEST_IDENTITY",
                "subject_kind": "generated_test",
                "discovery_id": original.stable_id,
                "discovery_source_kind": original.source_kind,
                "path": original.path,
                "name": original.name,
            }

            self.assertEqual(
                validate_catalog_staleness(root=root, tests={record["id"]: record}, facts=[original]),
                [],
            )
            source.write_text("#[test]\nfn renamed_inside_same_file() {}\n")
            renamed = scan_rust_tests(root).facts
            failures = validate_catalog_staleness(
                root=root,
                tests={record["id"]: record},
                facts=renamed,
            )

        self.assertTrue(any("discovery_id is absent" in item for item in failures))


def generated_record() -> dict:
    return {
        "id": "TEST_INVALID_MAGIC",
        "subject_kind": "generated_test",
        "discovery_id": generated_fact().stable_id,
        "discovery_source_kind": "rust_integration_test",
        "path": "crates/trust-runtime/tests/bytecode_container.rs",
        "name": "header_validation",
    }


def generated_fact(
    *,
    name: str = "header_validation",
    path: str = "crates/trust-runtime/tests/bytecode_container.rs",
    line: int = 9,
    source_kind: str = "rust_integration_test",
):
    return make_fact(
        source_kind=source_kind,
        name=name,
        path=path,
        line=line,
        package="trust-runtime",
        command_hint="cargo test -p trust-runtime --test bytecode_container header_validation",
        command_hint_authority="conservative",
        discovery_confidence="exact_attribute",
    )


class catalog_root:
    def __init__(self) -> None:
        self._temp = tempfile.TemporaryDirectory()
        self.root = Path(self._temp.name)

    def __enter__(self) -> Path:
        for relative in (
            "crates/trust-runtime/tests/bytecode_container.rs",
            "verification/cases/bytecode_vm/VM_SEAM_VALID_001.toml",
            "scripts/bytecode_validator_mutation.py",
        ):
            path = self.root / relative
            path.parent.mkdir(parents=True, exist_ok=True)
            path.write_text("fixture\n")
        return self.root

    def __exit__(self, exc_type, exc, tb) -> None:
        self._temp.cleanup()


if __name__ == "__main__":
    unittest.main()
