#!/usr/bin/env python3
"""Render Lesson 70 from its retained native architecture-comparison trace.

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
LABELS = ["Fixed PID", "Scheduled PID", "Feedback linearisation"]


def read_trace() -> dict[str, dict[str, list[tuple[float, float]]]]:
    panels: dict[str, dict[str, list[tuple[float, float]]]] = {}
    with (ROOT / "robot-joint-nonlinear-evidence-trace.csv").open(newline="", encoding="utf-8") as handle:
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
    error_panel = panels["Integrated absolute position error"]
    movement_panel = panels["Cumulative motor-torque movement"]

    error_max = max(y for series in error_panel.values() for _, y in series) * 1.1
    movement_max = max(y for series in movement_panel.values() for _, y in series) * 1.1

    e_top, e_bottom = 157.0, 317.0
    m_top, m_bottom = 407.0, 567.0

    body: list[str] = []
    body += axis_block(e_top, e_bottom, error_max, "Integrated absolute position error (rad s)")
    body.append(series_points(error_panel["Nominal payload"], e_top, e_bottom, error_max, BLUE))
    body.append(series_points(error_panel["Heavy payload"], e_top, e_bottom, error_max, RED))
    body += axis_block(m_top, m_bottom, movement_max, "Cumulative motor-torque movement (N m)")
    body.append(series_points(movement_panel["Nominal payload"], m_top, m_bottom, movement_max, BLUE))
    body.append(series_points(movement_panel["Heavy payload"], m_top, m_bottom, movement_max, RED))

    legend = (
        f'<line x1="55" y1="108" x2="83" y2="108" stroke="{BLUE}" stroke-width="3"/>'
        '<text x="89" y="113" font-size="12">Nominal payload</text>'
        f'<line x1="205" y1="108" x2="233" y2="108" stroke="{RED}" stroke-width="3" stroke-dasharray="7 5"/>'
        '<text x="239" y="113" font-size="12">Heavy payload</text>'
    )

    return f'''<svg xmlns="http://www.w3.org/2000/svg" role="img" aria-labelledby="title desc" viewBox="0 0 900 650">
<title id="title">Wide-range nonlinear control must survive payload change</title>
<desc id="desc">Fixed, scheduled, and feedback-linearised architectures compared on nominal and heavy-payload plants; case labels match the shipped trace and summary CSVs.</desc>
<defs><marker id="arrow" markerWidth="8" markerHeight="8" refX="7" refY="3" orient="auto"><path d="M0,0 L0,6 L8,3 z" fill="{INK}"/></marker></defs>
<rect width="900" height="650" fill="{PAPER}" />
<style>text{{font-family:system-ui,-apple-system,sans-serif;fill:{INK}}} .axis{{stroke:{AXIS};stroke-width:1}} .grid{{stroke:{GRID};stroke-width:1}}</style>
<text x="55" y="42" font-size="24" font-weight="700">Wide-range nonlinear control must survive payload change</text>
<text x="55" y="70" font-size="14">Fixed, scheduled, and feedback-linearised architectures on nominal and heavy-payload plants.</text>
{legend}
{"".join(body)}
</svg>
'''


def main() -> None:
    (ROOT / "robot-joint-nonlinear-evidence.svg").write_text(render(), encoding="utf-8")


if __name__ == "__main__":
    main()
