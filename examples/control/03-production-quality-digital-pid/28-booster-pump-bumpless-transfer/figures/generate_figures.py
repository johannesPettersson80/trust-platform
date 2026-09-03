#!/usr/bin/env python3
"""Render Lesson 28's bumpless-transfer figure from its retained trace."""

from __future__ import annotations

import csv
from pathlib import Path

ROOT = Path(__file__).resolve().parent
INK = "#243044"
GRID = "#d8e0eb"
AXIS = "#526078"
BLUE = "#0072b2"
ORANGE = "#d55e00"
DASH = "#555555"
PAPER = "#f7f9fc"

LEFT, RIGHT = 70.0, 860.0
P1_TOP, P1_BOTTOM = 155.0, 315.0
P2_TOP, P2_BOTTOM = 360.0, 520.0


def text(x: float, y: float, value: str, size: int = 11, anchor: str = "start", weight: str = "400") -> str:
    w = f' font-weight="{weight}"' if weight != "400" else ""
    return f'<text x="{x:.2f}" y="{y:.2f}" font-size="{size}" text-anchor="{anchor}"{w}>{value}</text>'


def hline(y: float, css: str = "grid") -> str:
    return f'<line class="{css}" x1="{LEFT}" y1="{y:.2f}" x2="{RIGHT}" y2="{y:.2f}" />'


def vline(x: float, top: float, bottom: float, css: str = "grid") -> str:
    return f'<line class="{css}" x1="{x:.2f}" y1="{top}" x2="{x:.2f}" y2="{bottom}" />'


def polyline(points: list[tuple[float, float]], colour: str, dash: bool = False) -> str:
    encoded = " ".join(f"{x:.2f},{y:.2f}" for x, y in points)
    da = ' stroke-dasharray="7 5"' if dash else ""
    return f'<polyline fill="none" stroke="{colour}" stroke-width="2.2"{da} points="{encoded}" />'


def y_ticks(lo: float, hi: float, n: int = 5) -> list[float]:
    if hi <= lo:
        hi = lo + 1.0
    pad = (hi - lo) * 0.15 or 1.0
    lo, hi = max(0.0, lo - pad), hi + pad
    return [hi - (hi - lo) * i / (n - 1) for i in range(n)]


def panel(top: float, bottom: float, title: str, x_labels: list[str], series: list[tuple[str, list[float], bool]]) -> list[str]:
    values = [v for _, vals, _ in series for v in vals]
    lo, hi = min(values), max(values)
    ticks = y_ticks(lo, hi)
    body = [text(LEFT, top - 12, title, 16, weight="700")]
    for t in ticks:
        y = bottom - (t - ticks[-1]) / (ticks[0] - ticks[-1]) * (bottom - top)
        body += [hline(y), text(LEFT - 8, y + 4, f"{t:.2f}", anchor="end")]

    def sx(i: int) -> float:
        return LEFT if len(x_labels) == 1 else LEFT + i / (len(x_labels) - 1) * (RIGHT - LEFT)

    def sy(v: float) -> float:
        return bottom - (v - ticks[-1]) / (ticks[0] - ticks[-1]) * (bottom - top)

    for i, label in enumerate(x_labels):
        body += [vline(sx(i), top, bottom), text(sx(i), bottom + 18, label, anchor="middle")]
    for colour, vals, dashed in series:
        body.append(polyline([(sx(i), sy(v)) for i, v in enumerate(vals)], colour, dashed))
    body.append(f'<rect class="axis" x="{LEFT}" y="{top}" width="{RIGHT - LEFT}" height="{bottom - top}" fill="none" />')
    return body


def legend(items: list[tuple[float, float, str, str, bool]]) -> list[str]:
    body = []
    for x, y, colour, label, dashed in items:
        da = ' stroke-dasharray="7 5"' if dashed else ""
        body.append(f'<line x1="{x:.1f}" y1="{y:.1f}" x2="{x + 28:.1f}" y2="{y:.1f}" stroke="{colour}" stroke-width="3"{da} />')
        body.append(text(x + 34, y + 5, label, 12))
    return body


def render() -> str:
    with (ROOT / "bumpless-transfer-trace.csv").open(newline="", encoding="utf-8") as handle:
        rows = list(csv.DictReader(handle))
    labels = [row["case"] for row in rows]

    last_applied = [float(row["last_applied"]) for row in rows]
    first_request = [float(row["first_new_request"]) for row in rows]
    output_step = [float(row["output_step"]) for row in rows]

    body: list[str] = []
    body += legend([
        (55, 96, DASH, "last applied", True),
        (183, 96, BLUE, "first new request", False),
        (348, 96, ORANGE, "output step", False),
    ])
    body += panel(P1_TOP, P1_BOTTOM, "Last applied and first requested valve output (%)", labels, [
        (DASH, last_applied, True),
        (BLUE, first_request, False),
    ])
    body += panel(P2_TOP, P2_BOTTOM, "Authority-transition output step (pp)", labels, [
        (ORANGE, output_step, False),
    ])
    body.append(text(465, 598, "decision case", 12, anchor="middle"))

    title = "Tracking aligns authority transfer"
    desc = "Configured Simulation evidence · same pump state; authority alignment changes"
    return f'''<svg xmlns="http://www.w3.org/2000/svg" role="img" aria-labelledby="title desc" viewBox="0 0 900 610">
<title id="title">{title}</title>
<desc id="desc">{desc}</desc>
<rect width="900" height="610" fill="{PAPER}"/>
<style>text{{font-family:system-ui,-apple-system,sans-serif;fill:{INK}}} .axis{{stroke:{AXIS};stroke-width:1}} .grid{{stroke:{GRID};stroke-width:1}} .marker{{stroke:#b43c32;stroke-width:1.5;stroke-dasharray:4 4}}</style>
{text(55, 42, title, 24, weight="700")}
{text(55, 68, desc, 14)}
{"".join(body)}
</svg>
'''


def main() -> None:
    (ROOT / "bumpless-transfer.svg").write_text(render(), encoding="utf-8")


if __name__ == "__main__":
    main()
