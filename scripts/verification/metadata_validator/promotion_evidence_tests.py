"""Tests for invariant promotion evidence and evidence-scope honesty."""

from __future__ import annotations

import hashlib
import tempfile
import unittest
from pathlib import Path

from scripts.verification.metadata_validator.promotion_evidence import (
    validate_evidence_scope,
    validate_invariant_promotion_evidence,
)
from scripts.verification.metadata_validator.core import Validator


PATH = Path("verification/evidence-index.toml")


class EvidenceScopeTests(unittest.TestCase):
    def test_targeted_closing_proof_requires_explicit_scope(self) -> None:
        failures = validate_scope(evidence_record(proof_kind="green"))

        self.assert_contains(failures, "green proof must use proof_scope targeted")

    def test_targeted_scope_rejects_non_proof_evidence(self) -> None:
        failures = validate_scope(evidence_record(proof_scope="targeted"))

        self.assert_contains(
            failures, "proof_scope targeted requires a proof-producing proof_kind"
        )

    def test_broad_remote_scope_is_bound_to_successful_remote_gate_evidence(self) -> None:
        valid = evidence_record(
            proof_scope="broad_remote_gate",
            suite_id="nightly",
            platform="trust-builder-linux-x86_64",
            command_exit_status=0,
            producer="reviewed-gate v1",
        )
        self.assertEqual(validate_scope(valid), [])

        cases = (
            ({"suite_id": "supporting_local"}, "requires suite_id pr, nightly, or hardware_lab"),
            ({"platform": "local-linux-aarch64"}, "committed_file must use an exclusive trust-builder platform"),
            (
                {"platform": "local; trust-builder-linux-x86_64"},
                "committed_file must use an exclusive trust-builder platform",
            ),
            ({"command_exit_status": 1}, "requires command_exit_status = 0"),
            ({"kind": "release_object"}, "requires committed_file, ci_artifact, or lab_report evidence"),
        )
        for overrides, expected in cases:
            with self.subTest(overrides=overrides):
                failures = validate_scope({**valid, **overrides})
                self.assertTrue(any(expected in failure for failure in failures), failures)

    def test_release_public_scope_requires_release_object(self) -> None:
        valid = evidence_record(
            proof_scope="release_public",
            kind="release_object",
            suite_id="release",
            release_object="v1.2.3",
            url="https://example.invalid/releases/v1.2.3",
            producer="reviewed-gate v1",
        )
        self.assertEqual(validate_scope(valid), [])

        wrong_kind = validate_scope({**valid, "kind": "ci_artifact"})
        self.assert_contains(
            wrong_kind, "proof_scope release_public requires kind release_object"
        )

        wrong_suite = validate_scope({**valid, "suite_id": "nightly"})
        self.assert_contains(
            wrong_suite, "proof_scope release_public may only name suite_id release"
        )

    def test_broad_and_release_scope_require_suite_approved_producer(self) -> None:
        for scope, overrides in (
            (
                "broad_remote_gate",
                {
                    "suite_id": "nightly",
                    "platform": "trust-builder-linux-x86_64",
                    "command_exit_status": 0,
                },
            ),
            (
                "release_public",
                {
                    "kind": "release_object",
                    "suite_id": "release",
                    "release_object": "v1.2.3",
                    "url": "https://example.invalid/releases/v1.2.3",
                },
            ),
        ):
            with self.subTest(scope=scope):
                record = evidence_record(proof_scope=scope, **overrides)
                record["producer"] = "manual"

                failures = validate_scope(record)

                self.assert_contains(failures, "producer must be approved by suite")

    def test_suite_scoped_producer_cannot_borrow_another_suite_approval(self) -> None:
        record = evidence_record(
            proof_scope="broad_remote_gate",
            suite_id="nightly",
            platform="trust-builder-linux-x86_64",
            command_exit_status=0,
            producer="pr-only-gate v1",
        )

        failures = validate_scope(record)

        self.assert_contains(failures, "producer must be approved by suite nightly")

        targeted = evidence_record(
            proof_kind="green",
            proof_scope="targeted",
            suite_id="nightly",
            producer="pr-only-gate v1",
        )

        targeted_failures = validate_scope(targeted)

        self.assert_contains(
            targeted_failures, "producer must be approved by suite nightly"
        )

    def test_release_object_without_suite_id_uses_release_approval(self) -> None:
        record = evidence_record(
            proof_scope="release_public",
            kind="release_object",
            release_object="v1.2.3",
            url="https://example.invalid/releases/v1.2.3",
            producer="nightly-only-gate v1",
        )
        record.pop("suite_id")

        failures = validate_scope(record)

        self.assert_contains(failures, "producer must be approved by suite release")

        record["producer"] = "reviewed-gate v1"
        self.assertEqual(validate_scope(record), [])

    def test_scoped_closing_evidence_requires_clean_full_commit(self) -> None:
        for marker in ("dirty:0123456789ab", "0123456789ab"):
            with self.subTest(marker=marker):
                failures = validate_scope(
                    evidence_record(
                        proof_kind="green",
                        proof_scope="targeted",
                        commit=marker,
                    )
                )
                self.assertTrue(
                    any(
                        "requires a clean full 40-hex commit" in failure
                        for failure in failures
                    ),
                    failures,
                )

    def test_scoped_evidence_commit_must_resolve(self) -> None:
        failures: list[str] = []
        validate_evidence_scope(
            fail=lambda _path, message: failures.append(message),
            path=PATH,
            record=evidence_record(
                proof_kind="green",
                proof_scope="targeted",
            ),
            revision_exists=lambda _revision: False,
        )

        self.assert_contains(failures, "proof_scope targeted commit")
        self.assert_contains(failures, "does not resolve")

    def test_hostile_scope_types_fail_without_an_exception(self) -> None:
        for field, value in (("proof_scope", []), ("proof_kind", {}), ("suite_id", [])):
            with self.subTest(field=field):
                record = evidence_record(
                    proof_scope="broad_remote_gate",
                    suite_id="nightly",
                    platform="trust-builder-linux-x86_64",
                    command_exit_status=0,
                )
                record[field] = value

                failures = validate_scope(record)

                self.assertTrue(failures)

    def assert_contains(self, failures: list[str], expected: str) -> None:
        self.assertTrue(any(expected in failure for failure in failures), failures)


class InvariantPromotionEvidenceTests(unittest.TestCase):
    def test_test_written_requires_targeted_red_or_protective_evidence(self) -> None:
        invariant = invariant_record(
            "T1", ["EVID_RED"], status="test_written"
        )
        evidence = {
            "EVID_RED": evidence_record(
                evidence_id="EVID_RED",
                proof_kind="red",
                proof_scope="targeted",
                failure_kind="assertion_failure",
                case_file_digest="sha256:cases",
                command_exit_status=1,
                red_case_ids=["CASE_RED"],
                per_case_summary=["CASE_RED:failed"],
            )
        }

        self.assertEqual(validate_promotion(invariant, evidence), [])

        evidence["EVID_RED"]["proof_scope"] = "broad_remote_gate"
        failures = validate_promotion(invariant, evidence)
        self.assert_contains(
            failures, "status test_written requires targeted red/protective evidence"
        )

        evidence["EVID_RED"] = evidence_record(
            evidence_id="EVID_RED",
            proof_kind="red",
            proof_scope="targeted",
        )
        failures = validate_promotion(invariant, evidence)
        self.assert_contains(
            failures, "status test_written requires targeted red/protective evidence"
        )

    def test_implemented_requires_targeted_green_or_lock_proof(self) -> None:
        invariant = invariant_record(
            "I1", ["EVID_GREEN"], status="implemented"
        )
        evidence = {
            "EVID_GREEN": evidence_record(
                evidence_id="EVID_GREEN",
                proof_kind="green",
                proof_scope="targeted",
            )
        }

        self.assertEqual(validate_promotion(invariant, evidence), [])

        evidence["EVID_GREEN"]["proof_kind"] = "red"
        failures = validate_promotion(invariant, evidence)
        self.assert_contains(
            failures, "status implemented requires targeted green/lock proof"
        )

    def test_g1_requires_targeted_green_or_lock_proof(self) -> None:
        invariant = invariant_record("G1", ["EVID_TARGETED"])
        evidence = {
            "EVID_TARGETED": evidence_record(
                evidence_id="EVID_TARGETED",
                proof_kind="green",
                proof_scope="targeted",
            )
        }

        self.assertEqual(validate_promotion(invariant, evidence), [])

        evidence["EVID_TARGETED"]["proof_kind"] = "red"
        failures = validate_promotion(invariant, evidence)
        self.assert_contains(failures, "proof_level G1 requires targeted green/lock proof")

    def test_g2_requires_targeted_and_broad_remote_evidence(self) -> None:
        invariant = invariant_record("G2", ["EVID_TARGETED", "EVID_BROAD"])
        evidence = {
            "EVID_TARGETED": evidence_record(
                evidence_id="EVID_TARGETED",
                proof_kind="lock_compare",
                proof_scope="targeted",
            ),
            "EVID_BROAD": evidence_record(
                evidence_id="EVID_BROAD",
                proof_scope="broad_remote_gate",
                suite_id="nightly",
                platform="trust-builder-linux-x86_64",
                command_exit_status=0,
                producer="reviewed-gate v1",
            ),
        }

        self.assertEqual(validate_promotion(invariant, evidence), [])

        evidence["EVID_BROAD"]["linked_tests"] = []
        failures = validate_promotion(invariant, evidence)
        self.assert_contains(failures, "proof_level G2 requires broad remote gate evidence")

    def test_case_backed_broad_producer_requires_current_positive_execution_binding(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            case_path = root / "verification/cases/runtime_safety/TIMER.toml"
            case_path.parent.mkdir(parents=True)
            case_path.write_text(
                '[[case]]\nid = "CASE_ONE"\n\n[[case]]\nid = "CASE_TWO"\n'
            )
            case_digest = "sha256:" + hashlib.sha256(case_path.read_bytes()).hexdigest()
            invariant = invariant_record("G2", ["EVID_TARGETED", "EVID_BROAD"])
            current_test = {
                "id": "TEST_ONE",
                "status": "mapped",
                "suite_tiers": ["pr"],
                "discovery_id": "DISC_TEST_ONE",
                "discovery_source_kind": "rust_integration_test",
                "command": "cargo test -p trust-runtime --test timer TEST_ONE -- --exact",
                "case_file": "verification/cases/runtime_safety/TIMER.toml",
                "case_file_digest": case_digest,
            }
            evidence = {
                "EVID_TARGETED": evidence_record(
                    evidence_id="EVID_TARGETED",
                    proof_kind="green",
                    proof_scope="targeted",
                ),
                "EVID_BROAD": evidence_record(
                    evidence_id="EVID_BROAD",
                    proof_scope="broad_remote_gate",
                    suite_id="pr",
                    platform="trust-builder-linux-x86_64",
                    command_exit_status=0,
                    producer="broad-remote-gate.py v1",
                    executed_tests=[
                        {
                            "test_id": "TEST_ONE",
                            "discovery_id": "DISC_TEST_ONE",
                            "discovery_source_kind": "rust_integration_test",
                            "command": current_test["command"],
                            "case_file_digest": case_digest,
                            "per_case_summary": ["CASE_ONE:passed", "CASE_TWO:passed"],
                            "exit_status": 0,
                        }
                    ],
                ),
            }

            self.assertEqual(
                validate_promotion(
                    invariant,
                    evidence,
                    tests={"TEST_ONE": current_test},
                    root=root,
                ),
                [],
            )

            changed = dict(current_test, command="cargo test -p trust-runtime other")
            failures = validate_promotion(
                invariant,
                evidence,
                tests={"TEST_ONE": changed},
                root=root,
            )
            self.assert_contains(
                failures, "proof_level G2 requires broad remote gate evidence"
            )

            for summary in (
                ["CASE_ONE:passed"],
                ["CASE_ONE:passed", "CASE_ONE:passed"],
                ["CASE_ONE:passed", "CASE_TWO:passed", "INVENTED:passed"],
            ):
                with self.subTest(summary=summary):
                    evidence["EVID_BROAD"]["executed_tests"][0][
                        "per_case_summary"
                    ] = summary
                    failures = validate_promotion(
                        invariant,
                        evidence,
                        tests={"TEST_ONE": current_test},
                        root=root,
                    )
                    self.assert_contains(
                        failures, "proof_level G2 requires broad remote gate evidence"
                    )

            evidence["EVID_BROAD"]["executed_tests"][0]["per_case_summary"] = [
                "CASE_ONE:passed",
                "CASE_TWO:passed",
            ]
            case_path.write_text('[[case]]\nid = "CASE_CHANGED"\n')
            failures = validate_promotion(
                invariant,
                evidence,
                tests={"TEST_ONE": current_test},
                root=root,
            )
            self.assert_contains(
                failures, "proof_level G2 requires broad remote gate evidence"
            )

            case_path.write_text(
                '[[case]]\nid = "CASE_ONE"\n\n[[case]]\nid = "CASE_TWO"\n'
            )
            evidence["EVID_BROAD"].pop("executed_tests")
            failures = validate_promotion(
                invariant,
                evidence,
                tests={"TEST_ONE": current_test},
                root=root,
            )
            self.assert_contains(
                failures, "proof_level G2 requires broad remote gate evidence"
            )

    def test_case_backed_broad_evidence_rejects_hostile_tiers_and_current_ignore(self) -> None:
        invariant = invariant_record("G2", ["EVID_TARGETED", "EVID_BROAD"])
        current_test = {
            "id": "TEST_ONE",
            "status": "mapped",
            "suite_tiers": 7,
            "discovery_id": "DISC_TEST_ONE",
            "discovery_source_kind": "rust_integration_test",
            "command": "cargo test -p trust-runtime --test timer TEST_ONE -- --exact",
            "case_file": "verification/cases/runtime_safety/TIMER.toml",
            "case_file_digest": "sha256:" + "a" * 64,
        }
        evidence = {
            "EVID_TARGETED": evidence_record(
                evidence_id="EVID_TARGETED",
                proof_kind="green",
                proof_scope="targeted",
            ),
            "EVID_BROAD": evidence_record(
                evidence_id="EVID_BROAD",
                proof_scope="broad_remote_gate",
                suite_id="pr",
                platform="trust-builder-linux-x86_64",
                command_exit_status=0,
                producer="broad-remote-gate.py v1",
                executed_tests=[],
            ),
        }

        failures = validate_promotion(
            invariant,
            evidence,
            tests={"TEST_ONE": current_test},
        )
        self.assert_contains(failures, "proof_level G2 requires broad remote gate evidence")

        current_test["suite_tiers"] = ["pr"]
        failures = validate_promotion(
            invariant,
            evidence,
            tests={"TEST_ONE": current_test},
            ignored_tests={
                "IGNORED_ONE": {
                    "test_id": "TEST_ONE",
                    "discovery_id": "DISC_TEST_ONE",
                }
            },
        )
        self.assert_contains(failures, "proof_level G2 requires broad remote gate evidence")

    def test_multi_invariant_broad_union_qualifies_each_current_invariant_subset(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            case_path = root / "verification/cases/runtime_safety/TIMER.toml"
            case_path.parent.mkdir(parents=True)
            case_path.write_text('[[case]]\nid = "CASE_ONE"\n')
            digest = "sha256:" + hashlib.sha256(case_path.read_bytes()).hexdigest()
            tests = {
                test_id: {
                    "id": test_id,
                    "status": "mapped",
                    "suite_tiers": ["pr"],
                    "discovery_id": f"DISC_{test_id}",
                    "discovery_source_kind": "rust_integration_test",
                    "command": f"cargo test {test_id}",
                    "case_file": "verification/cases/runtime_safety/TIMER.toml",
                    "case_file_digest": digest,
                }
                for test_id in ("TEST_ONE", "TEST_TWO")
            }
            executed = [
                {
                    "test_id": test_id,
                    "discovery_id": f"DISC_{test_id}",
                    "discovery_source_kind": "rust_integration_test",
                    "command": tests[test_id]["command"],
                    "case_file_digest": digest,
                    "per_case_summary": ["CASE_ONE:passed"],
                    "exit_status": 0,
                }
                for test_id in ("TEST_ONE", "TEST_TWO")
            ]
            evidence = {
                "EVID_TARGETED": evidence_record(
                    evidence_id="EVID_TARGETED",
                    proof_kind="green",
                    proof_scope="targeted",
                ),
                "EVID_BROAD": evidence_record(
                    evidence_id="EVID_BROAD",
                    proof_scope="broad_remote_gate",
                    suite_id="pr",
                    platform="trust-builder-linux-x86_64",
                    command_exit_status=0,
                    producer="broad-remote-gate.py v1",
                    linked_invariants=["INV", "INV_B"],
                    linked_tests=["TEST_ONE", "TEST_TWO"],
                    executed_tests=executed,
                ),
            }

            self.assertEqual(
                validate_promotion(
                    invariant_record("G2", ["EVID_TARGETED", "EVID_BROAD"]),
                    evidence,
                    tests=tests,
                    root=root,
                ),
                [],
            )

            del tests["TEST_TWO"]
            failures = validate_promotion(
                invariant_record("G2", ["EVID_TARGETED", "EVID_BROAD"]),
                evidence,
                tests=tests,
                root=root,
            )
            self.assert_contains(
                failures, "proof_level G2 requires broad remote gate evidence"
            )

    def test_g2_rejects_broad_evidence_older_than_targeted_closing_proof(self) -> None:
        evidence = causal_evidence()
        evidence["EVID_TARGETED"]["commit"] = COMMIT_B
        evidence["EVID_BROAD"]["commit"] = COMMIT_A

        failures = validate_promotion(
            invariant_record("G2", ["EVID_TARGETED", "EVID_BROAD"]),
            evidence,
            ancestor_pairs={(COMMIT_A, COMMIT_B)},
        )

        self.assert_contains(
            failures,
            "proof_level G2 requires broad remote gate evidence at or after targeted proof",
        )

    def test_g2_accepts_equal_or_descendant_broad_evidence(self) -> None:
        for broad_commit, ancestor_pairs in (
            (COMMIT_A, set()),
            (COMMIT_B, {(COMMIT_A, COMMIT_B)}),
        ):
            with self.subTest(broad_commit=broad_commit):
                evidence = causal_evidence()
                evidence["EVID_TARGETED"]["commit"] = COMMIT_A
                evidence["EVID_BROAD"]["commit"] = broad_commit

                failures = validate_promotion(
                    invariant_record("G2", ["EVID_TARGETED", "EVID_BROAD"]),
                    evidence,
                    ancestor_pairs=ancestor_pairs,
                )

                self.assertEqual(failures, [])

    def test_r1_requires_targeted_broad_and_release_public_evidence(self) -> None:
        invariant = invariant_record(
            "R1", ["EVID_TARGETED", "EVID_BROAD", "EVID_RELEASE"]
        )
        evidence = {
            "EVID_TARGETED": evidence_record(
                evidence_id="EVID_TARGETED",
                proof_kind="green",
                proof_scope="targeted",
            ),
            "EVID_BROAD": evidence_record(
                evidence_id="EVID_BROAD",
                proof_scope="broad_remote_gate",
                suite_id="pr",
                kind="ci_artifact",
                platform="github-linux-x86_64",
                command_exit_status=0,
                producer="reviewed-gate v1",
            ),
            "EVID_RELEASE": evidence_record(
                evidence_id="EVID_RELEASE",
                proof_scope="release_public",
                kind="release_object",
                suite_id="release",
                release_object="v1.2.3",
                url="https://example.invalid/releases/v1.2.3",
                producer="reviewed-gate v1",
            ),
        }

        self.assertEqual(validate_promotion(invariant, evidence), [])

        evidence["EVID_RELEASE"]["linked_invariants"] = []
        failures = validate_promotion(invariant, evidence)
        self.assert_contains(failures, "proof_level R1 requires release/public evidence")

    def test_r1_rejects_release_evidence_older_than_causal_broad_gate(self) -> None:
        evidence = causal_evidence(include_release=True)
        evidence["EVID_TARGETED"]["commit"] = COMMIT_A
        evidence["EVID_BROAD"]["commit"] = COMMIT_B
        evidence["EVID_RELEASE"]["commit"] = COMMIT_A

        failures = validate_promotion(
            invariant_record(
                "R1", ["EVID_TARGETED", "EVID_BROAD", "EVID_RELEASE"]
            ),
            evidence,
            ancestor_pairs={(COMMIT_A, COMMIT_B)},
        )

        self.assert_contains(
            failures,
            "proof_level R1 requires release/public evidence at or after broad gate",
        )

    def test_r1_accepts_equal_or_descendant_release_evidence(self) -> None:
        for release_commit, ancestor_pairs in (
            (COMMIT_B, {(COMMIT_A, COMMIT_B)}),
            (COMMIT_C, {(COMMIT_A, COMMIT_B), (COMMIT_B, COMMIT_C)}),
        ):
            with self.subTest(release_commit=release_commit):
                evidence = causal_evidence(include_release=True)
                evidence["EVID_TARGETED"]["commit"] = COMMIT_A
                evidence["EVID_BROAD"]["commit"] = COMMIT_B
                evidence["EVID_RELEASE"]["commit"] = release_commit

                failures = validate_promotion(
                    invariant_record(
                        "R1", ["EVID_TARGETED", "EVID_BROAD", "EVID_RELEASE"]
                    ),
                    evidence,
                    ancestor_pairs=ancestor_pairs,
                )

                self.assertEqual(failures, [])

    def test_promotion_rejects_producer_approved_only_by_wrong_suite(self) -> None:
        evidence = causal_evidence()
        evidence["EVID_BROAD"]["producer"] = "pr-only-gate v1"

        failures = validate_promotion(
            invariant_record("G2", ["EVID_TARGETED", "EVID_BROAD"]),
            evidence,
            ancestor_pairs={(COMMIT_A, COMMIT_B)},
        )

        self.assert_contains(failures, "proof_level G2 requires broad remote gate evidence")

    def test_lower_proof_levels_do_not_require_promotion_evidence(self) -> None:
        for proof_level in ("S0", "S1", "D1", "T1", "D2", "I1"):
            with self.subTest(proof_level=proof_level):
                self.assertEqual(
                    validate_promotion(invariant_record(proof_level, []), {}),
                    [],
                )

    def test_promotion_evidence_must_be_bidirectionally_linked(self) -> None:
        invariant = invariant_record("G1", ["EVID_TARGETED"])
        evidence = {
            "EVID_TARGETED": evidence_record(
                evidence_id="EVID_TARGETED",
                proof_kind="green",
                proof_scope="targeted",
                linked_invariants=[],
            )
        }

        failures = validate_promotion(invariant, evidence)

        self.assert_contains(failures, "proof_level G1 requires targeted green/lock proof")

    def test_source_revision_targeted_proof_cannot_promote_current_invariant(self) -> None:
        evidence = {
            "EVID_HISTORICAL": evidence_record(
                evidence_id="EVID_HISTORICAL",
                proof_kind="green",
                proof_scope="targeted",
                proof_contract_binding="source_revision",
            )
        }

        failures = validate_promotion(
            invariant_record("G1", ["EVID_HISTORICAL"]),
            evidence,
        )

        self.assert_contains(failures, "proof_level G1 requires targeted green/lock proof")

    def test_hostile_reference_types_fail_without_an_exception(self) -> None:
        invariant = invariant_record("G1", [])
        invariant["evidence_refs"] = [{"not": "hashable"}]

        failures = validate_promotion(invariant, {})

        self.assert_contains(failures, "proof_level G1 requires targeted green/lock proof")

    def assert_contains(self, failures: list[str], expected: str) -> None:
        self.assertTrue(any(expected in failure for failure in failures), failures)


class FullValidatorPromotionEvidenceTests(unittest.TestCase):
    def test_full_validator_rejects_dirty_targeted_proof_scope(self) -> None:
        validator = Validator()
        validator.load_records()
        record = validator.evidence[
            "EVID_P1B_BYTECODE_VALIDATOR_MUTATION_SHARD_20260709"
        ]
        record["proof_kind"] = "red"
        record["proof_scope"] = "targeted"
        record["producer"] = "prove.py v1"
        record["commit"] = "dirty:0123456789ab"

        validator.validate()

        messages = [failure.message for failure in validator.failures]
        self.assertTrue(
            any(
                "proof_scope targeted requires a clean full 40-hex commit"
                in message
                for message in messages
            ),
            messages,
        )

    def test_r1_cannot_bypass_any_promotion_evidence_tier(self) -> None:
        validator = Validator()
        validator.load_records()
        validator.invariants["UI_STATUS_001"]["proof_level"] = "R1"

        validator.validate()

        messages = [failure.message for failure in validator.failures]
        for expected in (
            "proof_level R1 requires targeted green/lock proof",
            "proof_level R1 requires broad remote gate evidence",
            "proof_level R1 requires release/public evidence",
        ):
            with self.subTest(expected=expected):
                self.assertTrue(
                    any(expected in message for message in messages),
                    messages,
                )


COMMIT_A = "1111111111111111111111111111111111111111"
COMMIT_B = "2222222222222222222222222222222222222222"
COMMIT_C = "3333333333333333333333333333333333333333"


def suite_records() -> dict[str, dict[str, object]]:
    return {
        "supporting_local": {
            "id": "supporting_local",
            "approved_proof_producers": [],
        },
        "pr": {
            "id": "pr",
            "approved_proof_producers": [
                "reviewed-gate v1",
                "pr-only-gate v1",
                "broad-remote-gate.py v1",
            ],
        },
        "nightly": {
            "id": "nightly",
            "approved_proof_producers": [
                "reviewed-gate v1",
                "nightly-only-gate v1",
            ],
        },
        "release": {
            "id": "release",
            "approved_proof_producers": ["reviewed-gate v1"],
        },
    }


def validate_scope(record: dict[str, object]) -> list[str]:
    failures: list[str] = []
    validate_evidence_scope(
        fail=lambda _path, message: failures.append(message),
        path=PATH,
        record=record,
        revision_exists=lambda _revision: True,
        suites=suite_records(),
    )
    return failures


def validate_promotion(
    invariant: dict[str, object],
    evidence: dict[str, dict[str, object]],
    *,
    ancestor_pairs: set[tuple[str, str]] | None = None,
    tests: dict[str, dict[str, object]] | None = None,
    ignored_tests: dict[str, dict[str, object]] | None = None,
    root: Path | None = None,
) -> list[str]:
    failures: list[str] = []
    validate_invariant_promotion_evidence(
        fail=lambda _path, message: failures.append(message),
        path=Path("verification/invariants/runtime_safety/INV.toml"),
        invariant=invariant,
        evidence=evidence,
        suites=suite_records(),
        tests=tests or {},
        ignored_tests=ignored_tests or {},
        root=root,
        is_ancestor=lambda ancestor, descendant: (ancestor, descendant)
        in (ancestor_pairs or set()),
    )
    return failures


def causal_evidence(*, include_release: bool = False) -> dict[str, dict[str, object]]:
    evidence = {
        "EVID_TARGETED": evidence_record(
            evidence_id="EVID_TARGETED",
            proof_kind="green",
            proof_scope="targeted",
            commit=COMMIT_A,
        ),
        "EVID_BROAD": evidence_record(
            evidence_id="EVID_BROAD",
            proof_scope="broad_remote_gate",
            suite_id="nightly",
            platform="trust-builder-linux-x86_64",
            command_exit_status=0,
            producer="reviewed-gate v1",
            commit=COMMIT_B,
        ),
    }
    if include_release:
        evidence["EVID_RELEASE"] = evidence_record(
            evidence_id="EVID_RELEASE",
            proof_scope="release_public",
            kind="release_object",
            suite_id="release",
            release_object="v1.2.3",
            url="https://example.invalid/releases/v1.2.3",
            producer="reviewed-gate v1",
            commit=COMMIT_C,
        )
    return evidence


def invariant_record(
    proof_level: str,
    evidence_refs: list[str],
    *,
    status: str = "gap_open",
) -> dict[str, object]:
    return {
        "id": "INV",
        "status": status,
        "proof_level": proof_level,
        "tests": ["TEST_ONE"],
        "evidence_refs": evidence_refs,
    }


def evidence_record(
    *,
    evidence_id: str = "EVID",
    proof_kind: str = "none",
    proof_scope: str | None = None,
    linked_invariants: list[str] | None = None,
    linked_tests: list[str] | None = None,
    **overrides: object,
) -> dict[str, object]:
    record: dict[str, object] = {
        "id": evidence_id,
        "kind": "committed_file",
        "commit": "0123456789abcdef0123456789abcdef01234567",
        "platform": "local-linux-aarch64",
        "suite_id": "supporting_local",
        "producer": "prove.py v1",
        "proof_kind": proof_kind,
        "linked_invariants": ["INV"] if linked_invariants is None else linked_invariants,
        "linked_tests": ["TEST_ONE"] if linked_tests is None else linked_tests,
    }
    if proof_scope is not None:
        record["proof_scope"] = proof_scope
    record.update(overrides)
    return record


if __name__ == "__main__":
    unittest.main()
