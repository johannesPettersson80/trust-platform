"""Focused tests for frozen-denominator mapping candidate inputs."""

from __future__ import annotations

import json
import tomllib
import unittest
from pathlib import Path
from tempfile import TemporaryDirectory
from unittest import mock

from scripts.verification.code_spec_authority_selection import (
    AuthoritySelectionApplication,
)
from scripts.verification.code_spec_live import CodeSpecMetadata
from scripts.verification.coverage_trace_inputs import (
    focused_rust_trace_input_provenance,
)
from scripts.verification.coverage_trace_resolver import (
    mechanical_resolved_rust_command,
)
from scripts.verification.focused_mapping_context import (
    authenticate_projected_code_spec_authority,
    build_focused_mapping_context,
    canonical_input_file,
    validate_focused_rust_trace,
)
from scripts.verification.focused_test_facts import focused_rust_input_digest
from scripts.verification.test_catalog_common import stable_discovery_id


CODE_ID = "CODE_" + "A" * 64
INVARIANT_ID = "INV_EXAMPLE_001"
SPEC_REF = "SPEC_EXAMPLE_001#behavior"


class FocusedMappingContextTests(unittest.TestCase):
    def test_snapshot_context_ignores_transient_live_source_mutation(self) -> None:
        with TemporaryDirectory() as temporary:
            root = Path(temporary)
            source = root / "crates/example/src/lib.rs"
            source.parent.mkdir(parents=True)
            source.write_text("#[test]\nfn frozen_case() {}\n")
            manifest = root / "crates/example/Cargo.toml"
            manifest.write_text(
                '[package]\nname = "example"\nversion = "0.1.0"\n'
            )
            discovery_id = stable_discovery_id(
                source_kind="rust_unit_test",
                package="example",
                native_id="crates/example/src/lib.rs#frozen_case",
            )
            _write_denominator(
                root,
                discovery_id=discovery_id,
                name="frozen_case",
            )
            _write_denominator_schema(root)
            ignored = root / "verification/ignored-tests.toml"
            ignored.write_text("schema_version = 1\nignored_tests = []\n")
            source_snapshot = source.read_bytes()
            manifest_snapshot = manifest.read_bytes()
            denominator = tomllib.loads(
                (root / "verification/test-catalog-denominator.toml").read_text()
            )
            schema = json.loads(
                (
                    root
                    / "verification/schemas/test-catalog-denominator.schema.json"
                ).read_text()
            )
            source.write_text("#[test]\nfn transient_forgery() {}\n")

            context = build_focused_mapping_context(
                root=root,
                discovery_ids=[discovery_id],
                tests={},
                denominator_document=denominator,
                denominator_schema_document=schema,
                ignored_document=tomllib.loads(ignored.read_text()),
                source_input_preconditions={
                    "crates/example/src/lib.rs": source_snapshot,
                    "crates/example/Cargo.toml": manifest_snapshot,
                },
            )

            self.assertEqual(
                "frozen_case",
                context.selected_facts[discovery_id]["name"],
            )

    def test_rejects_stale_selected_fact_instead_of_trusting_denominator(
        self,
    ) -> None:
        with TemporaryDirectory() as temporary:
            root = Path(temporary)
            source = root / "crates/example/src/lib.rs"
            source.parent.mkdir(parents=True)
            source.write_text("#[test]\nfn live_case() {}\n")
            (root / "crates/example/Cargo.toml").write_text(
                '[package]\nname = "example"\nversion = "0.1.0"\n'
            )
            discovery_id = stable_discovery_id(
                source_kind="rust_unit_test",
                package="example",
                native_id="crates/example/src/lib.rs#stale_case",
            )
            _write_denominator(
                root,
                discovery_id=discovery_id,
                name="stale_case",
            )
            _write_denominator_schema(root)
            (root / "verification/ignored-tests.toml").write_text(
                "schema_version = 1\nignored_tests = []\n"
            )

            with self.assertRaisesRegex(
                ValueError,
                "absent from its reviewed live source file",
            ):
                build_focused_mapping_context(
                    root=root,
                    discovery_ids=[discovery_id],
                    tests={},
                )

    def test_raw_authority_selection_is_not_accepted_as_projection(self) -> None:
        with TemporaryDirectory() as temporary:
            root = Path(temporary)
            selection_path = root / "target/gate-artifacts/selection.json"
            selection_path.parent.mkdir(parents=True)
            (root / "verification").mkdir()
            (root / "verification/code-spec-map.toml").write_text(
                "schema_version = 1\nmappings = []\n"
            )
            selection_path.write_text(
                json.dumps(
                    {
                        "schema_version": 1,
                        "reviewed_date": "2026-07-24",
                        "review_provenance": {},
                        "rows": [],
                    }
                )
            )
            metadata = CodeSpecMetadata(
                code_map={"schema_version": 1},
                mappings={},
                invariants={},
            )

            with self.assertRaisesRegex(
                ValueError,
                "not an applicator projection manifest",
            ):
                authenticate_projected_code_spec_authority(
                    root=root,
                    metadata=metadata,
                    plan_path=selection_path,
                )

    def test_authenticated_authority_projection_is_rebuilt_exactly(self) -> None:
        with TemporaryDirectory() as temporary:
            root = Path(temporary)
            artifact_root = root / "target/gate-artifacts/authority"
            artifact_root.mkdir(parents=True)
            verification = root / "verification"
            verification.mkdir()
            code_map = b"schema_version = 1\nmappings = []\n"
            (verification / "code-spec-map.toml").write_bytes(code_map)
            selection_path = artifact_root / "selection.json"
            selection_bytes = b'{"reviewed":true}\n'
            selection_path.write_bytes(selection_bytes)
            manifest_path = artifact_root / "projection.json"
            projected_path = artifact_root / "projection.projected-code-spec-map.toml"
            projected_bytes = (
                "schema_version = 1\n"
                "[[mappings]]\n"
                f'code_id = "{CODE_ID}"\n'
                'area = "compiler_iec"\n'
                'disposition = "specified"\n'
                f'spec_ref = "{SPEC_REF}"\n'
            ).encode()
            projected_path.write_bytes(projected_bytes)
            manifest = {
                "schema_version": 1,
                "kind": "reviewed_code_spec_authority_selection",
                "selection_path": selection_path.relative_to(root).as_posix(),
                "projected_code_map_path": projected_path.relative_to(root).as_posix(),
                "rows": [],
            }
            manifest_bytes = (
                json.dumps(manifest, indent=2, sort_keys=True) + "\n"
            ).encode()
            manifest_path.write_bytes(manifest_bytes)
            authority_inputs = {
                "verification/test-catalog.toml": b"catalog authority\n",
                "verification/invariants/compiler_iec/INV_EXAMPLE_001.toml": (
                    b"invariant authority\n"
                ),
                "docs/specs/example.md": b"# Specification authority\n",
                "target/gate-artifacts/authority/proposal.json": (
                    b'{"proposal":"authority"}\n'
                ),
            }
            application = AuthoritySelectionApplication(
                replacements={},
                quarantine_replacements={
                    manifest_path.relative_to(root).as_posix(): manifest_bytes,
                    projected_path.relative_to(root).as_posix(): projected_bytes,
                },
                manifest=manifest,
                code_map_before_bytes=code_map,
                projected_code_map_bytes=projected_bytes,
                selection_path=selection_path.relative_to(root).as_posix(),
                output_path=manifest_path.relative_to(root).as_posix(),
                expected_originals={
                    "verification/code-spec-map.toml": code_map,
                    selection_path.relative_to(root).as_posix(): (
                        selection_bytes
                    ),
                    manifest_path.relative_to(root).as_posix(): manifest_bytes,
                    projected_path.relative_to(root).as_posix(): projected_bytes,
                    **authority_inputs,
                },
            )
            metadata = CodeSpecMetadata(
                code_map={"schema_version": 1, "mappings": []},
                mappings={},
                invariants={},
            )

            with mock.patch(
                "scripts.verification.focused_mapping_context."
                "build_authority_selection_application",
                return_value=application,
            ) as rebuild:
                projection = authenticate_projected_code_spec_authority(
                    root=root,
                    metadata=metadata,
                    plan_path=manifest_path,
                )

            rebuild.assert_called_once_with(
                root=root.resolve(),
                selection_path=selection_path,
                output_path=manifest_path,
            )
            self.assertEqual(
                "specified",
                projection.metadata.mappings[CODE_ID]["disposition"],
            )
            self.assertEqual(
                code_map,
                projection.expected_originals["verification/code-spec-map.toml"],
            )
            self.assertEqual(
                selection_bytes,
                projection.expected_originals[
                    selection_path.relative_to(root).as_posix()
                ],
            )
            for path, content in authority_inputs.items():
                self.assertEqual(
                    content,
                    projection.expected_originals[path],
                    path,
                )

    def test_focused_trace_rejects_source_drift_after_execution(self) -> None:
        with TemporaryDirectory() as temporary:
            root = Path(temporary)
            source = root / "crates/example/src/lib.rs"
            source.parent.mkdir(parents=True)
            source.write_text("#[test]\nfn live_case() {}\n")
            (root / "crates/example/Cargo.toml").write_text(
                '[package]\nname = "example"\nversion = "0.1.0"\n'
            )
            discovery_id = stable_discovery_id(
                source_kind="rust_unit_test",
                package="example",
                native_id="crates/example/src/lib.rs#live_case",
            )
            _write_denominator(
                root=root,
                discovery_id=discovery_id,
                name="live_case",
            )
            _write_denominator_schema(root)
            (root / "verification/ignored-tests.toml").write_text(
                "schema_version = 1\nignored_tests = []\n"
            )
            context = build_focused_mapping_context(
                root=root,
                discovery_ids=[discovery_id],
                tests={},
            )
            metadata = CodeSpecMetadata(
                code_map={"schema_version": 1, "mappings": []},
                mappings={},
                invariants={},
            )
            resolved_row = {
                **context.selected_facts[discovery_id],
                "package": "example",
                "nextest_binary_id": "example::lib",
                "nextest_binary_path": "/tmp/example",
                "nextest_test_name": "tests::live_case",
                "cargo_target_kind": "lib",
                "cargo_target_name": None,
                "command_authority": "verified_synthesized",
            }
            resolved_row["command"] = mechanical_resolved_rust_command(
                resolved_row,
                ["hir"],
            )
            resolved = [resolved_row]
            provenance = focused_rust_trace_input_provenance(
                source_input_digest=focused_rust_input_digest(
                    root=root,
                    facts=context.selected_live_facts,
                    denominator=context.denominator,
                ),
                resolved=resolved,
                code_map=metadata.code_map,
            )
            trace = {
                "scope": {
                    "mode": "focused_shard",
                    "requested_discovery_ids": [discovery_id],
                    "source_kinds": ["rust_unit_test"],
                },
                "code_inventory_digests": {},
                "input_digest": provenance["input_digest"],
                "input_provenance": provenance,
                "tests": [
                    {
                        **resolved[0],
                        "trace_status": "observed",
                        "exit_code": 0,
                        "attempt_id": "attempt-1",
                        "profile_digest": "sha256:" + "1" * 64,
                        "attribution_kind": "semantic_source_coverage",
                        "covered_code_ids": [],
                    }
                ],
            }

            validate_focused_rust_trace(
                root=root,
                trace=trace,
                context=context,
                metadata=metadata,
            )
            tampered = json.loads(json.dumps(trace))
            tampered_resolved = tampered["input_provenance"]["resolved"]
            tampered_resolved[0]["command"] = "cargo nextest run fabricated"
            tampered["tests"][0]["command"] = "cargo nextest run fabricated"
            tampered_provenance = focused_rust_trace_input_provenance(
                source_input_digest=provenance["source_input_digest"],
                resolved=tampered_resolved,
                code_map=metadata.code_map,
            )
            tampered["input_provenance"] = tampered_provenance
            tampered["input_digest"] = tampered_provenance["input_digest"]
            with self.assertRaisesRegex(
                ValueError,
                "command is not mechanically derived",
            ):
                validate_focused_rust_trace(
                    root=root,
                    trace=tampered,
                    context=context,
                    metadata=metadata,
                )
            source.write_text("#[test]\nfn live_case() { assert!(true); }\n")
            with self.assertRaisesRegex(
                ValueError,
                "source inputs are stale",
            ):
                validate_focused_rust_trace(
                    root=root,
                    trace=trace,
                    context=context,
                    metadata=metadata,
                )

    def test_canonical_input_rejects_symlink_components(self) -> None:
        with TemporaryDirectory() as temporary:
            root = Path(temporary)
            outside = root / "outside"
            outside.mkdir()
            (outside / "trace.json").write_text("{}")
            governed = root / "target/gate-artifacts"
            governed.mkdir(parents=True)
            (governed / "linked").symlink_to(outside, target_is_directory=True)

            with self.assertRaisesRegex(ValueError, "must not traverse symlinks"):
                canonical_input_file(
                    root,
                    governed / "linked/trace.json",
                    "trace",
                    required_parent="target/gate-artifacts",
                )


def _write_denominator(
    root: Path,
    *,
    discovery_id: str,
    name: str,
) -> None:
    verification = root / "verification"
    verification.mkdir(exist_ok=True)
    (verification / "test-catalog-denominator.toml").write_text(
        (
            "schema_version = 1\n"
            'id = "TEST_CATALOG_DENOMINATOR_REVIEW_V1"\n'
            'title = "Fixture"\n'
            'denominator_basis = "all_live_existing_test_scanner_facts"\n'
            'review_basis = "explicit_per_discovery_id_no_semantic_inference"\n'
            'last_reviewed = "2026-07-24"\n'
            "[reason_definitions]\n"
            'no_reviewed_spec_or_invariant_binding = "The fact remains part of its native test suite, but no hand-reviewed catalog row authorizes a specification, invariant, oracle, or expected-result claim for it. It is retired from mapping debt without being deleted."\n'
            'assertion_oracle_unresolved = "The test has product-facing assertions, but the reviewed assertion extractor cannot close one or more helper or external-oracle calls. It remains runnable and explicit without claiming catalog proof."\n'
            'not_catalogable_product_oracle = "The test is a harness, static-presence, zero-assertion, or authority-contradictory check that cannot honestly prove an accepted product invariant. It remains runnable without catalog proof."\n'
            'ignored_test_registry_owned = "The ignored or conditional fact is governed by its exact Phase 3 ignored-test registry row and has no separate hand-reviewed invariant catalog mapping."\n'
            'fuzz_program_owned = "The fuzz target is governed by the dedicated Phase 9 fuzz-program and crash-regression ledgers rather than an inferred invariant catalog mapping."\n'
            'gate_inventory_owned = "The gate script or workflow job is governed by the Phase 5 gate inventory and suite-routing contracts rather than a product-test invariant mapping."\n'
            "[[reviews]]\n"
            f'discovery_id = "{discovery_id}"\n'
            'discovery_source_kind = "rust_unit_test"\n'
            'path = "crates/example/src/lib.rs"\n'
            f'name = "{name}"\n'
            'ignore_state = "not_ignored"\n'
            'disposition = "reviewed_nonmapping"\n'
            'reason_code = "no_reviewed_spec_or_invariant_binding"\n'
            'last_reviewed = "2026-07-24"\n'
        )
    )


def _write_denominator_schema(root: Path) -> None:
    schema_path = root / "verification/schemas/test-catalog-denominator.schema.json"
    schema_path.parent.mkdir()
    schema_path.write_text(
        json.dumps({"type": "object"}),
    )


if __name__ == "__main__":
    unittest.main()
