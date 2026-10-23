"""Tests for the reviewed Phase 5 gate/workflow inventory contract."""

from __future__ import annotations

import copy
import json
import tempfile
import tomllib
import unittest
from pathlib import Path

from scripts.verification.gate_inventory import (
    INVENTORY_PATH,
    GateInventoryError,
    _validate_verification_workflow_source_contract,
    load_gate_inventory,
    validate_gate_inventory,
)
from scripts.verification.test_catalog_surfaces import scan_gate_scripts, scan_workflow_jobs


ROOT = Path(__file__).resolve().parents[2]


class GateInventoryTests(unittest.TestCase):
    def test_live_inventory_is_an_exhaustive_disjoint_partition(self) -> None:
        records = load_gate_inventory(ROOT)
        failures = validate_gate_inventory(ROOT, records)

        self.assertEqual(failures, [])
        live = [record for record in records.values() if "discovery_id" in record]
        templates = [record for record in records.values() if record["source_kind"] == "workflow_template"]
        gate_scripts = scan_gate_scripts(ROOT)
        workflow_jobs = scan_workflow_jobs(ROOT)
        self.assertEqual(len(live), len(gate_scripts.facts) + len(workflow_jobs.facts))
        self.assertEqual(
            sum(record["source_kind"] == "gate_script" for record in live),
            len(gate_scripts.facts),
        )
        self.assertEqual(
            sum(record["source_kind"] == "github_workflow_job" for record in live),
            len(workflow_jobs.facts),
        )
        self.assertEqual(len(templates), 1)
        recipes = [record for record in records.values() if record["source_kind"] == "just_recipe"]
        self.assertEqual([record["name"] for record in recipes], ["verification-veryquick"])
        catalog_commands = [
            record for record in records.values() if record["source_kind"] == "catalog_test_command"
        ]
        self.assertEqual(
            [record["name"] for record in catalog_commands],
            ["TEST_BYTECODE_VALIDATOR_MUTATION_SHARD_001"],
        )
        self.assertEqual(
            len(records),
            len(live) + len(templates) + len(recipes) + len(catalog_commands),
        )
        self.assertEqual(
            {record["discovery_id"] for record in live},
            live_discovery_ids(ROOT),
        )
    def test_missing_invented_and_duplicate_live_facts_fail_closed(self) -> None:
        with fixture_root() as root:
            records = fixture_records(root)
            gate_id = next(
                record_id
                for record_id, record in records.items()
                if record["source_kind"] == "gate_script"
            )

            missing = copy.deepcopy(records)
            missing.pop(gate_id)
            self.assertFailure(missing, root, "missing inventory record for live fact")

            invented = copy.deepcopy(records)
            invented["GATE_INVENTED"] = {
                **invented[next(iter(invented))],
                "id": "GATE_INVENTED",
                "discovery_id": "DISC_00000000000000000000",
            }
            self.assertFailure(invented, root, "unknown live discovery_id")

            duplicate = copy.deepcopy(records)
            duplicate["GATE_DUPLICATE"] = {
                **duplicate[gate_id],
                "id": "GATE_DUPLICATE",
            }
            self.assertFailure(duplicate, root, "is mapped by multiple inventory records")

    def test_removed_gate_script_leaves_stale_inventory_record(self) -> None:
        with fixture_root() as root:
            records = fixture_records(root)
            gate_record = next(
                record
                for record in records.values()
                if record["source_kind"] == "gate_script"
            )
            (root / gate_record["path"]).unlink()

            failures = validate_gate_inventory(root, records)

        self.assertTrue(
            any("unknown live discovery_id" in failure for failure in failures),
            failures,
        )

    def test_live_identity_and_command_drift_fail_closed(self) -> None:
        with fixture_root() as root:
            baseline = fixture_records(root)
            record_id = next(
                item_id
                for item_id, item in baseline.items()
                if item["source_kind"] == "github_workflow_job"
            )
            changes = {
                "path": ".github/workflows/renamed.yml",
                "name": "Fixture CI / renamed",
                "source_kind": "gate_script",
                "command": "workflow job .github/workflows/fixture.yml#renamed",
            }
            for field, value in changes.items():
                with self.subTest(field=field):
                    records = copy.deepcopy(baseline)
                    records[record_id][field] = value
                    self.assertFailure(records, root, f"{field} binding mismatch")

    def test_disposition_and_enforcement_coupling_fails_closed(self) -> None:
        with fixture_root() as root:
            baseline = fixture_records(root)
            assigned_id = next(
                item_id
                for item_id, item in baseline.items()
                if item["disposition"] == "assigned"
            )
            cases = (
                ("assigned suites", {"suite_ids": []}, "assigned requires at least one suite_id"),
                (
                    "assigned enforcement",
                    {"enforcement": "report_only"},
                    "assigned forbids enforcement = report_only",
                ),
                (
                    "report-only enforcement",
                    {"disposition": "report_only", "enforcement": "required"},
                    "report_only requires enforcement = report_only",
                ),
                (
                    "supporting suite",
                    {
                        "disposition": "supporting",
                        "enforcement": "supporting",
                        "suite_ids": ["pr"],
                    },
                    "supporting requires suite_ids = ['supporting_local']",
                ),
                (
                    "excluded suite",
                    {
                        "disposition": "excluded",
                        "enforcement": "excluded",
                        "suite_ids": ["nightly"],
                    },
                    "excluded requires empty suite_ids",
                ),
            )
            for label, updates, expected in cases:
                with self.subTest(label=label):
                    records = copy.deepcopy(baseline)
                    records[assigned_id].update(updates)
                    self.assertFailure(records, root, expected)

    def test_artifact_kind_paths_and_retention_are_coupled(self) -> None:
        with fixture_root() as root:
            baseline = fixture_records(root)
            record_id = next(
                item_id
                for item_id, item in baseline.items()
                if item["source_kind"] == "gate_script"
            )
            cases = (
                (
                    {"artifact_kind": "none", "artifact_paths": ["target/out"]},
                    "artifact_kind = none requires empty artifact_paths",
                ),
                (
                    {
                        "artifact_kind": "ci_artifact",
                        "artifact_paths": ["gate-output"],
                        "artifact_retention": "none",
                    },
                    "ci_artifact requires artifact_retention = repository_default",
                ),
                (
                    {
                        "artifact_kind": "machine_local",
                        "artifact_paths": [],
                        "artifact_retention": "machine_local",
                    },
                    "machine_local requires non-empty artifact_paths",
                ),
            )
            for updates, expected in cases:
                with self.subTest(updates=updates):
                    records = copy.deepcopy(baseline)
                    records[record_id].update(updates)
                    self.assertFailure(records, root, expected)

    def test_hardware_lab_assignment_requires_strict_opt_in(self) -> None:
        with fixture_root() as root:
            records = fixture_records(root)
            record_id = next(
                item_id
                for item_id, item in records.items()
                if item["source_kind"] == "gate_script"
            )
            records[record_id].update(
                {
                    "suite_ids": ["hardware_lab"],
                    "environment": "github_or_lab_runner",
                    "required_env": [],
                }
            )

            self.assertFailure(
                records,
                root,
                "hardware_lab requires TRUST_DIT_REQUIRE_HARDWARE=1 in required_env",
            )

    def test_static_hardware_opt_in_is_reserved_to_the_strict_hardware_entrypoint(self) -> None:
        with fixture_root() as root:
            baseline = fixture_records(root)
            helper_id = next(
                item_id
                for item_id, item in baseline.items()
                if item["source_kind"] == "gate_script"
            )
            cases = (
                (
                    helper_id,
                    {
                        "command_role": "helper",
                        "required_env": ["TRUST_DIT_REQUIRE_HARDWARE=1"],
                    },
                ),
                (
                    "GATE_FIXTURE_TEMPLATE",
                    {"required_env": ["TRUST_DIT_REQUIRE_HARDWARE=1"]},
                ),
            )

            for record_id, updates in cases:
                with self.subTest(record_id=record_id):
                    records = copy.deepcopy(baseline)
                    records[record_id].update(updates)
                    self.assertFailure(
                        records,
                        root,
                        "TRUST_DIT_REQUIRE_HARDWARE=1 is reserved to the exclusive "
                        "hardware_lab entrypoint",
                    )

    def test_nested_template_is_only_a_non_executable_exclusion(self) -> None:
        with fixture_root() as root:
            baseline = fixture_records(root)
            template_id = next(
                item_id
                for item_id, item in baseline.items()
                if item["source_kind"] == "workflow_template"
            )
            cases = (
                ({"disposition": "assigned", "suite_ids": ["pr"]}, "workflow_template must be excluded"),
                ({"enforcement": "required"}, "workflow_template must be non_executable"),
                ({"discovery_id": "DISC_00000000000000000000"}, "workflow_template forbids discovery_id"),
                ({"name": "Fixture Template / renamed"}, "template name binding mismatch"),
            )
            for updates, expected in cases:
                with self.subTest(updates=updates):
                    records = copy.deepcopy(baseline)
                    records[template_id].update(updates)
                    self.assertFailure(records, root, expected)

    def test_just_recipe_binding_fails_when_recipe_is_missing_or_renamed(self) -> None:
        with fixture_root() as root:
            records = fixture_records(root)
            recipe = just_recipe_record()
            records[recipe["id"]] = recipe
            self.assertEqual(validate_gate_inventory(root, records), [])

            (root / "justfile").write_text("other-recipe:\n\ttrue\n")

            self.assertFailure(records, root, "just recipe verification-veryquick is absent")

    def test_just_recipe_binding_pins_the_reviewed_command_sequence(self) -> None:
        with fixture_root() as root:
            records = fixture_records(root)
            recipe = just_recipe_record()
            records[recipe["id"]] = recipe
            self.assertEqual(validate_gate_inventory(root, records), [])

            text = (root / "justfile").read_text()
            (root / "justfile").write_text(
                text.replace("\tjust test-fast\n", "\ttrue\n")
            )

            self.assertFailure(records, root, "reviewed command sequence")

    def test_workflow_artifact_claims_are_bound_to_the_owning_job(self) -> None:
        with fixture_root() as root:
            records = fixture_records(root)
            workflow_id = next(
                item_id
                for item_id, item in records.items()
                if item["source_kind"] == "github_workflow_job"
            )
            records[workflow_id].update(
                artifact_kind="ci_artifact",
                artifact_paths=["fixture-artifact"],
                artifact_retention="repository_default",
            )
            self.assertEqual(validate_gate_inventory(root, records), [])

            records[workflow_id]["artifact_paths"] = ["invented-artifact"]

            self.assertFailure(records, root, "artifact claim is absent from workflow job source")

    def test_ci_job_result_locator_is_derived_from_the_live_workflow_identity(self) -> None:
        records = load_gate_inventory(ROOT)
        records["GATE_JOB_RELEASE_PREFLIGHT"]["artifact_paths"] = [
            "completely-invented-ci-result"
        ]

        self.assertFailure(
            records,
            ROOT,
            "CI job result locator must equal the live workflow identity",
        )

    def test_strict_hardware_artifact_path_is_bound_to_the_script_default(self) -> None:
        records = load_gate_inventory(ROOT)
        records["GATE_SCRIPT_RUNTIME_DEVICE_IN_LOOP"]["artifact_paths"] = [
            "totally/invented/artifact"
        ]

        self.assertFailure(
            records,
            ROOT,
            "strict hardware artifact_paths must equal the reviewed script default",
        )

    def test_verification_workflow_requires_strict_mode_and_read_only_permissions(self) -> None:
        record = load_gate_inventory(ROOT)["GATE_JOB_VERIFICATION_REPORT"]
        baseline = (ROOT / record["path"]).read_text()
        enforcing = baseline
        failures: list[str] = []
        _validate_verification_workflow_source_contract(
            ROOT,
            {"GATE_JOB_VERIFICATION_REPORT": record},
            failures,
            workflow_text=enforcing,
        )
        self.assertEqual(failures, [])

        self.assertIn("schedule:", baseline)
        self.assertIn("--smoke", baseline)
        self.assertIn("scripts/check_verification_tooling_selftests.py", baseline)
        self.assertIn("github.event_name == 'pull_request'", baseline)
        self.assertIn("github.event_name != 'pull_request'", baseline)
        self.assertEqual(baseline.count("python3 scripts/verification_report_gate.py"), 2)

        cases = (
            (
                enforcing.replace("            --strict \\\n", ""),
                "must pass --strict",
            ),
            (
                enforcing.replace(
                    "permissions:\n  contents: read",
                    "permissions:\n  contents: write",
                ),
                "read-only permissions",
            ),
        )
        for text, expected in cases:
            with self.subTest(expected=expected), tempfile.TemporaryDirectory() as temp:
                root = Path(temp)
                failures: list[str] = []

                _validate_verification_workflow_source_contract(
                    root,
                    {"GATE_JOB_VERIFICATION_REPORT": record},
                    failures,
                    workflow_text=text,
                )

                self.assertTrue(any(expected in failure for failure in failures), failures)

    def test_catalog_command_binding_is_exact_and_unique(self) -> None:
        with fixture_root() as root:
            records = fixture_records(root)
            record = catalog_command_record()
            records[record["id"]] = record
            self.assertEqual(validate_gate_inventory(root, records), [])

            records[record["id"]]["command"] = "python3 scripts/invented.py"

            self.assertFailure(records, root, "catalog command binding mismatch")

    def test_failure_callback_receives_every_failure(self) -> None:
        with fixture_root() as root:
            records = fixture_records(root)
            records.pop(next(iter(records)))
            seen: list[tuple[Path, str]] = []

            failures = validate_gate_inventory(
                root,
                records,
                on_failure=lambda path, message: seen.append((path, message)),
            )

        self.assertEqual([message for _, message in seen], failures)
        self.assertTrue(all(path == root / INVENTORY_PATH for path, _ in seen))

    def test_loader_rejects_duplicate_record_ids_before_indexing(self) -> None:
        with tempfile.TemporaryDirectory() as temp:
            root = Path(temp)
            path = root / INVENTORY_PATH
            path.parent.mkdir(parents=True)
            path.write_text(
                "schema_version = 1\n"
                "[[surfaces]]\nid = 'GATE_DUP'\n"
                "[[surfaces]]\nid = 'GATE_DUP'\n"
            )

            with self.assertRaisesRegex(GateInventoryError, "duplicate gate inventory id GATE_DUP"):
                load_gate_inventory(root)

    def test_loader_rejects_unknown_top_level_fields(self) -> None:
        with tempfile.TemporaryDirectory() as temp:
            root = Path(temp)
            path = root / INVENTORY_PATH
            path.parent.mkdir(parents=True)
            path.write_text(
                "schema_version = 1\n"
                "unexpected = 'accepted'\n"
                "surfaces = []\n"
            )

            with self.assertRaisesRegex(GateInventoryError, "top-level fields drift"):
                load_gate_inventory(root)

    def test_schema_is_closed_and_pins_the_contract_fields(self) -> None:
        schema = json.loads((ROOT / "verification/schemas/gate-inventory.schema.json").read_text())
        row = schema["properties"]["surfaces"]["items"]

        self.assertFalse(schema["additionalProperties"])
        self.assertFalse(row["additionalProperties"])
        self.assertEqual(
            set(row["required"]),
            {
                "schema_version",
                "id",
                "source_kind",
                "path",
                "name",
                "command",
                "variant",
                "command_role",
                "disposition",
                "suite_ids",
                "owner",
                "duration_class",
                "environment",
                "artifact_kind",
                "artifact_paths",
                "artifact_retention",
                "enforcement",
                "required_env",
                "rationale",
            },
        )

    def assertFailure(self, records: dict[str, dict], root: Path, expected: str) -> None:
        failures = validate_gate_inventory(root, records)
        self.assertTrue(any(expected in failure for failure in failures), failures)


def live_discovery_ids(root: Path) -> set[str]:
    return {
        fact.stable_id
        for batch in (scan_gate_scripts(root), scan_workflow_jobs(root))
        for fact in batch.facts
    }


def fixture_records(root: Path) -> dict[str, dict]:
    facts = [
        fact
        for batch in (scan_gate_scripts(root), scan_workflow_jobs(root))
        for fact in batch.facts
    ]
    records: dict[str, dict] = {}
    for index, fact in enumerate(facts, start=1):
        record_id = f"GATE_FIXTURE_{index:03d}"
        records[record_id] = {
            "schema_version": 1,
            "id": record_id,
            "discovery_id": fact.stable_id,
            "source_kind": fact.source_kind,
            "path": fact.path,
            "name": fact.name,
            "command": fact.command_hint,
            "variant": "fixture",
            "command_role": "entrypoint",
            "disposition": "assigned",
            "suite_ids": ["pr"],
            "owner": "verification",
            "duration_class": "fast",
            "environment": "github_ubuntu",
            "artifact_kind": "none",
            "artifact_paths": [],
            "artifact_retention": "none",
            "enforcement": "required",
            "required_env": [],
            "rationale": "Fixture live binding.",
        }
    records["GATE_FIXTURE_TEMPLATE"] = {
        "schema_version": 1,
        "id": "GATE_FIXTURE_TEMPLATE",
        "source_kind": "workflow_template",
        "path": ".github/workflows/templates/fixture.yml",
        "name": "Fixture Template / plc-ci",
        "command": "non-executable workflow template .github/workflows/templates/fixture.yml#plc-ci",
        "variant": "consumer_template",
        "command_role": "reference",
        "disposition": "excluded",
        "suite_ids": [],
        "owner": "verification",
        "duration_class": "manual",
        "environment": "consumer_project_ci",
        "artifact_kind": "none",
        "artifact_paths": [],
        "artifact_retention": "none",
        "enforcement": "non_executable",
        "required_env": [],
        "rationale": "Nested workflow templates are not executed by this repository.",
    }
    return records


def just_recipe_record() -> dict:
    return {
        "schema_version": 1,
        "id": "GATE_JUST_VERIFICATION_VERYQUICK",
        "source_kind": "just_recipe",
        "path": "justfile",
        "name": "verification-veryquick",
        "command": "just verification-veryquick",
        "variant": "builder-only bounded verification feedback",
        "command_role": "entrypoint",
        "disposition": "assigned",
        "suite_ids": ["veryquick"],
        "owner": "verification",
        "duration_class": "fast",
        "environment": "trust_builder",
        "artifact_kind": "machine_local",
        "artifact_paths": ["target/gate-artifacts/veryquick/"],
        "artifact_retention": "machine_local",
        "enforcement": "planned",
        "required_env": [],
        "rationale": "Maps the final veryquick suite to one bounded trust-builder recipe.",
    }


def catalog_command_record() -> dict:
    return {
        "schema_version": 1,
        "id": "GATE_MUTATION_BYTECODE_VALIDATOR",
        "source_kind": "catalog_test_command",
        "path": "scripts/bytecode_validator_mutation.py",
        "name": "TEST_BYTECODE_VALIDATOR_MUTATION_SHARD_001",
        "command": "python3 scripts/bytecode_validator_mutation.py",
        "variant": "catalog-bound selected mutation shard",
        "command_role": "entrypoint",
        "disposition": "assigned",
        "suite_ids": ["nightly"],
        "owner": "trust-runtime",
        "duration_class": "slow",
        "environment": "trust_builder",
        "artifact_kind": "machine_local",
        "artifact_paths": ["target/gate-artifacts/verification/bytecode-validator-mutation.json"],
        "artifact_retention": "machine_local",
        "enforcement": "planned",
        "required_env": [],
        "rationale": "Fixture catalog command.",
    }


class fixture_root:
    def __init__(self) -> None:
        self._temp = tempfile.TemporaryDirectory()
        self.root = Path(self._temp.name)

    def __enter__(self) -> Path:
        write(self.root / "justfile", reviewed_veryquick_recipe())
        write(self.root / "scripts/sample_gate.sh", "#!/usr/bin/env bash\nexit 0\n")
        write(self.root / "scripts/bytecode_validator_mutation.py", "raise SystemExit(0)\n")
        write(
            self.root / "verification/test-catalog.toml",
            "schema_version = 2\n"
            "[[tests]]\n"
            "id = 'TEST_BYTECODE_VALIDATOR_MUTATION_SHARD_001'\n"
            "path = 'scripts/bytecode_validator_mutation.py'\n"
            "command = 'python3 scripts/bytecode_validator_mutation.py'\n",
        )
        write(
            self.root / ".github/workflows/fixture.yml",
            "name: Fixture CI\njobs:\n  checks:\n    runs-on: ubuntu-latest\n"
            "    steps:\n      - uses: actions/upload-artifact@v7\n"
            "        with:\n          name: fixture-artifact\n          path: fixture.txt\n",
        )
        write(
            self.root / ".github/workflows/templates/fixture.yml",
            "name: Fixture Template\njobs:\n  plc-ci:\n    runs-on: ubuntu-latest\n    steps: []\n",
        )
        return self.root

    def __exit__(self, exc_type, exc, tb) -> None:
        self._temp.cleanup()


def write(path: Path, content: str) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_text(content)


def reviewed_veryquick_recipe() -> str:
    return (
        "verification-veryquick:\n"
        "\tmkdir -p target/gate-artifacts/veryquick\n"
        "\tpython3 -m unittest scripts.verification.report_gate_tests "
        "scripts.verification.focused_test_suite_tests || "
        "echo \"advisory: verification report smoke reported findings\" >&2\n"
        "\tscripts/verification_metadata_gate.sh || "
        "echo \"advisory: verification metadata reported findings\" >&2\n"
        "\tjust test-hir-fast\n"
        "\tjust test-fast\n"
        "\t./scripts/cargo_test_fast_link.sh test -p trust-syntax --lib\n"
        "\t./scripts/cargo_test_fast_link.sh test -p trust-runtime-core --lib\n"
        "\t./scripts/cargo_test_fast_link.sh test -p trust-runtime --test bytecode_validation\n"
        "\tcargo run -p trust-runtime --bin trust-runtime -- conformance --suite-root conformance "
        "--filter cfm_arithmetic_conversion_compare_001 --output "
        "target/gate-artifacts/veryquick/conformance.json\n"
    )


if __name__ == "__main__":
    unittest.main()
