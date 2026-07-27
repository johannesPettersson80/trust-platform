import unittest

from scripts.verification.runtime_mesh_assignments import (
    BEHAVIOR_BY_TEST,
    _is_non_oracle,
    _is_setup,
)


class RuntimeMeshAssignmentsTests(unittest.TestCase):
    def test_reviewed_rules_cover_ten_tests(self) -> None:
        self.assertEqual(10, len(BEHAVIOR_BY_TEST))
        self.assertNotIn(
            "mesh_tls_publish_applies_updates",
            BEHAVIOR_BY_TEST,
        )
        self.assertNotIn(
            "mixed_version_policy_rejects_minor_mismatch",
            BEHAVIOR_BY_TEST,
        )

    def test_envelope_construction_and_decode_are_setup(self) -> None:
        self.assertTrue(
            _is_setup(
                name=(
                    "mesh_payload_propagates_source_identity_and_"
                    "sequence_metadata"
                ),
                expression='build_mesh_payload ("runtime-a", 41, & value).expect("payload")',
            )
        )

    def test_payload_equalities_are_product_oracles(self) -> None:
        self.assertFalse(
            _is_setup(
                name="mesh_payload_encode_decode_fuzz_smoke_budget",
                expression="assert_eq ! (decoded . 0 , value)",
            )
        )

    def test_cloud_ready_message_substring_is_not_product_oracle(self) -> None:
        self.assertTrue(
            _is_non_oracle(
                name="mesh_cloud_ready_wait_times_out_for_degraded_state",
                expression='error . as_str () . contains ("mesh cloud readiness timed out")',
            )
        )


if __name__ == "__main__":
    unittest.main()
