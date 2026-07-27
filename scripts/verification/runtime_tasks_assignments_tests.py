import unittest

from scripts.verification.runtime_tasks_assignments import (
    PRODUCT_BEHAVIORS,
    _is_setup,
)


class RuntimeTasksAssignmentsTests(unittest.TestCase):
    def test_reviewed_rules_cover_eight_tests(self) -> None:
        self.assertEqual(8, len(PRODUCT_BEHAVIORS))

    def test_execution_success_is_setup(self) -> None:
        self.assertTrue(_is_setup("runtime . execute_cycle () . unwrap ()"))

    def test_observed_storage_value_is_product_oracle(self) -> None:
        self.assertFalse(
            _is_setup(
                'assert_eq ! (runtime . storage_mut () . get_global ("count") '
                ", Some (& Value :: Int (1)))"
            )
        )


if __name__ == "__main__":
    unittest.main()
