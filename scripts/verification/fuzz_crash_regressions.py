"""Committed crash-to-regression registry for bounded fuzz campaigns."""

from __future__ import annotations

import re
import tomllib
from collections.abc import Mapping, Sequence
from pathlib import Path
from typing import Any


REGISTRY_PATH = "verification/fuzz-crash-regressions.toml"
DIGEST_RE = re.compile(r"^sha256:[0-9a-f]{64}$")
TOP_FIELDS = {
    "schema_version",
    "id",
    "status",
    "required_disposition",
    "regressions",
}
ROW_FIELDS = {"target_id", "artifact_sha256", "test_id", "rationale"}


def load_crash_registry(root: Path) -> dict[str, Any]:
    return tomllib.loads((root / REGISTRY_PATH).read_text())


def validate_crash_registry(
    registry: object,
    *,
    program: Mapping[str, Any],
    tests: Mapping[str, Mapping[str, Any]],
) -> list[str]:
    failures: list[str] = []
    if not isinstance(registry, Mapping):
        return ["crash regression registry must be a table"]
    if set(registry) != TOP_FIELDS:
        failures.append(f"crash regression registry fields must equal {sorted(TOP_FIELDS)}")
    for field, expected in (
        ("schema_version", 1),
        ("id", "FUZZ_CRASH_REGRESSIONS_V1"),
        ("status", "mapped"),
        ("required_disposition", "deterministic_regression"),
    ):
        if registry.get(field) != expected:
            failures.append(f"crash regression registry {field} must equal {expected!r}")

    target_ids = {
        row.get("id")
        for row in program.get("targets", [])
        if isinstance(row, Mapping) and isinstance(row.get("id"), str)
    }
    rows = registry.get("regressions")
    if not isinstance(rows, list):
        failures.append("crash regression registry regressions must be an array")
        return sorted(set(failures))
    seen: set[tuple[str, str]] = set()
    canonical: list[tuple[str, str]] = []
    for index, row in enumerate(rows):
        label = f"crash regression registry regressions[{index}]"
        if not isinstance(row, Mapping):
            failures.append(f"{label} must be a table")
            continue
        if set(row) != ROW_FIELDS:
            failures.append(f"{label} fields must equal {sorted(ROW_FIELDS)}")
        target_id = row.get("target_id")
        digest = row.get("artifact_sha256")
        if target_id not in target_ids:
            failures.append(f"{label} target_id {target_id!r} is not registered")
        if not isinstance(digest, str) or not DIGEST_RE.fullmatch(digest):
            failures.append(f"{label} artifact_sha256 must be sha256:<64 lowercase hex>")
        key = (str(target_id), str(digest))
        if key in seen:
            failures.append(f"{label} duplicates target and artifact digest {key!r}")
        seen.add(key)
        canonical.append(key)
        test_id = row.get("test_id")
        test = tests.get(test_id) if isinstance(test_id, str) else None
        if not isinstance(test, Mapping) or test.get("status") != "mapped" or not test.get(
            "command"
        ):
            failures.append(f"{label} test_id {test_id!r} is not a mapped test")
        rationale = row.get("rationale")
        if not isinstance(rationale, str) or not rationale.strip():
            failures.append(f"{label} rationale must be non-empty")
    if canonical != sorted(canonical):
        failures.append("crash regression registry rows must use canonical target/digest order")
    return sorted(set(failures))


def campaign_regressions(
    registry: Mapping[str, Any], results: Sequence[Mapping[str, Any]]
) -> list[dict[str, Any]]:
    observed = {
        (str(result.get("target_id")), str(artifact.get("sha256")))
        for result in results
        for artifact in _artifacts(result)
    }
    rows = registry.get("regressions", [])
    if not isinstance(rows, list):
        return []
    return [
        dict(row)
        for row in rows
        if isinstance(row, Mapping)
        and (str(row.get("target_id")), str(row.get("artifact_sha256"))) in observed
    ]


def _artifacts(result: Mapping[str, Any]) -> list[Mapping[str, Any]]:
    value = result.get("artifact_files")
    if not isinstance(value, list):
        return []
    return [row for row in value if isinstance(row, Mapping)]
