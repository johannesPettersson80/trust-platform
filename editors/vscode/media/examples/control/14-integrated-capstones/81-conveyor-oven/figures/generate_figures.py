#!/usr/bin/env python3
"""Render Lesson 81's evidence figure from its retained native trace.

Pure Python, no third-party dependencies. Reads
figures/conveyor-oven-capstone-evidence-trace.csv and rewrites
figures/conveyor-oven-capstone-evidence.svg with the same two panels,
series, and axes the shipped figure uses. The signal-path diagram
(figures/conveyor-oven-capstone-signal-path.svg) is a structural topology
diagram, not chart data, and is not reproducible from this CSV; it is left
untouched by this script.
"""

from __future__ import annotations

import csv
from pathlib import Path

ROOT = Path(__file__).resolve().parent

# Panel order and axis ceiling match the shipped figure exactly. The ceiling
# is a round number comfortably above the panel's largest retained value.
PANELS = [
    {
        "name": "Setpoint integrated absolute error",
        "series": "Setpoint IAE",
        "axis_title": "Setpoint integrated absolute error (degC*s)",
        "y_max": 600.0,
        "colour": "#2563eb",
        "top": 195.0,
        "bottom": 355.0,
        "x_axis_label_y": 391.0,
    },
    {
        "name": "Regulatory integrated absolute error",
        "series": "Regulatory IAE",
        "axis_title": "Regulatory integrated absolute error (degC*s)",
        "y_max": 60.0,
        "colour": "#16a34a",
        "top": 445.0,
        "bottom": 605.0,
        "x_axis_label_y": 641.0,
    },
]

LEFT, RIGHT = 75.0, 855.0
X_MIN, X_MAX = -0.05, 1.05


def read_trace() -> dict[str, list[tuple[float, float]]]:
    with (ROOT / "conveyor-oven-capstone-evidence-trace.csv").open(newline="", encoding="utf-8") as handle:
        rows = list(csv.DictReader(handle))
    by_panel: dict[str, list[tuple[float, float]]] = {}
    for row in rows:
        by_panel.setdefault(row["panel"], []).append((float(row["x"]), float(row["y"])))
    return by_panel


def sx(index: float) -> float:
    return LEFT + (index - X_MIN) / (X_MAX - X_MIN) * (RIGHT - LEFT)


def sy(value: float, y_max: float, top: float, bottom: float) -> float:
    return bottom - value / y_max * (bottom - top)


def render(by_panel: dict[str, list[tuple[float, float]]]) -> str:
    body: list[str] = []
    body.append(
        '<line x1="55" y1="146" x2="83" y2="146" stroke="#2563eb" stroke-width="3"/>'
        '<text x="89" y="151" font-size="12">Setpoint IAE</text>'
    )
    body.append(
        '<line x1="200" y1="146" x2="228" y2="146" stroke="#16a34a" stroke-width="3"/>'
        '<text x="234" y="151" font-size="12">Regulatory IAE</text>'
    )

    for panel in PANELS:
        points = by_panel[panel["name"]]
        top, bottom, y_max = panel["top"], panel["bottom"], panel["y_max"]
        body.append(f'<text x="75" y="{top - 12:.2f}" font-size="16" font-weight="700">{panel["axis_title"]}</text>')
        for step in range(5):
            value = y_max * (4 - step) / 4
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
                f'<circle cx="{sx(index):.2f}" cy="{sy(value, y_max, top, bottom):.2f}" r="4" fill="{panel["colour"]}"/>'
            )
        body.append(f'<rect class="axis" x="75" y="{top}" width="780" height="{bottom - top}" fill="none"/>')
        body.append(f'<text x="465" y="{panel["x_axis_label_y"]:.2f}" text-anchor="middle" font-size="12">design index</text>')

    return f'''<svg xmlns="http://www.w3.org/2000/svg" role="img" aria-labelledby="title desc" viewBox="0 0 900 650">
<title id="title">Delay compensation earns authority only on qualified model evidence</title>
<desc id="desc">Index 0 is ordinary PI; index 1 is Smith prediction for setpoint response and measured feedforward for disturbance rejection.</desc>
<defs><marker id="arrow" markerWidth="8" markerHeight="8" refX="7" refY="3" orient="auto"><path d="M0,0 L0,6 L8,3 z" fill="#243044"/></marker></defs>
<rect width="900" height="650" fill="#f7f9fc"/>
<style>text{{font-family:system-ui,-apple-system,sans-serif;fill:#243044}} .axis{{stroke:#526078;stroke-width:1}} .grid{{stroke:#d8e0eb;stroke-width:1}}</style>
<text x="55" y="42" font-size="24" font-weight="700">Delay compensation earns authority only on qualified model</text>
<text x="55" y="70" font-size="24" font-weight="700">evidence</text>
<text x="55" y="98" font-size="14">Index 0 is ordinary PI; index 1 is Smith prediction for setpoint response and measured</text>
<text x="55" y="116" font-size="14">feedforward for disturbance rejection.</text>
{"".join(body)}
</svg>
'''


def main() -> None:
    by_panel = read_trace()
    svg = render(by_panel)
    (ROOT / "conveyor-oven-capstone-evidence.svg").write_text(svg, encoding="utf-8")


if __name__ == "__main__":
    main()
