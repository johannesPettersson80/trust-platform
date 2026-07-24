"""Focused LLVM tracing for ST and conformance tests run through Rust binaries."""

from __future__ import annotations

import hashlib
import json
import os
import shlex
import subprocess
import time
from collections.abc import Mapping, Sequence
from pathlib import Path
from typing import Any

from .code_spec_rust import RustInventory
from .coverage_trace_document import build_rust_trace_document
from .coverage_trace_inventory import code_inventory_digest
from .coverage_trace_resume import reusable_completed_attempt_ids
from .coverage_trace_tools import export_attempt_coverage


DIRECT_SOURCE_KINDS = frozenset({"structured_text_test", "conformance_case"})


def direct_test_command(
    fact: Mapping[str, Any],
    *,
    root: Path,
    trust_dev_bin: Path,
    trust_runtime_bin: Path,
) -> tuple[list[str], str]:
    """Resolve scanner facts to shell-free exact executable arguments."""

    source_kind = _required_string(fact, "discovery_source_kind", "scanner fact")
    name = _required_string(fact, "name", "scanner fact")
    path = _required_string(fact, "path", "scanner fact")
    command_hint = _required_string(fact, "command_hint", "scanner fact")
    if source_kind == "conformance_case":
        expected = (
            "cargo run -p trust-runtime --bin trust-runtime -- conformance "
            f"--suite-root conformance --filter {name}"
        )
        if command_hint != expected:
            raise ValueError("conformance command hint drifted from scanner contract")
        return (
            [
                str(trust_runtime_bin.resolve()),
                "conformance",
                "--suite-root",
                "conformance",
                "--filter",
                name,
            ],
            "exact",
        )
    if source_kind == "structured_text_test":
        source = (root / path).resolve()
        try:
            source.relative_to(root.resolve())
        except ValueError as error:
            raise ValueError("Structured Text test path escapes repository") from error
        project = _st_project_root(source)
        if project is None:
            raise ValueError(f"Structured Text test has no project root: {path}")
        project_relative = project.relative_to(root.resolve()).as_posix()
        expected = (
            "cargo run -p trust-dev -- test "
            f"--project {project_relative} --filter {name}"
        )
        if command_hint != expected:
            raise ValueError("Structured Text command hint drifted from scanner contract")
        return (
            [
                str(trust_dev_bin.resolve()),
                "test",
                "--project",
                project_relative,
                "--filter",
                name,
            ],
            "verified_synthesized",
        )
    raise ValueError(f"unsupported direct trace source kind {source_kind!r}")


def direct_test_request(
    fact: Mapping[str, Any],
    *,
    command: Sequence[str],
    authority: str,
) -> dict[str, Any]:
    """Describe an exact direct test in the shared resumable-attempt schema."""

    executable = Path(command[0]).resolve()
    return {
        "discovery_id": _required_string(fact, "discovery_id", "scanner fact"),
        "discovery_source_kind": _required_string(
            fact, "discovery_source_kind", "scanner fact"
        ),
        "path": _required_string(fact, "path", "scanner fact"),
        "name": _required_string(fact, "name", "scanner fact"),
        "package": fact.get("package"),
        "nextest_binary_id": executable.name,
        "nextest_binary_path": str(executable),
        "nextest_test_name": _required_string(fact, "name", "scanner fact"),
        "command": shlex.join(command),
        "command_authority": authority,
    }


def build_instrumented_binaries(
    *,
    root: Path,
    cargo_target_dir: Path,
    base_environment: Mapping[str, str],
) -> tuple[dict[str, Path], dict[str, str]]:
    """Build both direct runners once using cargo-llvm-cov's exact environment."""

    shown = subprocess.run(
        ["cargo", "llvm-cov", "show-env", "--no-cfg-coverage"],
        cwd=root,
        env=dict(base_environment),
        check=False,
        capture_output=True,
        text=True,
    )
    if shown.returncode != 0:
        raise ValueError(
            "cargo llvm-cov show-env failed: "
            f"{shown.stderr.strip() or shown.stdout.strip()}"
        )
    environment = direct_instrumentation_environment(
        base_environment,
        parse_llvm_cov_environment(shown.stdout),
    )
    target = cargo_target_dir.resolve()
    target.mkdir(parents=True, exist_ok=True)
    environment["CARGO_TARGET_DIR"] = str(target)
    environment["CARGO_LLVM_COV_TARGET_DIR"] = str(target)
    environment["CARGO_LLVM_COV_BUILD_DIR"] = str(target)
    for package, binary in (
        ("trust-dev", "trust-dev"),
        ("trust-runtime", "trust-runtime"),
    ):
        built = subprocess.run(
            ["cargo", "build", "-p", package, "--bin", binary],
            cwd=root,
            env=environment,
            check=False,
            capture_output=True,
            text=True,
        )
        if built.returncode != 0:
            raise ValueError(
                f"instrumented {binary} build failed: "
                f"{built.stderr.strip() or built.stdout.strip()}"
            )
    suffix = ".exe" if os.name == "nt" else ""
    binaries = {
        "trust-dev": target / "debug" / f"trust-dev{suffix}",
        "trust-runtime": target / "debug" / f"trust-runtime{suffix}",
    }
    missing = sorted(name for name, path in binaries.items() if not path.is_file())
    if missing:
        raise ValueError(f"instrumented binaries are missing: {missing}")
    return binaries, environment


def direct_instrumentation_environment(
    base_environment: Mapping[str, str],
    llvm_cov_environment: Mapping[str, str],
) -> dict[str, str]:
    """Expose coverage flags to Cargo's fingerprint instead of hiding in a wrapper."""

    encoded_flags = llvm_cov_environment.get(
        "__CARGO_LLVM_COV_RUSTC_WRAPPER_RUSTFLAGS"
    )
    if not isinstance(encoded_flags, str) or not encoded_flags:
        raise ValueError("cargo llvm-cov environment lacks encoded Rust flags")
    flags = [flag for flag in encoded_flags.split("\x1f") if flag]
    if not flags:
        raise ValueError("cargo llvm-cov encoded Rust flags are empty")
    environment = dict(base_environment)
    existing = environment.get("RUSTFLAGS", "").strip()
    environment["RUSTFLAGS"] = " ".join(
        [part for part in (existing, *flags) if part]
    )
    previous_wrapper = llvm_cov_environment.get(
        "__CARGO_LLVM_COV_RUSTC_WRAPPER_PRE_EXISTING"
    )
    if isinstance(previous_wrapper, str) and previous_wrapper:
        environment["RUSTC_WRAPPER"] = previous_wrapper
    else:
        environment.pop("RUSTC_WRAPPER", None)
    environment["CARGO_INCREMENTAL"] = "0"
    environment["LLVM_PROFILE_FILE"] = str(
        llvm_cov_environment["LLVM_PROFILE_FILE"]
    )
    return environment


def parse_llvm_cov_environment(output: str) -> dict[str, str]:
    """Parse cargo-llvm-cov assignment output without evaluating a shell."""

    environment: dict[str, str] = {}
    for line_number, raw_line in enumerate(output.splitlines(), start=1):
        line = raw_line.strip()
        if not line:
            continue
        try:
            parts = shlex.split(line, posix=True)
        except ValueError as error:
            raise ValueError(
                f"cargo llvm-cov environment line {line_number} is invalid"
            ) from error
        if len(parts) != 1 or "=" not in parts[0]:
            raise ValueError(
                f"cargo llvm-cov environment line {line_number} is not an assignment"
            )
        key, value = parts[0].split("=", 1)
        if not key or not key.replace("_", "").isalnum() or key[0].isdigit():
            raise ValueError(
                f"cargo llvm-cov environment line {line_number} has invalid key"
            )
        environment[key] = value
    required = {
        "LLVM_PROFILE_FILE",
        "CARGO_LLVM_COV",
        "RUSTC_WRAPPER",
    }
    missing = sorted(required - set(environment))
    if missing:
        raise ValueError(f"cargo llvm-cov environment lacks {missing}")
    return environment


def run_direct_trace(
    *,
    root: Path,
    facts: Sequence[Mapping[str, Any]],
    trace_root: Path,
    output_path: Path,
    binaries: Mapping[str, Path],
    environment: Mapping[str, str],
    llvm_tools_dir: Path,
    rust_inventory: RustInventory,
    source_commit: str,
    platform: str,
    input_digest: str,
    mode: str,
    resume: bool = False,
) -> dict[str, Any]:
    """Execute exact direct tests, isolate profiles, and build a Rust trace."""

    if trace_root.exists() and any(trace_root.iterdir()) and not resume:
        raise ValueError("direct trace root must be absent or empty")
    trace_root.mkdir(parents=True, exist_ok=True)
    requested: list[dict[str, Any]] = []
    prepared: list[tuple[Mapping[str, Any], list[str], str]] = []
    trust_dev_bin = _required_binary(binaries, "trust-dev")
    trust_runtime_bin = _required_binary(binaries, "trust-runtime")
    for fact in sorted(facts, key=lambda row: str(row.get("discovery_id", ""))):
        discovery_id = _required_string(fact, "discovery_id", "scanner fact")
        source_kind = _required_string(
            fact, "discovery_source_kind", discovery_id
        )
        if source_kind not in DIRECT_SOURCE_KINDS:
            raise ValueError(
                f"{discovery_id} is not a supported direct Rust-backed test"
            )
        command, authority = direct_test_command(
            fact,
            root=root,
            trust_dev_bin=trust_dev_bin,
            trust_runtime_bin=trust_runtime_bin,
        )
        request = direct_test_request(
            fact,
            command=command,
            authority=authority,
        )
        binary_id = _required_string(
            request, "nextest_binary_id", discovery_id
        )
        requested.append(request)
        prepared.append((fact, command, binary_id))
    reusable = (
        reusable_completed_attempt_ids(
            trace_root=trace_root,
            resolved=requested,
            purge_incomplete=True,
        )
        if resume
        else set()
    )
    for fact, command, binary_id in prepared:
        discovery_id = _required_string(fact, "discovery_id", "scanner fact")
        if discovery_id in reusable:
            continue
        attempt_dir = (
            trace_root
            / "attempts"
            / hashlib.sha256(discovery_id.encode()).hexdigest()
        )
        profiles = attempt_dir / "profiles"
        profiles.mkdir(parents=True)
        attempt_environment = dict(environment)
        attempt_environment["LLVM_PROFILE_FILE"] = str(
            profiles / "%m-%p.profraw"
        )
        started = time.monotonic()
        completed = subprocess.run(
            command,
            cwd=root,
            env=attempt_environment,
            check=False,
            capture_output=True,
            text=True,
        )
        duration_ms = max(0, round((time.monotonic() - started) * 1000))
        if completed.returncode != 0:
            raise ValueError(
                f"{discovery_id} direct test failed with {completed.returncode}: "
                f"{completed.stderr.strip() or completed.stdout.strip()}"
            )
        if not any(profiles.glob("*.profraw")):
            raise ValueError(f"{discovery_id} produced no LLVM profile")
        attempt_id = f"direct:{discovery_id}"
        manifest = {
            "schema_version": 1,
            "discovery_id": discovery_id,
            "binary_id": binary_id,
            "test_name": _required_string(fact, "name", discovery_id),
            "attempt_id": attempt_id,
            "test_executable": str(Path(command[0]).resolve()),
            "test_executable_digest": _file_digest(Path(command[0])),
            "test_arguments": command[1:],
            "status": "completed",
            "exit_code": 0,
            "duration_ms": duration_ms,
        }
        (attempt_dir / "attempt.json").write_text(
            json.dumps(manifest, indent=2, sort_keys=True) + "\n"
        )
    exports = export_attempt_coverage(
        root=root,
        trace_root=trace_root,
        llvm_tools_dir=llvm_tools_dir,
        source_paths=rust_inventory.source_paths,
    )
    sources = {
        path: (root / path).read_bytes() for path in rust_inventory.source_paths
    }
    document = build_rust_trace_document(
        root=root,
        trace_root=trace_root,
        requested=requested,
        coverage_exports=exports,
        sources=sources,
        spans=rust_inventory.spans,
        source_commit=source_commit,
        platform=platform,
        input_digest=input_digest,
        code_inventory_digests={
            "rust_product": code_inventory_digest(
                rust_inventory.facts,
                rust_inventory.spans,
            )
        },
        mode=mode,
    )
    output_path.parent.mkdir(parents=True, exist_ok=True)
    output_path.write_text(json.dumps(document, indent=2, sort_keys=True) + "\n")
    return document


def _st_project_root(source: Path) -> Path | None:
    for parent in source.parents:
        if parent.name != "src":
            continue
        project = parent.parent
        if any(ancestor.name == "tests" for ancestor in project.parents):
            return project
    return None


def _required_binary(binaries: Mapping[str, Path], name: str) -> Path:
    path = binaries.get(name)
    if not isinstance(path, Path) or not path.is_file():
        raise ValueError(f"required instrumented binary is missing: {name}")
    return path


def _file_digest(path: Path) -> str:
    return "sha256:" + hashlib.sha256(path.read_bytes()).hexdigest()


def _required_string(
    record: Mapping[str, Any],
    field: str,
    label: str,
) -> str:
    value = record.get(field)
    if not isinstance(value, str) or not value:
        raise ValueError(f"{label} requires non-empty {field}")
    return value
