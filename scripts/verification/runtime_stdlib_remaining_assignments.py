"""Generate reviewed assignments for the frozen remaining runtime stdlib batch."""

from __future__ import annotations

import argparse
import hashlib
import json
import tomllib
from collections import Counter
from collections.abc import Mapping, Sequence
from pathlib import Path
from typing import Any

from scripts.verification.mapping_assertions import validate_assertion_batch
from scripts.verification.runtime_bytecode_assignments import generate_path_plans


TestKey = tuple[str, str]
BehaviorSelector = str | int
BehaviorRef = tuple[str, BehaviorSelector]

CONVERSION = "RT_STDLIB_CONVERSION_001"
COMPARISON = "RT_STDLIB_COMPARISON_VALIDATE_001"
NUMERIC_BIT = "RT_STDLIB_NUMERIC_BIT_001"
SELECTION = "RT_STDLIB_SELECTION_001"
STRING = "RT_STDLIB_STRING_001"
ASSERTION = "RT_STDLIB_ASSERTION_001"
VALUE_MODEL = "RT_CORE_VALUE_PROGRAM_MODEL_001"
STRING_BOUND = "VM_SEAM_STRING_BOUND_001"
TIME_FUNCTIONS = "IEC_STANDARD_TIME_FUNCTIONS_001"
CALL_BINDING = "VM_CALL_BINDING_001"
TIMER = "IEC_TIMER_001"
COUNTER = "RT_STDLIB_COUNTER_STATE_001"
EDGE = "RT_STDLIB_EDGE_STATE_001"
BISTABLE = "RT_STDLIB_BISTABLE_STATE_001"

TEST_BINDINGS: Mapping[TestKey, BehaviorRef] = {
    ("crates/trust-runtime/tests/stdlib_conv.rs", "conversion_functions"): (
        CONVERSION,
        "BOOLEAN_AND_INTEGER_WIDTH_CONVERSION",
    ),
    ("crates/trust-runtime/tests/stdlib_conv_full.rs", "conversion_full"): (
        CONVERSION,
        "ROUND_TO_NEAREST_TIES_EVEN",
    ),
    (
        "crates/trust-runtime/tests/stdlib_conv_full.rs",
        "string_to_real_rejects_non_finite_text",
    ): (CONVERSION, "NONFINITE_TEXT_TO_REAL_REJECTION"),
    ("crates/trust-runtime/tests/iec_examples.rs", "conversion_usage_examples"): (
        CONVERSION,
        "TRUNCATION_AND_NUMERIC_WIDENING",
    ),
    ("crates/trust-runtime/tests/stdlib_enum_validate.rs", "enum_comparisons"): (
        COMPARISON,
        "SAME_ENUM_EQ_NE",
    ),
    (
        "crates/trust-runtime/tests/stdlib_enum_validate.rs",
        "is_valid_real_values",
    ): (COMPARISON, "REAL_FINITE_AND_NONFINITE_VALIDITY"),
    (
        "crates/trust-runtime/tests/stdlib_enum_validate.rs",
        "is_valid_bcd_bit_strings",
    ): (COMPARISON, "BCD_NIBBLE_VALIDITY"),
    ("crates/trust-runtime/tests/stdlib_numeric.rs", "numeric_functions"): (
        NUMERIC_BIT,
        "NUMERICAL_REPRESENTATIVE_VALUES",
    ),
    ("crates/trust-runtime/tests/stdlib_bit_full.rs", "bit_full"): (
        NUMERIC_BIT,
        "SHIFT_AND_ROTATE_WIDTH_VALUES",
    ),
    ("crates/trust-runtime/tests/stdlib_select_full.rs", "selection_full"): (
        SELECTION,
        "SEL_MIN_MAX_RUNTIME_VALUES",
    ),
    ("crates/trust-runtime/tests/expr_calls.rs", "stdlib_named_args"): (
        SELECTION,
        "SEL_NAMED_ARGUMENT_BINDING",
    ),
    ("crates/trust-runtime/tests/stdlib_string.rs", "string_functions"): (
        STRING,
        "ASCII_STRING_FUNCTION_RESULTS",
    ),
    ("crates/trust-runtime/tests/stdlib_string_full.rs", "string_full"): (
        STRING,
        "ASCII_STRING_FUNCTION_RESULTS",
    ),
    (
        "crates/trust-runtime/tests/stdlib_string_full.rs",
        "string_full_non_ascii_uses_character_elements",
    ): (STRING, "UNICODE_SCALAR_STRING_ELEMENTS"),
    (
        "crates/trust-runtime/tests/stdlib_string.rs",
        "bounded_string_assignment_and_concat_respect_declared_capacity",
    ): (STRING_BOUND, "STRING_3_CONCAT_ASSIGNMENT_TRUNCATION"),
    (
        "crates/trust-runtime/tests/stdlib_assertions.rs",
        "assertion_functions_pass_when_conditions_hold",
    ): (ASSERTION, "ASSERTION_SUCCESS_NULL"),
    (
        "crates/trust-runtime/tests/stdlib_assertions.rs",
        "assertion_functions_fail_with_assertion_error",
    ): (ASSERTION, "ASSERTION_CONDITION_FAILURE"),
    (
        "crates/trust-runtime/tests/stdlib_assertions.rs",
        "assertion_failure_messages_use_user_facing_value_strings",
    ): (ASSERTION, "ASSERTION_RELATIONAL_MESSAGE_FORMS"),
    (
        "crates/trust-runtime/tests/stdlib_assertions.rs",
        "assertion_comparison_functions_coerce_numeric_types",
    ): (ASSERTION, "ASSERTION_SUCCESS_NULL"),
    ("crates/trust-runtime/tests/datetime_profile.rs", "default_profile"): (
        VALUE_MODEL,
        "DATETIME_DEFAULT_PROFILE_AND_LONG_UNITS",
    ),
    ("crates/trust-runtime/tests/datetime_profile.rs", "timezone_naive"): (
        VALUE_MODEL,
        "DATETIME_RANGE_AND_TIMEZONE_REJECTION",
    ),
    ("crates/trust-runtime/tests/datetime_range.rs", "out_of_range_error"): (
        VALUE_MODEL,
        "DATETIME_RANGE_AND_TIMEZONE_REJECTION",
    ),
    ("crates/trust-runtime/tests/datetime_types.rs", "ltime_epoch_and_units"): (
        VALUE_MODEL,
        "DATETIME_DEFAULT_PROFILE_AND_LONG_UNITS",
    ),
    (
        "crates/trust-runtime/tests/stdlib_split_locals.rs",
        "split_date_writes_function_local_outputs_in_runtime_and_vm",
    ): (TIME_FUNCTIONS, "REGISTERED_COMPONENT_SPLIT"),
    ("crates/trust-runtime/tests/fb_timers.rs", "ton_tof_tp"): (
        TIMER,
        "TON_TIME_BASIC_DELAY",
    ),
    ("crates/trust-runtime/tests/iec_timers.rs", "timing_diagrams"): (
        TIMER,
        "TON_TIME_BASIC_DELAY",
    ),
    ("crates/trust-runtime/tests/iec_counters.rs", "counter_examples"): (
        COUNTER,
        "CTU_RISING_EDGE_RESET_AND_THRESHOLD",
    ),
    ("crates/trust-runtime/tests/fb_counters.rs", "ctu_ctd_ctud"): (
        COUNTER,
        "CTU_RISING_EDGE_RESET_AND_THRESHOLD",
    ),
    ("crates/trust-runtime/tests/fb_counters_full.rs", "counter_variants"): (
        COUNTER,
        "CTU_RISING_EDGE_RESET_AND_THRESHOLD",
    ),
    (
        "crates/trust-runtime/tests/fb_counters_full.rs",
        "generic_counter_uses_call_value_type_after_null_default",
    ): (COUNTER, "COUNTER_SELECTED_RUNTIME_VALUE_TYPE"),
    ("crates/trust-runtime/tests/fb_edges.rs", "r_trig_f_trig"): (
        EDGE,
        "R_TRIG_RISING_EDGE_AND_HELD_HIGH",
    ),
    (
        "crates/trust-runtime/tests/fb_edges.rs",
        "difu_difd_aliases_match_edge_behavior",
    ): (EDGE, "DIFU_DIFD_RUNTIME_EDGE_EQUIVALENCE"),
    ("crates/trust-runtime/tests/fb_bistable.rs", "sr_rs"): (
        BISTABLE,
        "SR_LATCH_RESET_AND_SET_DOMINANCE",
    ),
}

REVIEWED_ASSERTION_COUNTS: Mapping[TestKey, int] = {
    ("crates/trust-runtime/tests/datetime_profile.rs", "default_profile"): 2,
    ("crates/trust-runtime/tests/datetime_profile.rs", "timezone_naive"): 1,
    ("crates/trust-runtime/tests/datetime_range.rs", "out_of_range_error"): 3,
    ("crates/trust-runtime/tests/datetime_types.rs", "ltime_epoch_and_units"): 3,
    ("crates/trust-runtime/tests/expr_calls.rs", "stdlib_named_args"): 2,
    ("crates/trust-runtime/tests/fb_bistable.rs", "sr_rs"): 11,
    ("crates/trust-runtime/tests/fb_counters.rs", "ctu_ctd_ctud"): 23,
    ("crates/trust-runtime/tests/fb_counters_full.rs", "counter_variants"): 37,
    (
        "crates/trust-runtime/tests/fb_counters_full.rs",
        "generic_counter_uses_call_value_type_after_null_default",
    ): 3,
    (
        "crates/trust-runtime/tests/fb_edges.rs",
        "difu_difd_aliases_match_edge_behavior",
    ): 9,
    ("crates/trust-runtime/tests/fb_edges.rs", "r_trig_f_trig"): 13,
    ("crates/trust-runtime/tests/fb_timers.rs", "ton_tof_tp"): 28,
    ("crates/trust-runtime/tests/iec_counters.rs", "counter_examples"): 17,
    ("crates/trust-runtime/tests/iec_examples.rs", "conversion_usage_examples"): 7,
    ("crates/trust-runtime/tests/iec_timers.rs", "timing_diagrams"): 31,
    (
        "crates/trust-runtime/tests/stdlib_assertions.rs",
        "assertion_comparison_functions_coerce_numeric_types",
    ): 1,
    (
        "crates/trust-runtime/tests/stdlib_assertions.rs",
        "assertion_failure_messages_use_user_facing_value_strings",
    ): 9,
    (
        "crates/trust-runtime/tests/stdlib_assertions.rs",
        "assertion_functions_fail_with_assertion_error",
    ): 16,
    (
        "crates/trust-runtime/tests/stdlib_assertions.rs",
        "assertion_functions_pass_when_conditions_hold",
    ): 11,
    ("crates/trust-runtime/tests/stdlib_bit_full.rs", "bit_full"): 9,
    ("crates/trust-runtime/tests/stdlib_conv.rs", "conversion_functions"): 5,
    ("crates/trust-runtime/tests/stdlib_conv_full.rs", "conversion_full"): 38,
    (
        "crates/trust-runtime/tests/stdlib_conv_full.rs",
        "string_to_real_rejects_non_finite_text",
    ): 4,
    ("crates/trust-runtime/tests/stdlib_enum_validate.rs", "enum_comparisons"): 4,
    (
        "crates/trust-runtime/tests/stdlib_enum_validate.rs",
        "is_valid_bcd_bit_strings",
    ): 6,
    (
        "crates/trust-runtime/tests/stdlib_enum_validate.rs",
        "is_valid_real_values",
    ): 4,
    ("crates/trust-runtime/tests/stdlib_numeric.rs", "numeric_functions"): 7,
    ("crates/trust-runtime/tests/stdlib_select_full.rs", "selection_full"): 8,
    (
        "crates/trust-runtime/tests/stdlib_split_locals.rs",
        "split_date_writes_function_local_outputs_in_runtime_and_vm",
    ): 8,
    (
        "crates/trust-runtime/tests/stdlib_string.rs",
        "bounded_string_assignment_and_concat_respect_declared_capacity",
    ): 3,
    ("crates/trust-runtime/tests/stdlib_string.rs", "string_functions"): 2,
    ("crates/trust-runtime/tests/stdlib_string_full.rs", "string_full"): 8,
    (
        "crates/trust-runtime/tests/stdlib_string_full.rs",
        "string_full_non_ascii_uses_character_elements",
    ): 5,
}

REVIEWED_KIND_COUNTS = Counter(
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
)
SETUP_KINDS = frozenset({"method:unwrap", "method:expect"})
REVIEW_LOCK_PATH = Path(
    "verification/p18/review-locks/runtime-stdlib-33-semantic-lock.json"
)


def combine_inventory_documents(
    documents: Sequence[Mapping[str, Any]],
) -> dict[str, Any]:
    inventories: dict[str, Any] = {}
    for document in documents:
        rows = document.get("inventories")
        if not isinstance(rows, Mapping):
            raise ValueError("runtime stdlib assertion document lacks inventories")
        for discovery_id, row in rows.items():
            if discovery_id in inventories:
                raise ValueError(f"duplicate runtime stdlib inventory {discovery_id}")
            inventories[str(discovery_id)] = row
    return {"schema_version": 2, "inventories": inventories}


def _canonical_inventories(
    root: Path, document: Mapping[str, Any]
) -> dict[str, dict[str, Any]]:
    if set(document) != {"schema_version", "inventories"} or document.get(
        "schema_version"
    ) != 2:
        raise ValueError("runtime stdlib assertion document fields drifted")
    rows = document.get("inventories")
    if not isinstance(rows, Mapping):
        raise ValueError("runtime stdlib assertion document lacks inventories")
    requested = []
    batch_rows = []
    saved: dict[str, dict[str, Any]] = {}
    for discovery_id, raw in sorted(rows.items()):
        if not isinstance(discovery_id, str) or not isinstance(raw, Mapping):
            raise ValueError("runtime stdlib inventory is invalid")
        path = _string(raw, "path", discovery_id)
        name = _string(raw, "test_name", discovery_id)
        requested.append({"discovery_id": discovery_id, "path": path, "name": name})
        batch_rows.append(
            {key: value for key, value in raw.items() if key != "inventory_digest"}
        )
        saved[discovery_id] = dict(raw)
    canonical = validate_assertion_batch(
        root,
        {"schema_version": 2, "inventories": batch_rows},
        requested,
    )
    if canonical != saved:
        raise ValueError("saved runtime stdlib assertion inventory is not canonical")
    return canonical


def select_live_inventories(
    root: Path, document: Mapping[str, Any]
) -> dict[str, Any]:
    inventories = _canonical_inventories(root, document)
    denominator = tomllib.loads(
        (root / "verification/test-catalog-denominator.toml").read_text()
    )
    expected = {
        str(row["discovery_id"]): (str(row["path"]), str(row["name"]))
        for row in denominator.get("reviews", [])
        if isinstance(row, Mapping)
        and (str(row.get("path")), str(row.get("name"))) in TEST_BINDINGS
        and row.get("disposition") == "reviewed_nonmapping"
        and row.get("reason_code") == "no_reviewed_spec_or_invariant_binding"
    }
    if set(expected.values()) != set(TEST_BINDINGS):
        raise ValueError("live runtime stdlib denominator does not equal reviewed bindings")
    selected = {
        discovery_id: inventories[discovery_id]
        for discovery_id in sorted(expected)
        if discovery_id in inventories
    }
    if set(selected) != set(expected):
        raise ValueError("saved inventory lacks live runtime stdlib mapping debt")
    return {"schema_version": 2, "inventories": selected}


def generate_assignments(
    root: Path,
    document: Mapping[str, Any],
    *,
    review_lock_path: Path = REVIEW_LOCK_PATH,
) -> dict[str, Any]:
    inventories = _canonical_inventories(root, document)
    _validate_review_lock(root, inventories, review_lock_path)
    if set(REVIEWED_ASSERTION_COUNTS) != set(TEST_BINDINGS):
        raise ValueError("reviewed runtime stdlib assertion counts are not exhaustive")
    behavior_indexes = _behavior_indexes(root)
    rows = []
    seen: set[TestKey] = set()
    observed_kinds: Counter[str] = Counter()
    direct_oracle_count = 0
    for discovery_id, raw in sorted(inventories.items()):
        path = _string(raw, "path", discovery_id)
        name = _string(raw, "test_name", discovery_id)
        key = (path, name)
        if key not in TEST_BINDINGS or key in seen:
            raise ValueError(f"unexpected or duplicate runtime stdlib test {path}:{name}")
        seen.add(key)
        if raw.get("unresolved_assertion_helpers") != []:
            raise ValueError(f"{path}:{name} has unresolved assertion helpers")
        assertions = raw.get("assertions")
        if (
            not isinstance(assertions, Sequence)
            or isinstance(assertions, (str, bytes))
            or len(assertions) != REVIEWED_ASSERTION_COUNTS[key]
        ):
            raise ValueError(f"{path}:{name} assertion count drifted")
        assertion_rows = []
        setup_ids = []
        non_oracle_ids = []
        for index, assertion in enumerate(assertions):
            if not isinstance(assertion, Mapping):
                raise ValueError(f"{path}:{name} assertion is invalid")
            assertion_id = _string(assertion, "assertion_id", name)
            expression = _string(assertion, "expression", assertion_id)
            kind = _string(assertion, "kind", assertion_id)
            observed_kinds[kind] += 1
            invariant_id, selector = _behavior_for_assertion(
                key=key,
                expression=expression,
                index=index,
                kind=kind,
            )
            assertion_rows.append(
                {
                    "assertion_id": assertion_id,
                    "invariant_id": invariant_id,
                    "behavior_index": _resolve_behavior_index(
                        behavior_indexes, invariant_id, selector
                    ),
                }
            )
            if kind in SETUP_KINDS:
                setup_ids.append(assertion_id)
            elif _is_non_oracle_observation(key=key, expression=expression, kind=kind):
                non_oracle_ids.append(assertion_id)
            else:
                direct_oracle_count += 1
        if len(setup_ids) + len(non_oracle_ids) == len(assertion_rows):
            raise ValueError(f"{path}:{name} has no direct product oracle")
        rows.append(
            {
                "path": path,
                "name": name,
                "assertion_assignments": assertion_rows,
                "setup_assertion_ids": setup_ids,
                "non_oracle_assertion_ids": non_oracle_ids,
            }
        )
    if seen != set(TEST_BINDINGS):
        raise ValueError("runtime stdlib assignments are not exhaustive")
    if observed_kinds != REVIEWED_KIND_COUNTS or direct_oracle_count != 316:
        raise ValueError("reviewed runtime stdlib assertion-kind profile drifted")
    return {
        "schema_version": 3,
        "basis": "reviewed_runtime_stdlib_semantic_group_and_assertion_rules",
        "assignments": rows,
    }


def _behavior_for_assertion(
    *,
    key: TestKey,
    expression: str,
    index: int,
    kind: str,
) -> BehaviorRef:
    path, name = key
    default = TEST_BINDINGS[key]
    if name == "conversion_usage_examples":
        if kind in SETUP_KINDS or '"a1"' in expression or '"a2"' in expression:
            return (CONVERSION, "TRUNCATION_AND_NUMERIC_WIDENING")
        if any(f'"r{number}"' in expression for number in range(1, 5)):
            return (CONVERSION, "ROUND_TO_NEAREST_TIES_EVEN")
        raise ValueError(f"unreviewed conversion example assertion {expression}")
    if name == "conversion_functions":
        if "REAL_TO_INT" in expression:
            return (CONVERSION, "ROUND_TO_NEAREST_TIES_EVEN")
        if "INT_TO_REAL" in expression:
            return (CONVERSION, "TRUNCATION_AND_NUMERIC_WIDENING")
        if any(token in expression for token in ("BOOL_TO_INT", "INT_TO_DINT", "DINT_TO_INT")):
            return (CONVERSION, "BOOLEAN_AND_INTEGER_WIDTH_CONVERSION")
        raise ValueError(f"unreviewed conversion assertion {expression}")
    if name == "conversion_full":
        return _conversion_full_behavior(expression)
    if name == "is_valid_real_values":
        return (
            COMPARISON,
            "VALIDATION_DOMAIN_REJECTION"
            if "Value :: DInt" in expression
            else "REAL_FINITE_AND_NONFINITE_VALIDITY",
        )
    if name == "is_valid_bcd_bit_strings":
        return (
            COMPARISON,
            "VALIDATION_DOMAIN_REJECTION"
            if '& [Value :: Bool' in expression
            else "BCD_NIBBLE_VALIDITY",
        )
    if name == "bit_full":
        return (
            NUMERIC_BIT,
            "SHIFT_AND_ROTATE_WIDTH_VALUES"
            if any(f'"{function}"' in expression for function in ("SHL", "SHR", "ROL", "ROR"))
            else "BITWISE_SAME_AND_MIXED_WIDTH_VALUES",
        )
    if name == "selection_full":
        if kind in SETUP_KINDS or "red . clone" in expression:
            return (SELECTION, "ENUM_SELECTION_RUNTIME_IDENTITY")
        if any(f'"{function}"' in expression for function in ("LIMIT", "MUX")):
            return (SELECTION, "LIMIT_AND_MUX_RUNTIME_VALUES")
        if any(f'"{function}"' in expression for function in ("SEL", "MIN", "MAX")):
            return (SELECTION, "SEL_MIN_MAX_RUNTIME_VALUES")
        raise ValueError(f"unreviewed selection assertion {expression}")
    if name == "assertion_functions_fail_with_assertion_error":
        if 'contains ("ASSERT_NEAR")' in expression or 'contains ("delta")' in expression:
            return (ASSERTION, "ASSERT_NEAR_FAILURE_CONTEXT")
        if "message . as_ref" in expression or "! message . contains" in expression:
            return (ASSERTION, "ASSERTION_RELATIONAL_MESSAGE_FORMS")
        return (ASSERTION, "ASSERTION_CONDITION_FAILURE")
    if name == "assertion_failure_messages_use_user_facing_value_strings":
        if "message . as_ref" in expression or "! message . contains" in expression:
            return (ASSERTION, "ASSERTION_RELATIONAL_MESSAGE_FORMS")
        return (ASSERTION, "ASSERTION_CONDITION_FAILURE")
    if name == "ton_tof_tp":
        if index < 8:
            return (TIMER, "TON_TIME_BASIC_DELAY")
        if index < 17:
            return (TIMER, "TOF_TIME_POST_EXPIRY_HOLD")
        return (TIMER, "TP_TIME_POST_EXPIRY_HOLD_AND_REARM")
    if name == "timing_diagrams":
        if kind in SETUP_KINDS or "_ton" in expression:
            return (TIMER, "TON_TIME_BASIC_DELAY")
        if "_tof" in expression:
            return (TIMER, "TOF_TIME_POST_EXPIRY_HOLD")
        if "_tp" in expression:
            return (TIMER, "TP_TIME_POST_EXPIRY_HOLD_AND_REARM")
        raise ValueError(f"unreviewed timer assertion {expression}")
    if name == "ctu_ctd_ctud":
        if index < 8:
            return (COUNTER, "CTU_RISING_EDGE_RESET_AND_THRESHOLD")
        if index < 14:
            return (COUNTER, "CTD_RISING_EDGE_LOAD_AND_ZERO")
        return (COUNTER, "CTUD_EDGE_RESET_AND_OUTPUTS")
    if name == "counter_examples":
        if kind in SETUP_KINDS or "_u" in expression:
            return (COUNTER, "CTU_RISING_EDGE_RESET_AND_THRESHOLD")
        if "_d" in expression:
            return (COUNTER, "CTD_RISING_EDGE_LOAD_AND_ZERO")
        raise ValueError(f"unreviewed counter example assertion {expression}")
    if name == "counter_variants":
        if kind in SETUP_KINDS:
            return (COUNTER, "CTU_RISING_EDGE_RESET_AND_THRESHOLD")
        if "udint" in expression or '"q_ud"' in expression:
            return (COUNTER, "COUNTER_SELECTED_RUNTIME_VALUE_TYPE")
        if "_ud" in expression:
            return (COUNTER, "CTUD_EDGE_RESET_AND_OUTPUTS")
        if "_u" in expression:
            return (COUNTER, "CTU_RISING_EDGE_RESET_AND_THRESHOLD")
        if "_d" in expression:
            return (COUNTER, "CTD_RISING_EDGE_LOAD_AND_ZERO")
        raise ValueError(f"unreviewed counter assertion {expression}")
    if name == "r_trig_f_trig":
        if kind in SETUP_KINDS or '"q_r"' in expression:
            return (EDGE, "R_TRIG_RISING_EDGE_AND_HELD_HIGH")
        if '"q_f"' in expression:
            return (EDGE, "F_TRIG_COLD_START_AND_FALLING_EDGE")
        raise ValueError(f"unreviewed edge assertion {expression}")
    if name == "sr_rs":
        if kind in SETUP_KINDS or '"q_sr"' in expression:
            return (BISTABLE, "SR_LATCH_RESET_AND_SET_DOMINANCE")
        if '"q_rs"' in expression:
            return (BISTABLE, "RS_LATCH_RESET_AND_RESET_DOMINANCE")
        raise ValueError(f"unreviewed bistable assertion {expression}")
    if key not in TEST_BINDINGS:
        raise ValueError(f"unreviewed runtime stdlib test {path}:{name}")
    return default


def _conversion_full_behavior(expression: str) -> BehaviorRef:
    if "REAL_TO_INT" in expression:
        return (CONVERSION, "ROUND_TO_NEAREST_TIES_EVEN")
    if any(token in expression for token in ("TRUNC_INT", "LREAL_TO_DINT", "TO_LREAL")):
        return (CONVERSION, "TRUNCATION_AND_NUMERIC_WIDENING")
    if any(
        token in expression
        for token in ("WORD_TO_BYTE", "BYTE_TO_WORD", "SINT_TO_WORD", "WORD_TO_SINT")
    ):
        return (CONVERSION, "BIT_WIDTH_TRANSFER")
    if "BOOL_TO_INT" in expression:
        return (CONVERSION, "BOOLEAN_AND_INTEGER_WIDTH_CONVERSION")
    if any(token in expression for token in ("REAL_TO_DWORD", "DWORD_TO_REAL")):
        return (CONVERSION, "REAL_BINARY_TRANSFER")
    if "TIME_TO_DWORD" in expression and "from_millis (- 1)" in expression:
        return (CONVERSION, "TIME_TO_DWORD_NEGATIVE_REJECTION")
    if "TIME_TO_DWORD" in expression and "u32 :: MAX" in expression:
        return (CONVERSION, "TIME_TO_DWORD_ABOVE_MAX_REJECTION")
    if any(
        token in expression
        for token in (
            "DT_TO_DATE",
            "DT_TO_TOD",
            "LDT_TO_DT",
            "LTOD_TO_TOD",
            "TOD_TO_LTOD",
            "TIME_TO_DWORD",
            "DWORD_TO_TIME",
        )
    ):
        return (CONVERSION, "DATE_TIME_AND_DURATION_CONVERSION")
    if any(
        token in expression
        for token in (
            "STRING_TO_WSTRING",
            "STRING_TO_CHAR",
            "BYTE_TO_CHAR",
            "CHAR_TO_BYTE",
            "WCHAR_TO_WORD",
            "WCHAR_TO_CHAR",
        )
    ):
        return (CONVERSION, "CHARACTER_FAMILY_REPRESENTATIVES")
    if any(
        token in expression
        for token in (
            "DINT_TO_STRING",
            "REAL_TO_STRING",
            "DWORD_TO_STRING",
            "STRING_TO_DINT",
        )
    ):
        return (CONVERSION, "TEXT_CONVERSION_REPRESENTATIVES")
    if "0xFA" in expression:
        return (CONVERSION, "INVALID_BCD_REJECTION")
    if any(token in expression for token in ("USINT_TO_BCD_BYTE", "BYTE_BCD_TO_UINT")):
        return (CONVERSION, "VALID_BCD_CONVERSION")
    raise ValueError(f"unreviewed full conversion assertion {expression}")


def _is_non_oracle_observation(
    *, key: TestKey, expression: str, kind: str
) -> bool:
    return (
        key
        == (
            "crates/trust-runtime/tests/stdlib_conv_full.rs",
            "string_to_real_rejects_non_finite_text",
        )
        and kind == "macro:assert"
        and "expected finite-value conversion error" in expression
    )


def _review_lock_document(
    inventories: Mapping[str, Mapping[str, Any]],
) -> dict[str, Any]:
    rows = []
    seen: set[TestKey] = set()
    for raw in inventories.values():
        path = _string(raw, "path", "review lock inventory")
        name = _string(raw, "test_name", "review lock inventory")
        key = (path, name)
        binding = TEST_BINDINGS.get(key)
        assertions = raw.get("assertions")
        if (
            binding is None
            or key in seen
            or not isinstance(assertions, Sequence)
            or isinstance(assertions, (str, bytes))
        ):
            raise ValueError(f"review lock has unexpected inventory {path}:{name}")
        seen.add(key)
        expressions = []
        for index, assertion in enumerate(assertions):
            if not isinstance(assertion, Mapping):
                raise ValueError(f"review lock assertion is invalid for {path}:{name}")
            kind = _string(assertion, "kind", f"{path}:{name}")
            expression = _string(assertion, "expression", f"{path}:{name}")
            behavior_invariant_id, behavior_selector = _behavior_for_assertion(
                key=key,
                expression=expression,
                index=index,
                kind=kind,
            )
            expressions.append(
                {
                    "kind": kind,
                    "expression": expression,
                    "invariant_id": behavior_invariant_id,
                    "behavior_selector": behavior_selector,
                    "disposition": (
                        "setup"
                        if kind in SETUP_KINDS
                        else "non_oracle"
                        if _is_non_oracle_observation(
                            key=key,
                            expression=expression,
                            kind=kind,
                        )
                        else "oracle"
                    ),
                }
            )
        invariant_id, selector = binding
        semantic_record = {
            "path": path,
            "name": name,
            "invariant_id": invariant_id,
            "behavior_selector": selector,
            "assertions": sorted(
                expressions,
                key=lambda row: (
                    row["kind"],
                    row["expression"],
                    row["invariant_id"],
                    str(row["behavior_selector"]),
                    row["disposition"],
                ),
            ),
        }
        digest = hashlib.sha256(
            json.dumps(
                semantic_record,
                sort_keys=True,
                separators=(",", ":"),
            ).encode()
        ).hexdigest()
        rows.append(
            {
                "path": path,
                "name": name,
                "invariant_id": invariant_id,
                "behavior_selector": selector,
                "assertion_count": len(expressions),
                "semantic_digest": f"sha256:{digest}",
            }
        )
    if seen != set(TEST_BINDINGS):
        raise ValueError("review lock does not cover the exact binding set")
    return {
        "schema_version": 1,
        "batch": "runtime-stdlib-33",
        "tests": sorted(rows, key=lambda row: (row["path"], row["name"])),
    }


def _validate_review_lock(
    root: Path,
    inventories: Mapping[str, Mapping[str, Any]],
    review_lock_path: Path,
) -> None:
    path = review_lock_path if review_lock_path.is_absolute() else root / review_lock_path
    try:
        saved = json.loads(path.read_text())
    except FileNotFoundError as error:
        raise ValueError(f"reviewed semantic lock is missing: {review_lock_path}") from error
    expected = _review_lock_document(inventories)
    if saved != expected:
        raise ValueError(
            "runtime stdlib reviewed expressions or bindings drifted from semantic lock"
        )


def _behavior_indexes(root: Path) -> dict[str, int]:
    result: dict[str, int] = {}
    invariant_ids = {invariant_id for invariant_id, _ in TEST_BINDINGS.values()}
    for invariant_id in invariant_ids:
        matches = list((root / "verification/invariants").glob(f"**/{invariant_id}.toml"))
        if len(matches) != 1:
            raise ValueError(f"expected one {invariant_id} invariant")
        record = tomllib.loads(matches[0].read_text())
        behaviors = record.get("behavior")
        if not isinstance(behaviors, list):
            raise ValueError(f"{invariant_id} lacks behaviors")
        result[f"{invariant_id}#count"] = len(behaviors)
        for index, behavior in enumerate(behaviors):
            partition = behavior.get("partition") if isinstance(behavior, Mapping) else None
            name = partition.get("equals") if isinstance(partition, Mapping) else None
            if isinstance(name, str) and name:
                result[f"{invariant_id}#{name}"] = index
    return result


def _resolve_behavior_index(
    indexes: Mapping[str, int], invariant_id: str, selector: BehaviorSelector
) -> int:
    if isinstance(selector, int):
        count = indexes.get(f"{invariant_id}#count")
        if count is None or not 0 <= selector < count:
            raise ValueError(f"{invariant_id} behavior index {selector} is invalid")
        return selector
    try:
        return indexes[f"{invariant_id}#{selector}"]
    except KeyError as error:
        raise ValueError(f"{invariant_id} lacks behavior {selector}") from error


def _string(row: Mapping[str, Any], field: str, label: str) -> str:
    value = row.get(field)
    if not isinstance(value, str) or not value:
        raise ValueError(f"{label} lacks {field}")
    return value


def _rooted(root: Path, path: Path) -> Path:
    return path if path.is_absolute() else root / path


def _write_json(path: Path, document: Mapping[str, Any]) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_text(json.dumps(document, sort_keys=True, indent=2) + "\n")


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--root", type=Path, default=Path("."))
    parser.add_argument("--inventory", type=Path, action="append", required=True)
    parser.add_argument("--selected-inventory-out", type=Path)
    parser.add_argument("--assignments-out", type=Path)
    parser.add_argument("--review-lock", type=Path, default=REVIEW_LOCK_PATH)
    parser.add_argument("--emit-review-lock", action="store_true")
    parser.add_argument("--execution-dir", type=Path)
    parser.add_argument("--plans-out", type=Path)
    parser.add_argument("--reviewed-date")
    parser.add_argument("--timestamp")
    args = parser.parse_args()
    root = args.root.resolve()
    combined = combine_inventory_documents(
        [json.loads(_rooted(root, path).read_text()) for path in args.inventory]
    )
    selected = select_live_inventories(root, combined)
    if args.emit_review_lock:
        if args.selected_inventory_out is not None or args.assignments_out is not None:
            raise ValueError("review-lock emission cannot also generate assignments")
        lock = _review_lock_document(_canonical_inventories(root, selected))
        _write_json(_rooted(root, args.review_lock), lock)
        print(f"runtime stdlib semantic review lock generated: {len(lock['tests'])}")
        return 0
    if args.selected_inventory_out is None or args.assignments_out is None:
        raise ValueError("assignment outputs are required unless emitting review lock")
    assignments = generate_assignments(
        root,
        selected,
        review_lock_path=args.review_lock,
    )
    _write_json(_rooted(root, args.selected_inventory_out), selected)
    _write_json(_rooted(root, args.assignments_out), assignments)
    print(f"remaining runtime stdlib assignments generated: {len(assignments['assignments'])}")
    plan_args = (args.execution_dir, args.plans_out, args.reviewed_date, args.timestamp)
    if any(value is not None for value in plan_args):
        if not all(value is not None for value in plan_args):
            raise ValueError("runtime stdlib plan arguments must be complete")
        index = generate_path_plans(
            root=root,
            inventory_document=selected,
            assignments_document=assignments,
            execution_dir=_rooted(root, args.execution_dir),
            output_dir=_rooted(root, args.plans_out),
            reviewed_date=args.reviewed_date,
            timestamp=args.timestamp,
            target_profile="runtime-remaining",
        )
        print(
            "remaining runtime stdlib catalog plans generated: "
            f"{index['expected_count']} tests across {len(index['paths'])} paths"
        )
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
