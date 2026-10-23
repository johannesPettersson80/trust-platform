"""Fail-closed companion checks for behavior-changing production diffs."""

from __future__ import annotations

from collections.abc import Sequence


PRODUCTION_PREFIXES = (
    ".github/workflows/",
    "crates/",
    "editors/vscode/src/",
    "scripts/",
    "xtask/",
)
PRODUCTION_PATHS = {"Cargo.toml", "Cargo.lock", "justfile"}
PRODUCTION_SUFFIXES = (
    ".c",
    ".cjs",
    ".cpp",
    ".css",
    ".h",
    ".hpp",
    ".html",
    ".js",
    ".json",
    ".mjs",
    ".ps1",
    ".py",
    ".rs",
    ".scss",
    ".sh",
    ".st",
    ".toml",
    ".ts",
    ".tsx",
    ".yaml",
    ".yml",
)
TEST_PATH_MARKERS = ("/tests/", "/test/suite/", "/conformance/", "/fuzz/")
TEST_SUFFIXES = (
    "_test.rs",
    "_tests.rs",
    "_test.py",
    "_tests.py",
    ".test.ts",
    ".spec.ts",
)
SPEC_PREFIXES = ("docs/specs/",)
SPEC_PATHS = {
    "conformance/contract.md",
    "docs/IEC_DECISIONS.md",
    "docs/IEC_DEVIATIONS.md",
    "docs/PLCOPEN_DECISIONS.md",
    "docs/PLCOPEN_DEVIATIONS.md",
    "docs/internal/testing/checklists/plc-verification-program-checklist.md",
}
OWNING_SPEC_ROUTES = (
    (
        "MQTT",
        (
            "crates/trust-runtime/src/io/mqtt/",
            "crates/trust-runtime/src/io/mqtt_tag_mapping.rs",
        ),
        "docs/specs/32-mqtt-io.md",
    ),
)


def validate_direct_change_contract(changed_files: Sequence[str]) -> list[str]:
    """Require direct spec and native-test companions for production changes."""
    paths = tuple(_normalize(path) for path in changed_files)
    if not any(_is_production_source(path) for path in paths):
        return []

    failures: list[str] = []
    if not any(_is_written_specification(path) for path in paths):
        failures.append(
            "behavior-changing production paths require a changed written specification"
        )
    if not any(_is_native_test(path) for path in paths):
        failures.append(
            "behavior-changing production paths require a changed native executable test"
        )
    for owner, prefixes, specification in OWNING_SPEC_ROUTES:
        if any(path.startswith(prefixes) for path in paths) and specification not in paths:
            failures.append(
                f"{owner} production paths require the owning specification {specification}"
            )
    return sorted(failures)


def _normalize(path: str) -> str:
    normalized = path.strip().replace("\\", "/")
    while normalized.startswith("./"):
        normalized = normalized[2:]
    return normalized


def _is_production_source(path: str) -> bool:
    return (
        path in PRODUCTION_PATHS
        or (
            path.startswith(PRODUCTION_PREFIXES)
            and path.endswith(PRODUCTION_SUFFIXES)
        )
    ) and not _is_native_test(path)


def _is_native_test(path: str) -> bool:
    marked = f"/{path}"
    return any(marker in marked for marker in TEST_PATH_MARKERS) or path.endswith(
        TEST_SUFFIXES
    )


def _is_written_specification(path: str) -> bool:
    return path.startswith(SPEC_PREFIXES) or path in SPEC_PATHS
