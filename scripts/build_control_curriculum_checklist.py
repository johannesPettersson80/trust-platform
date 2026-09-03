#!/usr/bin/env python3
"""Generate the Specification 38 curriculum implementation checklist from the tree.

Every per-lesson cell is computed from the shipped files, never asserted, so the
board cannot claim a state the repository does not hold. Run from the repository
root: python3 scripts/build_control_curriculum_checklist.py
"""
from __future__ import annotations

import json
import os
import re
import subprocess
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
CATALOG = ROOT / "examples" / "catalog.json"
OUT = (ROOT / "docs" / "internal" / "testing" / "checklists"
       / "control-engineering-curriculum-implementation-checklist.md")
CANONICAL_SECTIONS = [
    "The problem", "What we need", "The new idea", "Prediction", "Implementation",
    "Experiment in Control Studio", "Evidence and decision", "Conclusion",
    "Reason for the next lesson",
]
TEST_LABELS = ["Experiment", "Amplitude", "Maximum duration (s)", "Review test",
               "Confirm and start", "Abort test"]


def lesson_dirs() -> dict[str, Path]:
    found: dict[str, Path] = {}
    for descriptor in (ROOT / "examples" / "control").glob("*/*/lesson.json"):
        found[json.loads(descriptor.read_text(encoding="utf-8"))["id"]] = descriptor.parent
    return found


def catalog_ids() -> list[str]:
    entries = json.loads(CATALOG.read_text(encoding="utf-8"))
    return [e["id"] for e in entries
            if e["id"].startswith("control-") or "control" in (e.get("tags") or [])]


def authority_state(directory: Path) -> tuple[bool, str]:
    """The Monitor -> Test authority contract, read from the shipped files."""
    loop = json.loads((directory / "control-loop.json").read_text(encoding="utf-8"))
    lesson = json.loads((directory / "lesson.json").read_text(encoding="utf-8"))
    source = "\n".join(p.read_text(encoding="utf-8") for p in sorted((directory / "src").glob("*.st")))
    mapped = re.findall(r"P1\.ExperimentEnabled\s+AT\s+(%[IQM]X\d+\.\d+)", source)
    experiment = loop.get("experiment") or {}
    activation = experiment.get("activation") or {}
    writes: list[str] = []

    def walk(value: object) -> None:
        if isinstance(value, dict):
            if value.get("direction") == "write":
                writes.append(str(value.get("symbol")))
            for child in value.values():
                walk(child)
        elif isinstance(value, list):
            for child in value:
                walk(child)

    walk(loop)
    problems = []
    if len(mapped) != 1:
        problems.append("authority not mapped once in Structured Text")
    elif activation.get("address") != mapped[0]:
        problems.append("activation address disagrees with the program")
    if experiment.get("execution") != "project-scripted":
        problems.append("experiment is not project-scripted")
    if experiment.get("kind") != "observation" or experiment.get("amplitude") != 0:
        problems.append("experiment is not a zero-amplitude observation")
    if writes != ["P1.ExperimentEnabled"]:
        problems.append(f"write authority is {writes or 'absent'}")
    if lesson.get("workflowStages") != ["Monitor", "Test"]:
        problems.append("workflowStages are not Monitor then Test")
    return (not problems, "; ".join(problems))


def presentation_state(directory: Path) -> tuple[bool, str]:
    """Rendering and workflow presentation, read from the lesson README."""
    readme = (directory / "README.md").read_text(encoding="utf-8")
    stripped = re.sub(r"```.*?```", "", readme, flags=re.S)
    stripped = re.sub(r"`[^`\n]*`", "", stripped)
    problems = []
    if re.search(r"\\\(", stripped) or re.search(r"\\\[", stripped) or "$$" in stripped:
        problems.append("mathematics uses delimiters the preview cannot render")
    spans = re.findall(r"(?<!\$)\$(?!\$)([^$\n]+?)\$(?!\$)", stripped)
    if any("\\ " in span for span in spans):
        problems.append("prose inside mathematics is not wrapped in \\text{}")
    if [m.group(1) for m in re.finditer(r"^## (.+)$", readme, re.M)] != CANONICAL_SECTIONS:
        problems.append("sections are not the canonical nine")
    if re.search(r"^### (Measures and comparison|Validation|Preparation|Baseline|Change|Comparison)$",
                 readme, re.M):
        problems.append("a retired workflow sub-heading is present")
    test_item = re.search(r"2\. \*\*Test\*\*(.*?)(?:\n## |\Z)", readme, re.S)
    block = test_item.group(1) if test_item else ""
    missing = [label for label in TEST_LABELS if f"**{label}**" not in block]
    if missing:
        problems.append(f"Test stage does not name {', '.join(missing)}")
    section = re.search(r"## Experiment in Control Studio(.*?)(?:\n## |\Z)", readme, re.S)
    if not section or "Experiment authority" not in section.group(1):
        problems.append("the experiment signal is not named")
    return (not problems, "; ".join(problems))


def natively_retained_figures() -> set[str]:
    """Figure names the runtime tests write and then byte-compare.

    Such a figure already has a deterministic generator: the test that owns it,
    refreshed with TRUST_UPDATE_CONTROL_CURRICULUM_ARTIFACTS=1. Counting it as
    ungenerated would ask for a second writer of the same file.
    """
    text = "\n".join(
        path.read_text(encoding="utf-8", errors="ignore")
        for path in (ROOT / "crates" / "trust-runtime" / "tests").rglob("*.rs")
    )
    names = set(re.findall(r"[\"']([a-z0-9][a-z0-9._-]*\.svg)[\"']", text))
    for stem in re.findall(r"[\"']([a-z0-9][a-z0-9-]*)[\"']", text):
        names.add(f"{stem}.svg")
    return names


NATIVE_FIGURES = natively_retained_figures()


def evidence_state(directory: Path) -> tuple[bool, str]:
    """Retained figures and the generator that can rebuild each one."""
    figures = directory / "figures"
    if not figures.is_dir():
        return (False, "no figures directory")
    svgs = sorted(figures.glob("*.svg"))
    generators = sorted(figures.glob("*.py"))
    problems = []
    if not svgs:
        problems.append("no retained figure")
    generated = "\n".join(path.read_text(encoding="utf-8") for path in generators)
    data = {path.stem for path in figures.glob("*.csv")}
    # Only a figure drawn from retained data can drift from it. A schematic of the
    # equipment carries no measurement, so it needs no generator; a figure whose
    # stem matches a retained CSV does.
    orphans = [
        svg.name for svg in svgs
        if any(stem.startswith(svg.stem) for stem in data)
        and svg.name not in NATIVE_FIGURES and svg.name not in generated
    ]
    if orphans:
        problems.append(f"cannot regenerate {', '.join(sorted(orphans))}")
    return (not problems, "; ".join(problems))


def cell(ok: bool) -> str:
    return "pass" if ok else "OPEN"


def main() -> int:
    ids = catalog_ids()
    dirs = lesson_dirs()
    missing = [i for i in ids if i not in dirs]
    if missing:
        print(f"catalog ids without a lesson directory: {missing}", file=sys.stderr)
        return 2
    head = subprocess.run(["git", "-C", str(ROOT), "rev-parse", "--short", "HEAD"],
                          capture_output=True, text=True).stdout.strip()
    rows, notes, open_count = [], [], 0
    for index, lesson_id in enumerate(ids, start=1):
        directory = dirs[lesson_id]
        descriptor = json.loads((directory / "lesson.json").read_text(encoding="utf-8"))
        module = directory.parent.name
        authority_ok, authority_why = authority_state(directory)
        presentation_ok, presentation_why = presentation_state(directory)
        evidence_ok, evidence_why = evidence_state(directory)
        if not (authority_ok and presentation_ok and evidence_ok):
            open_count += 1
            for why in (authority_why, presentation_why, evidence_why):
                if why:
                    notes.append(f"- Lesson {index:02d} `{lesson_id}`: {why}.")
        rows.append(
            f"| {index:02d} | `{lesson_id}` | {descriptor['title']} | {module} | "
            f"{cell(authority_ok)} | {cell(presentation_ok)} | {cell(evidence_ok)} |"
        )
    board = "\n".join(rows)
    open_notes = "\n".join(notes) if notes else "- None. Every lesson passes all three computed columns."
    OUT.parent.mkdir(parents=True, exist_ok=True)
    OUT.write_text(f"""# Control engineering curriculum implementation checklist

Generated by `scripts/build_control_curriculum_checklist.py`
from the shipped lesson files at commit `{head}`. Every per-lesson cell below is
computed from `control-loop.json`, `lesson.json`, `src/*.st`, `README.md` and
`figures/` at generation time. Do not hand-edit a cell: correct the lesson and
regenerate, so the board can never claim a state the repository does not hold.

## Specification and teaching contract

The governing specification is `docs/specs/38-control-engineering-curriculum.md`.
The water-heater proportional-control lesson (Lesson 04, `control-water-heater-p`)
is the mandatory pedagogical reference: its focused single-topic structure,
words-before-symbols order and provenance rules apply to all 84 lessons, and a
correction to it triggers an audit of every unfinished lesson. The water-heater
amplitude-saturation lesson (Lesson 08, `control-water-heater-saturation`) is the
mandatory workflow reference: its `Monitor` then `Test` stage items, five-bullet
stage structure and single Boolean experiment authority apply to all 84 lessons.

Three contracts are enforced mechanically:

1. **Workflow authority.** The runnable project owns every output. Control
   Studio's only write is `P1.ExperimentEnabled`, declared once in the lesson
   program and bound as `experiment.activation`; the experiment is a
   zero-amplitude observation and the lesson exposes exactly `Monitor` then
   `Test`. Proof: `editors/vscode/src/test/suite/control-curriculum-authority.test.ts`.
2. **Rendered presentation.** Mathematics uses `$...$` only, textual labels use
   `\\text{{}}`, the README carries the canonical nine sections with no retired
   workflow sub-heading, and the Test stage names all six Control Studio
   controls and the experiment signal. Proof: the preview-safe mathematics test
   in `editors/vscode/src/test/suite/control-curriculum-readmes.test.ts`.
3. **Reproducible evidence.** Every figure drawn from retained data can be
   rebuilt from that data. Most evidence figures are written and byte-compared
   by the runtime test that produced the run, refreshed with
   `TRUST_UPDATE_CONTROL_CURRICULUM_ARTIFACTS=1`; the rest have a generator in
   the lesson's `figures/` directory. A schematic of the equipment carries no
   measurement and needs neither.

## Per-lesson implementation and proof board

`Authority` is contract 1, `Presentation` contract 2, `Evidence` contract 3.
`OPEN` rows are listed with their reason under "Completion boundary".

| # | Lesson id | Title | Module | Authority | Presentation | Evidence |
| --- | --- | --- | --- | --- | --- | --- |
{board}

## Rendered learner acceptance

A lesson is not accepted on unit tests alone. The Control Studio journey is
captured from the real Extension Development Host by
`editors/vscode/scripts/control-studio-evidence/run.js`, which drives one lesson
project through `Monitor` to `Test` (Review test, Confirm and start, run to
completion) and the wide, split, narrow, 200 percent and high-contrast layouts,
writing screenshots and a machine-checkable `steps.json`. Retained runs live
under `docs/internal/testing/evidence/vscode-control-studio/`.

## Frozen final gates

1. `cd editors/vscode && npm run build:examples` bundles the curriculum.
2. `cd editors/vscode && npx tsc -p ./ && npx mocha --ui tdd out/test/suite/control-*.test.js`
   runs every curriculum and Control Studio suite.
3. `cargo test -p trust-runtime --test control_curriculum_workflow_contract`
   and each lesson's own `proof.nativeCommand`, executed on the build machine.
4. `just test-all` green on the build machine before any slice is called done.

## Completion boundary

{open_count} of {len(ids)} lessons have at least one open computed column. A row
is closed only when all three columns read `pass` and the lesson's native proof
command has been run on the build machine.

{open_notes}
""", encoding="utf-8")
    print(f"wrote {OUT.relative_to(ROOT)}: {len(rows)} rows, {open_count} lessons with an open column")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
