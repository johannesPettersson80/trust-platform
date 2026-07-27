import unittest

from scripts.verification.verification_cases_assignments import (
    BEHAVIOR_BY_TEST,
    _is_setup,
)


class VerificationCasesAssignmentsTests(unittest.TestCase):
    def test_reviewed_rules_cover_twelve_tests(self) -> None:
        self.assertEqual(12, len(BEHAVIOR_BY_TEST))

    def test_temp_directory_operations_are_setup(self) -> None:
        self.assertTrue(
            _is_setup(
                name=(
                    "stamped_artifact_dir_overrides_compile_time_workspace_path"
                ),
                expression="fs :: create_dir_all (& path) . unwrap ()",
            )
        )

    def test_directory_equality_is_product_oracle(self) -> None:
        self.assertFalse(
            _is_setup(
                name=(
                    "stamped_artifact_dir_overrides_compile_time_workspace_path"
                ),
                expression="assert_eq ! (config . artifact_dir , dir)",
            )
        )


if __name__ == "__main__":
    unittest.main()
