"""Apply reviewed authority selections to existing code-spec gap rows."""

from __future__ import annotations

import argparse
import hashlib
import json
import re
import sys
import tomllib
from collections.abc import Mapping, Sequence
from dataclasses import dataclass, field
from datetime import date
from pathlib import Path
from typing import Any

from .code_spec_fragments import source_fragment_exists
from .code_spec_authority_contract import (
    catalog_tests_by_discovery,
    oracle_eligible,
    validate_discoveries_and_tests,
    validate_selected_invariants,
)
from .code_spec_authority_review import build_authority_review_selection
from .mapping_transaction import apply_mapping_transaction
from .quarantine_paths import QUARANTINE_ROOT, quarantine_path
from .code_spec_authority_selection_support import (
    MappingBlock as _MappingBlock,
    capture_review_provenance_inputs as _capture_review_provenance_inputs,
    capture_spec_document as _capture_spec_document,
    canonical_file as _canonical_file,
    closed_string_array as _closed_string_array,
    digest as _digest,
    load_indexed_toml_captured as _load_indexed_toml_captured,
    load_invariants as _load_invariants,
    load_toml_captured as _load_toml_captured,
    mapping_blocks as _mapping_blocks,
    object_array as _object_array,
    parse_mapping_block as _parse_mapping_block,
    render_selected_block as _render_selected_block,
    replace_mapping_blocks as _replace_mapping_blocks,
    required_string as _required_string,
    rooted_path as _rooted_path,
    test_id_for_discovery as _test_id_for_discovery,
    unique_by_id as _unique_by_id,
    validate_only_semantic_transition as _validate_only_semantic_transition,
)


CODE_MAP_PATH = "verification/code-spec-map.toml"
SPEC_SOURCES_PATH = "verification/spec-sources.toml"
TEST_CATALOG_PATH = "verification/test-catalog.toml"
DENOMINATOR_PATH = "verification/test-catalog-denominator.toml"
INVARIANT_ROOT = "verification/invariants"
ROOT_FIELDS = {
    "schema_version",
    "reviewed_date",
    "review_provenance",
    "rows",
}
ROW_FIELDS = {"code_id", "spec_ref", "invariant_ids", "discovery_ids"}
REVIEW_PROVENANCE_FIELDS = {
    "compiler",
    "proposal_path",
    "proposal_digest",
    "review_card_path",
    "review_card_digest",
    "human_selection_path",
    "human_selection_digest",
    "reviewer_ids",
    "candidate_provenance",
}
CODE_ID_RE = re.compile(r"^CODE_[0-9A-F]{64}$")
DISCOVERY_ID_RE = re.compile(r"^DISC_([0-9A-F]{20})$")
INVARIANT_ID_RE = re.compile(r"^[A-Z0-9_]+$")
SPEC_REF_RE = re.compile(r"^([A-Z0-9_]+)#([A-Za-z0-9._:-]+)$")
MAPPING_BLOCK_RE = re.compile(br"(?m)^\[\[mappings\]\][ \t]*\r?\n")
SHA256_PREFIX = "sha256:"


@dataclass(frozen=True)
class AuthoritySelectionApplication:
    """Deterministic replacements and manifest for one reviewed selection."""

    replacements: dict[str, bytes]
    quarantine_replacements: dict[str, bytes]
    manifest: dict[str, Any]
    code_map_before_bytes: bytes
    projected_code_map_bytes: bytes
    selection_path: str = ""
    output_path: str = ""
    expected_originals: dict[str, bytes | None] = field(default_factory=dict)


def build_authority_selection_application(
    *,
    root: Path,
    selection_path: Path,
    output_path: Path,
) -> AuthoritySelectionApplication:
    """Build the reviewed code-map transition without writing it."""

    root = root.resolve()
    selection_relative, selection_resolved = quarantine_path(
        root, selection_path, label="selection", must_exist=True
    )
    output_relative, output_resolved = quarantine_path(
        root, output_path, label="output", must_exist=False
    )
    if selection_resolved == output_resolved:
        raise ValueError("selection and output paths must differ")
    if output_resolved.suffix != ".json":
        raise ValueError("authority-selection output must be JSON")
    projected_relative, projected_resolved = quarantine_path(
        root,
        output_resolved.with_name(
            output_resolved.stem + ".projected-code-spec-map.toml"
        ),
        label="projected code map output",
        must_exist=False,
    )
    if projected_resolved in {selection_resolved, output_resolved}:
        raise ValueError(
            "selection, manifest, and projected code-map paths must differ"
        )

    selection_bytes = selection_resolved.read_bytes()
    selection = _load_selection(selection_resolved, selection_bytes)
    reviewed_date, selected_rows = _validate_selection(root, selection)
    expected_originals: dict[str, bytes | None] = {
        selection_relative: selection_bytes
    }
    _capture_review_provenance_inputs(
        root, selection["review_provenance"], expected_originals
    )

    code_map_path = _canonical_file(
        root, root / CODE_MAP_PATH, "code-spec map"
    )
    code_map_before = code_map_path.read_bytes()
    expected_originals[CODE_MAP_PATH] = code_map_before
    blocks = _mapping_blocks(code_map_before)
    mappings = _unique_by_id(
        [block.row for block in blocks], "code_id", "code-spec map"
    )
    blocks_by_id = {block.code_id: block for block in blocks}

    spec_sources = _load_indexed_toml_captured(
        root,
        root / SPEC_SOURCES_PATH,
        "spec_sources",
        "spec source",
        expected_originals,
    )
    invariants = _load_invariants(root, expected_originals)
    tests = _load_indexed_toml_captured(
        root,
        root / TEST_CATALOG_PATH,
        "tests",
        "test catalog",
        expected_originals,
    )
    tests_by_discovery = catalog_tests_by_discovery(tests.values())
    denominator = _load_toml_captured(
        root,
        root / DENOMINATOR_PATH,
        "test catalog denominator",
        expected_originals,
    )
    denominator_reviews = _unique_by_id(
        _object_array(denominator.get("reviews"), "denominator reviews"),
        "discovery_id",
        "denominator",
    )

    rendered_by_code_id: dict[str, bytes] = {}
    manifest_rows: list[dict[str, Any]] = []
    missing_catalog_test_ids: set[str] = set()
    missing_invariant_backlinks: set[str] = set()
    selected_code_ids: set[str] = set()
    for selection_row in sorted(
        selected_rows, key=lambda row: str(row["code_id"])
    ):
        code_id = _required_string(selection_row, "code_id", "selection row")
        if code_id in selected_code_ids:
            raise ValueError(f"selection duplicates code_id {code_id}")
        selected_code_ids.add(code_id)
        mapping = mappings.get(code_id)
        block = blocks_by_id.get(code_id)
        if mapping is None or block is None:
            raise ValueError(f"unknown code_id {code_id}")
        if mapping.get("disposition") != "spec_gap":
            raise ValueError(f"{code_id} must currently be spec_gap")
        if (
            not isinstance(mapping.get("spec_gap_ref"), str)
            or not mapping.get("spec_gap_ref")
        ):
            raise ValueError(f"{code_id} spec_gap row lacks spec_gap_ref")
        for forbidden in ("spec_ref", "invariant_ids", "test_ids"):
            if forbidden in mapping:
                raise ValueError(
                    f"{code_id} spec_gap row contains forbidden {forbidden}"
                )

        spec_ref = _required_string(selection_row, "spec_ref", code_id)
        spec_match = SPEC_REF_RE.fullmatch(spec_ref)
        if spec_match is None:
            raise ValueError(f"{code_id} requires exact spec_ref fragment")
        source_id, fragment = spec_match.groups()
        source = spec_sources.get(source_id)
        if not oracle_eligible(source):
            raise ValueError(
                f"{code_id} spec source is not active and oracle-eligible"
            )
        area = _required_string(mapping, "area", code_id)
        if source.get("area") != area:
            raise ValueError(f"{code_id} spec source area does not match code area")
        if not source_fragment_exists(source, fragment, root=root):
            raise ValueError(f"{code_id} spec_ref fragment does not exist")
        _capture_spec_document(
            root,
            source,
            expected_originals,
            code_id=code_id,
        )

        invariant_ids = _closed_string_array(
            selection_row.get("invariant_ids"),
            f"{code_id} invariant_ids",
            pattern=INVARIANT_ID_RE,
        )
        discovery_ids = _closed_string_array(
            selection_row.get("discovery_ids"),
            f"{code_id} discovery_ids",
            pattern=DISCOVERY_ID_RE,
        )
        selected_invariants = validate_selected_invariants(
            code_id=code_id,
            invariant_ids=invariant_ids,
            invariants=invariants,
            area=area,
            spec_ref=spec_ref,
            source_id=source_id,
        )
        test_ids = [
            _test_id_for_discovery(discovery_id)
            for discovery_id in discovery_ids
        ]
        missing_tests, missing_backlinks = validate_discoveries_and_tests(
            code_id=code_id,
            discovery_ids=discovery_ids,
            test_ids=test_ids,
            denominator_reviews=denominator_reviews,
            tests=tests,
            tests_by_discovery=tests_by_discovery,
            invariant_ids=invariant_ids,
            invariants=selected_invariants,
            area=area,
            spec_ref=spec_ref,
        )
        missing_catalog_test_ids.update(missing_tests)
        missing_invariant_backlinks.update(missing_backlinks)

        updated = _render_selected_block(
            block.content,
            spec_ref=spec_ref,
            invariant_ids=invariant_ids,
            test_ids=test_ids,
            reviewed_date=reviewed_date,
        )
        updated_row = _parse_mapping_block(updated)
        _validate_only_semantic_transition(
            code_id=code_id,
            before=mapping,
            after=updated_row,
            spec_ref=spec_ref,
            invariant_ids=invariant_ids,
            test_ids=test_ids,
            reviewed_date=reviewed_date,
        )
        rendered_by_code_id[code_id] = updated
        manifest_rows.append(
            {
                "code_id": code_id,
                "spec_ref": spec_ref,
                "invariant_ids": invariant_ids,
                "discovery_ids": discovery_ids,
                "test_ids": test_ids,
                "before_row_sha256": _digest(block.content),
                "after_row_sha256": _digest(updated),
            }
        )

    code_map_after = _replace_mapping_blocks(
        code_map_before, blocks, rendered_by_code_id
    )
    manifest = {
        "schema_version": 1,
        "kind": "reviewed_code_spec_authority_selection",
        "selection_path": selection_relative,
        "selection_sha256": _digest(selection_bytes),
        "reviewed_date": reviewed_date,
        "review_provenance": dict(selection["review_provenance"]),
        "code_map_path": CODE_MAP_PATH,
        "code_map_before_sha256": _digest(code_map_before),
        "code_map_after_sha256": _digest(code_map_after),
        "projected_code_map_path": projected_relative,
        "canonical_apply_ready": not missing_catalog_test_ids
        and not missing_invariant_backlinks,
        "missing_catalog_test_ids": sorted(missing_catalog_test_ids),
        "missing_invariant_backlinks": sorted(missing_invariant_backlinks),
        "row_count": len(manifest_rows),
        "rows": manifest_rows,
    }
    manifest_bytes = (
        json.dumps(manifest, indent=2, sort_keys=True, ensure_ascii=False) + "\n"
    ).encode()
    quarantine_replacements = {
        output_relative: manifest_bytes,
        projected_relative: code_map_after,
    }
    for relative in quarantine_replacements:
        target = root / relative
        expected_originals[relative] = (
            target.read_bytes() if target.is_file() else None
        )
    return AuthoritySelectionApplication(
        replacements={
            CODE_MAP_PATH: code_map_after,
            **quarantine_replacements,
        },
        quarantine_replacements=quarantine_replacements,
        manifest=manifest,
        code_map_before_bytes=code_map_before,
        projected_code_map_bytes=code_map_after,
        selection_path=selection_relative,
        output_path=output_relative,
        expected_originals=expected_originals,
    )


def apply_authority_selection(
    *,
    root: Path,
    selection_path: Path,
    output_path: Path,
) -> AuthoritySelectionApplication:
    """Atomically apply a validated code map and its quarantine manifest."""

    root = root.resolve()
    application = build_authority_selection_application(
        root=root,
        selection_path=selection_path,
        output_path=output_path,
    )
    apply_authority_selection_application(
        root=root,
        selection_path=selection_path,
        output_path=output_path,
        application=application,
    )
    return application


def apply_authority_selection_application(
    *,
    root: Path,
    selection_path: Path,
    output_path: Path,
    application: AuthoritySelectionApplication,
) -> None:
    """Rebuild and apply only an exact canonical authority projection."""

    root = root.resolve()
    rebuilt = build_authority_selection_application(
        root=root,
        selection_path=selection_path,
        output_path=output_path,
    )
    if application != rebuilt:
        raise ValueError(
            "authority application differs from canonical rebuilt inputs"
        )
    if not rebuilt.manifest["canonical_apply_ready"]:
        raise ValueError(
            "standalone apply requires canonical catalog rows and invariant "
            "backlinks; use the projected code-map bytes in a combined atomic "
            "review transaction"
        )

    def validate() -> list[str]:
        failures = []
        for relative, expected in rebuilt.replacements.items():
            target = root / relative
            if not target.is_file() or target.read_bytes() != expected:
                failures.append(f"{relative}: atomic replacement differs")
        return failures

    apply_mapping_transaction(
        root=root,
        replacements=rebuilt.replacements,
        expected_originals=rebuilt.expected_originals,
        validate=validate,
    )


def project_authority_selection(
    *,
    root: Path,
    selection_path: Path,
    output_path: Path,
) -> AuthoritySelectionApplication:
    """Atomically write only quarantine plan artifacts, never canonical metadata."""

    root = root.resolve()
    application = build_authority_selection_application(
        root=root,
        selection_path=selection_path,
        output_path=output_path,
    )

    def validate() -> list[str]:
        failures = []
        for relative, expected in application.quarantine_replacements.items():
            target = root / relative
            if not target.is_file() or target.read_bytes() != expected:
                failures.append(f"{relative}: projected artifact differs")
        if (root / CODE_MAP_PATH).read_bytes() != application.code_map_before_bytes:
            failures.append(f"{CODE_MAP_PATH}: project mode changed canonical map")
        return failures

    apply_mapping_transaction(
        root=root,
        replacements=application.quarantine_replacements,
        expected_originals=application.expected_originals,
        validate=validate,
    )
    return application


def main(argv: Sequence[str] | None = None) -> int:
    parser = argparse.ArgumentParser(
        description="Apply a reviewed code-spec authority selection"
    )
    parser.add_argument("--root", default=".")
    parser.add_argument("--selection", required=True)
    parser.add_argument("--output", required=True)
    mode = parser.add_mutually_exclusive_group(required=True)
    mode.add_argument("--check", action="store_true")
    mode.add_argument("--project", action="store_true")
    mode.add_argument("--apply", action="store_true")
    args = parser.parse_args(argv)
    root = Path(args.root).resolve()
    selection_path = _rooted_path(root, args.selection)
    output_path = _rooted_path(root, args.output)
    try:
        if args.apply:
            application = apply_authority_selection(
                root=root,
                selection_path=selection_path,
                output_path=output_path,
            )
            verb = "applied"
        elif args.project:
            application = project_authority_selection(
                root=root,
                selection_path=selection_path,
                output_path=output_path,
            )
            verb = "projected"
        else:
            application = build_authority_selection_application(
                root=root,
                selection_path=selection_path,
                output_path=output_path,
            )
            verb = "valid"
    except (OSError, UnicodeError, ValueError, json.JSONDecodeError) as error:
        print(
            f"code-spec authority selection failed: {error}",
            file=sys.stderr,
        )
        return 1
    print(
        f"code-spec authority selection {verb}: "
        f"rows={application.manifest['row_count']} "
        f"canonical_apply_ready="
        f"{str(application.manifest['canonical_apply_ready']).lower()} "
        f"before={application.manifest['code_map_before_sha256']} "
        f"after={application.manifest['code_map_after_sha256']}"
    )
    return 0


def _load_selection(path: Path, content: bytes) -> dict[str, Any]:
    if path.suffix == ".json":
        value = json.loads(content)
    elif path.suffix == ".toml":
        value = tomllib.loads(content.decode())
    else:
        raise ValueError("authority selection must be JSON or TOML")
    if not isinstance(value, dict):
        raise ValueError("authority selection root must be an object")
    return value


def _validate_selection(
    root: Path,
    selection: Mapping[str, Any],
) -> tuple[str, list[Mapping[str, Any]]]:
    if set(selection) != ROOT_FIELDS:
        raise ValueError("authority selection root fields drift")
    if selection.get("schema_version") != 1:
        raise ValueError("authority selection schema_version must equal 1")
    reviewed_date = _required_string(
        selection, "reviewed_date", "authority selection"
    )
    provenance = selection.get("review_provenance")
    if not isinstance(provenance, Mapping) or set(provenance) != (
        REVIEW_PROVENANCE_FIELDS
    ):
        raise ValueError("authority selection review provenance fields drift")
    if provenance.get("compiler") != "numbered_code_spec_authority_review_v1":
        raise ValueError("authority selection requires numbered review compiler")
    for field in (
        "proposal_path",
        "review_card_path",
        "human_selection_path",
    ):
        value = provenance.get(field)
        if (
            not isinstance(value, str)
            or not value.startswith(QUARANTINE_ROOT + "/")
            or ".." in Path(value).parts
        ):
            raise ValueError(
                f"authority selection provenance {field} is malformed"
            )
    for field in (
        "proposal_digest",
        "review_card_digest",
        "human_selection_digest",
    ):
        value = provenance.get(field)
        if (
            not isinstance(value, str)
            or re.fullmatch(r"sha256:[0-9a-f]{64}", value) is None
        ):
            raise ValueError(
                f"authority selection provenance {field} is malformed"
            )
    _closed_string_array(
        provenance.get("reviewer_ids"),
        "authority selection reviewer_ids",
        pattern=None,
    )
    try:
        if date.fromisoformat(reviewed_date).isoformat() != reviewed_date:
            raise ValueError
    except ValueError as error:
        raise ValueError(
            "authority selection reviewed_date must be YYYY-MM-DD"
        ) from error
    rows = _object_array(selection.get("rows"), "authority selection rows")
    if not rows:
        raise ValueError("authority selection rows must be non-empty")
    for index, row in enumerate(rows):
        if set(row) != ROW_FIELDS:
            raise ValueError(f"selection row fields drift at index {index}")
        code_id = row.get("code_id")
        if not isinstance(code_id, str) or CODE_ID_RE.fullmatch(code_id) is None:
            raise ValueError(f"selection row {index} has malformed code_id")
    rebuilt = build_authority_review_selection(
        root=root,
        proposal_path=root / str(provenance["proposal_path"]),
        review_card_path=root / str(provenance["review_card_path"]),
        human_selection_path=root / str(provenance["human_selection_path"]),
    )
    if dict(selection) != rebuilt:
        raise ValueError(
            "authority selection does not match compiled review inputs"
        )
    return reviewed_date, rows


if __name__ == "__main__":
    raise SystemExit(main())
