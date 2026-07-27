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
    derive_frozen_selection_from_evidence,
    main,
    normalize_nextest_jsonl,
    split_feature_shards,
)
from scripts.verification.frozen_rust_nextest_target import (
    HIR_SEMANTIC_CONTROL_FLOW_PROFILE,
    TARGET_PROFILES,
    get_target_profile,
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

    def test_hir_profile_uses_exact_integration_target_filter(self) -> None:
        prefix = HIR_SEMANTIC_CONTROL_FLOW_PROFILE.test_prefix

        command = build_nextest_command(
            (
                f"{prefix}second",
                f"{prefix}first",
            ),
            profile=HIR_SEMANTIC_CONTROL_FLOW_PROFILE,
        )

        self.assertEqual(
            [
                "cargo",
                "nextest",
                "run",
                "-p",
                "trust-hir",
                "--test",
                "semantic_type_checking",
            ],
            command[:7],
        )
        expression = command[-1]
        self.assertIn(
            "binary_id(=trust-hir::semantic_type_checking)",
            expression,
        )
        self.assertIn(
            "test(=control_flow_and_calls::first)",
            expression,
        )
        self.assertNotIn("assignments_and_var_access::", expression)
        with self.assertRaisesRegex(
            ValueError,
            "has no no-default-features shard",
        ):
            build_nextest_command(
                (f"{prefix}first",),
                no_default_features=True,
                profile=HIR_SEMANTIC_CONTROL_FLOW_PROFILE,
            )

    def test_hir_remaining_profile_is_closed_over_reviewed_source_paths(self) -> None:
        profile = get_target_profile(
            "hir-remaining",
            path=(
                "crates/trust-hir/tests/semantic_type_checking/"
                "basics_and_warnings.rs"
            ),
        )

        self.assertEqual("trust-hir", profile.package)
        self.assertEqual("semantic_type_checking", profile.target_name)
        self.assertEqual(
            "trust-hir::semantic_type_checking",
            profile.binary_id,
        )
        self.assertEqual("basics_and_warnings::", profile.test_prefix)
        self.assertEqual(
            "trust-hir::semantic_type_checking$",
            profile.encoded_test_name_prefix,
        )
        with self.assertRaisesRegex(ValueError, "outside the closed HIR"):
            get_target_profile(
                "hir-remaining",
                path="crates/trust-hir/tests/not_reviewed.rs",
            )

    def test_syntax_remaining_profile_is_closed_over_reviewed_source_paths(self) -> None:
        self.assertIn("syntax-remaining", TARGET_PROFILES)
        integration = get_target_profile(
            "syntax-remaining",
            path="crates/trust-syntax/tests/parser_expressions.rs",
        )

        self.assertEqual("trust-syntax", integration.package)
        self.assertEqual("parser_expressions", integration.target_name)
        self.assertEqual(
            "trust-syntax::parser_expressions",
            integration.binary_id,
        )
        self.assertEqual("", integration.test_prefix)
        self.assertEqual(
            "trust-syntax::parser_expressions$",
            integration.encoded_test_name_prefix,
        )

        for excluded_path in (
            "crates/trust-syntax/src/syntax/mod.rs",
            "crates/trust-syntax/tests/parser_project_examples.rs",
            "crates/trust-syntax/tests/not_reviewed.rs",
        ):
            with self.assertRaisesRegex(ValueError, "outside the closed syntax"):
                get_target_profile(
                    "syntax-remaining",
                    path=excluded_path,
                )

    def test_debug_and_runtime_core_profiles_are_closed_over_reviewed_paths(
        self,
    ) -> None:
        debug = get_target_profile(
            "debug-remaining",
            path="crates/trust-debug/src/adapter/tests_part_03.rs",
        )
        runtime_core = get_target_profile(
            "runtime-core-remaining",
            path="crates/trust-runtime-core/src/value/reference.rs",
        )

        self.assertEqual("trust-debug", debug.package)
        self.assertEqual(
            "adapter::tests::tests_part_03::",
            debug.test_prefix,
        )
        self.assertEqual("trust-runtime-core", runtime_core.package)
        self.assertEqual(
            "value::reference::tests::",
            runtime_core.test_prefix,
        )
        with self.assertRaisesRegex(ValueError, "outside the closed debug"):
            get_target_profile(
                "debug-remaining",
                path="crates/trust-debug/src/not_reviewed.rs",
            )
        with self.assertRaisesRegex(
            ValueError,
            "outside the closed runtime-core",
        ):
            get_target_profile(
                "runtime-core-remaining",
                path="crates/trust-runtime-core/src/not_reviewed.rs",
            )

    def test_runtime_remaining_profile_resolves_reviewed_integration_path(
        self,
    ) -> None:
        profile = get_target_profile(
            "runtime-remaining",
            path="crates/trust-runtime/tests/bounded_value_semantics.rs",
        )

        self.assertEqual("trust-runtime", profile.package)
        self.assertEqual(
            ("--test", "bounded_value_semantics"),
            profile.cargo_target_args,
        )
        self.assertEqual(
            "trust-runtime::bounded_value_semantics",
            profile.binary_id,
        )

    def test_runtime_remaining_profile_resolves_openot_targets(self) -> None:
        unit = get_target_profile(
            "runtime-remaining",
            path="crates/trust-runtime/src/openot_authoring/tests.rs",
        )
        integration = get_target_profile(
            "runtime-remaining",
            path="crates/trust-runtime/tests/openot_telemetry.rs",
        )

        self.assertEqual(("--lib",), unit.cargo_target_args)
        self.assertEqual("openot_authoring::tests::", unit.test_prefix)
        self.assertEqual("rust_unit_test", unit.discovery_source_kind)
        self.assertEqual(
            ("--test", "openot_telemetry"),
            integration.cargo_target_args,
        )
        self.assertEqual(
            "trust-runtime::openot_telemetry",
            integration.binary_id,
        )
        with self.assertRaisesRegex(ValueError, "outside the closed runtime"):
            get_target_profile(
                "runtime-remaining",
                path="crates/trust-runtime/src/not_reviewed.rs",
            )

    def test_hir_selection_requires_integration_discovery_kind(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            verification = root / "verification"
            verification.mkdir()
            _write_toml(
                verification / "test-catalog-denominator.toml",
                """
                [[reviews]]
                discovery_id = "DISC_HIR"
                discovery_source_kind = "rust_integration_test"
                path = "tests/control_flow.rs"
                name = "one"
                disposition = "reviewed_nonmapping"
                ignore_state = "not_ignored"
                """,
            )
            _write_toml(
                verification / "test-catalog.toml",
                "tests = []",
            )

            selection = derive_frozen_selection(
                root=root,
                path="tests/control_flow.rs",
                expected_reviewed_nonmapping=1,
                expected_catalog_mapped=0,
                profile=HIR_SEMANTIC_CONTROL_FLOW_PROFILE,
            )

            self.assertEqual(
                ("control_flow_and_calls::one",),
                selection.full_names,
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

    def test_hir_profile_rejects_every_cfg_shard(self) -> None:
        with self.assertRaisesRegex(ValueError, "unsupported cfg attributes"):
            split_feature_shards(
                source="""
                #[cfg(unix)]
                #[test]
                fn platform_case() {}
                """,
                expected_full_names=(
                    "control_flow_and_calls::platform_case",
                ),
                profile=HIR_SEMANTIC_CONTROL_FLOW_PROFILE,
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

    def test_normalizer_accepts_only_hir_profile_encoded_name(self) -> None:
        raw = json.dumps(
            {
                "type": "test",
                "event": "ok",
                "name": (
                    "trust-hir::semantic_type_checking$"
                    "control_flow_and_calls::one"
                ),
                "exec_time": 0.001,
            }
        )

        self.assertEqual(
            [{"title": "one", "duration": 1, "state": "passed"}],
            normalize_nextest_jsonl(
                raw,
                expected_full_names={"control_flow_and_calls::one"},
                profile=HIR_SEMANTIC_CONTROL_FLOW_PROFILE,
            ),
        )
        with self.assertRaisesRegex(ValueError, "outside required prefix"):
            normalize_nextest_jsonl(
                raw,
                expected_full_names={f"{TEST_NAME_PREFIX}one"},
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

    def test_normalize_only_reuses_combined_evidence_after_ledger_apply(
        self,
    ) -> None:
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            source_path = Path(
                "crates/trust-runtime/src/control/tests/core.rs"
            )
            raw_path = root / "target/gate-artifacts/combined.jsonl"
            metadata_path = raw_path.with_suffix(".execution.json")
            source_content = b"#[test]\nfn first() {}\n"
            raw_content = (
                json.dumps(
                    {
                        "type": "test",
                        "event": "ok",
                        "name": f"{TEST_NAME_PREFIX}first",
                        "exec_time": 0.001,
                    }
                ).encode()
                + b"\n"
            )
            initial_denominator = b"reviews = []\n"
            initial_catalog = b"tests = []\n"
            contents = {
                source_path: source_content,
                DENOMINATOR_PATH: initial_denominator,
                CATALOG_PATH: initial_catalog,
                RUNNER_PATH: b"runner module\n",
                SHIM_PATH: b"runner shim\n",
                raw_path.relative_to(root): raw_content,
            }
            for relative, content in contents.items():
                path = root / relative
                path.parent.mkdir(parents=True, exist_ok=True)
                path.write_bytes(content)

            full_names = (f"{TEST_NAME_PREFIX}first",)
            command = build_nextest_command(full_names)
            selected_digest = _sha256(
                json.dumps(
                    list(full_names),
                    ensure_ascii=False,
                    separators=(",", ":"),
                ).encode()
            )
            missing_shard = Path(
                "target/gate-artifacts/combined.default.jsonl"
            )
            metadata = {
                "schema_version": 2,
                "command": shlex.join(command),
                "argv": [command],
                "environment": {
                    "NEXTEST_EXPERIMENTAL_LIBTEST_JSON": "1"
                },
                "nextest_version": "cargo-nextest 0.9.140",
                "exit_code": 0,
                "started_at": "2026-07-26T00:00:00Z",
                "finished_at": "2026-07-26T00:00:01Z",
                "path": source_path.as_posix(),
                "reviewed_nonmapping_count": 1,
                "catalog_mapped_count": 0,
                "selected_test_count": 1,
                "selected_names_sha256": selected_digest,
                "source_sha256": _sha256(source_content),
                "denominator_sha256": _sha256(initial_denominator),
                "catalog_sha256": _sha256(initial_catalog),
                "module_path": RUNNER_PATH.as_posix(),
                "module_sha256": _sha256(contents[RUNNER_PATH]),
                "shim_path": SHIM_PATH.as_posix(),
                "shim_sha256": _sha256(contents[SHIM_PATH]),
                "raw_results_path": raw_path.relative_to(root).as_posix(),
                "raw_results_sha256": _sha256(raw_content),
                "shards": [
                    {
                        "name": "default",
                        "command": shlex.join(command),
                        "argv": command,
                        "selected_test_count": 1,
                        "selected_names_sha256": selected_digest,
                        "raw_results_path": missing_shard.as_posix(),
                        "raw_results_sha256": "sha256:" + "0" * 64,
                        "exit_code": 0,
                        "started_at": "2026-07-26T00:00:00Z",
                        "finished_at": "2026-07-26T00:00:01Z",
                    }
                ],
            }
            metadata_path.write_text(json.dumps(metadata))
            (root / DENOMINATOR_PATH).write_text(
                '[[reviews]]\ndisposition = "catalog_mapped"\n'
            )
            (root / CATALOG_PATH).write_text(
                '[[tests]]\nstatus = "mapped"\n'
            )

            selection = derive_frozen_selection_from_evidence(
                root=root,
                path=source_path.as_posix(),
                raw_path=raw_path,
                metadata=metadata,
            )
            document = build_normalized_document(
                root=root,
                selection=selection,
                raw_path=raw_path,
                metadata=metadata,
                reuse_frozen_evidence=True,
            )

            self.assertEqual(
                [{"title": "first", "duration": 1, "state": "passed"}],
                document["tests"],
            )
            self.assertFalse((root / missing_shard).exists())
            results_path = Path("target/gate-artifacts/reused.json")
            self.assertEqual(
                0,
                main(
                    [
                        "--root",
                        str(root),
                        "--path",
                        source_path.as_posix(),
                        "--raw-results",
                        raw_path.relative_to(root).as_posix(),
                        "--execution-metadata",
                        metadata_path.relative_to(root).as_posix(),
                        "--results",
                        results_path.as_posix(),
                        "--normalize-only",
                    ]
                ),
            )
            self.assertTrue((root / results_path).is_file())


if __name__ == "__main__":
    unittest.main()
