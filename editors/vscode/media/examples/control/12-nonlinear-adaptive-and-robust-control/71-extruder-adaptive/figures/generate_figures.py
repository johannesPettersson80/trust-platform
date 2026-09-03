#!/usr/bin/env python3
"""Render Lesson 71 from its retained native architecture-comparison trace.

Reads the two shipped evidence CSVs and writes the results SVG. Pure
Python, no third-party dependencies, matching the pattern used by
examples/control/*/08-*/figures/generate_figures.py.
"""

from __future__ import annotations

import csv
from pathlib import Path

ROOT = Path(__file__).resolve().parent
INK = "#243044"
GRID = "#d8e0eb"
AXIS = "#526078"
BLUE = "#2563eb"
GREEN = "#16a34a"
PAPER = "#f7f9fc"

LEFT, RIGHT = 75.0, 855.0
LABELS = ["Fixed PI", "Recipe-scheduled PI", "Supervised adaptive PI"]


def read_trace() -> dict[str, dict[str, list[tuple[float, float]]]]:
    panels: dict[str, dict[str, list[tuple[float, float]]]] = {}
    with (ROOT / "extruder-adaptive-evidence-trace.csv").open(newline="", encoding="utf-8") as handle:
        for row in csv.DictReader(handle):
            series = panels.setdefault(row["panel"], {})
            series.setdefault(row["series"], []).append(
                (float(row["x"]), float(row["y"]))
            )
    return panels


def axis_block(top: float, bottom: float, y_max: float, title: str) -> list[str]:
    sy = lambda value: bottom - (value / y_max) * (bottom - top)
    sx = lambda point: LEFT + (point / (len(LABELS) - 1)) * (RIGHT - LEFT)
    body = [f'<text x="{LEFT:.0f}" y="{top - 12:.0f}" font-size="16" font-weight="700">{title}</text>']
    for step in range(5):
        value = y_max * (4 - step) / 4
        y = sy(value)
        body.append(f'<line class="grid" x1="{LEFT:.0f}" y1="{y:.2f}" x2="{RIGHT:.0f}" y2="{y:.2f}" />')
        body.append(f'<text x="{LEFT - 8:.0f}" y="{y + 4:.2f}" text-anchor="end" font-size="11">{value:.2f}</text>')
    for point, label in enumerate(LABELS):
        x = sx(float(point))
        body.append(f'<line class="grid" x1="{x:.2f}" y1="{top:.0f}" x2="{x:.2f}" y2="{bottom:.0f}" />')
        body.append(f'<text x="{x:.2f}" y="{bottom + 18:.0f}" text-anchor="middle" font-size="11">{label}</text>')
    body.append(f'<rect class="axis" x="{LEFT:.0f}" y="{top:.0f}" width="{RIGHT - LEFT:.0f}" height="{bottom - top:.0f}" fill="none" />')
    return body


def series_points(points: list[tuple[float, float]], top: float, bottom: float, y_max: float, colour: str) -> str:
    sy = lambda value: bottom - (value / y_max) * (bottom - top)
    sx = lambda point: LEFT + (point / (len(LABELS) - 1)) * (RIGHT - LEFT)
    return "".join(
        f'<circle cx="{sx(point):.2f}" cy="{sy(value):.2f}" r="4" fill="{colour}"/>'
        for point, value in sorted(points, key=lambda item: item[0])
    )


def render() -> str:
    panels = read_trace()
    error_panel = panels["Grade-B tracking error"]
    movement_panel = panels["Cumulative screw-speed command movement"]

    error_max = max(y for _, y in error_panel["IAE"]) * 1.1
    movement_max = max(y for _, y in movement_panel["Movement"]) * 1.1

    e_top, e_bottom = 177.0, 337.0
    m_top, m_bottom = 427.0, 587.0

    body: list[str] = []
    body += axis_block(e_top, e_bottom, error_max, "Grade-B tracking error (bar s)")
    body.append(series_points(error_panel["IAE"], e_top, e_bottom, error_max, BLUE))
    body += axis_block(m_top, m_bottom, movement_max, "Cumulative screw-speed command movement (percentage points)")
    body.append(series_points(movement_panel["Movement"], m_top, m_bottom, movement_max, GREEN))

    legend = (
        f'<line x1="55" y1="128" x2="83" y2="128" stroke="{BLUE}" stroke-width="3"/>'
        '<text x="89" y="133" font-size="12">IAE</text>'
    )

    return f'''<svg xmlns="http://www.w3.org/2000/svg" role="img" aria-labelledby="title desc" viewBox="0 0 900 650">
<title id="title">Adaptation earns authority only from informative healthy data</title>
<desc id="desc">Fixed, recipe-scheduled, and supervised-adaptive PI compared on the common grade-B window; case labels match the shipped trace and summary CSVs.</desc>
<defs><marker id="arrow" markerWidth="8" markerHeight="8" refX="7" refY="3" orient="auto"><path d="M0,0 L0,6 L8,3 z" fill="{INK}"/></marker></defs>
<rect width="900" height="650" fill="{PAPER}" />
<style>text{{font-family:system-ui,-apple-system,sans-serif;fill:{INK}}} .axis{{stroke:{AXIS};stroke-width:1}} .grid{{stroke:{GRID};stroke-width:1}}</style>
<text x="55" y="42" font-size="24" font-weight="700">Adaptation earns authority only from informative healthy</text>
<text x="55" y="70" font-size="24" font-weight="700">data</text>
<text x="55" y="98" font-size="14">Fixed, recipe-scheduled, and supervised-adaptive PI on the common grade-B window.</text>
{legend}
{"".join(body)}
</svg>
'''


def main() -> None:
    (ROOT / "extruder-adaptive-evidence.svg").write_text(render(), encoding="utf-8")


if __name__ == "__main__":
    main()
