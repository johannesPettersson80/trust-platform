"""Focused transaction tests for scanner-free mapping review."""

from __future__ import annotations

import copy
import json
import tomllib
import unittest
from contextlib import ExitStack
from pathlib import Path
from tempfile import TemporaryDirectory
from unittest import mock

from scripts.verification.code_spec_live import CodeSpecMetadata
from scripts.verification.code_spec_authority_selection import (
    AuthoritySelectionApplication,
)
from scripts.verification.focused_mapping_context import FocusedMappingContext
from scripts.verification.focused_mapping_review import (
    DEFAULT_RECEIPT_PATH,
    FocusedMappingReviewResult,
    execute_focused_review_batch,
)
from scripts.verification.mapping_catalog_updates import (
    render_denominator_document,
)
from scripts.verification.mapping_review_batch_tests import _proposal
from scripts.verification.mapping_review_batch import MappingReviewBatch
from scripts.verification.mapping_review_cli import main as review_cli_main
from scripts.verification.mapping_review_selection import canonical_digest
from scripts.verification.mapping_toml import render_proposals_toml
from scripts.verification.mapping_transaction import (
    apply_mapping_transaction as real_transaction,
)
from scripts.verification.test_catalog_denominator import (
    NONMAPPING_REASON_DEFINITIONS,
)
from scripts.verification.test_catalog_models import InferredTestFact


REPOSITORY_ROOT = Path(__file__).resolve().parents[2]
HEAD = "a" * 40
SCHEMA_PATHS = (
    "test-catalog-proposals.schema.json",
    "test-catalog-mapping-decisions.schema.json",
    "test-catalog-mapping-review-selection.schema.json",
    "test-catalog-mapping-review-input.schema.json",
    "test-catalog-denominator.schema.json",
)


class FocusedMappingReviewTests(unittest.TestCase):
    def test_authority_backed_acceptance_closes_in_one_transaction(self) -> None:
        with ReviewFixture() as fixture:
            application = fixture.authority_application(
                [fixture.proposals[0]["discovery_id"]]
            )
            with fixture.patches(), mock.patch(
                "scripts.verification.focused_mapping_review."
                "build_authority_selection_application",
                return_value=application,
            ) as build, mock.patch(
                "scripts.verification.focused_mapping_review."
                "_prebuilt_rust_assertion_inventories",
                side_effect=fixture.inventory_loader,
            ) as inventory:
                status = review_cli_main(
                    [
                        "--root",
                        str(fixture.root),
                        "--selection",
                        str(fixture.selection),
                        "--reviewed-date",
                        "2026-07-24",
                        "--authority-selection",
                        str(fixture.authority_selection),
                    ]
                )

            self.assertEqual(0, status)
            build.assert_called_once()
            inventory.assert_called_once()
            self.assertEqual(
                application.projected_code_map_bytes,
                fixture.code_map.read_bytes(),
            )
            receipt = json.loads(fixture.receipt.read_text())
            self.assertEqual(1, receipt["authority"]["row_count"])
            self.assertTrue(fixture.authority_manifest.exists())

    def test_authority_closure_rejects_unaccepted_discovery_atomically(
        self,
    ) -> None:
        with ReviewFixture() as fixture:
            application = fixture.authority_application(
                [
                    fixture.proposals[0]["discovery_id"],
                    fixture.proposals[1]["discovery_id"],
                ]
            )
            originals = fixture.transaction_originals()
            with fixture.patches(), mock.patch(
                "scripts.verification.focused_mapping_review."
                "build_authority_selection_application",
                return_value=application,
            ):
                with self.assertRaisesRegex(
                    ValueError, "not an accepted current proposal"
                ):
                    fixture.execute(
                        authority_selection_path=fixture.authority_selection
                    )

            for path, content in originals.items():
                self.assertEqual(content, path.read_bytes())
            self.assertFalse(fixture.receipt.exists())
            self.assertFalse(fixture.authority_manifest.exists())

    def test_authority_scope_rejects_already_cataloged_extra_discovery(
        self,
    ) -> None:
        extra = "DISC_" + "F" * 20
        with ReviewFixture() as fixture:
            fixture.catalog.write_text(
                "[[tests]]\n"
                'id = "TEST_AUTO_' + "F" * 20 + '"\n'
                'subject_kind = "generated_test"\n'
                f'discovery_id = "{extra}"\n'
            )
            application = fixture.authority_application(
                [fixture.proposals[0]["discovery_id"], extra]
            )
            with fixture.patches(), mock.patch(
                "scripts.verification.focused_mapping_review."
                "build_authority_selection_application",
                return_value=application,
            ):
                with self.assertRaisesRegex(
                    ValueError, "not an accepted current proposal"
                ):
                    fixture.execute(
                        authority_selection_path=fixture.authority_selection
                    )

            self.assertFalse(fixture.receipt.exists())
            self.assertFalse(fixture.authority_manifest.exists())

    def test_authority_scope_rejects_unrelated_code_and_invariants(self) -> None:
        with ReviewFixture() as fixture:
            application = fixture.authority_application(
                [fixture.proposals[0]["discovery_id"]]
            )
            application.manifest["rows"][0]["code_id"] = "CODE_" + "B" * 64
            with fixture.patches(), mock.patch(
                "scripts.verification.focused_mapping_review."
                "build_authority_selection_application",
                return_value=application,
            ):
                with self.assertRaisesRegex(
                    ValueError, "trace-covered spec-gap code fact"
                ):
                    fixture.execute(
                        authority_selection_path=fixture.authority_selection
                    )

        with ReviewFixture() as fixture:
            application = fixture.authority_application(
                [fixture.proposals[0]["discovery_id"]]
            )
            application.manifest["rows"][0]["invariant_ids"] = ["INV_2"]
            with fixture.patches(), mock.patch(
                "scripts.verification.focused_mapping_review."
                "build_authority_selection_application",
                return_value=application,
            ):
                with self.assertRaisesRegex(
                    ValueError, "differ from accepted review"
                ):
                    fixture.execute(
                        authority_selection_path=fixture.authority_selection
                    )

    def test_initial_catalog_snapshot_detects_planning_window_race(self) -> None:
        with ReviewFixture() as fixture:
            concurrent = b"tests = []\n# changed during metadata load\n"

            def drift(_root: Path) -> CodeSpecMetadata:
                fixture.catalog.write_bytes(concurrent)
                return CodeSpecMetadata(
                    invariants=fixture.invariants, tests={}
                )

            with fixture.patches(), mock.patch(
                "scripts.verification.focused_mapping_review."
                "load_code_spec_metadata",
                side_effect=drift,
            ):
                with self.assertRaisesRegex(
                    ValueError, "changed after initial snapshot"
                ):
                    fixture.execute()

            self.assertEqual(concurrent, fixture.catalog.read_bytes())
            self.assertFalse(fixture.receipt.exists())

    def test_source_change_during_assertion_refresh_fails_cas(self) -> None:
        with ReviewFixture() as fixture:
            concurrent = b"#[test]\nfn changed_during_assertions() {}\n"

            def drift(
                root: Path, facts: list[dict[str, object]]
            ) -> dict[str, dict[str, object]]:
                fixture.source.write_bytes(concurrent)
                return fixture.inventory_loader(root, facts)

            with fixture.patches():
                with self.assertRaisesRegex(
                    ValueError, "precondition failed"
                ):
                    execute_focused_review_batch(
                        root=fixture.root,
                        review_selection_path=fixture.selection,
                        reviewed_date="2026-07-24",
                        assertion_inventory_loader=drift,
                    )

            self.assertEqual(concurrent, fixture.source.read_bytes())
            self.assertFalse(fixture.receipt.exists())

    def test_stale_authority_input_and_disallowed_output_fail_closed(
        self,
    ) -> None:
        with ReviewFixture() as fixture:
            application = fixture.authority_application(
                [fixture.proposals[0]["discovery_id"]]
            )
            stale = b"schema_version = 1\n# authority changed\n"

            def drift(**_kwargs: object) -> AuthoritySelectionApplication:
                fixture.spec_sources.write_bytes(stale)
                return application

            with fixture.patches(), mock.patch(
                "scripts.verification.focused_mapping_review."
                "build_authority_selection_application",
                side_effect=drift,
            ):
                with self.assertRaisesRegex(
                    ValueError, "precondition failed"
                ):
                    fixture.execute(
                        authority_selection_path=fixture.authority_selection
                    )
            self.assertEqual(stale, fixture.spec_sources.read_bytes())
            self.assertFalse(fixture.receipt.exists())

        with ReviewFixture() as fixture:
            application = fixture.authority_application(
                [fixture.proposals[0]["discovery_id"]]
            )
            application.replacements[
                "target/gate-artifacts/verification/disallowed.json"
            ] = b"forbidden\n"
            application.expected_originals[
                "target/gate-artifacts/verification/disallowed.json"
            ] = None
            with fixture.patches(), mock.patch(
                "scripts.verification.focused_mapping_review."
                "build_authority_selection_application",
                return_value=application,
            ):
                with self.assertRaisesRegex(
                    ValueError, "exact allowlist"
                ):
                    fixture.execute(
                        authority_selection_path=fixture.authority_selection
                    )
            self.assertFalse(fixture.receipt.exists())

    def test_cli_defaults_to_focused_and_requires_flag_for_legacy(self) -> None:
        plan = MappingReviewBatch(
            replacements={}, accepted_test_ids=(), rejected_proposal_ids=()
        )
        focused_result = FocusedMappingReviewResult(
            batch=plan, receipt={}, replacements={}
        )
        with TemporaryDirectory() as temporary:
            root = Path(temporary)
            selection = root / "selection.json"
            with mock.patch(
                "scripts.verification.mapping_review_cli."
                "execute_focused_review_batch",
                return_value=focused_result,
            ) as focused, mock.patch(
                "scripts.verification.mapping_review_cli.execute_review_batch",
                return_value=plan,
            ) as legacy:
                self.assertEqual(
                    0,
                    review_cli_main(
                        [
                            "--root",
                            str(root),
                            "--selection",
                            str(selection),
                            "--reviewed-date",
                            "2026-07-24",
                        ]
                    ),
                )
                focused.assert_called_once()
                legacy.assert_not_called()

                self.assertEqual(
                    0,
                    review_cli_main(
                        [
                            "--root",
                            str(root),
                            "--selection",
                            str(selection),
                            "--reviewed-date",
                            "2026-07-24",
                            "--legacy-full-scan",
                        ]
                    ),
                )
                legacy.assert_called_once()

    def test_mixed_batch_commits_without_full_scan_or_broad_reports(self) -> None:
        with ReviewFixture() as fixture:
            with fixture.patches():
                result = fixture.execute()

            catalog = tomllib.loads(fixture.catalog.read_text())
            denominator = tomllib.loads(fixture.denominator.read_text())
            receipt = json.loads(fixture.receipt.read_text())
            self.assertEqual(1, len(catalog["tests"]))
            self.assertEqual(
                ["catalog_mapped", "reviewed_nonmapping"],
                [row["disposition"] for row in denominator["reviews"]],
            )
            self.assertEqual(1, len(result.batch.accepted_test_ids))
            self.assertEqual(1, len(result.batch.rejected_proposal_ids))
            self.assertFalse(receipt["broad_reports_regenerated"])
            self.assertEqual(
                "frozen_denominator_plus_live_selected_rust",
                receipt["basis"],
            )
            self.assertEqual(
                fixture.broad_before,
                fixture.broad_report.read_bytes(),
            )
            self.assertFalse(
                hasattr(
                    __import__(
                        "scripts.verification.focused_mapping_review",
                        fromlist=["scan_repository"],
                    ),
                    "scan_repository",
                )
            )

    def test_lost_update_precondition_preserves_concurrent_catalog_change(
        self,
    ) -> None:
        with ReviewFixture() as fixture:
            concurrent = b"tests = []\n# concurrent owner update\n"

            def race(**kwargs: object) -> None:
                fixture.catalog.write_bytes(concurrent)
                real_transaction(**kwargs)

            with fixture.patches(), mock.patch(
                "scripts.verification.focused_mapping_review."
                "apply_mapping_transaction",
                side_effect=race,
            ):
                with self.assertRaisesRegex(
                    ValueError, "precondition failed"
                ):
                    fixture.execute()

            self.assertEqual(concurrent, fixture.catalog.read_bytes())
            self.assertFalse(fixture.receipt.exists())

    def test_post_validation_failure_rolls_back_every_replacement(self) -> None:
        with ReviewFixture() as fixture:
            originals = fixture.transaction_originals()
            with fixture.patches(), mock.patch(
                "scripts.verification.focused_mapping_review."
                "validate_focused_post_state",
                return_value=["forced focused post-state failure"],
            ):
                with self.assertRaisesRegex(
                    ValueError, "forced focused post-state failure"
                ):
                    fixture.execute()

            for path, content in originals.items():
                self.assertEqual(content, path.read_bytes())
            self.assertFalse(fixture.receipt.exists())
            self.assertFalse(fixture.decisions.exists())

    def test_selected_source_race_is_a_read_only_precondition(self) -> None:
        with ReviewFixture() as fixture:
            concurrent = b"#[test]\nfn concurrent_case() {}\n"

            def race(**kwargs: object) -> None:
                fixture.source.write_bytes(concurrent)
                real_transaction(**kwargs)

            with fixture.patches(), mock.patch(
                "scripts.verification.focused_mapping_review."
                "apply_mapping_transaction",
                side_effect=race,
            ):
                with self.assertRaisesRegex(
                    ValueError, "precondition failed"
                ):
                    fixture.execute()

            self.assertEqual(concurrent, fixture.source.read_bytes())
            self.assertFalse(fixture.receipt.exists())

    def test_symlinked_quarantine_inputs_and_outputs_are_rejected(self) -> None:
        with ReviewFixture() as fixture:
            linked_selection = fixture.artifacts / "linked-selection.json"
            linked_selection.symlink_to(fixture.selection)
            with fixture.patches(), self.assertRaisesRegex(
                ValueError, "symlink"
            ):
                execute_focused_review_batch(
                    root=fixture.root,
                    review_selection_path=linked_selection,
                    reviewed_date="2026-07-24",
                    assertion_inventory_loader=fixture.inventory_loader,
                )

        with ReviewFixture() as fixture:
            victim = fixture.artifacts / "victim.json"
            victim.write_text("unchanged\n")
            receipt_link = fixture.artifacts / "linked-receipt.json"
            receipt_link.symlink_to(victim)
            with fixture.patches(), self.assertRaisesRegex(
                ValueError, "symlink"
            ):
                fixture.execute(receipt_path=receipt_link)
            self.assertEqual("unchanged\n", victim.read_text())


class ReviewFixture:
    def __enter__(self) -> "ReviewFixture":
        self.temporary = TemporaryDirectory()
        self.root = Path(self.temporary.name)
        self.verification = self.root / "verification"
        self.schemas = self.verification / "schemas"
        self.artifacts = self.root / "target/gate-artifacts/verification"
        self.invariants_root = (
            self.verification / "invariants/runtime_safety"
        )
        for directory in (
            self.schemas,
            self.artifacts,
            self.invariants_root,
            self.root / "crates/example/src",
        ):
            directory.mkdir(parents=True, exist_ok=True)
        for name in SCHEMA_PATHS:
            (self.schemas / name).write_bytes(
                (REPOSITORY_ROOT / "verification/schemas" / name).read_bytes()
            )
        self.source = self.root / "crates/example/src/lib.rs"
        self.source.write_text(
            "#[test]\nfn case_1() { assert!(true); }\n"
            "#[test]\nfn case_2() { assert!(true); }\n"
        )
        (self.root / "crates/example/Cargo.toml").write_text(
            '[package]\nname = "example"\nversion = "0.1.0"\n'
        )
        self.proposals = [_proposal(1), _proposal(2)]
        for proposal in self.proposals:
            proposal["path"] = "crates/example/src/lib.rs"
        authority_code_id = "CODE_" + "A" * 64
        self.proposals[0]["covered_code_ids"] = [authority_code_id]
        self.proposals[0]["specified_code_ids"] = []
        self.proposals[0]["spec_gap_code_ids"] = [authority_code_id]
        self.proposal_document = {
            "schema_version": 1,
            "generator": "test-catalog-proposals",
            "generator_version": "1",
            "source_commit": HEAD,
            "trace_digest": "sha256:" + "d" * 64,
            "candidate_report_digest": "sha256:" + "e" * 64,
            "last_generated": "2026-07-23T20:30:00Z",
            "demands": [],
            "demand_ranking": [],
            "structural_demand_ranking": [],
            "unmapped_code_demand_ranking": [],
            "proposals": self.proposals,
        }
        self.proposal_queue = (
            self.verification / "test-catalog-proposals.toml"
        )
        self.proposal_queue.write_text(
            render_proposals_toml(self.proposal_document)
        )
        self.catalog = self.verification / "test-catalog.toml"
        self.catalog.write_text("tests = []\n")
        (self.verification / "ignored-tests.toml").write_text(
            "schema_version = 1\nignored_tests = []\n"
        )
        self.facts = tuple(self._fact(proposal) for proposal in self.proposals)
        denominator_document = {
            "schema_version": 1,
            "id": "TEST_CATALOG_DENOMINATOR_REVIEW_V1",
            "title": "Focused review fixture",
            "denominator_basis": "all_live_existing_test_scanner_facts",
            "review_basis": "explicit_per_discovery_id_no_semantic_inference",
            "reason_definitions": NONMAPPING_REASON_DEFINITIONS,
            "last_reviewed": "2026-07-23",
            "reviews": [
                {
                    "discovery_id": fact.stable_id,
                    "discovery_source_kind": fact.source_kind,
                    "path": fact.path,
                    "name": fact.name,
                    "ignore_state": fact.ignore_state,
                    "disposition": "reviewed_nonmapping",
                    "reason_code": "no_reviewed_spec_or_invariant_binding",
                    "last_reviewed": "2026-07-23",
                }
                for fact in self.facts
            ],
        }
        self.denominator = (
            self.verification / "test-catalog-denominator.toml"
        )
        self.denominator.write_text(
            render_denominator_document(denominator_document)
        )
        self.invariants = {
            "INV_1": self._invariant("INV_1", "SPEC_1#exact"),
            "INV_2": self._invariant("INV_2", "SPEC_2#exact"),
        }
        for invariant_id in self.invariants:
            (self.invariants_root / f"{invariant_id}.toml").write_text(
                f'schema_version = 1\nid = "{invariant_id}"\n'
                'area = "verification"\ntests = []\n'
            )
        self.selection = self.artifacts / "selection.json"
        self.selection.write_text(
            json.dumps(
                {
                    "schema_version": 1,
                    "proposal_queue_digest": canonical_digest(
                        self.proposal_document
                    ),
                    "reviews": [
                        {
                            "proposal_index": 1,
                            "outcome": "accepted",
                            "reviewer_ids": ["reviewer-a"],
                            "selected_invariant_indices": [1],
                            "oracle_bindings": [
                                {
                                    "assertion_index": 1,
                                    "behavior_indices": [1],
                                }
                            ],
                            "setup_assertion_indices": [],
                            "non_oracle_assertion_indices": [],
                            "confirm_generated_catalog": True,
                            "requires_network": False,
                        },
                        {
                            "proposal_index": 2,
                            "outcome": "rejected",
                            "reviewer_ids": ["reviewer-a"],
                            "rejection_reason": (
                                "The candidate assertion does not establish "
                                "the proposed invariant."
                            ),
                        },
                    ],
                },
                sort_keys=True,
            )
        )
        self.receipt = self.root / DEFAULT_RECEIPT_PATH
        self.authority_selection = (
            self.artifacts / "authority-selection.json"
        )
        self.authority_selection.write_text('{"reviewed":true}\n')
        self.authority_manifest = (
            self.artifacts / "authority-selection.application.json"
        )
        self.code_map = self.verification / "code-spec-map.toml"
        self.code_map.write_bytes(
            b'[[mappings]]\ncode_id = "CODE_'
            + b"A" * 64
            + b'"\ndisposition = "spec_gap"\n'
        )
        self.spec_sources = self.verification / "spec-sources.toml"
        self.spec_sources.write_text("schema_version = 1\nspec_sources = []\n")
        self.decisions = (
            self.verification / "test-catalog-mapping-decisions.toml"
        )
        self.broad_report = (
            self.artifacts / "unmapped-test-debt-report.json"
        )
        self.broad_before = b'{"sentinel":"unchanged"}\n'
        self.broad_report.write_bytes(self.broad_before)
        return self

    def __exit__(self, *args: object) -> None:
        self.temporary.cleanup()

    def patches(self) -> ExitStack:
        stack = ExitStack()
        stack.enter_context(
            mock.patch(
                "scripts.verification.focused_mapping_review.git_head",
                return_value=HEAD,
            )
        )
        stack.enter_context(
            mock.patch(
                "scripts.verification.focused_mapping_review."
                "load_code_spec_metadata",
                return_value=CodeSpecMetadata(
                    invariants=self.invariants, tests={}
                ),
            )
        )
        stack.enter_context(
            mock.patch(
                "scripts.verification.focused_mapping_review."
                "build_focused_mapping_context",
                return_value=self.context(),
            )
        )
        stack.enter_context(
            mock.patch(
                "scripts.verification.focused_mapping_review.subprocess.run",
                side_effect=AssertionError(
                    "focused review invoked a subprocess"
                ),
            )
        )
        return stack

    def execute(self, **kwargs: object):
        return execute_focused_review_batch(
            root=self.root,
            review_selection_path=self.selection,
            reviewed_date="2026-07-24",
            assertion_inventory_loader=self.inventory_loader,
            **kwargs,
        )

    def context(self) -> FocusedMappingContext:
        selected = {
            str(proposal["discovery_id"]): {
                "discovery_id": proposal["discovery_id"],
                "discovery_source_kind": proposal[
                    "discovery_source_kind"
                ],
                "path": proposal["path"],
                "name": proposal["name"],
                "ignore_state": proposal["ignore_state"],
            }
            for proposal in self.proposals
        }
        denominator = tomllib.loads(self.denominator.read_text())
        return FocusedMappingContext(
            denominator=denominator,
            reviews_by_id={
                row["discovery_id"]: row for row in denominator["reviews"]
            },
            selected_facts=selected,
            selected_live_facts=self.facts,
            frozen_facts=self.facts,
        )

    def inventory_loader(
        self,
        _root: Path,
        facts: list[dict[str, object]],
    ) -> dict[str, dict[str, object]]:
        selected = {str(fact["discovery_id"]) for fact in facts}
        return {
            str(proposal["discovery_id"]): {
                "source_digest": proposal["test_source_digest"],
                "inventory_digest": proposal["assertion_inventory_digest"],
                "assertions": copy.deepcopy(proposal["assertions"]),
                "unresolved_assertion_helpers": copy.deepcopy(
                    proposal["unresolved_assertion_helpers"]
                ),
            }
            for proposal in self.proposals
            if str(proposal["discovery_id"]) in selected
        }

    def transaction_originals(self) -> dict[Path, bytes]:
        paths = [
            self.catalog,
            self.denominator,
            self.proposal_queue,
            self.invariants_root / "INV_1.toml",
            self.code_map,
        ]
        return {path: path.read_bytes() for path in paths}

    def authority_application(
        self, discovery_ids: list[object]
    ) -> AuthoritySelectionApplication:
        before = self.code_map.read_bytes()
        after = before.replace(b'"spec_gap"', b'"specified"')
        test_ids = [
            "TEST_AUTO_" + str(discovery_id).removeprefix("DISC_")
            for discovery_id in discovery_ids
        ]
        projected_path = (
            self.artifacts
            / "authority-selection.application.projected-code-spec-map.toml"
        )
        manifest = {
            "kind": "reviewed_code_spec_authority_selection",
            "reviewed_date": "2026-07-24",
            "row_count": 1,
            "selection_sha256": "sha256:" + "0" * 64,
            "code_map_before_sha256": "sha256:" + "1" * 64,
            "code_map_after_sha256": "sha256:" + "2" * 64,
            "canonical_apply_ready": False,
            "projected_code_map_path": (
                projected_path.relative_to(self.root).as_posix()
            ),
            "rows": [
                {
                    "code_id": "CODE_" + "A" * 64,
                    "spec_ref": "SPEC_1#exact",
                    "invariant_ids": ["INV_1"],
                    "discovery_ids": list(discovery_ids),
                    "test_ids": test_ids,
                }
            ],
        }
        manifest_bytes = (
            json.dumps(manifest, indent=2, sort_keys=True) + "\n"
        ).encode()
        quarantine = {
            self.authority_manifest.relative_to(self.root).as_posix(): (
                manifest_bytes
            ),
            projected_path.relative_to(self.root).as_posix(): after,
        }
        expected_originals = {
            "verification/code-spec-map.toml": before,
            "verification/spec-sources.toml": (
                self.spec_sources.read_bytes()
            ),
            self.authority_selection.relative_to(
                self.root
            ).as_posix(): self.authority_selection.read_bytes(),
            self.authority_manifest.relative_to(
                self.root
            ).as_posix(): None,
            projected_path.relative_to(self.root).as_posix(): None,
        }
        return AuthoritySelectionApplication(
            replacements={
                "verification/code-spec-map.toml": after,
                **quarantine,
            },
            quarantine_replacements=quarantine,
            manifest=manifest,
            code_map_before_bytes=before,
            projected_code_map_bytes=after,
            selection_path=self.authority_selection.relative_to(
                self.root
            ).as_posix(),
            output_path=self.authority_manifest.relative_to(
                self.root
            ).as_posix(),
            expected_originals=expected_originals,
        )

    @staticmethod
    def _invariant(invariant_id: str, oracle_ref: str) -> dict[str, object]:
        return {
            "id": invariant_id,
            "title": "Exact fixture behavior",
            "area": "verification",
            "owner": "verification",
            "risk": "maintenance",
            "gates": ["pr"],
            "oracle": {"ref": oracle_ref},
            "behavior": [
                {
                    "case_family": "happy_path",
                    "outcome": "accept_value",
                    "rationale": "The exact fixture value is preserved.",
                }
            ],
            "tests": [],
        }

    @staticmethod
    def _fact(proposal: dict[str, object]) -> InferredTestFact:
        return InferredTestFact(
            stable_id=str(proposal["discovery_id"]),
            native_id=(
                f"crates/example/src/lib.rs#{proposal['name']}"
            ),
            source_kind="rust_unit_test",
            name=str(proposal["name"]),
            path="crates/example/src/lib.rs",
            line=1,
            package="example",
            command_hint=str(proposal["command"]),
            command_hint_authority="exact",
            discovery_confidence="high",
            ignore_state="not_ignored",
            ignore_reason=None,
            reference_candidates=(),
        )


if __name__ == "__main__":
    unittest.main()
