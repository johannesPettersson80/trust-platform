"""Generate reviewed assignments for the runtime hot-reload batch."""

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


INVARIANT = "RT_RELOAD_001"
PATH = "crates/trust-runtime/tests/hot_reload.rs"
REVIEW_LOCK_PATH = Path(
    "verification/p18/review-locks/hot-reload-6-semantic-lock.json"
)

COMPLETED_SCAN = "COMPLETED_SCAN_BOUNDARY_COMMIT"
WARM_STORAGE = "SUCCESSFUL_WARM_RELOAD_STORAGE_IMAGE"
IO_REBIND = "SUCCESSFUL_IO_REBIND"
ENTRYPOINT = "ENTRYPOINT_AND_SCAN_COUNTER_RESTART"
RETAIN_REJECTION = "RETAIN_MIGRATION_REJECTION_PRESERVES_LIVE_RUNTIME"
INVALID_BYTECODE = "INVALID_BYTECODE_TYPED_REJECTION"

TEST_BINDINGS: Mapping[str, str] = {
    "hot_reload_cycle_boundary_contract_holds_for_vm_reload": COMPLETED_SCAN,
    "hot_reload_migrates_retain_and_resets_nonretain_and_instances": (
        WARM_STORAGE
    ),
    "hot_reload_rebinds_instance_backed_io_after_warm_restart": IO_REBIND,
    "hot_reload_changed_body_restarts_at_entrypoint_policy": ENTRYPOINT,
    "hot_reload_retain_migration_failure_preserves_live_runtime": (
        RETAIN_REJECTION
    ),
    "hot_reload_invalid_module_reports_deterministic_error": INVALID_BYTECODE,
}

GLOBAL_EXCLUSION = (
    "No test in this batch proves DAP stReload, paused frames or breakpoints, "
    "source compilation, file watching, metadata publication, concurrency "
    "stress, or external interoperability."
)

TEST_EXCLUSIONS: Mapping[str, tuple[str, ...]] = {
    "hot_reload_cycle_boundary_contract_holds_for_vm_reload": (
        "The 40 ms delay is orchestration evidence, not a latency SLA.",
        "The test does not prove that the changed +10 program body executes.",
        GLOBAL_EXCLUSION,
    ),
    "hot_reload_migrates_retain_and_resets_nonretain_and_instances": (
        "The test covers identical-schema reload only; it does not prove "
        "type or shape migration.",
        "The test does not prove reload events, durable-file state, logical "
        "time, orphan cleanup, or structural migration.",
        GLOBAL_EXCLUSION,
    ),
    "hot_reload_rebinds_instance_backed_io_after_warm_restart": (
        "The test covers one BOOL output at %QX0.0, not inputs, multiple "
        "bindings, physical drivers, or external hardware.",
        "The test does not independently reassert the post-reload address "
        "beyond the single-binding value evidence.",
        GLOBAL_EXCLUSION,
    ),
    "hot_reload_changed_body_restarts_at_entrypoint_policy": (
        "The test covers one VM program and one non-retained scalar, not task "
        "schedules, retained state, I/O, debug mutation, or multiple resources.",
        GLOBAL_EXCLUSION,
    ),
    "hot_reload_retain_migration_failure_preserves_live_runtime": (
        "The test covers one custom-store out-of-range INT and proves only "
        "executable and cycle-counter preservation.",
        "The test does not prove process images, bindings, forces, logical "
        "time, status, tasks, or the durable-file codec.",
        GLOBAL_EXCLUSION,
    ),
    "hot_reload_invalid_module_reports_deterministic_error": (
        "The test does not prove old-runtime preservation, a stable message, "
        "repeat determinism, validator completeness, or resource errors.",
        GLOBAL_EXCLUSION,
    ),
}

# These assertion identities are the reviewed semantic decision. Anything not
# listed here fails closed instead of being inferred from names or syntax.
ORACLE_ASSERTION_IDS: Mapping[str, frozenset[str]] = {
    "hot_reload_retain_migration_failure_preserves_live_runtime": frozenset(
        {
            "ASSERT_DA35F7BFC83C68EA2C3E2F8E",
            "ASSERT_0EFDD57986AFB66DED032186",
            "ASSERT_625CAED2005DDF4B20E850CB",
            "ASSERT_C59349633E7221CB8D457BB5",
            "ASSERT_FFBBEB0E73C46FBC503F8C7C",
            "ASSERT_71E5047A08D10FD15889EC69",
            "ASSERT_4EB172F018AB80FC2763D170",
            "ASSERT_B9EB3308EAA9867D461056DD",
            "ASSERT_CD7D224AD732FD90380E16E9",
            "ASSERT_96E199D09BC74D3224134FC8",
        }
    ),
    "hot_reload_rebinds_instance_backed_io_after_warm_restart": frozenset(
        {
            "ASSERT_C0743260EB26ABBB644CD24D",
            "ASSERT_3AF8449732D4A3FFFC1D4F9C",
            "ASSERT_86EBA383A108A2A40B4C6254",
            "ASSERT_8798F73EA3FAC50FC6D25D49",
            "ASSERT_9B1A60980279553941869B73",
            "ASSERT_1ADDE8185D955818F0A66179",
            "ASSERT_FDACF60866AF14B23F367251",
            "ASSERT_A19BD4CE43DA586C148C82D0",
            "ASSERT_56EA08AACBDD6A2112237B7D",
        }
    ),
    "hot_reload_invalid_module_reports_deterministic_error": frozenset(
        {
            "ASSERT_B7FDA156150519BA2E86D37E",
            "ASSERT_D53D4FDC4594E8038AE6CF35",
        }
    ),
    "hot_reload_cycle_boundary_contract_holds_for_vm_reload": frozenset(
        {
            "ASSERT_2CAD803FBD1355B18C5C8804",
            "ASSERT_D80C3DCCB39EC014821DF903",
            "ASSERT_AC225DA913E3C3E1725BFB6F",
        }
    ),
    "hot_reload_migrates_retain_and_resets_nonretain_and_instances": (
        frozenset(
            {
                "ASSERT_6D6AD47B0598D199D5061E46",
                "ASSERT_A69BAD735458F6A87F2D84A0",
                "ASSERT_7D07BAF40823683F1EF57BAC",
                "ASSERT_70233D5D15AF26A9092B494B",
                "ASSERT_7F40780CBD84A701E1339455",
                "ASSERT_8D3117B55B70B1264F376734",
            }
        )
    ),
    "hot_reload_changed_body_restarts_at_entrypoint_policy": frozenset(
        {
            "ASSERT_DE39D90ECF794B32EA1D7C68",
            "ASSERT_C8488926885D1DBEA59B63D7",
            "ASSERT_A9A89D2FEF28361E72A39D8B",
            "ASSERT_47093AF60C9E02FF1CBCDE5F",
            "ASSERT_8B2426A66E797D1D1585BFF3",
            "ASSERT_F8440FCAC7D33C8E9CB3A284",
            "ASSERT_82468D144D401DE923B06AF4",
        }
    ),
}

SETUP_ASSERTION_IDS: Mapping[str, frozenset[str]] = {
    "hot_reload_retain_migration_failure_preserves_live_runtime": frozenset(
        {
            "ASSERT_C9C895ECDAAD585A38AB88BB",
            "ASSERT_0D41661CCC2DDEB39E172615",
            "ASSERT_874365149FFDDC80703CC78F",
            "ASSERT_5DBA4BABDA005CC827296C22",
            "ASSERT_C696B5E0091FD2648728D86C",
            "ASSERT_D100462FA338C7FC18186AB5",
        }
    ),
    "hot_reload_rebinds_instance_backed_io_after_warm_restart": frozenset(
        {
            "ASSERT_40A09EE56EB564AE0305D228",
            "ASSERT_4CC073840094EE8C839AB12F",
            "ASSERT_19C14C682C512D98C234DC3F",
            "ASSERT_14A89AB015F59BC3F190640B",
            "ASSERT_08D67D34FA5B8C6CB7E9B11D",
            "ASSERT_17D66A33855D12863E211334",
            "ASSERT_13C741244B419A271452D909",
        }
    ),
    "hot_reload_invalid_module_reports_deterministic_error": frozenset(
        {
            "ASSERT_1FD132444DC00518C1EBF19A",
            "ASSERT_B1E996E1CD369537A7B07A2E",
            "ASSERT_E30E4E8D88E7B4F5B5A0BA82",
            "ASSERT_9F7A6B0611C8428D46D0A436",
            "ASSERT_DFAF9C1ABF626C08120A4F9E",
            "ASSERT_7AA3363867FD6644CCA6599B",
            "ASSERT_20A30E90E932F3C6869B0CC6",
            "ASSERT_D33459274B873A44AB7776C0",
            "ASSERT_07B8E8892A3648CE5336B5F9",
        }
    ),
    "hot_reload_cycle_boundary_contract_holds_for_vm_reload": frozenset(
        {
            "ASSERT_2BAC4ABE19657A51322A84D6",
            "ASSERT_1A0A58CC1651C9FDE7A2DAB5",
            "ASSERT_7D8AD7C25D661E296C5DCB34",
            "ASSERT_9F98D2A4393AD7CB7AA4136D",
            "ASSERT_B86EE5DE23BF76D092AB5ED1",
            "ASSERT_E9F7A8DB71CEBEAABF286AD1",
            "ASSERT_6CEC4F0BB6206FBD9E3BF656",
            "ASSERT_7098508A74F24A8700AF3CF0",
            "ASSERT_D73D6179EF1E94D48A25E24F",
            "ASSERT_5A388C7498F82F38129F743E",
            "ASSERT_2A75CA2D571CF01E7D9BE9FE",
            "ASSERT_388AFAE3D783BD78CBA2F5E6",
        }
    ),
    "hot_reload_migrates_retain_and_resets_nonretain_and_instances": (
        frozenset(
            {
                "ASSERT_57AF67FAE9D9B62187C10385",
                "ASSERT_680E2FB71C0EB1D183B438B9",
                "ASSERT_3E91B68084B04287B7EA1542",
                "ASSERT_3840C0BC38B95BD4B5C01860",
                "ASSERT_E83FDD839C63C5C5D60EB015",
                "ASSERT_ADDEECECE78C3DE07C1C44B5",
                "ASSERT_6438F08DCBB2446478D2A6F5",
            }
        )
    ),
    "hot_reload_changed_body_restarts_at_entrypoint_policy": frozenset(
        {
            "ASSERT_73BC7D08944A86D66613F072",
            "ASSERT_811A52580240E7CD065750A2",
            "ASSERT_047EBC126CDCBEB29BC949A5",
            "ASSERT_FE19D95F631B3CFC674F9898",
            "ASSERT_34DBADBCD4AEEFF43BC6A170",
            "ASSERT_646807BF5F669EC97E93765A",
        }
    ),
}


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
        and str(row.get("name")) in TEST_BINDINGS
    }
    if set(expected.values()) != set(TEST_BINDINGS):
        raise ValueError("live hot-reload denominator differs from review")
    selected = {
        discovery_id: inventories[discovery_id]
        for discovery_id in sorted(expected)
        if discovery_id in inventories
    }
    if set(selected) != set(expected):
        raise ValueError("saved inventory lacks hot-reload mapping debt")
    for discovery_id, raw in selected.items():
        if (raw.get("path"), raw.get("test_name")) != (
            PATH,
            expected[discovery_id],
        ):
            raise ValueError(f"{discovery_id} hot-reload identity drifted")
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
        if path != PATH or name not in TEST_BINDINGS or name in seen:
            raise ValueError(f"unexpected or duplicate hot-reload test {path}:{name}")
        seen.add(name)
        if raw.get("unresolved_assertion_helpers") != []:
            raise ValueError(f"{path}:{name} has unresolved assertion helpers")
        assertions = raw.get("assertions")
        if not isinstance(assertions, list) or not assertions:
            raise ValueError(f"{path}:{name} lacks assertions")
        assignments = []
        setup_ids = []
        for assertion in assertions:
            if not isinstance(assertion, Mapping):
                raise ValueError(f"{path}:{name} assertion is invalid")
            assertion_id = _string(assertion, "assertion_id", name)
            disposition = _disposition(
                test_name=name,
                assertion_id=assertion_id,
            )
            if disposition == "setup_or_precondition":
                setup_ids.append(assertion_id)
            behavior = TEST_BINDINGS[name]
            assignments.append(
                {
                    "assertion_id": assertion_id,
                    "invariant_id": INVARIANT,
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
                "setup_assertion_ids": setup_ids,
                "non_oracle_assertion_ids": [],
            }
        )
    if seen != set(TEST_BINDINGS):
        raise ValueError("hot-reload assignments are not exhaustive")
    if sum(dispositions.values()) != 84:
        raise ValueError("hot-reload assertion count drifted from reviewed 84")
    return {
        "schema_version": 3,
        "basis": "reviewed_hot_reload_6_semantic_lock",
        "summary": {
            "tests": len(rows),
            "assertions": sum(dispositions.values()),
            "direct_oracle": dispositions["oracle_bound"],
            "setup_or_precondition": dispositions[
                "setup_or_precondition"
            ],
            "non_oracle_observation": 0,
            "behavior_assertions": dict(sorted(behavior_counts.items())),
        },
        "assignments": rows,
    }


def _disposition(*, test_name: str, assertion_id: str) -> str:
    if test_name not in TEST_BINDINGS:
        raise ValueError(f"unreviewed hot-reload test {test_name!r}")
    if assertion_id in ORACLE_ASSERTION_IDS[test_name]:
        return "oracle_bound"
    if assertion_id in SETUP_ASSERTION_IDS[test_name]:
        return "setup_or_precondition"
    raise ValueError(
        f"unreviewed hot-reload assertion {test_name}:{assertion_id}"
    )


def _canonical_inventories(
    root: Path,
    document: Mapping[str, Any],
) -> dict[str, dict[str, Any]]:
    if set(document) != {"schema_version", "inventories"} or document.get(
        "schema_version"
    ) != 2:
        raise ValueError("hot-reload assertion document fields drifted")
    raw_rows = document.get("inventories")
    if not isinstance(raw_rows, Mapping):
        raise ValueError("hot-reload document lacks inventories")
    requested = []
    batch_rows = []
    saved: dict[str, dict[str, Any]] = {}
    for discovery_id, raw in sorted(raw_rows.items()):
        if not isinstance(discovery_id, str) or not isinstance(raw, Mapping):
            raise ValueError("hot-reload inventory is invalid")
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
            "saved hot-reload assertion inventory is not canonical"
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
    if not set(TEST_BINDINGS.values()) <= set(result):
        raise ValueError("hot-reload invariant behaviors drifted")
    return result


def _review_lock_document(
    inventories: Mapping[str, Any],
) -> dict[str, Any]:
    tests = []
    for discovery_id, raw in sorted(inventories.items()):
        path = _string(raw, "path", discovery_id)
        name = _string(raw, "test_name", discovery_id)
        if path != PATH or name not in TEST_BINDINGS:
            raise ValueError(f"unexpected hot-reload test {path}:{name}")
        assertions = raw.get("assertions")
        if not isinstance(assertions, list):
            raise ValueError(f"{path}:{name} lacks assertions")
        locked_assertions = []
        for assertion in assertions:
            if not isinstance(assertion, Mapping):
                raise ValueError(f"{path}:{name} assertion is invalid")
            assertion_id = _string(assertion, "assertion_id", name)
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
                    "disposition": _disposition(
                        test_name=name,
                        assertion_id=assertion_id,
                    ),
                    "invariant_id": INVARIANT,
                    "behavior_label": TEST_BINDINGS[name],
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
        "id": "HOT_RELOAD_6_SEMANTIC_LOCK",
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
            f"reviewed hot-reload semantic lock is missing: "
            f"{review_lock_path}"
        ) from error
    if actual != _review_lock_document(inventories):
        raise ValueError(
            "hot-reload expressions or bindings drifted from semantic lock"
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
        "hot-reload assignments generated: "
        f"{summary['tests']} tests, {summary['direct_oracle']} direct, "
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
            raise ValueError("hot-reload plan arguments must be complete")
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
            f"hot-reload catalog plans generated: {index['expected_count']} tests"
        )
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
