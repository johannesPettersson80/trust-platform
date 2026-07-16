"""Reconstruct proof contracts from immutable Git revisions."""

from __future__ import annotations

import re
import subprocess
import tomllib
from functools import lru_cache
from pathlib import Path
from typing import Any

from .constants import ROOT


FULL_COMMIT_RE = re.compile(r"^[0-9a-f]{40}$")
CATALOG_PATH = "verification/test-catalog.toml"
INVARIANT_ROOT = "verification/invariants"


class HistoricalProofContractError(ValueError):
    """Raised when a source revision cannot reconstruct its proof contract."""


@lru_cache(maxsize=32)
def load_historical_proof_contract(
    revision: str,
    test_id: str,
    *,
    root: Path = ROOT,
) -> tuple[dict[str, Any], dict[str, dict[str, Any]]]:
    if not FULL_COMMIT_RE.fullmatch(revision):
        raise HistoricalProofContractError("source-revision proof requires a clean full commit")
    if not isinstance(test_id, str) or not test_id:
        raise HistoricalProofContractError("source-revision proof requires one test id")

    catalog = _load_toml_at(root, revision, CATALOG_PATH)
    tests = catalog.get("tests")
    if not isinstance(tests, list):
        raise HistoricalProofContractError("historical test catalog has no tests array")
    matches = [record for record in tests if isinstance(record, dict) and record.get("id") == test_id]
    if len(matches) != 1:
        raise HistoricalProofContractError(
            f"historical test {test_id} resolves to {len(matches)} catalog records"
        )
    test = matches[0]

    invariant_ids = test.get("invariants")
    if not isinstance(invariant_ids, list) or any(
        not isinstance(invariant_id, str) or not invariant_id
        for invariant_id in invariant_ids
    ):
        raise HistoricalProofContractError(
            f"historical test {test_id} has invalid invariants"
        )
    invariant_paths = _invariant_paths_at(root, revision)
    invariants: dict[str, dict[str, Any]] = {}
    for invariant_path in invariant_paths:
        document = _load_toml_at(root, revision, invariant_path)
        invariant_id = document.get("id")
        if not isinstance(invariant_id, str) or not invariant_id:
            raise HistoricalProofContractError(
                f"historical invariant {invariant_path} has no id"
            )
        if invariant_id in invariants:
            raise HistoricalProofContractError(
                f"historical invariant id {invariant_id} is duplicated"
            )
        invariants[invariant_id] = document
    missing = [invariant_id for invariant_id in invariant_ids if invariant_id not in invariants]
    if missing:
        raise HistoricalProofContractError(
            f"historical test {test_id} links missing invariants {missing}"
        )
    return test, invariants


def _invariant_paths_at(root: Path, revision: str) -> list[str]:
    result = subprocess.run(
        [
            "git",
            "ls-tree",
            "-r",
            "--name-only",
            revision,
            "--",
            INVARIANT_ROOT,
        ],
        cwd=root,
        text=True,
        stdout=subprocess.PIPE,
        stderr=subprocess.PIPE,
        check=False,
    )
    if result.returncode != 0:
        raise HistoricalProofContractError(
            f"cannot list historical invariants at {revision}: {result.stderr.strip()}"
        )
    paths = sorted(
        line
        for line in result.stdout.splitlines()
        if line.startswith(INVARIANT_ROOT + "/") and line.endswith(".toml")
    )
    if not paths:
        raise HistoricalProofContractError(
            f"historical revision {revision} has no invariant records"
        )
    return paths


def _load_toml_at(root: Path, revision: str, path: str) -> dict[str, Any]:
    result = subprocess.run(
        ["git", "show", f"{revision}:{path}"],
        cwd=root,
        stdout=subprocess.PIPE,
        stderr=subprocess.PIPE,
        check=False,
    )
    if result.returncode != 0:
        raise HistoricalProofContractError(
            f"cannot read {path} at {revision}: {result.stderr.decode(errors='replace').strip()}"
        )
    try:
        value = tomllib.loads(result.stdout.decode("utf-8"))
    except (UnicodeDecodeError, tomllib.TOMLDecodeError) as exc:
        raise HistoricalProofContractError(
            f"cannot parse {path} at {revision}: {exc}"
        ) from exc
    if not isinstance(value, dict):
        raise HistoricalProofContractError(f"{path} at {revision} is not a TOML table")
    return value
