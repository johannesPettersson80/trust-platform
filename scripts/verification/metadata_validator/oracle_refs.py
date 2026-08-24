"""Oracle and behavior-row reference checks for verification metadata."""

from __future__ import annotations

import re
from pathlib import Path
from typing import Any, Callable

from .constants import CASE_FAMILIES


Fail = Callable[[Path, str], None]

LABEL_RE = re.compile(r"^[A-Z][A-Z0-9_]*$")
ORACLE_AUTHORITIES = {
    "normative_external",
    "normative_product",
    "reviewed_decision",
    "reviewed_deviation",
}
ERROR_MODEL_TAG = "stable_error_code_model"
ALLOWED_PARTITION_SHAPES = {
    frozenset({"min"}),
    frozenset({"max"}),
    frozenset({"min", "max"}),
    frozenset({"below"}),
    frozenset({"above"}),
    frozenset({"equals"}),
    frozenset({"wrong_type"}),
    frozenset({"malformed"}),
}


def validate_oracle_ref(
    *,
    fail: Fail,
    path: Path,
    owner_id: str,
    oracle_ref: Any,
    spec_sources: dict[str, dict[str, Any]],
) -> None:
    if not isinstance(oracle_ref, str) or not oracle_ref:
        fail(path, f"{owner_id} oracle_ref must be a non-empty string")
        return
    source_id = oracle_ref.split("#", 1)[0]
    source = spec_sources.get(source_id)
    if source is None:
        fail(path, f"{owner_id} oracle_ref references unknown spec source {source_id!r}")
        return
    if source.get("source_status") != "active":
        fail(path, f"{owner_id} oracle_ref references non-active spec source {source_id!r}")
    if source.get("oracle_eligible") is not True:
        fail(
            path,
            f"{owner_id} oracle_ref references provenance-only spec source {source_id!r}",
        )
    if source.get("authority") not in ORACLE_AUTHORITIES:
        fail(path, f"{owner_id} oracle_ref cannot use authority {source.get('authority')!r}")


def validate_error_code_ref(
    *,
    fail: Fail,
    path: Path,
    owner_id: str,
    behavior: dict[str, Any],
    spec_sources: dict[str, dict[str, Any]],
) -> None:
    if "error_code" not in behavior:
        return
    if not any(
        source.get("source_status") == "active" and ERROR_MODEL_TAG in source.get("covers", [])
        for source in spec_sources.values()
    ):
        fail(path, f"{owner_id} behavior error_code requires an active {ERROR_MODEL_TAG} spec source")


def validate_partition_contract(
    *,
    fail: Fail,
    path: Path,
    owner_id: str,
    behavior: dict[str, Any],
) -> None:
    partition = behavior.get("partition")
    if not isinstance(partition, dict):
        return
    key_set = frozenset(partition)
    if key_set not in ALLOWED_PARTITION_SHAPES:
        fail(path, f"{owner_id} behavior partition has unsupported key set {sorted(key_set)}")
    if "equals" not in partition:
        if "case_family" in behavior:
            fail(path, f"{owner_id} case_family is only allowed for equals partitions")
        dimension = behavior.get("coverage_dimension")
        if dimension is not None and dimension not in CASE_FAMILIES:
            fail(path, f"{owner_id} behavior coverage_dimension must be canonical")
        return
    value = partition["equals"]
    if isinstance(value, str) and not LABEL_RE.fullmatch(value):
        fail(path, f"{owner_id} partition.equals must be an opaque UPPER_CASE_LABEL, got {value!r}")
    family = behavior.get("case_family")
    if family not in CASE_FAMILIES:
        fail(path, f"{owner_id} equals partition requires canonical case_family")
    if "coverage_dimension" in behavior:
        fail(path, f"{owner_id} equals partition uses case_family, not coverage_dimension")
