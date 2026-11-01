"""Fail-closed companion checks for behavior-changing production diffs (specification 30, "Owning-spec companion
contract")."""

from __future__ import annotations

from collections.abc import Sequence
from pathlib import Path, PurePosixPath


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


def validate_direct_change_contract(
    changed_files: Sequence[str], specifications: Sequence[str] = (), root: Path = Path(".")
) -> list[str]:
    """Require direct spec and native-test companions for production changes.

    The written specification is changed in the diff, or named by the `Specification: <specification> <section>` trailer of
    one of the change's commits (`specifications`, the trailers' values) when the change follows a section that it does not
    change. Every named section must exist in its specification under `root`, whatever the diff holds.
    """
    paths = tuple(_normalize(path) for path in changed_files)
    named: set[str] = set()
    failures: list[str] = []
    for value in specifications:
        specification, failure = _named_specification(value, root)
        if failure:
            failures.append(failure)
        else:
            named.add(specification)
    if not any(_is_production_source(path) for path in paths):
        return sorted(failures)

    if not named and not any(_is_written_specification(path) for path in paths):
        failures.append(
            "behavior-changing production paths require a changed written specification, or a "
            "`Specification: <specification> <section>` commit trailer naming the section they follow"
        )
    if not any(_is_native_test(path) for path in paths):
        failures.append(
            "behavior-changing production paths require a changed native executable test"
        )
    for owner, prefixes, specification in OWNING_SPEC_ROUTES:
        if (
            any(path.startswith(prefixes) for path in paths)
            and specification not in paths
            and specification not in named
        ):
            failures.append(
                f"{owner} production paths require the owning specification {specification}"
            )
    return sorted(failures)


def _named_specification(value: str, root: Path) -> tuple[str, str | None]:
    """The written specification that a trailer value `<specification> <section>` names, or why it names none. The
    specification is a number (`35`, the one file `docs/specs/35-*.md`) or a path; the section is text that the file holds,
    such as `B3.4` or a heading."""
    name, _, section = value.strip().partition(" ")
    section = section.strip()
    prefix = f"Specification trailer {value.strip()!r}"
    if not name or not section:
        return "", f"{prefix} must name a specification and its section"
    if name.isdigit():
        numbered = sorted(
            path.relative_to(root).as_posix()
            for path in (root / "docs/specs").glob("*.md")
            if path.name.split("-", 1)[0].isdigit() and int(path.name.split("-", 1)[0]) == int(name)
        )
        if len(numbered) != 1:
            return "", f"{prefix}: docs/specs has no single specification numbered {name}"
        specification = numbered[0]
    else:
        specification = _normalize(name)
        parts = PurePosixPath(specification).parts
        if (
            ".." in parts
            or specification.startswith("/")
            or not _is_written_specification(specification)
            or not (root / specification).is_file()
        ):
            return "", f"{prefix}: {specification} is not a written specification"
    if section not in (root / specification).read_text(encoding="utf-8"):
        return "", f"{prefix}: {specification} has no section {section!r}"
    return specification, None


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
