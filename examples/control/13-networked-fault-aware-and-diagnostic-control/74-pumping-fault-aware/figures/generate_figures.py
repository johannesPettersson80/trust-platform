#!/usr/bin/env python3
"""Render Lesson 74 from its retained native pumping-fault-aware trace."""

from __future__ import annotations

import csv
from pathlib import Path


ROOT = Path(__file__).resolve().parent
INK = "#172033"
MUTED = "#526078"
GRID = "#cbd5e1"
RED = "#dc2626"
BLUE = "#2563eb"
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
    with (ROOT / "pumping-fault-aware-evidence-trace.csv").open(newline="", encoding="utf-8") as handle:
        return list(csv.DictReader(handle))


def series(rows: list[dict[str, str]], panel: str, name: str) -> list[tuple[float, float]]:
    return sorted(
        (float(row["x"]), float(row["y"])) for row in rows
        if row["panel"] == panel and row["series"] == name
    )


def render() -> str:
    rows = load_rows()
    left, right = 92.0, 850.0
    top1, bottom1 = 150.0, 355.0
    top2, bottom2 = 435.0, 610.0
    x_max = 28.0
    x_ticks = [0, 5, 10, 15, 20, 25]
    sx = lambda v: left + v / x_max * (right - left)
    sy1 = lambda v: bottom1 - v / 6.0 * (bottom1 - top1)
    sy2 = lambda v: bottom2 - v / 100.0 * (bottom2 - top2)

    fault = series(rows, "Diagnostic and operating state codes", "Fault")
    mode = series(rows, "Diagnostic and operating state codes", "Mode")
    applied = series(rows, "Protected applied pump command", "Applied command")

    body: list[str] = [text(left, top1 - 12, "Fault code and operating mode", "label")]
    for value in range(0, 7):
        body += [line(left, sy1(value), right, sy1(value), "grid"), text(left - 12, sy1(value) + 4, str(value), anchor="end")]
    for value in x_ticks:
        body.append(line(sx(value), top1, sx(value), bottom1, "grid"))
    body += [
        line(left, top1, left, bottom1, "axis"), line(left, bottom1, right, bottom1, "axis"),
        polyline([(sx(x), sy1(y)) for x, y in fault], RED, 2.4),
        polyline([(sx(x), sy1(y)) for x, y in mode], BLUE, 2.0, dash="7 5"),
        text((left + right) / 2, bottom1 + 24, "Time after start (s)", "label", "middle"),
    ]

    body.append(text(left, top2 - 12, "Protected applied pump command (%)", "label"))
    for value in [0, 25, 50, 75, 100]:
        body += [line(left, sy2(value), right, sy2(value), "grid"), text(left - 12, sy2(value) + 4, str(value), anchor="end")]
    for value in x_ticks:
        body += [line(sx(value), top2, sx(value), bottom2, "grid"), text(sx(value), bottom2 + 24, str(value), anchor="middle")]
    body += [
        line(left, top2, left, bottom2, "axis"), line(left, bottom2, right, bottom2, "axis"),
        polyline([(sx(x), sy2(y)) for x, y in applied], GREEN, 2.4),
        text((left + right) / 2, bottom2 + 42, "Time after start (s)", "label", "middle"),
    ]

    body += [
        line(245, 100, 275, 100, "fault"), text(282, 104, "Fault code"),
        line(345, 100, 375, 100, "mode"), text(382, 104, "Mode"),
    ]
    return f'''<svg xmlns="http://www.w3.org/2000/svg" role="img" aria-labelledby="title desc" viewBox="0 0 900 660">
<title id="title">Fault-aware control isolates, degrades, and recovers explicitly</title>
<desc id="desc">Fault codes: 0 healthy, 1 pressure A, 3 ambiguous, 4 stale level, 5 low level, 6 actuator. Modes: 0 automatic, 1 degraded, 2 safe. The lower panel plots the same run's rate-limited protected applied pump command.</desc>
<rect width="900" height="660" rx="12" fill="{PAPER}" stroke="{GRID}" />
<style>
  text {{ font-family: system-ui, -apple-system, sans-serif; fill: {INK}; }}
  .title {{ font-size: 22px; font-weight: 700; fill: {INK}; }} .subtitle {{ font-size: 13px; fill: {MUTED}; }}
  .axis {{ stroke: #8792a5; stroke-width: 1.4; }} .grid {{ stroke: {GRID}; stroke-width: 1; stroke-dasharray: 3 5; }}
  .note {{ fill: {MUTED}; font-size: 11px; }} .label {{ font-size: 13px; font-weight: 700; fill: {INK}; }}
  .fault {{ stroke: {RED}; stroke-width: 3; }} .mode {{ stroke: {BLUE}; stroke-width: 2.4; stroke-dasharray: 7 5; }}
</style>
{text(48, 42, "Fault code, operating mode, and rate-limited recovery", "title")}
{text(48, 65, "Simulation evidence · shipped ST · 100 ms samples · seed 20260829", "subtitle")}
{"".join(body)}
</svg>
'''


def main() -> None:
    (ROOT / "pumping-fault-aware-evidence.svg").write_text(render(), encoding="utf-8")


if __name__ == "__main__":
    main()
