"""Bind the Phase 9 program to one durable, fully reconciled fuzz campaign."""

from __future__ import annotations

import hashlib
import tomllib
from collections.abc import Mapping
from pathlib import Path
from typing import Any

from .fuzz_campaign_contract import validate_campaign_json
from .fuzz_crash_regressions import REGISTRY_PATH, load_crash_registry


CAMPAIGN_EVIDENCE_PATH = (
    "docs/internal/testing/evidence/plc-verification-program/2026-07-18/"
    "p16-fuzz-campaign.json"
)
TEST_CATALOG_PATH = "verification/test-catalog.toml"


def validate_campaign_binding(
    root: Path,
    handoff: object,
    *,
    program: Mapping[str, Any],
) -> list[str]:
    if not isinstance(handoff, Mapping):
        return ["fuzz program crash-regression handoff must be a table"]
    failures: list[str] = []
    campaign_path = root / CAMPAIGN_EVIDENCE_PATH
    try:
        campaign_text = campaign_path.read_text()
    except OSError as exc:
        return [f"bounded fuzz campaign evidence cannot be read: {exc}"]
    digest = hashlib.sha256(campaign_text.encode()).hexdigest()
    if handoff.get("campaign_evidence_sha256") != digest:
        failures.append("bounded fuzz campaign evidence digest does not match the handoff")
    if handoff.get("campaign_evidence_path") != CAMPAIGN_EVIDENCE_PATH:
        failures.append("bounded fuzz campaign evidence path drifted")
    if handoff.get("registry_path") != REGISTRY_PATH:
        failures.append("fuzz crash-regression registry path drifted")
    try:
        registry = load_crash_registry(root)
        catalog = tomllib.loads((root / TEST_CATALOG_PATH).read_text())
    except (OSError, tomllib.TOMLDecodeError) as exc:
        return sorted(set([*failures, f"campaign binding input cannot be read: {exc}"]))
    tests = {
        row.get("id"): row
        for row in catalog.get("tests", [])
        if isinstance(row, Mapping) and isinstance(row.get("id"), str)
    }
    failures.extend(
        validate_campaign_json(
            campaign_text,
            program=program,
            tests=tests,
            regression_registry=registry,
        )
    )
    return sorted(set(failures))
