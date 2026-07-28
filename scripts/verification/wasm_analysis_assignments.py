"""Generate reviewed assignments for the functional browser-analysis batch."""

from __future__ import annotations

import argparse
import json
import tomllib
from collections import Counter
from collections.abc import Mapping, Sequence
from pathlib import Path
from typing import Any

from scripts.verification.mapping_assertions import validate_assertion_batch
from scripts.verification.runtime_bytecode_assignments import generate_path_plans


INVARIANT = "BROWSER_ANALYSIS_PROJECTION_001"
REVIEW_LOCK_PATH = Path(
    "verification/p18/review-locks/"
    "wasm-analysis-functional-15-semantic-lock.json"
)

PARITY = "REVIEWED_NATIVE_FEATURE_PARITY"
PROJECTION = "BROWSER_HOVER_COMPLETION_PROJECTION"
PLAIN_IDENTITY = "PLAIN_DOCUMENT_IDENTITY_SYMBOL_PROJECTION"
PUNCTUATION = "PUNCTUATION_ADJACENT_SYMBOL_RESOLUTION"
MALFORMED_JSON = "MALFORMED_DOCUMENT_JSON_REJECTION"
JSON_RESULTS = "JSON_APPLY_STATUS_AND_DIAGNOSTICS"
FULL_REPLACEMENT = "REPEATED_FULL_DOCUMENT_REPLACEMENT"

TestKey = tuple[str, str]

TEST_BINDINGS: Mapping[TestKey, str] = {
    (
        "crates/trust-wasm-analysis/tests/mp010_parity/"
        "mp010_parity_part_01.rs",
        "diagnostics_parity_matches_native_analysis",
    ): PARITY,
    (
        "crates/trust-wasm-analysis/tests/mp010_parity/"
        "mp010_parity_part_01.rs",
        "hover_and_completion_parity_matches_native_analysis",
    ): PARITY,
    (
        "crates/trust-wasm-analysis/tests/mp010_parity/"
        "mp010_parity_part_01.rs",
        "completion_for_struct_member_access_returns_expected_members",
    ): PROJECTION,
    (
        "crates/trust-wasm-analysis/tests/mp010_parity/"
        "mp010_parity_part_01.rs",
        "completion_for_statement_prefixes_exposes_program_variables",
    ): PROJECTION,
    (
        "crates/trust-wasm-analysis/tests/mp010_parity/"
        "mp010_parity_part_01.rs",
        "hover_function_block_signature_in_wasm_uses_declared_types",
    ): PROJECTION,
    (
        "crates/trust-wasm-analysis/tests/mp010_parity/"
        "mp010_parity_part_02.rs",
        "definition_references_and_rename_work_with_plain_demo_uris",
    ): PLAIN_IDENTITY,
    (
        "crates/trust-wasm-analysis/tests/mp010_parity/"
        "mp010_parity_part_02.rs",
        "definition_supports_boundary_cursor_positions_with_plain_demo_uris",
    ): PUNCTUATION,
    (
        "crates/trust-wasm-analysis/tests/mp010_parity/"
        "mp010_parity_part_02.rs",
        "references_and_rename_work_with_plain_demo_uris",
    ): PLAIN_IDENTITY,
    (
        "crates/trust-wasm-analysis/tests/mp010_parity/"
        "mp010_parity_part_03.rs",
        "definition_for_pump_controller_type_with_plain_demo_uris_returns_target_uri",
    ): PLAIN_IDENTITY,
    (
        "crates/trust-wasm-analysis/tests/mp010_parity/"
        "mp010_parity_part_03.rs",
        "references_for_program_variable_work_with_plain_demo_uris",
    ): PLAIN_IDENTITY,
    (
        "crates/trust-wasm-analysis/tests/mp010_parity/"
        "mp010_parity_part_03.rs",
        "document_highlight_for_local_symbol_returns_multiple_occurrences",
    ): PLAIN_IDENTITY,
    (
        "crates/trust-wasm-analysis/tests/mp010_parity/"
        "mp010_parity_part_03.rs",
        "definition_references_and_rename_accept_punctuation_adjacent_cursor_positions",
    ): PUNCTUATION,
    (
        "crates/trust-wasm-analysis/tests/mp010_parity/"
        "mp010_parity_part_03.rs",
        "wasm_json_adapter_contract_is_stable",
    ): JSON_RESULTS,
    (
        "crates/trust-wasm-analysis/tests/mp010_parity/"
        "mp010_parity_part_03.rs",
        "browser_host_smoke_apply_documents_then_diagnostics_round_trip",
    ): JSON_RESULTS,
    (
        "crates/trust-wasm-analysis/tests/mp010_parity/"
        "mp010_parity_part_04.rs",
        "multi_document_incremental_update_flow_handles_realistic_edit_streams",
    ): FULL_REPLACEMENT,
}


def select_live_inventories(
    *,
    root: Path,
    inventory_documents: Sequence[Mapping[str, Any]],
) -> dict[str, Any]:
    merged: dict[str, Any] = {}
    for document in inventory_documents:
        for discovery_id, inventory in _canonical_inventories(
            root, document
        ).items():
            if discovery_id in merged and merged[discovery_id] != inventory:
                raise ValueError(
                    f"conflicting browser-analysis inventory {discovery_id}"
                )
            merged[discovery_id] = inventory

    denominator = tomllib.loads(
        (root / "verification/test-catalog-denominator.toml").read_text()
    )
    expected = {
        str(row["discovery_id"]): (str(row["path"]), str(row["name"]))
        for row in denominator.get("reviews", [])
        if isinstance(row, Mapping)
        and row.get("disposition") == "reviewed_nonmapping"
        and row.get("reason_code")
        == "no_reviewed_spec_or_invariant_binding"
        and (str(row.get("path")), str(row.get("name"))) in TEST_BINDINGS
    }
    if set(expected.values()) != set(TEST_BINDINGS):
        raise ValueError(
            "live browser-analysis denominator does not equal reviewed bindings"
        )
    selected = {
        discovery_id: merged[discovery_id]
        for discovery_id in sorted(expected)
        if discovery_id in merged
    }
    if set(selected) != set(expected):
        raise ValueError(
            "saved inventories lack functional browser-analysis debt rows"
        )
    for discovery_id, raw in selected.items():
        if (raw.get("path"), raw.get("test_name")) != expected[discovery_id]:
            raise ValueError(
                f"{discovery_id} browser-analysis inventory identity drifted"
            )
    return {"schema_version": 2, "inventories": selected}


def generate_assignments(
    *,
    root: Path,
    inventory_document: Mapping[str, Any],
    review_lock_path: Path = REVIEW_LOCK_PATH,
) -> dict[str, Any]:
    inventories = _canonical_inventories(root, inventory_document)
    _validate_review_lock(root, inventories, review_lock_path)
    behavior_indexes = _behavior_indexes(root)
    rows = []
    seen: set[TestKey] = set()
    dispositions: Counter[str] = Counter()
    kinds: Counter[str] = Counter()
    behavior_counts: Counter[str] = Counter()
    for discovery_id, raw in sorted(inventories.items()):
        path = _string(raw, "path", discovery_id)
        name = _string(raw, "test_name", discovery_id)
        key = (path, name)
        if key not in TEST_BINDINGS or key in seen:
            raise ValueError(
                f"unexpected or duplicate browser-analysis test {path}:{name}"
            )
        seen.add(key)
        if raw.get("unresolved_assertion_helpers") != []:
            raise ValueError(f"{path}:{name} has unresolved assertion helpers")
        assertions = raw.get("assertions")
        if (
            not isinstance(assertions, Sequence)
            or isinstance(assertions, (str, bytes))
            or not assertions
        ):
            raise ValueError(f"{path}:{name} lacks assertions")
        assertion_rows = []
        setup_ids = []
        for assertion in assertions:
            if not isinstance(assertion, Mapping):
                raise ValueError(f"{path}:{name} assertion is invalid")
            assertion_id = _string(assertion, "assertion_id", name)
            kind = _string(assertion, "kind", assertion_id)
            expression = _string(assertion, "expression", assertion_id)
            behavior_label = _behavior_for_assertion(
                name=name,
                default=TEST_BINDINGS[key],
                kind=kind,
                expression=expression,
            )
            assertion_rows.append(
                {
                    "assertion_id": assertion_id,
                    "invariant_id": INVARIANT,
                    "behavior_index": behavior_indexes[behavior_label],
                }
            )
            disposition = _disposition(kind)
            dispositions[disposition] += 1
            kinds[kind] += 1
            behavior_counts[behavior_label] += 1
            if disposition == "setup_or_precondition":
                setup_ids.append(assertion_id)
        if len(setup_ids) == len(assertion_rows):
            raise ValueError(f"{path}:{name} has no direct product oracle")
        rows.append(
            {
                "path": path,
                "name": name,
                "assertion_assignments": assertion_rows,
                "setup_assertion_ids": setup_ids,
                "non_oracle_assertion_ids": [],
            }
        )
    if seen != set(TEST_BINDINGS):
        raise ValueError("browser-analysis assignments are not exhaustive")
    return {
        "schema_version": 3,
        "basis": (
            "reviewed_browser_analysis_semantic_group_and_assertion_rules"
        ),
        "summary": {
            "tests": len(rows),
            "assertions": sum(dispositions.values()),
            "direct_oracle": dispositions["oracle_bound"],
            "setup_or_precondition": dispositions["setup_or_precondition"],
            "non_oracle_observation": 0,
            "assertion_kinds": dict(sorted(kinds.items())),
            "behavior_assertions": dict(sorted(behavior_counts.items())),
        },
        "assignments": rows,
    }


def _behavior_for_assertion(
    *,
    name: str,
    default: str,
    kind: str,
    expression: str,
) -> str:
    if name != "wasm_json_adapter_contract_is_stable":
        return default
    if kind == "method:expect_err" or "invalid documents json" in expression:
        return MALFORMED_JSON
    return default


def _disposition(kind: str) -> str:
    if kind == "method:expect":
        return "setup_or_precondition"
    if kind in {
        "method:expect_err",
        "macro:assert",
        "macro:assert_eq",
        "macro:assert_ne",
    }:
        return "oracle_bound"
    raise ValueError(
        f"unreviewed browser-analysis assertion kind {kind!r}"
    )


def _canonical_inventories(
    root: Path, document: Mapping[str, Any]
) -> dict[str, dict[str, Any]]:
    if set(document) != {"schema_version", "inventories"} or document.get(
        "schema_version"
    ) != 2:
        raise ValueError("browser-analysis assertion document fields drifted")
    rows = document.get("inventories")
    if not isinstance(rows, Mapping):
        raise ValueError("browser-analysis assertion document lacks inventories")
    requested = []
    batch_rows = []
    saved: dict[str, dict[str, Any]] = {}
    for discovery_id, raw in sorted(rows.items()):
        if not isinstance(discovery_id, str) or not isinstance(raw, Mapping):
            raise ValueError("browser-analysis inventory is invalid")
        path = _string(raw, "path", discovery_id)
        name = _string(raw, "test_name", discovery_id)
        requested.append(
            {"discovery_id": discovery_id, "path": path, "name": name}
        )
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
        raise ValueError(
            "saved browser-analysis assertion inventory is not canonical"
        )
    return canonical


def _behavior_indexes(root: Path) -> dict[str, int]:
    matches = list(
        (root / "verification/invariants").glob(f"**/{INVARIANT}.toml")
    )
    if len(matches) != 1:
        raise ValueError(f"expected one {INVARIANT} invariant")
    record = tomllib.loads(matches[0].read_text())
    behaviors = record.get("behavior")
    if not isinstance(behaviors, list):
        raise ValueError(f"{INVARIANT} lacks behaviors")
    result = {}
    for index, behavior in enumerate(behaviors):
        partition = behavior.get("partition")
        label = (
            partition.get("equals")
            if isinstance(partition, Mapping)
            else None
        )
        if isinstance(label, str) and label:
            if label in result:
                raise ValueError(f"duplicate behavior selector {label}")
            result[label] = index
    expected = set(TEST_BINDINGS.values()) | {MALFORMED_JSON}
    missing = expected - set(result)
    if missing:
        raise ValueError(
            f"missing browser-analysis behavior selectors: {sorted(missing)}"
        )
    return result


def _review_lock_document(inventories: Mapping[str, Any]) -> dict[str, Any]:
    tests = []
    for discovery_id, raw in sorted(inventories.items()):
        path = _string(raw, "path", discovery_id)
        name = _string(raw, "test_name", discovery_id)
        assertions = raw.get("assertions")
        if not isinstance(assertions, list):
            raise ValueError(f"{path}:{name} lacks assertions")
        locked_assertions = []
        for assertion in assertions:
            if not isinstance(assertion, Mapping):
                raise ValueError(f"{path}:{name} assertion is invalid")
            kind = _string(assertion, "kind", name)
            expression = _string(assertion, "expression", name)
            locked_assertions.append(
                {
                    "kind": kind,
                    "function": _string(assertion, "function", name),
                    "expression": expression,
                    "disposition": _disposition(kind),
                    "invariant_id": INVARIANT,
                    "behavior_label": _behavior_for_assertion(
                        name=name,
                        default=TEST_BINDINGS[(path, name)],
                        kind=kind,
                        expression=expression,
                    ),
                }
            )
        tests.append(
            {"path": path, "name": name, "assertions": locked_assertions}
        )
    return {
        "schema_version": 1,
        "id": "WASM_ANALYSIS_FUNCTIONAL_15_SEMANTIC_LOCK",
        "tests": tests,
    }


def _validate_review_lock(
    root: Path,
    inventories: Mapping[str, Any],
    review_lock_path: Path,
) -> None:
    path = (
        review_lock_path
        if review_lock_path.is_absolute()
        else root / review_lock_path
    )
    try:
        actual = json.loads(path.read_text())
    except FileNotFoundError as error:
        raise ValueError(
            "reviewed browser-analysis semantic lock is missing: "
            f"{review_lock_path}"
        ) from error
    if actual != _review_lock_document(inventories):
        raise ValueError(
            "browser-analysis reviewed expressions, dispositions, or "
            "bindings drifted from semantic lock"
        )


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
    parser.add_argument("--selected-inventory-out", type=Path, required=True)
    parser.add_argument("--assignments-out", type=Path, required=True)
    parser.add_argument("--review-lock", type=Path, default=REVIEW_LOCK_PATH)
    parser.add_argument("--emit-review-lock", action="store_true")
    parser.add_argument("--execution-dir", type=Path)
    parser.add_argument("--plans-out", type=Path)
    parser.add_argument("--reviewed-date")
    parser.add_argument("--timestamp")
    args = parser.parse_args()
    root = args.root.resolve()
    documents = [
        json.loads(_rooted(root, path).read_text()) for path in args.inventory
    ]
    selected = select_live_inventories(
        root=root,
        inventory_documents=documents,
    )
    if args.emit_review_lock:
        _write_json(
            _rooted(root, args.review_lock),
            _review_lock_document(_canonical_inventories(root, selected)),
        )
    assignments = generate_assignments(
        root=root,
        inventory_document=selected,
        review_lock_path=args.review_lock,
    )
    _write_json(_rooted(root, args.selected_inventory_out), selected)
    _write_json(_rooted(root, args.assignments_out), assignments)
    summary = assignments["summary"]
    print(
        "browser-analysis assignments generated: "
        f"{summary['tests']} tests, {summary['assertions']} assertions, "
        f"{summary['direct_oracle']} direct oracles, "
        f"{summary['setup_or_precondition']} setup"
    )
    plan_arguments = (
        args.execution_dir,
        args.plans_out,
        args.reviewed_date,
        args.timestamp,
    )
    if any(value is not None for value in plan_arguments):
        if not all(value is not None for value in plan_arguments):
            raise ValueError(
                "browser-analysis plan generation arguments must be complete"
            )
        index = generate_path_plans(
            root=root,
            inventory_document=selected,
            assignments_document=assignments,
            execution_dir=_rooted(root, args.execution_dir),
            output_dir=_rooted(root, args.plans_out),
            reviewed_date=args.reviewed_date,
            timestamp=args.timestamp,
            target_profile="wasm-analysis-remaining",
        )
        print(
            "browser-analysis catalog plans generated: "
            f"{index['expected_count']} tests across "
            f"{len(index['paths'])} paths"
        )
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
