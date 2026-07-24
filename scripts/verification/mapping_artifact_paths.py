"""Fail-closed output confinement for mapping pilot artifacts."""

from __future__ import annotations

from pathlib import Path

from .report_input_contract import resolve_report_output_path


MAPPING_ARTIFACT_ROOT = Path("target/gate-artifacts/verification")


def resolve_mapping_artifact_output(
    root: Path,
    value: Path,
    label: str,
) -> Path:
    """Resolve one output strictly below the mapping artifact quarantine."""

    root = root.resolve()
    if value.is_absolute():
        try:
            relative = value.relative_to(root)
        except ValueError as error:
            raise ValueError(
                f"{label} must stay inside the mapping artifact quarantine"
            ) from error
    else:
        relative = value
    _, output = resolve_report_output_path(root, relative, label)
    artifact_root = (root / MAPPING_ARTIFACT_ROOT).resolve()
    try:
        output.relative_to(artifact_root)
    except ValueError as error:
        raise ValueError(
            f"{label} must stay inside the mapping artifact quarantine"
        ) from error
    if output == artifact_root:
        raise ValueError(f"{label} must name a file in the artifact quarantine")
    return output
