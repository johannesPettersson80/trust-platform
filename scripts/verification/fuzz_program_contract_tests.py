"""Contract tests for the Phase 9 fuzz-program inventory."""

from __future__ import annotations

import copy
from dataclasses import replace
import json
import re
import tempfile
import unittest
from pathlib import Path
from unittest import mock

from .fuzz_program_contract import (
    FUZZ_PROGRAM_PATH,
    FUZZ_PROGRAM_SCHEMA_PATH,
    REQUIRED_SURFACE_IDS,
    load_fuzz_program,
    _validate_corpus_storage,
    validate_fuzz_program_contract,
)
from .fuzz_program_source_contract import (
    REVIEWED_RUNTIME_COMMS_COMMANDS,
    REVIEWED_VM_FUZZ_COMMANDS,
    _parse_runtime_comms_commands,
    _parse_salsa_run_targets,
    _parse_vm_fuzz_commands,
    validate_execution_source_bindings,
    validate_reviewed_execution_source_digests,
)
from .fuzz_program_analysis import analyze_fuzz_program
from .fuzz_program_discovery import (
    has_unmodeled_property_framework,
    is_fuzz_like_test_name,
    scan_cargo_fuzz_targets,
    scan_fuzz_like_tests,
)
from .metadata_validator.constants import ROOT
from .metadata_validator.core import Validator


EXPECTED_CARGO_TARGETS = {
    ("fuzz/Cargo.toml", "syntax_parse", "fuzz/fuzz_targets/syntax_parse.rs"),
    ("fuzz/Cargo.toml", "hir_semantic", "fuzz/fuzz_targets/hir_semantic.rs"),
    ("fuzz/Cargo.toml", "hir_lowering", "fuzz/fuzz_targets/hir_lowering.rs"),
    ("fuzz/Cargo.toml", "plcopen_xml", "fuzz/fuzz_targets/plcopen_xml.rs"),
    ("fuzz/Cargo.toml", "bytecode_container", "fuzz/fuzz_targets/bytecode_container.rs"),
    ("fuzz/Cargo.toml", "runtime_config", "fuzz/fuzz_targets/runtime_config.rs"),
    ("fuzz/Cargo.toml", "lsp_incremental", "fuzz/fuzz_targets/lsp_incremental.rs"),
    ("fuzz/Cargo.toml", "hmi_payloads", "fuzz/fuzz_targets/hmi_payloads.rs"),
    (
        "crates/trust-ads-server/fuzz/Cargo.toml",
        "ams_frame",
        "crates/trust-ads-server/fuzz/fuzz_targets/ams_frame.rs",
    ),
    (
        "crates/trust-ads-server/fuzz/Cargo.toml",
        "boundary_noop",
        "crates/trust-ads-server/fuzz/fuzz_targets/boundary_noop.rs",
    ),
    (
        "crates/trust-ads-server/fuzz/Cargo.toml",
        "command_dispatch",
        "crates/trust-ads-server/fuzz/fuzz_targets/command_dispatch.rs",
    ),
}

EXPECTED_SMOKE_IDS = {
    "DISC_21449A3BBD5F3F55D531",
    "DISC_49D6842FE830D483460D",
    "DISC_794F59E9A339F867023D",
    "DISC_A6037EE5CFAA0C4994D2",
    "DISC_E97822CE4B2200DD8928",
    "DISC_FB4371C17A9F9FB83CA9",
}


class FuzzProgramDiscoveryTests(unittest.TestCase):
    def test_live_inventory_covers_root_and_nested_cargo_fuzz_targets(self) -> None:
        result = scan_cargo_fuzz_targets(ROOT)
        self.assertEqual([], list(result.diagnostics))
        self.assertEqual(
            EXPECTED_CARGO_TARGETS,
            {
                (fact.manifest_path, fact.name, fact.path)
                for fact in result.facts
            },
        )

    def test_live_inventory_covers_reviewed_fuzz_and_property_smokes(self) -> None:
        result = scan_fuzz_like_tests(ROOT)
        self.assertEqual([], list(result.diagnostics))
        self.assertEqual(EXPECTED_SMOKE_IDS, {fact.stable_id for fact in result.facts})

    def test_candidate_vocabulary_is_narrow_and_explicit(self) -> None:
        self.assertTrue(is_fuzz_like_test_name("decoder_fuzz_smoke_budget"))
        self.assertTrue(is_fuzz_like_test_name("parser_property_smoke_generated_shapes"))
        self.assertTrue(is_fuzz_like_test_name("randomized_payload_smoke_budget"))
        self.assertTrue(is_fuzz_like_test_name("smoke_payload_randomized"))
        self.assertTrue(is_fuzz_like_test_name("proptest_decoder"))
        self.assertFalse(is_fuzz_like_test_name("rejects_one_malformed_document"))
        self.assertFalse(is_fuzz_like_test_name("test_property_accessor"))
        for source in (
            "#[quickcheck]\nfn reverse_twice() {}",
            "use quickcheck_macros::quickcheck;",
            "proptest! { #[test] fn property() {} }",
            "quickcheck::quickcheck(reverse_twice as fn(Vec<u8>) -> bool);",
        ):
            with self.subTest(source=source):
                self.assertTrue(has_unmodeled_property_framework(source))

    def test_nested_manifest_target_escape_fails_visibly(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            manifest = root / "crates/example/fuzz/Cargo.toml"
            manifest.parent.mkdir(parents=True)
            manifest.write_text(
                "[package]\nname = \"example-fuzz\"\n"
                "publish = false\n[package.metadata]\ncargo-fuzz = true\n"
                "[[bin]]\nname = \"escape\"\npath = \"../escape.rs\"\n"
                "test = false\ndoc = false\nbench = false\n"
            )
            (manifest.parent.parent / "escape.rs").write_text("#![no_main]\n")
            result = scan_cargo_fuzz_targets(root, tracked_paths={
                "crates/example/fuzz/Cargo.toml",
                "crates/example/escape.rs",
            })
            self.assertEqual([], list(result.facts))
            self.assertTrue(
                any(item.kind == "fuzz_target_path" for item in result.diagnostics),
                result.diagnostics,
            )

    def test_non_table_manifest_metadata_fails_with_a_diagnostic(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            manifest = root / "fuzz/Cargo.toml"
            manifest.parent.mkdir(parents=True)
            manifest.write_text(
                "[package]\nname = \"example-fuzz\"\npublish = false\n"
                "metadata = \"hostile\"\n"
                "[[bin]]\nname = \"known\"\npath = \"fuzz_targets/known.rs\"\n"
                "test = false\ndoc = false\nbench = false\n"
            )
            result = scan_cargo_fuzz_targets(root, tracked_paths={"fuzz/Cargo.toml"})
            self.assertTrue(
                any(item.kind == "fuzz_manifest_contract" for item in result.diagnostics),
                result.diagnostics,
            )

    def test_orphan_fuzz_target_source_fails_visibly(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            manifest = root / "fuzz/Cargo.toml"
            target_root = root / "fuzz/fuzz_targets"
            target_root.mkdir(parents=True)
            manifest.write_text(
                "[package]\nname = \"example-fuzz\"\npublish = false\n"
                "[package.metadata]\ncargo-fuzz = true\n"
                "[[bin]]\nname = \"known\"\npath = \"fuzz_targets/known.rs\"\n"
                "test = false\ndoc = false\nbench = false\n"
            )
            (target_root / "known.rs").write_text("#![no_main]\n")
            (target_root / "orphan.rs").write_text("#![no_main]\n")
            tracked = {
                "fuzz/Cargo.toml",
                "fuzz/fuzz_targets/known.rs",
                "fuzz/fuzz_targets/orphan.rs",
            }
            result = scan_cargo_fuzz_targets(root, tracked_paths=tracked)
            self.assertTrue(
                any(item.kind == "fuzz_target_unregistered" for item in result.diagnostics),
                result.diagnostics,
            )

    def test_symlinked_fuzz_target_fails_visibly(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            target_root = root / "fuzz/fuzz_targets"
            target_root.mkdir(parents=True)
            (root / "fuzz/Cargo.toml").write_text(
                "[package]\nname = \"example-fuzz\"\npublish = false\n"
                "[package.metadata]\ncargo-fuzz = true\n"
                "[[bin]]\nname = \"linked\"\npath = \"fuzz_targets/linked.rs\"\n"
                "test = false\ndoc = false\nbench = false\n"
            )
            (root / "outside.rs").write_text("#![no_main]\n")
            (target_root / "linked.rs").symlink_to(root / "outside.rs")
            result = scan_cargo_fuzz_targets(
                root,
                tracked_paths={"fuzz/Cargo.toml", "fuzz/fuzz_targets/linked.rs"},
            )
            self.assertTrue(
                any(item.kind == "fuzz_target_missing" for item in result.diagnostics),
                result.diagnostics,
            )

    def test_new_fuzz_like_candidate_requires_reviewed_registration(self) -> None:
        cargo = scan_cargo_fuzz_targets(ROOT)
        smokes = scan_fuzz_like_tests(ROOT)
        fake = replace(
            smokes.facts[0],
            stable_id="DISC_FFFFFFFFFFFFFFFFFFFF",
            native_id="new_fuzz_smoke_budget",
            name="new_fuzz_smoke_budget",
        )
        smokes.facts.append(fake)
        program = load_fuzz_program(ROOT)
        _, failures = analyze_fuzz_program(program, cargo, smokes)
        self.assertTrue(
            any("does not exactly match live candidate facts" in item for item in failures),
            failures,
        )

    def test_ignored_or_conditional_smoke_cannot_retain_a_runnable_tier(self) -> None:
        for ignore_state in ("ignored", "conditional"):
            with self.subTest(ignore_state=ignore_state):
                cargo = scan_cargo_fuzz_targets(ROOT)
                smokes = scan_fuzz_like_tests(ROOT)
                smokes.facts[0] = replace(smokes.facts[0], ignore_state=ignore_state)
                _, failures = analyze_fuzz_program(load_fuzz_program(ROOT), cargo, smokes)
                self.assertTrue(
                    any("must be not_ignored" in item for item in failures),
                    failures,
                )


class FuzzProgramContractTests(unittest.TestCase):
    @classmethod
    def setUpClass(cls) -> None:
        cls.program = load_fuzz_program(ROOT)

    def test_required_surface_order_is_exact(self) -> None:
        self.assertEqual(
            list(REQUIRED_SURFACE_IDS),
            [row["id"] for row in self.program["surfaces"]],
        )

    def test_live_contract_is_valid(self) -> None:
        self.assertEqual([], validate_fuzz_program_contract(ROOT, self.program))

    def test_program_and_schema_are_closed_and_order_pinned(self) -> None:
        schema = json.loads((ROOT / FUZZ_PROGRAM_SCHEMA_PATH).read_text())
        self.assertFalse(schema["additionalProperties"])
        self.assertEqual(
            list(REQUIRED_SURFACE_IDS),
            schema["$defs"]["surface_id"]["enum"],
        )

    def test_schema_weakening_is_drift_detected(self) -> None:
        schema = json.loads((ROOT / FUZZ_PROGRAM_SCHEMA_PATH).read_text())
        schema["properties"]["title"].pop("minLength")
        schema["$defs"]["target"]["required"].remove("command")
        schema["properties"]["targets"]["minItems"] = 0
        schema["$defs"]["target"]["properties"]["path"] = {"type": "integer"}
        schema["$defs"]["target"]["properties"]["command"].pop("minLength")
        schema["$defs"]["target"]["properties"]["additional_tiers"] = {"type": "array"}
        schema["$defs"]["surface"]["additionalProperties"] = True
        schema["$defs"]["surface"]["properties"]["id"] = {"type": "string"}
        schema["$defs"]["surface"]["properties"]["area"] = {"type": "string"}
        schema["$defs"]["surface_association"]["properties"]["surface_id"] = {
            "type": "string"
        }
        schema["$defs"]["area"]["enum"].append("invented")
        with mock.patch(
            "scripts.verification.fuzz_program_contract.json.loads",
            return_value=schema,
        ):
            failures = validate_fuzz_program_contract(ROOT, self.program)
        self.assertTrue(any("target required fields drifted" in item for item in failures), failures)
        self.assertTrue(any("root title contract drifted" in item for item in failures), failures)
        self.assertTrue(any("targets array contract drifted" in item for item in failures), failures)
        self.assertTrue(any("target path contract drifted" in item for item in failures), failures)
        self.assertTrue(any("target command contract drifted" in item for item in failures), failures)
        self.assertTrue(any("target additional_tiers contract drifted" in item for item in failures), failures)
        self.assertTrue(any("surface must be a closed object" in item for item in failures), failures)
        self.assertTrue(any("surface id binding drifted" in item for item in failures), failures)
        self.assertTrue(any("surface area binding drifted" in item for item in failures), failures)
        self.assertTrue(any("association surface_id binding drifted" in item for item in failures), failures)
        self.assertTrue(any("area enum drifted" in item for item in failures), failures)
        self.assertTrue(any("semantic digest drifted" in item for item in failures), failures)

    def test_every_live_target_has_one_registered_record(self) -> None:
        self.assertEqual(17, len(self.program["targets"]))
        self.assertEqual(
            11,
            sum(row["target_kind"] == "cargo_fuzz" for row in self.program["targets"]),
        )
        self.assertEqual(
            6,
            sum(row["target_kind"] == "bounded_rust_smoke" for row in self.program["targets"]),
        )

    def test_bounded_rust_smokes_select_one_test_binary(self) -> None:
        for row in self.program["targets"]:
            if row["target_kind"] != "bounded_rust_smoke":
                continue
            self.assertTrue(
                " --lib " in row["command"] or " --test " in row["command"],
                f"{row['id']} compiles unrelated test binaries: {row['command']}",
            )

    def test_crash_handoff_is_policy_only_and_p9_005_stays_open(self) -> None:
        handoff = self.program["crash_regression_handoff"]
        self.assertEqual("not_enforced", handoff["enforcement_status"])
        self.assertTrue(handoff["p9_005_row_remains_open"])

    def test_unknown_surface_and_target_fields_fail(self) -> None:
        corrupted = copy.deepcopy(self.program)
        corrupted["surfaces"][0]["invented"] = True
        corrupted["targets"][0]["invented"] = True
        failures = validate_fuzz_program_contract(ROOT, corrupted)
        self.assertTrue(any("additional" in item or "unexpected" in item for item in failures))

    def test_tier_and_surface_claim_drift_fail(self) -> None:
        corrupted = copy.deepcopy(self.program)
        corrupted["targets"][0]["primary_tier"] = "release"
        corrupted["targets"][0]["surface_associations"][0]["surface_id"] = "hmi_schema_payloads"
        failures = validate_fuzz_program_contract(ROOT, corrupted)
        self.assertTrue(any("tier" in item for item in failures), failures)
        self.assertTrue(any("live target contract" in item for item in failures), failures)

    def test_target_kind_union_fields_fail_closed(self) -> None:
        corrupted = copy.deepcopy(self.program)
        corrupted["targets"][0]["discovery_id"] = "DISC_00000000000000000000"
        corrupted["targets"][11]["corpus_path"] = "fuzz/corpus/invented"
        failures = validate_fuzz_program_contract(ROOT, corrupted)
        self.assertTrue(any("union contract" in item for item in failures), failures)

    def test_static_validator_pins_target_identity_before_live_scan(self) -> None:
        corrupted = copy.deepcopy(self.program)
        corrupted["targets"][0]["name"] = "invented"
        corrupted["targets"][0]["path"] = "fuzz/fuzz_targets/invented.rs"
        corrupted["targets"][0]["command"] = "cd fuzz && cargo fuzz run invented"
        failures = validate_fuzz_program_contract(ROOT, corrupted)
        self.assertTrue(any("identity fields drift" in item for item in failures), failures)

    def test_reviewed_titles_and_mapping_rationales_cannot_drift(self) -> None:
        corrupted = copy.deepcopy(self.program)
        corrupted["title"] = "Generic fuzz list"
        corrupted["surfaces"][0]["title"] = "All compiler input"
        corrupted["surfaces"][0]["rationale"] = "Anything parser-shaped is accepted."
        corrupted["targets"][1]["surface_associations"][1]["rationale"] = (
            "This target exercises the complete LSP protocol boundary and all edit shapes."
        )
        failures = validate_fuzz_program_contract(ROOT, corrupted)
        self.assertTrue(any("reviewed program title" in item for item in failures), failures)
        self.assertTrue(any("reviewed surface rows" in item for item in failures), failures)
        self.assertTrue(any("reviewed association rationale" in item for item in failures), failures)

    def test_hostile_registry_list_shapes_return_failures_not_exceptions(self) -> None:
        for field, value in (
            ("additional_tiers", 7),
            ("execution_basis_ids", None),
            ("surface_associations", {"surface_id": "st_lexer_parser"}),
        ):
            with self.subTest(field=field):
                corrupted = copy.deepcopy(self.program)
                corrupted["targets"][0][field] = value
                failures = validate_fuzz_program_contract(ROOT, corrupted)
                self.assertTrue(failures)

    def test_effective_ignore_negation_is_rejected(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            import subprocess

            subprocess.run(["git", "init", "-q", str(root)], check=True)
            for relative in ("fuzz/.gitignore", "crates/trust-ads-server/fuzz/.gitignore"):
                path = root / relative
                path.parent.mkdir(parents=True, exist_ok=True)
                path.write_text("artifacts/\ncorpus/\ncoverage/\ntarget/\n!corpus/\n")
            failures: list[str] = []
            _validate_corpus_storage(root, failures)
            self.assertTrue(any("exactly" in item for item in failures), failures)
            self.assertTrue(any("effectively ignore corpus/" in item for item in failures), failures)

    def test_removed_execution_fragment_fails_tier_binding(self) -> None:
        self.assertEqual(set(), _parse_salsa_run_targets('# run_target "syntax_parse"\n'))
        self.assertEqual(
            {"syntax_parse", "invented"},
            _parse_salsa_run_targets('run_target "syntax_parse"\nrun_target "invented"\n'),
        )
        self.assertEqual(
            set(),
            _parse_runtime_comms_commands(
                "# cargo test -p trust-runtime invented_fuzz_smoke_budget\n"
            ),
        )
        self.assertEqual(
            set(),
            _parse_runtime_comms_commands(
                "echo cargo test -p trust-runtime randomized_payload_smoke_budget -- --nocapture\n"
            ),
        )
        wrong = (
            'run_observed "runtime-comms-fuzz" "probe" "1" "probe.log" '
            "cargo test -p trust-syntax --lib randomized_payload_smoke_budget -- --nocapture\n"
        )
        self.assertEqual(1, len(_parse_runtime_comms_commands(wrong)))
        vm_summary_only = (
            'cat <<MD\n'
            'cargo test -p trust-runtime --test bytecode_vm_core '
            'vm_malformed_bytecode_fuzz_smoke_budget -- --nocapture\n'
            'MD\n'
        )
        self.assertEqual(set(), _parse_vm_fuzz_commands(vm_summary_only))
        failures: list[str] = []
        validate_execution_source_bindings(
            ROOT, self.program["targets"], tuple(row["id"] for row in self.program["targets"]), failures
        )
        self.assertEqual([], failures)

    def test_execution_claims_are_bound_to_reviewed_script_and_job_blocks(self) -> None:
        source_paths = (
            "scripts/salsa_fuzz_gate.sh",
            "scripts/runtime_comms_fuzz_gate.sh",
            "scripts/runtime_vm_malformed_bytecode_fuzz_gate.sh",
            ".github/workflows/salsa-hardening.yml",
            ".github/workflows/ci.yml",
            "Cargo.toml",
        )
        sources = {relative: (ROOT / relative).read_bytes() for relative in source_paths}
        failures: list[str] = []
        validate_reviewed_execution_source_digests(sources, failures)
        self.assertEqual([], failures)

        dead_script = dict(sources)
        path = "scripts/runtime_comms_fuzz_gate.sh"
        dead_script[path] = b"if false; then\n" + dead_script[path] + b"\nfi\n"
        failures = []
        validate_reviewed_execution_source_digests(dead_script, failures)
        self.assertTrue(any(path in item and "digest drifted" in item for item in failures), failures)

        echo_only = dict(sources)
        path = ".github/workflows/ci.yml"
        echo_only[path] = echo_only[path].replace(
            b"cargo test --all-targets",
            b"echo cargo test --all-targets",
            1,
        )
        failures = []
        validate_reviewed_execution_source_digests(echo_only, failures)
        self.assertTrue(any("ci.yml#test" in item and "digest drifted" in item for item in failures), failures)

        package_restricted = dict(sources)
        package_restricted[path] = package_restricted[path].replace(
            b"cargo test --all-targets",
            b"cargo test --all-targets -p trust-runtime",
            1,
        )
        failures = []
        validate_reviewed_execution_source_digests(package_restricted, failures)
        self.assertTrue(any("ci.yml#test" in item and "digest drifted" in item for item in failures), failures)

    def test_vm_execution_parser_ignores_summaries_echoes_and_wrong_commands(self) -> None:
        source = (ROOT / "scripts/runtime_vm_malformed_bytecode_fuzz_gate.sh").read_text()
        without_execution = re.sub(
            r"^python3 ./scripts/run_with_progress\.py \\\n(?:.*\\\n)*?.*vm_malformed_bytecode_fuzz_smoke_budget.*\n",
            "",
            source,
            count=1,
            flags=re.MULTILINE,
        )
        self.assertEqual(set(), _parse_vm_fuzz_commands(without_execution))
        self.assertEqual(
            set(),
            _parse_vm_fuzz_commands(
                "echo python3 ./scripts/run_with_progress.py -- env cargo test "
                "-p trust-runtime vm_malformed_bytecode_fuzz_smoke_budget\n"
                "true # python3 ./scripts/run_with_progress.py -- env cargo test "
                "-p trust-runtime vm_malformed_bytecode_fuzz_smoke_budget\n"
            ),
        )
        wrong_package = source.replace(
            "cargo test -p trust-runtime --test bytecode_vm_core",
            "cargo test -p trust-syntax --test wrong_binary",
            1,
        )
        self.assertNotEqual(REVIEWED_VM_FUZZ_COMMANDS, _parse_vm_fuzz_commands(wrong_package))
        extra = source + (
            "\npython3 ./scripts/run_with_progress.py -- env -u OUT_DIR cargo test "
            "-p trust-runtime --test bytecode_vm_core extra_smoke -- --nocapture\n"
        )
        self.assertEqual(2, len(_parse_vm_fuzz_commands(extra)))

    def test_runtime_comms_execution_parser_pins_package_and_exact_command_set(self) -> None:
        source = (ROOT / "scripts/runtime_comms_fuzz_gate.sh").read_text()
        self.assertEqual(
            REVIEWED_RUNTIME_COMMS_COMMANDS,
            _parse_runtime_comms_commands(source),
        )
        wrong_package = source.replace("cargo test -p trust-runtime", "cargo test -p trust-syntax", 1)
        self.assertNotEqual(
            REVIEWED_RUNTIME_COMMS_COMMANDS,
            _parse_runtime_comms_commands(wrong_package),
        )
        extra = source + (
            "\nrun_observed probe probe 1 probe.log cargo test -p trust-runtime "
            "extra_randomized_smoke -- --nocapture\n"
        )
        self.assertEqual(5, len(_parse_runtime_comms_commands(extra)))

    def test_full_validator_rejects_corrupted_program(self) -> None:
        corrupted = copy.deepcopy(self.program)
        corrupted["proof_posture"] = "coverage_proven"
        with mock.patch(
            "scripts.verification.metadata_validator.core.load_fuzz_program",
            return_value=corrupted,
        ):
            validator = Validator()
            validator.load_records()
            validator.validate()
        self.assertTrue(
            any("proof_posture" in failure.message for failure in validator.failures),
            [failure.message for failure in validator.failures],
        )

    def test_full_validator_rejects_corrupted_crash_registry(self) -> None:
        corrupted = {
            "schema_version": 1,
            "id": "FUZZ_CRASH_REGRESSIONS_V1",
            "status": "mapped",
            "required_disposition": "deterministic_regression",
            "regressions": [
                {
                    "target_id": "FUZZ_TARGET_SYNTAX_PARSE",
                    "artifact_sha256": "sha256:" + "d" * 64,
                    "test_id": "TEST_NOT_REGISTERED",
                    "rationale": "Invented mapping.",
                }
            ],
        }
        with mock.patch(
            "scripts.verification.metadata_validator.core.load_crash_registry",
            return_value=corrupted,
        ):
            validator = Validator()
            validator.load_records()
            validator.validate()
        self.assertTrue(
            any("TEST_NOT_REGISTERED" in failure.message for failure in validator.failures),
            [failure.message for failure in validator.failures],
        )


if __name__ == "__main__":
    unittest.main()
