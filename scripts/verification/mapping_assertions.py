"""Syntax-aware assertion inventories for semantic mapping review."""

from __future__ import annotations

import hashlib
import json
import os
import re
import subprocess
import tomllib
from collections.abc import Mapping, Sequence
from pathlib import Path
from typing import Any

from .code_spec_common import sha256_digest
from .test_catalog_st import TEST_DECLARATION_RE, sanitize_st


RUST_SOURCE_KINDS = {"rust_integration_test", "rust_unit_test"}
SUPPORTED_SOURCE_KINDS = frozenset(
    {
        *RUST_SOURCE_KINDS,
        "structured_text_test",
        "vscode_test",
        "conformance_case",
    }
)
ROOT_FIELDS = frozenset({"schema_version", "inventories"})
INVENTORY_FIELDS = frozenset(
    {
        "schema_version",
        "path",
        "test_name",
        "source_digest",
        "assertions",
        "unresolved_assertion_helpers",
    }
)
ASSERTION_FIELDS = frozenset(
    {
        "assertion_id",
        "kind",
        "function",
        "line",
        "expression_digest",
        "expression",
    }
)
ASSERTION_ID_RE = re.compile(r"^ASSERT_[A-F0-9]{24}$")
ST_ASSERTION_RE = re.compile(r"\b(ASSERT_[A-Z0-9_]+)\s*\(")


def invoke_assertion_inventories(
    root: Path,
    facts: Sequence[Mapping[str, Any]],
    *,
    environment: Mapping[str, str] | None = None,
    workspace_root: Path | None = None,
) -> dict[str, dict[str, Any]]:
    """Dispatch one mixed proposal batch to syntax-aware source adapters."""

    partitions: dict[str, list[Mapping[str, Any]]] = {
        "rust": [],
        "structured_text_test": [],
        "vscode_test": [],
        "conformance_case": [],
    }
    for fact in facts:
        discovery_id = _required_string(fact, "discovery_id", "test fact")
        source_kind = _required_string(
            fact, "discovery_source_kind", discovery_id
        )
        if source_kind not in SUPPORTED_SOURCE_KINDS:
            raise ValueError(
                f"{discovery_id} assertion adapter does not support {source_kind}"
            )
        key = "rust" if source_kind in RUST_SOURCE_KINDS else source_kind
        partitions[key].append(fact)
    inventories: dict[str, dict[str, Any]] = {}
    adapters = (
        (
            "rust",
            lambda rows: invoke_rust_assertion_inventory(
                root,
                rows,
                environment=environment,
                workspace_root=workspace_root,
            ),
        ),
        ("structured_text_test", lambda rows: invoke_st_assertion_inventory(root, rows)),
        (
            "vscode_test",
            lambda rows: invoke_vscode_assertion_inventory(
                root, rows, environment=environment
            ),
        ),
        (
            "conformance_case",
            lambda rows: invoke_conformance_assertion_inventory(root, rows),
        ),
    )
    for key, adapter in adapters:
        rows = partitions[key]
        if not rows:
            continue
        generated = adapter(rows)
        overlap = sorted(set(inventories) & set(generated))
        if overlap:
            raise ValueError(f"assertion adapters overlap discovery IDs: {overlap}")
        inventories.update(generated)
    return dict(sorted(inventories.items()))


def invoke_st_assertion_inventory(
    root: Path,
    facts: Sequence[Mapping[str, Any]],
) -> dict[str, dict[str, Any]]:
    inventories = []
    for fact in facts:
        path = _required_string(fact, "path", "Structured Text fact")
        name = _required_string(fact, "name", path)
        source_path = _regular_source(root, path)
        source = source_path.read_text()
        sanitized = sanitize_st(source)
        declarations = list(TEST_DECLARATION_RE.finditer(sanitized))
        matches = [match for match in declarations if match.group(2) == name]
        if len(matches) != 1:
            raise ValueError(
                f"{path}#{name} expected one Structured Text declaration, "
                f"got {len(matches)}"
            )
        start = matches[0].start()
        following = [
            match.start() for match in declarations if match.start() > start
        ]
        end = min(following) if following else len(source)
        assertions = []
        recognized_starts: set[int] = set()
        for match in ST_ASSERTION_RE.finditer(sanitized, start, end):
            open_paren = sanitized.find("(", match.start(), match.end() + 1)
            close_paren = _balanced_close(sanitized, open_paren, end)
            expression_end = close_paren + 1
            while (
                expression_end < end
                and source[expression_end] in {" ", "\t", "\r", "\n"}
            ):
                expression_end += 1
            if expression_end < end and source[expression_end] == ";":
                expression_end += 1
            expression = source[match.start() : expression_end].strip()
            line = source.count("\n", 0, match.start()) + 1
            function = match.group(1)
            kind = f"st_call:{function}"
            expression_digest = sha256_digest(expression.encode())
            assertions.append(
                {
                    "assertion_id": _assertion_id(
                        path=path,
                        name=name,
                        function=function,
                        kind=kind,
                        line=line,
                        expression_digest=expression_digest,
                    ),
                    "kind": kind,
                    "function": function,
                    "line": line,
                    "expression_digest": expression_digest,
                    "expression": expression,
                }
            )
            recognized_starts.add(match.start())
        helper_calls = {
            match.group(1)
            for match in re.finditer(
                r"\b([A-Za-z_][A-Za-z0-9_]*ASSERT[A-Za-z0-9_]*)\s*\(",
                sanitized[start:end],
                flags=re.IGNORECASE,
            )
            if start + match.start() not in recognized_starts
        }
        inventories.append(
            {
                "schema_version": 1,
                "path": path,
                "test_name": name,
                "source_digest": sha256_digest(source_path.read_bytes()),
                "assertions": assertions,
                "unresolved_assertion_helpers": sorted(helper_calls),
            }
        )
    return validate_assertion_batch(
        root,
        {"schema_version": 1, "inventories": inventories},
        facts,
    )


def invoke_vscode_assertion_inventory(
    root: Path,
    facts: Sequence[Mapping[str, Any]],
    *,
    environment: Mapping[str, str] | None = None,
) -> dict[str, dict[str, Any]]:
    request = {
        "root": str(root.resolve()),
        "tests": [
            {
                "path": _required_string(fact, "path", "VS Code fact"),
                "name": _required_string(fact, "name", "VS Code fact"),
            }
            for fact in facts
        ],
    }
    completed = subprocess.run(
        [
            "node",
            str(Path(__file__).with_name("test_assertions_ts_scan.mjs")),
        ],
        cwd=root,
        input=json.dumps(request, sort_keys=True),
        capture_output=True,
        text=True,
        check=False,
        env=dict(environment or os.environ),
    )
    if completed.returncode != 0:
        raise ValueError(
            "VS Code assertion extraction failed: "
            f"{completed.stderr.strip() or completed.stdout.strip()}"
        )
    try:
        document = json.loads(completed.stdout)
    except json.JSONDecodeError as error:
        raise ValueError("VS Code assertion extractor emitted invalid JSON") from error
    return validate_assertion_batch(root, document, facts)


def invoke_conformance_assertion_inventory(
    root: Path,
    facts: Sequence[Mapping[str, Any]],
) -> dict[str, dict[str, Any]]:
    inventories = []
    for fact in facts:
        path = _required_string(fact, "path", "conformance fact")
        name = _required_string(fact, "name", path)
        source_path = _regular_source(root, path)
        manifest = tomllib.loads(source_path.read_text())
        if manifest.get("id") != name:
            raise ValueError(f"{path} conformance ID drifted")
        category = manifest.get("category")
        if not isinstance(category, str) or not category:
            raise ValueError(f"{path} lacks conformance category")
        expected_path = root / "conformance/expected" / category / f"{name}.json"
        if not expected_path.is_file() or expected_path.is_symlink():
            raise ValueError(f"{path} lacks conformance expected oracle")
        expected = json.loads(expected_path.read_text())
        if not isinstance(expected, Mapping) or expected.get("case_id") != name:
            raise ValueError(f"{expected_path} expected oracle identity drifted")
        expression = json.dumps(
            expected,
            sort_keys=True,
            separators=(",", ":"),
            ensure_ascii=False,
        )
        expression_digest = sha256_digest(expression.encode())
        kind = "golden:conformance_expected"
        function = "conformance_expected"
        inventories.append(
            {
                "schema_version": 1,
                "path": path,
                "test_name": name,
                "source_digest": sha256_digest(source_path.read_bytes()),
                "assertions": [
                    {
                        "assertion_id": _assertion_id(
                            path=path,
                            name=name,
                            function=function,
                            kind=kind,
                            line=1,
                            expression_digest=expression_digest,
                        ),
                        "kind": kind,
                        "function": function,
                        "line": 1,
                        "expression_digest": expression_digest,
                        "expression": expression,
                    }
                ],
                "unresolved_assertion_helpers": [],
            }
        )
    return validate_assertion_batch(
        root,
        {"schema_version": 1, "inventories": inventories},
        facts,
    )


def invoke_rust_assertion_inventory(
    root: Path,
    facts: Sequence[Mapping[str, Any]],
    *,
    environment: Mapping[str, str] | None = None,
    workspace_root: Path | None = None,
) -> dict[str, dict[str, Any]]:
    """Extract all requested Rust tests in one xtask process."""

    requested = []
    for fact in facts:
        discovery_id = _required_string(fact, "discovery_id", "test fact")
        source_kind = _required_string(
            fact, "discovery_source_kind", discovery_id
        )
        if source_kind not in RUST_SOURCE_KINDS:
            raise ValueError(
                f"{discovery_id} assertion adapter does not support {source_kind}"
            )
        requested.append(
            {
                "discovery_id": discovery_id,
                "path": _required_string(fact, "path", discovery_id),
                "name": _required_string(fact, "name", discovery_id),
            }
        )
    request_document = {
        "schema_version": 1,
        "tests": [
            {"path": row["path"], "name": row["name"]}
            for row in sorted(
                requested,
                key=lambda row: (row["path"], row["name"], row["discovery_id"]),
            )
        ],
    }
    invocation_environment = dict(environment or os.environ)
    invocation_environment["TRUST_VERIFICATION_INPUT_ROOT"] = str(
        root.resolve()
    )
    completed = subprocess.run(
        [
            "cargo",
            "run",
            "-q",
            "-p",
            "xtask",
            "--",
            "test-assertions-batch",
        ],
        cwd=workspace_root or root,
        input=json.dumps(request_document, sort_keys=True),
        capture_output=True,
        text=True,
        check=False,
        env=invocation_environment,
    )
    if completed.returncode != 0:
        detail = completed.stderr.strip() or completed.stdout.strip()
        raise ValueError(f"Rust assertion extraction failed: {detail}")
    try:
        document = json.loads(completed.stdout)
    except json.JSONDecodeError as error:
        raise ValueError("Rust assertion extractor emitted invalid JSON") from error
    return validate_assertion_batch(root, document, requested)


def validate_assertion_batch(
    root: Path,
    document: Mapping[str, Any],
    requested: Sequence[Mapping[str, Any]],
) -> dict[str, dict[str, Any]]:
    """Validate closed extractor output and bind it to discovery identities."""

    _exact_fields(document, ROOT_FIELDS, "assertion batch")
    if document.get("schema_version") != 1:
        raise ValueError("assertion batch schema_version must equal 1")
    inventories = _object_array(document.get("inventories"), "inventories")
    by_key: dict[tuple[str, str], Mapping[str, Any]] = {}
    for index, inventory in enumerate(inventories):
        label = f"inventories[{index}]"
        _exact_fields(inventory, INVENTORY_FIELDS, label)
        if inventory.get("schema_version") != 1:
            raise ValueError(f"{label} schema_version must equal 1")
        path = _required_string(inventory, "path", label)
        name = _required_string(inventory, "test_name", label)
        key = (path, name)
        if key in by_key:
            raise ValueError(f"assertion batch duplicates {path}#{name}")
        by_key[key] = inventory
    expected_keys = {
        (
            _required_string(row, "path", "requested test"),
            _required_string(row, "name", "requested test"),
        )
        for row in requested
    }
    if set(by_key) != expected_keys:
        raise ValueError(
            "assertion batch does not match requests: "
            f"missing={sorted(expected_keys - set(by_key))}, "
            f"extra={sorted(set(by_key) - expected_keys)}"
        )
    discovery_by_key = {
        (
            _required_string(row, "path", "requested test"),
            _required_string(row, "name", "requested test"),
        ): _required_string(row, "discovery_id", "requested test")
        for row in requested
    }
    output: dict[str, dict[str, Any]] = {}
    for key in sorted(by_key):
        inventory = by_key[key]
        path, name = key
        source_path = (root.resolve() / path).resolve()
        try:
            source_path.relative_to(root.resolve())
        except ValueError as error:
            raise ValueError(f"assertion source escapes repository: {path}") from error
        if not source_path.is_file() or source_path.is_symlink():
            raise ValueError(f"assertion source must be a regular file: {path}")
        expected_source_digest = sha256_digest(source_path.read_bytes())
        if inventory.get("source_digest") != expected_source_digest:
            raise ValueError(f"assertion source digest is stale: {path}")
        assertions = _object_array(
            inventory.get("assertions"), f"{path}#{name} assertions"
        )
        validated_assertions = []
        seen_assertion_ids: set[str] = set()
        for index, assertion in enumerate(assertions):
            label = f"{path}#{name} assertions[{index}]"
            _exact_fields(assertion, ASSERTION_FIELDS, label)
            assertion_id = _required_string(assertion, "assertion_id", label)
            kind = _required_string(assertion, "kind", label)
            function = _required_string(assertion, "function", label)
            expression = _required_string(assertion, "expression", label)
            expression_digest = _required_string(
                assertion, "expression_digest", label
            )
            line = assertion.get("line")
            if not isinstance(line, int) or isinstance(line, bool) or line < 1:
                raise ValueError(f"{label} line must be positive")
            if expression_digest != sha256_digest(expression.encode()):
                raise ValueError(f"{label} expression digest is stale")
            expected_id = _assertion_id(
                path=path,
                name=name,
                function=function,
                kind=kind,
                line=line,
                expression_digest=expression_digest,
            )
            if assertion_id != expected_id or not ASSERTION_ID_RE.fullmatch(
                assertion_id
            ):
                raise ValueError(f"{label} assertion ID is not canonical")
            if assertion_id in seen_assertion_ids:
                raise ValueError(f"{label} duplicates assertion ID {assertion_id}")
            seen_assertion_ids.add(assertion_id)
            validated_assertions.append(dict(assertion))
        helpers = _string_array(
            inventory.get("unresolved_assertion_helpers"),
            f"{path}#{name} unresolved assertion helpers",
        )
        if helpers != sorted(set(helpers)):
            raise ValueError(
                f"{path}#{name} unresolved assertion helpers must be sorted and unique"
            )
        canonical_inventory = {
            "schema_version": 1,
            "path": path,
            "test_name": name,
            "source_digest": expected_source_digest,
            "assertions": validated_assertions,
            "unresolved_assertion_helpers": helpers,
        }
        canonical_inventory["inventory_digest"] = _canonical_digest(
            canonical_inventory
        )
        output[discovery_by_key[key]] = canonical_inventory
    return dict(sorted(output.items()))


def validate_proposal_assertion_inventories(
    *,
    proposals: Sequence[Mapping[str, Any]],
    inventories: Mapping[str, Mapping[str, Any]],
) -> list[str]:
    """Require proposal assertion data to equal a fresh adapter inventory."""

    failures: list[str] = []
    seen: set[str] = set()
    for index, proposal in enumerate(proposals):
        proposal_id = proposal.get("id")
        label = (
            str(proposal_id)
            if isinstance(proposal_id, str) and proposal_id
            else f"proposal[{index}]"
        )
        discovery_id = proposal.get("discovery_id")
        if not isinstance(discovery_id, str) or not discovery_id:
            failures.append(f"{label} lacks discovery identity")
            continue
        if discovery_id in seen:
            failures.append(
                f"{label} duplicates assertion validation for {discovery_id}"
            )
            continue
        seen.add(discovery_id)
        inventory = inventories.get(discovery_id)
        if inventory is None:
            failures.append(f"{label} lacks fresh assertion inventory")
            continue
        for proposal_field, inventory_field in (
            ("test_source_digest", "source_digest"),
            ("assertion_inventory_digest", "inventory_digest"),
            ("assertions", "assertions"),
            ("unresolved_assertion_helpers", "unresolved_assertion_helpers"),
        ):
            if proposal.get(proposal_field) != inventory.get(inventory_field):
                human_field = proposal_field.replace("_", " ")
                verb = "do" if proposal_field == "assertions" else "does"
                failures.append(
                    f"{label} {human_field} {verb} not match fresh inventory"
                )
    extra = sorted(set(inventories) - seen)
    if extra:
        failures.append(
            "fresh assertion inventories contain unselected discovery IDs: "
            + ", ".join(extra)
        )
    return sorted(set(failures))


def _assertion_id(
    *,
    path: str,
    name: str,
    function: str,
    kind: str,
    line: int,
    expression_digest: str,
) -> str:
    identity = (
        f"{path}\0{name}\0{function}\0{kind}\0{line}\0{expression_digest}"
    ).encode()
    return "ASSERT_" + hashlib.sha256(identity).hexdigest()[:24].upper()


def _regular_source(root: Path, relative: str) -> Path:
    source = (root.resolve() / relative).resolve()
    try:
        source.relative_to(root.resolve())
    except ValueError as error:
        raise ValueError(f"assertion source escapes repository: {relative}") from error
    if not source.is_file() or source.is_symlink():
        raise ValueError(
            f"assertion source must be a regular file: {relative}"
        )
    return source


def _balanced_close(source: str, open_paren: int, limit: int) -> int:
    if open_paren < 0 or open_paren >= limit or source[open_paren] != "(":
        raise ValueError("Structured Text assertion has no opening parenthesis")
    depth = 0
    for index in range(open_paren, limit):
        char = source[index]
        if char == "(":
            depth += 1
        elif char == ")":
            depth -= 1
            if depth == 0:
                return index
    raise ValueError("Structured Text assertion has unbalanced parentheses")


def _canonical_digest(value: Mapping[str, Any]) -> str:
    encoded = json.dumps(
        value,
        sort_keys=True,
        separators=(",", ":"),
        ensure_ascii=False,
    ).encode()
    return sha256_digest(encoded)


def _exact_fields(
    record: Mapping[str, Any], expected: frozenset[str], label: str
) -> None:
    if set(record) != expected:
        raise ValueError(
            f"{label} fields drift: "
            f"missing={sorted(expected - set(record))}, "
            f"extra={sorted(set(record) - expected)}"
        )


def _object_array(value: Any, label: str) -> list[Mapping[str, Any]]:
    if (
        not isinstance(value, Sequence)
        or isinstance(value, (str, bytes))
        or any(not isinstance(item, Mapping) for item in value)
    ):
        raise ValueError(f"{label} must be an object array")
    return list(value)


def _string_array(value: Any, label: str) -> list[str]:
    if (
        not isinstance(value, Sequence)
        or isinstance(value, (str, bytes))
        or any(not isinstance(item, str) or not item for item in value)
    ):
        raise ValueError(f"{label} must be a non-empty-string array")
    return list(value)


def _required_string(
    record: Mapping[str, Any], field: str, label: str
) -> str:
    value = record.get(field)
    if not isinstance(value, str) or not value:
        raise ValueError(f"{label} requires non-empty {field}")
    return value
