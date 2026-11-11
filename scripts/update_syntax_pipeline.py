#!/usr/bin/env python3
"""Update docs/diagrams/syntax/syntax-pipeline.puml and docs/diagrams/generated/syntax-stats.md from the lexer.

The stats come from what the source defines: the variants of `TokenKind` (crates/trust-syntax/src/lexer/tokens/
token_kind.rs, one per line at the enum's indentation), its keywords (the `Kw` variants, each with the text of its last
`#[token]`), its trivia (the variants that `is_trivia` lists, token_kind_impl_classification.rs) and the Pratt binding
powers (token_kind_impl_parsing.rs). A source without one of them fails the run instead of writing zeros.

`--check` writes nothing and fails when either file differs from what a run would write, as CI's Format job runs it
(specification 22, "Documentation that follows the product").

Usage: update_syntax_pipeline.py [--check] [ROOT]
"""
from __future__ import annotations

import re
import sys
from dataclasses import dataclass
from pathlib import Path

TOKENS = Path("crates/trust-syntax/src/lexer/tokens")
TOKEN_KIND = TOKENS / "token_kind.rs"
CLASSIFICATION = TOKENS / "token_kind_impl_classification.rs"
PARSING = TOKENS / "token_kind_impl_parsing.rs"
PUML = Path("docs/diagrams/syntax/syntax-pipeline.puml")
OUTPUT = Path("docs/diagrams/generated/syntax-stats.md")

# A variant: an identifier at the enum's own indentation (rustfmt's four spaces), so that the arguments of an
# attribute that spans lines (`priority = 2`) are no variants.
VARIANT = re.compile(r"^    ([A-Z][A-Za-z0-9_]*)\s*(?:,|=)")
TOKEN_TEXT = re.compile(r'#\[token\("([^"]+)"')


class SourceError(Exception):
    """The lexer's source lacks what the stats are made from."""


@dataclass
class TokenStats:
    total: int
    keywords: list[str]
    trivia: list[str]
    variant_to_literal: dict[str, str]


def enum_body(text: str, path: Path) -> str:
    """The lines between `pub enum TokenKind {` and its closing brace."""
    start = re.search(r"pub\s+enum\s+TokenKind\s*\{", text)
    if not start:
        raise SourceError(f"no `pub enum TokenKind` in {path}")
    end = text.find("\n}", start.end())
    return text[start.end():end if end >= 0 else len(text)]


def matches_of(text: str, function: str, path: Path) -> list[str]:
    """The variants that the `matches!` of `fn <function>` lists."""
    start = text.find(f"fn {function}(")
    if start < 0:
        raise SourceError(f"no `fn {function}` in {path}")
    end = text.find("\n    }", start)
    return re.findall(r"Self::([A-Za-z0-9_]+)", text[start:end if end >= 0 else len(text)])


def parse_tokenkind(token_kind: str, classification: str) -> TokenStats:
    """The stats of `TokenKind` from the text of token_kind.rs and of its classification methods."""
    total = 0
    pending_literal: str | None = None
    variant_to_literal: dict[str, str] = {}
    keyword_literals: list[str] = []
    for line in enum_body(token_kind, TOKEN_KIND).splitlines():
        token = TOKEN_TEXT.search(line)
        if token:
            pending_literal = token.group(1)
            continue
        variant = VARIANT.match(line)
        if not variant:
            continue
        name = variant.group(1)
        total += 1
        if pending_literal:
            variant_to_literal[name] = pending_literal
            if name.startswith("Kw"):
                keyword_literals.append(pending_literal)
        pending_literal = None
    if total == 0:
        raise SourceError(f"no variant of `TokenKind` in {TOKEN_KIND}")
    trivia = matches_of(classification, "is_trivia", CLASSIFICATION)
    return TokenStats(
        total=total,
        keywords=sorted(set(keyword_literals)),
        trivia=trivia,
        variant_to_literal=variant_to_literal,
    )


def parse_binding_powers(text: str, variant_to_literal: dict[str, str]) -> tuple[list[str], list[str]]:
    """The precedence table's lines from `infix_binding_power` and `prefix_binding_power`."""
    infix_entries: list[tuple[int, int, list[str]]] = []
    prefix_entries: list[tuple[int, list[str]]] = []
    in_infix = False
    in_prefix = False
    for line in text.splitlines():
        if "fn infix_binding_power" in line:
            in_infix = True
            continue
        if "fn prefix_binding_power" in line:
            in_infix = False
            in_prefix = True
            continue
        if "=>" not in line or "_ =>" in line or not (in_infix or in_prefix):
            continue
        names = [token.strip().replace("Self::", "") for token in line.split("=>", 1)[0].split("|")]
        names = [name for name in names if name]
        if in_infix:
            match = re.search(r"\((\d+),\s*(\d+)\)", line)
            if match:
                infix_entries.append((int(match.group(1)), int(match.group(2)), names))
        else:
            match = re.search(r"=>\s*(?:Some\()?(\d+)", line)
            if match:
                prefix_entries.append((int(match.group(1)), names))
    if not infix_entries or not prefix_entries:
        raise SourceError(f"no binding powers in {PARSING}")

    def display(name: str) -> str:
        if name in variant_to_literal:
            return variant_to_literal[name]
        if name.startswith("Kw"):
            return name[2:].upper()
        return name

    infix_lines = ["Precedence (low → high):"]
    for l_bp, r_bp, names in sorted(infix_entries, key=lambda item: item[0]):
        suffix = " (right assoc)" if l_bp > r_bp else ""
        infix_lines.append(f"{l_bp}-{r_bp}:   {', '.join(display(name) for name in names)}{suffix}")
    prefix_lines = [f"{bp}:    prefix {', '.join(display(name) for name in names)}" for bp, names in prefix_entries]
    return infix_lines, prefix_lines


def replace_block(text: str, start_marker: str, end_marker: str, replacement: str) -> str:
    start_match = re.search(rf"(?m)^(?P<indent>\s*){re.escape(start_marker)}\s*$", text)
    end_match = re.search(rf"(?m)^(?P<indent>\s*){re.escape(end_marker)}\s*$", text)
    if not start_match or not end_match or end_match.start() <= start_match.end():
        raise SourceError(f"the markers {start_marker} and {end_marker} are missing or out of order in {PUML}")
    indent = start_match.group("indent")
    replaced = "\n".join(f"{indent}{line}" if line else indent for line in replacement.splitlines())
    return text[: start_match.end()] + "\n" + replaced + "\n" + text[end_match.start():]


def render(root: Path) -> dict[Path, str]:
    """The text that a run writes into each generated file, by its path relative to `root`."""
    stats = parse_tokenkind((root / TOKEN_KIND).read_text(encoding="utf-8"),
                            (root / CLASSIFICATION).read_text(encoding="utf-8"))
    infix, prefix = parse_binding_powers((root / PARSING).read_text(encoding="utf-8"), stats.variant_to_literal)
    others = stats.total - len(stats.keywords) - len(stats.trivia)
    counts = [
        f"TokenKind variants: {stats.total}",
        f"• Keywords: {len(stats.keywords)}",
        f"• Trivia: {len(stats.trivia)}",
        f"• Punctuation, operators, literals and identifiers: {others}",
    ]
    precedence = infix + prefix
    puml = (root / PUML).read_text(encoding="utf-8")
    puml = replace_block(puml, "<<TOKEN_STATS>>", "<<TOKEN_STATS_END>>",
                         "\n".join([*counts, "", f"Keywords ({len(stats.keywords)}) in {OUTPUT.as_posix()}"]))
    puml = replace_block(puml, "<<PRECEDENCE_TABLE>>", "<<PRECEDENCE_TABLE_END>>", "\n".join(precedence))
    stats_md = "\n".join([
        "# Syntax Stats",
        "",
        *[f"- {line.removeprefix('• ')}" for line in counts],
        "",
        f"- Keywords ({len(stats.keywords)}):",
        "  " + ", ".join(stats.keywords),
        "",
        "- Pratt precedence:",
        *["  " + line for line in precedence],
    ]) + "\n"
    return {PUML: puml, OUTPUT: stats_md}


def main(argv: list[str]) -> int:
    args = argv[1:]
    check = "--check" in args
    paths = [arg for arg in args if arg != "--check"]
    root = Path(paths[0]) if paths else Path(__file__).resolve().parents[1]
    try:
        generated = render(root)
    except SourceError as error:
        print(f"update_syntax_pipeline: {error}", file=sys.stderr)
        return 1
    if check:
        stale = [path for path, text in generated.items()
                 if not (root / path).is_file() or (root / path).read_text(encoding="utf-8") != text]
        for path in stale:
            print(f"{path.as_posix()} is stale: run `python3 scripts/update_syntax_pipeline.py` and commit it")
        return 1 if stale else 0
    for path, text in generated.items():
        (root / path).parent.mkdir(parents=True, exist_ok=True)
        (root / path).write_text(text, encoding="utf-8")
    return 0


if __name__ == "__main__":
    raise SystemExit(main(sys.argv))
