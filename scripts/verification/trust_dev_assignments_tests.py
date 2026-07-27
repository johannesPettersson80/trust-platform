from __future__ import annotations

import copy
import hashlib
import json
import tempfile
import unittest
from pathlib import Path

from scripts.verification.mapping_assertions import (
    _assertion_id,
    validate_assertion_batch,
)
from scripts.verification.trust_dev_assignments import (
    EXPECTED_PATH_COUNTS,
    TEST_BINDINGS,
    generate_assignments,
    select_live_inventories,
)

EXPECTED_BINDING_SNAPSHOT = """
crates/trust-dev/src/agent.rs|decode_json_value_supports_typed_scalars|RT_HARNESS_BOUNDARY_IO_001|TYPED_JSON_SCALAR_CODEC
crates/trust-dev/src/agent.rs|encode_value_emits_typed_payload|RT_HARNESS_BOUNDARY_IO_001|TYPED_JSON_SCALAR_CODEC
crates/trust-dev/src/agent.rs|normalize_workspace_path_collapses_current_dir_segments|DEV_AGENT_WORKSPACE_PATH_001|RELATIVE_PATH_WITH_CURRENT_OR_IN_SCOPE_PARENT_COMPONENT
crates/trust-dev/src/agent.rs|normalize_workspace_path_rejects_parent_escape|DEV_AGENT_WORKSPACE_PATH_001|PARENT_COMPONENT_ESCAPES_LEXICAL_ROOT
crates/trust-dev/src/ci.rs|classify_build_failure_code|DEV_CI_EXIT_CLASSIFICATION_001|ONE_RECOGNIZED_FAILURE_CLASS
crates/trust-dev/src/ci.rs|classify_internal_code|DEV_CI_EXIT_CLASSIFICATION_001|ONE_RECOGNIZED_FAILURE_CLASS
crates/trust-dev/src/ci.rs|classify_invalid_config_code|DEV_CI_EXIT_CLASSIFICATION_001|ONE_RECOGNIZED_FAILURE_CLASS
crates/trust-dev/src/ci.rs|classify_test_failure_code|DEV_CI_EXIT_CLASSIFICATION_001|ONE_RECOGNIZED_FAILURE_CLASS
crates/trust-dev/src/ci.rs|classify_timeout_code|DEV_CI_EXIT_CLASSIFICATION_001|ONE_RECOGNIZED_FAILURE_CLASS
crates/trust-dev/src/ci.rs|classify_with_command_falls_back_for_internal|DEV_CI_EXIT_CLASSIFICATION_001|UNCLASSIFIED_MESSAGE_WITH_OPTIONAL_COMMAND
crates/trust-dev/src/commit.rs|commit_rejects_pre_staged_path_inside_project_without_mutation|DEV_COMMIT_SCOPE_001|PRESTAGED_SCOPE_COLLISION
crates/trust-dev/src/commit.rs|commit_scopes_commit_to_project_path_without_sweeping_pre_staged_files|DEV_COMMIT_SCOPE_001|PRESTAGED_OUTSIDE_SCOPE
crates/trust-dev/src/commit.rs|dry_run_with_pre_staged_collision_reports_without_mutation|DEV_COMMIT_SCOPE_001|PRESTAGED_SCOPE_COLLISION
crates/trust-dev/src/commit.rs|git_status_accepts_non_utf8_project_path_without_panic|DEV_COMMIT_SCOPE_001|STATUS_PATH_FIDELITY
crates/trust-dev/src/commit.rs|git_status_decodes_quoted_porcelain_paths_for_summary|DEV_COMMIT_SCOPE_001|STATUS_PATH_FIDELITY
crates/trust-dev/src/commit.rs|repository_root_commit_rejects_any_pre_staged_path|DEV_COMMIT_SCOPE_001|PRESTAGED_SCOPE_COLLISION
crates/trust-dev/src/docs/tests.rs|broken_tag_diagnostics_are_reported|DEV_DOCS_GENERATION_001|TAG_EXTRACTION_AND_DIAGNOSTICS
crates/trust-dev/src/docs/tests.rs|html_output_snapshot|DEV_DOCS_GENERATION_001|HTML_RENDER_FIXTURE
crates/trust-dev/src/docs/tests.rs|markdown_output_snapshot|DEV_DOCS_GENERATION_001|MARKDOWN_RENDER_FIXTURE
crates/trust-dev/src/docs/tests.rs|parser_extraction_for_tagged_comments|DEV_DOCS_GENERATION_001|TAG_EXTRACTION_AND_DIAGNOSTICS
crates/trust-dev/src/test_cmd/tests.rs|ci_mode_defaults_human_output_to_junit|DEV_TEST_REPORTING_001|CI_OUTPUT_SELECTION
crates/trust-dev/src/test_cmd/tests.rs|discovery_finds_test_pous_with_namespace_qualification|DEV_TEST_DISCOVERY_001|TEST_POU_DECLARATION_EXTRACTION
crates/trust-dev/src/test_cmd/tests.rs|discovery_ignores_comments_after_test_name|DEV_TEST_DISCOVERY_001|TEST_POU_DECLARATION_EXTRACTION
crates/trust-dev/src/test_cmd/tests.rs|execute_test_case_keeps_unconfigured_test_program_out_of_default_runtime|DEV_TEST_EXECUTION_001|CONFIGURATION_TEST_PROGRAM_BINDING
crates/trust-dev/src/test_cmd/tests.rs|execute_test_case_returns_execution_timeout_for_deadline_overrun|DEV_TEST_EXECUTION_001|CASE_DEADLINE
crates/trust-dev/src/test_cmd/tests.rs|execute_test_case_runs_test_program_when_session_registers_extra_program_instance|DEV_TEST_EXECUTION_001|CONFIGURATION_TEST_PROGRAM_BINDING
crates/trust-dev/src/test_cmd/tests.rs|execution_isolated_per_test_case|DEV_TEST_EXECUTION_001|COLD_CASE_ISOLATION
crates/trust-dev/src/test_cmd/tests.rs|execution_reports_assertion_failure_for_test_program|DEV_TEST_EXECUTION_001|PROGRAM_ASSERTION_RESULT
crates/trust-dev/src/test_cmd/tests.rs|execution_runs_test_function_block|DEV_TEST_EXECUTION_001|FUNCTION_BLOCK_BODY_EXECUTION
crates/trust-dev/src/test_cmd/tests.rs|human_output_filter_zero_message_is_clear|DEV_TEST_REPORTING_001|HUMAN_AND_LIST_REPORTS
crates/trust-dev/src/test_cmd/tests.rs|human_output_shows_failure_summary_with_source_context|DEV_TEST_REPORTING_001|HUMAN_AND_LIST_REPORTS
crates/trust-dev/src/test_cmd/tests.rs|json_output_contract|DEV_TEST_REPORTING_001|MACHINE_REPORT_FORMATS
crates/trust-dev/src/test_cmd/tests.rs|junit_output_contract|DEV_TEST_REPORTING_001|MACHINE_REPORT_FORMATS
crates/trust-dev/src/test_cmd/tests.rs|list_output_contract|DEV_TEST_REPORTING_001|HUMAN_AND_LIST_REPORTS
crates/trust-dev/src/test_cmd/tests.rs|load_sources_finds_mixed_case_extensions_under_literal_glob_chars|DEV_TEST_DISCOVERY_001|SUPPORTED_EXTENSION_CASES
crates/trust-dev/src/test_cmd/tests.rs|prepared_runtime_cold_restarts_between_cases|DEV_TEST_EXECUTION_001|COLD_CASE_ISOLATION
crates/trust-dev/src/test_cmd/tests.rs|run_test_executes_test_program_when_configuration_is_present|DEV_TEST_EXECUTION_001|CONFIGURATION_TEST_PROGRAM_BINDING
crates/trust-dev/src/test_cmd/tests.rs|tap_output_contract|DEV_TEST_REPORTING_001|MACHINE_REPORT_FORMATS
crates/trust-dev/src/test_cmd/tests.rs|timeout_message_pluralization|DEV_TEST_REPORTING_001|HUMAN_AND_LIST_REPORTS
""".strip()


class TrustDevAssignmentsTests(unittest.TestCase):
    def _fixture(self) -> tuple[tempfile.TemporaryDirectory, dict[str, object]]:
        temp = tempfile.TemporaryDirectory()
        root = Path(temp.name)
        inventories = {}
        reviews = []
        for index, ((path, name), (invariant, partition)) in enumerate(
            sorted(TEST_BINDINGS.items())
        ):
            source = root / path
            source.parent.mkdir(parents=True, exist_ok=True)
            if not source.exists():
                source.write_text(f"// fixture for {path}\n")
            expression = f'assert ! (observed_{index} , "{name}")'
            expression_digest = "sha256:" + hashlib.sha256(
                expression.encode()
            ).hexdigest()
            source_digest = "sha256:" + hashlib.sha256(
                source.read_bytes()
            ).hexdigest()
            discovery_id = f"DISC_{index:04d}"
            assertion = {
                "assertion_id": _assertion_id(
                    path=path,
                    name=name,
                    source_path=path,
                    function=name,
                    kind="macro:assert",
                    line=1,
                    expression_digest=expression_digest,
                ),
                "expression": expression,
                "expression_digest": expression_digest,
                "function": name,
                "kind": "macro:assert",
                "line": 1,
                "source_path": path,
            }
            inventories[discovery_id] = {
                "schema_version": 2,
                "path": path,
                "test_name": name,
                "source_digest": source_digest,
                "source_digests": {path: source_digest},
                "assertions": [assertion],
                "unresolved_assertion_helpers": [],
            }
            reviews.append(
                "[[reviews]]\n"
                f'discovery_id = "{discovery_id}"\n'
                f'path = "{path}"\n'
                f'name = "{name}"\n'
                'disposition = "reviewed_nonmapping"\n'
                'reason_code = "no_reviewed_spec_or_invariant_binding"\n'
            )
            invariant_path = (
                root
                / "verification"
                / "invariants"
                / f"{invariant}.toml"
            )
            if not invariant_path.exists():
                invariant_path.parent.mkdir(parents=True, exist_ok=True)
                invariant_path.write_text(f'id = "{invariant}"\n')
            text = invariant_path.read_text()
            marker = f'partition = {{ equals = "{partition}" }}'
            if marker not in text:
                invariant_path.write_text(
                    text + f'\n[[behavior]]\n{marker}\n'
                )
        document = {
            "schema_version": 2,
            "inventories": list(inventories.values()),
        }
        canonical = validate_assertion_batch(
            root,
            document,
            [
                {
                    "discovery_id": discovery_id,
                    "path": row["path"],
                    "name": row["test_name"],
                }
                for discovery_id, row in inventories.items()
            ],
        )
        (root / "verification/test-catalog-denominator.toml").write_text(
            "schema_version = 1\n" + "\n".join(reviews)
        )
        return temp, {"schema_version": 2, "inventories": canonical}

    @staticmethod
    def _oracle_snapshot(document: dict[str, object]) -> str:
        rows = []
        for row in document["inventories"].values():
            name = row["test_name"]
            for assertion in row["assertions"]:
                if (
                    assertion["function"].rsplit("::", 1)[-1] == name
                    and assertion["expression"].startswith("assert")
                ):
                    rows.append(
                        json.dumps(
                            (row["path"], name, assertion["expression"]),
                            ensure_ascii=False,
                            separators=(",", ":"),
                        )
                    )
        return "\n".join(sorted(rows))

    def test_exact_live_selection_and_assignments(self) -> None:
        temp, document = self._fixture()
        self.addCleanup(temp.cleanup)
        root = Path(temp.name)
        selected = select_live_inventories(
            root=root,
            inventory_document=document,
        )
        self.assertEqual(39, len(selected["inventories"]))
        self.assertEqual(
            dict(EXPECTED_PATH_COUNTS),
            {
                path: sum(
                    row["path"] == path
                    for row in selected["inventories"].values()
                )
                for path in EXPECTED_PATH_COUNTS
            },
        )
        assignments = generate_assignments(
            root=root,
            inventory_document=selected,
            reviewed_oracle_snapshot=self._oracle_snapshot(document),
        )
        self.assertEqual(39, len(assignments["assignments"]))
        self.assertTrue(
            all(
                len(row["assertion_assignments"]) == 1
                and row["setup_assertion_ids"] == []
                for row in assignments["assignments"]
            )
        )

    def test_denominator_drift_is_rejected(self) -> None:
        temp, document = self._fixture()
        self.addCleanup(temp.cleanup)
        root = Path(temp.name)
        denominator = root / "verification/test-catalog-denominator.toml"
        denominator.write_text(
            denominator.read_text().replace(
                'reason_code = "no_reviewed_spec_or_invariant_binding"',
                'reason_code = "other"',
                1,
            )
        )
        with self.assertRaisesRegex(ValueError, "does not equal reviewed bindings"):
            select_live_inventories(root=root, inventory_document=document)

    def test_stale_source_is_rejected(self) -> None:
        temp, document = self._fixture()
        self.addCleanup(temp.cleanup)
        root = Path(temp.name)
        first = next(iter(document["inventories"].values()))
        snapshot = self._oracle_snapshot(document)
        (root / first["path"]).write_text("// changed\n")
        with self.assertRaisesRegex(ValueError, "source digest is stale"):
            generate_assignments(
                root=root,
                inventory_document=document,
                reviewed_oracle_snapshot=snapshot,
            )

    def test_tampered_expression_is_rejected(self) -> None:
        temp, document = self._fixture()
        self.addCleanup(temp.cleanup)
        root = Path(temp.name)
        changed = copy.deepcopy(document)
        snapshot = self._oracle_snapshot(document)
        first = next(iter(changed["inventories"].values()))
        first["assertions"][0]["expression"] += " "
        with self.assertRaisesRegex(ValueError, "expression digest is stale"):
            generate_assignments(
                root=root,
                inventory_document=changed,
                reviewed_oracle_snapshot=snapshot,
            )

    def test_missing_partition_is_rejected(self) -> None:
        temp, document = self._fixture()
        self.addCleanup(temp.cleanup)
        root = Path(temp.name)
        snapshot = self._oracle_snapshot(document)
        invariant, partition = next(iter(TEST_BINDINGS.values()))
        path = next(
            (root / "verification/invariants").glob(f"**/{invariant}.toml")
        )
        path.write_text(
            path.read_text().replace(
                f'partition = {{ equals = "{partition}" }}',
                'partition = { equals = "REMOVED" }',
                1,
            )
        )
        with self.assertRaisesRegex(ValueError, "selects missing behavior"):
            generate_assignments(
                root=root,
                inventory_document=document,
                reviewed_oracle_snapshot=snapshot,
            )

    def test_setup_only_test_is_rejected(self) -> None:
        temp, document = self._fixture()
        self.addCleanup(temp.cleanup)
        root = Path(temp.name)
        changed = copy.deepcopy(document)
        snapshot = self._oracle_snapshot(document)
        first = next(iter(changed["inventories"].values()))
        first["assertions"][0]["function"] = "fixture_setup"
        expression = first["assertions"][0]["expression"]
        expression_digest = first["assertions"][0]["expression_digest"]
        first["assertions"][0]["assertion_id"] = _assertion_id(
            path=first["path"],
            name=first["test_name"],
            source_path=first["path"],
            function="fixture_setup",
            kind="macro:assert",
            line=1,
            expression_digest=expression_digest,
        )
        first.pop("inventory_digest", None)
        canonical = validate_assertion_batch(
            root,
            {"schema_version": 2, "inventories": [first]},
            [
                {
                    "discovery_id": next(iter(changed["inventories"])),
                    "path": first["path"],
                    "name": first["test_name"],
                }
            ],
        )
        first.update(next(iter(canonical.values())))
        with self.assertRaisesRegex(ValueError, "lacks a direct reviewed"):
            generate_assignments(
                root=root,
                inventory_document=changed,
                reviewed_oracle_snapshot=snapshot,
            )

    def test_fresh_unreviewed_oracle_expression_is_rejected(self) -> None:
        temp, document = self._fixture()
        self.addCleanup(temp.cleanup)
        root = Path(temp.name)
        snapshot = self._oracle_snapshot(document)
        changed = copy.deepcopy(document)
        discovery_id, first = next(iter(changed["inventories"].items()))
        assertion = first["assertions"][0]
        expression = 'assert ! (true , "unreviewed replacement")'
        expression_digest = "sha256:" + hashlib.sha256(
            expression.encode()
        ).hexdigest()
        assertion.update(
            {
                "expression": expression,
                "expression_digest": expression_digest,
                "assertion_id": _assertion_id(
                    path=first["path"],
                    name=first["test_name"],
                    source_path=first["path"],
                    function=first["test_name"],
                    kind="macro:assert",
                    line=1,
                    expression_digest=expression_digest,
                ),
            }
        )
        first.pop("inventory_digest", None)
        canonical = validate_assertion_batch(
            root,
            {"schema_version": 2, "inventories": [first]},
            [
                {
                    "discovery_id": discovery_id,
                    "path": first["path"],
                    "name": first["test_name"],
                }
            ],
        )
        first.update(next(iter(canonical.values())))
        with self.assertRaisesRegex(ValueError, "direct oracle expressions drifted"):
            generate_assignments(
                root=root,
                inventory_document=changed,
                reviewed_oracle_snapshot=snapshot,
            )

    def test_bindings_match_independent_literal_snapshot(self) -> None:
        actual = "\n".join(
            f"{path}|{name}|{invariant}|{partition}"
            for (path, name), (invariant, partition) in sorted(
                TEST_BINDINGS.items()
            )
        )
        self.assertEqual(EXPECTED_BINDING_SNAPSHOT, actual)


if __name__ == "__main__":
    unittest.main()
