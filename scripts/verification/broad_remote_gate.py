"""Execute the reviewed trust-builder broad gate and append authentic evidence."""

from __future__ import annotations

import datetime as dt
import subprocess
import time
import uuid
from dataclasses import dataclass
from pathlib import Path
from typing import Any, Callable, Mapping

from .broad_remote_artifacts import (
    BroadRemoteArtifactError,
    artifact_cleanup_shell,
    artifact_read_shell,
    execution_shell,
    validate_case_backed_test,
    validate_execution_artifact,
)
from .metadata_validator.broad_remote_gate_evidence import (
    CANONICAL_PATH,
    GENERATED_REPORT_VERSION,
    MIN_HOME_AVAILABLE_KIB,
    MIN_TMP_AVAILABLE_KIB,
    PLATFORM,
    PRODUCER,
    REMOTE_GATE_SHELL,
    REVIEWED_GATE_COMMAND,
    RUST_SOURCE_KINDS,
    SUITE_ID,
    validate_broad_remote_gate_evidence,
)
from .metadata_validator.constants import ROOT
from .metadata_validator.core import Validator
from .metadata_validator.integrity import RUNNABLE_TEST_STATUSES
from .proof_output import (
    ProofOutputError,
    ProofRevisionSession,
    append_evidence_record,
)


Executor = Callable[..., "CommandResult"]
EvidenceWriter = Callable[..., Path]
REMOTE_STATUS_SHELL = (
    'cd "$HOME/projects/trust-platform" && git status --porcelain --untracked-files=all'
)
REMOTE_HEAD_SHELL = (
    'cd "$HOME/projects/trust-platform" && git rev-parse --verify \'HEAD^{commit}\''
)
REMOTE_DISK_AUDIT_SHELL = (
    'df -hT /home/johannes /tmp && '
    'du -xhd1 "$HOME/projects" 2>/dev/null | sort -h | tail -20 && '
    'du -xhd1 "$HOME/.cache" 2>/dev/null | sort -h | tail -20'
)


class BroadRemoteGateError(RuntimeError):
    pass


@dataclass(frozen=True)
class CommandResult:
    returncode: int
    stdout: str
    stderr: str


@dataclass(frozen=True)
class BroadRemoteGateResult:
    record: dict[str, Any]
    evidence_path: Path


class BroadRemoteGateProducer:
    """Own the fixed PR broad-gate execution and its canonical evidence write."""

    def __init__(
        self,
        *,
        root: Path = ROOT,
        tests: Mapping[str, Mapping[str, Any]] | None = None,
        invariants: Mapping[str, Mapping[str, Any]] | None = None,
        ignored_tests: Mapping[str, Mapping[str, Any]] | None = None,
        suites: Mapping[str, Mapping[str, Any]] | None = None,
        executor: Executor | None = None,
        evidence_writer: EvidenceWriter = append_evidence_record,
        revision_provider: Callable[[], str] | None = None,
        utc_now: Callable[[], str] | None = None,
        monotonic: Callable[[], float] | None = None,
        validate_metadata: bool = True,
    ) -> None:
        self.root = root
        if validate_metadata:
            validator = _load_validated_metadata()
            tests = validator.tests
            invariants = validator.invariants
            ignored_tests = validator.ignored_tests
            suites = validator.suites
        self.tests = tests or {}
        self.invariants = invariants or {}
        self.ignored_tests = ignored_tests or {}
        self.suites = suites or {}
        self.executor = executor or _execute
        self.evidence_writer = evidence_writer
        self.proof_revision = ProofRevisionSession(
            root=root,
            revision_provider=revision_provider,
        )
        self.utc_now = utc_now or _utc_now
        self.monotonic = monotonic or time.monotonic
        self.evidence_index_path = root / CANONICAL_PATH

    def run(self, invariant_ids: list[str]) -> BroadRemoteGateResult:
        revision = self._begin_revision()
        selected, linked_tests, area = self._select_links(invariant_ids)
        self._require_suite_approval()
        platform, home_available, tmp_available = self._remote_preflight(revision)

        started_at = self.utc_now()
        started = self.monotonic()
        gate = self.executor(_ssh(REMOTE_GATE_SHELL), capture=False)
        executed_tests: list[dict[str, Any]] = []
        execution_error: str | None = None
        try:
            if gate.returncode == 0:
                for test_id in linked_tests:
                    executed_tests.append(self._run_selected_test(test_id))
        except (BroadRemoteGateError, BroadRemoteArtifactError) as exc:
            execution_error = str(exc)
        cleanup = self.executor(
            _ssh(artifact_cleanup_shell(linked_tests)),
            capture=True,
        )
        if cleanup.returncode != 0 and execution_error is None:
            execution_error = "failed to remove transient remote case artifacts"
        finished = self.monotonic()
        finished_at = self.utc_now()

        self._remote_postflight(revision)
        self._confirm_revision()
        if gate.returncode != 0:
            raise BroadRemoteGateError(
                f"reviewed broad gate failed with exit status {gate.returncode}; "
                "no evidence was written"
            )
        if execution_error is not None:
            raise BroadRemoteGateError(execution_error + "; no evidence was written")

        record = self._record(
            invariant_ids=selected,
            linked_tests=linked_tests,
            area=area,
            revision=revision,
            platform=platform,
            started_at=started_at,
            finished_at=finished_at,
            duration_milliseconds=round((finished - started) * 1000),
            executed_tests=executed_tests,
            home_available_kib=home_available,
            tmp_available_kib=tmp_available,
        )
        failures: list[str] = []
        validate_broad_remote_gate_evidence(
            fail=lambda _path, message: failures.append(message),
            path=self.evidence_index_path,
            record=record,
            invariants=self.invariants,
            tests=self.tests,
            ignored_tests=self.ignored_tests,
        )
        if failures:
            raise BroadRemoteGateError(
                "generated broad gate evidence failed its contract: " + "; ".join(failures)
            )
        try:
            evidence_path = self.evidence_writer(
                root=self.root,
                evidence_index_path=self.evidence_index_path,
                record=record,
            )
        except ProofOutputError as exc:
            raise BroadRemoteGateError(str(exc)) from exc
        return BroadRemoteGateResult(record=record, evidence_path=evidence_path)

    def _begin_revision(self) -> str:
        try:
            return self.proof_revision.begin()
        except ProofOutputError as exc:
            raise BroadRemoteGateError(str(exc)) from exc

    def _confirm_revision(self) -> None:
        try:
            self.proof_revision.confirm()
        except ProofOutputError as exc:
            raise BroadRemoteGateError(str(exc)) from exc

    def _select_links(self, requested: list[str]) -> tuple[list[str], list[str], str]:
        if not requested or any(not isinstance(value, str) or not value for value in requested):
            raise BroadRemoteGateError("at least one invariant id is required")
        invariant_ids = sorted(set(requested))
        tests: set[str] = set()
        areas: set[str] = set()
        for invariant_id in invariant_ids:
            invariant = self.invariants.get(invariant_id)
            if not isinstance(invariant, Mapping):
                raise BroadRemoteGateError(f"unknown invariant {invariant_id}")
            invariant_tests = invariant.get("tests")
            if not isinstance(invariant_tests, list) or not invariant_tests:
                raise BroadRemoteGateError(f"invariant {invariant_id} has no linked tests")
            for test_id in invariant_tests:
                if not isinstance(test_id, str) or test_id not in self.tests:
                    raise BroadRemoteGateError(
                        f"invariant {invariant_id} references unknown test {test_id!r}"
                    )
                test = self.tests[test_id]
                if test.get("status") not in RUNNABLE_TEST_STATUSES:
                    raise BroadRemoteGateError(f"linked test {test_id} is not runnable")
                if test.get("discovery_source_kind") not in RUST_SOURCE_KINDS:
                    raise BroadRemoteGateError(
                        f"linked test {test_id} is outside the reviewed Rust gate"
                    )
                suite_tiers = test.get("suite_tiers")
                if (
                    not isinstance(suite_tiers, list)
                    or any(not isinstance(tier, str) for tier in suite_tiers)
                    or "pr" not in suite_tiers
                ):
                    raise BroadRemoteGateError(f"linked test {test_id} is not assigned to suite pr")
                discovery_id = test.get("discovery_id")
                if not isinstance(discovery_id, str) or not discovery_id:
                    raise BroadRemoteGateError(f"linked test {test_id} has no discovery identity")
                if any(
                    ignored.get("discovery_id") == discovery_id
                    for ignored in self.ignored_tests.values()
                    if isinstance(ignored, Mapping)
                ):
                    raise BroadRemoteGateError(f"linked test {test_id} is ignored by the broad gate")
                try:
                    validate_case_backed_test(root=self.root, test_id=test_id, test=test)
                except BroadRemoteArtifactError as exc:
                    raise BroadRemoteGateError(str(exc)) from exc
                tests.add(test_id)
            area = invariant.get("area")
            if not isinstance(area, str) or not area:
                raise BroadRemoteGateError(f"invariant {invariant_id} has no area")
            areas.add(area)
        if len(areas) != 1:
            raise BroadRemoteGateError("one broad gate record cannot mix invariant areas")
        return invariant_ids, sorted(tests), next(iter(areas))

    def _require_suite_approval(self) -> None:
        suite = self.suites.get(SUITE_ID)
        approved = suite.get("approved_proof_producers") if isinstance(suite, Mapping) else None
        if not isinstance(approved, list) or PRODUCER not in approved:
            raise BroadRemoteGateError(f"suite {SUITE_ID} does not allowlist {PRODUCER}")

    def _remote_preflight(self, revision: str) -> tuple[str, int, int]:
        self._require_remote_clean_revision(revision, when="before")
        self._probe(_ssh(REMOTE_DISK_AUDIT_SHELL), "remote disk audit")
        home_available = self._available_kib("/home/johannes")
        tmp_available = self._available_kib("/tmp")
        if home_available < MIN_HOME_AVAILABLE_KIB:
            raise BroadRemoteGateError(
                "remote /home/johannes has insufficient free space for the reviewed broad gate"
            )
        if tmp_available < MIN_TMP_AVAILABLE_KIB:
            raise BroadRemoteGateError(
                "remote /tmp has insufficient free space for the reviewed broad gate"
            )
        system = self._probe(_ssh("uname -s"), "remote operating system")
        architecture = self._probe(_ssh("uname -m"), "remote architecture")
        platform = f"trust-builder-{system.lower()}-{architecture.lower()}"
        if platform != PLATFORM:
            raise BroadRemoteGateError(
                f"reviewed broad gate requires platform {PLATFORM}, found {platform}"
            )
        return platform, home_available, tmp_available

    def _available_kib(self, path: str) -> int:
        output = self._probe(_ssh(f"df -Pk {path}"), f"free space for {path}")
        lines = [line.split() for line in output.splitlines() if line.strip()]
        try:
            value = int(lines[-1][3])
        except (IndexError, ValueError) as exc:
            raise BroadRemoteGateError(
                f"could not parse free space for {path}"
            ) from exc
        return value

    def _run_selected_test(self, test_id: str) -> dict[str, Any]:
        test = self.tests[test_id]
        run_id = uuid.uuid4().hex
        result = self.executor(
            _ssh(execution_shell(test_id, test, run_id)),
            capture=False,
        )
        if result.returncode != 0:
            raise BroadRemoteGateError(
                f"selected catalog command for {test_id} failed with exit status "
                f"{result.returncode}"
            )
        raw_artifact = self._probe(
            _ssh(artifact_read_shell(test_id)),
            f"case artifact for {test_id}",
            strip=False,
        )
        return validate_execution_artifact(
            root=self.root,
            test_id=test_id,
            test=test,
            run_id=run_id,
            raw_artifact=raw_artifact,
        )

    def _remote_postflight(self, revision: str) -> None:
        self._require_remote_clean_revision(revision, when="after")

    def _require_remote_clean_revision(self, revision: str, *, when: str) -> None:
        status = self._probe(_ssh(REMOTE_STATUS_SHELL), "remote Git status", strip=False)
        if status:
            raise BroadRemoteGateError(f"remote worktree is dirty {when} broad gate execution")
        remote_revision = self._probe(_ssh(REMOTE_HEAD_SHELL), "remote HEAD")
        if remote_revision != revision:
            raise BroadRemoteGateError(
                f"remote HEAD {remote_revision!r} does not match local clean HEAD {revision}"
            )

    def _probe(self, argv: tuple[str, ...], label: str, *, strip: bool = True) -> str:
        result = self.executor(argv, capture=True)
        if result.returncode != 0:
            detail = result.stderr.strip() or f"exit {result.returncode}"
            raise BroadRemoteGateError(f"{label} probe failed: {detail}")
        return result.stdout.strip() if strip else result.stdout

    def _record(
        self,
        *,
        invariant_ids: list[str],
        linked_tests: list[str],
        area: str,
        revision: str,
        platform: str,
        started_at: str,
        finished_at: str,
        duration_milliseconds: int,
        executed_tests: list[dict[str, Any]],
        home_available_kib: int,
        tmp_available_kib: int,
    ) -> dict[str, Any]:
        date = started_at[:10]
        compact_date = date.replace("-", "")
        evidence_id = f"EVID_BROAD_REMOTE_PR_{compact_date}_{uuid.uuid4().hex[:12].upper()}"
        return {
            "schema_version": 1,
            "id": evidence_id,
            "title": "Reviewed PR broad gate for " + ", ".join(invariant_ids),
            "area": area,
            "owner": "verification",
            "status": "mapped",
            "kind": "committed_file",
            "path": CANONICAL_PATH,
            "command": REVIEWED_GATE_COMMAND,
            "commit": revision,
            "remote_commit": revision,
            "platform": platform,
            "date": date,
            "suite_id": SUITE_ID,
            "producer": PRODUCER,
            "generated_report_version": GENERATED_REPORT_VERSION,
            "linked_invariants": invariant_ids,
            "linked_tests": linked_tests,
            "linked_spec_gaps": [],
            "last_reviewed": date,
            "proof_kind": "none",
            "proof_scope": "broad_remote_gate",
            "command_exit_status": 0,
            "executed_tests": executed_tests,
            "gate_started_at": started_at,
            "gate_finished_at": finished_at,
            "gate_duration_milliseconds": duration_milliseconds,
            "local_source_clean_before": True,
            "local_source_clean_after": True,
            "remote_source_clean_before": True,
            "remote_source_clean_after": True,
            "disk_preflight_passed": True,
            "home_available_kib": home_available_kib,
            "tmp_available_kib": tmp_available_kib,
        }


def _load_validated_metadata() -> Validator:
    validator = Validator()
    validator.load_records()
    validator.validate()
    if validator.failures:
        detail = "; ".join(failure.message for failure in validator.failures[:10])
        raise BroadRemoteGateError(f"verification metadata is invalid: {detail}")
    return validator


def _execute(argv: tuple[str, ...], *, capture: bool) -> CommandResult:
    try:
        result = subprocess.run(
            argv,
            cwd=ROOT,
            text=True,
            stdout=subprocess.PIPE if capture else None,
            stderr=subprocess.PIPE if capture else None,
            check=False,
        )
    except OSError as exc:
        raise BroadRemoteGateError(f"failed to execute reviewed command: {exc}") from exc
    return CommandResult(result.returncode, result.stdout or "", result.stderr or "")


def _ssh(remote_shell: str) -> tuple[str, ...]:
    return ("ssh", "trust-builder", remote_shell)


def _utc_now() -> str:
    return dt.datetime.now(dt.timezone.utc).isoformat(timespec="seconds").replace("+00:00", "Z")
