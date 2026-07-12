"""Self-tests for the red/green verification prover slices."""

from __future__ import annotations

import json
import subprocess
import tempfile
import textwrap
import tomllib
import unittest
from pathlib import Path

from scripts.verification.proof_contract import proof_contract_digest
from scripts.verification.prover import (
    ProofError,
    ProofProducer,
    case_result_digest,
    sha256_file,
    validate_case_artifact,
)


class RedProofProducerTests(unittest.TestCase):
    def test_red_records_same_run_failed_case_artifact(self) -> None:
        with fixture() as fx:
            fx.add_case_file("CASE_FAIL")
            fx.add_catalog_test(status="mapped")
            fx.add_writer("failed", exit_code=1)

            result = fx.prover().red("TEST_RED")

            self.assertEqual(result.record["proof_kind"], "red")
            self.assertEqual(result.record["proof_scope"], "targeted")
            self.assertEqual(result.record["failure_kind"], "assertion_failure")
            self.assertEqual(result.record["red_case_ids"], ["CASE_FAIL"])
            self.assertEqual(result.record["linked_tests"], ["TEST_RED"])
            self.assertEqual(result.record["case_file_digest"], fx.case_digest)
            self.assertEqual(result.record["proof_contract_digest"], fx.contract_digest())
            self.assertTrue(result.evidence_path.exists())


class DurableProofProducerTests(unittest.TestCase):
    def test_red_appends_producer_record_to_tracked_evidence_index(self) -> None:
        with fixture() as fx:
            fx.add_case_file("CASE_FAIL")
            fx.add_catalog_test(status="mapped")
            fx.add_writer("failed", exit_code=1)
            fx.initialize_git()

            result = fx.default_durable_prover().red("TEST_RED")

            self.assertEqual(result.evidence_path, fx.verification / "evidence-index.toml")
            records = tomllib.loads(result.evidence_path.read_text())["evidence"]
            self.assertEqual(records[-1], result.record)
            self.assertEqual(result.record["path"], "verification/evidence-index.toml")
            self.assertEqual(result.record["proof_scope"], "targeted")
            self.assertEqual(result.record["commit"], fx.git("rev-parse", "HEAD"))
            self.assertRegex(result.record["commit"], r"^[0-9a-f]{40}$")

    def test_red_refuses_dirty_tree_before_running_test(self) -> None:
        with fixture() as fx:
            fx.add_case_file("CASE_FAIL")
            fx.add_catalog_test(status="mapped")
            fx.add_writer("failed", exit_code=1)
            fx.initialize_git()
            original_index = (fx.verification / "evidence-index.toml").read_bytes()
            (fx.root / "unrelated.txt").write_text("dirty\n")

            with self.assertRaisesRegex(ProofError, "clean Git worktree"):
                fx.default_durable_prover().red("TEST_RED")

            self.assertEqual((fx.verification / "evidence-index.toml").read_bytes(), original_index)
            self.assertFalse((fx.artifact_dir / "TEST_RED.json").exists())


class CaseArtifactProvenanceTests(unittest.TestCase):
    def test_hand_authored_trace_digest_mismatch_is_rejected(self) -> None:
        artifact = {
            "schema_version": 1,
            "test_id": "TEST_TRACE",
            "case_file_digest": "sha256:cases",
            "case_provenance_kind": "hand_authored_state_machine_v1",
            "trace_definition_digest": "sha256:" + "b" * 64,
            "trust_verify_test_id": "TEST_TRACE",
            "trust_verify_run_id": "run-trace",
            "trust_verify_case_file_digest": "sha256:cases",
            "trust_verify_artifact_dir": "target/trace",
            "cases": [{"id": "CASE_TRACE", "result": "passed"}],
        }

        with self.assertRaisesRegex(ProofError, "trace_definition_digest mismatch"):
            validate_case_artifact(
                artifact=artifact,
                expected_test_id="TEST_TRACE",
                expected_run_id="run-trace",
                expected_artifact_dir="target/trace",
                expected_case_file_digest="sha256:cases",
                expected_case_ids=["CASE_TRACE"],
                expected_case_provenance_kind="hand_authored_state_machine_v1",
                expected_trace_definition_digest="sha256:" + "a" * 64,
            )

    def test_green_requires_and_records_a_later_descendant_commit(self) -> None:
        with fixture() as fx:
            fx.add_case_file("CASE_FAIL")
            fx.add_catalog_test(status="mapped")
            fx.add_writer("failed", exit_code=1)
            fx.initialize_git()
            red = fx.default_durable_prover().red("TEST_RED")
            red_revision = red.record["commit"]
            fx.git("add", "verification/evidence-index.toml")
            fx.git("commit", "-qm", "record red proof")
            fx.add_writer("passed", exit_code=0)
            fx.git("add", "writer.py")
            fx.git("commit", "-qm", "fix behavior")

            green = fx.default_durable_prover().green("TEST_RED", red.record["id"])

            self.assertNotEqual(green.record["commit"], red_revision)
            self.assertEqual(
                fx.git("merge-base", "--is-ancestor", str(red_revision), str(green.record["commit"])),
                "",
            )
            records = tomllib.loads(green.evidence_path.read_text())["evidence"]
            self.assertEqual(records[-1], green.record)


class RedProofProducerAdditionalTests(unittest.TestCase):

    def test_stale_artifact_is_removed_and_cannot_be_red(self) -> None:
        with fixture() as fx:
            fx.add_case_file("CASE_FAIL")
            fx.add_catalog_test(status="mapped")
            fx.add_writer("none", exit_code=0)
            stale_path = fx.artifact_dir / "TEST_RED.json"
            fx.write_artifact(stale_path, case_id="CASE_FAIL", result="failed", run_id="stale")

            with self.assertRaises(ProofError) as raised:
                fx.prover().red("TEST_RED")

            self.assertEqual(raised.exception.failure_kind, "metadata_error")
            self.assertFalse((fx.evidence_dir / "EVID_TEST_RED_RED.toml").exists())

    def test_wrong_run_stamp_artifact_is_rejected(self) -> None:
        with fixture() as fx:
            fx.add_case_file("CASE_FAIL")
            fx.add_catalog_test(status="mapped")
            fx.add_writer("failed_wrong_run", exit_code=1)

            with self.assertRaises(ProofError) as raised:
                fx.prover().red("TEST_RED")

            self.assertEqual(raised.exception.failure_kind, "metadata_error")
            self.assertIn("TRUST_VERIFY_RUN_ID", str(raised.exception))

    def test_case_provenance_mismatch_is_rejected(self) -> None:
        with fixture() as fx:
            fx.add_case_file("CASE_FAIL")
            fx.add_catalog_test(status="mapped")
            fx.add_writer("wrong_provenance", exit_code=1)

            with self.assertRaises(ProofError) as raised:
                fx.prover().red("TEST_RED")

            self.assertEqual(raised.exception.failure_kind, "metadata_error")
            self.assertIn("case_provenance_kind mismatch", str(raised.exception))

    def test_non_artifact_command_failure_is_not_red(self) -> None:
        with fixture() as fx:
            fx.add_case_file("CASE_FAIL")
            fx.add_catalog_test(status="mapped")
            fx.add_writer("none", exit_code=1)

            with self.assertRaises(ProofError) as raised:
                fx.prover().red("TEST_RED")

            self.assertEqual(raised.exception.failure_kind, "compile_error")

    def test_expected_rejection_catalog_knob_is_rejected_until_validated(self) -> None:
        with fixture() as fx:
            fx.add_case_file("CASE_FAIL")
            fx.add_catalog_test(
                status="mapped",
                extra={"expected_red_failure_kind": "expected_rejection"},
            )
            fx.add_writer("none", exit_code=127)

            with self.assertRaises(ProofError) as raised:
                fx.prover().red("TEST_RED")

            self.assertEqual(raised.exception.failure_kind, "metadata_error")
            self.assertIn("expected_red_failure_kind", str(raised.exception))

    def test_timeout_is_classified_without_traceback(self) -> None:
        with fixture() as fx:
            fx.add_case_file("CASE_FAIL")
            fx.add_catalog_test(status="mapped")
            fx.add_writer("sleep", exit_code=0)

            with self.assertRaises(ProofError) as raised:
                fx.prover(command_timeout_seconds=0.01).red("TEST_RED")

            self.assertEqual(raised.exception.failure_kind, "timeout")

    def test_exit_zero_with_failed_cases_is_metadata_error(self) -> None:
        with fixture() as fx:
            fx.add_case_file("CASE_FAIL")
            fx.add_catalog_test(status="mapped")
            fx.add_writer("failed", exit_code=0)

            with self.assertRaises(ProofError) as raised:
                fx.prover().red("TEST_RED")

            self.assertEqual(raised.exception.failure_kind, "metadata_error")
            self.assertIn("exited 0", str(raised.exception))

    def test_nonzero_with_artifact_but_no_failed_cases_is_harness_panic(self) -> None:
        with fixture() as fx:
            fx.add_case_file("CASE_FAIL")
            fx.add_catalog_test(status="mapped")
            fx.add_writer("passed", exit_code=1)

            with self.assertRaises(ProofError) as raised:
                fx.prover().red("TEST_RED")

            self.assertEqual(raised.exception.failure_kind, "harness_panic")

    def test_exit_zero_with_no_failed_cases_is_not_red(self) -> None:
        with fixture() as fx:
            fx.add_case_file("CASE_FAIL")
            fx.add_catalog_test(status="mapped")
            fx.add_writer("passed", exit_code=0)

            with self.assertRaises(ProofError) as raised:
                fx.prover().red("TEST_RED")

            self.assertEqual(raised.exception.failure_kind, "none")

    def test_case_id_set_violations_are_rejected(self) -> None:
        for mode, expected in [
            ("unknown_case", "unknown case artifact id"),
            ("duplicate_case", "duplicate case artifact id"),
            ("missing_case", "case artifact missing cases"),
            ("skipped_case", "was skipped without waiver"),
        ]:
            with self.subTest(mode=mode), fixture() as fx:
                fx.add_case_file("CASE_FAIL")
                fx.add_catalog_test(status="mapped")
                fx.add_writer(mode, exit_code=1)

                with self.assertRaises(ProofError) as raised:
                    fx.prover().red("TEST_RED")

                self.assertEqual(raised.exception.failure_kind, "metadata_error")
                self.assertIn(expected, str(raised.exception))

    def test_planned_catalog_rows_are_not_runnable_proof(self) -> None:
        with fixture() as fx:
            fx.add_case_file("CASE_FAIL")
            fx.add_catalog_test(status="planned")
            fx.add_writer("failed", exit_code=1)

            with self.assertRaises(ProofError) as raised:
                fx.prover().red("TEST_RED")

            self.assertEqual(raised.exception.failure_kind, "metadata_error")
            self.assertIn("not runnable", str(raised.exception))

    def test_ignored_tests_are_refused(self) -> None:
        with fixture() as fx:
            fx.add_case_file("CASE_FAIL")
            fx.add_catalog_test(status="mapped")
            fx.add_ignored_test()
            fx.add_writer("failed", exit_code=1)

            with self.assertRaises(ProofError) as raised:
                fx.prover().red("TEST_RED")

            self.assertEqual(raised.exception.failure_kind, "metadata_error")
            self.assertIn("ignored", str(raised.exception))

    def test_ignored_registry_records_without_catalog_test_id_do_not_block_proof(self) -> None:
        with fixture() as fx:
            fx.add_case_file("CASE_FAIL")
            fx.add_catalog_test(status="mapped")
            fx.ignored["IGNORED_UNMAPPED_001"] = {
                "id": "IGNORED_UNMAPPED_001",
                "discovery_id": "DISC_0123456789ABCDEF0123",
            }
            fx.add_writer("failed", exit_code=1)

            result = fx.prover().red("TEST_RED")

            self.assertEqual(result.record["proof_kind"], "red")


class GreenProverTests(unittest.TestCase):
    def test_green_records_pair_when_formerly_red_cases_now_pass(self) -> None:
        with fixture() as fx:
            fx.add_case_file("CASE_FAIL")
            fx.add_catalog_test(status="mapped")
            fx.add_writer("failed", exit_code=1)
            red = fx.prover().red("TEST_RED")
            fx.add_writer("passed", exit_code=0)

            green = fx.prover().green("TEST_RED", red.record["id"])

            self.assertEqual(green.record["proof_kind"], "green")
            self.assertEqual(green.record["failure_kind"], "none")
            self.assertEqual(green.record["paired_red_evidence"], red.record["id"])
            self.assertEqual(green.record["formerly_red_case_ids"], ["CASE_FAIL"])
            self.assertEqual(green.record["case_file_digest"], fx.case_digest)
            self.assertEqual(green.record["linked_tests"], ["TEST_RED"])
            self.assertEqual(green.record["per_case_summary"], ["CASE_FAIL:passed"])
            self.assertEqual(
                green.record["proof_contract_digest"],
                red.record["proof_contract_digest"],
            )
            self.assertTrue(green.evidence_path.exists())

    def test_green_refuses_catalog_command_drift_before_running(self) -> None:
        with fixture() as fx:
            fx.add_case_file("CASE_FAIL")
            fx.add_catalog_test(status="mapped")
            fx.write_evidence("EVID_RED")
            fx.tests["TEST_RED"]["command"] = "python3 command-that-must-not-run.py"

            with self.assertRaisesRegex(ProofError, "proof_contract_digest"):
                fx.prover().green("TEST_RED", "EVID_RED")

            self.assertFalse((fx.artifact_dir / "TEST_RED.json").exists())

    def test_green_refuses_invariant_list_or_content_drift_before_running(self) -> None:
        for mutation in ("list", "content"):
            with self.subTest(mutation=mutation), fixture() as fx:
                fx.add_case_file("CASE_FAIL")
                fx.add_catalog_test(status="mapped")
                fx.write_evidence("EVID_RED")
                if mutation == "list":
                    fx.tests["TEST_RED"]["invariants"] = []
                else:
                    fx.invariants["INV"]["title"] = "Changed invariant"

                with self.assertRaisesRegex(
                    ProofError,
                    "proof_contract_digest|linked_invariants",
                ):
                    fx.prover().green("TEST_RED", "EVID_RED")

                self.assertFalse((fx.artifact_dir / "TEST_RED.json").exists())

    def test_green_refuses_arbitrary_catalog_row_drift_before_running(self) -> None:
        with fixture() as fx:
            fx.add_case_file("CASE_FAIL")
            fx.add_catalog_test(status="mapped")
            fx.write_evidence("EVID_RED")
            fx.tests["TEST_RED"]["title"] = "Retitled after red proof"

            with self.assertRaisesRegex(ProofError, "proof_contract_digest"):
                fx.prover().green("TEST_RED", "EVID_RED")

            self.assertFalse((fx.artifact_dir / "TEST_RED.json").exists())

    def test_green_rejects_non_red_pair(self) -> None:
        with fixture() as fx:
            fx.add_case_file("CASE_FAIL")
            fx.add_catalog_test(status="mapped")
            fx.write_evidence("EVID_NOTE", proof_kind="none")
            fx.add_writer("passed", exit_code=0)

            with self.assertRaises(ProofError) as raised:
                fx.prover().green("TEST_RED", "EVID_NOTE")

            self.assertEqual(raised.exception.failure_kind, "metadata_error")
            self.assertIn("not red/protective_red", str(raised.exception))

    def test_green_rejects_red_without_failed_case_data(self) -> None:
        with fixture() as fx:
            fx.add_case_file("CASE_FAIL")
            fx.add_catalog_test(status="mapped")
            fx.write_evidence("EVID_RED", red_case_ids=[], per_case_summary=[])
            fx.add_writer("passed", exit_code=0)

            with self.assertRaises(ProofError) as raised:
                fx.prover().green("TEST_RED", "EVID_RED")

            self.assertEqual(raised.exception.failure_kind, "metadata_error")
            self.assertIn("has no red_case_ids", str(raised.exception))

    def test_green_rejects_pair_for_wrong_test(self) -> None:
        with fixture() as fx:
            fx.add_case_file("CASE_FAIL")
            fx.add_catalog_test(status="mapped")
            fx.write_evidence("EVID_RED", linked_tests=["OTHER_TEST"])
            fx.add_writer("passed", exit_code=0)

            with self.assertRaises(ProofError) as raised:
                fx.prover().green("TEST_RED", "EVID_RED")

            self.assertEqual(raised.exception.failure_kind, "metadata_error")
            self.assertIn("linked_tests must be exactly", str(raised.exception))

    def test_green_rejects_case_file_digest_drift(self) -> None:
        with fixture() as fx:
            fx.add_case_file("CASE_FAIL")
            fx.add_catalog_test(status="mapped")
            fx.write_evidence("EVID_RED", case_file_digest="sha256:wrong")
            fx.add_writer("passed", exit_code=0)

            with self.assertRaises(ProofError) as raised:
                fx.prover().green("TEST_RED", "EVID_RED")

            self.assertEqual(raised.exception.failure_kind, "metadata_error")
            self.assertIn("case_file_digest", str(raised.exception))

    def test_green_rejects_formerly_red_case_still_failed(self) -> None:
        with fixture() as fx:
            fx.add_case_file("CASE_FAIL")
            fx.add_catalog_test(status="mapped")
            fx.write_evidence("EVID_RED")
            fx.add_writer("failed", exit_code=1)

            with self.assertRaises(ProofError) as raised:
                fx.prover().green("TEST_RED", "EVID_RED")

            self.assertEqual(raised.exception.failure_kind, "assertion_failure")

    def test_green_rejects_nonzero_command_without_failed_cases(self) -> None:
        with fixture() as fx:
            fx.add_case_file("CASE_FAIL")
            fx.add_catalog_test(status="mapped")
            fx.write_evidence("EVID_RED")
            fx.add_writer("passed", exit_code=1)

            with self.assertRaises(ProofError) as raised:
                fx.prover().green("TEST_RED", "EVID_RED")

            self.assertEqual(raised.exception.failure_kind, "harness_panic")

    def test_green_rejects_blocked_never_red_case(self) -> None:
        with fixture() as fx:
            fx.add_case_file("CASE_FAIL", "CASE_BLOCKED")
            fx.add_catalog_test(status="mapped")
            fx.write_evidence("EVID_RED")
            fx.add_writer("blocked_case", exit_code=0)

            with self.assertRaises(ProofError) as raised:
                fx.prover().green("TEST_RED", "EVID_RED")

            self.assertEqual(raised.exception.failure_kind, "metadata_error")
            self.assertIn("blocked cases", str(raised.exception))

    def test_green_rejects_unknown_case_result(self) -> None:
        with fixture() as fx:
            fx.add_case_file("CASE_FAIL", "CASE_OTHER")
            fx.add_catalog_test(status="mapped")
            fx.write_evidence("EVID_RED")
            fx.add_writer("errored_case", exit_code=0)

            with self.assertRaises(ProofError) as raised:
                fx.prover().green("TEST_RED", "EVID_RED")

            self.assertEqual(raised.exception.failure_kind, "metadata_error")
            self.assertIn("unknown case result", str(raised.exception))

    def test_green_rejects_unknown_red_evidence_id(self) -> None:
        with fixture() as fx:
            fx.add_case_file("CASE_FAIL")
            fx.add_catalog_test(status="mapped")
            fx.add_writer("passed", exit_code=0)

            with self.assertRaises(ProofError) as raised:
                fx.prover().green("TEST_RED", "EVID_MISSING")

            self.assertEqual(raised.exception.failure_kind, "metadata_error")
            self.assertIn("not found", str(raised.exception))

    def test_green_rejects_non_allowlisted_red_producer(self) -> None:
        with fixture() as fx:
            fx.add_case_file("CASE_FAIL")
            fx.add_catalog_test(status="mapped")
            fx.write_evidence("EVID_RED", producer="codex")
            fx.add_writer("passed", exit_code=0)

            with self.assertRaises(ProofError) as raised:
                fx.prover().green("TEST_RED", "EVID_RED")

            self.assertEqual(raised.exception.failure_kind, "metadata_error")
            self.assertIn("producer", str(raised.exception))

    def test_green_requires_red_pair_to_link_exactly_one_test(self) -> None:
        with fixture() as fx:
            fx.add_case_file("CASE_FAIL")
            fx.add_catalog_test(status="mapped")
            fx.write_evidence("EVID_RED", linked_tests=["TEST_RED", "OTHER_TEST"])
            fx.add_writer("passed", exit_code=0)

            with self.assertRaises(ProofError) as raised:
                fx.prover().green("TEST_RED", "EVID_RED")

            self.assertEqual(raised.exception.failure_kind, "metadata_error")
            self.assertIn("linked_tests", str(raised.exception))


class LockProverTests(unittest.TestCase):
    def test_lock_baseline_records_result_digest_and_case_summary(self) -> None:
        with fixture() as fx:
            fx.add_case_file("CASE_FAIL")
            fx.add_catalog_test(status="mapped", extra={"test_class": "behavior_lock"})
            fx.add_writer("passed", exit_code=0)

            baseline = fx.prover().lock_baseline("TEST_RED")

            self.assertEqual(baseline.record["proof_kind"], "lock_baseline")
            self.assertEqual(baseline.record["failure_kind"], "none")
            self.assertEqual(baseline.record["linked_tests"], ["TEST_RED"])
            self.assertEqual(baseline.record["case_file_digest"], fx.case_digest)
            self.assertEqual(baseline.record["command_exit_status"], 0)
            self.assertEqual(baseline.record["per_case_summary"], ["CASE_FAIL:passed"])
            self.assertEqual(baseline.record["proof_contract_digest"], fx.contract_digest())
            self.assertTrue(str(baseline.record["case_result_digest"]).startswith("sha256:"))
            self.assertTrue(str(baseline.record["case_artifact_digest"]).startswith("sha256:"))
            self.assertTrue(baseline.evidence_path.exists())

    def test_lock_compare_records_when_baseline_matches(self) -> None:
        with fixture() as fx:
            fx.add_case_file("CASE_FAIL")
            fx.add_catalog_test(status="mapped", extra={"test_class": "behavior_lock"})
            fx.add_writer("passed", exit_code=0)
            baseline = fx.prover().lock_baseline("TEST_RED")
            fx.add_writer("passed", exit_code=0)

            compare = fx.prover().lock_compare("TEST_RED", baseline.record["id"])

            self.assertEqual(compare.record["proof_kind"], "lock_compare")
            self.assertEqual(compare.record["failure_kind"], "none")
            self.assertEqual(compare.record["paired_lock_baseline"], baseline.record["id"])
            self.assertEqual(compare.record["case_file_digest"], fx.case_digest)
            self.assertEqual(compare.record["case_result_digest"], baseline.record["case_result_digest"])
            self.assertNotEqual(
                compare.record["case_artifact_digest"],
                baseline.record["case_artifact_digest"],
            )
            self.assertEqual(compare.record["per_case_summary"], ["CASE_FAIL:passed"])
            self.assertEqual(
                compare.record["proof_contract_digest"],
                baseline.record["proof_contract_digest"],
            )
            self.assertTrue(compare.evidence_path.exists())

    def test_lock_compare_refuses_contract_drift_before_running(self) -> None:
        for mutation in ("command", "invariant_content", "catalog_row"):
            with self.subTest(mutation=mutation), fixture() as fx:
                fx.add_case_file("CASE_FAIL")
                fx.add_catalog_test(status="mapped", extra={"test_class": "behavior_lock"})
                fx.write_lock_baseline("EVID_LOCK")
                if mutation == "command":
                    fx.tests["TEST_RED"]["command"] = "python3 command-that-must-not-run.py"
                elif mutation == "invariant_content":
                    fx.invariants["INV"]["title"] = "Changed invariant"
                else:
                    fx.tests["TEST_RED"]["duration_class"] = "slow"

                with self.assertRaisesRegex(ProofError, "proof_contract_digest"):
                    fx.prover().lock_compare("TEST_RED", "EVID_LOCK")

                self.assertFalse((fx.artifact_dir / "TEST_RED.json").exists())

    def test_lock_compare_rejects_missing_or_wrong_kind_baseline(self) -> None:
        with fixture() as fx:
            fx.add_case_file("CASE_FAIL")
            fx.add_catalog_test(status="mapped", extra={"test_class": "behavior_lock"})
            fx.add_writer("passed", exit_code=0)

            with self.assertRaises(ProofError) as missing:
                fx.prover().lock_compare("TEST_RED", "EVID_MISSING")
            self.assertEqual(missing.exception.failure_kind, "metadata_error")
            self.assertIn("not found", str(missing.exception))

            fx.write_evidence("EVID_RED")
            with self.assertRaises(ProofError) as wrong_kind:
                fx.prover().lock_compare("TEST_RED", "EVID_RED")
            self.assertEqual(wrong_kind.exception.failure_kind, "metadata_error")
            self.assertIn("not lock_baseline", str(wrong_kind.exception))

    def test_lock_compare_rejects_baseline_for_wrong_test(self) -> None:
        with fixture() as fx:
            fx.add_case_file("CASE_FAIL")
            fx.add_catalog_test(status="mapped", extra={"test_class": "behavior_lock"})
            fx.write_lock_baseline("EVID_LOCK", linked_tests=["OTHER_TEST"])
            fx.add_writer("passed", exit_code=0)

            with self.assertRaises(ProofError) as raised:
                fx.prover().lock_compare("TEST_RED", "EVID_LOCK")

            self.assertEqual(raised.exception.failure_kind, "metadata_error")
            self.assertIn("linked_tests", str(raised.exception))

    def test_lock_compare_rejects_case_file_digest_drift(self) -> None:
        with fixture() as fx:
            fx.add_case_file("CASE_FAIL")
            fx.add_catalog_test(status="mapped", extra={"test_class": "behavior_lock"})
            fx.write_lock_baseline("EVID_LOCK", case_file_digest="sha256:wrong")
            fx.add_writer("passed", exit_code=0)

            with self.assertRaises(ProofError) as raised:
                fx.prover().lock_compare("TEST_RED", "EVID_LOCK")

            self.assertEqual(raised.exception.failure_kind, "metadata_error")
            self.assertIn("case_file_digest", str(raised.exception))

    def test_lock_compare_rejects_non_allowlisted_baseline_producer(self) -> None:
        with fixture() as fx:
            fx.add_case_file("CASE_FAIL")
            fx.add_catalog_test(status="mapped", extra={"test_class": "behavior_lock"})
            fx.write_lock_baseline("EVID_LOCK", producer="codex")
            fx.add_writer("passed", exit_code=0)

            with self.assertRaises(ProofError) as raised:
                fx.prover().lock_compare("TEST_RED", "EVID_LOCK")

            self.assertEqual(raised.exception.failure_kind, "metadata_error")
            self.assertIn("producer", str(raised.exception))

    def test_lock_compare_rejects_baseline_command_drift(self) -> None:
        with fixture() as fx:
            fx.add_case_file("CASE_FAIL")
            fx.add_catalog_test(status="mapped", extra={"test_class": "behavior_lock"})
            fx.write_lock_baseline("EVID_LOCK", command="python3 old_writer.py")
            fx.add_writer("passed", exit_code=0)

            with self.assertRaises(ProofError) as raised:
                fx.prover().lock_compare("TEST_RED", "EVID_LOCK")

            self.assertEqual(raised.exception.failure_kind, "metadata_error")
            self.assertIn("command", str(raised.exception))

    def test_lock_compare_rejects_command_exit_delta(self) -> None:
        with fixture() as fx:
            fx.add_case_file("CASE_FAIL")
            fx.add_catalog_test(status="mapped", extra={"test_class": "behavior_lock"})
            fx.write_lock_baseline("EVID_LOCK", command_exit_status=1)
            fx.add_writer("passed", exit_code=0)

            with self.assertRaises(ProofError) as raised:
                fx.prover().lock_compare("TEST_RED", "EVID_LOCK")

            self.assertEqual(raised.exception.failure_kind, "metadata_error")
            self.assertIn("command_exit_status", str(raised.exception))

    def test_lock_compare_rejects_case_result_delta(self) -> None:
        with fixture() as fx:
            fx.add_case_file("CASE_FAIL", "CASE_OTHER")
            fx.add_catalog_test(status="mapped", extra={"test_class": "behavior_lock"})
            fx.write_lock_baseline(
                "EVID_LOCK",
                per_case_summary=["CASE_FAIL:passed", "CASE_OTHER:passed"],
            )
            fx.add_writer("changed_second_case", exit_code=0)

            with self.assertRaises(ProofError) as raised:
                fx.prover().lock_compare("TEST_RED", "EVID_LOCK")

            self.assertEqual(raised.exception.failure_kind, "metadata_error")
            self.assertIn("case result", str(raised.exception))

    def test_lock_refuses_failed_current_run(self) -> None:
        with fixture() as fx:
            fx.add_case_file("CASE_FAIL")
            fx.add_catalog_test(status="mapped", extra={"test_class": "behavior_lock"})
            fx.write_lock_baseline("EVID_LOCK")
            fx.add_writer("failed", exit_code=1)

            with self.assertRaises(ProofError) as raised:
                fx.prover().lock_compare("TEST_RED", "EVID_LOCK")

            self.assertEqual(raised.exception.failure_kind, "metadata_error")

    def test_lock_refuses_blocked_current_run(self) -> None:
        with fixture() as fx:
            fx.add_case_file("CASE_FAIL", "CASE_BLOCKED")
            fx.add_catalog_test(status="mapped", extra={"test_class": "behavior_lock"})
            fx.add_writer("passed_two_cases", exit_code=0)
            baseline = fx.prover().lock_baseline("TEST_RED")
            fx.add_writer("blocked_case", exit_code=0)

            with self.assertRaises(ProofError) as raised:
                fx.prover().lock_compare("TEST_RED", baseline.record["id"])

            self.assertEqual(raised.exception.failure_kind, "metadata_error")
            self.assertIn("blocked cases", str(raised.exception))

    def test_lock_baseline_refuses_failed_blocked_or_inconsistent_runs(self) -> None:
        for mode, exit_code, expected_kind, expected in [
            ("failed", 1, "metadata_error", "failed cases"),
            ("blocked_case", 0, "metadata_error", "blocked cases"),
            ("failed", 0, "metadata_error", "failed cases"),
            ("passed", 1, "harness_panic", "command failed"),
        ]:
            with self.subTest(mode=mode, exit_code=exit_code), fixture() as fx:
                if mode == "blocked_case":
                    fx.add_case_file("CASE_FAIL", "CASE_BLOCKED")
                else:
                    fx.add_case_file("CASE_FAIL")
                fx.add_catalog_test(status="mapped", extra={"test_class": "behavior_lock"})
                fx.add_writer(mode, exit_code=exit_code)

                with self.assertRaises(ProofError) as raised:
                    fx.prover().lock_baseline("TEST_RED")

                self.assertEqual(raised.exception.failure_kind, expected_kind)
                self.assertIn(expected, str(raised.exception))

    def test_lock_baseline_refuses_case_file_less_rows(self) -> None:
        with fixture() as fx:
            fx.add_catalog_test(
                status="mapped",
                extra={"test_class": "behavior_lock"},
                with_case_file=False,
            )
            fx.add_writer("none", exit_code=0)

            with self.assertRaises(ProofError) as raised:
                fx.prover().lock_baseline("TEST_RED")

            self.assertEqual(raised.exception.failure_kind, "metadata_error")
            self.assertIn("case_file", str(raised.exception))


class fixture:
    def __enter__(self) -> "fixture":
        self.temp = tempfile.TemporaryDirectory()
        self.root = Path(self.temp.name)
        self.verification = self.root / "verification"
        self.case_dir = self.verification / "cases" / "bytecode_vm"
        self.artifact_dir = self.root / "target" / "gate-artifacts" / "cases"
        self.evidence_dir = self.root / "target" / "gate-artifacts" / "prove"
        self.case_dir.mkdir(parents=True)
        (self.verification).mkdir(exist_ok=True)
        self.case_file = self.case_dir / "TEST_CASES.toml"
        self.case_digest = ""
        self.run_id_counter = 0
        self.proof_revision_counter = 0
        self.tests: dict[str, dict[str, object]] = {}
        self.invariants: dict[str, dict[str, object]] = {
            "INV": {
                "schema_version": 1,
                "id": "INV",
                "title": "Proof invariant",
                "status": "gap_open",
                "proof_level": "S0",
            }
        }
        self.ignored: dict[str, dict[str, object]] = {}
        return self

    def initialize_git(self) -> None:
        (self.root / ".gitignore").write_text("/target/\n")
        (self.verification / "evidence-index.toml").write_text(
            "[[evidence]]\n"
            'schema_version = 1\n'
            'id = "EVID_EXISTING"\n'
            'proof_kind = "none"\n'
        )
        self.git("init", "-q")
        self.git("config", "user.email", "verification@example.invalid")
        self.git("config", "user.name", "Verification Tests")
        self.git("add", ".")
        self.git("commit", "-qm", "fixture")

    def git(self, *args: str) -> str:
        result = subprocess.run(
            ["git", *args],
            cwd=self.root,
            text=True,
            stdout=subprocess.PIPE,
            stderr=subprocess.PIPE,
            check=True,
        )
        return result.stdout.strip()

    def __exit__(self, *exc: object) -> None:
        self.temp.cleanup()

    def add_case_file(self, *case_ids: str) -> None:
        cases = "\n".join(
            textwrap.dedent(
                f"""\
                [[case]]
                id = "{case_id}"
                family = "happy_path"
                input = {{ scenario = "RUN" }}
                expect = {{ outcome = "accept_value", oracle_ref = "SPEC#case" }}
                """
            )
            for case_id in case_ids
        )
        self.case_file.write_text(
            textwrap.dedent(
                f"""\
                schema_version = 1
                id = "CASES_TEST"
                title = "Cases"
                area = "bytecode_vm"
                owner = "verification"
                status = "mapped"
                invariant = "INV"
                generator = "test"
                generator_digest = "sha256:generator"
                source_digest = "sha256:source"
                last_reviewed = "2026-07-09"

                {cases}
                """
            )
        )
        self.case_digest = sha256_file(self.case_file)

    def add_catalog_test(
        self,
        *,
        status: str,
        extra: dict[str, object] | None = None,
        with_case_file: bool = True,
    ) -> None:
        record: dict[str, object] = {
            "id": "TEST_RED",
            "title": "Red test",
            "test_class": "failing_regression",
            "area": "bytecode_vm",
            "path": "verification/cases/bytecode_vm/TEST_CASES.toml",
            "command": "python3 writer.py",
            "owner": "verification",
            "status": status,
            "invariants": ["INV"],
            "suite_tiers": ["veryquick"],
        }
        if with_case_file:
            record["case_file"] = str(self.case_file.relative_to(self.root))
            record["case_file_digest"] = self.case_digest
        if extra:
            record.update(extra)
        self.tests["TEST_RED"] = record

    def add_ignored_test(self) -> None:
        self.ignored["IGNORED_TEST_RED_001"] = {
            "id": "IGNORED_TEST_RED_001",
            "test_id": "TEST_RED",
            "reason": "ignored for test",
        }

    def write_evidence(
        self,
        evidence_id: str,
        *,
        proof_kind: str = "red",
        linked_tests: list[str] | None = None,
        case_file_digest: str | None = None,
        red_case_ids: list[str] | None = None,
        per_case_summary: list[str] | None = None,
        producer: str = "prove.py v1",
    ) -> None:
        record = {
            "schema_version": 1,
            "id": evidence_id,
            "title": "Generated red proof",
            "area": "bytecode_vm",
            "owner": "verification",
            "status": "mapped",
            "kind": "committed_file",
            "path": f"target/gate-artifacts/prove/{evidence_id}.toml",
            "command": "python3 writer.py",
            "commit": "f" * 40,
            "platform": "test",
            "date": "2026-07-09",
            "suite_id": "veryquick",
            "producer": producer,
            "generated_report_version": "prove-red-v1",
            "linked_invariants": ["INV"],
            "linked_tests": linked_tests if linked_tests is not None else ["TEST_RED"],
            "last_reviewed": "2026-07-09",
            "proof_kind": proof_kind,
            "failure_kind": "assertion_failure",
            "trust_verify_run_id": "red-run",
            "command_exit_status": 1,
            "red_case_ids": red_case_ids if red_case_ids is not None else ["CASE_FAIL"],
            "per_case_summary": (
                per_case_summary if per_case_summary is not None else ["CASE_FAIL:failed"]
            ),
            "case_file_digest": case_file_digest if case_file_digest is not None else self.case_digest,
            "proof_contract_digest": self.contract_digest(),
        }
        path = self.evidence_dir / f"{evidence_id}.toml"
        path.parent.mkdir(parents=True, exist_ok=True)
        path.write_text(render_test_evidence(record))

    def write_lock_baseline(
        self,
        evidence_id: str,
        *,
        linked_tests: list[str] | None = None,
        case_file_digest: str | None = None,
        command_exit_status: int = 0,
        per_case_summary: list[str] | None = None,
        case_result_digest_override: str | None = None,
        proof_kind: str = "lock_baseline",
        producer: str = "prove.py v1",
        command: str = "python3 writer.py",
    ) -> None:
        summary = per_case_summary if per_case_summary is not None else ["CASE_FAIL:passed"]
        result_digest = case_result_digest_override or case_result_digest(
            command_exit_status=command_exit_status,
            per_case_summary=summary,
        )
        record = {
            "schema_version": 1,
            "id": evidence_id,
            "title": "Generated lock baseline",
            "area": "bytecode_vm",
            "owner": "verification",
            "status": "mapped",
            "kind": "committed_file",
            "path": f"target/gate-artifacts/prove/{evidence_id}.toml",
            "command": command,
            "commit": "f" * 40,
            "platform": "test",
            "date": "2026-07-09",
            "suite_id": "veryquick",
            "producer": producer,
            "generated_report_version": "prove-lock-v1",
            "linked_invariants": ["INV"],
            "linked_tests": linked_tests if linked_tests is not None else ["TEST_RED"],
            "last_reviewed": "2026-07-09",
            "proof_kind": proof_kind,
            "failure_kind": "none",
            "trust_verify_run_id": "baseline-run",
            "command_exit_status": command_exit_status,
            "per_case_summary": summary,
            "case_file_digest": case_file_digest if case_file_digest is not None else self.case_digest,
            "case_result_digest": result_digest,
            "case_artifact_digest": "sha256:artifact",
            "proof_contract_digest": self.contract_digest(),
        }
        path = self.evidence_dir / f"{evidence_id}.toml"
        path.parent.mkdir(parents=True, exist_ok=True)
        path.write_text(render_test_evidence(record))

    def add_writer(self, mode: str, *, exit_code: int) -> None:
        writer = self.root / "writer.py"
        writer.write_text(
            textwrap.dedent(
                f"""\
                import json
                import os
                import pathlib
                import sys
                import time

                mode = {mode!r}
                artifact_dir = pathlib.Path(os.environ["TRUST_VERIFY_ARTIFACT_DIR"])
                artifact_dir.mkdir(parents=True, exist_ok=True)
                if mode == "sleep":
                    time.sleep(10)
                if mode != "none":
                    run_id = os.environ["TRUST_VERIFY_RUN_ID"]
                    if mode == "failed_wrong_run":
                        run_id = "wrong-run"
                    result = "failed" if mode in {{"failed", "failed_wrong_run"}} else "passed"
                    if mode == "skipped_case":
                        result = "skipped"
                    case_ids = ["CASE_FAIL"]
                    if mode == "unknown_case":
                        case_ids = ["CASE_UNKNOWN"]
                    elif mode == "duplicate_case":
                        case_ids = ["CASE_FAIL", "CASE_FAIL"]
                    elif mode == "missing_case":
                        case_ids = []
                    elif mode == "blocked_case":
                        case_ids = ["CASE_FAIL", "CASE_BLOCKED"]
                    elif mode == "errored_case":
                        case_ids = ["CASE_FAIL", "CASE_OTHER"]
                    elif mode == "changed_second_case":
                        case_ids = ["CASE_FAIL", "CASE_OTHER"]
                    elif mode == "passed_two_cases":
                        case_ids = ["CASE_FAIL", "CASE_BLOCKED"]
                    result_by_id = {{}}
                    if mode == "blocked_case":
                        result_by_id = {{"CASE_FAIL": "passed", "CASE_BLOCKED": "blocked"}}
                    elif mode == "errored_case":
                        result_by_id = {{"CASE_FAIL": "passed", "CASE_OTHER": "errored"}}
                    elif mode == "changed_second_case":
                        result_by_id = {{"CASE_FAIL": "passed", "CASE_OTHER": "failed"}}
                    artifact = {{
                        "schema_version": 1,
                        "test_id": os.environ["TRUST_VERIFY_TEST_ID"],
                        "case_file": "verification/cases/bytecode_vm/TEST_CASES.toml",
                        "case_file_digest": os.environ["TRUST_VERIFY_CASE_FILE_DIGEST"],
                        "helper_version": "test-helper",
                        "case_provenance_kind": (
                            "hand_authored_state_machine_v1"
                            if mode == "wrong_provenance"
                            else "generated_decision_table_v1"
                        ),
                        "trace_definition_digest": None,
                        "trust_verify_test_id": os.environ["TRUST_VERIFY_TEST_ID"],
                        "trust_verify_run_id": run_id,
                        "trust_verify_case_file_digest": os.environ["TRUST_VERIFY_CASE_FILE_DIGEST"],
                        "trust_verify_artifact_dir": os.environ["TRUST_VERIFY_ARTIFACT_DIR"],
                        "cases": [{{
                            "id": case_id,
                            "family": "happy_path",
                            "result": result_by_id.get(case_id, result),
                            "spec_gap_ref": None,
                            "observed_error": "assertion failed" if result == "failed" else None,
                            "observed_status": None,
                            "state_delta": "changed",
                            "before": None,
                            "after": None,
                        }} for case_id in case_ids],
                    }}
                    (artifact_dir / "TEST_RED.json").write_text(json.dumps(artifact, sort_keys=True))
                sys.exit({exit_code})
                """
            )
        )

    def write_artifact(self, path: Path, *, case_id: str, result: str, run_id: str) -> None:
        path.parent.mkdir(parents=True, exist_ok=True)
        path.write_text(
            json.dumps(
                {
                    "schema_version": 1,
                    "test_id": "TEST_RED",
                    "case_file": "verification/cases/bytecode_vm/TEST_CASES.toml",
                    "case_file_digest": self.case_digest,
                    "helper_version": "test-helper",
                    "case_provenance_kind": "generated_decision_table_v1",
                    "trace_definition_digest": None,
                    "trust_verify_test_id": "TEST_RED",
                    "trust_verify_run_id": run_id,
                    "trust_verify_case_file_digest": self.case_digest,
                    "trust_verify_artifact_dir": str(self.artifact_dir),
                    "cases": [{"id": case_id, "family": "happy_path", "result": result}],
                }
            )
        )

    def prover(
        self,
        *,
        command_timeout_seconds: float = 120,
        durable: bool = False,
    ) -> ProofProducer:
        self.proof_revision_counter += 1
        proof_revision = f"{self.proof_revision_counter:040x}"
        return ProofProducer(
            root=self.root,
            tests=self.tests,
            invariants=self.invariants,
            ignored_tests=self.ignored,
            evidence={},
            artifact_dir=self.artifact_dir,
            evidence_dir=self.evidence_dir,
            evidence_index_path=(self.verification / "evidence-index.toml") if durable else None,
            revision_provider=None if durable else (lambda: proof_revision),
            ancestry_checker=None if durable else (lambda _before, _after: True),
            run_id_factory=self.next_run_id,
            command_timeout_seconds=command_timeout_seconds,
            validate_metadata=False,
        )

    def default_durable_prover(self) -> ProofProducer:
        return ProofProducer(
            root=self.root,
            tests=self.tests,
            invariants=self.invariants,
            ignored_tests=self.ignored,
            evidence={},
            artifact_dir=self.artifact_dir,
            run_id_factory=self.next_run_id,
            command_timeout_seconds=120,
            validate_metadata=False,
        )

    def next_run_id(self) -> str:
        self.run_id_counter += 1
        return f"run-{self.run_id_counter}"

    def contract_digest(self) -> str:
        return proof_contract_digest(
            test=self.tests["TEST_RED"],
            invariants=self.invariants,
        )


def render_test_evidence(record: dict[str, object]) -> str:
    lines = ["[[evidence]]"]
    for key, value in record.items():
        lines.append(f"{key} = {render_toml_value(value)}")
    return "\n".join(lines) + "\n"


def render_toml_value(value: object) -> str:
    if isinstance(value, str):
        return json.dumps(value)
    if isinstance(value, int):
        return str(value)
    if isinstance(value, list):
        return "[" + ", ".join(render_toml_value(item) for item in value) + "]"
    raise TypeError(f"unsupported TOML test value {value!r}")


if __name__ == "__main__":
    unittest.main()
