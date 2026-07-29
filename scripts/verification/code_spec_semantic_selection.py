"""Live-reconstructed, recoverable acceptance of code-spec semantic reviews."""

from __future__ import annotations

import argparse
import json
import sys
from collections.abc import Callable, Mapping, Sequence
from dataclasses import dataclass
from pathlib import Path
from typing import Any

from .code_spec_refresh_projection import CODE_MAP_PATH
from .code_spec_refresh_selection import (
    reconstruct_live_refresh_proposal,
    validate_refresh_post_state,
)
from .code_spec_semantic_projection import (
    SemanticProjection,
    project_semantic_selection,
)
from .mapping_transaction import apply_mapping_transaction
from .quarantine_paths import quarantine_path


LiveReconstructor = Callable[[Path, bytes], Mapping[str, Any]]
PostValidator = Callable[[Path], Sequence[str]]


@dataclass(frozen=True)
class SemanticApplication:
    """Exact semantic projection and compare-and-swap preconditions."""

    projection: SemanticProjection
    replacements: dict[str, bytes]
    expected_originals: dict[str, bytes | None]
    manifest: dict[str, Any]
    manifest_bytes: bytes
    proposal_path: str
    selection_path: str
    output_path: str


def build_semantic_application(
    *,
    root: Path,
    proposal_path: Path,
    selection_path: Path,
    output_path: Path,
    reconstruct_live: LiveReconstructor | None = None,
) -> SemanticApplication:
    """Reconstruct live inputs and build a write-free semantic projection."""

    root = root.resolve()
    proposal_relative, proposal_resolved = quarantine_path(
        root, proposal_path, label="semantic proposal", must_exist=True
    )
    selection_relative, selection_resolved = quarantine_path(
        root, selection_path, label="semantic selection", must_exist=True
    )
    output_relative, output_resolved = quarantine_path(
        root, output_path, label="semantic application output", must_exist=False
    )
    if len({proposal_resolved, selection_resolved, output_resolved}) != 3:
        raise ValueError(
            "proposal, selection, and application output paths must differ"
        )
    if output_resolved.suffix != ".json":
        raise ValueError("semantic application output must be JSON")

    proposal_bytes = proposal_resolved.read_bytes()
    selection_bytes = selection_resolved.read_bytes()
    reconstruct = reconstruct_live or reconstruct_live_refresh_proposal
    live_reconstruction = dict(reconstruct(root, proposal_bytes))
    if not live_reconstruction:
        raise ValueError("live semantic reconstruction returned no bindings")

    code_map_path = root / CODE_MAP_PATH
    code_map_bytes = code_map_path.read_bytes()
    projection = project_semantic_selection(
        proposal_bytes=proposal_bytes,
        selection_bytes=selection_bytes,
        code_map_bytes=code_map_bytes,
    )
    manifest = {
        **projection.manifest,
        "kind": "p18_code_spec_semantic_acceptance_application",
        "proposal_path": proposal_relative,
        "selection_path": selection_relative,
        "application_path": output_relative,
        "live_reconstruction": live_reconstruction,
        "canonical_replacement_paths": sorted(projection.replacements),
    }
    manifest_bytes = (
        json.dumps(manifest, indent=2, sort_keys=True, ensure_ascii=False)
        + "\n"
    ).encode()
    expected_originals: dict[str, bytes | None] = {
        CODE_MAP_PATH: code_map_bytes,
        proposal_relative: proposal_bytes,
        selection_relative: selection_bytes,
        output_relative: (
            output_resolved.read_bytes() if output_resolved.is_file() else None
        ),
    }
    replacements = {
        **projection.replacements,
        output_relative: manifest_bytes,
    }
    return SemanticApplication(
        projection=projection,
        replacements=replacements,
        expected_originals=expected_originals,
        manifest=manifest,
        manifest_bytes=manifest_bytes,
        proposal_path=proposal_relative,
        selection_path=selection_relative,
        output_path=output_relative,
    )


def apply_semantic_application(
    *,
    root: Path,
    proposal_path: Path,
    selection_path: Path,
    output_path: Path,
    application: SemanticApplication,
    reconstruct_live: LiveReconstructor | None = None,
    post_validate: PostValidator | None = None,
) -> None:
    """Rebuild, compare, and atomically apply only the exact projection."""

    root = root.resolve()
    rebuilt = build_semantic_application(
        root=root,
        proposal_path=proposal_path,
        selection_path=selection_path,
        output_path=output_path,
        reconstruct_live=reconstruct_live,
    )
    if rebuilt != application:
        raise ValueError(
            "semantic application differs from canonical rebuilt inputs"
        )
    validate_after = post_validate or validate_refresh_post_state

    def validate() -> list[str]:
        failures = []
        for relative, expected in rebuilt.replacements.items():
            path = root / relative
            if not path.is_file() or path.read_bytes() != expected:
                failures.append(f"{relative}: atomic replacement differs")
        failures.extend(validate_after(root))
        return failures

    apply_mapping_transaction(
        root=root,
        replacements=rebuilt.replacements,
        expected_originals=rebuilt.expected_originals,
        validate=validate,
    )


def main(argv: Sequence[str] | None = None) -> int:
    parser = argparse.ArgumentParser(
        description="Apply a fully reviewed code-spec semantic queue."
    )
    parser.add_argument("--root", type=Path, default=Path("."))
    parser.add_argument("--proposal", type=Path, required=True)
    parser.add_argument("--selection", type=Path, required=True)
    parser.add_argument("--output", type=Path, required=True)
    mode = parser.add_mutually_exclusive_group(required=True)
    mode.add_argument("--check", action="store_true")
    mode.add_argument("--apply", action="store_true")
    args = parser.parse_args(argv)
    try:
        application = build_semantic_application(
            root=args.root,
            proposal_path=args.proposal,
            selection_path=args.selection,
            output_path=args.output,
        )
        if args.apply:
            apply_semantic_application(
                root=args.root,
                proposal_path=args.proposal,
                selection_path=args.selection,
                output_path=args.output,
                application=application,
            )
    except (OSError, ValueError) as error:
        print(f"code-spec semantic acceptance failed: {error}", file=sys.stderr)
        return 1
    manifest = application.manifest
    action = "applied" if args.apply else "valid"
    print(
        "code-spec semantic acceptance "
        f"{action}: groups={manifest['semantic_review_groups']} "
        f"cross_area={manifest['accepted_cross_area_groups']} "
        f"removed_tests={manifest['removed_mapped_test_groups']} "
        f"after={manifest['code_map_after_sha256']}"
    )
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
