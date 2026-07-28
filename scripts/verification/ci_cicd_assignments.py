"""Generate reviewed assignments for native CI fixture contract tests."""

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


PATH = "crates/trust-runtime/tests/ci_cicd_contract.rs"
INVARIANT = "DEV_NATIVE_CI_FIXTURE_001"
REVIEW_LOCK_PATH = Path(
    "verification/p18/review-locks/ci-cicd-3-semantic-lock.json"
)

JSON_SUCCESS = "VERSIONED_BUILD_VALIDATE_TEST_JSON_SUCCESS"
GREEN_JUNIT = "GREEN_PROJECT_JUNIT_SUCCESS"
BROKEN_JUNIT = "BROKEN_PROJECT_JUNIT_FAILURE"

JSON_TEST = "ci_json_summary_mode_contract_is_stable"
GREEN_TEST = "ci_template_workflow_passes_on_green_fixture"
BROKEN_TEST = (
    "ci_template_workflow_fails_on_broken_fixture_"
    "with_expected_code_and_report"
)

REVIEWED_IDENTITIES: Mapping[str, str] = {
    "DISC_4692745A9A337DAF089C": GREEN_TEST,
    "DISC_85AF4E1985BEEBE3A2A6": JSON_TEST,
    "DISC_AFC7F8F303A4F231290C": BROKEN_TEST,
}

EXCLUDED_TIMING_IDENTITIES: Mapping[str, str] = {
    "DISC_26D85E382B6B90BB76E6": (
        "ci_run_all_tests_scales_roughly_linearly_for_small_projects"
    ),
    "DISC_627DBD026621561D0A46": (
        "ci_clean_setup_first_passing_test_is_under_ten_minutes"
    ),
    "DISC_A7027A42E2C68847E276": (
        "ci_single_test_feedback_is_under_two_seconds_for_small_project"
    ),
}

BOUNDED_FIXTURE_EXCLUSION = (
    "These are bounded green and broken fixture contracts. They do not "
    "certify arbitrary projects, every JSON field, JUnit XML schema "
    "conformance, or diagnostic prose beyond the reviewed marker."
)
PERFORMANCE_EXCLUSION = (
    "These mappings establish no timing, performance, throughput, scaling, "
    "latency, clean-install, or clean-toolchain authority."
)

TEST_EXCLUSIONS: Mapping[str, tuple[str, ...]] = {
    JSON_TEST: (
        "The JSON test covers only the named version-1 build, validate, and "
        "test fields for the reviewed green fixture.",
        BOUNDED_FIXTURE_EXCLUSION,
        PERFORMANCE_EXCLUSION,
    ),
    GREEN_TEST: (
        "The green JUnit test observes one testsuite marker and zero failures; "
        "it does not validate the complete XML document.",
        BOUNDED_FIXTURE_EXCLUSION,
        PERFORMANCE_EXCLUSION,
    ),
    BROKEN_TEST: (
        "The broken JUnit test covers process class 12, one failure marker, "
        "and the named failed-ST-test stderr marker only.",
        BOUNDED_FIXTURE_EXCLUSION,
        PERFORMANCE_EXCLUSION,
    ),
}

Route = tuple[str, str, str]


def _reviewed_routes() -> dict[str, dict[str, Route]]:
    result: dict[str, dict[str, Route]] = {}

    def register(
        test_name: str,
        behavior: str,
        *,
        setup: tuple[str, ...],
        oracle: tuple[str, ...],
    ) -> None:
        routes = result.setdefault(test_name, {})
        for assertion_id, disposition in (
            *((value, "setup_or_precondition") for value in setup),
            *((value, "oracle_bound") for value in oracle),
        ):
            if assertion_id in routes:
                raise ValueError(f"duplicate reviewed assertion {assertion_id}")
            routes[assertion_id] = (INVARIANT, behavior, disposition)

    register(
        JSON_TEST,
        JSON_SUCCESS,
        setup=(
            "ASSERT_57BBC5831F532BE90275D338",
            "ASSERT_79DF5D2282F7F87A528DA43E",
            "ASSERT_32AF5E9065860CEB548E2236",
            "ASSERT_6B93CAD4E9B5261B2A89A93D",
            "ASSERT_61D115018DB3E0F505C12E9E",
            "ASSERT_6F9006F5A4602F516A3CBEDC",
            "ASSERT_3A64E369C13413D275FC4579",
            "ASSERT_344A1EC1B509B213640590DF",
            "ASSERT_773B40CC14CD374E7E2F44FC",
            "ASSERT_5DA66DB03DEC26A40B40FAA6",
            "ASSERT_E31BEB2AE2B71F0528E0C670",
            "ASSERT_7EC777520666E2DF00D8EE5D",
            "ASSERT_6C2863C0E2C693DA3AE1622A",
        ),
        oracle=(
            "ASSERT_FB9A7EA222D874228650E343",
            "ASSERT_61E5AA50F3B2FD63B95E257A",
            "ASSERT_3BFD10915283B6CF252757F9",
            "ASSERT_CE2DA5E158D85AD01480E2D3",
            "ASSERT_CBA2E3857866CCB4D045E9DC",
            "ASSERT_830FFA33DC54E69506AE5C30",
            "ASSERT_026BAE883EA85D0E6F2CEBCA",
            "ASSERT_0A0E9F4308A0194995E2790E",
            "ASSERT_120FD8B3EE9BCCB07830C3F5",
            "ASSERT_C7507CF07BFE119AEE865BEC",
            "ASSERT_DB1D8BC2BCE4C3EA1F738904",
            "ASSERT_578DD6400E8FA3F31D7464DE",
            "ASSERT_6892734E44E17D02141F3992",
            "ASSERT_39DACF8FA01A526201AC7543",
            "ASSERT_AD905E1D8CB57B6CC7C0C49F",
            "ASSERT_BD048153A7A9C4952EC9D794",
            "ASSERT_8328513E4FAD8FBBBF26D939",
            "ASSERT_F3D56E676407361D30471707",
            "ASSERT_104EF62209FD2B6EF1C7D36A",
        ),
    )
    register(
        GREEN_TEST,
        GREEN_JUNIT,
        setup=(
            "ASSERT_41290EEEBB780E24FDDD46EF",
            "ASSERT_1119FE85F97A7533BB7A4752",
            "ASSERT_1DBB87C7F76DB4D85ACA9001",
            "ASSERT_ED812C45C1BC9000169B0D5F",
            "ASSERT_FE3337E75523643C1304B181",
            "ASSERT_29A60F1B1D116CE5D739C239",
            "ASSERT_4AA9821882E981850D2F12A2",
            "ASSERT_0DB9A690AA595C4DB30FAFD5",
            "ASSERT_FB9FD6900A817CE193E5DA17",
            "ASSERT_F4A60EBD89E34D574C281DDF",
        ),
        oracle=(
            "ASSERT_DC2AF8D768D8A45F04F8C606",
            "ASSERT_FE0554456BDC6D0DA56665A0",
            "ASSERT_FBDF1090E0AA2E5E37C360AA",
        ),
    )
    register(
        BROKEN_TEST,
        BROKEN_JUNIT,
        setup=(
            "ASSERT_3720BC7F577E8D14A18C97FC",
            "ASSERT_4622D8DE1678633BE48235DE",
            "ASSERT_3AF4C4CA8D6948AB26A559C7",
            "ASSERT_D0445292C343ABA229126B06",
            "ASSERT_526BEBFDE1D550E590CD1E8F",
            "ASSERT_F4A6C95E8F2C952B8A5288A7",
            "ASSERT_422EA2B6C4B445ABB89C939A",
            "ASSERT_2DD11A105C41720F9AB82D2A",
            "ASSERT_4269C8D4590C24FF6CF72588",
            "ASSERT_D6AB2C618F64804092ED04E1",
        ),
        oracle=(
            "ASSERT_743F5ECE9248E42B2CF81599",
            "ASSERT_FA2FD30130E946E0605170AF",
            "ASSERT_D156F03F8C8446400A7D630D",
            "ASSERT_B4786EAED7192215454B8B41",
            "ASSERT_190559B687D24DABA77A5C82",
            "ASSERT_6934A407E175A3A5DDAFC863",
        ),
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
    matched: dict[str, str] = {}
    for row in denominator.get("reviews", []):
        if not isinstance(row, Mapping):
            continue
        discovery_id = str(row.get("discovery_id"))
        if discovery_id not in REVIEWED_IDENTITIES:
            continue
        expected_name = REVIEWED_IDENTITIES[discovery_id]
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
        if (
            row.get("path") != PATH
            or row.get("name") != expected_name
            or not (reviewed_nonmapping or catalog_mapped)
        ):
            raise ValueError(
                f"{discovery_id} CI/CD denominator identity drifted"
            )
        if discovery_id in matched:
            raise ValueError(f"duplicate CI/CD denominator {discovery_id}")
        matched[discovery_id] = expected_name
    if matched != REVIEWED_IDENTITIES:
        raise ValueError("live CI/CD denominator differs from semantic review")
    selected = {
        discovery_id: inventories[discovery_id]
        for discovery_id in sorted(REVIEWED_IDENTITIES)
        if discovery_id in inventories
    }
    if set(selected) != set(REVIEWED_IDENTITIES):
        raise ValueError("saved inventory lacks reviewed CI/CD identities")
    for discovery_id, raw in selected.items():
        if (raw.get("path"), raw.get("test_name")) != (
            PATH,
            REVIEWED_IDENTITIES[discovery_id],
        ):
            raise ValueError(f"{discovery_id} CI/CD inventory identity drifted")
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
        if (
            path != PATH
            or name not in REVIEWED_ROUTES
            or name in seen
            or REVIEWED_IDENTITIES.get(discovery_id) != name
        ):
            raise ValueError(
                f"unexpected or duplicate CI/CD test {discovery_id}:{path}:{name}"
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
            if disposition == "setup_or_precondition":
                setup_ids.append(assertion_id)
            assignment_rows.append(
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
                "assertion_assignments": assignment_rows,
                "setup_assertion_ids": setup_ids,
                "non_oracle_assertion_ids": [],
            }
        )
    if seen != set(REVIEWED_ROUTES):
        raise ValueError("CI/CD assignments are not exhaustive")
    if sum(dispositions.values()) != 61:
        raise ValueError("CI/CD assertion count drifted from reviewed 61")
    if dispositions != Counter(
        {"setup_or_precondition": 33, "oracle_bound": 28}
    ):
        raise ValueError("CI/CD assertion dispositions drifted from review")
    return {
        "schema_version": 3,
        "basis": "reviewed_ci_cicd_3_semantic_lock",
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


def _route(*, test_name: str, assertion_id: str) -> Route:
    routes = REVIEWED_ROUTES.get(test_name)
    if routes is None:
        raise ValueError(f"unreviewed CI/CD test {test_name!r}")
    route = routes.get(assertion_id)
    if route is None:
        raise ValueError(
            f"unreviewed CI/CD assertion {test_name}:{assertion_id}"
        )
    return route


def _canonical_inventories(
    root: Path,
    document: Mapping[str, Any],
) -> dict[str, dict[str, Any]]:
    if set(document) != {"schema_version", "inventories"} or document.get(
        "schema_version"
    ) != 2:
        raise ValueError("CI/CD assertion document fields drifted")
    raw_rows = document.get("inventories")
    if not isinstance(raw_rows, Mapping):
        raise ValueError("CI/CD document lacks inventories")
    requested = []
    batch_rows = []
    saved: dict[str, dict[str, Any]] = {}
    for discovery_id, raw in sorted(raw_rows.items()):
        if not isinstance(discovery_id, str) or not isinstance(raw, Mapping):
            raise ValueError("CI/CD inventory is invalid")
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
        raise ValueError("saved CI/CD assertion inventory is not canonical")
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
    required = {JSON_SUCCESS, GREEN_JUNIT, BROKEN_JUNIT}
    if required != set(result):
        raise ValueError("CI/CD invariant behaviors drifted")
    return result


def _review_lock_document(
    inventories: Mapping[str, Any],
) -> dict[str, Any]:
    tests = []
    for discovery_id, raw in sorted(inventories.items()):
        path = _string(raw, "path", discovery_id)
        name = _string(raw, "test_name", discovery_id)
        if (
            REVIEWED_IDENTITIES.get(discovery_id) != name
            or path != PATH
            or name not in REVIEWED_ROUTES
        ):
            raise ValueError(
                f"unexpected CI/CD test {discovery_id}:{path}:{name}"
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
                "exclusions": list(TEST_EXCLUSIONS[name]),
                "assertions": locked_assertions,
            }
        )
    return {
        "schema_version": 1,
        "id": "CI_CICD_3_SEMANTIC_LOCK",
        "excluded_timing_identities": dict(EXCLUDED_TIMING_IDENTITIES),
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
            f"reviewed CI/CD semantic lock is missing: {review_lock_path}"
        ) from error
    if actual != _review_lock_document(inventories):
        raise ValueError(
            "CI/CD expressions or bindings drifted from semantic lock"
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
        "CI/CD assignments generated: "
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
            raise ValueError("CI/CD plan arguments must be complete")
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
            f"CI/CD catalog plans generated: {index['expected_count']} tests"
        )
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
