"""Generate reviewed assignments for syntax-kind classifier tests."""

from __future__ import annotations

import argparse
import json
import tomllib
from collections import Counter
from collections.abc import Mapping
from pathlib import Path
from typing import Any

from scripts.verification.mapping_assertions import validate_assertion_batch
from scripts.verification.runtime_bytecode_assignments import (
    generate_path_plans,
)


PATH = "crates/trust-syntax/src/syntax/mod.rs"
INVARIANT = "TRUST_SYNTAX_KIND_CLASSIFICATION_001"
REVIEW_LOCK_PATH = Path(
    "verification/p18/review-locks/syntax-kind-5-semantic-lock.json"
)

TOKEN_CONVERSION = "TOKEN_TO_SYNTAX_KIND_REPRESENTATIVES"
TRIVIA = "TRIVIA_REPRESENTATIVES"
TOKEN_NODE = "TOKEN_NODE_REPRESENTATIVES"
GENERIC_EXPRESSION = "GENERIC_EXPRESSION_EXCLUDES_AGGREGATE_INITIALIZER"
AGGREGATE_INITIALIZER = "AGGREGATE_INITIALIZER_CLASSIFICATION"
INITIALIZER_RHS = "INITIALIZER_RHS_UNION"
TRIVIA_EXCLUSION = "TRIVIA_EXCLUDED_FROM_SEMANTIC_NODE_CLASSIFIERS"
POU_SEQUENCE = "POU_LIKE_SEMANTIC_OWNER_SEQUENCE"

TOKEN_TEST = "test_token_kind_to_syntax_kind"
TRIVIA_TEST = "test_is_trivia"
TOKEN_NODE_TEST = "test_is_token_vs_node"
INITIALIZER_TEST = "test_initializer_classifier_sets"
POU_TEST = "test_pou_declaration_classifier_set"

GLOBAL_EXCLUSION = (
    "These representative tests do not prove exhaustive token conversion or "
    "every non-trivia and non-node SyntaxKind."
)

TEST_EXCLUSIONS: Mapping[str, tuple[str, ...]] = {
    TOKEN_TEST: (
        "The test covers one keyword, one identifier, and one operator token; "
        "it is not an exhaustive TokenKind conversion table.",
        GLOBAL_EXCLUSION,
    ),
    TRIVIA_TEST: (
        "The test covers whitespace, line comment, block comment, pragma, and "
        "one identifier negative representative only.",
        GLOBAL_EXCLUSION,
    ),
    TOKEN_NODE_TEST: (
        "The test covers identifier, IF keyword/statement, and function-block "
        "representatives, not every token and node kind.",
        GLOBAL_EXCLUSION,
    ),
    INITIALIZER_TEST: (
        "The test covers selected literal, name, call, initializer, and pragma "
        "classifiers; it does not prove grammar acceptance or every expression.",
        GLOBAL_EXCLUSION,
    ),
    POU_TEST: (
        "The constant slice order is intentionally the canonical product "
        "sequence; it does not define IEC declaration order.",
        "The misleading assertion message 'should be a POU' is test wording "
        "only and must not become IEC authority.",
        GLOBAL_EXCLUSION,
    ),
}

Route = tuple[str, str, str]


def _reviewed_routes() -> dict[str, dict[str, Route]]:
    behavior_ids: Mapping[str, Mapping[str, tuple[str, ...]]] = {
        TOKEN_TEST: {
            TOKEN_CONVERSION: (
                "ASSERT_70D01B3A64AA8EEBFC012EC0",
                "ASSERT_C05917D6C4C5AFF519C66823",
                "ASSERT_7EA315B79CA28320893E30C1",
            )
        },
        TRIVIA_TEST: {
            TRIVIA: (
                "ASSERT_08F8FDFED4EE4737AC14BE59",
                "ASSERT_F9E78F8C3CFCA082F1F7338C",
                "ASSERT_32C49C3B8494AD9C80F95002",
                "ASSERT_8D9905536095084A68283DBC",
                "ASSERT_8A92AF99B596E72E27DE1CF4",
            )
        },
        TOKEN_NODE_TEST: {
            TOKEN_NODE: (
                "ASSERT_23AC7FF20B3A1BC3792B030F",
                "ASSERT_6A20681A495A4D5A4FAB6C77",
                "ASSERT_CE5AF0EF6A46D5819E0F736E",
                "ASSERT_C78907FC2D96D939CC6A6F05",
                "ASSERT_356D6A72309442655039C73A",
                "ASSERT_88A683D5FA6AC134C27F5A40",
            )
        },
        INITIALIZER_TEST: {
            GENERIC_EXPRESSION: (
                "ASSERT_5970538B9F146D61EF2A07CC",
                "ASSERT_1B5FD870E9EF770CFE192A9C",
                "ASSERT_53DFE1192B2D303D781BBC6E",
                "ASSERT_34A5ADCF27CC59E30C2AB46F",
                "ASSERT_4790D14C6709CAA89C9253F3",
            ),
            AGGREGATE_INITIALIZER: (
                "ASSERT_FAF2643386F7290E12839939",
                "ASSERT_1835ACCC73F88F7B30001CA0",
            ),
            INITIALIZER_RHS: (
                "ASSERT_8A2DA29C192798ECA7615A08",
                "ASSERT_747FDCD77D4AAA824C9530A7",
                "ASSERT_8E5D1FCFDFC9706A1E964A7A",
            ),
            TRIVIA_EXCLUSION: (
                "ASSERT_E005E6E91482CD39C145DB84",
                "ASSERT_49E0CAB46192596EF6D39A26",
                "ASSERT_FEC0B92C34ADACF21E349356",
            ),
        },
        POU_TEST: {
            POU_SEQUENCE: (
                "ASSERT_773426496565CCC768C5715B",
                "ASSERT_96C4623ECE39FAB7CC03C2CB",
                "ASSERT_B806745E5F30396EF5B8C80B",
            )
        },
    }
    result: dict[str, dict[str, Route]] = {}
    for test_name, behaviors in behavior_ids.items():
        routes = result.setdefault(test_name, {})
        for behavior, assertion_ids in behaviors.items():
            for assertion_id in assertion_ids:
                if assertion_id in routes:
                    raise ValueError(
                        f"duplicate reviewed assertion {assertion_id}"
                    )
                routes[assertion_id] = (
                    INVARIANT,
                    behavior,
                    "oracle_bound",
                )
    return result


REVIEWED_ROUTES: Mapping[str, Mapping[str, Route]] = _reviewed_routes()


def select_live_inventories(
    *,
    root: Path,
    inventory_document: Mapping[str, Any],
) -> dict[str, Any]:
    inventories = _canonical_inventories(root, inventory_document)
    denominator = tomllib.loads(
        (root / "verification/test-catalog-denominator.toml").read_text()
    )
    expected = {
        str(row["discovery_id"]): str(row["name"])
        for row in denominator.get("reviews", [])
        if isinstance(row, Mapping)
        and row.get("path") == PATH
        and row.get("disposition") == "reviewed_nonmapping"
        and row.get("reason_code")
        == "no_reviewed_spec_or_invariant_binding"
        and str(row.get("name")) in REVIEWED_ROUTES
    }
    if set(expected.values()) != set(REVIEWED_ROUTES):
        raise ValueError("live syntax-kind denominator differs from review")
    selected = {
        discovery_id: inventories[discovery_id]
        for discovery_id in sorted(expected)
        if discovery_id in inventories
    }
    if set(selected) != set(expected):
        raise ValueError("saved inventory lacks syntax-kind mapping debt")
    for discovery_id, raw in selected.items():
        if (raw.get("path"), raw.get("test_name")) != (
            PATH,
            expected[discovery_id],
        ):
            raise ValueError(f"{discovery_id} syntax-kind identity drifted")
    return {"schema_version": 2, "inventories": selected}


def generate_assignments(
    *,
    root: Path,
    inventory_document: Mapping[str, Any],
    review_lock_path: Path = REVIEW_LOCK_PATH,
) -> dict[str, Any]:
    inventories = _canonical_inventories(root, inventory_document)
    _validate_review_lock(root, inventories, review_lock_path)
    return _generate_assignment_document(inventories, _behavior_indexes(root))


def _generate_assignment_document(
    inventories: Mapping[str, Any],
    behavior_indexes: Mapping[str, int],
) -> dict[str, Any]:
    rows = []
    seen: set[str] = set()
    dispositions: Counter[str] = Counter()
    behavior_counts: Counter[str] = Counter()
    for discovery_id, raw in sorted(inventories.items()):
        path = _string(raw, "path", discovery_id)
        name = _string(raw, "test_name", discovery_id)
        if path != PATH or name not in REVIEWED_ROUTES or name in seen:
            raise ValueError(
                f"unexpected or duplicate syntax-kind test {path}:{name}"
            )
        seen.add(name)
        if raw.get("unresolved_assertion_helpers") != []:
            raise ValueError(f"{path}:{name} has unresolved assertion helpers")
        assertions = raw.get("assertions")
        if not isinstance(assertions, list) or not assertions:
            raise ValueError(f"{path}:{name} lacks assertions")
        assignments = []
        for assertion in assertions:
            if not isinstance(assertion, Mapping):
                raise ValueError(f"{path}:{name} assertion is invalid")
            assertion_id = _string(assertion, "assertion_id", name)
            invariant, behavior, disposition = _route(
                test_name=name,
                assertion_id=assertion_id,
            )
            assignments.append(
                {
                    "assertion_id": assertion_id,
                    "invariant_id": invariant,
                    "behavior_index": behavior_indexes[behavior],
                }
            )
            dispositions[disposition] += 1
            behavior_counts[behavior] += 1
        rows.append(
            {
                "path": path,
                "name": name,
                "assertion_assignments": assignments,
                "setup_assertion_ids": [],
                "non_oracle_assertion_ids": [],
            }
        )
    if seen != set(REVIEWED_ROUTES):
        raise ValueError("syntax-kind assignments are not exhaustive")
    if sum(dispositions.values()) != 30:
        raise ValueError("syntax-kind assertion count drifted from reviewed 30")
    return {
        "schema_version": 3,
        "basis": "reviewed_syntax_kind_5_semantic_lock",
        "summary": {
            "tests": len(rows),
            "assertions": sum(dispositions.values()),
            "direct_oracle": dispositions["oracle_bound"],
            "setup_or_precondition": 0,
            "non_oracle_observation": 0,
            "behavior_assertions": dict(sorted(behavior_counts.items())),
        },
        "assignments": rows,
    }


def _route(*, test_name: str, assertion_id: str) -> Route:
    routes = REVIEWED_ROUTES.get(test_name)
    if routes is None:
        raise ValueError(f"unreviewed syntax-kind test {test_name!r}")
    route = routes.get(assertion_id)
    if route is None:
        raise ValueError(
            f"unreviewed syntax-kind assertion {test_name}:{assertion_id}"
        )
    return route


def _canonical_inventories(
    root: Path,
    document: Mapping[str, Any],
) -> dict[str, dict[str, Any]]:
    if set(document) != {"schema_version", "inventories"} or document.get(
        "schema_version"
    ) != 2:
        raise ValueError("syntax-kind assertion document fields drifted")
    raw_rows = document.get("inventories")
    if not isinstance(raw_rows, Mapping):
        raise ValueError("syntax-kind document lacks inventories")
    requested = []
    batch_rows = []
    saved: dict[str, dict[str, Any]] = {}
    for discovery_id, raw in sorted(raw_rows.items()):
        if not isinstance(discovery_id, str) or not isinstance(raw, Mapping):
            raise ValueError("syntax-kind inventory is invalid")
        path = _string(raw, "path", discovery_id)
        name = _string(raw, "test_name", discovery_id)
        requested.append(
            {"discovery_id": discovery_id, "path": path, "name": name}
        )
        batch_rows.append(
            {
                key: value
                for key, value in raw.items()
                if key != "inventory_digest"
            }
        )
        saved[discovery_id] = dict(raw)
    canonical = validate_assertion_batch(
        root,
        {"schema_version": 2, "inventories": batch_rows},
        requested,
    )
    if canonical != saved:
        raise ValueError(
            "saved syntax-kind assertion inventory is not canonical"
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
            result[label] = index
    required = {
        route[1]
        for routes in REVIEWED_ROUTES.values()
        for route in routes.values()
    }
    if not required <= set(result):
        raise ValueError("syntax-kind invariant behaviors drifted")
    return result


def _review_lock_document(
    inventories: Mapping[str, Any],
) -> dict[str, Any]:
    tests = []
    for discovery_id, raw in sorted(inventories.items()):
        path = _string(raw, "path", discovery_id)
        name = _string(raw, "test_name", discovery_id)
        if path != PATH or name not in REVIEWED_ROUTES:
            raise ValueError(f"unexpected syntax-kind test {path}:{name}")
        assertions = raw.get("assertions")
        if not isinstance(assertions, list):
            raise ValueError(f"{path}:{name} lacks assertions")
        locked_assertions = []
        for assertion in assertions:
            if not isinstance(assertion, Mapping):
                raise ValueError(f"{path}:{name} assertion is invalid")
            assertion_id = _string(assertion, "assertion_id", name)
            invariant, behavior, disposition = _route(
                test_name=name,
                assertion_id=assertion_id,
            )
            locked_assertions.append(
                {
                    "assertion_id": assertion_id,
                    "kind": _string(assertion, "kind", assertion_id),
                    "function": _string(
                        assertion,
                        "function",
                        assertion_id,
                    ),
                    "expression": _string(
                        assertion,
                        "expression",
                        assertion_id,
                    ),
                    "disposition": disposition,
                    "invariant_id": invariant,
                    "behavior_label": behavior,
                }
            )
        tests.append(
            {
                "path": path,
                "name": name,
                "exclusions": list(TEST_EXCLUSIONS[name]),
                "assertions": locked_assertions,
            }
        )
    return {
        "schema_version": 1,
        "id": "SYNTAX_KIND_5_SEMANTIC_LOCK",
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
            f"reviewed syntax-kind semantic lock is missing: "
            f"{review_lock_path}"
        ) from error
    if actual != _review_lock_document(inventories):
        raise ValueError(
            "syntax-kind expressions or bindings drifted from semantic lock"
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
    parser.add_argument("--inventory", type=Path, required=True)
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
    document = json.loads(_rooted(root, args.inventory).read_text())
    selected = select_live_inventories(
        root=root,
        inventory_document=document,
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
        "syntax-kind assignments generated: "
        f"{summary['tests']} tests, {summary['direct_oracle']} direct"
    )
    plan_arguments = (
        args.execution_dir,
        args.plans_out,
        args.reviewed_date,
        args.timestamp,
    )
    if any(value is not None for value in plan_arguments):
        if not all(value is not None for value in plan_arguments):
            raise ValueError("syntax-kind plan arguments must be complete")
        index = generate_path_plans(
            root=root,
            inventory_document=selected,
            assignments_document=assignments,
            execution_dir=_rooted(root, args.execution_dir),
            output_dir=_rooted(root, args.plans_out),
            reviewed_date=args.reviewed_date,
            timestamp=args.timestamp,
            target_profile="syntax-remaining",
        )
        print(
            f"syntax-kind catalog plans generated: "
            f"{index['expected_count']} tests"
        )
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
