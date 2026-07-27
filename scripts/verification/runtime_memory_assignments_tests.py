import unittest

from scripts.verification.runtime_memory_assignments import (
    BEHAVIOR_BY_TEST,
    _is_setup,
)


class RuntimeMemoryAssignmentsTests(unittest.TestCase):
    def test_reviewed_rules_cover_expected_behavior_partitions(self) -> None:
        self.assertEqual(len(BEHAVIOR_BY_TEST), 14)
        self.assertEqual(set(BEHAVIOR_BY_TEST.values()), {0, 1, 2, 3, 4})

    def test_cache_poisoning_actions_are_setup(self) -> None:
        name = "variable_storage_clone_recovers_from_poisoned_cache_lock"
        self.assertTrue(
            _is_setup(name=name, expression='expect ("test cache lock")')
        )
        self.assertTrue(
            _is_setup(
                name=name,
                expression='panic ! ("poison instance field cache")',
            )
        )
        self.assertFalse(
            _is_setup(
                name=name,
                expression="assert ! (cloned . globals . is_empty ())",
            )
        )

    def test_copy_on_write_mutation_is_not_setup(self) -> None:
        name = "write_by_ref_path_preserves_struct_copy_on_write_isolation"
        self.assertFalse(
            _is_setup(
                name=name,
                expression="assert ! (storage . write_by_ref_parts (...))",
            )
        )
        self.assertTrue(
            _is_setup(
                name=name,
                expression='storage . get_global ("left") . expect ("left")',
            )
        )

    def test_cache_invalidation_transition_is_product_oracle(self) -> None:
        name = "direct_instance_field_miss_cache_invalidates_on_new_insert"
        self.assertFalse(
            _is_setup(
                name=name,
                expression="assert ! (storage . set_instance_var (...))",
            )
        )


if __name__ == "__main__":
    unittest.main()
