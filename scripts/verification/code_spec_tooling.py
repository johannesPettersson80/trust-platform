"""Phase 17 verification and operational tooling denominator."""

from __future__ import annotations

import ast
import json
import os
import subprocess
from dataclasses import dataclass, field
from pathlib import Path
from typing import Any, Mapping

from .code_spec_common import RISK_SIGNALS, CodeFact, sha256_digest, stable_code_id
from .code_spec_rust import CodeSpecRustError, rust_inventory_from_scan
from .code_spec_source_adapter import (
    CodeSpecSourceError,
    candidate_area_owners,
    governed_source_bytes,
    select_area_owner,
)


SCRIPT_ROOTS = ("scripts", ".claude/hooks")
DECLARATIVE_EDITOR_EXCLUSIONS = frozenset(
    {
        "editors/neovim/README.md",
        "editors/neovim/lspconfig.lua",
        "editors/zed/README.md",
        "editors/zed/settings.json",
    }
)


class CodeSpecToolingError(ValueError):
    """Raised when the tooling denominator is incomplete or ambiguous."""


@dataclass(frozen=True)
class ToolingInventory:
    facts: Mapping[str, CodeFact] = field(default_factory=dict)
    source_paths: tuple[str, ...] = ()
    source_counts: Mapping[str, int] = field(default_factory=dict)
    rust_target_count: int = 0
    unmapped_code_ids: tuple[str, ...] = ()


def extract_python_source(module: str, source: bytes) -> list[dict[str, Any]]:
    """Extract behavior-bearing Python facts from exact UTF-8 source bytes."""

    text = source.decode("utf-8")
    tree = ast.parse(text)
    line_starts = _line_starts(source)
    facts: list[dict[str, Any]] = []

    def add(node: ast.AST, context: tuple[str, ...], kind: str, signature: str) -> None:
        if not hasattr(node, "end_lineno") or node.end_lineno is None:
            raise ValueError("Python AST node has no exact end position")
        start = line_starts[node.lineno - 1] + node.col_offset  # type: ignore[attr-defined]
        end = line_starts[node.end_lineno - 1] + node.end_col_offset  # type: ignore[attr-defined]
        name = context[-1]
        facts.append(
            {
                "qualified_symbol": "::".join(context),
                "item_kind": kind,
                "visibility": "private" if name.startswith("_") else "public",
                "signature": " ".join(signature.split()),
                "body_start": start,
                "body_end": end,
                "line": node.lineno,  # type: ignore[attr-defined]
                "risk_signals": _python_risk_signals(node),
            }
        )

    class Visitor(ast.NodeVisitor):
        def __init__(self) -> None:
            self.context = (module,)
            self.class_depth = 0

        def visit_ClassDef(self, node: ast.ClassDef) -> None:  # noqa: N802
            previous = self.context
            self.context = (*self.context, node.name)
            self.class_depth += 1
            self.generic_visit(node)
            self.class_depth -= 1
            self.context = previous

        def _visit_function(
            self,
            node: ast.FunctionDef | ast.AsyncFunctionDef,
            *,
            asynchronous: bool,
        ) -> None:
            context = (*self.context, node.name)
            prefix = "async def" if asynchronous else "def"
            returns = f" -> {ast.unparse(node.returns)}" if node.returns is not None else ""
            add(
                node,
                context,
                "python_method" if self.class_depth else "python_function",
                f"{prefix} {node.name}({ast.unparse(node.args)}){returns}",
            )
            previous = self.context
            self.context = context
            self.generic_visit(node)
            self.context = previous

        def visit_FunctionDef(self, node: ast.FunctionDef) -> None:  # noqa: N802
            self._visit_function(node, asynchronous=False)

        def visit_AsyncFunctionDef(self, node: ast.AsyncFunctionDef) -> None:  # noqa: N802
            self._visit_function(node, asynchronous=True)

        def visit_If(self, node: ast.If) -> None:  # noqa: N802
            if self.context == (module,) and _is_main_guard(node.test):
                add(
                    node,
                    (module, "<entrypoint>"),
                    "python_entrypoint",
                    'if __name__ == "__main__"',
                )
            self.generic_visit(node)

    Visitor().visit(tree)
    return sorted(
        facts,
        key=lambda fact: (fact["qualified_symbol"], fact["item_kind"], fact["body_start"]),
    )


def extract_shell_source(module: str, source: bytes) -> list[dict[str, Any]]:
    """Represent one validated shell entrypoint as a file-level fact."""

    source.decode("utf-8")
    return [
        {
            "qualified_symbol": f"{module}::<entrypoint>",
            "item_kind": "shell_entrypoint",
            "visibility": "public",
            "signature": "shell entrypoint",
            "body_start": 0,
            "body_end": len(source),
            "line": 1,
            "risk_signals": [
                "filesystem_or_environment",
                "state_mutation",
                "subprocess_or_thread",
            ],
        }
    ]


def discover_script_sources(root: Path) -> list[dict[str, str]]:
    """Return the exact non-test Python/shell operational source census."""

    directories = [root / relative for relative in SCRIPT_ROOTS]
    skill_root = root / ".codex/skills"
    if not skill_root.is_dir():
        raise CodeSpecToolingError("missing operational skill root .codex/skills")
    directories.extend(sorted(path for path in skill_root.glob("*/scripts") if path.is_dir()))
    sources: dict[str, dict[str, str]] = {}
    root_counts = {relative: 0 for relative in (*SCRIPT_ROOTS, ".codex/skills/**/scripts")}
    for directory in directories:
        if not directory.is_dir():
            relative = directory.relative_to(root).as_posix()
            if relative in SCRIPT_ROOTS:
                raise CodeSpecToolingError(f"missing tooling source root {relative}")
            continue
        count_key = (
            ".codex/skills/**/scripts"
            if directory.as_posix().startswith(skill_root.as_posix() + "/")
            else directory.relative_to(root).as_posix()
        )
        for current, child_directories, files in os.walk(directory, followlinks=False):
            current_path = Path(current)
            child_directories[:] = sorted(
                name
                for name in child_directories
                if name
                not in {
                    "__pycache__",
                    "fixtures",
                    "node_modules",
                    "target",
                    "test",
                    "tests",
                    ".venv",
                    ".venv-docs",
                }
                and not (current_path / name).is_symlink()
            )
            for name in sorted(files):
                candidate = current_path / name
                if candidate.is_symlink():
                    raise CodeSpecToolingError(
                        f"tooling source census rejects symlink {candidate.relative_to(root).as_posix()}"
                    )
                lower = name.lower()
                if lower.endswith(("_test.py", "_tests.py")) or lower.startswith("test_"):
                    continue
                source_kind = _script_source_kind(candidate)
                if source_kind is None:
                    continue
                relative = candidate.relative_to(root).as_posix()
                module = relative.rsplit(".", 1)[0].replace("-", "_").replace("/", "::")
                sources[relative] = {
                    "path": relative,
                    "module": module,
                    "source_kind": source_kind,
                }
                root_counts[count_key] += 1
    zero_roots = sorted(root_name for root_name, count in root_counts.items() if count == 0)
    if zero_roots:
        raise CodeSpecToolingError(f"tooling source roots have zero entrypoints: {zero_roots}")
    return [sources[path] for path in sorted(sources)]


def validate_declarative_editor_exclusions(root: Path) -> None:
    actual = {
        path.relative_to(root).as_posix()
        for editor_root in (root / "editors/neovim", root / "editors/zed")
        for path in editor_root.rglob("*")
        if path.is_file()
    }
    if actual != DECLARATIVE_EDITOR_EXCLUSIONS:
        raise CodeSpecToolingError(
            "declarative Neovim/Zed exclusion census drift: "
            f"missing={sorted(DECLARATIVE_EDITOR_EXCLUSIONS - actual)}, "
            f"extra={sorted(actual - DECLARATIVE_EDITOR_EXCLUSIONS)}"
        )


def invoke_tooling_extractor(
    root: Path,
    matrix: Mapping[str, Any],
    *,
    area_bindings: Mapping[str, str] | None = None,
) -> ToolingInventory:
    """Build the exact Rust/Python/shell verification-tooling inventory."""

    area_bindings = area_bindings or {}
    validate_declarative_editor_exclusions(root)
    completed = subprocess.run(
        ["cargo", "run", "-q", "-p", "xtask", "--", "code-spec-rust-tooling"],
        cwd=root,
        check=False,
        capture_output=True,
        text=True,
    )
    try:
        rust_document = json.loads(completed.stdout)
    except json.JSONDecodeError as error:
        detail = completed.stderr.strip() or completed.stdout.strip() or "no extractor output"
        raise CodeSpecToolingError(f"Rust tooling extractor emitted invalid JSON: {detail}") from error
    try:
        rust = rust_inventory_from_scan(
            root,
            rust_document,
            matrix,
            area_bindings=area_bindings,
            reject_invented_area_bindings=False,
        )
    except CodeSpecRustError as error:
        raise CodeSpecToolingError(str(error)) from error

    facts = dict(rust.facts)
    source_paths = set(rust.source_paths)
    counts = {"rust": rust.source_count, "python": 0, "shell": 0}
    script_sources = discover_script_sources(root)
    for source_record in script_sources:
        raw_path = source_record["path"]
        source_paths.add(raw_path)
        try:
            path, source = governed_source_bytes(root, raw_path)
            candidates = candidate_area_owners(matrix, path)
        except CodeSpecSourceError as error:
            raise CodeSpecToolingError(str(error)) from error
        source_kind = source_record["source_kind"]
        if source_kind == "python":
            try:
                extracted = extract_python_source(source_record["module"], source)
            except (SyntaxError, UnicodeDecodeError, ValueError) as error:
                raise CodeSpecToolingError(f"unsupported Python tooling syntax in {path}: {error}") from error
        else:
            shell = subprocess.run(
                ["bash", "-n", str(root / path)],
                check=False,
                capture_output=True,
                text=True,
            )
            if shell.returncode != 0:
                raise CodeSpecToolingError(
                    f"invalid shell tooling entrypoint {path}: {shell.stderr.strip()}"
                )
            extracted = extract_shell_source(source_record["module"], source)
        counts[source_kind] += 1
        for record in extracted:
            signature = str(record["signature"])
            code_id = stable_code_id(
                source_kind=source_kind,
                path=path,
                qualified_symbol=str(record["qualified_symbol"]),
                item_kind=str(record["item_kind"]),
                signature=signature,
            )
            if code_id in facts:
                raise CodeSpecToolingError(f"duplicate live code identity {code_id}")
            try:
                area, owner = select_area_owner(code_id, candidates, area_bindings)
            except CodeSpecSourceError as error:
                raise CodeSpecToolingError(str(error)) from error
            body_start = int(record["body_start"])
            body_end = int(record["body_end"])
            facts[code_id] = CodeFact(
                code_id=code_id,
                source_kind=source_kind,
                path=path,
                qualified_symbol=str(record["qualified_symbol"]),
                item_kind=str(record["item_kind"]),
                signature=signature,
                signature_digest=sha256_digest(signature.encode("utf-8")),
                body_digest=sha256_digest(source[body_start:body_end]),
                owner=owner,
                area=area,
                visibility=str(record["visibility"]),
                risk_signals=_validated_risks(record, path),
                line=int(record["line"]),
            )
    return ToolingInventory(
        facts=dict(sorted(facts.items())),
        source_paths=tuple(sorted(source_paths)),
        source_counts=dict(sorted(counts.items())),
        rust_target_count=rust.target_count,
        unmapped_code_ids=tuple(
            code_id for code_id, fact in sorted(facts.items()) if fact.area == "unmapped"
        ),
    )


def _line_starts(source: bytes) -> list[int]:
    starts = [0]
    for index, byte in enumerate(source):
        if byte == 0x0A:
            starts.append(index + 1)
    return starts


def _is_main_guard(test: ast.expr) -> bool:
    if not isinstance(test, ast.Compare) or len(test.ops) != 1 or len(test.comparators) != 1:
        return False
    if not isinstance(test.ops[0], ast.Eq):
        return False
    left, right = test.left, test.comparators[0]
    pairs = ((left, right), (right, left))
    return any(
        isinstance(name, ast.Name)
        and name.id == "__name__"
        and isinstance(value, ast.Constant)
        and value.value == "__main__"
        for name, value in pairs
    )


def _script_source_kind(path: Path) -> str | None:
    if path.suffix == ".py":
        return "python"
    if path.suffix == ".sh" or not path.suffix:
        try:
            first_line = path.read_bytes().splitlines()[0]
        except (OSError, IndexError):
            return None
        if b"bash" in first_line or b"sh" in first_line:
            return "shell"
    return None


def _python_risk_signals(root: ast.AST) -> list[str]:
    signals: set[str] = set()

    class RiskVisitor(ast.NodeVisitor):
        def visit_FunctionDef(self, node: ast.FunctionDef) -> None:  # noqa: N802
            if node is root:
                self.generic_visit(node)

        def visit_AsyncFunctionDef(self, node: ast.AsyncFunctionDef) -> None:  # noqa: N802
            if node is root:
                self.generic_visit(node)

        def visit_Lambda(self, node: ast.Lambda) -> None:  # noqa: N802
            if node is root:
                self.generic_visit(node)

        def visit_Call(self, node: ast.Call) -> None:  # noqa: N802
            _add_python_risks(ast.unparse(node), signals)
            self.generic_visit(node)

        def visit_Attribute(self, node: ast.Attribute) -> None:  # noqa: N802
            _add_python_risks(ast.unparse(node), signals)
            self.generic_visit(node)

        def visit_Subscript(self, node: ast.Subscript) -> None:  # noqa: N802
            _add_python_risks(ast.unparse(node), signals)
            self.generic_visit(node)

        def visit_Assign(self, node: ast.Assign) -> None:  # noqa: N802
            if any(isinstance(target, (ast.Attribute, ast.Subscript)) for target in node.targets):
                signals.add("state_mutation")
            self.generic_visit(node)

        def visit_AnnAssign(self, node: ast.AnnAssign) -> None:  # noqa: N802
            if isinstance(node.target, (ast.Attribute, ast.Subscript)):
                signals.add("state_mutation")
            self.generic_visit(node)

        def visit_AugAssign(self, node: ast.AugAssign) -> None:  # noqa: N802
            if isinstance(node.target, (ast.Attribute, ast.Subscript)):
                signals.add("state_mutation")
            self.generic_visit(node)

        def visit_ExceptHandler(self, node: ast.ExceptHandler) -> None:  # noqa: N802
            exception = ast.unparse(node.type) if node.type is not None else ""
            if not exception or exception in {"Exception", "BaseException"}:
                signals.add("broad_error_suppression")
            self.generic_visit(node)

    RiskVisitor().visit(root)
    return sorted(signals)


def _add_python_risks(name: str, signals: set[str]) -> None:
    normalized = name.lower()
    policies = (
        ("host_time", ("time.time", "datetime.now", "datetime.utcnow", "date.today")),
        ("randomness", ("random.", "secrets.", "os.urandom", "uuid4")),
        (
            "filesystem_or_environment",
            ("os.environ", "os.getenv", "pathlib", "open(", "read_text", "write_text", "read_bytes", "write_bytes"),
        ),
        ("network_io", ("socket.", "requests.", "urllib.", "http.client", "websocket")),
        ("subprocess_or_thread", ("subprocess.", "threading.", "multiprocessing.", "asyncio.create_task")),
        ("state_mutation", (".append(", ".extend(", ".insert(", ".pop(", ".remove(", ".update(")),
        ("output_or_safe_state_commit", ("safe_state", "force_output", "release_output", "commit_output")),
        ("authorization", ("authorize", "authorization", "authz", "permission", "role")),
        ("fault_mapping", ("map_error", "fault", "error_response")),
        ("protocol_response_or_status", ("response", "status", "health")),
        ("persistence_retain_reload_restart", ("persist", "retain", "reload", "restart")),
    )
    for signal, needles in policies:
        if any(needle in normalized for needle in needles):
            signals.add(signal)


def _validated_risks(record: Mapping[str, Any], path: str) -> tuple[str, ...]:
    raw = record.get("risk_signals")
    if not isinstance(raw, list) or any(not isinstance(signal, str) for signal in raw):
        raise CodeSpecToolingError(f"tooling fact in {path} has invalid risk_signals")
    risks = tuple(sorted(set(raw)))
    unknown = sorted(set(risks) - RISK_SIGNALS)
    if unknown:
        raise CodeSpecToolingError(f"tooling fact in {path} has unknown risks {unknown}")
    return risks
