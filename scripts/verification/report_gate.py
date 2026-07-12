"""Report-only CI gate for the verification pilot."""

from __future__ import annotations

import argparse
import json
import subprocess
import sys
import tomllib
from dataclasses import dataclass
from pathlib import Path
from typing import Callable

from .metadata_validator.constants import ROOT


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
) -> VerificationReport:
    runner = command_runner or default_runner(root)
    normalized = sorted({normalize_changed_file(path) for path in changed_files if path.strip()})
    commands: list[CommandResult] = [
        runner(["scripts/verification_metadata_gate.sh"]),
    ]
    readiness_command = [
        "python3",
        "-m",
        "scripts.verification.phase16_readiness",
        *(f"--changed-file={path}" for path in normalized),
    ]
    commands.append(runner(readiness_command))
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
        mode="report-only",
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
    if command[:1] == ["scripts/verification_metadata_gate.sh"]:
        return "verification_metadata_gate"
    if command[:3] == ["python3", "-m", "scripts.verification.phase16_readiness"]:
        return "phase16_readiness"
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
    catalog_paths = load_catalog_paths(root)
    missing: list[str] = []
    for path in changed_files:
        normalized = normalize_changed_file(path)
        if is_test_like(normalized) and normalized not in catalog_paths:
            missing.append(normalized)
    return sorted(set(missing))


def load_catalog_paths(root: Path) -> set[str]:
    catalog = root / "verification/test-catalog.toml"
    if not catalog.exists():
        return set()
    data = tomllib.loads(catalog.read_text())
    records = data.get("tests", [])
    if not isinstance(records, list):
        return set()
    paths: set[str] = set()
    for record in records:
        if isinstance(record, dict) and isinstance(record.get("path"), str):
            paths.add(normalize_changed_file(record["path"]))
    return paths


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
    if any(command.exit_code != 0 for command in report.commands):
        return 1
    if report.uncataloged_tests:
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

    lines.extend(["", "Uncataloged changed tests:"])
    if report.uncataloged_tests:
        lines.extend(f"- `{path}`" for path in report.uncataloged_tests)
    else:
        lines.append("- none")
    lines.extend(
        [
            "",
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
    parser = argparse.ArgumentParser(description="Run the report-only verification gate.")
    parser.add_argument("--intent", default="bugfix", choices=["bugfix", "feature", "refactor", "docs", "test-refactor"])
    parser.add_argument("--base", help="Base revision for PR/diff reports")
    parser.add_argument("--head", default="HEAD", help="Head revision for PR/diff reports")
    parser.add_argument("--changed", nargs="*", help="Explicit changed-file list")
    parser.add_argument("--out-dir", default=str(DEFAULT_OUTPUT_DIR), help="Report output directory")
    parser.add_argument("--strict", action="store_true", help="Return non-zero on reported failures")
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
        )
        write_reports(report, ROOT / args.out_dir)
        print(render_markdown(report), end="")
        return report_exit_code(report, strict=args.strict)
    except Exception as exc:
        print(f"verification report gate error: {exc}", file=sys.stderr)
        return 1 if args.strict else 0


if __name__ == "__main__":
    raise SystemExit(main())
