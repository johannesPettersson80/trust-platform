#!/usr/bin/env python3
"""Render Lesson 72 from its retained native worst-case comparison trace.

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
RED = "#dc2626"
PAPER = "#f7f9fc"

LEFT, RIGHT = 75.0, 855.0
LABELS = ["Nominal P", "Selected filtered PD"]


def read_trace() -> dict[str, dict[str, list[tuple[float, float]]]]:
    panels: dict[str, dict[str, list[tuple[float, float]]]] = {}
    with (ROOT / "flexible-airframe-robust-evidence-trace.csv").open(newline="", encoding="utf-8") as handle:
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
    iae_panel = panels["Worst integrated bending acceleration"]
    peak_panel = panels["Worst peak bending acceleration"]

    iae_max = max(y for _, y in iae_panel["Worst IAE"]) * 1.1
    peak_max = max(y for _, y in peak_panel["Worst peak"]) * 1.1

    i_top, i_bottom = 157.0, 317.0
    p_top, p_bottom = 407.0, 567.0

    body: list[str] = []
    body += axis_block(i_top, i_bottom, iae_max, "Worst integrated bending acceleration (g s)")
    body.append(series_points(iae_panel["Worst IAE"], i_top, i_bottom, iae_max, BLUE))
    body += axis_block(p_top, p_bottom, peak_max, "Worst peak bending acceleration (g)")
    body.append(series_points(peak_panel["Worst peak"], p_top, p_bottom, peak_max, RED))

    legend = (
        f'<line x1="55" y1="108" x2="83" y2="108" stroke="{BLUE}" stroke-width="3"/>'
        '<text x="89" y="113" font-size="12">Worst IAE</text>'
        f'<line x1="205" y1="108" x2="233" y2="108" stroke="{RED}" stroke-width="3"/>'
        '<text x="239" y="113" font-size="12">Worst peak</text>'
    )

    return f'''<svg xmlns="http://www.w3.org/2000/svg" role="img" aria-labelledby="title desc" viewBox="0 0 900 650">
<title id="title">Robust design improves the worst declared airframe case</title>
<desc id="desc">Nominal P and the selected filtered-PD candidate compared on their worst case across all 27 structured plants; case labels match the shipped trace and summary CSVs.</desc>
<defs><marker id="arrow" markerWidth="8" markerHeight="8" refX="7" refY="3" orient="auto"><path d="M0,0 L0,6 L8,3 z" fill="{INK}"/></marker></defs>
<rect width="900" height="650" fill="{PAPER}" />
<style>text{{font-family:system-ui,-apple-system,sans-serif;fill:{INK}}} .axis{{stroke:{AXIS};stroke-width:1}} .grid{{stroke:{GRID};stroke-width:1}}</style>
<text x="55" y="42" font-size="24" font-weight="700">Robust design improves the worst declared airframe case</text>
<text x="55" y="70" font-size="14">Nominal P and the selected filtered-PD candidate on their worst case across 27 structured plants.</text>
{legend}
{"".join(body)}
</svg>
'''


def main() -> None:
    (ROOT / "flexible-airframe-robust-evidence.svg").write_text(render(), encoding="utf-8")


if __name__ == "__main__":
    main()
