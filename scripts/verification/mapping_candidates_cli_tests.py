"""Focused tests for candidate report and quarantine selection."""

from __future__ import annotations

import hashlib
import json
import unittest
from pathlib import Path
from tempfile import TemporaryDirectory
from unittest import mock

from scripts.verification.focused_mapping_candidates import (
    _selected_input_preconditions,
    _validate_candidate_output_separation,
    generate_focused_mapping_candidates,
)
from scripts.verification.mapping_candidates_cli import (
    build_parser,
    build_candidate_document,
    generate_pilot_candidate_document,
    generate_pilot_candidates,
    initial_decision_document,
    main,
    select_quarantine_candidates,
)


class MappingCandidatesCliTests(unittest.TestCase):
    def test_focused_batch_accepts_multiple_tests_in_one_source_file(
        self,
    ) -> None:
        with TemporaryDirectory() as temporary:
            root = Path(temporary)
            source = root / "crates/example/src/lib.rs"
            source.parent.mkdir(parents=True)
            source.write_text(
                "#[test]\nfn first() {}\n#[test]\nfn second() {}\n"
            )
            manifest = root / "crates/example/Cargo.toml"
            manifest.write_text(
                '[package]\nname = "example"\nversion = "0.1.0"\n'
            )
            first = "DISC_" + "1" * 20
            second = "DISC_" + "2" * 20
            source_digest = (
                "sha256:" + hashlib.sha256(source.read_bytes()).hexdigest()
            )
            with mock.patch(
                "scripts.verification.focused_mapping_candidates."
                "invoke_assertion_inventories",
                return_value={
                    first: {
                        "source_digests": {
                            "crates/example/src/lib.rs": source_digest
                        }
                    },
                    second: {
                        "source_digests": {
                            "crates/example/src/lib.rs": source_digest
                        }
                    },
                },
            ):
                result = _selected_input_preconditions(
                    root=root,
                    requested=[first, second],
                    trace={
                        "tests": [
                            {
                                "discovery_id": first,
                                "covered_code_ids": [],
                            },
                            {
                                "discovery_id": second,
                                "covered_code_ids": [],
                            },
                        ]
                    },
                    metadata_preconditions={
                        "verification/test-catalog-denominator.toml": (
                            (
                                "[[reviews]]\n"
                                f'discovery_id = "{first}"\n'
                                'discovery_source_kind = "rust_unit_test"\n'
                                'path = "crates/example/src/lib.rs"\n'
                                'name = "first"\n'
                                "[[reviews]]\n"
                                f'discovery_id = "{second}"\n'
                                'discovery_source_kind = "rust_unit_test"\n'
                                'path = "crates/example/src/lib.rs"\n'
                                'name = "second"\n'
                            ).encode()
                        ),
                        "verification/code-spec-map.toml": b"mappings = []\n",
                    },
                )

            self.assertEqual(
                {
                    "crates/example/Cargo.toml",
                    "crates/example/src/lib.rs",
                },
                set(result),
            )

    def test_focused_preconditions_capture_live_rust_topology_closure(
        self,
    ) -> None:
        with TemporaryDirectory() as temporary:
            root = Path(temporary)
            crate = root / "crates/example"
            (crate / "src/database").mkdir(parents=True)
            (crate / "tests").mkdir()
            manifest = crate / "Cargo.toml"
            manifest.write_text(
                '[package]\nname = "example"\nversion = "0.1.0"\n'
            )
            test_relative = "crates/example/tests/check.rs"
            test_source = root / test_relative
            test_source.write_text("#[test]\nfn check() {}\n")
            root_relative = "crates/example/src/lib.rs"
            parent_relative = "crates/example/src/database.rs"
            fragment_relative = (
                "crates/example/src/database/database_part_01.rs"
            )
            (root / root_relative).write_text("mod database;\n")
            (root / parent_relative).write_text(
                'include!("database/database_part_01.rs");\n'
            )
            (root / fragment_relative).write_text("fn live() {}\n")
            discovery_id = "DISC_" + "3" * 20
            code_id = "CODE_" + "4" * 20
            source_digest = (
                "sha256:" + hashlib.sha256(test_source.read_bytes()).hexdigest()
            )
            topology_sources = {
                relative: (root / relative).read_bytes()
                for relative in (
                    root_relative,
                    parent_relative,
                    fragment_relative,
                )
            }

            with mock.patch(
                "scripts.verification.focused_mapping_candidates."
                "invoke_assertion_inventories",
                return_value={
                    discovery_id: {
                        "source_digests": {test_relative: source_digest}
                    }
                },
            ), mock.patch(
                "scripts.verification.focused_mapping_candidates."
                "focused_rust_topology_sources",
                return_value=topology_sources,
            ) as inventory:
                result = _selected_input_preconditions(
                    root=root,
                    requested=[discovery_id],
                    trace={
                        "tests": [
                            {
                                "discovery_id": discovery_id,
                                "covered_code_ids": [code_id],
                            }
                        ]
                    },
                    metadata_preconditions={
                        "verification/test-catalog-denominator.toml": (
                            (
                                "[[reviews]]\n"
                                f'discovery_id = "{discovery_id}"\n'
                                'discovery_source_kind = "rust_integration_test"\n'
                                f'path = "{test_relative}"\n'
                                'name = "check"\n'
                            ).encode()
                        ),
                        "verification/code-spec-map.toml": (
                            (
                                "[[mappings]]\n"
                                f'code_id = "{code_id}"\n'
                                'source_kind = "rust"\n'
                                f'path = "{fragment_relative}"\n'
                            ).encode()
                        ),
                    },
                )

            inventory.assert_called_once()
            self.assertEqual(
                {
                    "crates/example/Cargo.toml",
                    test_relative,
                    root_relative,
                    parent_relative,
                    fragment_relative,
                },
                set(result),
            )

    def test_candidate_outputs_reject_raw_and_authority_input_overlap(self) -> None:
        for label, authenticated in (
            (
                "raw trace evidence",
                {"target/gate-artifacts/trace/attempts/x/attempt.json"},
            ),
            (
                "authority artifacts",
                {"target/gate-artifacts/authority/candidate-card.json"},
            ),
        ):
            output = next(iter(authenticated))
            with self.subTest(label=label), self.assertRaisesRegex(
                ValueError, "collide with authenticated"
            ):
                _validate_candidate_output_separation(
                    outputs={output},
                    authenticated_inputs=authenticated,
                    label=label,
                )

    def test_omitted_production_mode_fails_without_invoking_a_generator(
        self,
    ) -> None:
        empty_candidates = {"candidates": [], "demands": []}
        empty_proposals = {"proposals": []}
        with mock.patch(
            "scripts.verification.mapping_candidates_cli."
            "generate_focused_mapping_candidates",
            return_value=(empty_candidates, empty_proposals),
        ) as focused, mock.patch(
            "scripts.verification.mapping_candidates_cli.generate_mapping_candidates",
            return_value=(empty_candidates, empty_proposals),
        ) as legacy:
            exit_code = main(["--trace", "trace.json"])

        self.assertEqual(2, exit_code)
        focused.assert_not_called()
        legacy.assert_not_called()

    def test_focused_route_requires_explicit_rust_option_name(self) -> None:
        discovery_id = "DISC_" + "A" * 20
        args = build_parser().parse_args(
            [
                "--trace",
                "trace.json",
                "--focused-rust-discovery-id",
                discovery_id,
            ]
        )

        self.assertEqual([discovery_id], args.focused_rust_discovery_id)

    def test_focused_route_accepts_authenticated_trace_receipt(self) -> None:
        args = build_parser().parse_args(
            [
                "--trace",
                "trace.json",
                "--focused-rust-discovery-id",
                "DISC_" + "A" * 20,
                "--authenticated-trace-receipt",
                "target/gate-artifacts/trace-receipt.json",
            ]
        )

        self.assertEqual(
            Path("target/gate-artifacts/trace-receipt.json"),
            args.authenticated_trace_receipt,
        )

    def test_focused_route_exposes_trace_authentication_paths(self) -> None:
        help_text = build_parser().format_help()

        for option in (
            "--authority-routes",
            "--trace-root",
            "--cargo-target-dir",
            "--llvm-tools-dir",
            "--temporary-dir",
        ):
            with self.subTest(option=option):
                self.assertIn(option, help_text)

    def test_focused_route_requires_every_trace_authentication_path(
        self,
    ) -> None:
        discovery_id = "DISC_" + "A" * 20
        path_options = (
            ("--trace-root", "trace-root"),
            ("--cargo-target-dir", "cargo-target"),
            ("--llvm-tools-dir", "llvm-tools"),
            ("--temporary-dir", "temporary"),
        )
        empty_candidates = {"candidates": [], "demands": []}
        empty_proposals = {"proposals": []}
        for missing_option, _ in path_options:
            arguments = [
                "--trace",
                "trace.json",
                "--focused-rust-discovery-id",
                discovery_id,
            ]
            for option, value in path_options:
                if option != missing_option:
                    arguments.extend((option, value))
            with self.subTest(missing=missing_option), mock.patch(
                "scripts.verification.mapping_candidates_cli."
                "generate_focused_mapping_candidates",
                return_value=(empty_candidates, empty_proposals),
            ) as focused, mock.patch(
                "scripts.verification.mapping_candidates_cli."
                "generate_mapping_candidates",
                return_value=(empty_candidates, empty_proposals),
            ) as legacy:
                exit_code = main(arguments)

            self.assertEqual(2, exit_code)
            focused.assert_not_called()
            legacy.assert_not_called()

    def test_focused_ids_invoke_only_focused_generator(self) -> None:
        discovery_id = "DISC_" + "A" * 20
        empty_candidates = {"candidates": [], "demands": []}
        empty_proposals = {"proposals": []}
        with mock.patch(
            "scripts.verification.mapping_candidates_cli."
            "generate_focused_mapping_candidates",
            return_value=(empty_candidates, empty_proposals),
        ) as focused, mock.patch(
            "scripts.verification.mapping_candidates_cli.generate_mapping_candidates",
            return_value=(empty_candidates, empty_proposals),
        ) as legacy:
            exit_code = main(
                [
                    "--root",
                    "/repository",
                    "--trace",
                    "trace.json",
                    "--focused-rust-discovery-id",
                    discovery_id,
                    "--authority-routes",
                    "target/gate-artifacts/authority-routes.json",
                    "--trace-root",
                    "target/gate-artifacts/focused-trace",
                    "--cargo-target-dir",
                    "target/cargo",
                    "--llvm-tools-dir",
                    "target/llvm-tools",
                    "--temporary-dir",
                    "target/temporary",
                ]
            )

        self.assertEqual(0, exit_code)
        legacy.assert_not_called()
        self.assertEqual(
            [discovery_id],
            focused.call_args.kwargs["rust_discovery_ids"],
        )
        self.assertEqual(
            Path("/repository/target/gate-artifacts/focused-trace"),
            focused.call_args.kwargs["trace_root"],
        )
        self.assertEqual(
            Path("/repository/target/cargo"),
            focused.call_args.kwargs["cargo_target_dir"],
        )
        self.assertEqual(
            Path("/repository/target/llvm-tools"),
            focused.call_args.kwargs["llvm_tools_dir"],
        )
        self.assertEqual(
            Path("/repository/target/temporary"),
            focused.call_args.kwargs["temporary_dir"],
        )
        self.assertEqual(
            Path("/repository/target/gate-artifacts/authority-routes.json"),
            focused.call_args.kwargs["authority_routes_path"],
        )

    def test_authority_routes_require_focused_generation(self) -> None:
        empty_candidates = {"candidates": [], "demands": []}
        empty_proposals = {"proposals": []}
        with mock.patch(
            "scripts.verification.mapping_candidates_cli."
            "generate_focused_mapping_candidates",
            return_value=(empty_candidates, empty_proposals),
        ) as focused, mock.patch(
            "scripts.verification.mapping_candidates_cli.generate_mapping_candidates",
            return_value=(empty_candidates, empty_proposals),
        ) as legacy:
            exit_code = main(
                [
                    "--trace",
                    "trace.json",
                    "--authority-routes",
                    "target/gate-artifacts/authority-routes.json",
                ]
            )

        self.assertEqual(2, exit_code)
        focused.assert_not_called()
        legacy.assert_not_called()

    def test_legacy_repository_scan_requires_explicit_option(self) -> None:
        self.assertIn("--legacy-full-scan", build_parser().format_help())

    def test_explicit_legacy_full_scan_invokes_only_legacy_generator(
        self,
    ) -> None:
        empty_candidates = {"candidates": [], "demands": []}
        empty_proposals = {"proposals": []}
        with mock.patch(
            "scripts.verification.mapping_candidates_cli."
            "generate_focused_mapping_candidates",
            return_value=(empty_candidates, empty_proposals),
        ) as focused, mock.patch(
            "scripts.verification.mapping_candidates_cli.generate_mapping_candidates",
            return_value=(empty_candidates, empty_proposals),
        ) as legacy:
            exit_code = main(
                ["--trace", "trace.json", "--legacy-full-scan"]
            )

        self.assertEqual(0, exit_code)
        focused.assert_not_called()
        legacy.assert_called_once()

    def test_legacy_full_scan_rejects_other_generation_modes(self) -> None:
        discovery_id = "DISC_" + "A" * 20
        empty_candidates = {"candidates": [], "demands": []}
        empty_proposals = {"proposals": []}
        for conflicting_option in (
            ["--focused-rust-discovery-id", discovery_id],
            ["--pilot-only"],
        ):
            with self.subTest(option=conflicting_option), mock.patch(
                "scripts.verification.mapping_candidates_cli."
                "generate_focused_mapping_candidates",
                return_value=(empty_candidates, empty_proposals),
            ) as focused, mock.patch(
                "scripts.verification.mapping_candidates_cli."
                "generate_mapping_candidates",
                return_value=(empty_candidates, empty_proposals),
            ) as legacy, mock.patch(
                "scripts.verification.mapping_candidates_cli."
                "generate_pilot_candidates",
                return_value=empty_candidates,
            ) as pilot:
                exit_code = main(
                    [
                        "--trace",
                        "trace.json",
                        "--legacy-full-scan",
                        *conflicting_option,
                    ]
                )

            self.assertEqual(2, exit_code)
            focused.assert_not_called()
            legacy.assert_not_called()
            pilot.assert_not_called()

    def test_pilot_only_rejects_catalog_plane_output_before_loading_trace(
        self,
    ) -> None:
        with TemporaryDirectory() as temporary:
            root = Path(temporary)
            with self.assertRaisesRegex(ValueError, "artifact quarantine"):
                generate_pilot_candidates(
                    root=root,
                    trace_path=root / "missing-trace.json",
                    candidates_path=root / "verification/test-catalog.toml",
                )

    def test_decision_ledger_initialization_is_generated_not_hand_authored(self) -> None:
        document = initial_decision_document("a" * 40)

        self.assertEqual("test-catalog-mapping-review", document.get("generator"))
        self.assertEqual([], document.get("proposal_snapshots"))
        self.assertEqual([], document.get("decisions"))
        self.assertEqual("a" * 40, document.get("source_commit"))

    def test_candidate_document_adds_generated_provenance(self) -> None:
        document = build_candidate_document(
            source_commit="a" * 40,
            trace_digest="sha256:" + "1" * 64,
            analysis={"candidates": [], "demands": []},
        )

        self.assertEqual("test-mapping-candidates", document["generator"])
        self.assertEqual([], document["candidates"])
        self.assertEqual([], document["demands"])

    def test_pilot_candidate_generation_does_not_scan_or_mutate_quarantine(
        self,
    ) -> None:
        trace = {
            "source_commit": "a" * 40,
            "tests": [],
        }
        with mock.patch(
            "scripts.verification.mapping_candidates_cli.analyze_mapping_candidates",
            return_value={"candidates": [], "demands": []},
        ), mock.patch(
            "scripts.verification.mapping_candidates_cli.validated_live_code_facts",
            return_value={},
        ):
            document = generate_pilot_candidate_document(
                root=mock.Mock(),
                trace=trace,
                trace_digest="sha256:" + "1" * 64,
                metadata=mock.Mock(mappings={}, invariants={}),
            )

        self.assertEqual("test-mapping-candidates", document["generator"])
        self.assertEqual([], document["candidates"])

    def test_only_nonmapped_reviewed_candidates_enter_quarantine(self) -> None:
        candidates = [
            {"discovery_id": "DISC_MAPPED"},
            {"discovery_id": "DISC_PENDING"},
            {"discovery_id": "DISC_WRONG_REASON"},
            {
                "discovery_id": "DISC_CODE_MAP_STALE",
                "unmapped_code_ids": ["CODE_LIVE_UNMAPPED"],
            },
        ]
        selected = select_quarantine_candidates(
            candidates,
            catalog_discovery_ids={"DISC_MAPPED"},
            denominator_reviews={
                "DISC_MAPPED": {"disposition": "catalog_mapped"},
                "DISC_PENDING": {"disposition": "reviewed_nonmapping"},
                "DISC_WRONG_REASON": {"disposition": "catalog_mapped"},
                "DISC_CODE_MAP_STALE": {"disposition": "reviewed_nonmapping"},
            },
        )

        self.assertEqual(["DISC_PENDING"], [row["discovery_id"] for row in selected])

    def test_focused_production_route_uses_one_frozen_snapshot_during_aba(
        self,
    ) -> None:
        discovery_id = "DISC_" + "A" * 20
        trace = {
            "source_commit": "a" * 40,
            "tests": [
                {
                    "discovery_id": discovery_id,
                    "covered_code_ids": [],
                }
            ],
        }
        candidate_document = build_candidate_document(
            source_commit="a" * 40,
            trace_digest="sha256:" + "1" * 64,
            analysis={
                "candidates": [
                    {
                        "discovery_id": discovery_id,
                        "unmapped_code_ids": [],
                    }
                ],
                "demands": [],
            },
        )
        context = mock.Mock(
            reviews_by_id={
                discovery_id: {"disposition": "reviewed_nonmapping"}
            },
            selected_facts={
                discovery_id: {
                    "discovery_id": discovery_id,
                    "discovery_source_kind": "rust_unit_test",
                    "path": "crates/example/src/lib.rs",
                    "name": "example",
                    "ignore_state": "not_ignored",
                }
            },
        )
        metadata = mock.Mock(tests={}, mappings={}, invariants={})
        with TemporaryDirectory() as temporary:
            root = Path(temporary)
            artifact_root = root / "target/gate-artifacts"
            artifact_root.mkdir(parents=True)
            trace_path = artifact_root / "trace.json"
            trace_path.write_text(json.dumps(trace))
            candidates_path = artifact_root / "candidates.json"
            verification = root / "verification"
            schemas = verification / "schemas"
            schemas.mkdir(parents=True)
            proposals_path = verification / "test-catalog-proposals.toml"
            decisions_path = (
                verification / "test-catalog-mapping-decisions.toml"
            )
            proposals_path.write_text("schema_version = 1\n")
            decisions_path.write_text("schema_version = 1\n")
            source = root / "crates/example/src/lib.rs"
            source.parent.mkdir(parents=True)
            frozen_source = b"#[test]\nfn example() { assert!(true); }\n"
            source.write_bytes(frozen_source)
            snapshot_roots: list[Path] = []

            def validate_live(
                source_root: Path,
                *_args: object,
                workspace_root: Path,
            ) -> dict[str, object]:
                self.assertEqual(root, workspace_root)
                self.assertEqual(frozen_source, source_root.joinpath(
                    "crates/example/src/lib.rs"
                ).read_bytes())
                source.write_text(
                    "#[test]\nfn transient_forgery() { assert!(false); }\n"
                )
                snapshot_roots.append(source_root)
                return {}

            def assertions_from_snapshot(
                source_root: Path,
                *_args: object,
                **kwargs: object,
            ) -> dict[str, object]:
                self.assertEqual(root, kwargs["workspace_root"])
                self.assertEqual(frozen_source, source_root.joinpath(
                    "crates/example/src/lib.rs"
                ).read_bytes())
                self.assertIn(b"transient_forgery", source.read_bytes())
                snapshot_roots.append(source_root)
                return {}

            def proposals_from_snapshot(
                *,
                root: Path,
                **_kwargs: object,
            ) -> list[dict[str, str]]:
                self.assertEqual(frozen_source, root.joinpath(
                    "crates/example/src/lib.rs"
                ).read_bytes())
                self.assertIn(b"transient_forgery", source.read_bytes())
                snapshot_roots.append(root)
                source.write_bytes(frozen_source)
                return [
                    {
                        "discovery_id": discovery_id,
                        "test_source_digest": "sha256:"
                        + hashlib.sha256(frozen_source).hexdigest(),
                    }
                ]
            for name in (
                "test-coverage-trace.schema.json",
                "mapping-candidates.schema.json",
                "test-catalog-proposals.schema.json",
            ):
                (schemas / name).write_text("{}")
            with mock.patch(
                "scripts.verification.focused_mapping_candidates.scan_repository",
                side_effect=AssertionError("full scanner invoked"),
                create=True,
            ), mock.patch(
                "scripts.verification.focused_mapping_candidates._load_json",
                side_effect=lambda path: (
                    trace if path.name == "trace.json" else {}
                ),
            ), mock.patch(
                "scripts.verification.focused_mapping_candidates.validate_json_schema_instance",
                return_value=[],
            ), mock.patch(
                "scripts.verification.focused_mapping_candidates._git_head",
                return_value="a" * 40,
            ), mock.patch(
                "scripts.verification.focused_mapping_candidates._metadata_preconditions",
                return_value={
                    "verification/code-spec-map.toml": b"map",
                    "verification/test-catalog-denominator.toml": (
                        b"schema_version = 1\nreviews = []\n"
                    ),
                    "verification/ignored-tests.toml": b"ignored_tests = []\n",
                    "verification/schemas/test-coverage-trace.schema.json": b"{}",
                    "verification/schemas/mapping-candidates.schema.json": b"{}",
                    "verification/schemas/test-catalog-proposals.schema.json": b"{}",
                    "verification/schemas/test-catalog-mapping-decisions.schema.json": b"{}",
                    "verification/schemas/test-catalog-denominator.schema.json": b"{}",
                },
            ), mock.patch(
                "scripts.verification.focused_mapping_candidates."
                "capture_raw_trace_evidence",
                return_value=mock.Mock(repository_originals={}),
            ), mock.patch(
                "scripts.verification.focused_mapping_candidates."
                "_selected_input_preconditions",
                return_value={
                    "crates/example/src/lib.rs": frozen_source,
                },
            ), mock.patch(
                "scripts.verification.focused_mapping_candidates."
                "_metadata_from_preconditions",
                return_value=metadata,
            ), mock.patch(
                "scripts.verification.focused_mapping_candidates.build_focused_mapping_context",
                return_value=context,
            ), mock.patch(
                "scripts.verification.focused_mapping_candidates.validate_focused_rust_trace",
            ), mock.patch(
                "scripts.verification.focused_mapping_candidates."
                "authenticate_focused_trace",
                return_value=mock.Mock(source_originals={}),
            ), mock.patch(
                "scripts.verification.focused_mapping_candidates.validated_live_code_facts",
                side_effect=validate_live,
            ), mock.patch(
                "scripts.verification.focused_mapping_candidates.analyze_mapping_candidates",
                return_value={
                    "candidates": candidate_document["candidates"],
                    "demands": [],
                },
            ), mock.patch(
                "scripts.verification.focused_mapping_candidates.invoke_assertion_inventories",
                side_effect=assertions_from_snapshot,
            ) as assertions, mock.patch(
                "scripts.verification.focused_mapping_candidates.build_mapping_proposals",
                side_effect=proposals_from_snapshot,
            ), mock.patch(
                "scripts.verification.focused_mapping_candidates.validate_mapping_quarantine",
                return_value=[],
            ), mock.patch(
                "scripts.verification.focused_mapping_candidates."
                "_load_and_filter_mapping_decisions",
                side_effect=lambda **kwargs: (
                    initial_decision_document("a" * 40),
                    kwargs["proposals"],
                ),
            ), mock.patch(
                "scripts.verification.focused_mapping_candidates."
                "render_proposals_toml",
                return_value="schema_version = 1\n",
            ), mock.patch(
                "scripts.verification.focused_mapping_candidates.apply_mapping_transaction",
            ) as transaction:
                generated, proposals = generate_focused_mapping_candidates(
                    root=root,
                    trace_path=trace_path,
                    candidates_path=candidates_path,
                    proposals_path=proposals_path,
                    decisions_path=decisions_path,
                    rust_discovery_ids=[discovery_id],
                    authority_plan_path=None,
                    authority_routes_path=None,
                    trace_root=artifact_root / "raw-trace",
                    cargo_target_dir=root / "target/cargo",
                    llvm_tools_dir=root / "target/llvm-tools",
                    temporary_dir=root / "target/temporary",
                    timestamp="2026-07-24T00:00:00Z",
                )

        self.assertEqual(candidate_document["candidates"], generated["candidates"])
        self.assertEqual([], generated["demands"])
        self.assertEqual(
            "sha256:" + hashlib.sha256(frozen_source).hexdigest(),
            proposals["proposals"][0]["test_source_digest"],
        )
        self.assertEqual(3, len(snapshot_roots))
        self.assertEqual(1, len(set(snapshot_roots)))
        self.assertNotEqual(root, snapshot_roots[0])
        assertions.assert_called_once()
        expected_originals = transaction.call_args.kwargs["expected_originals"]
        self.assertEqual(
            json.dumps(trace).encode(),
            expected_originals["target/gate-artifacts/trace.json"],
        )
        self.assertIsNone(
            expected_originals["target/gate-artifacts/candidates.json"]
        )
        self.assertEqual(
            b"schema_version = 1\n",
            expected_originals["verification/test-catalog-proposals.toml"],
        )
        self.assertEqual(
            b"schema_version = 1\n",
            expected_originals[
                "verification/test-catalog-mapping-decisions.toml"
            ],
        )


if __name__ == "__main__":
    unittest.main()
