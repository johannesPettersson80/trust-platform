"""Phase 17 enforcement policy over mode-neutral live findings."""

from __future__ import annotations

from collections.abc import Mapping, Sequence
from typing import Any


ENFORCEMENT_MODES = frozenset({"report", "changed", "full"})


def finding_kind(failure: str) -> str:
    _code_id, separator, detail = failure.partition(": ")
    return detail.split(maxsplit=1)[0] if separator else "global"


def blocking_failures(
    mode: str,
    failures: Sequence[str],
    debt_code_ids: set[str] | frozenset[str],
) -> list[str]:
    if mode not in ENFORCEMENT_MODES:
        raise ValueError(f"unknown code-spec enforcement mode {mode!r}")
    if mode == "report":
        return []
    if mode == "full":
        return sorted(set(failures))
    blocking = []
    for failure in sorted(set(failures)):
        code_id, separator, _detail = failure.partition(": ")
        if (
            separator
            and code_id in debt_code_ids
            and finding_kind(failure) in {"missing_spec", "unmapped"}
        ):
            continue
        blocking.append(failure)
    return blocking


def changed_path_outcomes(
    changed_paths: Sequence[str],
    *,
    facts: Mapping[str, Any],
    mappings: Mapping[str, Mapping[str, Any]],
    failures: Sequence[str],
) -> list[str]:
    normalized_paths = {path.replace("\\", "/") for path in changed_paths}
    changed_code_ids = {
        code_id
        for code_id, fact in facts.items()
        if _fact_path(fact) in normalized_paths
    }
    outcomes = [
        failure
        for failure in failures
        if failure.partition(": ")[0] in changed_code_ids
    ]
    for code_id in sorted(changed_code_ids):
        mapping = mappings.get(code_id)
        if mapping and mapping.get("disposition") == "spec_gap":
            gap = mapping.get("spec_gap_ref", "<missing-gap-ref>")
            outcomes.append(f"{code_id}: spec_gap {gap}")
    return sorted(set(outcomes))


def _fact_path(fact: Any) -> str | None:
    if isinstance(fact, Mapping):
        value = fact.get("path")
    else:
        value = getattr(fact, "path", None)
    return value if isinstance(value, str) else None
