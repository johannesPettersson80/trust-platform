"""Thin orchestration boundary for Phase 18 proof-class validators."""

from __future__ import annotations

from dataclasses import dataclass
from pathlib import Path
from typing import Any, Mapping


from . import (
    assertion_reachability,
    concurrency_time,
    coverage,
    existing_test_mapping,
    fuzz_property,
    hardware_endurance,
    mutation,
    platform_claims,
    release_evidence,
    rendered_ux,
    semantic_chains,
    unsafe_ffi,
)
from .contracts import validate_bundle


PROOF_VALIDATORS = {
    "semantic_chains": semantic_chains.validate,
    "test_mapping": existing_test_mapping.validate,
    "assertion_reachability": assertion_reachability.validate,
    "coverage": coverage.validate,
    "mutation": mutation.validate,
    "fuzz_property": fuzz_property.validate,
    "concurrency_time": concurrency_time.validate,
    "unsafe_ffi": unsafe_ffi.validate,
    "hardware_endurance": hardware_endurance.validate,
    "rendered_ux": rendered_ux.validate,
    "platform_claims": platform_claims.validate,
    "release_evidence": release_evidence.validate,
}


@dataclass(frozen=True)
class GateResult:
    exit_code: int
    report: dict[str, Any]


def evaluate_bundle(
    ledger: Mapping[str, Any],
    budgets: Mapping[str, Any],
    claims: Mapping[str, Any],
    *,
    artifact_root: Path,
    expected_source_sha: str,
    mode: str,
) -> GateResult:
    failures = validate_bundle(
        ledger,
        budgets,
        claims,
        artifact_root=artifact_root,
        expected_source_sha=expected_source_sha,
        validators=PROOF_VALIDATORS,
    )
    summary = dict(ledger.get("current", {})) if isinstance(ledger.get("current"), Mapping) else {}
    open_debt = any(isinstance(value, int) and value != 0 for value in summary.values())
    open_state = any(
        document.get("state") not in {"closed", "frozen"}
        for document in (ledger, budgets, claims)
    )
    if failures:
        status = "contract_error"
        exit_code = 2
    elif mode not in {"report", "strict"}:
        status = "contract_error"
        failures = [f"unsupported mode {mode!r}"]
        exit_code = 2
    elif mode == "strict" and (open_debt or open_state):
        status = "open_debt"
        exit_code = 1
    else:
        status = "clear" if not open_debt and not open_state else "report_only"
        exit_code = 0
    report = {
        "schema_version": 1,
        "status": status,
        "source_sha": expected_source_sha,
        "summary": summary,
        "proof_classes": [row.get("id") for row in ledger.get("proof_classes", [])],
        "findings": failures,
    }
    return GateResult(exit_code=exit_code, report=report)
