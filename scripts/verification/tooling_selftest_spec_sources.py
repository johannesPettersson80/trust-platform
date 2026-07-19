"""Phase 6A fixtures for the production specification-source scanner."""

from __future__ import annotations

import subprocess
import tempfile
from collections.abc import Callable, Iterator, Mapping
from contextlib import contextmanager
from pathlib import Path
from typing import Any, NamedTuple

from .spec_source_analysis import analyze_spec_sources
from .spec_source_scanner import discover_spec_documents


class SpecSourceRawResult(NamedTuple):
    disposition: str
    signal: str
    full_wiring_signal: str = ""
    forbidden_side_effect: bool = False


def spec_source_scan_known_good() -> SpecSourceRawResult:
    analysis = _analyze(
        {"docs/specs/runtime.md": "# Runtime\n\nNormative runtime behavior.\n"},
        {
            "SPEC_P6A_RUNTIME": _tracked_source(
                "SPEC_P6A_RUNTIME", "docs/specs/runtime.md"
            )
        },
        required_specs={
            "REQ_P6A_RUNTIME": {
                "id": "REQ_P6A_RUNTIME",
                "area": "runtime_safety",
                "tag": "runtime_behavior",
                "title": "Runtime behavior",
                "owner": "verification",
                "status": "mapped",
                "source_ref": "SPEC_P6A_RUNTIME",
            }
        },
    )
    errors = _error_findings(analysis)
    if errors:
        return SpecSourceRawResult("reject", _render_findings(errors))
    summary = _summary(analysis)
    if summary.get("bound_sources") != 1 or summary.get("required_topics_mapped") != 1:
        return SpecSourceRawResult("reject", f"unexpected good-scan summary: {summary}")
    return SpecSourceRawResult("accept", "no scanner or analysis failures")


def spec_source_missing_registered_path() -> SpecSourceRawResult:
    analysis = _analyze(
        {"docs/specs/runtime.md": "# Runtime\n"},
        {
            "SPEC_P6A_MISSING": _tracked_source(
                "SPEC_P6A_MISSING", "docs/specs/missing.md"
            )
        },
    )
    return _expected_error(analysis, "registered_source_missing")


def spec_source_unclosed_fence() -> SpecSourceRawResult:
    analysis = _analyze(
        {"docs/public/fence.md": "# Fence\n\n```text\nnot visible\n"},
        {},
    )
    return _expected_error(analysis, "scanner_unclosed_code_fence")


def spec_source_stale_claim_text() -> SpecSourceRawResult:
    analysis = _analyze(
        {"README.md": "# Product\n\nCurrent public claim.\n"},
        {
            "PUBLIC_CLAIM_P6A": {
                **_tracked_source("PUBLIC_CLAIM_P6A", "README.md"),
                "authority": "public_claim",
                "oracle_eligible": False,
                "claim_text": "Stale public claim.",
                "surface_ref": "README.md#product",
            }
        },
    )
    return _expected_error(analysis, "public_claim_missing")


def spec_source_escaping_include() -> SpecSourceRawResult:
    analysis = _analyze(
        {"docs/public/include.md": '# Include\n\n--8<-- "../escape.md"\n'},
        {},
    )
    return _expected_error(analysis, "scanner_escaping_include")


def spec_source_unreviewed_prose() -> SpecSourceRawResult:
    analysis = _analyze(
        {"docs/public/unreviewed.md": "# Unreviewed\n\nVisible unreviewed prose.\n"},
        {},
    )
    errors = _error_findings(analysis)
    if errors:
        return SpecSourceRawResult("reject", _render_findings(errors))
    count = _summary(analysis).get("unreviewed_public_blocks")
    if not isinstance(count, int) or count < 1:
        return SpecSourceRawResult("accept", "unreviewed public prose was not reported")
    return SpecSourceRawResult("report", f"unreviewed public prose: {count} blocks")


def _analyze(
    files: Mapping[str, str | bytes],
    spec_sources: Mapping[str, Mapping[str, Any]],
    *,
    required_specs: Mapping[str, Mapping[str, Any]] | None = None,
    spec_gaps: Mapping[str, Mapping[str, Any]] | None = None,
) -> dict[str, Any]:
    with _tracked_repository(files) as root:
        scan = discover_spec_documents(root)
        return analyze_spec_sources(
            root,
            scan=scan,
            spec_sources=spec_sources,
            required_specs=required_specs or {},
            spec_gaps=spec_gaps or {},
            obvious_topics=(),
        )


def _tracked_source(source_id: str, path: str) -> dict[str, Any]:
    return {
        "id": source_id,
        "path": path,
        "locator_kind": "tracked_file",
        "area": "runtime_safety",
        "authority": "normative_product",
        "visibility": "internal",
        "source_status": "active",
        "oracle_eligible": True,
        "last_reviewed": "2026-07-13",
        "conflicts_with": [],
    }


def _expected_error(analysis: Mapping[str, Any], expected_code: str) -> SpecSourceRawResult:
    errors = _error_findings(analysis)
    codes = [str(item.get("code")) for item in errors]
    if codes != [expected_code]:
        disposition = "reject" if errors else "accept"
        return SpecSourceRawResult(disposition, f"unexpected error codes: {codes}")
    return SpecSourceRawResult("reject", _render_findings(errors))


def _error_findings(analysis: Mapping[str, Any]) -> list[Mapping[str, Any]]:
    findings = analysis.get("findings")
    if not isinstance(findings, list):
        return [{"code": "invalid_analysis", "message": "findings is not a list"}]
    return [
        item
        for item in findings
        if isinstance(item, Mapping) and item.get("severity") == "error"
    ]


def _render_findings(findings: list[Mapping[str, Any]]) -> str:
    return "\n".join(
        f"{item.get('code')}: {item.get('message')}" for item in findings
    )


def _summary(analysis: Mapping[str, Any]) -> Mapping[str, Any]:
    value = analysis.get("summary")
    return value if isinstance(value, Mapping) else {}


@contextmanager
def _tracked_repository(files: Mapping[str, str | bytes]) -> Iterator[Path]:
    with tempfile.TemporaryDirectory() as temporary:
        root = Path(temporary)
        subprocess.run(["git", "init", "-q", str(root)], check=True)
        for relative, content in files.items():
            path = root / relative
            path.parent.mkdir(parents=True, exist_ok=True)
            if isinstance(content, bytes):
                path.write_bytes(content)
            else:
                path.write_text(content)
        subprocess.run(["git", "-C", str(root), "add", "--all"], check=True)
        yield root


SPEC_SOURCE_SCENARIO_HANDLERS: dict[str, Callable[[], SpecSourceRawResult]] = {
    "spec_source_scan_known_good": spec_source_scan_known_good,
    "spec_source_missing_registered_path": spec_source_missing_registered_path,
    "spec_source_unclosed_fence": spec_source_unclosed_fence,
    "spec_source_stale_claim_text": spec_source_stale_claim_text,
    "spec_source_escaping_include": spec_source_escaping_include,
    "spec_source_unreviewed_prose": spec_source_unreviewed_prose,
}
