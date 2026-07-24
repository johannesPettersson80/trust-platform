"""Compile numbered human review into an internal code-spec authority selection."""

from __future__ import annotations

import argparse
import hashlib
import json
import re
import sys
from collections.abc import Mapping, Sequence
from pathlib import Path
from typing import Any

from .mapping_transaction import apply_mapping_transaction
from .quarantine_paths import QUARANTINE_ROOT, quarantine_path


PROPOSAL_FIELDS = {"schema_version", "reviewed_date", "rows"}
PROPOSAL_FIELDS.add("candidate_provenance")
PROPOSAL_ROW_FIELDS = {
    "code_id",
    "spec_ref",
    "invariant_ids",
    "discovery_ids",
}
CARD_FIELDS = {
    "schema_version",
    "kind",
    "proposal_path",
    "proposal_digest",
    "reviewed_date",
    "rows",
    "review_card_digest",
}
HUMAN_FIELDS = {
    "schema_version",
    "kind",
    "generated_binding",
    "reviewer_ids",
    "accepted_row_indices",
}
GENERATED_BINDING_FIELDS = {
    "proposal_path",
    "proposal_digest",
    "review_card_path",
    "review_card_digest",
    "reviewed_date",
}
CANDIDATE_PROVENANCE_FIELDS = {
    "compiler",
    "candidate_card_path",
    "candidate_card_digest",
    "candidate_human_selection_path",
    "candidate_human_selection_digest",
    "trace_path",
    "trace_digest",
    "proposer_ids",
    "risk_classes",
    "independent_authority_review_required",
}
SHA256_RE = re.compile(r"^sha256:[0-9a-f]{64}$")


def write_authority_review_card(
    *,
    root: Path,
    proposal_path: Path,
    output_path: Path,
) -> dict[str, Any]:
    """Write a numbered review card bound to the exact internal proposal."""

    root = root.resolve()
    proposal_relative, proposal_resolved = quarantine_path(
        root, proposal_path, label="proposal", must_exist=True
    )
    output_relative, output_resolved = quarantine_path(
        root, output_path, label="review card output", must_exist=False
    )
    if output_resolved == proposal_resolved:
        raise ValueError("proposal and review card output paths must differ")
    proposal_bytes = proposal_resolved.read_bytes()
    proposal = _load_json_object(proposal_bytes, "authority proposal")
    reviewed_date, rows = _validate_proposal(proposal)
    card_rows = _card_rows(rows)
    card_without_digest = {
        "schema_version": 1,
        "kind": "numbered_code_spec_authority_review",
        "proposal_path": proposal_relative,
        "proposal_digest": _digest(proposal_bytes),
        "reviewed_date": reviewed_date,
        "rows": card_rows,
    }
    card = {
        **card_without_digest,
        "review_card_digest": _canonical_digest(card_without_digest),
    }
    card_bytes = _json_bytes(card)
    _write_quarantine(
        root, {output_relative: card_bytes}, label="authority review card"
    )
    return card


def write_authority_human_selection_template(
    *,
    root: Path,
    review_card_path: Path,
    output_path: Path,
) -> dict[str, Any]:
    """Generate binding fields so humans edit only reviewers and row indices."""

    root = root.resolve()
    card_relative, card_resolved = quarantine_path(
        root, review_card_path, label="review card", must_exist=True
    )
    output_relative, output_resolved = quarantine_path(
        root,
        output_path,
        label="human selection template output",
        must_exist=False,
    )
    if output_resolved == card_resolved:
        raise ValueError(
            "review card and human selection template paths must differ"
        )
    card = _load_json_object(
        card_resolved.read_bytes(), "authority review card"
    )
    proposal_path = card.get("proposal_path")
    if not isinstance(proposal_path, str):
        raise ValueError("authority review card proposal path is malformed")
    proposal_relative, proposal_resolved = quarantine_path(
        root,
        root / proposal_path,
        label="proposal",
        must_exist=True,
    )
    proposal_bytes = proposal_resolved.read_bytes()
    proposal = _load_json_object(proposal_bytes, "authority proposal")
    reviewed_date, rows = _validate_proposal(proposal)
    _validate_card(
        card,
        proposal_path=proposal_relative,
        proposal_digest=_digest(proposal_bytes),
        reviewed_date=reviewed_date,
        proposal_rows=rows,
    )
    template = {
        "schema_version": 1,
        "kind": "generated_numbered_authority_selection_template",
        "generated_binding": {
            "proposal_path": proposal_relative,
            "proposal_digest": _digest(proposal_bytes),
            "review_card_path": card_relative,
            "review_card_digest": card["review_card_digest"],
            "reviewed_date": reviewed_date,
        },
        "reviewer_ids": [],
        "accepted_row_indices": [],
    }
    _write_quarantine(
        root,
        {output_relative: _json_bytes(template)},
        label="human authority selection template",
    )
    return template


def build_authority_review_selection(
    *,
    root: Path,
    proposal_path: Path,
    review_card_path: Path,
    human_selection_path: Path,
) -> dict[str, Any]:
    """Purely rebuild the compiled selection from its three bound inputs."""

    root = root.resolve()
    proposal_relative, proposal_resolved = quarantine_path(
        root, proposal_path, label="proposal", must_exist=True
    )
    card_relative, card_resolved = quarantine_path(
        root, review_card_path, label="review card", must_exist=True
    )
    human_relative, human_resolved = quarantine_path(
        root, human_selection_path, label="human selection", must_exist=True
    )
    proposal_bytes = proposal_resolved.read_bytes()
    card_bytes = card_resolved.read_bytes()
    human_bytes = human_resolved.read_bytes()
    proposal = _load_json_object(proposal_bytes, "authority proposal")
    reviewed_date, rows = _validate_proposal(proposal)
    card = _load_json_object(card_bytes, "authority review card")
    _validate_card(
        card,
        proposal_path=proposal_relative,
        proposal_digest=_digest(proposal_bytes),
        reviewed_date=reviewed_date,
        proposal_rows=rows,
    )
    human = _load_json_object(human_bytes, "human authority selection")
    if set(human) != HUMAN_FIELDS:
        raise ValueError("human authority selection fields drift")
    if (
        human.get("schema_version") != 1
        or human.get("kind")
        != "generated_numbered_authority_selection_template"
    ):
        raise ValueError("human authority selection template identity drift")
    expected_binding = {
        "proposal_path": proposal_relative,
        "proposal_digest": _digest(proposal_bytes),
        "review_card_path": card_relative,
        "review_card_digest": card["review_card_digest"],
        "reviewed_date": reviewed_date,
    }
    binding = human.get("generated_binding")
    if (
        not isinstance(binding, Mapping)
        or set(binding) != GENERATED_BINDING_FIELDS
        or dict(binding) != expected_binding
    ):
        raise ValueError("human authority selection generated binding is stale")
    reviewers = _unique_strings(
        human.get("reviewer_ids"), "human authority selection reviewers"
    )
    candidate_provenance = proposal["candidate_provenance"]
    if candidate_provenance["independent_authority_review_required"]:
        proposer_ids = set(candidate_provenance["proposer_ids"])
        if proposer_ids.intersection(reviewers):
            raise ValueError(
                "high-risk authority reviewers must be independent of proposers"
            )
        if len(reviewers) < 2:
            raise ValueError(
                "high-risk authority review requires two distinct reviewers"
            )
    indices = _indices(
        human.get("accepted_row_indices"),
        len(rows),
        "accepted row indices",
    )
    return {
        "schema_version": 1,
        "reviewed_date": reviewed_date,
        "review_provenance": {
            "compiler": "numbered_code_spec_authority_review_v1",
            "proposal_path": proposal_relative,
            "proposal_digest": _digest(proposal_bytes),
            "review_card_path": card_relative,
            "review_card_digest": card["review_card_digest"],
            "human_selection_path": human_relative,
            "human_selection_digest": _digest(human_bytes),
            "reviewer_ids": reviewers,
            "candidate_provenance": dict(candidate_provenance),
        },
        "rows": [dict(rows[index - 1]) for index in indices],
    }


def compile_authority_review_selection(
    *,
    root: Path,
    proposal_path: Path,
    review_card_path: Path,
    human_selection_path: Path,
    output_path: Path,
) -> dict[str, Any]:
    """Compile an ID-free numbered decision into the internal selection."""

    root = root.resolve()
    _, proposal_resolved = quarantine_path(
        root, proposal_path, label="proposal", must_exist=True
    )
    _, card_resolved = quarantine_path(
        root, review_card_path, label="review card", must_exist=True
    )
    _, human_resolved = quarantine_path(
        root, human_selection_path, label="human selection", must_exist=True
    )
    output_relative, output_resolved = quarantine_path(
        root, output_path, label="compiled selection output", must_exist=False
    )
    if output_resolved in {
        proposal_resolved,
        card_resolved,
        human_resolved,
    }:
        raise ValueError(
            "compiled selection output must differ from every review input"
        )
    compiled = build_authority_review_selection(
        root=root,
        proposal_path=proposal_resolved,
        review_card_path=card_resolved,
        human_selection_path=human_resolved,
    )
    _write_quarantine(
        root,
        {output_relative: _json_bytes(compiled)},
        label="compiled authority selection",
    )
    return compiled


def main(argv: Sequence[str] | None = None) -> int:
    parser = argparse.ArgumentParser(
        description="Build or compile numbered code-spec authority review"
    )
    subparsers = parser.add_subparsers(dest="command", required=True)
    card = subparsers.add_parser("card")
    card.add_argument("--root", default=".")
    card.add_argument("--proposal", required=True)
    card.add_argument("--output", required=True)
    template = subparsers.add_parser("template")
    template.add_argument("--root", default=".")
    template.add_argument("--review-card", required=True)
    template.add_argument("--output", required=True)
    compile_parser = subparsers.add_parser("compile")
    compile_parser.add_argument("--root", default=".")
    compile_parser.add_argument("--proposal", required=True)
    compile_parser.add_argument("--review-card", required=True)
    compile_parser.add_argument("--human-selection", required=True)
    compile_parser.add_argument("--output", required=True)
    args = parser.parse_args(argv)
    root = Path(args.root).resolve()
    try:
        if args.command == "card":
            result = write_authority_review_card(
                root=root,
                proposal_path=_rooted(root, args.proposal),
                output_path=_rooted(root, args.output),
            )
            label = "card"
            count = len(result["rows"])
        elif args.command == "template":
            result = write_authority_human_selection_template(
                root=root,
                review_card_path=_rooted(root, args.review_card),
                output_path=_rooted(root, args.output),
            )
            label = "template"
            count = len(result["accepted_row_indices"])
        else:
            result = compile_authority_review_selection(
                root=root,
                proposal_path=_rooted(root, args.proposal),
                review_card_path=_rooted(root, args.review_card),
                human_selection_path=_rooted(root, args.human_selection),
                output_path=_rooted(root, args.output),
            )
            label = "selection"
            count = len(result["rows"])
    except (OSError, UnicodeError, ValueError, json.JSONDecodeError) as error:
        print(f"code-spec authority review failed: {error}", file=sys.stderr)
        return 1
    print(f"code-spec authority review {label} written: rows={count}")
    return 0


def _validate_proposal(
    proposal: Mapping[str, Any],
) -> tuple[str, list[Mapping[str, Any]]]:
    if set(proposal) != PROPOSAL_FIELDS or proposal.get("schema_version") != 1:
        raise ValueError("authority proposal fields or schema version drift")
    reviewed_date = proposal.get("reviewed_date")
    if not isinstance(reviewed_date, str) or not re.fullmatch(
        r"\d{4}-\d{2}-\d{2}", reviewed_date
    ):
        raise ValueError("authority proposal reviewed_date is malformed")
    rows = _objects(proposal.get("rows"), "authority proposal rows")
    if not rows:
        raise ValueError("authority proposal rows must be non-empty")
    for index, row in enumerate(rows):
        if set(row) != PROPOSAL_ROW_FIELDS:
            raise ValueError(f"authority proposal row {index} fields drift")
        _required_string(row.get("code_id"), f"proposal row {index} code_id")
        _required_string(row.get("spec_ref"), f"proposal row {index} spec_ref")
        _unique_strings(
            row.get("invariant_ids"), f"proposal row {index} invariants"
        )
        _unique_strings(
            row.get("discovery_ids"), f"proposal row {index} discoveries"
        )
    _validate_candidate_provenance(proposal.get("candidate_provenance"))
    return reviewed_date, rows


def _validate_candidate_provenance(value: Any) -> None:
    if (
        not isinstance(value, Mapping)
        or set(value) != CANDIDATE_PROVENANCE_FIELDS
        or value.get("compiler")
        != "numbered_code_spec_authority_candidates_v1"
    ):
        raise ValueError("authority proposal candidate provenance fields drift")
    for field in (
        "candidate_card_path",
        "candidate_human_selection_path",
        "trace_path",
    ):
        path = value.get(field)
        if (
            not isinstance(path, str)
            or not path.startswith(QUARANTINE_ROOT + "/")
            or ".." in Path(path).parts
        ):
            raise ValueError(
                f"authority proposal candidate provenance {field} is malformed"
            )
    for field in (
        "candidate_card_digest",
        "candidate_human_selection_digest",
        "trace_digest",
    ):
        if not isinstance(value.get(field), str) or not SHA256_RE.fullmatch(
            str(value[field])
        ):
            raise ValueError(
                f"authority proposal candidate provenance {field} is malformed"
            )
    _unique_strings(
        value.get("proposer_ids"),
        "authority proposal candidate proposer IDs",
    )
    _unique_strings(
        value.get("risk_classes"),
        "authority proposal candidate risk classes",
    )
    if not isinstance(value.get("independent_authority_review_required"), bool):
        raise ValueError(
            "authority proposal candidate independence flag is malformed"
        )


def _validate_card(
    card: Mapping[str, Any],
    *,
    proposal_path: str,
    proposal_digest: str,
    reviewed_date: str,
    proposal_rows: Sequence[Mapping[str, Any]],
) -> None:
    if set(card) != CARD_FIELDS:
        raise ValueError("authority review card fields drift")
    claimed = card.get("review_card_digest")
    without_digest = {
        key: value for key, value in card.items() if key != "review_card_digest"
    }
    if (
        card.get("schema_version") != 1
        or card.get("kind") != "numbered_code_spec_authority_review"
        or card.get("proposal_path") != proposal_path
        or card.get("proposal_digest") != proposal_digest
        or card.get("reviewed_date") != reviewed_date
        or not isinstance(claimed, str)
        or claimed != _canonical_digest(without_digest)
    ):
        raise ValueError("authority review card digest or source binding is stale")
    rows = _objects(card.get("rows"), "authority review card rows")
    if rows != _card_rows(proposal_rows):
        raise ValueError("authority review card numbered rows drift")


def _card_rows(
    rows: Sequence[Mapping[str, Any]],
) -> list[dict[str, Any]]:
    return [
        {
            "row_index": row_index,
            "code_id": row["code_id"],
            "spec_ref": row["spec_ref"],
            "invariants": [
                {
                    "invariant_index": invariant_index,
                    "invariant_id": invariant_id,
                }
                for invariant_index, invariant_id in enumerate(
                    row["invariant_ids"], start=1
                )
            ],
            "discoveries": [
                {
                    "discovery_index": discovery_index,
                    "discovery_id": discovery_id,
                }
                for discovery_index, discovery_id in enumerate(
                    row["discovery_ids"], start=1
                )
            ],
        }
        for row_index, row in enumerate(rows, start=1)
    ]


def _indices(value: Any, maximum: int, label: str) -> list[int]:
    if (
        not isinstance(value, list)
        or not value
        or any(not isinstance(index, int) or isinstance(index, bool) for index in value)
        or len(set(value)) != len(value)
        or any(index < 1 or index > maximum for index in value)
    ):
        raise ValueError(f"{label} must be unique and in range")
    return sorted(value)


def _unique_strings(value: Any, label: str) -> list[str]:
    if (
        not isinstance(value, list)
        or not value
        or any(not isinstance(item, str) or not item for item in value)
        or len(set(value)) != len(value)
    ):
        raise ValueError(f"{label} must be a non-empty unique string array")
    return sorted(value)


def _objects(value: Any, label: str) -> list[Mapping[str, Any]]:
    if (
        not isinstance(value, list)
        or any(not isinstance(row, Mapping) for row in value)
    ):
        raise ValueError(f"{label} must be an array of objects")
    return list(value)


def _required_string(value: Any, label: str) -> str:
    if not isinstance(value, str) or not value:
        raise ValueError(f"{label} must be a non-empty string")
    return value


def _write_quarantine(
    root: Path, replacements: Mapping[str, bytes], *, label: str
) -> None:
    apply_mapping_transaction(
        root=root,
        replacements=replacements,
        validate=lambda: [
            f"{relative}: {label} differs"
            for relative, expected in replacements.items()
            if not (root / relative).is_file()
            or (root / relative).read_bytes() != expected
        ],
    )


def _load_json_object(content: bytes, label: str) -> dict[str, Any]:
    value = json.loads(content)
    if not isinstance(value, dict):
        raise ValueError(f"{label} root must be an object")
    return value


def _canonical_digest(value: Mapping[str, Any]) -> str:
    encoded = json.dumps(
        value, sort_keys=True, separators=(",", ":"), ensure_ascii=False
    ).encode()
    return _digest(encoded)


def _digest(content: bytes) -> str:
    return "sha256:" + hashlib.sha256(content).hexdigest()


def _json_bytes(value: Mapping[str, Any]) -> bytes:
    return (
        json.dumps(value, indent=2, sort_keys=True, ensure_ascii=False) + "\n"
    ).encode()


def _rooted(root: Path, value: str) -> Path:
    path = Path(value)
    return path if path.is_absolute() else root / path


if __name__ == "__main__":
    raise SystemExit(main())
