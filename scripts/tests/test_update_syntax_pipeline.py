"""The syntax diagram's stats come from the lexer's source (specification 22, "Documentation that follows the product"): the variants of
`TokenKind`, its keywords and trivia and the Pratt binding powers, and `--check` fails on a stale result. Each expected
value follows from the source text the test writes."""

from __future__ import annotations

import contextlib
import io
import sys
import tempfile
import unittest
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]
sys.path.insert(0, str(ROOT / "scripts"))

import update_syntax_pipeline as pipeline  # noqa: E402

TOKEN_KIND = """use logos::Logos;

#[derive(Logos)]
pub enum TokenKind {
    #[regex(r"[ \\t]+")]
    Whitespace,
    #[token(";")]
    Semicolon,
    #[token("IF", ignore(ascii_case))]
    KwIf,
    #[token("**")]
    Power,
    #[regex(
        r"[0-9]+",
        priority = 2
    )]
    IntLiteral,
}
"""

CLASSIFICATION = """impl TokenKind {
    pub fn is_trivia(self) -> bool {
        matches!(self, Self::Whitespace)
    }
}
"""

PARSING = """impl TokenKind {
    pub fn infix_binding_power(self) -> Option<(u8, u8)> {
        Some(match self {
            Self::KwIf => (1, 2),
            Self::Power => (14, 13),
            _ => return None,
        })
    }

    pub fn prefix_binding_power(self) -> Option<u8> {
        Some(match self {
            Self::Semicolon => 15,
            _ => return None,
        })
    }
}
"""

PUML = """@startuml
note as Stats
<<TOKEN_STATS>>
old
<<TOKEN_STATS_END>>
end note
note as Precedence
<<PRECEDENCE_TABLE>>
old
<<PRECEDENCE_TABLE_END>>
end note
@enduml
"""


def write_tree(root: Path, token_kind: str = TOKEN_KIND) -> None:
    """A tree with the lexer's three files and the diagram, as the script reads them."""
    for path, text in [
        (pipeline.TOKEN_KIND, token_kind),
        (pipeline.CLASSIFICATION, CLASSIFICATION),
        (pipeline.PARSING, PARSING),
        (pipeline.PUML, PUML),
    ]:
        (root / path).parent.mkdir(parents=True, exist_ok=True)
        (root / path).write_text(text, encoding="utf-8")


class TestStats(unittest.TestCase):
    """What the stats count."""

    def test_the_variants_keywords_and_trivia_come_from_the_enum_and_is_trivia(self) -> None:
        """Five variants, the keyword `IF` and the trivia `Whitespace`; `priority = 2` in an attribute that spans lines
        is no variant. Catches the count of the arguments of such an attribute, which made 217 of 214 variants."""
        stats = pipeline.parse_tokenkind(TOKEN_KIND, CLASSIFICATION)
        self.assertEqual(stats.total, 5)
        self.assertEqual(stats.keywords, ["IF"])
        self.assertEqual(stats.trivia, ["Whitespace"])

    def test_a_source_without_the_enum_or_its_trivia_fails(self) -> None:
        """No `pub enum TokenKind`, or no `fn is_trivia`: an error, never zero counts. Catches the run that read a file
        the enum had left and wrote 0 into the diagram."""
        with self.assertRaises(pipeline.SourceError):
            pipeline.parse_tokenkind("pub struct Other;\n", CLASSIFICATION)
        with self.assertRaises(pipeline.SourceError):
            pipeline.parse_tokenkind(TOKEN_KIND, "impl TokenKind {}\n")

    def test_the_precedence_table_reads_each_binding_power(self) -> None:
        """`(1, 2)` is left-associative and `(14, 13)` right-associative, each listed by its token's text, and the
        prefix power 15. Catches a table that drops the associativity or names tokens by their variants."""
        stats = pipeline.parse_tokenkind(TOKEN_KIND, CLASSIFICATION)
        infix, prefix = pipeline.parse_binding_powers(PARSING, stats.variant_to_literal)
        self.assertEqual(infix, ["Precedence (low → high):", "1-2:   IF", "14-13:   ** (right assoc)"])
        self.assertEqual(prefix, ["15:    prefix ;"])


class TestMain(unittest.TestCase):
    """The command over a tree."""

    def test_check_fails_on_a_stale_result_and_passes_after_a_run(self) -> None:
        """`--check` fails while the diagram holds `old` and the stats file is missing, writes nothing, and passes after
        a run has written both. Catches a check that always passes, and one that writes."""
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            write_tree(root)
            with contextlib.redirect_stdout(io.StringIO()) as printed:
                self.assertEqual(pipeline.main(["update_syntax_pipeline.py", "--check", str(root)]), 1)
            self.assertIn("syntax-pipeline.puml is stale", printed.getvalue())
            self.assertFalse((root / pipeline.OUTPUT).exists())
            self.assertEqual(pipeline.main(["update_syntax_pipeline.py", str(root)]), 0)
            self.assertIn("TokenKind variants: 5", (root / pipeline.PUML).read_text(encoding="utf-8"))
            self.assertIn("- Keywords (1):\n  IF\n", (root / pipeline.OUTPUT).read_text(encoding="utf-8"))
            self.assertEqual(pipeline.main(["update_syntax_pipeline.py", "--check", str(root)]), 0)

    def test_a_run_over_a_source_without_the_enum_fails_and_writes_nothing(self) -> None:
        """A tree whose token_kind.rs lacks the enum: exit 1, the diagram unchanged, no stats file. Catches the zero
        counts written into the diagram."""
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            write_tree(root, token_kind="pub struct Other;\n")
            with contextlib.redirect_stderr(io.StringIO()) as printed:
                self.assertEqual(pipeline.main(["update_syntax_pipeline.py", str(root)]), 1)
            self.assertIn("no `pub enum TokenKind`", printed.getvalue())
            self.assertEqual((root / pipeline.PUML).read_text(encoding="utf-8"), PUML)
            self.assertFalse((root / pipeline.OUTPUT).exists())

    def test_the_repository_is_current(self) -> None:
        """The committed diagram and stats are what a run writes from this tree's lexer. Catches a token added without
        the stats run."""
        self.assertEqual(pipeline.main(["update_syntax_pipeline.py", "--check", str(ROOT)]), 0)


if __name__ == "__main__":
    unittest.main()
