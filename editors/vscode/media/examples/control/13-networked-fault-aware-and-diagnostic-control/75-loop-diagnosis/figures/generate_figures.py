#!/usr/bin/env python3
"""Render Lesson 75 from its retained native loop-diagnosis casebook trace."""

from __future__ import annotations

import csv
from pathlib import Path


ROOT = Path(__file__).resolve().parent
INK = "#172033"
MUTED = "#526078"
GRID = "#cbd5e1"
BLUE = "#2563eb"
RED = "#dc2626"
GREEN = "#16a34a"
PAPER = "#f8fafc"


def text(x: float, y: float, value: str, css: str = "note", anchor: str = "start") -> str:
    return f'<text x="{x:.1f}" y="{y:.1f}" class="{css}" text-anchor="{anchor}">{value}</text>'


def line(x1: float, y1: float, x2: float, y2: float, css: str) -> str:
    return f'<line x1="{x1:.1f}" y1="{y1:.1f}" x2="{x2:.1f}" y2="{y2:.1f}" class="{css}" />'


def polyline(points: list[tuple[float, float]], colour: str, width: float = 2.2, dash: str = "") -> str:
    encoded = " ".join(f"{x:.1f},{y:.1f}" for x, y in points)
    dashed = f' stroke-dasharray="{dash}"' if dash else ""
    return f'<polyline points="{encoded}" fill="none" stroke="{colour}" stroke-width="{width}"{dashed} />'


def load_rows() -> list[dict[str, str]]:
    with (ROOT / "loop-diagnosis-evidence-trace.csv").open(newline="", encoding="utf-8") as handle:
        return list(csv.DictReader(handle))


def series(rows: list[dict[str, str]], panel: str, name: str) -> list[tuple[float, float]]:
    return sorted(
        (float(row["x"]), float(row["y"])) for row in rows
        if row["panel"] == panel and row["series"] == name
    )


def render() -> str:
    rows = load_rows()
    left, right = 92.0, 850.0
    top1, bottom1 = 150.0, 345.0
    top2, bottom2 = 425.0, 610.0
    x_max = 60.0
    x_ticks = [0, 10, 20, 30, 40, 50, 60]
    sx = lambda v: left + v / x_max * (right - left)
    sy1 = lambda v: bottom1 - v / 100.0 * (bottom1 - top1)
    sy2 = lambda v: bottom2 - (v - 48.0) / 8.0 * (bottom2 - top2)

    demand = series(rows, "Stiction fingerprint: demand moves while stem holds", "Requested valve")
    stem = series(rows, "Stiction fingerprint: demand moves while stem holds", "Measured stem")
    pv = series(rows, "Sensor drift: measured PV (red); reference (green dashed)", "Measured PV")
    reference = series(rows, "Sensor drift: measured PV (red); reference (green dashed)", "Independent reference")

    body: list[str] = [
        text(left, top1 - 30, "Stiction fingerprint (case-2.toml): demand moves while stem holds (%)", "label"),
        line(left, top1 - 12, left + 30, top1 - 12, "demand"), text(left + 37, top1 - 8, "Requested valve demand"),
        line(left + 220, top1 - 12, left + 250, top1 - 12, "stem"), text(left + 257, top1 - 8, "Measured stem position"),
    ]
    for value in [0, 25, 50, 75, 100]:
        body += [line(left, sy1(value), right, sy1(value), "grid"), text(left - 12, sy1(value) + 4, str(value), anchor="end")]
    for value in x_ticks:
        body.append(line(sx(value), top1, sx(value), bottom1, "grid"))
    body += [
        line(left, top1, left, bottom1, "axis"), line(left, bottom1, right, bottom1, "axis"),
        polyline([(sx(x), sy1(y)) for x, y in demand], BLUE, 2.2),
        polyline([(sx(x), sy1(y)) for x, y in stem], RED, 2.2, dash="7 5"),
        text((left + right) / 2, bottom1 + 24, "Time after start (s)", "label", "middle"),
    ]

    body += [
        text(left, top2 - 30, "Sensor drift (case-3.toml): primary and reference steam flow (t/h)", "label"),
        line(left, top2 - 12, left + 30, top2 - 12, "pv"), text(left + 37, top2 - 8, "Primary steam flow (measured PV)"),
        line(left + 260, top2 - 12, left + 290, top2 - 12, "reference"), text(left + 297, top2 - 8, "Independent reference flow"),
    ]
    for value in [48, 50, 52, 54, 56]:
        body += [line(left, sy2(value), right, sy2(value), "grid"), text(left - 12, sy2(value) + 4, str(value), anchor="end")]
    for value in x_ticks:
        body += [line(sx(value), top2, sx(value), bottom2, "grid"), text(sx(value), bottom2 + 24, str(value), anchor="middle")]
    body += [
        line(left, top2, left, bottom2, "axis"), line(left, bottom2, right, bottom2, "axis"),
        polyline([(sx(x), sy2(y)) for x, y in pv], RED, 2.2),
        polyline([(sx(x), sy2(y)) for x, y in reference], GREEN, 2.2, dash="7 5"),
        text((left + right) / 2, bottom2 + 42, "Time after start (s)", "label", "middle"),
    ]

    return f'''<svg xmlns="http://www.w3.org/2000/svg" role="img" aria-labelledby="title desc" viewBox="0 0 900 660">
<title id="title">Diagnosis follows physical evidence before retuning</title>
<desc id="desc">Valve stem feedback exposes stiction in case-2; the independent flow reference exposes sensor drift in case-3.</desc>
<rect width="900" height="660" rx="12" fill="{PAPER}" stroke="{GRID}" />
<style>
  text {{ font-family: system-ui, -apple-system, sans-serif; fill: {INK}; }}
  .title {{ font-size: 22px; font-weight: 700; fill: {INK}; }} .subtitle {{ font-size: 13px; fill: {MUTED}; }}
  .axis {{ stroke: #8792a5; stroke-width: 1.4; }} .grid {{ stroke: {GRID}; stroke-width: 1; stroke-dasharray: 3 5; }}
  .note {{ fill: {MUTED}; font-size: 11px; }} .label {{ font-size: 13px; font-weight: 700; fill: {INK}; }}
  .demand {{ stroke: {BLUE}; stroke-width: 2.2; }} .stem {{ stroke: {RED}; stroke-width: 2.2; stroke-dasharray: 7 5; }}
  .pv {{ stroke: {RED}; stroke-width: 2.2; }} .reference {{ stroke: {GREEN}; stroke-width: 2.2; stroke-dasharray: 7 5; }}
</style>
{text(48, 42, "Diagnosis follows physical evidence before retuning", "title")}
{text(48, 65, "Simulation evidence · shipped ST · 100 ms samples · seed 20260829", "subtitle")}
{"".join(body)}
</svg>
'''


def main() -> None:
    (ROOT / "loop-diagnosis-evidence.svg").write_text(render(), encoding="utf-8")


if __name__ == "__main__":
    main()
