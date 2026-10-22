#!/usr/bin/env python3
"""List the files left in editors/vscode/src/controlStudio/ and how the product reaches each one (V6.4).

Every file is printed with its line count, its stage in checklist Appendix B, the number of test suites that import it
directly, and one status:

- core: a kept route reaches it (the extension entry, panel.ts, loopSetup/loopSetupPanel.ts, the webview entries) without
  passing through a parked module. The user reviews these: nothing there calculates, decides or validates, apart from the
  parked features of V7.
- parked: a V7 module of Appendix B, or reached only through one. It keeps its TypeScript until its V7 port (decision D9).
- unreached: no product entry reaches it. A V0 to V5 file in this state is a deletion candidate (S13), unless test suites
  still import it: then the tests move to the files that hold the behaviour first.

The refs column says how the product reaches the file: `rt` when a runtime reference reaches it, `type` when only a
type-only import does. A type-only import still breaks the build when its file is removed, so such a file stays until its
importer changes.

Reference edges: imports and re-exports (`import type` and `export type` included), dynamic `import()` and `require()`, and
file names in string literals such as `"identificationWorker.js"`, which start worker scripts by path. An `import("…")` in a type
position (after `:`, `as`, `typeof`, `<`, `|`, `&`, `keyof`, or on the right of `type Name =`) is only a type: it is `type`, not `rt`.

The stage of each file comes from the checklist's Appendix B, read with `--checklist`. Its default is the checklist under
`docs/internal/testing/checklists/`, which git ignores and only the primary checkout holds.

Usage: python3 scripts/list_control_studio_files.py [--root PATH] [--checklist PATH]
Exit status: 0 when every relative import of the product and the tests resolves, 1 otherwise.
"""

from __future__ import annotations

import argparse
import os
import re
import sys
from collections import defaultdict
from pathlib import Path

SKIP_DIRS = {"node_modules", "out", "dist", ".vscode-test"}
CHECKLIST = "docs/internal/testing/checklists/rust-logic-migration.md"
SOURCE_SUFFIXES = (".ts", ".tsx")
CODE_SUFFIXES = {"", ".ts", ".tsx", ".js", ".mjs", ".cjs"}
CONTROL_STUDIO = "editors/vscode/src/controlStudio"
DELETABLE_STAGES = {"V0", "V1", "V2", "V3", "V4", "V5"}
STATUS_ORDER = {"core": 0, "parked": 1, "unreached": 2}

# `import x from "y"`, `export {a} from "y"`, `export * from "y"`, with an optional leading `type`.
FROM_RE = re.compile(r"""(?m)^[ \t]*(import|export)\s+(type\s+)?([^;'"]*?)\s*from\s*["']([^"']+)["']""")
SIDE_EFFECT_RE = re.compile(r"""(?m)^[ \t]*import\s+["']([^"']+)["']""")
DYNAMIC_RE = re.compile(r"""\bimport\(\s*["']([^"']+)["']\s*\)""")
# The text before an `import("…")` that makes it a type: `: import(…)`, `as import(…)`, `typeof import(…)`, `<import(…)`,
# `| import(…)`, `& import(…)`, `keyof import(…)` and `type Name = import(…)`.
TYPE_POSITION_RE = re.compile(r"(?:\bas|\btypeof|\bkeyof|:|<|\||&)\s*$")
TYPE_ALIAS_RE = re.compile(r"\btype\s+[\w$]+\s*(?:<[^>]*>)?\s*=\s*$")
REQUIRE_RE = re.compile(r"""\brequire\(\s*["']([^"']+)["']\s*\)""")
JS_NAME_RE = re.compile(r"""["'`]([^"'`\n]+?)\.js["'`]""")
STAGE_RE = re.compile(r"^#### (V\d+|stays) ")
ROW_RE = re.compile(r"^\|\s*`([^`]+\.tsx?)`\s*\|")


def source_files(src: Path, tests: bool) -> dict[Path, str]:
    """The TypeScript files under src, read once: the product's (`tests` False) or the test suites' (`tests` True)."""
    test_root = src / "test"
    files: dict[Path, str] = {}
    for dirpath, dirnames, filenames in os.walk(src):
        dirnames[:] = [d for d in dirnames if d not in SKIP_DIRS]
        if Path(dirpath).is_relative_to(test_root) != tests:
            continue
        for name in filenames:
            if name.endswith(SOURCE_SUFFIXES) and not name.endswith(".d.ts"):
                path = Path(dirpath, name)
                files[path] = path.read_text(encoding="utf-8")
    return files


def is_type_only(type_keyword: str, clause: str) -> bool:
    """True for `import type …` and for a named import whose every name is a type (`import { type A }`)."""
    if type_keyword:
        return True
    braces = re.fullmatch(r"\{(.*)\}", clause.strip(), re.S)
    if not braces:
        return False
    names = [name.strip() for name in braces.group(1).split(",") if name.strip()]
    return bool(names) and all(name.startswith("type ") for name in names)


def resolve(spec: str, origin: Path, known: dict[Path, str]) -> Path | None:
    """The source file a relative specifier names; None for a package or a file outside the sources. A dotted name such as
    `./suite.integration.test` names `suite.integration.test.ts`, so the suffix alone does not decide."""
    if not spec.startswith("."):
        return None
    base = Path(os.path.normpath(origin.parent / spec))
    candidates = []
    if base.suffix in (".js", ".ts", ".tsx"):
        stem = base.with_suffix("")
        candidates += [stem.with_suffix(".ts"), stem.with_suffix(".tsx")]
    candidates += [Path(f"{base}.ts"), Path(f"{base}.tsx"), base / "index.ts", base / "index.tsx", base]
    return next((candidate for candidate in candidates if candidate in known), None)


def read_stages(checklist: Path) -> dict[str, str]:
    """The stage of each file in Appendix B: the table rows under each `#### V<n>` or `#### stays` heading."""
    stages: dict[str, str] = {}
    stage = None
    inside = False
    for line in checklist.read_text(encoding="utf-8").splitlines():
        if line.startswith("## Appendix B"):
            inside = True
            continue
        if line.startswith("## Appendix C"):
            break
        if not inside:
            continue
        heading = STAGE_RE.match(line)
        if heading:
            stage = heading.group(1)
            continue
        row = ROW_RE.match(line)
        if row and stage:
            stages[row.group(1)] = stage
    return stages


def closure(start: set[Path], edges: dict[Path, set[tuple[Path, str]]], kinds: set[str],
            barrier: frozenset[Path] = frozenset()) -> set[Path]:
    """Every file reached from `start` through edges of `kinds`; a barrier file is reached but not expanded."""
    seen: set[Path] = set()
    stack = list(start)
    while stack:
        current = stack.pop()
        if current in seen:
            continue
        seen.add(current)
        if current in barrier:
            continue
        stack.extend(target for target, kind in edges.get(current, ()) if kind in kinds and target not in seen)
    return seen


def references(text: str) -> list[tuple[str, str]]:
    """Each module specifier a source text names, with its kind: `type` when it is only a type, else `rt`."""
    found: list[tuple[str, str]] = []
    for _, type_keyword, clause, spec in FROM_RE.findall(text):
        found.append((spec, "type" if is_type_only(type_keyword, clause) else "rt"))
    found += [(spec, "rt") for spec in SIDE_EFFECT_RE.findall(text)]
    for match in DYNAMIC_RE.finditer(text):
        before = text[max(0, match.start() - 200):match.start()]
        is_type = TYPE_POSITION_RE.search(before) or TYPE_ALIAS_RE.search(before)
        found.append((match.group(1), "type" if is_type else "rt"))
    found += [(spec, "rt") for spec in REQUIRE_RE.findall(text)]
    return found


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    parser.add_argument("--root", type=Path, default=Path(__file__).resolve().parents[1],
                        help="the repository root (default: the checkout this script is in)")
    parser.add_argument("--checklist", type=Path, default=None,
                        help="the rust-logic-migration checklist that holds Appendix B (default: under --root)")
    args = parser.parse_args()
    root = args.root.resolve()
    checklist = args.checklist or root / CHECKLIST
    if not checklist.is_file():
        print(f"The checklist with Appendix B is not at {checklist}; pass --checklist.", file=sys.stderr)
        return 2

    src = root / "editors/vscode/src"
    product = source_files(src, tests=False)
    suites = source_files(src, tests=True)
    known = {**product, **suites}
    control_dir = root / CONTROL_STUDIO
    stages = read_stages(checklist)
    stage_of = {path: stages.get(path.relative_to(control_dir).as_posix(), "unlisted")
                for path in product if path.is_relative_to(control_dir)}
    parked = frozenset(path for path, stage in stage_of.items() if stage == "V7")
    by_name: dict[str, list[Path]] = defaultdict(list)
    for path in stage_of:
        by_name[path.stem].append(path)

    edges: dict[Path, set[tuple[Path, str]]] = defaultdict(set)
    test_imports: dict[Path, set[Path]] = defaultdict(set)
    unresolved: list[tuple[Path, str]] = []
    for origin, text in known.items():
        for spec, kind in references(text):
            target = resolve(spec, origin, known)
            if target is None:
                if spec.startswith(".") and Path(spec).suffix in CODE_SUFFIXES:
                    unresolved.append((origin, spec))
                continue
            if origin in suites:
                if target in stage_of:
                    test_imports[target].add(origin)
            else:
                edges[origin].add((target, kind))
        for name in JS_NAME_RE.findall(text):
            if origin in product:
                for target in by_name.get(Path(name).name, []):
                    edges[origin].add((target, "rt"))

    entries = {path for path in product if "webview" in path.parts}
    entries |= {src / "extension.ts", control_dir / "panel.ts", control_dir / "loopSetup" / "loopSetupPanel.ts"}
    entries = {path for path in entries if path in product}

    everything = closure(entries, edges, {"rt", "type"})
    runtime_only = closure(entries, edges, {"rt"})
    kept = closure(entries, edges, {"rt", "type"}, barrier=parked)

    rows = []
    for path, stage in stage_of.items():
        if path in parked:
            status = "parked"
        elif path in kept:
            status = "core"
        elif path in everything:
            status = "parked"
        else:
            status = "unreached"
        refs = "-" if path not in everything else ("rt" if path in runtime_only else "type")
        text = product[path]
        lines = text.count("\n") + (0 if not text or text.endswith("\n") else 1)
        tests = len(test_imports.get(path, ()))
        rows.append((STATUS_ORDER[status], path.relative_to(control_dir).as_posix(), status, refs, stage, tests, lines))
    rows.sort()

    print(f"{'status':<10} {'refs':<5} {'stage':<9} {'tests':>5} {'lines':>6}  file")
    for _, relative, status, refs, stage, tests, lines in rows:
        print(f"{status:<10} {refs:<5} {stage:<9} {tests:>5} {lines:>6}  {relative}")
    counts: dict[str, int] = defaultdict(int)
    for row in rows:
        counts[row[2]] += 1
    candidates = [row for row in rows if row[2] == "unreached" and row[4] in DELETABLE_STAGES]
    print()
    print(f"Files: {len(rows)} (core {counts['core']}, parked {counts['parked']}, unreached {counts['unreached']}); "
          f"lines {sum(row[6] for row in rows)}")
    print(f"Deletion candidates (unreached V0 to V5 files): {len(candidates)}, lines {sum(row[6] for row in candidates)}")
    for _, relative, _, _, stage, tests, _ in candidates:
        note = f"still imported by {tests} test suite(s)" if tests else "no importer"
        print(f"  {stage:<3} {relative}  ({note})")
    hubs: dict[Path, list[Path]] = defaultdict(list)
    for origin in kept:
        if origin in parked:
            continue
        for target, kind in edges.get(origin, ()):
            if kind == "rt" and target in parked:
                hubs[target].append(origin)
    if hubs:
        print(f"Parked (V7) modules that core files import at runtime: {len(hubs)} (the core path reaches their code):")
        for target in sorted(hubs, key=lambda item: item.relative_to(control_dir).as_posix()):
            importers = sorted(origin.relative_to(root).as_posix().removeprefix(CONTROL_STUDIO + "/")
                               for origin in set(hubs[target]))
            print(f"  {target.relative_to(control_dir).as_posix()} <- {', '.join(importers)}")
    missing = sorted(path for path in stages if not (control_dir / path).is_file())
    if missing:
        print(f"Listed in Appendix B but not present: {', '.join(missing)}")
    unlisted = sum(1 for row in rows if row[4] == "unlisted")
    print(f"Not in Appendix B (new engine files, generated types, webview files): {unlisted}")
    if unresolved:
        print("Unresolved relative imports (the product or a test suite names a missing file):")
        for origin, spec in unresolved:
            print(f"  {origin.relative_to(root).as_posix()}: {spec}")
    return 1 if unresolved else 0


if __name__ == "__main__":
    raise SystemExit(main())
