"""Validated Phase 17 TypeScript/JavaScript product denominator adapter."""

from __future__ import annotations

import json
import os
import subprocess
from dataclasses import dataclass, field
from pathlib import Path
from typing import Any, Mapping

from .code_spec_common import RISK_SIGNALS, CodeFact, sha256_digest, stable_code_id
from .code_spec_source_adapter import (
    CodeSpecSourceError,
    candidate_area_owners,
    governed_source_bytes,
    select_area_owner,
)


PRODUCT_ROOTS = (
    "editors/vscode/src",
    "crates/trust-runtime/src/web/ui",
    "crates/trust-runtime/web/ide-frontend/src",
)

# These build definitions are executable packaging inputs for the generated IDE
# bundles below. They are exact files rather than an additional recursive product
# root so unrelated frontend build tooling cannot enter the denominator silently.
PACKAGING_INPUTS = (
    "crates/trust-runtime/web/ide-frontend/vite.config.ts",
    "crates/trust-runtime/web/ide-frontend/vite.monaco.config.ts",
)

ASSET_SOURCE_MODES = {
    "crates/trust-runtime/src/web/ui/assets/ide-codemirror.20260215.js": "generated",
    "crates/trust-runtime/src/web/ui/assets/ide-monaco.20260215.js": "generated",
    "crates/trust-runtime/src/web/ui/modules/cytoscape.min.js": "vendored_external",
}


class CodeSpecTypeScriptError(ValueError):
    """Raised when the browser/extension denominator is incomplete."""


@dataclass(frozen=True)
class TypeScriptInventory:
    facts: Mapping[str, CodeFact] = field(default_factory=dict)
    source_paths: tuple[str, ...] = ()
    source_count: int = 0
    source_counts: Mapping[str, int] = field(default_factory=dict)
    unmapped_code_ids: tuple[str, ...] = ()


def discover_product_sources(root: Path) -> list[dict[str, str]]:
    sources: list[dict[str, str]] = []
    excluded_directories = {
        "node_modules",
        "out",
        "dist",
        "coverage",
        "test",
        "tests",
        "__tests__",
        ".vscode-test",
    }
    for product_root in PRODUCT_ROOTS:
        directory = root / product_root
        if not directory.is_dir():
            raise CodeSpecTypeScriptError(f"missing TypeScript/JavaScript product root {product_root}")
        root_count = 0
        for current, directories, files in os.walk(directory, followlinks=False):
            current_path = Path(current)
            kept_directories = []
            for name in directories:
                candidate = current_path / name
                if candidate.is_symlink():
                    raise CodeSpecTypeScriptError(
                        f"product source census rejects symlink {candidate.relative_to(root).as_posix()}"
                    )
                if name not in excluded_directories:
                    kept_directories.append(name)
            directories[:] = kept_directories
            for name in files:
                candidate = current_path / name
                if candidate.is_symlink():
                    raise CodeSpecTypeScriptError(
                        f"product source census rejects symlink {candidate.relative_to(root).as_posix()}"
                    )
                suffix = candidate.suffix.lower()
                if suffix not in {".ts", ".js", ".mjs"}:
                    continue
                lower_name = name.lower()
                if lower_name.endswith(".d.ts") or any(
                    lower_name.endswith(marker + suffix) for marker in (".test", ".spec")
                ):
                    continue
                relative = candidate.relative_to(root).as_posix()
                module_path = relative.rsplit(".", 1)[0].replace("-", "_")
                sources.append(
                    {
                        "path": relative,
                        "module": module_path.replace("/", "::"),
                        "source_kind": "typescript" if suffix == ".ts" else "javascript",
                        "scan_mode": "asset" if relative in ASSET_SOURCE_MODES else "ast",
                    }
                )
                root_count += 1
        if root_count == 0:
            raise CodeSpecTypeScriptError(
                f"TypeScript/JavaScript product root has zero source files: {product_root}"
            )
    for relative in PACKAGING_INPUTS:
        candidate = root / relative
        if candidate.is_symlink():
            raise CodeSpecTypeScriptError(
                f"packaging input census rejects symlink {relative}"
            )
        if not candidate.is_file():
            raise CodeSpecTypeScriptError(f"missing TypeScript packaging input {relative}")
        module_path = relative.rsplit(".", 1)[0].replace("-", "_")
        sources.append(
            {
                "path": relative,
                "module": module_path.replace("/", "::"),
                "source_kind": "typescript",
                "scan_mode": "asset",
            }
        )
    sources.sort(key=lambda record: (record["path"], record["module"]))
    return sources


def invoke_typescript_extractor(
    root: Path,
    matrix: Mapping[str, Any],
    *,
    area_bindings: Mapping[str, str] | None = None,
) -> TypeScriptInventory:
    area_bindings = area_bindings or {}
    requested_sources = discover_product_sources(root)
    request = {
        "schema_version": 1,
        "root": str(root),
        "files": requested_sources,
    }
    scanner = Path(__file__).with_name("code_spec_ts_scan.mjs")
    completed = subprocess.run(
        ["node", str(scanner)],
        input=json.dumps(request),
        text=True,
        capture_output=True,
        check=False,
    )
    try:
        document = json.loads(completed.stdout)
    except json.JSONDecodeError as error:
        detail = completed.stderr.strip() or completed.stdout.strip() or "no scanner output"
        raise CodeSpecTypeScriptError(f"TypeScript scanner emitted invalid JSON: {detail}") from error
    if set(document) != {"schema_version", "sources", "diagnostics"}:
        raise CodeSpecTypeScriptError("TypeScript scanner root fields drift")
    if document.get("schema_version") != 1:
        raise CodeSpecTypeScriptError("TypeScript scanner schema_version must equal 1")
    diagnostics = document.get("diagnostics")
    if not isinstance(diagnostics, list) or any(not isinstance(item, str) for item in diagnostics):
        raise CodeSpecTypeScriptError("TypeScript scanner diagnostics must be an array of strings")
    if diagnostics:
        raise CodeSpecTypeScriptError(
            "; ".join(f"extractor diagnostic: {diagnostic}" for diagnostic in diagnostics)
        )
    if completed.returncode != 0:
        raise CodeSpecTypeScriptError(
            f"TypeScript scanner exited {completed.returncode} without a diagnostic"
        )
    sources = document.get("sources")
    if not isinstance(sources, list) or any(not isinstance(item, Mapping) for item in sources):
        raise CodeSpecTypeScriptError("TypeScript scanner sources must be an array of objects")
    requested_keys = {
        (record["path"], record["module"], record["source_kind"], record["scan_mode"])
        for record in requested_sources
    }
    emitted_keys = {
        (
            record.get("path"),
            record.get("module"),
            record.get("source_kind"),
            record.get("scan_mode"),
        )
        for record in sources
    }
    if emitted_keys != requested_keys or len(sources) != len(requested_sources):
        raise CodeSpecTypeScriptError("TypeScript scanner source census does not match requested files")

    facts: dict[str, CodeFact] = {}
    source_counts = {product_root: 0 for product_root in PRODUCT_ROOTS}
    source_counts["packaging_inputs"] = 0
    for source_index, source_record in enumerate(sources):
        label = f"sources[{source_index}]"
        if set(source_record) != {"path", "module", "source_kind", "scan_mode", "facts"}:
            raise CodeSpecTypeScriptError(f"{label} fields drift")
        raw_path = _string(source_record.get("path"), f"{label}.path")
        try:
            path, source_bytes = governed_source_bytes(root, raw_path)
            candidates = candidate_area_owners(matrix, path)
        except CodeSpecSourceError as error:
            raise CodeSpecTypeScriptError(str(error)) from error
        source_kind = _string(source_record.get("source_kind"), f"{label}.source_kind")
        if source_kind not in {"typescript", "javascript"}:
            raise CodeSpecTypeScriptError(f"{label}.source_kind is not TypeScript or JavaScript")
        scan_mode = _string(source_record.get("scan_mode"), f"{label}.scan_mode")
        expected_scan_mode = (
            "asset" if path in ASSET_SOURCE_MODES or path in PACKAGING_INPUTS else "ast"
        )
        if scan_mode != expected_scan_mode:
            raise CodeSpecTypeScriptError(
                f"{label}.scan_mode {scan_mode!r} does not match reviewed mode {expected_scan_mode!r}"
            )
        module = _string(source_record.get("module"), f"{label}.module")
        source_facts = source_record.get("facts")
        if not isinstance(source_facts, list) or any(not isinstance(item, Mapping) for item in source_facts):
            raise CodeSpecTypeScriptError(f"{label}.facts must be an array of objects")
        for product_root in PRODUCT_ROOTS:
            if path == product_root or path.startswith(product_root + "/"):
                source_counts[product_root] += 1
                break
        else:
            if path in PACKAGING_INPUTS:
                source_counts["packaging_inputs"] += 1
        for fact_index, record in enumerate(source_facts):
            fact_label = f"{label}.facts[{fact_index}]"
            expected = {
                "qualified_symbol",
                "item_kind",
                "visibility",
                "signature",
                "body_start",
                "body_end",
                "line",
                "risk_signals",
            }
            if set(record) != expected:
                raise CodeSpecTypeScriptError(f"{fact_label} fields drift")
            symbol = _string(record.get("qualified_symbol"), f"{fact_label}.qualified_symbol")
            item_kind = _string(record.get("item_kind"), f"{fact_label}.item_kind")
            signature = _string(record.get("signature"), f"{fact_label}.signature")
            visibility = _string(record.get("visibility"), f"{fact_label}.visibility")
            body_start = _integer(record.get("body_start"), f"{fact_label}.body_start")
            body_end = _integer(record.get("body_end"), f"{fact_label}.body_end")
            line = _integer(record.get("line"), f"{fact_label}.line")
            raw_risks = record.get("risk_signals")
            if not isinstance(raw_risks, list) or any(
                not isinstance(signal, str) or not signal for signal in raw_risks
            ):
                raise CodeSpecTypeScriptError(f"{fact_label}.risk_signals must be an array of strings")
            risk_signals = tuple(sorted(set(raw_risks)))
            unknown_risks = sorted(set(risk_signals) - RISK_SIGNALS)
            if unknown_risks:
                raise CodeSpecTypeScriptError(
                    f"{fact_label}.risk_signals contains unknown values {unknown_risks}"
                )
            if body_start < 0 or body_end < body_start or body_end > len(source_bytes):
                raise CodeSpecTypeScriptError(
                    f"{fact_label} has invalid body byte range {body_start}..{body_end}"
                )
            if line < 1:
                raise CodeSpecTypeScriptError(f"{fact_label}.line must be positive")
            code_id = stable_code_id(
                source_kind=source_kind,
                path=path,
                qualified_symbol=symbol,
                item_kind=item_kind,
                signature=signature,
            )
            if code_id in facts:
                raise CodeSpecTypeScriptError(f"duplicate live code identity {code_id}")
            try:
                area, owner = select_area_owner(code_id, candidates, area_bindings)
            except CodeSpecSourceError as error:
                raise CodeSpecTypeScriptError(str(error)) from error
            facts[code_id] = CodeFact(
                code_id=code_id,
                source_kind=source_kind,
                path=path,
                qualified_symbol=symbol,
                item_kind=item_kind,
                signature=signature,
                signature_digest=sha256_digest(signature.encode("utf-8")),
                body_digest=sha256_digest(source_bytes[body_start:body_end]),
                owner=owner,
                area=area,
                area_candidates=tuple(sorted(candidates)),
                visibility=visibility,
                risk_signals=risk_signals,
                line=line,
            )
    if not facts:
        raise CodeSpecTypeScriptError("TypeScript/JavaScript parser returned zero product facts")
    zero_fact_roots = [
        product_root
        for product_root in PRODUCT_ROOTS
        if not any(
            fact.path == product_root or fact.path.startswith(product_root + "/")
            for fact in facts.values()
        )
    ]
    if zero_fact_roots:
        raise CodeSpecTypeScriptError(
            f"TypeScript/JavaScript product roots have zero facts: {zero_fact_roots}"
        )
    packaging_fact_paths = {
        fact.path for fact in facts.values() if fact.path in PACKAGING_INPUTS
    }
    missing_packaging_facts = sorted(set(PACKAGING_INPUTS) - packaging_fact_paths)
    if missing_packaging_facts:
        raise CodeSpecTypeScriptError(
            f"TypeScript packaging inputs have zero facts: {missing_packaging_facts}"
        )
    invented_bindings = sorted(set(area_bindings) - set(facts))
    # Bindings for other language denominators are expected; reject only TS/JS-looking
    # identities cannot be determined from the hash, so the complete live join owns the
    # global invented-identity check.
    del invented_bindings
    return TypeScriptInventory(
        facts=dict(sorted(facts.items())),
        source_paths=tuple(sorted(record["path"] for record in requested_sources)),
        source_count=len(sources),
        source_counts=dict(sorted(source_counts.items())),
        unmapped_code_ids=tuple(
            code_id for code_id, fact in sorted(facts.items()) if fact.area == "unmapped"
        ),
    )


def _string(value: Any, label: str) -> str:
    if not isinstance(value, str) or not value:
        raise CodeSpecTypeScriptError(f"{label} must be a nonempty string")
    return value


def _integer(value: Any, label: str) -> int:
    if not isinstance(value, int) or isinstance(value, bool):
        raise CodeSpecTypeScriptError(f"{label} must be an integer")
    return value
