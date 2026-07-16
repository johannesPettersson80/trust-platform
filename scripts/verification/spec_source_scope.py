"""Mechanical tracked-document scope for the specification audit."""

from __future__ import annotations

from dataclasses import asdict, dataclass
from pathlib import PurePosixPath
from typing import Any


TEXT_SUFFIXES = frozenset({".md", ".txt"})
ROOT_SPEC_DOCUMENTS = frozenset(
    {
        "CHANGELOG.md",
        "CODE_OF_CONDUCT.md",
        "CONTRIBUTING.md",
        "README.md",
        "SECURITY.md",
    }
)
EVIDENCE_BACKED_SPEC_DOCUMENTS = frozenset(
    {
        "docs/internal/testing/evidence/plc-verification-program/2026-07-08/review-verdict.md",
        "docs/internal/testing/evidence/runtime-connectivity-conformance/2026-07-04-phase-03/opcua-client-subscription-spike.md",
    }
)
EVIDENCE_PLANE_PREFIX = "docs/internal/testing/evidence/"
REVIEWED_POSTURES = (
    "source_present",
    "gap_open_partial",
    "gap_open",
    "partial_source_no_gap",
    "nonoracle_context_only",
    "unrepresented",
    "partial_gap_no_source",
    "gap_open_public_context_only",
    "gap_open_nonoracle_context",
)


@dataclass(frozen=True)
class ObviousSpecTopic:
    topic_id: str
    board_topic: str
    reviewed_posture: str
    eligible_source_ids: tuple[str, ...] = ()
    nonoracle_source_ids: tuple[str, ...] = ()
    open_spec_gap_ids: tuple[str, ...] = ()
    public_claim_context_ids: tuple[str, ...] = ()
    areas: tuple[str, ...] = ()

    def to_dict(self) -> dict[str, Any]:
        payload = asdict(self)
        for field in (
            "eligible_source_ids",
            "nonoracle_source_ids",
            "open_spec_gap_ids",
            "public_claim_context_ids",
            "areas",
        ):
            payload[field] = list(payload[field])
        return payload


OBVIOUS_SPEC_TOPICS = (
    ObviousSpecTopic("P1A004_BYTECODE_FORMAT", "bytecode format", "source_present", ("SPEC_BYTECODE_FORMAT_001",), areas=("bytecode_vm",)),
    ObviousSpecTopic("P1A004_BYTECODE_VALIDATOR", "bytecode validator", "source_present", ("SPEC_BYTECODE_FORMAT_001",), areas=("bytecode_vm",)),
    ObviousSpecTopic("P1A004_VM_VALUE_SEMANTICS", "VM value semantics", "source_present", ("SPEC_VM_VALUE_SEMANTICS_001",), areas=("bytecode_vm",)),
    ObviousSpecTopic("P1A004_SCAN_CYCLE_LIFECYCLE", "scan-cycle lifecycle", "source_present", ("SPEC_RUNTIME_ENGINE_001", "SPEC_RUNTIME_SEMANTICS_001"), areas=("runtime_safety",)),
    ObviousSpecTopic("P1A004_STOP_SAFE_STATE", "stop/safe-state", "source_present", ("SPEC_RUNTIME_ENGINE_001", "SPEC_RUNTIME_SAFETY_FAIL_CLOSED_BOARD_001"), areas=("runtime_safety",)),
    ObviousSpecTopic("P1A004_RETAIN_RESTART", "retain/restart", "source_present", ("SPEC_IEC_DECISIONS_001", "SPEC_RUNTIME_ENGINE_001", "SPEC_RUNTIME_SAFETY_FAIL_CLOSED_BOARD_001", "SPEC_RUNTIME_SEMANTICS_001"), areas=("runtime_safety",)),
    ObviousSpecTopic("P1A004_PROTOCOL_STATUS_DISCOVERY", "protocol status/discovery", "source_present", ("SPEC_OPCUA_CLIENT_LIFECYCLE_DECISION_001", "SPEC_RUNTIME_ENGINE_001"), public_claim_context_ids=("PUBLIC_CLAIM_RUNTIME_WIRE_001",), areas=("protocols",)),
    ObviousSpecTopic("P1A004_HMI_API_UI", "HMI API/UI", "gap_open_partial", ("SPEC_RUNTIME_ENGINE_001",), open_spec_gap_ids=("SPEC_GAP_UI_STATUS_VOCABULARY_001",), public_claim_context_ids=("PUBLIC_CLAIM_RUNTIME_WIRE_001",), areas=("control_security", "hmi_ui")),
    ObviousSpecTopic("P1A004_SOURCE_TRANSFORMATIONS", "source transformations", "source_present", ("SPEC_BYTECODE_FORMAT_001", "SPEC_RUNTIME_SEMANTICS_001"), areas=("bytecode_vm",)),
    ObviousSpecTopic("P1A004_LSP_SYNC_POSITIONS_CANCELLATION", "LSP sync/positions/cancellation", "source_present", ("SPEC_LSP_CONTRACT_001",), areas=("editor_safety",)),
    ObviousSpecTopic("P1A004_DEBUG_DAP_FORCE_WRITE_RELEASE_LIFECYCLE", "debug/DAP force-write-release lifecycle", "source_present", ("SPEC_DEBUG_ADAPTER_001", "SPEC_RUNTIME_ENGINE_001"), areas=("control_security", "editor_safety", "runtime_safety")),
    ObviousSpecTopic("P1A004_CONTROL_RBAC_SECURITY", "control/RBAC/security", "source_present", ("SPEC_DEBUG_ADAPTER_001", "SPEC_RUNTIME_ENGINE_001"), areas=("control_security",)),
    ObviousSpecTopic("P1A004_PLCOPEN_IMPORT_EXPORT", "PLCopen import/export", "partial_source_no_gap", ("SPEC_PLCOPEN_IMPORT_DECISION_001",), areas=("plcopen_devtools",)),
    ObviousSpecTopic("P1A004_TEST_HARNESS_SIMULATION_SEMANTICS", "test-harness/simulation semantics", "nonoracle_context_only", nonoracle_source_ids=("SPEC_CONFORMANCE_CONTRACT_001",), public_claim_context_ids=("PUBLIC_CLAIM_BEHAVIOR_LOCKED_001",), areas=("verification",)),
    ObviousSpecTopic("P1A004_RUNTIME_PROJECT_HMI_CONFIG_SCHEMAS", "runtime/project/HMI config schemas", "unrepresented", areas=("hmi_ui", "runtime_safety")),
    ObviousSpecTopic("P1A004_CLI_CONTROL_SOCKET_SURFACES", "CLI and control-socket surfaces", "source_present", ("SPEC_RUNTIME_ENGINE_001",), areas=("control_security", "runtime_safety")),
    ObviousSpecTopic("P1A004_GPIO", "GPIO", "unrepresented", areas=("runtime_safety",)),
    ObviousSpecTopic("P1A004_RUNTIME_PERFORMANCE_BUDGETS", "runtime performance budgets", "unrepresented", areas=("runtime_safety",)),
    ObviousSpecTopic("P1A004_SUPPLY_CHAIN", "supply chain", "gap_open_public_context_only", open_spec_gap_ids=("SPEC_GAP_ARTIFACT_PROVENANCE_001", "SPEC_GAP_DEPENDENCY_AUDIT_POLICY_001"), public_claim_context_ids=("PUBLIC_CLAIM_SOURCE_BUILD_RUNTIME_001", "PUBLIC_CLAIM_SUPPORTED_PLATFORMS_001"), areas=("supply_chain_platform",)),
    ObviousSpecTopic("P1A004_PLATFORM_PACKAGE_BEHAVIOR", "platform/package behavior", "gap_open_public_context_only", open_spec_gap_ids=("SPEC_GAP_ARTIFACT_PROVENANCE_001", "SPEC_GAP_PLATFORM_SUPPORT_MATRIX_001", "SPEC_GAP_SOURCE_BUILD_PUBLIC_CLAIM_001"), public_claim_context_ids=("PUBLIC_CLAIM_SOURCE_BUILD_RUNTIME_001", "PUBLIC_CLAIM_SUPPORTED_PLATFORMS_001"), areas=("release", "supply_chain_platform")),
    ObviousSpecTopic("P1A004_RELEASE_PROOF", "release proof", "gap_open_nonoracle_context", nonoracle_source_ids=("SPEC_CONFORMANCE_CONTRACT_001",), open_spec_gap_ids=("SPEC_GAP_ARTIFACT_PROVENANCE_001", "SPEC_GAP_BEHAVIOR_LOCKED_PUBLIC_CLAIM_001", "SPEC_GAP_CONFORMANCE_PUBLICATION_001", "SPEC_GAP_HARDWARE_PUBLIC_CLAIM_001", "SPEC_GAP_PLATFORM_SUPPORT_MATRIX_001", "SPEC_GAP_RELEASE_VERSION_CHAIN_001", "SPEC_GAP_SOURCE_BUILD_PUBLIC_CLAIM_001"), public_claim_context_ids=("PUBLIC_CLAIM_BEHAVIOR_LOCKED_001", "PUBLIC_CLAIM_RUNTIME_WIRE_001", "PUBLIC_CLAIM_SOURCE_BUILD_RUNTIME_001", "PUBLIC_CLAIM_SUPPORTED_PLATFORMS_001"), areas=("release",)),
)


def is_spec_document_path(path: str) -> bool:
    """Return whether a normalized tracked path belongs to the reviewed corpus."""

    candidate = PurePosixPath(path)
    if candidate.suffix.lower() != ".md":
        return False
    if path.startswith(EVIDENCE_PLANE_PREFIX):
        return path in EVIDENCE_BACKED_SPEC_DOCUMENTS
    if path in ROOT_SPEC_DOCUMENTS:
        return True
    return bool(candidate.parts and candidate.parts[0] in {"docs", "conformance"})


def is_primary_public_path(path: str) -> bool:
    """Return whether a tracked path is an entry point in the rendered public corpus."""

    if path == "README.md":
        return True
    candidate = PurePosixPath(path)
    return (
        candidate.suffix.lower() == ".md"
        and len(candidate.parts) >= 3
        and candidate.parts[:2] == ("docs", "public")
    )


def is_text_path(path: str) -> bool:
    return PurePosixPath(path).suffix.lower() in TEXT_SUFFIXES
