"""Generate reviewed semantic assignments for the expr_calls batch."""

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


CALL_BINDING = "VM_CALL_BINDING_001"
IEC_CALL_BINDING = "IEC_HIR_CALL_BINDING_001"
ENCODER_SEAM = "VM_SEAM_ENC_001"
INVENTORY_PATH = Path(
    "target/gate-artifacts/verification/expr-actionable-assertions.json"
)
REVIEW_LOCK_PATH = Path(
    "verification/p18/review-locks/expr-calls-6-semantic-lock.json"
)

TestKey = tuple[str, str]
BehaviorSelector = tuple[str, str]

_EXPR_CALLS_PATH = "crates/trust-runtime/tests/expr_calls.rs"
REVIEWED_IDENTITIES: Mapping[str, TestKey] = {
    "DISC_D125E8A7C82805AEE276": (_EXPR_CALLS_PATH, "function_call_expr"),
    "DISC_4670B9A0570719575EB2": (
        _EXPR_CALLS_PATH,
        "function_call_named_args",
    ),
    "DISC_3FEF14488929BE4E886D": (
        _EXPR_CALLS_PATH,
        "function_call_output_positional",
    ),
    "DISC_C61007734167A2A4CE7E": (_EXPR_CALLS_PATH, "function_block_call"),
    "DISC_2D17DC9BA441E47099C3": (
        _EXPR_CALLS_PATH,
        "function_in_out_with_conversion_expression_regression_issue_13",
    ),
    "DISC_9754A8A2443EE9A1BED9": (
        _EXPR_CALLS_PATH,
        "function_in_out_without_conversion_expression_baseline",
    ),
}
TEST_BINDINGS: Mapping[TestKey, BehaviorSelector] = {
    (_EXPR_CALLS_PATH, "function_call_expr"): (
        CALL_BINDING,
        "DECLARED_POSITIONAL_AND_NAMED_BINDING",
    ),
    (_EXPR_CALLS_PATH, "function_call_named_args"): (
        CALL_BINDING,
        "DECLARED_POSITIONAL_AND_NAMED_BINDING",
    ),
    (_EXPR_CALLS_PATH, "function_call_output_positional"): (
        IEC_CALL_BINDING,
        "NONFORMAL_OUTPUT_POSITIONAL",
    ),
    (_EXPR_CALLS_PATH, "function_block_call"): (
        IEC_CALL_BINDING,
        "FUNCTION_BLOCK_INSTANCE_FORMAL_CALL",
    ),
    (
        _EXPR_CALLS_PATH,
        "function_in_out_with_conversion_expression_regression_issue_13",
    ): (ENCODER_SEAM, "EXPLICIT_VOID_FUNCTION_RETURN_TYPE"),
    (
        _EXPR_CALLS_PATH,
        "function_in_out_without_conversion_expression_baseline",
    ): (ENCODER_SEAM, "EXPLICIT_VOID_FUNCTION_RETURN_TYPE"),
}
REVIEWED_ASSERTION_COUNTS: Mapping[TestKey, int] = {
    key: 2 for key in TEST_BINDINGS
}
REVIEWED_KIND_COUNTS = Counter(
    {
        "method:unwrap": 4,
        "method:assert_eq": 4,
        "method:expect": 2,
        "macro:assert": 2,
    }
)


def generate_assignments(
    *,
    root: Path,
    inventory_document: Mapping[str, Any],
    review_lock_path: Path = REVIEW_LOCK_PATH,
) -> dict[str, Any]:
    """Validate the reviewed input and project its exact semantic bindings."""

    inventories = _canonical_inventories(root, inventory_document)
    _validate_review_lock(root, inventories, review_lock_path)
    behavior_indexes = _behavior_indexes(root)
    rows = []
    observed_kinds: Counter[str] = Counter()
    dispositions: Counter[str] = Counter()
    seen: set[TestKey] = set()

    for discovery_id, raw in sorted(inventories.items()):
        path = _string(raw, "path", discovery_id)
        name = _string(raw, "test_name", discovery_id)
        key = (path, name)
        if key not in TEST_BINDINGS or key in seen:
            raise ValueError(f"unexpected or duplicate expr_calls test {path}:{name}")
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

        invariant_id, behavior_label = TEST_BINDINGS[key]
        behavior_index = behavior_indexes[(invariant_id, behavior_label)]
        assertion_rows = []
        setup_ids = []
        for assertion in assertions:
            if not isinstance(assertion, Mapping):
                raise ValueError(f"{path}:{name} assertion is invalid")
            assertion_id = _string(assertion, "assertion_id", name)
            kind = _string(assertion, "kind", assertion_id)
            disposition = _disposition(kind)
            observed_kinds[kind] += 1
            dispositions[disposition] += 1
            assertion_rows.append(
                {
                    "assertion_id": assertion_id,
                    "invariant_id": invariant_id,
                    "behavior_index": behavior_index,
                }
            )
            if disposition == "setup_or_precondition":
                setup_ids.append(assertion_id)
        if len(setup_ids) != 1 or len(assertion_rows) - len(setup_ids) != 1:
            raise ValueError(
                f"{path}:{name} must retain one setup and one direct oracle"
            )
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
        raise ValueError("expr_calls assignments are not exhaustive")
    if observed_kinds != REVIEWED_KIND_COUNTS:
        raise ValueError("expr_calls reviewed assertion kind profile drifted")
    return {
        "schema_version": 3,
        "basis": "reviewed_expr_calls_semantic_test_and_assertion_rules",
        "summary": {
            "tests": len(rows),
            "assertions": sum(dispositions.values()),
            "direct_oracle": dispositions["oracle_bound"],
            "setup_or_precondition": dispositions["setup_or_precondition"],
            "non_oracle_observation": 0,
            "assertion_kinds": dict(sorted(observed_kinds.items())),
        },
        "assignments": rows,
    }


def _disposition(kind: str) -> str:
    if kind in {"method:unwrap", "method:expect"}:
        return "setup_or_precondition"
    if kind in {"method:assert_eq", "macro:assert"}:
        return "oracle_bound"
    raise ValueError(f"unreviewed expr_calls assertion kind {kind!r}")


def _canonical_inventories(
    root: Path,
    document: Mapping[str, Any],
) -> dict[str, dict[str, Any]]:
    if set(document) != {"schema_version", "inventories"} or document.get(
        "schema_version"
    ) != 2:
        raise ValueError("expr_calls assertion document fields drifted")
    rows = document.get("inventories")
    if not isinstance(rows, Mapping):
        raise ValueError("expr_calls assertion document lacks inventories")
    if set(rows) != set(REVIEWED_IDENTITIES):
        raise ValueError("expr_calls input does not equal exact reviewed identities")

    requested = []
    batch_rows = []
    saved: dict[str, dict[str, Any]] = {}
    for discovery_id, raw in sorted(rows.items()):
        if not isinstance(discovery_id, str) or not isinstance(raw, Mapping):
            raise ValueError("expr_calls inventory is invalid")
        path = _string(raw, "path", discovery_id)
        name = _string(raw, "test_name", discovery_id)
        if (path, name) != REVIEWED_IDENTITIES[discovery_id]:
            raise ValueError(f"{discovery_id} expr_calls identity drifted")
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
        raise ValueError("saved expr_calls assertion inventory is not canonical")
    return canonical


def _behavior_indexes(root: Path) -> dict[BehaviorSelector, int]:
    result: dict[BehaviorSelector, int] = {}
    for invariant_id in sorted({binding[0] for binding in TEST_BINDINGS.values()}):
        matches = list(
            (root / "verification/invariants").glob(f"**/{invariant_id}.toml")
        )
        if len(matches) != 1:
            raise ValueError(f"expected one {invariant_id} invariant")
        record = tomllib.loads(matches[0].read_text())
        behaviors = record.get("behavior")
        if not isinstance(behaviors, list):
            raise ValueError(f"{invariant_id} lacks behaviors")
        for index, behavior in enumerate(behaviors):
            if not isinstance(behavior, Mapping):
                continue
            partition = behavior.get("partition")
            label = (
                partition.get("equals")
                if isinstance(partition, Mapping)
                else None
            )
            if isinstance(label, str) and label:
                selector = (invariant_id, label)
                if selector in result:
                    raise ValueError(
                        f"duplicate behavior selector {invariant_id}:{label}"
                    )
                result[selector] = index
    missing = set(TEST_BINDINGS.values()) - set(result)
    if missing:
        raise ValueError(
            f"missing expr_calls behavior selectors: {sorted(missing)}"
        )
    return result


def _review_lock_document(
    inventories: Mapping[str, Any],
) -> dict[str, Any]:
    tests = []
    for discovery_id, raw in sorted(inventories.items()):
        if not isinstance(raw, Mapping):
            raise ValueError(f"{discovery_id} expr_calls inventory is invalid")
        path = _string(raw, "path", discovery_id)
        name = _string(raw, "test_name", discovery_id)
        invariant_id, behavior_label = TEST_BINDINGS[(path, name)]
        assertions = raw.get("assertions")
        if not isinstance(assertions, list):
            raise ValueError(f"{path}:{name} lacks assertions")
        locked_assertions = []
        for assertion in assertions:
            if not isinstance(assertion, Mapping):
                raise ValueError(f"{path}:{name} assertion is invalid")
            kind = _string(assertion, "kind", name)
            locked_assertions.append(
                {
                    "assertion_id": _string(assertion, "assertion_id", name),
                    "expression_digest": _string(
                        assertion,
                        "expression_digest",
                        name,
                    ),
                    "kind": kind,
                    "function": _string(assertion, "function", name),
                    "expression": _string(assertion, "expression", name),
                    "disposition": _disposition(kind),
                    "invariant_id": invariant_id,
                    "behavior_label": behavior_label,
                }
            )
        tests.append(
            {
                "discovery_id": discovery_id,
                "path": path,
                "name": name,
                "source_digest": _string(raw, "source_digest", discovery_id),
                "inventory_digest": _string(
                    raw,
                    "inventory_digest",
                    discovery_id,
                ),
                "assertions": locked_assertions,
            }
        )
    return {
        "schema_version": 1,
        "id": "EXPR_CALLS_6_SEMANTIC_LOCK",
        "tests": tests,
    }


def _validate_review_lock(
    root: Path,
    inventories: Mapping[str, Any],
    review_lock_path: Path,
) -> None:
    path = _rooted(root, review_lock_path)
    try:
        actual = json.loads(path.read_text())
    except FileNotFoundError as error:
        raise ValueError(
            f"reviewed expr_calls semantic lock is missing: {review_lock_path}"
        ) from error
    if actual != _review_lock_document(inventories):
        raise ValueError(
            "expr_calls reviewed expressions, dispositions, or bindings "
            "drifted from semantic lock"
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
    parser.add_argument("--inventory", type=Path, default=INVENTORY_PATH)
    parser.add_argument(
        "--assignments-out",
        type=Path,
        default=Path(
            "target/gate-artifacts/verification/expr-calls-6-assignments.json"
        ),
    )
    parser.add_argument("--review-lock", type=Path, default=REVIEW_LOCK_PATH)
    parser.add_argument("--emit-review-lock", action="store_true")
    parser.add_argument("--execution-dir", type=Path)
    parser.add_argument("--plans-out", type=Path)
    parser.add_argument("--reviewed-date")
    parser.add_argument("--timestamp")
    args = parser.parse_args()
    root = args.root.resolve()
    document = json.loads(_rooted(root, args.inventory).read_text())
    canonical = _canonical_inventories(root, document)
    if args.emit_review_lock:
        _write_json(
            _rooted(root, args.review_lock),
            _review_lock_document(canonical),
        )
    assignments = generate_assignments(
        root=root,
        inventory_document=document,
        review_lock_path=args.review_lock,
    )
    _write_json(_rooted(root, args.assignments_out), assignments)
    summary = assignments["summary"]
    print(
        "expr_calls assignments generated: "
        f"tests={summary['tests']} assertions={summary['assertions']} "
        f"direct_oracle={summary['direct_oracle']} "
        f"setup={summary['setup_or_precondition']}"
    )

    plan_args = (
        args.execution_dir,
        args.plans_out,
        args.reviewed_date,
        args.timestamp,
    )
    if any(value is not None for value in plan_args):
        if not all(value is not None for value in plan_args):
            parser.error(
                "--execution-dir, --plans-out, --reviewed-date, and "
                "--timestamp must be provided together"
            )
        index = generate_path_plans(
            root=root,
            inventory_document=document,
            assignments_document=assignments,
            execution_dir=_rooted(root, args.execution_dir),
            output_dir=_rooted(root, args.plans_out),
            reviewed_date=args.reviewed_date,
            timestamp=args.timestamp,
        )
        print(f"expr_calls plans generated: paths={len(index['paths'])}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
