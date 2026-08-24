"""Changed-file verification gate and durable report renderer."""

from __future__ import annotations

import argparse
import json
import subprocess
import sys
import tomllib
from dataclasses import dataclass
from datetime import date
from pathlib import Path
from typing import Callable

from .focused_test_suite import NON_TEST_SUFFIX_COLLISIONS, TEST_ROOT
from .metadata_validator.constants import ROOT
from .test_catalog_scanner import scan_repository


TEST_PATH_MARKERS = (
    "/tests/",
    "/src/test/",
    "/test/suite/",
    "/conformance/",
    "/fuzz/",
)
TEST_FILE_SUFFIXES = (
    "_test.rs",
    "_tests.rs",
    ".test.ts",
    ".spec.ts",
    "_test.py",
    "_tests.py",
)
DEFAULT_OUTPUT_DIR = Path("target/gate-artifacts/verification")
ADVISORY_COMMANDS = {
    "verification_focused_tests",
    "verification_tooling_exhaustive",
    "verification_metadata_gate",
    "phase16_readiness",
    "verification_governance",
    "plan_tests",
}
SMOKE_TEST_MODULES = (
    "scripts.verification.report_gate_tests",
    "scripts.verification.focused_test_suite_tests",
)


@dataclass(frozen=True)
class CommandResult:
    name: str
    command: list[str]
    exit_code: int
    stdout: str
    stderr: str

    def to_dict(self) -> dict[str, object]:
        return {
            "name": self.name,
            "command": self.command,
            "exit_code": self.exit_code,
            "stdout": self.stdout,
            "stderr": self.stderr,
        }


@dataclass(frozen=True)
class VerificationReport:
    mode: str
    intent: str
    baseline: str | None
    changed_files: list[str]
    commands: list[CommandResult]
    planner_exit_code: int | None
    planner_json: dict[str, object] | None
    uncataloged_tests: list[str]

    def to_json(self) -> str:
        return json.dumps(
            {
                "mode": self.mode,
                "intent": self.intent,
                "baseline": self.baseline,
                "changed_files": self.changed_files,
                "commands": [command.to_dict() for command in self.commands],
                "planner_exit_code": self.planner_exit_code,
                "planner_json": self.planner_json,
                "uncataloged_tests": self.uncataloged_tests,
            },
            indent=2,
            sort_keys=True,
        ) + "\n"


CommandRunner = Callable[[list[str]], CommandResult]


def build_report(
    *,
    root: Path = ROOT,
    changed_files: list[str],
    intent: str,
    baseline: str | None,
    command_runner: CommandRunner | None = None,
    run_planner: bool = True,
    enforcing: bool = False,
    smoke: bool = False,
) -> VerificationReport:
    runner = command_runner or default_runner(root)
    normalized = sorted({normalize_changed_file(path) for path in changed_files if path.strip()})
    if smoke:
        commands = [runner(["python3", "-m", "unittest", *SMOKE_TEST_MODULES])]
        return VerificationReport(
            mode="enforcing" if enforcing else "report-only",
            intent=intent,
            baseline=baseline,
            changed_files=normalized,
            commands=commands,
            planner_exit_code=None,
            planner_json=None,
            uncataloged_tests=find_uncataloged_tests(root=root, changed_files=normalized),
        )

    commands: list[CommandResult] = [
        runner(["python3", "scripts/run_verification_focused_tests.py"]),
        runner(["scripts/verification_metadata_gate.sh"]),
    ]
    readiness_command = [
        "python3",
        "-m",
        "scripts.verification.phase16_readiness",
        *(f"--changed-file={path}" for path in normalized),
    ]
    commands.append(runner(readiness_command))
    governance_command = [
        "python3",
        "-m",
        "scripts.verification.governance",
        "--today",
        date.today().isoformat(),
        *(f"--changed-file={path}" for path in normalized),
    ]
    commands.append(runner(governance_command))
    planner_exit_code: int | None = None
    planner_json: dict[str, object] | None = None
    if run_planner and normalized:
        planner_command = [
            "python3",
            "scripts/plan_tests.py",
            "--intent",
            intent,
            "--changed",
            *normalized,
            "--format",
            "json",
        ]
        if baseline:
            planner_command.extend(["--baseline", baseline])
        planner_result = runner(planner_command)
        commands.append(planner_result)
        planner_exit_code = planner_result.exit_code
        planner_json = parse_planner_json(planner_result.stdout)

    return VerificationReport(
        mode="enforcing" if enforcing else "report-only",
        intent=intent,
        baseline=baseline,
        changed_files=normalized,
        commands=commands,
        planner_exit_code=planner_exit_code,
        planner_json=planner_json,
        uncataloged_tests=find_uncataloged_tests(root=root, changed_files=normalized),
    )


def default_runner(root: Path) -> CommandRunner:
    def run(command: list[str]) -> CommandResult:
        completed = subprocess.run(
            command,
            cwd=root,
            text=True,
            stdout=subprocess.PIPE,
            stderr=subprocess.PIPE,
            check=False,
        )
        return CommandResult(
            command_name(command),
            command,
            completed.returncode,
            completed.stdout,
            completed.stderr,
        )

    return run


def command_name(command: list[str]) -> str:
    if command[:2] == ["python3", "scripts/run_verification_focused_tests.py"]:
        return "verification_tooling_exhaustive"
    if command == ["python3", "-m", "unittest", *SMOKE_TEST_MODULES]:
        return "verification_report_smoke"
    if command[:1] == ["scripts/verification_metadata_gate.sh"]:
        return "verification_metadata_gate"
    if command[:3] == ["python3", "-m", "scripts.verification.phase16_readiness"]:
        return "phase16_readiness"
    if command[:3] == ["python3", "-m", "scripts.verification.governance"]:
        return "verification_governance"
    if len(command) >= 2 and command[1] == "scripts/plan_tests.py":
        return "plan_tests"
    return Path(command[0]).name if command else "unknown"


def changed_files_from_git(root: Path, base: str, head: str) -> list[str]:
    completed = subprocess.run(
        [
            "git",
            "diff",
            "--name-status",
            "-z",
            "--find-renames",
            "--find-copies",
            "--diff-filter=ACDMRTUXB",
            "--merge-base",
            base,
            head,
            "--",
        ],
        cwd=root,
        text=True,
        stdout=subprocess.PIPE,
        stderr=subprocess.PIPE,
        check=False,
    )
    if completed.returncode != 0:
        raise RuntimeError(f"git diff failed: {completed.stderr.strip()}")
    return parse_name_status_z(completed.stdout)


def parse_name_status_z(output: str) -> list[str]:
    tokens = output.split("\0")
    if tokens and tokens[-1] == "":
        tokens.pop()
    paths: set[str] = set()
    index = 0
    while index < len(tokens):
        status = tokens[index]
        index += 1
        path_count = 2 if status[:1] in {"R", "C"} else 1
        if not status or index + path_count > len(tokens):
            raise RuntimeError("git diff returned malformed NUL-delimited name-status output")
        paths.update(tokens[index : index + path_count])
        index += path_count
    return sorted(paths)


def find_uncataloged_tests(*, root: Path, changed_files: list[str]) -> list[str]:
    candidates: set[str] = set()
    for path in changed_files:
        normalized = normalize_changed_file(path)
        relative = Path(normalized)
        if (
            relative not in NON_TEST_SUFFIX_COLLISIONS
            and relative.parent.is_relative_to(TEST_ROOT)
            and relative.name.endswith("_tests.py")
        ):
            continue
        if is_test_like(normalized):
            candidates.add(normalized)
    if not candidates:
        return []

    authorized_ids = load_authorized_discovery_ids(root)
    scan = scan_repository(root)
    facts_by_path: dict[str, set[str]] = {}
    for fact in scan.inferred_facts:
        facts_by_path.setdefault(normalize_changed_file(fact.path), set()).add(
            fact.stable_id
        )
    error_paths = {
        normalize_changed_file(item.path)
        for item in scan.diagnostics
        if item.severity == "error"
    }
    return sorted(
        path
        for path in candidates
        if path in error_paths
        or bool(facts_by_path.get(path, set()) - authorized_ids)
    )


def load_authorized_discovery_ids(root: Path) -> set[str]:
    authorized: set[str] = set()
    for path, collection in (
        (root / "verification/test-catalog.toml", "tests"),
        (root / "verification/test-catalog-denominator.toml", "reviews"),
    ):
        if not path.exists():
            continue
        try:
            data = tomllib.loads(path.read_text(encoding="utf-8"))
        except (OSError, UnicodeError, tomllib.TOMLDecodeError):
            # Catalog maintenance is advisory. A malformed metadata file must
            # not prevent the report or native product gates from running.
            continue
        records = data.get(collection, [])
        if not isinstance(records, list):
            continue
        for record in records:
            discovery_id = record.get("discovery_id") if isinstance(record, dict) else None
            if isinstance(discovery_id, str):
                authorized.add(discovery_id)
    return authorized


def is_test_like(path: str) -> bool:
    normalized = normalize_changed_file(path)
    if normalized.startswith(("target/", ".git/")):
        return False
    marker_path = f"/{normalized}"
    return any(marker in marker_path for marker in TEST_PATH_MARKERS) or normalized.endswith(TEST_FILE_SUFFIXES)


def parse_planner_json(stdout: str) -> dict[str, object] | None:
    if not stdout.strip():
        return None
    try:
        parsed = json.loads(stdout)
    except json.JSONDecodeError:
        return None
    return parsed if isinstance(parsed, dict) else None


def report_exit_code(report: VerificationReport, *, strict: bool) -> int:
    if not strict:
        return 0
    for command in report.commands:
        if command.exit_code != 0 and command.name not in ADVISORY_COMMANDS:
            return 1
    return 0


def render_markdown(report: VerificationReport) -> str:
    lines = [
        "# Verification Gate Report",
        "",
        f"Mode: `{report.mode}`",
        f"Intent: `{report.intent}`",
    ]
    if report.baseline:
        lines.append(f"Baseline: `{report.baseline}`")
    lines.extend(["", "Changed files:"])
    if report.changed_files:
        lines.extend(f"- `{path}`" for path in report.changed_files)
    else:
        lines.append("- none")

    lines.extend(["", "Commands:"])
    for command in report.commands:
        lines.append(f"- `{command.name}` exit `{command.exit_code}`")
    if report.planner_exit_code is not None:
        lines.extend(["", f"Planner exit: `{report.planner_exit_code}`"])
    if report.planner_json:
        verdict = report.planner_json.get("verdict", "<unknown>")
        lines.append(f"Planner verdict: `{verdict}`")
        if report.planner_exit_code not in (None, 0):
            lines.append("Planner finding: advisory maintenance information")

    lines.extend(["", "Uncataloged changed tests:"])
    if report.uncataloged_tests:
        lines.extend(f"- `{path}`" for path in report.uncataloged_tests)
    else:
        lines.append("- none")
    lines.append("")
    if report.mode == "enforcing":
        lines.extend(
            [
                "Strict mode remains fail-closed for any non-advisory command added to this report.",
                "The recursive verification-tooling and metadata commands are advisory maintenance; native product tests are enforced by their owning CI jobs.",
                "Planner and catalog observations are advisory and do not block merge.",
            ]
        )
    else:
        lines.extend(
            [
                "This gate is report-only during the bytecode/VM pilot burn-in.",
                "Findings are review inputs and do not enforce outside the pilot yet.",
            ]
        )
    return "\n".join(lines) + "\n"


def write_reports(report: VerificationReport, output_dir: Path) -> None:
    output_dir.mkdir(parents=True, exist_ok=True)
    (output_dir / "verification-gate-report.json").write_text(report.to_json())
    (output_dir / "verification-gate-report.md").write_text(render_markdown(report))


def normalize_changed_file(value: str) -> str:
    normalized = value.strip()
    while normalized.startswith("./"):
        normalized = normalized[2:]
    return normalized.replace("\\", "/")


def parse_args(argv: list[str]) -> argparse.Namespace:
    parser = argparse.ArgumentParser(description="Run the changed-file verification gate.")
    parser.add_argument("--intent", default="bugfix", choices=["bugfix", "feature", "refactor", "docs", "test-refactor"])
    parser.add_argument("--base", help="Base revision for PR/diff reports")
    parser.add_argument("--head", default="HEAD", help="Head revision for PR/diff reports")
    parser.add_argument("--changed", nargs="*", help="Explicit changed-file list")
    parser.add_argument("--out-dir", default=str(DEFAULT_OUTPUT_DIR), help="Report output directory")
    parser.add_argument("--strict", action="store_true", help="Return non-zero on reported failures")
    parser.add_argument(
        "--smoke",
        action="store_true",
        help="Run only report-boundary tests; exhaustive maintenance stays scheduled",
    )
    return parser.parse_args(argv)


def main(argv: list[str] | None = None) -> int:
    args = parse_args(argv or sys.argv[1:])
    try:
        changed_files = args.changed if args.changed is not None else []
        if not changed_files and args.base:
            changed_files = changed_files_from_git(ROOT, args.base, args.head)
        report = build_report(
            root=ROOT,
            changed_files=changed_files,
            intent=args.intent,
            baseline=args.base,
            enforcing=args.strict,
            smoke=args.smoke,
        )
        write_reports(report, ROOT / args.out_dir)
        print(render_markdown(report), end="")
        return report_exit_code(report, strict=args.strict)
    except Exception as exc:
        print(f"verification report gate error: {exc}", file=sys.stderr)
        return 1 if args.strict else 0


if __name__ == "__main__":
    raise SystemExit(main())
