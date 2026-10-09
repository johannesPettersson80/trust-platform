#!/usr/bin/env python3
"""Builds the Chinese, Japanese and Korean fonts the HMI ships (specification 39, §14.5 item 2).

Each is Noto Sans for one language's forms (SIL Open Font License 1.1), subset to that language's
national character set and to every character of truST's own words in it, with ASCII, the CJK
punctuation and the full-width forms, keeping the weight axis, as WOFF2:

- NotoSansSC-Subset.woff2: all of GB 2312, for Simplified Chinese (zh-Hans);
- NotoSansJP-Subset.woff2: all of JIS X 0208, for Japanese (ja);
- NotoSansKR-Subset.woff2: every modern Hangul syllable and KS X 1001's symbols, for Korean (ko).

The sources are the variable Noto Sans SC, JP and KR of google/fonts, pinned by commit and SHA-256.
The fonts, their licence and `cjk-fonts.json` (each file's SHA-256 and the characters it covers,
which the tests read) are written beside the runtime's Inter font; the VS Code build copies them
into the extension.

    python3 editors/vscode/scripts/build-cjk-fonts.py <folder holding the three source fonts and OFL.txt>
"""

import hashlib
import io
import json
import sys
import tomllib
from pathlib import Path

from fontTools import subset
from fontTools.ttLib import TTFont

ROOT = Path(__file__).resolve().parents[3]
FONTS = ROOT / "crates/trust-runtime/src/web/ui/assets/fonts"
BUILTIN = ROOT / "crates/trust-runtime/src/hmi/texts/builtin"
GOOGLE_FONTS = "https://raw.githubusercontent.com/google/fonts"


def codec_characters(codec: str, rows: range) -> set[str]:
    """Every character a double-byte national character set encodes in the given rows."""
    found = set()
    for row in rows:
        for cell in range(0xA1, 0xFF):
            try:
                found.add(bytes([row, cell]).decode(codec))
            except UnicodeDecodeError:
                pass
    return found


COMMON = (
    {chr(code) for code in range(0x20, 0x7F)}
    | {chr(code) for code in range(0x3000, 0x3040)}
    | {chr(code) for code in range(0xFF00, 0xFFF0)}
)

FAMILIES = [
    {
        "language": "zh-Hans",
        "family": "truST CJK SC",
        "file": "NotoSansSC-Subset.woff2",
        "source": "NotoSansSC[wght].ttf",
        "url": f"{GOOGLE_FONTS}/2894aab31764f10f29c421bdfd2340d3b382d384/ofl/notosanssc/NotoSansSC%5Bwght%5D.ttf",
        "sha256": "a3041811a78c361b1de50f953c805e0244951c21c5bd412f7232ef0d899af0da",
        "set": "GB 2312",
        "characters": lambda: codec_characters("gb2312", range(0xA1, 0xF8)),
    },
    {
        "language": "ja",
        "family": "truST CJK JP",
        "file": "NotoSansJP-Subset.woff2",
        "source": "NotoSansJP[wght].ttf",
        "url": f"{GOOGLE_FONTS}/295d98a7a0c17c68f1341eaeea354e7960ea70d3/ofl/notosansjp/NotoSansJP%5Bwght%5D.ttf",
        "sha256": "c2f3b4d463500a2ddcd3849cded1fceeb9fd6d1c32e6cbecd568453ba50fc68f",
        "set": "JIS X 0208",
        "characters": lambda: codec_characters("euc_jp", range(0xA1, 0xF5)),
    },
    {
        "language": "ko",
        "family": "truST CJK KR",
        "file": "NotoSansKR-Subset.woff2",
        "source": "NotoSansKR[wght].ttf",
        "url": f"{GOOGLE_FONTS}/4efc2774c63917927efe769ca845def6bd6debae/ofl/notosanskr/NotoSansKR%5Bwght%5D.ttf",
        "sha256": "194018e6b2b293a7964f037b25c0249ce1418bc9ab3c971060a03aa57861e252",
        "set": "modern Hangul and KS X 1001 symbols",
        "characters": lambda: {chr(code) for code in range(0xAC00, 0xD7A4)}
        | {chr(code) for code in range(0x3130, 0x3190)}
        | codec_characters("euc_kr", range(0xA1, 0xAD)),
    },
]


def builtin_characters(language: str) -> set[str]:
    """Every character of truST's own words in a language, so they always render."""
    table = tomllib.loads((BUILTIN / f"{language}.toml").read_text(encoding="utf-8"))
    return {character for text in table["texts"].values() for character in text}


def build(sources: Path) -> None:
    manifest = {"format": 1, "fonts": []}
    for family in FAMILIES:
        source = sources / family["source"]
        digest = hashlib.sha256(source.read_bytes()).hexdigest()
        if digest != family["sha256"]:
            sys.exit(f"{source}: SHA-256 {digest}, expected {family['sha256']} ({family['url']})")
        wanted = family["characters"]() | COMMON | builtin_characters(family["language"])
        # The source's timestamp is kept, so the same source always gives the same file.
        font = TTFont(source, recalcTimestamp=False)
        options = subset.Options()
        options.flavor = "woff2"
        options.hinting = False
        options.layout_features = ["*"]
        options.name_IDs = ["*"]
        subsetter = subset.Subsetter(options)
        subsetter.populate(text="".join(sorted(wanted)))
        subsetter.subset(font)
        covered = sorted(chr(code) for code in font.getBestCmap())
        missing = sorted(character for character in builtin_characters(family["language"]) if ord(character) not in font.getBestCmap() and not character.isspace())
        if missing:
            sys.exit(f"{family['source']} lacks truST's {family['language']} characters: {''.join(missing)}")
        written = io.BytesIO()
        font.flavor = "woff2"
        font.save(written, reorderTables=True)
        data = written.getvalue()
        (FONTS / family["file"]).write_bytes(data)
        manifest["fonts"].append(
            {
                "language": family["language"],
                "family": family["family"],
                "file": family["file"],
                "sha256": hashlib.sha256(data).hexdigest(),
                "set": family["set"],
                "source": {"url": family["url"], "sha256": family["sha256"]},
                "characters": "".join(covered),
            }
        )
        print(f"{family['file']}: {len(covered)} characters, {len(data) // 1024} KiB")
    (FONTS / "LICENSE-NOTO-CJK.txt").write_text((sources / "OFL.txt").read_text(encoding="utf-8"), encoding="utf-8")
    (FONTS / "cjk-fonts.json").write_text(json.dumps(manifest, ensure_ascii=False, indent=1) + "\n", encoding="utf-8")


if __name__ == "__main__":
    if len(sys.argv) != 2:
        sys.exit(__doc__)
    build(Path(sys.argv[1]))
