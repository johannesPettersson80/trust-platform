"""Validate only Rust code facts referenced by a focused coverage trace."""

from __future__ import annotations

import hashlib
import json
import os
import subprocess
import tomllib
from collections.abc import Mapping, Sequence
from dataclasses import dataclass
from pathlib import Path, PurePosixPath
from typing import Any

from .code_spec_common import (
    CodeSourceSpan,
    sha256_digest,
    stable_code_id,
)
from .focused_mapping_context import canonical_input_file


RUST_FACT_FIELDS = {
    "qualified_symbol",
    "item_kind",
    "visibility",
    "signature",
    "body_start",
    "body_end",
    "line",
    "risk_signals",
}


@dataclass(frozen=True)
class FocusedRustPathInventory:
    """Exact live facts and spans for selected mapped Rust source files."""

    facts: Mapping[str, Mapping[str, Any]]
    spans: Mapping[str, CodeSourceSpan]
    topology_sources: Mapping[str, bytes]


def focused_rust_topology_sources(
    root: Path,
    paths: Sequence[str],
    *,
    workspace_root: Path | None = None,
) -> dict[str, bytes]:
    """Capture authenticated parent/root bytes without extracting code facts."""

    root = root.resolve()
    requested = sorted(set(paths))
    if not requested:
        return {}
    resolved = _resolve_live_rust_topologies(
        extractor_root=workspace_root or root,
        source_root=root,
        paths=requested,
    )
    sources: dict[str, bytes] = {}
    for _modules, topology_sources in resolved.values():
        for path, content in topology_sources.items():
            previous = sources.setdefault(path, content)
            if previous != content:
                raise ValueError(f"focused Rust topology source conflicts at {path}")
    return dict(sorted(sources.items()))


def focused_rust_observed_source_projection(
    root: Path,
    observed_paths: Sequence[str],
    mappings: Mapping[str, Mapping[str, Any]],
) -> tuple[dict[str, bytes], str]:
    """Bind only mapped Rust paths observed in an unfiltered LLVM export."""

    root = root.resolve()
    requested = sorted(set(observed_paths))
    mapped_paths = {
        str(mapping["path"])
        for mapping in mappings.values()
        if mapping.get("source_kind") == "rust"
        and _focused_path_supported(mapping.get("path"))
        and isinstance(mapping.get("path"), str)
    }
    selected_paths = sorted(set(requested) & mapped_paths)
    rust_rows = [
        dict(mapping)
        for mapping in mappings.values()
        if mapping.get("source_kind") == "rust"
        and mapping.get("path") in selected_paths
    ]
    sources = {
        path: canonical_input_file(
            root,
            root / path,
            f"mapped Rust source {path}",
        ).read_bytes()
        for path in selected_paths
    }
    payload = {
        "kind": "focused_rust_observed_source_projection_v1",
        "observed_repository_paths": requested,
        "mappings": sorted(rust_rows, key=lambda row: str(row.get("code_id", ""))),
        "source_digests": {
            path: sha256_digest(content)
            for path, content in sorted(sources.items())
        },
    }
    return sources, _canonical_digest(payload)


def focused_rust_path_inventory(
    root: Path,
    paths: Sequence[str],
    mappings: Mapping[str, Mapping[str, Any]],
    *,
    workspace_root: Path | None = None,
) -> FocusedRustPathInventory:
    """Extract and validate only selected mapped Rust source files."""

    root = root.resolve()
    requested = sorted(set(paths))
    if not requested:
        return FocusedRustPathInventory(facts={}, spans={}, topology_sources={})
    expected_by_path: dict[str, dict[str, Mapping[str, Any]]] = {
        path: {} for path in requested
    }
    for code_id, mapping in mappings.items():
        if (
            mapping.get("source_kind") == "rust"
            and mapping.get("path") in expected_by_path
        ):
            expected_by_path[str(mapping["path"])][code_id] = mapping
    missing_paths = sorted(
        path for path, rows in expected_by_path.items() if not rows
    )
    if missing_paths:
        raise ValueError(
            f"focused Rust paths are absent from code map: {missing_paths}"
        )

    generated: dict[str, dict[str, Any]] = {}
    spans: dict[str, CodeSourceSpan] = {}
    topology_sources: dict[str, bytes] = {}
    resolved_topologies = _resolve_live_rust_topologies(
        extractor_root=workspace_root or root,
        source_root=root,
        paths=requested,
    )
    for path in requested:
        source_path = canonical_input_file(
            root,
            root / path,
            f"covered Rust source {path}",
        )
        modules, path_topology_sources = resolved_topologies[path]
        path_generated: dict[str, dict[str, Any]] = {}
        path_spans: dict[str, CodeSourceSpan] = {}
        source = source_path.read_bytes()
        for module in modules:
            module_facts, module_spans = _materialize_focused_rust_facts(
                document=_invoke_focused_rust_extractor(
                    root=workspace_root or root,
                    source_root=root,
                    path=path,
                    module=module,
                ),
                path=path,
                source=source,
            )
            duplicates = sorted(set(path_generated) & set(module_facts))
            if duplicates:
                raise ValueError(
                    f"{path} live Rust topology overlaps code facts {duplicates}"
                )
            path_generated.update(module_facts)
            path_spans.update(module_spans)
        expected_ids = set(expected_by_path[path])
        if set(path_generated) != expected_ids:
            raise ValueError(
                f"{path} focused Rust code-map identity set drift: "
                f"missing={sorted(expected_ids - set(path_generated))}, "
                f"extra={sorted(set(path_generated) - expected_ids)}"
            )
        duplicates = sorted(set(generated) & set(path_generated))
        if duplicates:
            raise ValueError(
                f"{path} focused Rust extraction duplicates {duplicates}"
            )
        generated.update(path_generated)
        spans.update(path_spans)
        for topology_path, content in path_topology_sources.items():
            previous = topology_sources.setdefault(topology_path, content)
            if previous != content:
                raise ValueError(
                    f"focused Rust topology source conflicts at {topology_path}"
                )

    result: dict[str, Mapping[str, Any]] = {}
    for path in requested:
        for code_id, expected in expected_by_path[path].items():
            actual = generated[code_id]
            for field in (
                "source_kind",
                "path",
                "qualified_symbol",
                "item_kind",
                "signature_digest",
                "reviewed_body_digest",
            ):
                if actual[field] != expected.get(field):
                    raise ValueError(
                        f"{code_id} focused live {field} differs from code map"
                    )
            result[code_id] = {
                "code_id": code_id,
                "area": _required_string(expected, "area", code_id),
                "path": actual["path"],
                "qualified_symbol": actual["qualified_symbol"],
            }
    return FocusedRustPathInventory(
        facts=dict(sorted(result.items())),
        spans=dict(sorted(spans.items())),
        topology_sources=dict(sorted(topology_sources.items())),
    )


def _resolve_live_rust_topologies(
    *,
    extractor_root: Path,
    source_root: Path,
    paths: Sequence[str],
) -> dict[str, tuple[list[str], dict[str, bytes]]]:
    """Resolve selected sources with one live graph scan per Cargo target root."""

    selected = sorted(set(paths))
    requests_by_target: dict[tuple[str, str], set[str]] = {}
    for path in selected:
        for root_path, root_module in _live_rust_target_roots(source_root, path):
            requests_by_target.setdefault((root_path, root_module), set()).add(path)
    binding_chains: dict[str, dict[str, tuple[str, ...]]] = {
        path: {} for path in selected
    }
    diagnostics: dict[str, list[str]] = {path: [] for path in selected}
    for (root_path, root_module), target_paths in sorted(requests_by_target.items()):
        requested_paths = sorted(target_paths)
        document = _invoke_focused_rust_topologies(
            root=extractor_root,
            source_root=source_root,
            root_path=root_path,
            root_module=root_module,
            selected_paths=requested_paths,
        )
        selections = _validate_topology_document(document, requested_paths)
        for selection in selections:
            path = str(selection["selected_path"])
            raw_bindings = selection["bindings"]
            if not raw_bindings:
                diagnostics[path].extend(
                    str(item) for item in selection["diagnostics"]
                )
                continue
            for binding in raw_bindings:
                module = str(binding["module"])
                topology_paths = tuple(
                    str(item) for item in binding["topology_paths"]
                )
                if (
                    binding["path"] != path
                    or not topology_paths
                    or topology_paths[0] != root_path
                    or topology_paths[-1] != path
                ):
                    raise ValueError(
                        f"{path} live Rust topology binding is incoherent"
                    )
                previous = binding_chains[path].setdefault(
                    module, topology_paths
                )
                if previous != topology_paths:
                    raise ValueError(
                        f"{path} has ambiguous live Rust topology for module {module}"
                    )
    resolved = {}
    for path in selected:
        if not binding_chains[path]:
            detail = (
                f": {'; '.join(sorted(set(diagnostics[path])))}"
                if diagnostics[path]
                else ""
            )
            raise ValueError(
                f"{path} is orphaned from every live Rust target{detail}"
            )
        sources = {}
        for topology_path in sorted(
            {
                item
                for chain in binding_chains[path].values()
                for item in chain
            }
        ):
            source = canonical_input_file(
                source_root,
                source_root / topology_path,
                f"focused Rust topology source {topology_path}",
            )
            sources[topology_path] = source.read_bytes()
        resolved[path] = (sorted(binding_chains[path]), sources)
    return resolved


def _materialize_focused_rust_facts(
    *,
    document: Mapping[str, Any],
    path: str,
    source: bytes,
) -> tuple[dict[str, dict[str, Any]], dict[str, CodeSourceSpan]]:
    generated: dict[str, dict[str, Any]] = {}
    spans: dict[str, CodeSourceSpan] = {}
    for fact in _rust_facts(document, path):
        signature = _required_string(fact, "signature", path)
        qualified_symbol = _required_string(fact, "qualified_symbol", path)
        item_kind = _required_string(fact, "item_kind", path)
        body_start = _integer(fact.get("body_start"), f"{path} body_start")
        body_end = _integer(fact.get("body_end"), f"{path} body_end")
        if body_start < 0 or body_end < body_start or body_end > len(source):
            raise ValueError(f"{path} focused Rust body range is invalid")
        code_id = stable_code_id(
            source_kind="rust",
            path=path,
            qualified_symbol=qualified_symbol,
            item_kind=item_kind,
            signature=signature,
        )
        if code_id in generated:
            raise ValueError(f"{path} focused Rust extraction duplicates {code_id}")
        generated[code_id] = {
            "code_id": code_id,
            "source_kind": "rust",
            "path": path,
            "qualified_symbol": qualified_symbol,
            "item_kind": item_kind,
            "signature_digest": sha256_digest(signature.encode()),
            "reviewed_body_digest": sha256_digest(source[body_start:body_end]),
        }
        spans[code_id] = CodeSourceSpan(
            code_id=code_id,
            path=path,
            body_start=body_start,
            body_end=body_end,
        )
    return generated, spans


def validated_live_code_facts(
    root: Path,
    trace: Mapping[str, Any],
    mappings: Mapping[str, Mapping[str, Any]],
    *,
    workspace_root: Path | None = None,
) -> dict[str, Mapping[str, Any]]:
    """Re-extract unique covered Rust files without the universe extractor."""

    root = root.resolve()
    covered = sorted(
        {
            str(code_id)
            for record in trace.get("tests", [])
            if isinstance(record, Mapping)
            for code_id in record.get("covered_code_ids", [])
            if isinstance(code_id, str)
        }
    )
    if not covered:
        return {}
    paths: set[str] = set()
    for code_id in covered:
        mapping = mappings.get(code_id)
        if mapping is None:
            raise ValueError(f"trace covered code ID is absent from code map: {code_id}")
        if mapping.get("source_kind") != "rust":
            raise ValueError(f"focused Rust trace covered non-Rust code fact {code_id}")
        path = _required_string(mapping, "path", code_id)
        _governed_rust_path(path)
        paths.add(path)
    inventory = focused_rust_path_inventory(
        root,
        sorted(paths),
        mappings,
        workspace_root=workspace_root,
    )
    missing = sorted(set(covered) - set(inventory.facts))
    if missing:
        raise ValueError(
            f"covered Rust facts are absent from focused live extraction: {missing}"
        )
    return {
        code_id: inventory.facts[code_id]
        for code_id in covered
    }


def _governed_rust_path(path: str) -> PurePosixPath:
    """Validate the closed path shape without inferring module authority."""

    pure = PurePosixPath(path)
    parts = pure.parts
    if (
        pure.is_absolute()
        or "\\" in path
        or ".." in parts
        or len(parts) < 4
        or parts[0] != "crates"
    ):
        raise ValueError(f"covered Rust path is not a governed crate source: {path}")
    try:
        source_index = parts.index("src")
    except ValueError as error:
        raise ValueError(f"covered Rust path lacks crate src root: {path}") from error
    tail = list(parts[source_index + 1 :])
    if not tail:
        raise ValueError(f"covered Rust path lacks source file: {path}")
    if tail[0] == "bin":
        if len(tail) < 2:
            raise ValueError(f"covered Rust binary path lacks target: {path}")
    if not tail[-1].endswith(".rs"):
        raise ValueError(f"covered Rust path lacks source file: {path}")
    return pure


def _live_rust_target_roots(
    source_root: Path,
    selected_path: str,
) -> list[tuple[str, str]]:
    """Enumerate live Cargo roots that can own a governed crate source."""

    pure = _governed_rust_path(selected_path)
    parts = pure.parts
    crate_root = PurePosixPath(*parts[:2])
    manifest_relative = (crate_root / "Cargo.toml").as_posix()
    manifest_path = canonical_input_file(
        source_root,
        source_root / manifest_relative,
        f"focused Rust manifest {manifest_relative}",
    )
    try:
        manifest = tomllib.loads(manifest_path.read_text())
    except (OSError, UnicodeDecodeError, tomllib.TOMLDecodeError) as error:
        raise ValueError(
            f"focused Rust manifest is unreadable: {manifest_relative}"
        ) from error
    package = manifest.get("package")
    if not isinstance(package, Mapping):
        raise ValueError(f"focused Rust manifest lacks package: {manifest_relative}")
    package_name = package.get("name")
    if not isinstance(package_name, str) or not package_name:
        raise ValueError(
            f"focused Rust manifest lacks package name: {manifest_relative}"
        )
    package_module = package_name.replace("-", "_")
    candidates: dict[str, str] = {}

    def register(relative: PurePosixPath, module: str) -> None:
        value = relative.as_posix()
        candidate = source_root / value
        if not candidate.is_file():
            return
        previous = candidates.setdefault(value, module)
        if previous != module:
            raise ValueError(
                f"focused Rust target root has conflicting modules: {value}"
            )

    raw_lib = manifest.get("lib")
    if raw_lib is not None and not isinstance(raw_lib, Mapping):
        raise ValueError(f"focused Rust manifest lib is malformed: {manifest_relative}")
    lib = raw_lib if isinstance(raw_lib, Mapping) else {}
    lib_path = lib.get("path", "src/lib.rs")
    lib_name = lib.get("name", package_module)
    if not isinstance(lib_path, str) or not isinstance(lib_name, str):
        raise ValueError(f"focused Rust manifest lib is malformed: {manifest_relative}")
    if raw_lib is not None or package.get("autolib", True) is not False:
        register(crate_root / lib_path, lib_name.replace("-", "_"))

    raw_bins = manifest.get("bin", [])
    if not isinstance(raw_bins, list):
        raise ValueError(f"focused Rust manifest bin is malformed: {manifest_relative}")
    for raw_bin in raw_bins:
        if not isinstance(raw_bin, Mapping):
            raise ValueError(
                f"focused Rust manifest bin entry is malformed: {manifest_relative}"
            )
        bin_name = raw_bin.get("name")
        bin_path = raw_bin.get("path")
        if not isinstance(bin_name, str) or not isinstance(bin_path, str):
            raise ValueError(
                f"focused Rust manifest bin entry is incomplete: {manifest_relative}"
            )
        register(crate_root / bin_path, bin_name.replace("-", "_"))

    auto_bins = package.get("autobins", True) is not False
    if auto_bins:
        register(crate_root / "src/main.rs", package_module)
    source_tail = parts[3:]
    if (
        auto_bins
        and source_tail
        and source_tail[0] == "bin"
        and len(source_tail) >= 2
    ):
        target = source_tail[1]
        if target.endswith(".rs"):
            register(
                crate_root / "src/bin" / target,
                PurePosixPath(target).stem.replace("-", "_"),
            )
        else:
            register(
                crate_root / "src/bin" / target / "main.rs",
                target.replace("-", "_"),
            )
    if not candidates:
        raise ValueError(
            f"{selected_path} has no live Cargo target root in {manifest_relative}"
        )
    return sorted(candidates.items())


def _invoke_focused_rust_topologies(
    *,
    root: Path,
    source_root: Path,
    root_path: str,
    root_module: str,
    selected_paths: Sequence[str],
) -> dict[str, Any]:
    environment = dict(os.environ)
    environment["TRUST_VERIFICATION_INPUT_ROOT"] = str(source_root.resolve())
    command = [
        "cargo",
        "run",
        "-q",
        "-p",
        "xtask",
        "--",
        "code-spec-rust",
        "--path",
        root_path,
        "--module",
        root_module,
    ]
    for selected_path in selected_paths:
        command.extend(["--topology-for", selected_path])
    completed = subprocess.run(
        command,
        cwd=root,
        check=False,
        capture_output=True,
        text=True,
        env=environment,
    )
    if completed.returncode != 0:
        raise ValueError(
            "focused Rust topology extractor failed: "
            f"{completed.stderr.strip() or completed.stdout.strip()}"
        )
    try:
        document = json.loads(completed.stdout)
    except json.JSONDecodeError as error:
        raise ValueError("focused Rust topology extractor emitted invalid JSON") from error
    if not isinstance(document, dict):
        raise ValueError("focused Rust topology extractor root must be an object")
    return document


def _validate_topology_document(
    document: Mapping[str, Any],
    selected_paths: Sequence[str],
) -> list[Mapping[str, Any]]:
    label = selected_paths[0] if selected_paths else "empty selection"
    if set(document) != {"schema_version", "selections"}:
        raise ValueError(f"{label} focused Rust topology fields drift")
    if document.get("schema_version") != 1:
        raise ValueError(f"{label} focused Rust topology version drift")
    selections = document.get("selections")
    if not isinstance(selections, list) or any(
        not isinstance(selection, Mapping)
        or set(selection)
        != {"schema_version", "selected_path", "bindings", "diagnostics"}
        for selection in selections
    ):
        raise ValueError(f"{label} focused Rust topology selections malformed")
    indexed = {}
    for selection in selections:
        selected_path = selection.get("selected_path")
        if (
            selection.get("schema_version") != 1
            or not isinstance(selected_path, str)
            or not selected_path
            or selected_path in indexed
        ):
            raise ValueError(f"{label} focused Rust topology selection malformed")
        diagnostics = selection.get("diagnostics")
        bindings = selection.get("bindings")
        if not isinstance(diagnostics, list) or any(
            not isinstance(item, str) for item in diagnostics
        ):
            raise ValueError(
                f"{selected_path} focused Rust topology diagnostics malformed"
            )
        if not isinstance(bindings, list):
            raise ValueError(
                f"{selected_path} focused Rust topology bindings malformed"
            )
        for binding in bindings:
            if not isinstance(binding, Mapping) or set(binding) != {
                "path",
                "module",
                "topology_paths",
            }:
                raise ValueError(
                    f"{selected_path} focused Rust topology binding malformed"
                )
            if (
                not isinstance(binding.get("path"), str)
                or not isinstance(binding.get("module"), str)
                or not binding["module"]
                or not isinstance(binding.get("topology_paths"), list)
                or not binding["topology_paths"]
                or any(
                    not isinstance(item, str)
                    or not item
                    or PurePosixPath(item).is_absolute()
                    or ".." in PurePosixPath(item).parts
                    for item in binding["topology_paths"]
                )
            ):
                raise ValueError(
                    f"{selected_path} focused Rust topology binding malformed"
                )
        indexed[selected_path] = selection
    if set(indexed) != set(selected_paths):
        raise ValueError(f"{label} focused Rust topology selected path drift")
    return [indexed[path] for path in sorted(indexed)]


def _invoke_focused_rust_extractor(
    *,
    root: Path,
    source_root: Path,
    path: str,
    module: str,
) -> dict[str, Any]:
    environment = dict(os.environ)
    environment["TRUST_VERIFICATION_INPUT_ROOT"] = str(source_root.resolve())
    completed = subprocess.run(
        [
            "cargo",
            "run",
            "-q",
            "-p",
            "xtask",
            "--",
            "code-spec-rust",
            "--path",
            path,
            "--module",
            module,
        ],
        cwd=root,
        check=False,
        capture_output=True,
        text=True,
        env=environment,
    )
    if completed.returncode != 0:
        raise ValueError(
            "focused Rust extractor failed: "
            f"{completed.stderr.strip() or completed.stdout.strip()}"
        )
    try:
        document = json.loads(completed.stdout)
    except json.JSONDecodeError as error:
        raise ValueError("focused Rust extractor emitted invalid JSON") from error
    if not isinstance(document, dict):
        raise ValueError("focused Rust extractor root must be an object")
    return document


def _rust_facts(
    document: Mapping[str, Any],
    path: str,
) -> list[Mapping[str, Any]]:
    if set(document) != {"schema_version", "facts", "diagnostics"}:
        raise ValueError(f"{path} focused Rust extractor fields drift")
    if document.get("schema_version") != 1:
        raise ValueError(f"{path} focused Rust extractor version drift")
    diagnostics = document.get("diagnostics")
    if diagnostics != []:
        raise ValueError(f"{path} focused Rust extractor has diagnostics")
    facts = document.get("facts")
    if not isinstance(facts, list) or any(
        not isinstance(fact, Mapping) or set(fact) != RUST_FACT_FIELDS
        for fact in facts
    ):
        raise ValueError(f"{path} focused Rust facts are malformed")
    return facts


def _required_string(
    record: Mapping[str, Any],
    field: str,
    label: str,
) -> str:
    value = record.get(field)
    if not isinstance(value, str) or not value:
        raise ValueError(f"{label} requires non-empty {field}")
    return value


def _integer(value: Any, label: str) -> int:
    if not isinstance(value, int) or isinstance(value, bool):
        raise ValueError(f"{label} must be an integer")
    return value


def _canonical_digest(value: Any) -> str:
    payload = json.dumps(
        value,
        sort_keys=True,
        separators=(",", ":"),
        ensure_ascii=False,
    ).encode()
    return "sha256:" + hashlib.sha256(payload).hexdigest()


def _focused_path_supported(value: Any) -> bool:
    if not isinstance(value, str):
        return False
    try:
        _governed_rust_path(value)
    except ValueError:
        return False
    return True
