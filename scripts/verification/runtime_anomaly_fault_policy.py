"""Source-level guard for Phase 8 production fault hooks."""

from __future__ import annotations

import re
import tomllib
from pathlib import Path

from .test_catalog_rust import sanitize_rust


RUNTIME_MANIFEST = "crates/trust-runtime/Cargo.toml"
RUNTIME_SOURCE_ROOT = "crates/trust-runtime/src"
FAULT_HOOK_TOKEN_RE = re.compile(
    r"(?:fault[_-]?(?:inject(?:ion)?|toggle|hook)|"
    r"(?:inject|toggle|enable|simulate)[_-]?fault|"
    r"chaos[_-]?(?:mode|hook|inject(?:ion)?))",
    re.IGNORECASE,
)
PUBLIC_SYMBOL_RE = re.compile(
    r"\bpub(?:\([^)]*\))?\s+"
    r"(?:(?:async|unsafe)\s+)*"
    r"(?:fn|struct|enum|trait|const|static|type)\s+"
    r"(?P<name>[A-Za-z_][A-Za-z0-9_]*)"
)


def validate_runtime_anomaly_fault_policy(root: Path) -> list[str]:
    """Reject production feature flags and public symbols shaped as fault hooks."""

    failures: list[str] = []
    manifest = root / RUNTIME_MANIFEST
    try:
        data = tomllib.loads(manifest.read_text())
    except Exception as exc:
        failures.append(f"runtime fault policy cannot read {RUNTIME_MANIFEST}: {exc}")
    else:
        features = data.get("features", {})
        if not isinstance(features, dict):
            failures.append("runtime fault policy requires Cargo features to be a table")
        else:
            for feature in sorted(features):
                if isinstance(feature, str) and FAULT_HOOK_TOKEN_RE.search(feature):
                    failures.append(
                        "runtime defines production fault-hook feature "
                        f"{feature!r}; a reviewed design is required before adding it"
                    )

    source_root = root / RUNTIME_SOURCE_ROOT
    if not source_root.is_dir():
        failures.append(f"runtime fault policy source root is missing: {RUNTIME_SOURCE_ROOT}")
        return sorted(set(failures))
    for path in sorted(source_root.rglob("*.rs")):
        if path.is_symlink() or not path.is_file():
            failures.append(
                "runtime fault policy source must be a regular non-symlink file: "
                f"{path.relative_to(root).as_posix()}"
            )
            continue
        source = sanitize_rust(path.read_text())
        for match in PUBLIC_SYMBOL_RE.finditer(source):
            name = match.group("name")
            if FAULT_HOOK_TOKEN_RE.search(name):
                failures.append(
                    "runtime exposes public production fault-hook symbol "
                    f"{name!r} in {path.relative_to(root).as_posix()}; "
                    "a reviewed design is required before adding it"
                )
    return sorted(set(failures))
