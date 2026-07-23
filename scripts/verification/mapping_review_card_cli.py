"""Generate a numbered semantic-review card for quarantined mappings."""

from __future__ import annotations

import argparse
import json
import sys
import tomllib
from collections.abc import Mapping
from pathlib import Path
from typing import Any

from .code_spec_live import load_code_spec_metadata
from .mapping_review_selection import build_review_card
from .test_catalog_json_schema import validate_json_schema_instance


DEFAULT_PROPOSALS = Path("verification/test-catalog-proposals.toml")
DEFAULT_JSON = Path(
    "target/gate-artifacts/verification/test-catalog-mapping-review-card.json"
)
DEFAULT_MARKDOWN = Path(
    "target/gate-artifacts/verification/test-catalog-mapping-review-card.md"
)
CARD_SCHEMA = Path(
    "verification/schemas/test-catalog-mapping-review-card.schema.json"
)


def generate_review_card(
    *,
    root: Path,
    proposals_path: Path,
    json_path: Path,
    markdown_path: Path,
) -> dict[str, Any]:
    root = root.resolve()
    proposal_document = _load_toml(proposals_path)
    metadata = load_code_spec_metadata(root)
    card = build_review_card(
        proposal_document=proposal_document,
        invariants=metadata.invariants,
    )
    schema = json.loads((root / CARD_SCHEMA).read_text())
    failures = validate_json_schema_instance(card, schema)
    if failures:
        raise ValueError("; ".join(failures))
    rendered_json = json.dumps(card, indent=2, sort_keys=True) + "\n"
    rendered_markdown = render_review_card_markdown(card)
    json_path.parent.mkdir(parents=True, exist_ok=True)
    markdown_path.parent.mkdir(parents=True, exist_ok=True)
    json_path.write_text(rendered_json)
    markdown_path.write_text(rendered_markdown)
    return card


def render_review_card_markdown(card: Mapping[str, Any]) -> str:
    proposals = card.get("proposals")
    if not isinstance(proposals, list):
        raise ValueError("mapping review card proposals must be an array")
    lines = [
        "# Test-Catalog Mapping Review Card",
        "",
        f"Source revision: `{card.get('source_commit')}`",
        f"Proposal queue digest: `{card.get('proposal_queue_digest')}`",
        "",
        "Submit only displayed one-based indices in the closed review-selection file.",
        "Do not copy proposal, discovery, invariant, assertion, behavior, digest,",
        "command, duration, count, provenance, catalog, or ledger identifiers.",
    ]
    for proposal in proposals:
        if not isinstance(proposal, Mapping):
            raise ValueError("mapping review card proposal must be an object")
        index = proposal.get("proposal_index")
        lines.extend(
            [
                "",
                f"## Proposal {index}: `{proposal.get('name')}`",
                "",
                f"- Source: `{proposal.get('path')}`",
                f"- Candidate tier: `{proposal.get('candidate_tier')}`",
                f"- Traced command: `{proposal.get('command')}`",
                f"- Measured duration: {proposal.get('duration_ms')} ms "
                f"(`{proposal.get('duration_class')}`)",
                f"- Suggested test class: `{proposal.get('suggested_test_class')}` "
                "(non-authoritative until confirmed)",
                "",
                "### Invariants",
                "",
                "| # | Invariant | Area | Owner | Risk | Oracle | Suites |",
                "| ---: | --- | --- | --- | --- | --- | --- |",
            ]
        )
        for invariant in proposal.get("invariants", []):
            lines.append(
                f"| {invariant['invariant_index']} | `{invariant['invariant_id']}` | "
                f"`{invariant['area']}` | `{invariant['owner']}` | "
                f"`{invariant['risk']}` | `{invariant['oracle_ref']}` | "
                f"`{', '.join(invariant['suite_tiers'])}` |"
            )
        lines.extend(
            [
                "",
                "### Behaviors",
                "",
                "| # | Invariant # | Family | Outcome | Rationale |",
                "| ---: | ---: | --- | --- | --- |",
            ]
        )
        for behavior in proposal.get("behaviors", []):
            lines.append(
                f"| {behavior['behavior_index']} | "
                f"{behavior['invariant_index']} | "
                f"`{behavior['case_family']}` | `{behavior['outcome']}` | "
                f"{_markdown_cell(behavior['rationale'])} |"
            )
        lines.extend(
            [
                "",
                "### Assertions",
                "",
                "| # | Kind | Function | Line | Expression |",
                "| ---: | --- | --- | ---: | --- |",
            ]
        )
        for assertion in proposal.get("assertions", []):
            lines.append(
                f"| {assertion['assertion_index']} | `{assertion['kind']}` | "
                f"`{assertion['function']}` | {assertion['line']} | "
                f"`{_markdown_cell(assertion['expression'])}` |"
            )
    if not proposals:
        lines.extend(["", "No pending proposals."])
    return "\n".join(lines) + "\n"


def build_parser() -> argparse.ArgumentParser:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--root", type=Path, default=Path.cwd())
    parser.add_argument("--proposals", type=Path, default=DEFAULT_PROPOSALS)
    parser.add_argument("--json-out", type=Path, default=DEFAULT_JSON)
    parser.add_argument("--markdown-out", type=Path, default=DEFAULT_MARKDOWN)
    return parser


def main(argv: list[str] | None = None) -> int:
    args = build_parser().parse_args(argv)
    root = args.root.resolve()
    try:
        card = generate_review_card(
            root=root,
            proposals_path=_resolve(root, args.proposals),
            json_path=_resolve(root, args.json_out),
            markdown_path=_resolve(root, args.markdown_out),
        )
    except (OSError, ValueError, json.JSONDecodeError, tomllib.TOMLDecodeError) as error:
        print(f"mapping review card generation failed: {error}", file=sys.stderr)
        return 2
    print(
        f"mapping review card generated: {len(card['proposals'])} numbered proposals"
    )
    return 0


def _load_toml(path: Path) -> dict[str, Any]:
    document = tomllib.loads(path.read_text())
    if not isinstance(document, dict):
        raise ValueError(f"{path} root must be a table")
    return document


def _resolve(root: Path, path: Path) -> Path:
    return path if path.is_absolute() else root / path


def _markdown_cell(value: Any) -> str:
    return (
        str(value)
        .replace("\\", "\\\\")
        .replace("|", "\\|")
        .replace("`", "\\`")
        .replace("\n", "\\n")
    )
