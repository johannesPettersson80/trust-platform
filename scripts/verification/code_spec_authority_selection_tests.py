"""Focused tests for reviewed code-spec authority selection application."""

from __future__ import annotations

import copy
import json
import tempfile
import tomllib
import unittest
from dataclasses import replace
from pathlib import Path

from scripts.verification.code_spec_authority_selection import (
    apply_authority_selection_application,
    apply_authority_selection,
    build_authority_selection_application,
    main,
)
from scripts.verification.code_spec_authority_review import (
    compile_authority_review_selection,
    write_authority_human_selection_template,
    write_authority_review_card,
)
from scripts.verification.code_spec_authority_selection_test_support import (
    CODE_ID,
    DISCOVERY_ID,
    INVARIANT_ID,
    OTHER_CODE_ID,
    OUTPUT_PATH,
    SECOND_CODE_ID,
    SPEC_REF,
    TEST_ID,
    AuthoritySelectionFixture as Fixture,
    digest as _digest,
    mapping_block as _mapping_block,
)


class CodeSpecAuthoritySelectionTests(unittest.TestCase):
    def setUp(self) -> None:
        self.temporary = tempfile.TemporaryDirectory()
        self.root = Path(self.temporary.name)
        self.fixture = Fixture(self.root)

    def tearDown(self) -> None:
        self.temporary.cleanup()

    def test_happy_path_derives_tests_and_changes_only_semantic_fields(self) -> None:
        application = self.fixture.build()
        updated_bytes = application.replacements[
            "verification/code-spec-map.toml"
        ]
        document = tomllib.loads(updated_bytes.decode())
        selected = document["mappings"][0]

        self.assertEqual("specified", selected["disposition"])
        self.assertEqual(SPEC_REF, selected["spec_ref"])
        self.assertEqual([INVARIANT_ID], selected["invariant_ids"])
        self.assertEqual([TEST_ID], selected["test_ids"])
        self.assertEqual("2026-07-24", selected["last_reviewed"])
        self.assertNotIn("spec_gap_ref", selected)
        before = tomllib.loads(self.fixture.code_map_bytes.decode())["mappings"][0]
        for field in (
            "disposition",
            "spec_gap_ref",
            "spec_ref",
            "invariant_ids",
            "test_ids",
            "last_reviewed",
        ):
            before.pop(field, None)
            selected.pop(field, None)
        self.assertEqual(before, selected)
        self.assertEqual(
            _digest(self.fixture.code_map_bytes),
            application.manifest["code_map_before_sha256"],
        )
        self.assertEqual(
            _digest(updated_bytes),
            application.manifest["code_map_after_sha256"],
        )
        self.assertIn(OUTPUT_PATH.as_posix(), application.replacements)
        self.assertFalse(application.manifest["canonical_apply_ready"])
        self.assertEqual(
            [TEST_ID],
            application.manifest["missing_catalog_test_ids"],
        )
        self.assertEqual(
            [f"{INVARIANT_ID}:{TEST_ID}"],
            application.manifest["missing_invariant_backlinks"],
        )

    def test_catalog_mapped_curated_test_identity_is_reused(self) -> None:
        fixture = Fixture(
            self.root / "curated-catalog-id",
            include_canonical_authority=True,
        )
        curated_test_id = "TEST_CURATED_EXACT_BEHAVIOR_001"
        fixture.rewrite_toml(
            f"verification/invariants/runtime_safety/{INVARIANT_ID}.toml",
            TEST_ID,
            curated_test_id,
        )
        fixture.rewrite_toml(
            "verification/test-catalog.toml",
            TEST_ID,
            curated_test_id,
        )
        fixture.rewrite_toml(
            "verification/test-catalog-denominator.toml",
            'disposition = "reviewed_nonmapping"\n'
            'reason_code = "no_reviewed_spec_or_invariant_binding"',
            'disposition = "catalog_mapped"\n'
            f'catalog_test_id = "{curated_test_id}"',
        )

        application = fixture.build()
        updated = tomllib.loads(
            application.replacements[
                "verification/code-spec-map.toml"
            ].decode()
        )

        self.assertEqual(
            [curated_test_id],
            updated["mappings"][0]["test_ids"],
        )
        self.assertEqual(
            [],
            application.manifest["missing_catalog_test_ids"],
        )
        self.assertEqual(
            [],
            application.manifest["missing_invariant_backlinks"],
        )

    def test_json_and_toml_selections_have_identical_results(self) -> None:
        json_application = self.fixture.build()
        other_root = self.root / "toml"
        toml_fixture = Fixture(other_root, selection_extension="toml")
        toml_application = toml_fixture.build()

        self.assertEqual(
            json_application.replacements["verification/code-spec-map.toml"],
            toml_application.replacements["verification/code-spec-map.toml"],
        )
        self.assertEqual(
            json_application.manifest["rows"],
            toml_application.manifest["rows"],
        )

    def test_unknown_or_stale_code_and_discovery_ids_are_rejected(self) -> None:
        cases = (
            ("code_id", "CODE_" + "D" * 64, "unknown code_id"),
            ("invariant_ids", ["INV_UNKNOWN_001"], "unknown invariant"),
            ("discovery_ids", ["DISC_" + "D" * 20], "unknown discovery_id"),
        )
        for field, value, message in cases:
            with self.subTest(field=field):
                fixture = Fixture(self.root / field)
                fixture.selection["rows"][0][field] = value
                fixture.write_selection()
                with self.assertRaisesRegex(ValueError, message):
                    fixture.build()

        self.fixture.rewrite_toml(
            "verification/code-spec-map.toml",
            'disposition = "spec_gap"',
            'disposition = "specified"',
        )
        with self.assertRaisesRegex(ValueError, "must currently be spec_gap"):
            self.fixture.build()

    def test_non_active_or_non_oracle_spec_source_is_rejected(self) -> None:
        cases = (
            ('source_status = "active"', 'source_status = "retired"'),
            ("oracle_eligible = true", "oracle_eligible = false"),
        )
        for old, new in cases:
            with self.subTest(new=new):
                fixture = Fixture(self.root / new.split()[-1])
                fixture.rewrite_toml(
                    "verification/spec-sources.toml", old, new
                )
                with self.assertRaisesRegex(
                    ValueError, "active and oracle-eligible"
                ):
                    fixture.build()

        fixture = Fixture(self.root / "unknown-authority")
        fixture.rewrite_toml(
            "verification/spec-sources.toml",
            'authority = "normative_product"',
            'authority = "made_up_authority"',
        )
        with self.assertRaisesRegex(ValueError, "active and oracle-eligible"):
            fixture.build()

    def test_invariant_area_mismatch_is_rejected(self) -> None:
        self.fixture.rewrite_toml(
            f"verification/invariants/runtime_safety/{INVARIANT_ID}.toml",
            'area = "runtime_safety"',
            'area = "compiler_iec"',
        )
        with self.assertRaisesRegex(ValueError, "invariant area"):
            self.fixture.build()

    def test_duplicate_and_disallowed_discovery_rows_are_rejected(self) -> None:
        self.fixture.selection["rows"][0]["discovery_ids"].append(DISCOVERY_ID)
        with self.assertRaisesRegex(ValueError, "unique string array"):
            self.fixture.write_selection()

        fixture = Fixture(self.root / "duplicate-denominator")
        denominator = (
            fixture.root / "verification/test-catalog-denominator.toml"
        )
        denominator.write_text(
            denominator.read_text(encoding="utf-8")
            + denominator.read_text(encoding="utf-8").split(
                "[[reviews]]", 1
            )[1].join(("[[reviews]]", "")),
            encoding="utf-8",
        )
        with self.assertRaisesRegex(ValueError, "denominator duplicates"):
            fixture.build()

        fixture = Fixture(self.root / "mapped")
        fixture.rewrite_toml(
            "verification/test-catalog-denominator.toml",
            'disposition = "reviewed_nonmapping"\n'
            'reason_code = "no_reviewed_spec_or_invariant_binding"',
            'disposition = "catalog_mapped"\n'
            f'catalog_test_id = "{TEST_ID}"',
        )
        with self.assertRaisesRegex(
            ValueError, "catalog_mapped.*exact catalog authority"
        ):
            fixture.build()

    def test_manual_test_id_field_is_forbidden(self) -> None:
        self.fixture.selection["rows"][0]["test_ids"] = ["TEST_MANUAL_001"]
        self.fixture._write_raw_selection()
        with self.assertRaisesRegex(ValueError, "selection row fields drift"):
            self.fixture.build()

    def test_compiled_numbered_review_is_valid_applicator_input(self) -> None:
        proposal_path = self.fixture.selection_path.parent / "proposal.json"
        card_path = self.fixture.selection_path.parent / "review-card.json"
        human_path = self.fixture.selection_path.parent / "human.json"
        proposal = {
            "schema_version": self.fixture.selection["schema_version"],
            "reviewed_date": self.fixture.selection["reviewed_date"],
            "candidate_provenance": self.fixture.selection[
                "review_provenance"
            ]["candidate_provenance"],
            "rows": self.fixture.selection["rows"],
        }
        proposal_path.write_text(json.dumps(proposal), encoding="utf-8")
        write_authority_review_card(
            root=self.root,
            proposal_path=proposal_path,
            output_path=card_path,
        )
        human = write_authority_human_selection_template(
            root=self.root,
            review_card_path=card_path,
            output_path=human_path,
        )
        human["reviewer_ids"] = ["reviewer-a"]
        human["accepted_row_indices"] = [1]
        human_path.write_text(json.dumps(human), encoding="utf-8")
        compile_authority_review_selection(
            root=self.root,
            proposal_path=proposal_path,
            review_card_path=card_path,
            human_selection_path=human_path,
            output_path=self.fixture.selection_path,
        )

        application = self.fixture.build()

        self.assertEqual(1, application.manifest["row_count"])
        self.assertEqual(
            ["reviewer-a"],
            application.manifest["review_provenance"]["reviewer_ids"],
        )

    def test_applicator_rejects_tampered_compiled_selection(self) -> None:
        self.fixture.selection["rows"][0]["spec_ref"] = (
            "SPEC_EXAMPLE_001#forged"
        )
        self.fixture._write_raw_selection()

        with self.assertRaisesRegex(
            ValueError, "does not match compiled review inputs"
        ):
            self.fixture.build()

    def test_unselected_mapping_block_is_byte_exact(self) -> None:
        application = self.fixture.build()
        before = self.fixture.code_map_bytes.split(
            b"\n# unselected marker\n", 1
        )[1]
        after = application.replacements[
            "verification/code-spec-map.toml"
        ].split(b"\n# unselected marker\n", 1)[1]
        self.assertEqual(before, after)

    def test_pre_catalog_check_is_read_only_and_apply_fails_closed(self) -> None:
        lock_path = self.root / ".mapping-transaction.lock"
        lock_before = lock_path.read_bytes()
        self.assertEqual(
            0,
            main(
                [
                    "--root",
                    str(self.root),
                    "--selection",
                    str(self.fixture.selection_path),
                    "--output",
                    str(self.fixture.output_path),
                    "--check",
                ]
            ),
        )
        self.assertFalse(self.fixture.output_path.exists())
        self.assertEqual(lock_before, lock_path.read_bytes())
        before = (self.root / "verification/code-spec-map.toml").read_bytes()
        with self.assertRaisesRegex(
            ValueError, "canonical catalog rows and invariant backlinks"
        ):
            apply_authority_selection(
                root=self.root,
                selection_path=self.fixture.selection_path,
                output_path=self.fixture.output_path,
            )
        self.assertEqual(
            before,
            (self.root / "verification/code-spec-map.toml").read_bytes(),
        )
        self.assertFalse(self.fixture.output_path.exists())

    def test_catalog_collision_or_mismatch_fails_projection(self) -> None:
        catalog = self.root / "verification/test-catalog.toml"
        catalog.write_text(
            (
                "schema_version = 1\n"
                "[[tests]]\n"
                f'id = "{TEST_ID}"\n'
                'subject_kind = "generated_test"\n'
                f'discovery_id = "DISC_{"D" * 20}"\n'
                'status = "mapped"\n'
                'area = "runtime_safety"\n'
                f'invariants = ["{INVARIANT_ID}"]\n'
                f'oracle_ref = "{SPEC_REF}"\n'
                'suite_tiers = ["pr"]\n'
            ),
            encoding="utf-8",
        )
        with self.assertRaisesRegex(ValueError, "collision|exact catalog"):
            self.fixture.build()

        fixture = Fixture(self.root / "discovery-collision")
        catalog = fixture.root / "verification/test-catalog.toml"
        catalog.write_text(
            (
                "schema_version = 1\n"
                "[[tests]]\n"
                'id = "TEST_OTHER_001"\n'
                'subject_kind = "generated_test"\n'
                f'discovery_id = "{DISCOVERY_ID}"\n'
                'status = "mapped"\n'
                'area = "runtime_safety"\n'
                f'invariants = ["{INVARIANT_ID}"]\n'
                f'oracle_ref = "{SPEC_REF}"\n'
                'suite_tiers = ["pr"]\n'
            ),
            encoding="utf-8",
        )
        with self.assertRaisesRegex(ValueError, "collision"):
            fixture.build()

    def test_same_discovery_can_authorize_multiple_selected_code_rows(self) -> None:
        code_map = self.root / "verification/code-spec-map.toml"
        code_map.write_bytes(
            code_map.read_bytes()
            + b"\n"
            + _mapping_block(SECOND_CODE_ID, selected=True).encode()
        )
        second = copy.deepcopy(self.fixture.selection["rows"][0])
        second["code_id"] = SECOND_CODE_ID
        self.fixture.selection["rows"].append(second)
        self.fixture.write_selection()

        application = self.fixture.build()

        self.assertEqual(2, application.manifest["row_count"])
        self.assertEqual(
            [TEST_ID],
            application.manifest["missing_catalog_test_ids"],
        )

    def test_catalog_invariant_superset_is_allowed_with_backlinks(self) -> None:
        fixture = Fixture(
            self.root / "superset",
            include_canonical_authority=True,
        )
        extra_invariant = "INV_EXTRA_001"
        fixture.rewrite_toml(
            "verification/test-catalog.toml",
            f'invariants = ["{INVARIANT_ID}"]',
            f'invariants = ["{INVARIANT_ID}", "{extra_invariant}"]',
        )
        extra_path = (
            fixture.root
            / "verification/invariants/runtime_safety"
            / f"{extra_invariant}.toml"
        )
        extra_path.write_text(
            (
                "schema_version = 1\n"
                f'id = "{extra_invariant}"\n'
                'area = "runtime_safety"\n'
                f'tests = ["{TEST_ID}"]\n'
                "[spec]\n"
                'status = "specified"\n'
                'source_refs = ["SPEC_EXAMPLE_001"]\n'
                "[oracle]\n"
                'kind = "trust_contract"\n'
                f'ref = "{SPEC_REF}"\n'
            ),
            encoding="utf-8",
        )

        application = fixture.build()

        self.assertTrue(application.manifest["canonical_apply_ready"])

    def test_present_test_must_include_every_selected_invariant(self) -> None:
        fixture = Fixture(
            self.root / "selected-subset",
            include_canonical_authority=True,
        )
        extra_invariant = "INV_SECOND_SELECTED_001"
        extra_path = (
            fixture.root
            / "verification/invariants/runtime_safety"
            / f"{extra_invariant}.toml"
        )
        extra_path.write_text(
            (
                "schema_version = 1\n"
                f'id = "{extra_invariant}"\n'
                'area = "runtime_safety"\n'
                f'tests = ["{TEST_ID}"]\n'
                "[spec]\n"
                'status = "specified"\n'
                'source_refs = ["SPEC_EXAMPLE_001"]\n'
                "[oracle]\n"
                'kind = "trust_contract"\n'
                f'ref = "{SPEC_REF}"\n'
            ),
            encoding="utf-8",
        )
        fixture.selection["rows"][0]["invariant_ids"].append(extra_invariant)
        fixture.write_selection()

        with self.assertRaisesRegex(ValueError, "exact catalog authority"):
            fixture.build()

    def test_catalog_mapped_denominator_requires_exact_derived_test(self) -> None:
        fixture = Fixture(
            self.root / "catalog-mapped",
            include_canonical_authority=True,
        )
        fixture.rewrite_toml(
            "verification/test-catalog-denominator.toml",
            'disposition = "reviewed_nonmapping"\n'
            'reason_code = "no_reviewed_spec_or_invariant_binding"',
            'disposition = "catalog_mapped"\n'
            f'catalog_test_id = "{TEST_ID}"',
        )
        self.assertTrue(fixture.build().manifest["canonical_apply_ready"])

        fixture.rewrite_toml(
            "verification/test-catalog-denominator.toml",
            f'catalog_test_id = "{TEST_ID}"',
            'catalog_test_id = "TEST_WRONG_001"',
        )
        with self.assertRaisesRegex(ValueError, "catalog_mapped"):
            fixture.build()

    def test_project_writes_only_quarantine_outputs(self) -> None:
        before = (self.root / "verification/code-spec-map.toml").read_bytes()

        self.assertEqual(
            0,
            main(
                [
                    "--root",
                    str(self.root),
                    "--selection",
                    str(self.fixture.selection_path),
                    "--output",
                    str(self.fixture.output_path),
                    "--project",
                ]
            ),
        )
        application = self.fixture.build()

        self.assertEqual(
            before,
            (self.root / "verification/code-spec-map.toml").read_bytes(),
        )
        self.assertTrue(self.fixture.output_path.is_file())
        projected = (
            self.fixture.output_path.parent
            / "plan.projected-code-spec-map.toml"
        )
        self.assertEqual(application.projected_code_map_bytes, projected.read_bytes())

    def test_canonical_inputs_and_invariant_ancestors_cannot_be_symlinks(self) -> None:
        source = self.root / "verification/spec-sources.toml"
        actual = self.root / "verification/spec-sources.actual.toml"
        source.rename(actual)
        source.symlink_to(actual.name)
        with self.assertRaisesRegex(ValueError, "symlink"):
            self.fixture.build()

        fixture = Fixture(self.root / "invariant-symlink")
        area = fixture.root / "verification/invariants/runtime_safety"
        actual_area = fixture.root / "verification/invariants/runtime_safety-real"
        area.rename(actual_area)
        area.symlink_to(actual_area.name, target_is_directory=True)
        with self.assertRaisesRegex(ValueError, "symlink"):
            fixture.build()

    def test_apply_rejects_code_map_changed_after_projection(self) -> None:
        fixture = Fixture(
            self.root / "lost-update",
            include_canonical_authority=True,
        )
        application = fixture.build()
        code_map = fixture.root / "verification/code-spec-map.toml"
        code_map.write_bytes(code_map.read_bytes() + b"\n# concurrent edit\n")

        with self.assertRaisesRegex(ValueError, "differs from canonical"):
            apply_authority_selection_application(
                root=fixture.root,
                selection_path=fixture.selection_path,
                output_path=fixture.output_path,
                application=application,
            )

        self.assertTrue(code_map.read_bytes().endswith(b"# concurrent edit\n"))

    def test_apply_rejects_forged_prebuilt_application(self) -> None:
        fixture = Fixture(
            self.root / "forged-application",
            include_canonical_authority=True,
        )
        application = fixture.build()
        forged = replace(
            application,
            replacements={
                **application.replacements,
                "verification/code-spec-map.toml": b"forged bytes",
            },
        )

        with self.assertRaisesRegex(ValueError, "differs from canonical"):
            apply_authority_selection_application(
                root=fixture.root,
                selection_path=fixture.selection_path,
                output_path=fixture.output_path,
                application=forged,
            )

        self.assertEqual(
            fixture.code_map_bytes,
            (fixture.root / "verification/code-spec-map.toml").read_bytes(),
        )

    def test_apply_rejects_stale_authority_inputs(self) -> None:
        cases = (
            (
                "catalog",
                "verification/test-catalog.toml",
                lambda content: content.replace(
                    f'id = "{TEST_ID}"', 'id = "TEST_STALE"'
                ),
            ),
            (
                "invariant",
                f"verification/invariants/runtime_safety/{INVARIANT_ID}.toml",
                lambda content: content + "\n# stale invariant edit\n",
            ),
            (
                "spec",
                "docs/specs/example.md",
                lambda content: content + "\n<!-- stale spec edit -->\n",
            ),
            (
                "provenance",
                "target/gate-artifacts/authority-selection/"
                "candidate-card.json",
                lambda content: content + " ",
            ),
        )
        for name, relative, mutate in cases:
            with self.subTest(name=name):
                fixture = Fixture(
                    self.root / f"stale-{name}",
                    include_canonical_authority=True,
                )
                application = fixture.build()
                path = fixture.root / relative
                path.write_text(mutate(path.read_text()), encoding="utf-8")

                with self.assertRaises((ValueError, KeyError)):
                    apply_authority_selection_application(
                        root=fixture.root,
                        selection_path=fixture.selection_path,
                        output_path=fixture.output_path,
                        application=application,
                    )

                self.assertEqual(
                    fixture.code_map_bytes,
                    (
                        fixture.root / "verification/code-spec-map.toml"
                    ).read_bytes(),
                )

    def test_post_catalog_apply_atomically_writes_map_and_manifest(self) -> None:
        fixture = Fixture(
            self.root / "canonical",
            include_canonical_authority=True,
        )
        application = apply_authority_selection(
            root=fixture.root,
            selection_path=fixture.selection_path,
            output_path=fixture.output_path,
        )
        self.assertTrue(application.manifest["canonical_apply_ready"])
        self.assertEqual(
            application.replacements["verification/code-spec-map.toml"],
            (fixture.root / "verification/code-spec-map.toml").read_bytes(),
        )
        self.assertEqual(
            application.replacements[OUTPUT_PATH.as_posix()],
            fixture.output_path.read_bytes(),
        )

    def test_selection_and_output_must_remain_in_target_quarantine(self) -> None:
        with self.assertRaisesRegex(ValueError, "target/gate-artifacts"):
            build_authority_selection_application(
                root=self.root,
                selection_path=self.root / "selection.json",
                output_path=self.fixture.output_path,
            )
        with self.assertRaisesRegex(ValueError, "target/gate-artifacts"):
            build_authority_selection_application(
                root=self.root,
                selection_path=self.fixture.selection_path,
                output_path=self.root / "plan.json",
            )

    def test_quarantine_paths_reject_symlink_ancestors(self) -> None:
        quarantine = self.root / "target/gate-artifacts"
        alias = quarantine / "alias"
        alias.symlink_to(
            self.fixture.selection_path.parent,
            target_is_directory=True,
        )
        with self.assertRaisesRegex(ValueError, "symlink"):
            build_authority_selection_application(
                root=self.root,
                selection_path=alias / self.fixture.selection_path.name,
                output_path=self.fixture.output_path,
            )
        with self.assertRaisesRegex(ValueError, "symlink"):
            build_authority_selection_application(
                root=self.root,
                selection_path=self.fixture.selection_path,
                output_path=alias / "plan.json",
            )


if __name__ == "__main__":
    unittest.main()
