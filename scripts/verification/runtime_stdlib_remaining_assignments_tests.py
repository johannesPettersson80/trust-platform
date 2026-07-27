from __future__ import annotations

import json
import tempfile
import unittest
from collections import Counter
from pathlib import Path
from unittest.mock import patch

from scripts.verification import runtime_stdlib_remaining_assignments as subject


class RuntimeStdlibRemainingAssignmentsTests(unittest.TestCase):
    def test_reviewed_binding_snapshot_is_exactly_33_tests(self) -> None:
        runtime_tests = "crates/trust-runtime/tests"
        expected = {
            (f"{runtime_tests}/stdlib_conv.rs", "conversion_functions"),
            (f"{runtime_tests}/stdlib_conv_full.rs", "conversion_full"),
            (
                f"{runtime_tests}/stdlib_conv_full.rs",
                "string_to_real_rejects_non_finite_text",
            ),
            (f"{runtime_tests}/iec_examples.rs", "conversion_usage_examples"),
            (f"{runtime_tests}/stdlib_enum_validate.rs", "enum_comparisons"),
            (f"{runtime_tests}/stdlib_enum_validate.rs", "is_valid_real_values"),
            (
                f"{runtime_tests}/stdlib_enum_validate.rs",
                "is_valid_bcd_bit_strings",
            ),
            (f"{runtime_tests}/stdlib_numeric.rs", "numeric_functions"),
            (f"{runtime_tests}/stdlib_bit_full.rs", "bit_full"),
            (f"{runtime_tests}/stdlib_select_full.rs", "selection_full"),
            (f"{runtime_tests}/expr_calls.rs", "stdlib_named_args"),
            (f"{runtime_tests}/stdlib_string.rs", "string_functions"),
            (f"{runtime_tests}/stdlib_string_full.rs", "string_full"),
            (
                f"{runtime_tests}/stdlib_string_full.rs",
                "string_full_non_ascii_uses_character_elements",
            ),
            (
                f"{runtime_tests}/stdlib_string.rs",
                "bounded_string_assignment_and_concat_respect_declared_capacity",
            ),
            (
                f"{runtime_tests}/stdlib_assertions.rs",
                "assertion_functions_pass_when_conditions_hold",
            ),
            (
                f"{runtime_tests}/stdlib_assertions.rs",
                "assertion_functions_fail_with_assertion_error",
            ),
            (
                f"{runtime_tests}/stdlib_assertions.rs",
                "assertion_failure_messages_use_user_facing_value_strings",
            ),
            (
                f"{runtime_tests}/stdlib_assertions.rs",
                "assertion_comparison_functions_coerce_numeric_types",
            ),
            (f"{runtime_tests}/datetime_profile.rs", "default_profile"),
            (f"{runtime_tests}/datetime_profile.rs", "timezone_naive"),
            (f"{runtime_tests}/datetime_range.rs", "out_of_range_error"),
            (f"{runtime_tests}/datetime_types.rs", "ltime_epoch_and_units"),
            (
                f"{runtime_tests}/stdlib_split_locals.rs",
                "split_date_writes_function_local_outputs_in_runtime_and_vm",
            ),
            (f"{runtime_tests}/fb_timers.rs", "ton_tof_tp"),
            (f"{runtime_tests}/iec_timers.rs", "timing_diagrams"),
            (f"{runtime_tests}/iec_counters.rs", "counter_examples"),
            (f"{runtime_tests}/fb_counters.rs", "ctu_ctd_ctud"),
            (f"{runtime_tests}/fb_counters_full.rs", "counter_variants"),
            (
                f"{runtime_tests}/fb_counters_full.rs",
                "generic_counter_uses_call_value_type_after_null_default",
            ),
            (f"{runtime_tests}/fb_edges.rs", "r_trig_f_trig"),
            (
                f"{runtime_tests}/fb_edges.rs",
                "difu_difd_aliases_match_edge_behavior",
            ),
            (f"{runtime_tests}/fb_bistable.rs", "sr_rs"),
        }
        self.assertEqual(set(subject.TEST_BINDINGS), expected)
        self.assertEqual(set(subject.REVIEWED_ASSERTION_COUNTS), expected)
        self.assertEqual(sum(subject.REVIEWED_ASSERTION_COUNTS.values()), 338)
        root = Path(__file__).resolve().parents[2]
        lock = json.loads((root / subject.REVIEW_LOCK_PATH).read_text())
        locked_bindings = {
            (
                row["path"],
                row["name"],
            ): (row["invariant_id"], row["behavior_selector"])
            for row in lock["tests"]
        }
        self.assertEqual(locked_bindings, subject.TEST_BINDINGS)

    def test_reviewed_assertion_kind_profile_is_closed(self) -> None:
        self.assertEqual(
            subject.REVIEWED_KIND_COUNTS,
            Counter(
                {
                    "macro:assert_eq": 128,
                    "method:assert_eq": 123,
                    "macro:assert": 50,
                    "method:unwrap": 11,
                    "method:unwrap_err": 10,
                    "method:expect": 9,
                    "macro:panic": 5,
                    "method:expect_err": 2,
                }
            ),
        )
        self.assertEqual(subject.SETUP_KINDS, {"method:unwrap", "method:expect"})
        self.assertEqual(
            sum(subject.REVIEWED_KIND_COUNTS.values())
            - sum(subject.REVIEWED_KIND_COUNTS[kind] for kind in subject.SETUP_KINDS),
            318,
        )

    def test_conversion_classifier_rejects_unknown_expression(self) -> None:
        with self.assertRaisesRegex(ValueError, "unreviewed full conversion"):
            subject._conversion_full_behavior("assert_eq ! (new_conversion (), 7)")

    def test_timer_and_counter_order_rules_are_bounded(self) -> None:
        timer = ("crates/trust-runtime/tests/fb_timers.rs", "ton_tof_tp")
        self.assertEqual(
            subject._behavior_for_assertion(
                key=timer, expression="assert ! (out.q)", index=7, kind="macro:assert"
            ),
            (subject.TIMER, "TON_TIME_BASIC_DELAY"),
        )
        self.assertEqual(
            subject._behavior_for_assertion(
                key=timer, expression="assert ! (out.q)", index=8, kind="macro:assert"
            ),
            (subject.TIMER, "TOF_TIME_POST_EXPIRY_HOLD"),
        )
        self.assertEqual(
            subject._behavior_for_assertion(
                key=timer,
                expression="assert ! (out.q)",
                index=17,
                kind="macro:assert",
            ),
            (subject.TIMER, "TP_TIME_POST_EXPIRY_HOLD_AND_REARM"),
        )
        counter = ("crates/trust-runtime/tests/fb_counters.rs", "ctu_ctd_ctud")
        self.assertEqual(
            subject._behavior_for_assertion(
                key=counter,
                expression="assert_eq ! (out.cv, 0)",
                index=13,
                kind="macro:assert_eq",
            ),
            (subject.COUNTER, "CTD_RISING_EDGE_LOAD_AND_ZERO"),
        )
        self.assertEqual(
            subject._behavior_for_assertion(
                key=counter,
                expression="assert_eq ! (out.cv, 0)",
                index=14,
                kind="macro:assert_eq",
            ),
            (subject.COUNTER, "CTUD_EDGE_RESET_AND_OUTPUTS"),
        )

    def test_assertion_message_and_near_rules_are_distinct(self) -> None:
        key = (
            "crates/trust-runtime/tests/stdlib_assertions.rs",
            "assertion_functions_fail_with_assertion_error",
        )
        self.assertEqual(
            subject._behavior_for_assertion(
                key=key,
                expression='assert_eq ! (message . as_ref (), expected)',
                index=0,
                kind="macro:assert_eq",
            ),
            (subject.ASSERTION, "ASSERTION_RELATIONAL_MESSAGE_FORMS"),
        )
        self.assertEqual(
            subject._behavior_for_assertion(
                key=key,
                expression='assert ! (message . contains ("delta"))',
                index=14,
                kind="macro:assert",
            ),
            (subject.ASSERTION, "ASSERT_NEAR_FAILURE_CONTEXT"),
        )

    def test_bcd_classifier_uses_input_domain_not_boolean_result(self) -> None:
        key = (
            "crates/trust-runtime/tests/stdlib_enum_validate.rs",
            "is_valid_bcd_bit_strings",
        )
        valid = (
            'assert_eq ! (lib . call ("IS_VALID_BCD" , & [Value :: Byte (0x12)])'
            " . unwrap () , Value :: Bool (true))"
        )
        invalid_domain = (
            'assert ! (lib . call ("IS_VALID_BCD" , & [Value :: Bool (true)])'
            " . is_err ())"
        )
        self.assertEqual(
            subject._behavior_for_assertion(
                key=key, expression=valid, index=0, kind="macro:assert_eq"
            ),
            (subject.COMPARISON, "BCD_NIBBLE_VALIDITY"),
        )
        self.assertEqual(
            subject._behavior_for_assertion(
                key=key, expression=invalid_domain, index=5, kind="macro:assert"
            ),
            (subject.COMPARISON, "VALIDATION_DOMAIN_REJECTION"),
        )

    def test_nonfinite_error_text_is_an_observation_not_an_oracle(self) -> None:
        key = (
            "crates/trust-runtime/tests/stdlib_conv_full.rs",
            "string_to_real_rejects_non_finite_text",
        )
        expression = (
            'assert ! (err . to_string () . contains ("overflow") || '
            'err . to_string () . contains ("type") , '
            '"expected finite-value conversion error, got {err}")'
        )
        self.assertTrue(
            subject._is_non_oracle_observation(
                key=key, expression=expression, kind="macro:assert"
            )
        )
        self.assertFalse(
            subject._is_non_oracle_observation(
                key=key,
                expression='lib . call ("STRING_TO_REAL") . expect_err ()',
                kind="method:expect_err",
            )
        )

    def test_semantic_lock_detects_same_kind_value_and_binding_replacement(self) -> None:
        key = ("crate/tests.rs", "case")
        inventory = {
            "DISC": {
                "path": key[0],
                "test_name": key[1],
                "assertions": [
                    {
                        "kind": "macro:assert_eq",
                        "expression": "assert_eq ! (value, 7)",
                    }
                ],
            }
        }
        changed = {
            "DISC": {
                **inventory["DISC"],
                "assertions": [
                    {
                        "kind": "macro:assert_eq",
                        "expression": "assert_eq ! (value, 999)",
                    }
                ],
            }
        }
        with patch.object(subject, "TEST_BINDINGS", {key: ("INV", "BEHAVIOR")}):
            locked = subject._review_lock_document(inventory)
            with tempfile.TemporaryDirectory() as directory:
                root = Path(directory)
                path = root / "lock.json"
                path.write_text(json.dumps(locked))
                with self.assertRaisesRegex(ValueError, "expressions or bindings drifted"):
                    subject._validate_review_lock(root, changed, Path("lock.json"))
        with patch.object(subject, "TEST_BINDINGS", {key: ("INV", "OTHER")}):
            with tempfile.TemporaryDirectory() as directory:
                root = Path(directory)
                path = root / "lock.json"
                path.write_text(json.dumps(locked))
                with self.assertRaisesRegex(ValueError, "expressions or bindings drifted"):
                    subject._validate_review_lock(root, inventory, Path("lock.json"))


if __name__ == "__main__":
    unittest.main()
