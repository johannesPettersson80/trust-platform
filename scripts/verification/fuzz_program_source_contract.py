"""Reviewed live execution-source bindings for the Phase 9 fuzz program."""

from __future__ import annotations

import hashlib
import re
import subprocess
import tomllib
from collections.abc import Mapping, Sequence
from pathlib import Path


EXECUTION_SOURCE_PATHS = (
    "scripts/salsa_fuzz_gate.sh",
    "scripts/runtime_comms_fuzz_gate.sh",
    "scripts/runtime_vm_malformed_bytecode_fuzz_gate.sh",
    ".github/workflows/salsa-hardening.yml",
    ".github/workflows/ci.yml",
    "Cargo.toml",
)
REVIEWED_EXECUTABLE_MODE = {
    "scripts/salsa_fuzz_gate.sh": "100755",
    "scripts/runtime_comms_fuzz_gate.sh": "100755",
    "scripts/runtime_vm_malformed_bytecode_fuzz_gate.sh": "100755",
}
REVIEWED_EXECUTION_FILE_DIGESTS = {
    "scripts/salsa_fuzz_gate.sh": "b9cabd8d43a8ae56182b6820c02126fbf6ac89f3d4270f014b8e2005dd321b29",
    "scripts/runtime_comms_fuzz_gate.sh": "1647add928d5c339c8557a68cd93639278cc246a59f4a9050508f23d3064b40d",
    "scripts/runtime_vm_malformed_bytecode_fuzz_gate.sh": "6c226b829e70d3dc32c4b91a44d8a930b963a50e76bab0515c0284e60deadc4f",
    ".github/workflows/ci.yml": "babd06aec25c2008c391637fdf66bc5a27dda47dd535c5c1abb3727fbe7acad6",
    ".github/workflows/salsa-hardening.yml": "d5851fe83317008d0b41a02d547f25062196b8d4e274d95aef507a65dca7675f",
}
REVIEWED_WORKFLOW_TRIGGER_DIGESTS = {
    ".github/workflows/ci.yml": "31d4f1cce89957fcdf3d991b9fcef339025ba16957f2ee17e7a023faa2b5fe38",
    ".github/workflows/salsa-hardening.yml": "dedbf26bfbf7a21c5ed87a71f25798555672ff6a3b18ca68fb727b5bf2d792fb",
}
REVIEWED_WORKFLOW_JOB_DIGESTS = {
    (".github/workflows/ci.yml", "test"): (
        "c105e1bf0a505029132939e74e99f4d15294b51d804e7f52982d6aeecdf1f727"
    ),
    (".github/workflows/ci.yml", "conformance"): (
        "609e762b27f408f50585bb0b0b5fc0e3bb00693ab11e48aa6e727adbe1e5ee46"
    ),
    (".github/workflows/salsa-hardening.yml", "fuzz-smoke"): (
        "21763164faa5f2cb205a60b7a0121d1dae6842397d8e73a66a221ef0960f0fa3"
    ),
    (".github/workflows/salsa-hardening.yml", "fuzz-extended-nightly"): (
        "3c5b4d59e79d58d96e0ac2ba8e3a3d15e91a76120774ab49b67bc98d1521c3d8"
    ),
}
REVIEWED_SALSA_TARGETS = {"syntax_parse", "hir_semantic"}
REVIEWED_RUNTIME_COMMS_COMMANDS = {
    'run_observed "runtime-comms-fuzz" "mesh-payload" "${GATE_TEST_TIMEOUT_SECONDS:-900}" "${OUT_DIR}/mesh_payload_fuzz.log" env TRUST_COMMS_FUZZ_ITERS="${ITERS}" cargo test -p trust-runtime --lib mesh::tests::mesh_payload_encode_decode_fuzz_smoke_budget -- --nocapture',
    'run_observed "runtime-comms-fuzz" "shm-header" "${GATE_TEST_TIMEOUT_SECONDS:-900}" "${OUT_DIR}/shm_header_fuzz.log" env TRUST_COMMS_FUZZ_ITERS="${ITERS}" cargo test -p trust-runtime --lib realtime::tests::t0_shm_header_fuzz_rejects_corruption_budget -- --nocapture',
    'run_observed "runtime-comms-fuzz" "runtime-cloud-api" "${GATE_TEST_TIMEOUT_SECONDS:-900}" "${OUT_DIR}/runtime_cloud_api_fuzz.log" env TRUST_COMMS_FUZZ_ITERS="${ITERS}" cargo test -p trust-runtime --lib runtime_cloud::routing::tests::runtime_cloud_api_payload_fuzz_smoke_budget -- --nocapture',
    'run_observed "runtime-comms-fuzz" "runtime-cloud-acl" "${GATE_TEST_TIMEOUT_SECONDS:-900}" "${OUT_DIR}/runtime_cloud_acl_fuzz.log" env TRUST_COMMS_FUZZ_ITERS="${ITERS}" cargo test -p trust-runtime --lib runtime_cloud::profile_policy::tests::wan_allowlist_parser_fuzz_smoke_budget -- --nocapture',
}
REVIEWED_VM_FUZZ_COMMANDS = {
    'python3 ./scripts/run_with_progress.py --phase runtime-vm-malformed-bytecode-fuzz --target malformed-bytecode-fuzz-smoke --timeout-seconds "${GATE_TEST_TIMEOUT_SECONDS:-900}" --progress-interval-seconds "${GATE_PROGRESS_INTERVAL_SECONDS:-30}" --log "${ARTIFACT_DIR}/malformed-bytecode-fuzz-smoke.log" -- env -u OUT_DIR cargo test -p trust-runtime --test bytecode_vm_core vm_malformed_bytecode_fuzz_smoke_budget -- --nocapture'
}


def validate_execution_source_bindings(
    root: Path,
    targets: object,
    target_id_order: Sequence[str],
    failures: list[str],
) -> None:
    """Bind tier claims to reviewed executable sources and effective workspace membership."""

    if not isinstance(targets, list):
        return
    target_ids = {
        row.get("id")
        for row in targets
        if isinstance(row, Mapping) and isinstance(row.get("id"), str)
    }
    if target_ids != set(target_id_order):
        failures.append("execution source bindings do not match the reviewed target IDs")
    sources = _load_execution_sources(root, failures)
    validate_reviewed_execution_source_digests(sources, failures)
    for relative, expected_mode in REVIEWED_EXECUTABLE_MODE.items():
        mode = _tracked_git_mode(root, relative)
        if mode != expected_mode:
            failures.append(
                f"execution source {relative} must retain tracked mode {expected_mode}; found {mode!r}"
            )
    _validate_exact_set(
        _parse_salsa_run_targets(
            _decode_source(sources.get("scripts/salsa_fuzz_gate.sh", b""), "scripts/salsa_fuzz_gate.sh", failures)
        ),
        REVIEWED_SALSA_TARGETS,
        "Salsa cargo-fuzz target invocations",
        failures,
    )
    _validate_exact_set(
        _parse_runtime_comms_commands(
            _decode_source(
                sources.get("scripts/runtime_comms_fuzz_gate.sh", b""),
                "scripts/runtime_comms_fuzz_gate.sh",
                failures,
            )
        ),
        REVIEWED_RUNTIME_COMMS_COMMANDS,
        "runtime communication fuzz-smoke invocations",
        failures,
    )
    _validate_exact_set(
        _parse_vm_fuzz_commands(
            _decode_source(
                sources.get("scripts/runtime_vm_malformed_bytecode_fuzz_gate.sh", b""),
                "scripts/runtime_vm_malformed_bytecode_fuzz_gate.sh",
                failures,
            )
        ),
        REVIEWED_VM_FUZZ_COMMANDS,
        "runtime VM fuzz-smoke invocations",
        failures,
    )
    salsa_text = _active_source_text(
        _decode_source(
            sources.get(".github/workflows/salsa-hardening.yml", b""),
            ".github/workflows/salsa-hardening.yml",
            failures,
        )
    )
    salsa_modes = set(
        re.findall(
            r"^\s*run:\s*./scripts/salsa_fuzz_gate[.]sh\s+(smoke|extended)\s*$",
            salsa_text,
            re.MULTILINE,
        )
    )
    _validate_exact_set(
        salsa_modes,
        {"smoke", "extended"},
        "Salsa workflow fuzz modes",
        failures,
    )
    ci_text = _active_source_text(
        _decode_source(
            sources.get(".github/workflows/ci.yml", b""),
            ".github/workflows/ci.yml",
            failures,
        )
    )
    if len(re.findall(r"^\s*./scripts/runtime_comms_fuzz_gate[.]sh\s*$", ci_text, re.MULTILINE)) != 1:
        failures.append("CI must contain exactly one active runtime communication fuzz-gate invocation")
    if "cargo test --all-targets" not in ci_text:
        failures.append("CI workspace test job must retain an active cargo test --all-targets command")
    _validate_effective_workspace_membership(
        _decode_source(sources.get("Cargo.toml", b""), "Cargo.toml", failures), failures
    )


def _load_execution_sources(root: Path, failures: list[str]) -> dict[str, bytes]:
    sources: dict[str, bytes] = {}
    for relative in EXECUTION_SOURCE_PATHS:
        path = root / relative
        try:
            if path.is_symlink():
                raise OSError("symlink is not allowed")
            raw = path.read_bytes()
            raw.decode()
            sources[relative] = raw
        except (OSError, UnicodeError) as exc:
            failures.append(f"execution source {relative} cannot be read: {exc}")
            sources[relative] = b""
    return sources


def validate_reviewed_execution_source_digests(
    sources: Mapping[str, bytes], failures: list[str]
) -> None:
    """Validate exact reviewed script, trigger, and workflow-job content boundaries."""

    for relative, expected in REVIEWED_EXECUTION_FILE_DIGESTS.items():
        actual = _raw_digest(sources.get(relative, b""))
        if actual != expected:
            failures.append(f"execution source {relative} digest drifted")
    for relative, expected in REVIEWED_WORKFLOW_TRIGGER_DIGESTS.items():
        workflow = _decode_source(sources.get(relative, b""), relative, failures)
        if sum(line == "on:" for line in workflow.splitlines()) != 1:
            failures.append(f"execution source {relative} must contain exactly one top-level on block")
        if sum(line == "jobs:" for line in workflow.splitlines()) != 1:
            failures.append(f"execution source {relative} must contain exactly one top-level jobs block")
        try:
            block = _workflow_trigger_block(workflow)
        except ValueError as exc:
            failures.append(f"execution source {relative}#on cannot be resolved: {exc}")
            continue
        if _text_digest(block) != expected:
            failures.append(f"execution source {relative}#on digest drifted")
    for (relative, job_id), expected in REVIEWED_WORKFLOW_JOB_DIGESTS.items():
        workflow = _decode_source(sources.get(relative, b""), relative, failures)
        if sum(
            re.fullmatch(rf"  {re.escape(job_id)}:\s*(?:#.*)?", line) is not None
            for line in workflow.splitlines()
        ) != 1:
            failures.append(
                f"execution source {relative} must contain exactly one reviewed job {job_id}"
            )
        try:
            block = _workflow_job_block(workflow, job_id)
        except ValueError as exc:
            failures.append(f"execution source {relative}#{job_id} cannot be resolved: {exc}")
            continue
        if _text_digest(block) != expected:
            failures.append(f"execution source {relative}#{job_id} digest drifted")


def _workflow_job_block(workflow: str, job_id: str) -> str:
    lines = workflow.splitlines()
    start = next(
        (
            index
            for index, line in enumerate(lines)
            if re.fullmatch(rf"  {re.escape(job_id)}:\s*(?:#.*)?", line)
        ),
        None,
    )
    if start is None:
        raise ValueError(f"reviewed workflow job block is missing: {job_id}")
    end = len(lines)
    for index in range(start + 1, len(lines)):
        if re.match(r"^  [A-Za-z0-9_-]+:\s*(?:#.*)?$", lines[index]):
            end = index
            break
        if lines[index] and not lines[index].startswith((" ", "\t", "#")):
            end = index
            break
    return "\n".join(lines[start:end])


def _workflow_trigger_block(workflow: str) -> str:
    lines = workflow.splitlines()
    start = next((index for index, line in enumerate(lines) if line == "on:"), None)
    if start is None:
        raise ValueError("reviewed workflow trigger block is missing")
    end = len(lines)
    for index in range(start + 1, len(lines)):
        if lines[index] and not lines[index].startswith((" ", "\t", "#")):
            end = index
            break
    return "\n".join(lines[start:end])


def _tracked_git_mode(root: Path, relative: str) -> str | None:
    result = subprocess.run(
        ["git", "ls-files", "--stage", "--", relative],
        cwd=root,
        check=False,
        capture_output=True,
        text=True,
    )
    if result.returncode != 0:
        return None
    rows = [line for line in result.stdout.splitlines() if line]
    if len(rows) != 1:
        return None
    return rows[0].split(maxsplit=1)[0]


def _validate_effective_workspace_membership(source: str, failures: list[str]) -> None:
    try:
        workspace = tomllib.loads(source).get("workspace")
    except tomllib.TOMLDecodeError as exc:
        failures.append(f"Cargo workspace cannot be parsed for parser-smoke binding: {exc}")
        return
    if not isinstance(workspace, Mapping):
        failures.append("Cargo workspace table is missing for parser-smoke binding")
        return
    members = workspace.get("members")
    if not isinstance(members, list) or "crates/trust-syntax" not in members:
        failures.append("Cargo workspace must retain crates/trust-syntax for the PR property smoke")
        return
    default_members = workspace.get("default-members", members)
    if not isinstance(default_members, list) or "crates/trust-syntax" not in default_members:
        failures.append(
            "Cargo effective default workspace must retain crates/trust-syntax for the PR property smoke"
        )


def _active_source_text(text: str) -> str:
    return "\n".join(
        line for line in text.splitlines() if line.strip() and not line.lstrip().startswith("#")
    )


def _parse_salsa_run_targets(text: str) -> set[str]:
    return set(
        re.findall(
            r'^\s*run_target\s+"([A-Za-z0-9_-]+)"\s*$',
            _active_source_text(text),
            re.MULTILINE,
        )
    )


def _parse_runtime_comms_commands(text: str) -> set[str]:
    return {
        command
        for command in _parse_shell_commands(text)
        if command.startswith("run_observed ") and " cargo test " in command
    }


def _parse_vm_fuzz_commands(text: str) -> set[str]:
    return {
        command
        for command in _parse_shell_commands(text)
        if command.startswith("python3 ./scripts/run_with_progress.py ")
        and " -- env -u OUT_DIR cargo test " in command
    }


def _parse_shell_commands(text: str) -> tuple[str, ...]:
    commands: list[str] = []
    current: list[str] = []
    heredoc_end: str | None = None
    for raw in text.splitlines():
        stripped = raw.strip()
        if heredoc_end is not None:
            if stripped == heredoc_end:
                heredoc_end = None
            continue
        if not stripped or stripped.startswith("#"):
            continue
        heredoc = re.search(r"<<-?([A-Za-z_][A-Za-z0-9_]*)", stripped)
        if heredoc:
            if current:
                commands.append(" ".join(current))
                current = []
            heredoc_end = heredoc.group(1)
            continue
        if stripped.endswith("\\"):
            current.append(stripped[:-1].rstrip())
            continue
        if current:
            current.append(stripped)
            commands.append(" ".join(current))
            current = []
        else:
            commands.append(stripped)
    if current:
        commands.append(" ".join(current))
    return tuple(commands)


def _validate_exact_set(
    actual: set[str], expected: set[str], label: str, failures: list[str]
) -> None:
    if actual != expected:
        failures.append(f"{label} drifted: expected={sorted(expected)!r}, actual={sorted(actual)!r}")


def _text_digest(text: str) -> str:
    return hashlib.sha256(text.encode()).hexdigest()


def _raw_digest(raw: bytes) -> str:
    return hashlib.sha256(raw).hexdigest()


def _decode_source(raw: bytes, relative: str, failures: list[str]) -> str:
    if not isinstance(raw, bytes):
        failures.append(f"execution source {relative} must be raw bytes")
        return ""
    try:
        return raw.decode()
    except UnicodeError as exc:
        failures.append(f"execution source {relative} is not UTF-8: {exc}")
        return ""
