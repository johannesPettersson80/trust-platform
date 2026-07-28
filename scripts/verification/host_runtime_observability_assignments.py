"""Generate reviewed assignments for the host-runtime observability batch."""

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


REVIEW_LOCK_PATH = Path(
    "verification/p18/review-locks/"
    "host-runtime-observability-15-semantic-lock.json"
)

EXECUTION_BACKEND_PATH = (
    "crates/trust-runtime/src/host/execution_backend.rs"
)
HISTORIAN_PATH = "crates/trust-runtime/src/host/historian/tests.rs"
METRICS_PATH = "crates/trust-runtime/src/host/metrics.rs"
API_SMOKE_PATH = "crates/trust-runtime/tests/api_smoke.rs"

BACKEND = "RT_EXECUTION_BACKEND_SELECTION_001"
HISTORIAN = "RT_HISTORIAN_RECORDING_001"
METRICS = "RT_RUNTIME_METRICS_PROFILING_001"

VM_SELECTION = "TRIMMED_CASE_INSENSITIVE_VM_SELECTION"
INVALID_BACKEND = "EMPTY_OR_UNKNOWN_BACKEND_REJECTED"
RETIRED_INTERPRETER = "RETIRED_INTERPRETER_BACKEND_REJECTED"
FRESH_RUNTIME = "FRESH_RUNTIME_DEFAULT_STATE"
LAZY_VM = "PRELOAD_SELECTION_AND_LAZY_VM_MATERIALIZATION"
METRICS_BACKEND = "METRICS_EFFECTIVE_BACKEND_PROJECTION"

TYPED_INTERVAL = "TYPED_CAPTURE_AND_SAMPLE_INTERVAL"
ALLOWLIST = "EXACT_AND_RETAIN_PREFIX_ALLOWLIST"
RELOAD = "FILE_BACKED_SERVICE_RELOAD"
PROMETHEUS = "RUNTIME_AND_HISTORIAN_PROMETHEUS_PROJECTION"

CALL_CONTRIBUTION = "CALL_IDENTITY_AND_CYCLE_CONTRIBUTION"
TOGGLE = "DISABLE_CLEAR_AND_REENABLE_COLLECTION"
TOP_CONTRIBUTOR = "TOP_CONTRIBUTOR_RANKING"
CYCLE_WINDOW = "FIVE_SAMPLE_NONWRAPPED_CYCLE_WINDOW"

INVALID_BACKEND_TEST = "parse_rejects_empty_and_invalid_values"
RETIRED_INTERPRETER_TEST = "parse_rejects_interpreter_values"
TRIMMED_VM_TEST = "parse_accepts_trimmed_values"
CASE_INSENSITIVE_VM_TEST = "parse_accepts_case_insensitive_values"
RECORDING_TEST = "recording_fidelity_and_sample_interval_are_enforced"
ALLOWLIST_TEST = "allowlist_mode_records_matching_paths_only"
RELOAD_TEST = "persistent_backend_reloads_across_service_restart"
PROMETHEUS_TEST = (
    "prometheus_render_includes_runtime_and_historian_metrics"
)
TOP_CONTRIBUTOR_TEST = (
    "profiling_top_contributors_ranked_by_cycle_budget"
)
TOGGLE_TEST = "profiling_toggle_disables_and_reenables_collection"
CYCLE_WINDOW_TEST = "cycle_percentiles_track_recent_window"
CALL_CONTRIBUTION_TEST = (
    "profiling_records_call_entries_with_cycle_contribution"
)
METRICS_BACKEND_TEST = (
    "runtime_metrics_snapshot_tracks_vm_backend_selection"
)
FRESH_RUNTIME_TEST = "loads_runtime"
LAZY_VM_TEST = (
    "runtime_execution_backend_defaults_and_lazy_vm_materialization"
)

REVIEWED_IDENTITIES: Mapping[str, tuple[str, str]] = {
    "DISC_58824C6943F8514F682A": (
        EXECUTION_BACKEND_PATH,
        INVALID_BACKEND_TEST,
    ),
    "DISC_698A7E9D7430C17901D5": (
        EXECUTION_BACKEND_PATH,
        RETIRED_INTERPRETER_TEST,
    ),
    "DISC_A31A953F14C30537DB64": (
        EXECUTION_BACKEND_PATH,
        TRIMMED_VM_TEST,
    ),
    "DISC_D52F4CAA5A2528313FE8": (
        EXECUTION_BACKEND_PATH,
        CASE_INSENSITIVE_VM_TEST,
    ),
    "DISC_1777F6DE6144940DA855": (HISTORIAN_PATH, RECORDING_TEST),
    "DISC_1B9EF96839057E1619C0": (HISTORIAN_PATH, ALLOWLIST_TEST),
    "DISC_A86EAC2CC2873C2FE31D": (HISTORIAN_PATH, RELOAD_TEST),
    "DISC_EFFABC152F3EA9FFB736": (HISTORIAN_PATH, PROMETHEUS_TEST),
    "DISC_0B198B435020505184EB": (METRICS_PATH, TOP_CONTRIBUTOR_TEST),
    "DISC_23B82380B693BF86A795": (METRICS_PATH, TOGGLE_TEST),
    "DISC_65395FAFC221AD5E0FE4": (METRICS_PATH, CYCLE_WINDOW_TEST),
    "DISC_ED49AC4813C08951E6A1": (
        METRICS_PATH,
        CALL_CONTRIBUTION_TEST,
    ),
    "DISC_56F28FCA5A66344E1C22": (
        API_SMOKE_PATH,
        METRICS_BACKEND_TEST,
    ),
    "DISC_A7805653302309DFDE9B": (
        API_SMOKE_PATH,
        FRESH_RUNTIME_TEST,
    ),
    "DISC_BA233F91FA4D55C7BE2D": (API_SMOKE_PATH, LAZY_VM_TEST),
}

COMMON_EXCLUSION = (
    "This batch does not establish arbitrary source or IEC-wide execution "
    "correctness, other production backend behavior, cross-version bytecode "
    "compatibility, or a performance budget."
)
TEST_EXCLUSIONS: Mapping[str, tuple[str, ...]] = {
    INVALID_BACKEND_TEST: (
        "The parser cases cover only the reviewed empty and unknown values, "
        "not every malformed configuration source or diagnostic transport.",
        COMMON_EXCLUSION,
    ),
    RETIRED_INTERPRETER_TEST: (
        "The retired interpreter checks prove rejection text only, not "
        "migration of arbitrary source or other production backend support.",
        COMMON_EXCLUSION,
    ),
    TRIMMED_VM_TEST: (
        "The trimmed selector case proves one whitespace-wrapped vm value, "
        "not locale-sensitive or arbitrary source normalization.",
        COMMON_EXCLUSION,
    ),
    CASE_INSENSITIVE_VM_TEST: (
        "The reviewed VM spellings do not establish a general identifier or "
        "configuration normalization rule.",
        COMMON_EXCLUSION,
    ),
    RECORDING_TEST: (
        "The reviewed in-memory sample sequence does not establish concurrent "
        "writer ordering, crash durability, or every runtime value type.",
        "The interval case uses a controlled clock and does not prove clock "
        "rollback, overflow, or long-running scheduler behavior.",
        COMMON_EXCLUSION,
    ),
    ALLOWLIST_TEST: (
        "The exact path and retain.* cases do not define a general wildcard "
        "grammar, escaping behavior, or concurrent configuration update.",
        COMMON_EXCLUSION,
    ),
    RELOAD_TEST: (
        "The orderly file-backed reopen proves one persisted value, not crash "
        "durability, corruption recovery, cross-version migration, or "
        "concurrent service access.",
        COMMON_EXCLUSION,
    ),
    PROMETHEUS_TEST: (
        "The four reviewed metric lines are not a complete Prometheus schema, "
        "escaping, cardinality, or scrape-concurrency contract.",
        COMMON_EXCLUSION,
    ),
    TOP_CONTRIBUTOR_TEST: (
        "The two reviewed entries prove relative ranking only, not stable tie "
        "ordering, concurrent collection, or a performance budget.",
        COMMON_EXCLUSION,
    ),
    TOGGLE_TEST: (
        "The sequential disable-clear-reenable case does not establish "
        "concurrent toggling or in-flight call behavior.",
        COMMON_EXCLUSION,
    ),
    CYCLE_WINDOW_TEST: (
        "The five nonwrapped samples do not prove window rollover, percentile "
        "accuracy for every population, concurrent updates, or clock faults.",
        COMMON_EXCLUSION,
    ),
    CALL_CONTRIBUTION_TEST: (
        "The reviewed calls establish identity and positive contribution only, "
        "not recursive aggregation, concurrent calls, or timing accuracy.",
        COMMON_EXCLUSION,
    ),
    METRICS_BACKEND_TEST: (
        "The attached-sink cycle proves the effective VM label only, not the "
        "complete Prometheus projection or alternate backend behavior.",
        COMMON_EXCLUSION,
    ),
    FRESH_RUNTIME_TEST: (
        "The fresh-state assertions do not execute arbitrary source, prove "
        "wall-clock behavior, or establish every constructor default.",
        COMMON_EXCLUSION,
    ),
    LAZY_VM_TEST: (
        "The one reviewed module and first cycle do not establish arbitrary "
        "source correctness, restart behavior, or cross-version loading.",
        COMMON_EXCLUSION,
    ),
}

BehaviorSelector = tuple[str, str]
Route = tuple[str, str, str]


def _reviewed_routes() -> dict[str, dict[str, Route]]:
    result: dict[str, dict[str, Route]] = {}

    def register(
        test_name: str,
        invariant: str,
        behavior: str,
        *,
        setup: tuple[str, ...] = (),
        oracle: tuple[str, ...] = (),
    ) -> None:
        routes = result.setdefault(test_name, {})
        for assertion_id, disposition in (
            *((value, "setup_or_precondition") for value in setup),
            *((value, "oracle_bound") for value in oracle),
        ):
            if assertion_id in routes:
                raise ValueError(f"duplicate reviewed assertion {assertion_id}")
            routes[assertion_id] = (invariant, behavior, disposition)

    register(
        INVALID_BACKEND_TEST,
        BACKEND,
        INVALID_BACKEND,
        oracle=(
            "ASSERT_677E95177B0F3145BA20561C",
            "ASSERT_D99B4D1A6DF7B65B0AA970ED",
            "ASSERT_7999FED1BC0E875A1E4E2FF6",
            "ASSERT_52A212E04FDA2A1452C3C38A",
        ),
    )
    register(
        RETIRED_INTERPRETER_TEST,
        BACKEND,
        RETIRED_INTERPRETER,
        oracle=(
            "ASSERT_65CF4E0466E74B89297F6D1C",
            "ASSERT_900C4EF3F27FAEB852FEA7DC",
        ),
    )
    register(
        TRIMMED_VM_TEST,
        BACKEND,
        VM_SELECTION,
        oracle=("ASSERT_0B5B8908DDEF68B0858978F1",),
    )
    register(
        CASE_INSENSITIVE_VM_TEST,
        BACKEND,
        VM_SELECTION,
        oracle=(
            "ASSERT_2871322DE4139D1CC01215E4",
            "ASSERT_5E371B526210C10BD1982F5F",
        ),
    )
    register(
        RECORDING_TEST,
        HISTORIAN,
        TYPED_INTERVAL,
        setup=(
            "ASSERT_499C71C53B3562218E5491DA",
            "ASSERT_36D58026D01AB2EF5E4F62DA",
        ),
        oracle=(
            "ASSERT_C63C584A17AFE9EE2F41D26E",
            "ASSERT_8623142687A83B2C08EBAC03",
            "ASSERT_FE81021E63C61114DA5C760C",
            "ASSERT_B72F2403E860F45243799D3E",
            "ASSERT_F3B42988159536FDDCA09DB9",
            "ASSERT_8F47964EA4066C1563E55CFA",
            "ASSERT_EE218ABEB636A7F2E5FDE739",
            "ASSERT_C8CCDA73A0F324B284999495",
            "ASSERT_458257EA181C3AD3DAF21C4B",
            "ASSERT_70F296EA57895CFA43F094EF",
            "ASSERT_C854BC41762270237A4B0D63",
            "ASSERT_CF0DD0C400588F661D7B28A4",
        ),
    )
    register(
        ALLOWLIST_TEST,
        HISTORIAN,
        ALLOWLIST,
        setup=(
            "ASSERT_779801194E82D1484F9FC0D2",
            "ASSERT_539CF06A2F04C8691DA8061E",
        ),
        oracle=(
            "ASSERT_155185BF0B2F96D4CEE5B617",
            "ASSERT_E1821E3C71D81545DCA42EE4",
            "ASSERT_C4B1B5836CBC7E4C7F29016A",
            "ASSERT_EDDFC9D5B5553FA31AB0B023",
        ),
    )
    register(
        RELOAD_TEST,
        HISTORIAN,
        RELOAD,
        setup=(
            "ASSERT_D06D3A038CF8CC1F7D6BEC73",
            "ASSERT_81DF3F68943F0559B7677A6A",
        ),
        oracle=(
            "ASSERT_A41615C912D564D2A7D6434B",
            "ASSERT_E80BC7B6CB4D714BA8F25E79",
            "ASSERT_D9398BE0C238D0F813FCD567",
            "ASSERT_05D2C099548725AAF274D610",
        ),
    )
    register(
        PROMETHEUS_TEST,
        HISTORIAN,
        PROMETHEUS,
        oracle=(
            "ASSERT_FC1D13C7C0EECF01D87272B0",
            "ASSERT_F14DFBBF215BFABE6EE3C43C",
            "ASSERT_383B98D57861278C4718DE7B",
            "ASSERT_33B05D0D50CADDF8D5D29CD1",
        ),
    )
    register(
        TOP_CONTRIBUTOR_TEST,
        METRICS,
        TOP_CONTRIBUTOR,
        oracle=(
            "ASSERT_C3AD624F17809B983214D35E",
            "ASSERT_601CE2B5D1FA756551FC4B89",
            "ASSERT_04D95E765A77C1FB773C7CF0",
        ),
    )
    register(
        TOGGLE_TEST,
        METRICS,
        TOGGLE,
        oracle=(
            "ASSERT_DE06990729CFA8DDA6B4A608",
            "ASSERT_2A71F7EBAA74F00850E47C1B",
            "ASSERT_F78032A37064FD8E816661C4",
            "ASSERT_79FB8A4A1495D303D2F38F83",
            "ASSERT_B6A1AA7D0E4D3940D376784B",
        ),
    )
    register(
        CYCLE_WINDOW_TEST,
        METRICS,
        CYCLE_WINDOW,
        oracle=(
            "ASSERT_ADF1B070D812DD2CFC81DA4C",
            "ASSERT_647FE4685B7AD799164895D5",
            "ASSERT_4CA48CB7AB289FD4F4F12D32",
            "ASSERT_3715159A84314C10B0F4A82B",
        ),
    )
    register(
        CALL_CONTRIBUTION_TEST,
        METRICS,
        CALL_CONTRIBUTION,
        oracle=(
            "ASSERT_5711E0EC5EF4AB8E0D8C9CAE",
            "ASSERT_71F071DE5358E5BC868534CD",
            "ASSERT_D656C439337CC71D699301C9",
            "ASSERT_E5F1D720F665AB32FD3AA83C",
            "ASSERT_5D0EE3CBF25C589E909546DD",
        ),
    )
    register(
        METRICS_BACKEND_TEST,
        BACKEND,
        METRICS_BACKEND,
        setup=(
            "ASSERT_09C90E4F097C15A66C9D1B22",
            "ASSERT_5C6FA57581BB4DA97844BB8F",
            "ASSERT_B7B99FDB5DC834174A6978DD",
            "ASSERT_F99B1E17B389C06AFFACB0BD",
        ),
        oracle=(
            "ASSERT_6D2AE4D1677FC8A25912E3A0",
            "ASSERT_B217AEA200792CD74A326EAE",
            "ASSERT_397B6F9211117EB07B5E2F15",
        ),
    )
    register(
        FRESH_RUNTIME_TEST,
        BACKEND,
        FRESH_RUNTIME,
        oracle=(
            "ASSERT_A6A8CCB13A6F9C1839791B58",
            "ASSERT_15DB596A89825A05C5EDDBB6",
            "ASSERT_155BEC7AFE6E4027EE0E44B1",
            "ASSERT_19624120572702A7CC49E21E",
            "ASSERT_4B561430B6E47DA8133275DF",
        ),
    )
    register(
        LAZY_VM_TEST,
        BACKEND,
        FRESH_RUNTIME,
        oracle=("ASSERT_D5FD746A549864B988740C27",),
    )
    register(
        LAZY_VM_TEST,
        BACKEND,
        LAZY_VM,
        setup=("ASSERT_EBA8A97C9863026A728450FF",),
        oracle=(
            "ASSERT_3EC31E81E7F31BE11261FF95",
            "ASSERT_6BFFE02E66D947ECCB7316DC",
            "ASSERT_B0367C67C60A16C995832FA2",
            "ASSERT_615D281269BAACB16FDC6C32",
            "ASSERT_77ABF5778561362933D4622E",
        ),
    )
    return result


REVIEWED_ROUTES: Mapping[str, Mapping[str, Route]] = _reviewed_routes()


def _denominator_row_is_live(
    row: Mapping[str, Any],
    *,
    discovery_id: str,
    expected_path: str,
    expected_name: str,
) -> bool:
    if (
        row.get("discovery_id") != discovery_id
        or row.get("path") != expected_path
        or row.get("name") != expected_name
    ):
        return False
    reviewed_nonmapping = (
        row.get("disposition") == "reviewed_nonmapping"
        and row.get("reason_code")
        == "no_reviewed_spec_or_invariant_binding"
    )
    catalog_mapped = (
        row.get("disposition") == "catalog_mapped"
        and row.get("catalog_test_id")
        == f"TEST_AUTO_{discovery_id.removeprefix('DISC_')}"
    )
    return reviewed_nonmapping or catalog_mapped


def select_live_inventories(
    *,
    root: Path,
    inventory_document: Mapping[str, Any],
) -> dict[str, Any]:
    """Select the exact pre-map or post-map denominator identities."""

    inventories = _canonical_inventories(root, inventory_document)
    denominator = tomllib.loads(
        (root / "verification/test-catalog-denominator.toml").read_text()
    )
    matched: dict[str, tuple[str, str]] = {}
    for row in denominator.get("reviews", []):
        if not isinstance(row, Mapping):
            continue
        discovery_id = str(row.get("discovery_id"))
        identity = REVIEWED_IDENTITIES.get(discovery_id)
        if identity is None:
            continue
        expected_path, expected_name = identity
        if not _denominator_row_is_live(
            row,
            discovery_id=discovery_id,
            expected_path=expected_path,
            expected_name=expected_name,
        ):
            raise ValueError(
                f"{discovery_id} host-runtime denominator identity drifted"
            )
        if discovery_id in matched:
            raise ValueError(
                f"duplicate host-runtime denominator {discovery_id}"
            )
        matched[discovery_id] = identity
    if matched != REVIEWED_IDENTITIES:
        raise ValueError(
            "live host-runtime denominator differs from semantic review"
        )
    selected = {
        discovery_id: inventories[discovery_id]
        for discovery_id in sorted(REVIEWED_IDENTITIES)
        if discovery_id in inventories
    }
    if set(selected) != set(REVIEWED_IDENTITIES):
        raise ValueError(
            "saved inventory lacks reviewed host-runtime identities"
        )
    for discovery_id, raw in selected.items():
        if (raw.get("path"), raw.get("test_name")) != REVIEWED_IDENTITIES[
            discovery_id
        ]:
            raise ValueError(
                f"{discovery_id} host-runtime inventory identity drifted"
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
    return _generate_assignment_document(inventories, _behavior_indexes(root))


def _generate_assignment_document(
    inventories: Mapping[str, Any],
    behavior_indexes: Mapping[BehaviorSelector, int],
) -> dict[str, Any]:
    rows = []
    seen: set[str] = set()
    dispositions: Counter[str] = Counter()
    behavior_counts: Counter[BehaviorSelector] = Counter()
    for discovery_id, raw in sorted(inventories.items()):
        path = _string(raw, "path", discovery_id)
        name = _string(raw, "test_name", discovery_id)
        if (
            (path, name) != REVIEWED_IDENTITIES.get(discovery_id)
            or name not in REVIEWED_ROUTES
            or name in seen
        ):
            raise ValueError(
                "unexpected or duplicate host-runtime test "
                f"{discovery_id}:{path}:{name}"
            )
        seen.add(name)
        if raw.get("unresolved_assertion_helpers") != []:
            raise ValueError(f"{path}:{name} has unresolved assertion helpers")
        assertions = raw.get("assertions")
        if not isinstance(assertions, list) or not assertions:
            raise ValueError(f"{path}:{name} lacks assertions")
        assignment_rows = []
        setup_ids = []
        for assertion in assertions:
            if not isinstance(assertion, Mapping):
                raise ValueError(f"{path}:{name} assertion is invalid")
            assertion_id = _string(assertion, "assertion_id", name)
            invariant, behavior, disposition = _route(
                test_name=name,
                assertion_id=assertion_id,
            )
            selector = (invariant, behavior)
            try:
                behavior_index = behavior_indexes[selector]
            except KeyError as error:
                raise ValueError(
                    f"missing behavior selector {invariant}:{behavior}"
                ) from error
            if disposition == "setup_or_precondition":
                setup_ids.append(assertion_id)
            assignment_rows.append(
                {
                    "assertion_id": assertion_id,
                    "invariant_id": invariant,
                    "behavior_index": behavior_index,
                }
            )
            dispositions[disposition] += 1
            behavior_counts[selector] += 1
        rows.append(
            {
                "path": path,
                "name": name,
                "assertion_assignments": assignment_rows,
                "setup_assertion_ids": setup_ids,
                "non_oracle_assertion_ids": [],
            }
        )
    if seen != set(REVIEWED_ROUTES):
        raise ValueError("host-runtime assignments are not exhaustive")
    if dispositions != Counter(
        {"oracle_bound": 64, "setup_or_precondition": 11}
    ):
        raise ValueError(
            "host-runtime assertion dispositions drifted from review"
        )
    return {
        "schema_version": 3,
        "basis": "reviewed_host_runtime_observability_15_semantic_lock",
        "summary": {
            "tests": len(rows),
            "assertions": sum(dispositions.values()),
            "direct_oracle": dispositions["oracle_bound"],
            "setup_or_precondition": dispositions[
                "setup_or_precondition"
            ],
            "non_oracle_observation": 0,
            "behavior_assertions": {
                f"{invariant}:{behavior}": count
                for (invariant, behavior), count in sorted(
                    behavior_counts.items()
                )
            },
        },
        "assignments": rows,
    }


def _route(*, test_name: str, assertion_id: str) -> Route:
    routes = REVIEWED_ROUTES.get(test_name)
    if routes is None:
        raise ValueError(f"unreviewed host-runtime test {test_name!r}")
    route = routes.get(assertion_id)
    if route is None:
        raise ValueError(
            f"unreviewed host-runtime assertion {test_name}:{assertion_id}"
        )
    return route


def _canonical_inventories(
    root: Path,
    document: Mapping[str, Any],
) -> dict[str, dict[str, Any]]:
    if set(document) != {"schema_version", "inventories"} or document.get(
        "schema_version"
    ) != 2:
        raise ValueError("host-runtime assertion document fields drifted")
    raw_rows = document.get("inventories")
    if not isinstance(raw_rows, Mapping):
        raise ValueError("host-runtime document lacks inventories")
    requested = []
    batch_rows = []
    saved: dict[str, dict[str, Any]] = {}
    for discovery_id, raw in sorted(raw_rows.items()):
        if not isinstance(discovery_id, str) or not isinstance(raw, Mapping):
            raise ValueError("host-runtime inventory is invalid")
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
            "saved host-runtime assertion inventory is not canonical"
        )
    return canonical


def _behavior_indexes(root: Path) -> dict[BehaviorSelector, int]:
    required = {
        (invariant, behavior)
        for routes in REVIEWED_ROUTES.values()
        for invariant, behavior, _disposition in routes.values()
    }
    result: dict[BehaviorSelector, int] = {}
    for invariant_id in {selector[0] for selector in required}:
        matches = list(
            (root / "verification/invariants").glob(
                f"**/{invariant_id}.toml"
            )
        )
        if len(matches) != 1:
            raise ValueError(f"expected one {invariant_id} invariant")
        record = tomllib.loads(matches[0].read_text())
        behaviors = record.get("behavior")
        if not isinstance(behaviors, list):
            raise ValueError(f"{invariant_id} lacks behaviors")
        for index, behavior in enumerate(behaviors):
            partition = behavior.get("partition")
            label = (
                partition.get("equals")
                if isinstance(partition, Mapping)
                else None
            )
            if not isinstance(label, str) or not label:
                continue
            selector = (invariant_id, label)
            if selector in result:
                raise ValueError(
                    f"duplicate behavior selector {invariant_id}:{label}"
                )
            result[selector] = index
    missing = required - set(result)
    if missing:
        raise ValueError(
            f"missing host-runtime behavior selectors: {sorted(missing)}"
        )
    return {selector: result[selector] for selector in sorted(required)}


def _review_lock_document(
    inventories: Mapping[str, Any],
) -> dict[str, Any]:
    tests = []
    for discovery_id, raw in sorted(inventories.items()):
        path = _string(raw, "path", discovery_id)
        name = _string(raw, "test_name", discovery_id)
        if (
            (path, name) != REVIEWED_IDENTITIES.get(discovery_id)
            or name not in REVIEWED_ROUTES
        ):
            raise ValueError(
                f"unexpected host-runtime test {discovery_id}:{path}:{name}"
            )
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
                "discovery_id": discovery_id,
                "path": path,
                "name": name,
                "source_digest": _string(
                    raw,
                    "source_digest",
                    discovery_id,
                ),
                "exclusions": list(TEST_EXCLUSIONS[name]),
                "assertions": locked_assertions,
            }
        )
    return {
        "schema_version": 1,
        "id": "HOST_RUNTIME_OBSERVABILITY_15_SEMANTIC_LOCK",
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
            "reviewed host-runtime semantic lock is missing: "
            f"{review_lock_path}"
        ) from error
    if actual != _review_lock_document(inventories):
        raise ValueError(
            "host-runtime expressions or bindings drifted from semantic lock"
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
        "host-runtime observability assignments generated: "
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
            raise ValueError(
                "host-runtime plan arguments must be complete"
            )
        index = generate_path_plans(
            root=root,
            inventory_document=selected,
            assignments_document=assignments,
            execution_dir=_rooted(root, args.execution_dir),
            output_dir=_rooted(root, args.plans_out),
            reviewed_date=args.reviewed_date,
            timestamp=args.timestamp,
        )
        print(
            "host-runtime observability catalog plans generated: "
            f"{index['expected_count']} tests across "
            f"{len(index['paths'])} paths"
        )
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
