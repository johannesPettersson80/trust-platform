"""Tests for reviewed test-refactor proposals and catalog redirects."""

from __future__ import annotations

import copy
import hashlib
import json
import subprocess
import tempfile
import unittest
from pathlib import Path

from scripts.verification.test_catalog_common import make_fact
from scripts.verification.test_refactor_contract import (
    PROPOSAL_SCHEMA_PATH,
    REDIRECT_SCHEMA_PATH,
    validate_test_refactor_records,
)


TEST_ID = "TEST_BYTECODE_CONTAINER_INVALID_MAGIC"
PROPOSAL_ID = "TEST_REFACTOR_BYTECODE_CONTAINER_INVALID_MAGIC_001"
COMMAND = "cargo test -p trust-runtime --test bytecode_container header_validation -- --exact"
ASSESSMENT_PATH = (
    "docs/internal/testing/evidence/plc-verification-program/2026-07-10/"
    "p2a-test-refactor-assessment.md"
)


class TestRefactorContractTests(unittest.TestCase):
    def test_no_refactor_needed_pilot_is_a_reviewed_terminal_decision(self) -> None:
        with contract_root() as root:
            fact = generated_fact()
            proposal = no_refactor_proposal(fact)
            failures = validate_test_refactor_records(
                root=root,
                proposals={PROPOSAL_ID: proposal},
                redirects={},
                tests={TEST_ID: catalog_record(fact)},
                evidence={},
                facts=[fact],
                assessment=assessment_payload(),
            )

        self.assertEqual(failures, [])

    def test_no_refactor_needed_requires_unchanged_identity_commands_and_empty_updates(self) -> None:
        with contract_root() as root:
            fact = generated_fact()
            base = no_refactor_proposal(fact)
            corruptions = (
                ("target identity", lambda item: item.__setitem__("target_identity", old_identity())),
                ("before and after commands", lambda item: item.__setitem__("after_command", "other")),
                ("stale_path_updates", lambda item: item.__setitem__("stale_path_updates", ["verification/test-catalog.toml"])),
                ("behavior-lock evidence", lambda item: item.__setitem__("before_behavior_lock_evidence", "EVID_LOCK")),
            )
            for expected, mutate in corruptions:
                with self.subTest(expected=expected):
                    proposal = copy.deepcopy(base)
                    mutate(proposal)
                    failures = validate_test_refactor_records(
                        root=root,
                        proposals={PROPOSAL_ID: proposal},
                        redirects={},
                        tests={TEST_ID: catalog_record(fact)},
                        evidence={},
                        facts=[fact],
                        assessment=assessment_payload(),
                    )
                    self.assertTrue(any(expected in failure for failure in failures), failures)

    def test_proposal_plan_fields_are_closed_and_fail_empty(self) -> None:
        with contract_root() as root:
            fact = generated_fact()
            proposal = no_refactor_proposal(fact)
            proposal["invented_field"] = "bypass"
            proposal["invariant_ids"] = []
            proposal["coverage_dimensions"] = []
            proposal["source_paths"] = []
            proposal["rationale"] = ""
            failures = validate_test_refactor_records(
                root=root,
                proposals={PROPOSAL_ID: proposal},
                redirects={},
                tests={TEST_ID: catalog_record(fact)},
                evidence={},
                facts=[fact],
            )

        self.assertTrue(any("additional field invented_field" in item for item in failures))
        self.assertTrue(any("invariant_ids" in item for item in failures))
        self.assertTrue(any("coverage_dimensions" in item for item in failures))
        self.assertTrue(any("source_paths" in item for item in failures))
        self.assertTrue(any("rationale" in item for item in failures))

    def test_proposal_binds_catalog_invariants_and_current_source(self) -> None:
        with contract_root() as root:
            fact = generated_fact()
            proposal = no_refactor_proposal(fact)
            proposal["invariant_ids"] = ["VM_UNKNOWN"]
            proposal["source_paths"] = ["crates/trust-runtime/tests/other.rs"]
            failures = validate_test_refactor_records(
                root=root,
                proposals={PROPOSAL_ID: proposal},
                redirects={},
                tests={TEST_ID: catalog_record(fact)},
                evidence={},
                facts=[fact],
            )

        self.assertTrue(any("invariant_ids do not match" in item for item in failures))
        self.assertTrue(any("source_paths must include catalog path" in item for item in failures))

    def test_proposal_dimensions_must_come_from_explicit_catalog_bindings(self) -> None:
        with contract_root() as root:
            fact = generated_fact()
            proposal = no_refactor_proposal(fact)
            proposal["coverage_dimensions"] = ["malformed_input_class:invented"]
            failures = validate_test_refactor_records(
                root=root,
                proposals={PROPOSAL_ID: proposal},
                redirects={},
                tests={TEST_ID: catalog_record(fact)},
                evidence={},
                facts=[fact],
            )

        self.assertTrue(any("coverage_dimensions do not match" in item for item in failures))

    def test_no_refactor_can_honestly_have_no_catalog_dimensions(self) -> None:
        with contract_root() as root:
            fact = generated_fact()
            proposal = no_refactor_proposal(fact)
            proposal["coverage_dimensions"] = []
            catalog = catalog_record(fact)
            catalog.pop("malformed_input_class_ids")
            failures = validate_test_refactor_records(
                root=root,
                proposals={PROPOSAL_ID: proposal},
                redirects={},
                tests={TEST_ID: catalog},
                evidence={},
                facts=[fact],
                assessment=assessment_payload(),
            )

        self.assertEqual(failures, [])

    def test_split_is_blocked_until_a_multi_target_contract_exists(self) -> None:
        with contract_root() as root:
            old = old_fact()
            new = generated_fact()
            proposal = refactor_proposal(old, new)
            proposal["disposition"] = "split"
            failures = validate_test_refactor_records(
                root=root,
                proposals={PROPOSAL_ID: proposal},
                redirects={"TEST_REDIRECT_001": redirect_record(old, new)},
                tests={TEST_ID: catalog_record(new)},
                evidence=lock_evidence(root),
                facts=[new],
            )

        self.assertTrue(any("multi-target contract" in item for item in failures))

    def test_lifecycle_is_canonical_and_cannot_skip_completion(self) -> None:
        with contract_root() as root:
            old = old_fact()
            new = generated_fact()
            proposal = refactor_proposal(old, new)
            proposal["lifecycle"] = ["proposed", "approved", "validated"]
            failures = validate_test_refactor_records(
                root=root,
                proposals={PROPOSAL_ID: proposal},
                redirects={},
                tests={TEST_ID: catalog_record(new)},
                evidence=lock_evidence(root),
                facts=[new],
            )

        self.assertTrue(any("lifecycle must equal" in item for item in failures))

    def test_completed_refactor_requires_a_paired_passing_behavior_lock(self) -> None:
        with contract_root() as root:
            old = old_fact()
            new = generated_fact()
            proposal = refactor_proposal(old, new)
            evidence = lock_evidence(root)
            evidence["EVID_AFTER"]["paired_lock_baseline"] = "EVID_OTHER"
            evidence["EVID_AFTER"]["case_result_digest"] = "sha256:" + "2" * 64
            evidence["EVID_AFTER"]["command_exit_status"] = 1
            failures = validate_test_refactor_records(
                root=root,
                proposals={PROPOSAL_ID: proposal},
                redirects={},
                tests={TEST_ID: catalog_record(new)},
                evidence=evidence,
                facts=[new],
            )

        self.assertTrue(any("does not pair to before evidence" in item for item in failures))
        self.assertTrue(any("result digest does not match" in item for item in failures))
        self.assertTrue(any("command_exit_status must be 0" in item for item in failures))

    def test_completed_refactor_requires_catalog_case_binding_for_lock_proof(self) -> None:
        with contract_root() as root:
            old = old_fact()
            new = generated_fact()
            catalog = catalog_record(new)
            catalog.pop("case_file")
            catalog.pop("case_file_digest")
            failures = validate_test_refactor_records(
                root=root,
                proposals={PROPOSAL_ID: refactor_proposal(old, new)},
                redirects={"TEST_REDIRECT_001": redirect_record(old, new)},
                tests={TEST_ID: catalog},
                evidence=lock_evidence(root),
                facts=[new],
            )

        self.assertTrue(any("requires catalog case_file and case_file_digest" in item for item in failures))

    def test_command_changing_refactor_is_blocked_by_current_lock_model(self) -> None:
        with contract_root() as root:
            old = old_fact()
            new = generated_fact()
            after_command = COMMAND + " --nocapture"
            proposal = refactor_proposal(old, new)
            proposal["after_command"] = after_command
            failures = validate_test_refactor_records(
                root=root,
                proposals={PROPOSAL_ID: proposal},
                redirects={"TEST_REDIRECT_001": redirect_record(old, new)},
                tests={TEST_ID: catalog_record(new, command=after_command)},
                evidence=lock_evidence(root, after_command=after_command),
                facts=[new],
            )

        self.assertTrue(any("current lock proof requires identical commands" in item for item in failures))

    def test_committed_completed_status_is_rejected_as_transient(self) -> None:
        with contract_root() as root:
            old = old_fact()
            new = generated_fact()
            proposal = refactor_proposal(old, new)
            proposal["status"] = "completed"
            proposal["lifecycle"] = ["proposed", "approved", "in_progress", "completed"]
            failures = validate_test_refactor_records(
                root=root,
                proposals={PROPOSAL_ID: proposal},
                redirects={},
                tests={TEST_ID: catalog_record(new)},
                evidence=lock_evidence(root),
                facts=[old, new],
            )

        self.assertTrue(any("completed is transient" in item for item in failures))

    def test_behavior_lock_revisions_must_be_distinct_and_ordered(self) -> None:
        with contract_root() as root:
            old = old_fact()
            new = generated_fact()
            failures = validate_test_refactor_records(
                root=root,
                proposals={PROPOSAL_ID: refactor_proposal(old, new)},
                redirects={"TEST_REDIRECT_001": redirect_record(old, new)},
                tests={TEST_ID: catalog_record(new)},
                evidence=lock_evidence(root, same_revision=True),
                facts=[new],
            )

        self.assertTrue(any("distinct source revisions" in item for item in failures))

    def test_behavior_lock_binds_exact_cases_invariants_and_run_ids(self) -> None:
        with contract_root() as root:
            old = old_fact()
            new = generated_fact()
            evidence = lock_evidence(root)
            evidence["EVID_BEFORE"]["per_case_summary"] = ["INVENTED:passed"]
            evidence["EVID_BEFORE"]["linked_invariants"] = ["VM_OTHER"]
            evidence["EVID_BEFORE"].pop("trust_verify_run_id")
            failures = validate_test_refactor_records(
                root=root,
                proposals={PROPOSAL_ID: refactor_proposal(old, new)},
                redirects={"TEST_REDIRECT_001": redirect_record(old, new)},
                tests={TEST_ID: catalog_record(new)},
                evidence=evidence,
                facts=[new],
            )

        self.assertTrue(any("proposal invariants" in item for item in failures))
        self.assertTrue(any("requires a run ID" in item for item in failures))
        self.assertTrue(any("case IDs do not match" in item for item in failures))

    def test_unsupported_change_assessment_can_only_remain_proposed(self) -> None:
        with contract_root() as root:
            old = old_fact()
            new = generated_fact()
            proposal = refactor_proposal(old, new)
            proposal["status"] = "proposed"
            proposal["lifecycle"] = ["proposed"]
            proposal.pop("before_behavior_lock_evidence")
            proposal.pop("after_behavior_lock_evidence")
            assessment = {
                "proposal_evaluations": [
                    {
                        "proposal_id": PROPOSAL_ID,
                        "disposition": "rename",
                        "source_paths": proposal["source_paths"],
                        "observed_signals": proposal["finding_refs"],
                        "supported": False,
                    }
                ]
            }
            failures = validate_test_refactor_records(
                root=root,
                proposals={PROPOSAL_ID: proposal},
                redirects={},
                tests={TEST_ID: catalog_record(old)},
                evidence={},
                facts=[old],
                assessment=assessment,
            )

        self.assertEqual(failures, [])

    def test_proposed_target_discovery_id_is_derived_from_source_identity(self) -> None:
        with contract_root() as root:
            old = old_fact()
            new = generated_fact()
            proposal = refactor_proposal(old, new)
            proposal["status"] = "proposed"
            proposal["lifecycle"] = ["proposed"]
            proposal.pop("before_behavior_lock_evidence")
            proposal.pop("after_behavior_lock_evidence")
            proposal["target_identity"]["discovery_id"] = "DISC_00000000000000000000"
            failures = validate_test_refactor_records(
                root=root,
                proposals={PROPOSAL_ID: proposal},
                redirects={},
                tests={TEST_ID: catalog_record(old)},
                evidence={},
                facts=[old],
            )

        self.assertTrue(any("target discovery_id does not match" in item for item in failures))

    def test_single_identity_plan_rejects_extra_source_and_decision_paths(self) -> None:
        with contract_root() as root:
            fact = generated_fact()
            proposal = no_refactor_proposal(fact)
            proposal["source_paths"] = [fact.path, "verification/test-catalog.toml"]
            proposal["decision_inputs"] = [ASSESSMENT_PATH, "verification/test-catalog.toml"]
            failures = validate_test_refactor_records(
                root=root,
                proposals={PROPOSAL_ID: proposal},
                redirects={},
                tests={TEST_ID: catalog_record(fact)},
                evidence={},
                facts=[fact],
            )

        self.assertTrue(any("source_paths must equal" in item for item in failures))
        self.assertTrue(any("decision_inputs must equal" in item for item in failures))

    def test_nonterminal_refactor_forbids_claimed_behavior_lock_evidence(self) -> None:
        with contract_root() as root:
            old = old_fact()
            new = generated_fact()
            proposal = refactor_proposal(old, new)
            proposal["status"] = "approved"
            proposal["lifecycle"] = ["proposed", "approved"]
            failures = validate_test_refactor_records(
                root=root,
                proposals={PROPOSAL_ID: proposal},
                redirects={},
                tests={TEST_ID: catalog_record(new)},
                evidence=lock_evidence(root),
                facts=[new],
            )

        self.assertTrue(any("non-completed proposal forbids" in item for item in failures))

    def test_redirect_accepts_only_exact_validated_proposal_and_live_endpoint(self) -> None:
        with contract_root() as root:
            old = old_fact()
            new = generated_fact()
            failures = validate_test_refactor_records(
                root=root,
                proposals={PROPOSAL_ID: refactor_proposal(old, new)},
                redirects={"TEST_REDIRECT_001": redirect_record(old, new)},
                tests={TEST_ID: catalog_record(new)},
                evidence=lock_evidence(root),
                facts=[new],
            )

        self.assertEqual(failures, [])

    def test_validated_refactor_requires_exactly_one_redirect(self) -> None:
        with contract_root() as root:
            old = old_fact()
            new = generated_fact()
            failures = validate_test_refactor_records(
                root=root,
                proposals={PROPOSAL_ID: refactor_proposal(old, new)},
                redirects={},
                tests={TEST_ID: catalog_record(new)},
                evidence=lock_evidence(root),
                facts=[new],
            )

        self.assertTrue(any("requires exactly one redirect" in item for item in failures))

    def test_redirect_history_chain_is_blocked_until_lock_ids_are_proposal_scoped(self) -> None:
        with contract_root() as root:
            first = old_fact()
            middle = generated_fact()
            final = final_fact()
            first_proposal = refactor_proposal(first, middle)
            second_proposal = refactor_proposal(middle, final)
            second_id = "TEST_REFACTOR_BYTECODE_CONTAINER_INVALID_MAGIC_002"
            second_proposal["id"] = second_id
            second_proposal["before_behavior_lock_evidence"] = "EVID_SECOND_BEFORE"
            second_proposal["after_behavior_lock_evidence"] = "EVID_SECOND_AFTER"
            first_redirect = redirect_record(first, middle)
            second_redirect = redirect_record(middle, final)
            second_redirect.update(
                id="TEST_REDIRECT_002",
                proposal_id=second_id,
                before_behavior_lock_evidence="EVID_SECOND_BEFORE",
                after_behavior_lock_evidence="EVID_SECOND_AFTER",
            )
            evidence = lock_evidence(root)
            evidence.update(lock_evidence(root, prefix="SECOND_"))
            failures = validate_test_refactor_records(
                root=root,
                proposals={PROPOSAL_ID: first_proposal, second_id: second_proposal},
                redirects={"TEST_REDIRECT_001": first_redirect, "TEST_REDIRECT_002": second_redirect},
                tests={TEST_ID: catalog_record(final)},
                evidence=evidence,
                facts=[final],
            )

        self.assertTrue(any("redirect chains are blocked" in item for item in failures))

    def test_redirect_rejects_orphan_and_no_refactor_proposal(self) -> None:
        with contract_root() as root:
            old = old_fact()
            new = generated_fact()
            orphan = redirect_record(old, new)
            orphan["proposal_id"] = "TEST_REFACTOR_UNKNOWN"
            no_change = no_refactor_proposal(new)
            no_change_redirect = redirect_record(old, new)
            failures = validate_test_refactor_records(
                root=root,
                proposals={PROPOSAL_ID: no_change},
                redirects={"TEST_REDIRECT_001": orphan, "TEST_REDIRECT_002": no_change_redirect},
                tests={TEST_ID: catalog_record(new)},
                evidence=lock_evidence(root),
                facts=[new],
            )

        self.assertTrue(any("orphan proposal" in item for item in failures))
        self.assertTrue(any("no_refactor_needed proposal cannot authorize" in item for item in failures))

    def test_redirect_rejects_live_old_missing_new_and_catalog_endpoint_drift(self) -> None:
        with contract_root() as root:
            old = old_fact()
            new = generated_fact()
            proposal = refactor_proposal(old, new)
            redirect = redirect_record(old, new)
            stale_catalog = catalog_record(old)
            failures = validate_test_refactor_records(
                root=root,
                proposals={PROPOSAL_ID: proposal},
                redirects={"TEST_REDIRECT_001": redirect},
                tests={TEST_ID: stale_catalog},
                evidence=lock_evidence(root),
                facts=[old],
            )

        self.assertTrue(any("old identity is still live" in item for item in failures))
        self.assertTrue(any("new identity is absent" in item for item in failures))
        self.assertTrue(any("catalog endpoint does not match" in item for item in failures))

    def test_redirect_rejects_identity_not_bound_by_proposal(self) -> None:
        with contract_root() as root:
            old = old_fact()
            new = generated_fact()
            redirect = redirect_record(old, new)
            redirect["old_identity"]["name"] = "unreviewed_old_name"
            failures = validate_test_refactor_records(
                root=root,
                proposals={PROPOSAL_ID: refactor_proposal(old, new)},
                redirects={"TEST_REDIRECT_001": redirect},
                tests={TEST_ID: catalog_record(new)},
                evidence=lock_evidence(root),
                facts=[new],
            )

        self.assertTrue(any("old identity does not match proposal" in item for item in failures))

    def test_live_endpoint_rejects_symlink_escape(self) -> None:
        with tempfile.TemporaryDirectory() as outside_temp, contract_root() as root:
            outside = Path(outside_temp) / "outside.rs"
            outside.write_text("#[test]\nfn header_validation() {}\n")
            endpoint = root / "crates/trust-runtime/tests/bytecode_container.rs"
            endpoint.unlink()
            endpoint.symlink_to(outside)
            fact = generated_fact()
            failures = validate_test_refactor_records(
                root=root,
                proposals={PROPOSAL_ID: no_refactor_proposal(fact)},
                redirects={},
                tests={TEST_ID: catalog_record(fact)},
                evidence={},
                facts=[fact],
            )

        self.assertTrue(any("symlink component" in item for item in failures))

    def test_redirect_graph_rejects_forks_merges_and_cycles(self) -> None:
        with contract_root() as root:
            first = old_fact()
            second = generated_fact()
            third = another_fact()
            redirects = {
                "TEST_REDIRECT_FORK_A": redirect_record(first, second),
                "TEST_REDIRECT_FORK_B": redirect_record(first, third),
                "TEST_REDIRECT_MERGE": redirect_record(third, second),
                "TEST_REDIRECT_CYCLE": redirect_record(second, first),
            }
            failures = validate_test_refactor_records(
                root=root,
                proposals={PROPOSAL_ID: refactor_proposal(first, second)},
                redirects=redirects,
                tests={TEST_ID: catalog_record(second)},
                evidence=lock_evidence(root),
                facts=[second],
            )

        self.assertTrue(any("redirect fork" in item for item in failures))
        self.assertTrue(any("redirect merge" in item for item in failures))
        self.assertTrue(any("redirect cycle" in item for item in failures))

    def test_assessment_must_support_no_refactor_without_actionable_signals(self) -> None:
        with contract_root() as root:
            fact = generated_fact()
            assessment = assessment_payload()
            assessment["proposal_evaluations"][0]["supported"] = False
            assessment["proposal_evaluations"][0]["observed_signals"] = ["large_mixed_file"]
            failures = validate_test_refactor_records(
                root=root,
                proposals={PROPOSAL_ID: no_refactor_proposal(fact)},
                redirects={},
                tests={TEST_ID: catalog_record(fact)},
                evidence={},
                facts=[fact],
                assessment=assessment,
            )

        self.assertTrue(any("assessment does not support" in item for item in failures))
        self.assertTrue(any("actionable assessment signals" in item for item in failures))

    def test_schemas_are_closed_at_every_object_level(self) -> None:
        root = Path(__file__).resolve().parents[2]
        for relative in (PROPOSAL_SCHEMA_PATH, REDIRECT_SCHEMA_PATH):
            schema = json.loads((root / relative).read_text())
            self.assertIs(schema.get("additionalProperties"), False)
            for definition in schema.get("$defs", {}).values():
                if definition.get("type") == "object":
                    self.assertIs(definition.get("additionalProperties"), False)


def no_refactor_proposal(fact) -> dict:
    current = fact_identity(fact)
    return {
        "schema_version": 1,
        "id": PROPOSAL_ID,
        "test_id": TEST_ID,
        "disposition": "no_refactor_needed",
        "status": "validated",
        "lifecycle": ["proposed", "reviewed", "validated"],
        "source_paths": [fact.path],
        "source_identity": current,
        "target_identity": copy.deepcopy(current),
        "decision_inputs": [ASSESSMENT_PATH],
        "finding_refs": [],
        "before_command": COMMAND,
        "after_command": COMMAND,
        "invariant_ids": ["VM_SEAM_VALID_001"],
        "coverage_dimensions": ["malformed_input_class:bad_magic"],
        "fixture_ownership": {
            "before_owner": "crates/trust-runtime/tests/bytecode_container.rs::valid_container",
            "after_owner": "crates/trust-runtime/tests/bytecode_container.rs::valid_container",
            "review": "The compact helper remains owned by its focused container-format test file.",
        },
        "stale_path_updates": [],
        "expected_behavior_delta": "none",
        "design_review": {
            "solid": "pass",
            "kiss": "pass",
            "dry": "pass",
            "rationale": "The focused test and local fixture have one responsibility and no justified split.",
        },
        "rationale": "The assessment reports no actionable move, split, or rename signal for this pilot test.",
        "last_reviewed": "2026-07-10",
    }


def refactor_proposal(old, new) -> dict:
    proposal = no_refactor_proposal(old)
    proposal.update(
        disposition="rename",
        status="validated",
        lifecycle=["proposed", "approved", "in_progress", "completed", "validated"],
        source_paths=[old.path],
        source_identity=fact_identity(old),
        target_identity=fact_identity(new),
        finding_refs=["mixed_purpose_file"],
        stale_path_updates=["verification/test-catalog.toml"],
        before_behavior_lock_evidence="EVID_BEFORE",
        after_behavior_lock_evidence="EVID_AFTER",
        rationale="A reviewed assessment signal justifies renaming while preserving behavior.",
    )
    return proposal


def redirect_record(old, new) -> dict:
    return {
        "schema_version": 1,
        "id": "TEST_REDIRECT_001",
        "proposal_id": PROPOSAL_ID,
        "test_id": TEST_ID,
        "status": "active",
        "old_identity": fact_identity(old),
        "new_identity": fact_identity(new),
        "before_behavior_lock_evidence": "EVID_BEFORE",
        "after_behavior_lock_evidence": "EVID_AFTER",
        "last_reviewed": "2026-07-10",
    }


def catalog_record(fact, *, command: str = COMMAND) -> dict:
    return {
        "id": TEST_ID,
        "subject_kind": "generated_test",
        "discovery_id": fact.stable_id,
        "discovery_source_kind": fact.source_kind,
        "path": fact.path,
        "name": fact.name,
        "command": command,
        "invariants": ["VM_SEAM_VALID_001"],
        "malformed_input_class_ids": ["bad_magic"],
        "case_file": "verification/cases/test.toml",
        "case_file_digest": "sha256:" + "a" * 64,
    }


def lock_evidence(
    root: Path,
    *,
    after_command: str = COMMAND,
    prefix: str = "",
    same_revision: bool = False,
) -> dict[str, dict]:
    summary = ["CASE_INVALID_MAGIC:passed"]
    digest = "sha256:" + hashlib.sha256(
        json.dumps(
            {"command_exit_status": 0, "per_case_summary": summary},
            sort_keys=True,
            separators=(",", ":"),
        ).encode()
    ).hexdigest()
    common = {
        "linked_tests": [TEST_ID],
        "linked_invariants": ["VM_SEAM_VALID_001"],
        "case_result_digest": digest,
        "case_file_digest": "sha256:" + "a" * 64,
        "case_artifact_digest": "sha256:" + "b" * 64,
        "command_exit_status": 0,
        "per_case_summary": summary,
    }
    before_id = f"EVID_{prefix}BEFORE"
    after_id = f"EVID_{prefix}AFTER"
    revisions = subprocess.run(
        ["git", "-C", str(root), "rev-list", "--reverse", "HEAD"],
        check=True,
        capture_output=True,
        text=True,
    ).stdout.splitlines()
    before_revision = revisions[0]
    after_revision = before_revision if same_revision else revisions[-1]
    before = {
        "id": before_id,
        "proof_kind": "lock_baseline",
        "command": COMMAND,
        "commit": f"dirty:{before_revision[:12]}",
        "trust_verify_run_id": f"{prefix}before-run",
        **common,
    }
    after = {
        "id": after_id,
        "proof_kind": "lock_compare",
        "paired_lock_baseline": before_id,
        "command": after_command,
        "commit": f"dirty:{after_revision[:12]}",
        "trust_verify_run_id": f"{prefix}after-run",
        **common,
    }
    return {before_id: before, after_id: after}


def assessment_payload() -> dict:
    return {
        "proposal_evaluations": [
            {
                "proposal_id": PROPOSAL_ID,
                "disposition": "no_refactor_needed",
                "source_paths": ["crates/trust-runtime/tests/bytecode_container.rs"],
                "observed_signals": [],
                "supported": True,
            }
        ]
    }


def fact_identity(fact) -> dict:
    return {
        "discovery_id": fact.stable_id,
        "discovery_source_kind": fact.source_kind,
        "path": fact.path,
        "name": fact.name,
    }


def old_identity() -> dict:
    return fact_identity(old_fact())


def generated_fact():
    return make_fact(
        source_kind="rust_integration_test",
        name="header_validation",
        path="crates/trust-runtime/tests/bytecode_container.rs",
        line=9,
        package="trust-runtime",
        command_hint=COMMAND,
        command_hint_authority="conservative",
        discovery_confidence="exact_attribute",
    )


def old_fact():
    return make_fact(
        source_kind="rust_integration_test",
        name="old_header_validation",
        path="crates/trust-runtime/tests/bytecode_container.rs",
        line=9,
        package="trust-runtime",
        command_hint=COMMAND,
        command_hint_authority="conservative",
        discovery_confidence="exact_attribute",
    )


def another_fact():
    return make_fact(
        source_kind="rust_integration_test",
        name="another_header_validation",
        path="crates/trust-runtime/tests/another_bytecode_container.rs",
        line=9,
        package="trust-runtime",
        command_hint=COMMAND,
        command_hint_authority="conservative",
        discovery_confidence="exact_attribute",
    )


def final_fact():
    return make_fact(
        source_kind="rust_integration_test",
        name="final_header_validation",
        path="crates/trust-runtime/tests/bytecode_container.rs",
        line=9,
        package="trust-runtime",
        command_hint=COMMAND,
        command_hint_authority="conservative",
        discovery_confidence="exact_attribute",
    )


class contract_root:
    def __init__(self) -> None:
        self._temp = tempfile.TemporaryDirectory()
        self.root = Path(self._temp.name)

    def __enter__(self) -> Path:
        for relative in (
            "crates/trust-runtime/tests/bytecode_container.rs",
            "verification/test-catalog.toml",
            "verification/cases/test.toml",
            ASSESSMENT_PATH,
        ):
            path = self.root / relative
            path.parent.mkdir(parents=True, exist_ok=True)
            if relative == "verification/cases/test.toml":
                path.write_text(
                    "[[case]]\n"
                    'id = "CASE_INVALID_MAGIC"\n'
                    "[case.input]\n"
                    'kind = "invalid_magic"\n'
                )
            else:
                path.write_text("fixture\n")
        subprocess.run(["git", "-C", str(self.root), "init", "-q"], check=True)
        subprocess.run(
            ["git", "-C", str(self.root), "config", "user.email", "test@example.invalid"],
            check=True,
        )
        subprocess.run(
            ["git", "-C", str(self.root), "config", "user.name", "Verification Test"],
            check=True,
        )
        subprocess.run(["git", "-C", str(self.root), "add", "."], check=True)
        subprocess.run(
            ["git", "-C", str(self.root), "commit", "-q", "-m", "before"],
            check=True,
        )
        marker = self.root / "after-marker"
        marker.write_text("after\n")
        subprocess.run(["git", "-C", str(self.root), "add", "after-marker"], check=True)
        subprocess.run(
            ["git", "-C", str(self.root), "commit", "-q", "-m", "after"],
            check=True,
        )
        return self.root

    def __exit__(self, exc_type, exc, tb) -> None:
        self._temp.cleanup()


if __name__ == "__main__":
    unittest.main()
