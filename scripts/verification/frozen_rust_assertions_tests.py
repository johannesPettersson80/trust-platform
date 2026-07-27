import tempfile
import unittest
from pathlib import Path

from scripts.verification.frozen_rust_assertions import select_frozen_facts


class FrozenRustAssertionsTests(unittest.TestCase):
    def test_selects_exact_names_from_one_frozen_path(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            verification = root / "verification"
            verification.mkdir()
            (verification / "test-catalog-denominator.toml").write_text(
                """
                [[reviews]]
                discovery_id = "DISC_A"
                discovery_source_kind = "rust_unit_test"
                path = "src/tests.rs"
                name = "selected"
                disposition = "reviewed_nonmapping"
                reason_code = "no_reviewed_spec_or_invariant_binding"

                [[reviews]]
                discovery_id = "DISC_B"
                discovery_source_kind = "rust_unit_test"
                path = "src/tests.rs"
                name = "deferred"
                disposition = "reviewed_nonmapping"
                reason_code = "no_reviewed_spec_or_invariant_binding"
                """
            )

            facts = select_frozen_facts(
                root=root,
                path="src/tests.rs",
                reason_code="no_reviewed_spec_or_invariant_binding",
                names=["selected"],
            )

            self.assertEqual(["selected"], [row["name"] for row in facts])
            self.assertEqual("DISC_A", facts[0]["discovery_id"])

    def test_rejects_name_outside_frozen_mapping_debt(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            verification = root / "verification"
            verification.mkdir()
            (verification / "test-catalog-denominator.toml").write_text(
                "schema_version = 1\nreviews = []\n"
            )

            with self.assertRaisesRegex(
                ValueError,
                "absent from frozen mapping debt",
            ):
                select_frozen_facts(
                    root=root,
                    path="src/tests.rs",
                    reason_code="no_reviewed_spec_or_invariant_binding",
                    names=["missing"],
                )


if __name__ == "__main__":
    unittest.main()
