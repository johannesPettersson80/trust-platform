from __future__ import annotations

import hashlib
import json
import shlex
import tempfile
import unittest
from pathlib import Path

from scripts.verification.frozen_rust_nextest_batch import (
    ADS_WIRE_DISABLED_CFG,
    BINARY_ID,
    CATALOG_PATH,
    DENOMINATOR_PATH,
    RUNNER_PATH,
    SHIM_PATH,
    TEST_NAME_PREFIX,
    FrozenSelection,
    build_nextest_command,
    build_normalized_document,
    derive_frozen_selection,
    normalize_nextest_jsonl,
    split_feature_shards,
)


def _write_toml(path: Path, content: str) -> None:
    path.write_text(content.strip() + "\n")


def _sha256(content: bytes) -> str:
    return "sha256:" + hashlib.sha256(content).hexdigest()


class FrozenRustNextestBatchTests(unittest.TestCase):
    def test_selection_is_exact_reviewed_plus_catalog_mapped_union(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            verification = root / "verification"
            verification.mkdir()
            _write_toml(
                verification / "test-catalog-denominator.toml",
                """
                [[reviews]]
                discovery_id = "DISC_REVIEWED"
                discovery_source_kind = "rust_unit_test"
                path = "tests/core.rs"
                name = "reviewed_case"
                disposition = "reviewed_nonmapping"
                ignore_state = "not_ignored"

                [[reviews]]
                discovery_id = "DISC_MAPPED"
                discovery_source_kind = "rust_unit_test"
                path = "tests/core.rs"
                name = "mapped_case"
                disposition = "catalog_mapped"
                ignore_state = "not_ignored"

                [[reviews]]
                discovery_id = "DISC_OTHER"
                discovery_source_kind = "rust_unit_test"
                path = "tests/other.rs"
                name = "other_case"
                disposition = "reviewed_nonmapping"
                ignore_state = "not_ignored"
                """,
            )
            _write_toml(
                verification / "test-catalog.toml",
                """
                [[tests]]
                discovery_id = "DISC_MAPPED"
                discovery_source_kind = "rust_unit_test"
                path = "tests/core.rs"
                name = "mapped_case"
                status = "mapped"
                """,
            )

            selection = derive_frozen_selection(
                root=root,
                path="tests/core.rs",
                expected_reviewed_nonmapping=1,
                expected_catalog_mapped=1,
            )

            self.assertEqual(("mapped_case", "reviewed_case"), selection.names)
            self.assertEqual(
                (
                    f"{TEST_NAME_PREFIX}mapped_case",
                    f"{TEST_NAME_PREFIX}reviewed_case",
                ),
                selection.full_names,
            )

    def test_selection_rejects_invalid_frozen_metadata(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            verification = root / "verification"
            verification.mkdir()
            _write_toml(
                verification / "test-catalog-denominator.toml",
                """
                [[reviews]]
                discovery_id = "DISC_A"
                discovery_source_kind = "rust_unit_test"
                path = "tests/core.rs"
                name = "same"
                disposition = "reviewed_nonmapping"
                ignore_state = "not_ignored"

                [[reviews]]
                discovery_id = "DISC_B"
                discovery_source_kind = "rust_unit_test"
                path = "tests/core.rs"
                name = "same"
                disposition = "catalog_mapped"
                ignore_state = "not_ignored"
                """,
            )
            _write_toml(
                verification / "test-catalog.toml",
                """
                [[tests]]
                discovery_id = "DISC_WRONG"
                discovery_source_kind = "rust_unit_test"
                path = "tests/core.rs"
                name = "same"
                status = "mapped"
                """,
            )

            with self.assertRaisesRegex(ValueError, "mapped denominator identities"):
                derive_frozen_selection(
                    root=root,
                    path="tests/core.rs",
                    expected_reviewed_nonmapping=1,
                    expected_catalog_mapped=1,
                )

            _write_toml(
                verification / "test-catalog.toml",
                """
                [[tests]]
                discovery_id = "DISC_B"
                discovery_source_kind = "rust_unit_test"
                path = "tests/core.rs"
                name = "same"
                status = "mapped"
                """,
            )
            with self.assertRaisesRegex(ValueError, "duplicate selected test name"):
                derive_frozen_selection(
                    root=root,
                    path="tests/core.rs",
                    expected_reviewed_nonmapping=1,
                    expected_catalog_mapped=1,
                )
            with self.assertRaisesRegex(ValueError, "expected 2"):
                derive_frozen_selection(
                    root=root,
                    path="tests/core.rs",
                    expected_reviewed_nonmapping=2,
                    expected_catalog_mapped=1,
                )

    def test_nextest_command_uses_one_exact_sorted_or_filter(self) -> None:
        command = build_nextest_command(
            (
                f"{TEST_NAME_PREFIX}second",
                f"{TEST_NAME_PREFIX}first",
            )
        )
        self.assertEqual(
            [
                "cargo",
                "nextest",
                "run",
                "-p",
                "trust-runtime",
                "--lib",
                "--no-fail-fast",
                "--message-format",
                "libtest-json-plus",
                "--message-format-version",
                "0.1",
                "--user-config-file",
                "none",
                "-E",
                (
                    f"(binary_id(={BINARY_ID}) & "
                    f"test(={TEST_NAME_PREFIX}first)) | "
                    f"(binary_id(={BINARY_ID}) & "
                    f"test(={TEST_NAME_PREFIX}second))"
                ),
            ],
            command,
        )

    def test_feature_shards_are_derived_from_exact_source_attributes(self) -> None:
        names = (
            f"{TEST_NAME_PREFIX}default_case",
            f"{TEST_NAME_PREFIX}without_wire",
            f"{TEST_NAME_PREFIX}server_case",
        )
        shards = split_feature_shards(
            source=f"""
            #[test]
            fn default_case() {{}}

            {ADS_WIRE_DISABLED_CFG}
            #[test]
            fn without_wire() {{}}

            #[cfg(feature = "ads-server")]
            #[test]
            fn server_case() {{}}
            """,
            expected_full_names=names,
        )

        self.assertEqual(
            ("default", "no-ads-wire"),
            tuple(shard.name for shard in shards),
        )
        self.assertEqual(
            (
                f"{TEST_NAME_PREFIX}default_case",
                f"{TEST_NAME_PREFIX}server_case",
            ),
            shards[0].full_names,
        )
        self.assertEqual(
            (f"{TEST_NAME_PREFIX}without_wire",),
            shards[1].full_names,
        )
        self.assertTrue(shards[1].no_default_features)
        self.assertIn(
            "--no-default-features",
            build_nextest_command(
                shards[1].full_names,
                no_default_features=shards[1].no_default_features,
            ),
        )

    def test_feature_shards_fail_closed_on_unknown_cfg(self) -> None:
        with self.assertRaisesRegex(ValueError, "unsupported cfg attributes"):
            split_feature_shards(
                source="""
                #[cfg(unix)]
                #[test]
                fn platform_case() {}
                """,
                expected_full_names=(f"{TEST_NAME_PREFIX}platform_case",),
            )

    def test_normalizer_records_exact_passing_set_and_milliseconds(self) -> None:
        raw = "\n".join(
            [
                json.dumps(
                    {"type": "suite", "event": "started", "test_count": 2}
                ),
                json.dumps(
                    {
                        "type": "test",
                        "event": "started",
                        "name": f"{TEST_NAME_PREFIX}first",
                    }
                ),
                json.dumps(
                    {
                        "type": "test",
                        "event": "ok",
                        "name": f"{TEST_NAME_PREFIX}first",
                        "exec_time": 0.0012,
                    }
                ),
                json.dumps(
                    {
                        "type": "test",
                        "event": "ok",
                        "name": f"{TEST_NAME_PREFIX}second",
                        "exec_time": 0,
                    }
                ),
                json.dumps({"type": "suite", "event": "ok"}),
            ]
        )
        tests = normalize_nextest_jsonl(
            raw,
            expected_full_names={
                f"{TEST_NAME_PREFIX}first",
                f"{TEST_NAME_PREFIX}second",
            },
        )

        self.assertEqual(
            [
                {"title": "first", "duration": 2, "state": "passed"},
                {"title": "second", "duration": 0, "state": "passed"},
            ],
            tests,
        )

    def test_normalizer_accepts_nextest_encoded_library_name(self) -> None:
        raw = json.dumps(
            {
                "type": "test",
                "event": "ok",
                "name": (
                    "trust-runtime::trust_runtime$"
                    f"{TEST_NAME_PREFIX}encoded"
                ),
                "exec_time": 0.001,
            }
        )

        self.assertEqual(
            [{"title": "encoded", "duration": 1, "state": "passed"}],
            normalize_nextest_jsonl(
                raw,
                expected_full_names={f"{TEST_NAME_PREFIX}encoded"},
            ),
        )

    def test_normalizer_rejects_missing_unexpected_duplicate_and_nonpass(self) -> None:
        expected = {f"{TEST_NAME_PREFIX}first"}

        with self.assertRaisesRegex(ValueError, "missing test results"):
            normalize_nextest_jsonl("", expected_full_names=expected)
        with self.assertRaisesRegex(ValueError, "outside required prefix"):
            normalize_nextest_jsonl(
                json.dumps(
                    {
                        "type": "test",
                        "event": "ok",
                        "name": "other::first",
                        "exec_time": 0.1,
                    }
                ),
                expected_full_names=expected,
            )
        unexpected = json.dumps(
            {
                "type": "test",
                "event": "ok",
                "name": f"{TEST_NAME_PREFIX}second",
                "exec_time": 0.1,
            }
        )
        with self.assertRaisesRegex(ValueError, "unexpected test result"):
            normalize_nextest_jsonl(
                unexpected,
                expected_full_names=expected,
            )
        passing = json.dumps(
            {
                "type": "test",
                "event": "ok",
                "name": f"{TEST_NAME_PREFIX}first",
                "exec_time": 0.1,
            }
        )
        with self.assertRaisesRegex(ValueError, "duplicate terminal result"):
            normalize_nextest_jsonl(
                passing + "\n" + passing,
                expected_full_names=expected,
            )
        failed = json.dumps(
            {
                "type": "test",
                "event": "failed",
                "name": f"{TEST_NAME_PREFIX}first",
                "exec_time": 0.1,
            }
        )
        with self.assertRaisesRegex(ValueError, "did not pass"):
            normalize_nextest_jsonl(
                failed,
                expected_full_names=expected,
            )

    def test_normalized_document_matches_rust_catalog_compiler_contract(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            source_path = Path("crates/trust-runtime/src/control/tests/core.rs")
            raw_path = root / "target/gate-artifacts/control-core.jsonl"
            contents = {
                source_path: b"#[test]\nfn first() {}\n",
                DENOMINATOR_PATH: b"reviews = []\n",
                CATALOG_PATH: b"tests = []\n",
                RUNNER_PATH: b"runner module\n",
                SHIM_PATH: b"runner shim\n",
                raw_path.relative_to(root): (
                    json.dumps(
                        {
                            "type": "test",
                            "event": "ok",
                            "name": f"{TEST_NAME_PREFIX}first",
                            "exec_time": 0.001,
                        }
                    ).encode()
                    + b"\n"
                ),
            }
            for relative, content in contents.items():
                path = root / relative
                path.parent.mkdir(parents=True, exist_ok=True)
                path.write_bytes(content)
            selection = FrozenSelection(
                path=source_path.as_posix(),
                reviewed_nonmapping_count=1,
                catalog_mapped_count=0,
                names=("first",),
                full_names=(f"{TEST_NAME_PREFIX}first",),
            )
            command = build_nextest_command(selection.full_names)
            selected_digest = _sha256(
                json.dumps(
                    sorted(selection.full_names),
                    ensure_ascii=False,
                    separators=(",", ":"),
                ).encode()
            )
            metadata = {
                "schema_version": 1,
                "command": shlex.join(command),
                "argv": command,
                "environment": {"NEXTEST_EXPERIMENTAL_LIBTEST_JSON": "1"},
                "nextest_version": "cargo-nextest 0.9.140",
                "exit_code": 0,
                "started_at": "2026-07-26T00:00:00Z",
                "finished_at": "2026-07-26T00:00:01Z",
                "path": source_path.as_posix(),
                "reviewed_nonmapping_count": 1,
                "catalog_mapped_count": 0,
                "selected_test_count": 1,
                "selected_names_sha256": selected_digest,
                "source_sha256": _sha256(contents[source_path]),
                "denominator_sha256": _sha256(contents[DENOMINATOR_PATH]),
                "catalog_sha256": _sha256(contents[CATALOG_PATH]),
                "module_path": RUNNER_PATH.as_posix(),
                "module_sha256": _sha256(contents[RUNNER_PATH]),
                "shim_path": SHIM_PATH.as_posix(),
                "shim_sha256": _sha256(contents[SHIM_PATH]),
                "raw_results_path": raw_path.relative_to(root).as_posix(),
                "raw_results_sha256": _sha256(
                    contents[raw_path.relative_to(root)]
                ),
            }

            document = build_normalized_document(
                root=root,
                selection=selection,
                raw_path=raw_path,
                metadata=metadata,
            )

            self.assertEqual(
                {
                    "kind",
                    "batch_command",
                    "runner_path",
                    "runner_sha256",
                    "source_path",
                    "source_sha256",
                    "raw_results_path",
                    "raw_results_sha256",
                    "nextest_version",
                    "package",
                    "target_kind",
                    "binary_id",
                    "test_prefix",
                },
                set(document["execution"]),
            )
            self.assertEqual(
                {
                    "tests": 1,
                    "passes": 1,
                    "failures": 0,
                    "pending": 0,
                },
                document["stats"],
            )


if __name__ == "__main__":
    unittest.main()
