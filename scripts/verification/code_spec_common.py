"""Shared Phase 17 code-to-spec fact identities and closed vocabularies."""

from __future__ import annotations

import hashlib
from dataclasses import dataclass
from pathlib import PurePosixPath


CODE_ID_PATTERN = r"^CODE_[0-9A-F]{64}$"
DIGEST_PATTERN = r"^sha256:[0-9a-f]{64}$"

SOURCE_KINDS = frozenset({"rust", "typescript", "javascript", "python", "shell"})
ITEM_KINDS = frozenset(
    {
        "rust_function",
        "rust_fuzz_target",
        "rust_method",
        "rust_trait_method",
        "rust_public_type",
        "rust_public_constant",
        "rust_registration",
        "rust_unsafe_site",
        "typescript_function",
        "typescript_method",
        "typescript_registration",
        "typescript_export",
        "typescript_asset",
        "javascript_function",
        "javascript_method",
        "javascript_registration",
        "javascript_export",
        "javascript_asset",
        "python_function",
        "python_method",
        "python_entrypoint",
        "shell_entrypoint",
    }
)
DISPOSITIONS = frozenset(
    {
        "specified",
        "spec_gap",
        "inherits_spec",
        "non_behavioral",
        "generated",
        "vendored_external",
    }
)
NON_BEHAVIORAL_REASONS = frozenset(
    {
        "pure_data_access",
        "mechanical_adapter",
        "serialization_shape",
        "build_scaffolding",
        "test_support_in_production_file",
    }
)
RISK_SIGNALS = frozenset(
    {
        "host_time",
        "randomness",
        "filesystem_or_environment",
        "network_io",
        "subprocess_or_thread",
        "unsafe_or_ffi",
        "state_mutation",
        "output_or_safe_state_commit",
        "authorization",
        "fault_mapping",
        "protocol_response_or_status",
        "broad_error_suppression",
        "persistence_retain_reload_restart",
    }
)


@dataclass(frozen=True)
class CodeFact:
    """Canonical source-derived identity for one behavior-bearing code unit."""

    code_id: str
    source_kind: str
    path: str
    qualified_symbol: str
    item_kind: str
    signature: str
    signature_digest: str
    body_digest: str
    owner: str
    area: str
    visibility: str = "private"
    risk_signals: tuple[str, ...] = ()
    line: int | None = None


def stable_code_id(
    *,
    source_kind: str,
    path: str,
    qualified_symbol: str,
    item_kind: str,
    signature: str,
) -> str:
    """Return the anchored stable Phase 17 identity for a code fact."""

    normalized_path = normalize_repo_path(path)
    identity = "\0".join(
        (source_kind, normalized_path, qualified_symbol, item_kind, signature)
    ).encode("utf-8")
    return "CODE_" + hashlib.sha256(identity).hexdigest().upper()


def sha256_digest(content: bytes) -> str:
    """Return a lower-case, tagged digest for exact source bytes."""

    return "sha256:" + hashlib.sha256(content).hexdigest()


def normalize_repo_path(path: str) -> str:
    """Normalize a repository-relative path and reject escapes or absolutes."""

    normalized = path.replace("\\", "/")
    parsed = PurePosixPath(normalized)
    if parsed.is_absolute() or not normalized or any(part == ".." for part in parsed.parts):
        raise ValueError(f"code fact path must stay repository-relative: {path!r}")
    parts = tuple(part for part in parsed.parts if part not in {"", "."})
    if not parts:
        raise ValueError(f"code fact path must name a file: {path!r}")
    return PurePosixPath(*parts).as_posix()
