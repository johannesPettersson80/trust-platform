#!/usr/bin/env python3
"""Render Lesson 82's evidence figure from its retained native trace.

Pure Python, no third-party dependencies. Reads
figures/distillation-capstone-evidence-trace.csv and rewrites
figures/distillation-capstone-evidence.svg with the same two panels,
series, and axes the shipped figure uses. The architecture-path diagram
(figures/distillation-column-capstone-signal-path.svg) is a structural
topology diagram, not chart data, and is not reproducible from this CSV; it
is left untouched by this script.
"""

from __future__ import annotations

import csv
from pathlib import Path

ROOT = Path(__file__).resolve().parent

PANELS = [
    {
        "name": "Worst normal-envelope final error",
        "series": "Final error",
        "axis_title": "Worst normal-envelope final error (percentage points)",
        "y_min": 0.0,
        "y_max": 0.20,
        "colour": "#2563eb",
        "top": 195.0,
        "bottom": 355.0,
        "x_axis_label_y": 391.0,
        "legend_x": 55,
    },
    {
        "name": "Infeasible +2/+2 final error",
        "series": "Constraint-case error",
        "axis_title": "Infeasible +2/+2 final error (percentage points)",
        "y_min": 0.0,
        "y_max": 1.10,
        "colour": "#dc2626",
        "top": 445.0,
        "bottom": 605.0,
        "x_axis_label_y": 641.0,
        "legend_x": 220,
    },
]

LEFT, RIGHT = 75.0, 855.0
X_MIN, X_MAX = -0.10, 2.10


def read_trace() -> dict[str, list[tuple[float, float]]]:
    with (ROOT / "distillation-capstone-evidence-trace.csv").open(newline="", encoding="utf-8") as handle:
        rows = list(csv.DictReader(handle))
    by_panel: dict[str, list[tuple[float, float]]] = {}
    for row in rows:
        by_panel.setdefault(row["panel"], []).append((float(row["x"]), float(row["y"])))
    return by_panel


def sx(index: float) -> float:
    return LEFT + (index - X_MIN) / (X_MAX - X_MIN) * (RIGHT - LEFT)


def sy(value: float, y_min: float, y_max: float, top: float, bottom: float) -> float:
    return bottom - (value - y_min) / (y_max - y_min) * (bottom - top)


def render(by_panel: dict[str, list[tuple[float, float]]]) -> str:
    body: list[str] = []
    for panel in PANELS:
        lx = panel["legend_x"]
        body.append(
            f'<line x1="{lx}" y1="146" x2="{lx + 28}" y2="146" stroke="{panel["colour"]}" stroke-width="3"/>'
            f'<text x="{lx + 34}" y="151" font-size="12">{panel["series"]}</text>'
        )

    for panel in PANELS:
        points = by_panel[panel["name"]]
        top, bottom = panel["top"], panel["bottom"]
        y_min, y_max = panel["y_min"], panel["y_max"]
        body.append(f'<text x="75" y="{top - 12:.2f}" font-size="16" font-weight="700">{panel["axis_title"]}</text>')
        for step in range(5):
            value = y_min + (y_max - y_min) * (4 - step) / 4
            y = top + step / 4 * (bottom - top)
            body.append(f'<line class="grid" x1="75" y1="{y:.2f}" x2="855" y2="{y:.2f}"/>')
            body.append(f'<text x="67" y="{y + 4:.2f}" text-anchor="end" font-size="11">{value:.2f}</text>')
        for step in range(5):
            index = X_MIN + step / 4 * (X_MAX - X_MIN)
            x = sx(index)
            body.append(f'<line class="grid" x1="{x:.2f}" y1="{top}" x2="{x:.2f}" y2="{bottom}"/>')
            body.append(f'<text x="{x:.2f}" y="{bottom + 18:.2f}" text-anchor="middle" font-size="11">{index:.2f}</text>')
        for index, value in points:
            body.append(
                f'<circle cx="{sx(index):.2f}" cy="{sy(value, y_min, y_max, top, bottom):.2f}" r="4" fill="{panel["colour"]}"/>'
            )
        body.append(f'<rect class="axis" x="75" y="{top}" width="780" height="{bottom - top}" fill="none"/>')
        body.append(f'<text x="465" y="{panel["x_axis_label_y"]:.2f}" text-anchor="middle" font-size="12">architecture index</text>')

    return f'''<svg xmlns="http://www.w3.org/2000/svg" role="img" aria-labelledby="title desc" viewBox="0 0 900 650">
<title id="title">Static decoupling earns the normal-envelope distillation recommendation</title>
<desc id="desc">Indices 0, 1, and 2 are direct pairing, static decoupling, and central constraint coordination; the infeasible request remains labelled infeasible.</desc>
<defs><marker id="arrow" markerWidth="8" markerHeight="8" refX="7" refY="3" orient="auto"><path d="M0,0 L0,6 L8,3 z" fill="#243044"/></marker></defs>
<rect width="900" height="650" fill="#f7f9fc"/>
<style>text{{font-family:system-ui,-apple-system,sans-serif;fill:#243044}} .axis{{stroke:#526078;stroke-width:1}} .grid{{stroke:#d8e0eb;stroke-width:1}}</style>
<text x="55" y="42" font-size="24" font-weight="700">Static decoupling earns the normal-envelope distillation</text>
<text x="55" y="70" font-size="24" font-weight="700">recommendation</text>
<text x="55" y="98" font-size="14">Indices 0, 1, and 2 are direct pairing, static decoupling, and central constraint</text>
<text x="55" y="116" font-size="14">coordination; the infeasible request remains labelled infeasible.</text>
{"".join(body)}
</svg>
'''


def main() -> None:
    by_panel = read_trace()
    svg = render(by_panel)
    (ROOT / "distillation-capstone-evidence.svg").write_text(svg, encoding="utf-8")


if __name__ == "__main__":
    main()
