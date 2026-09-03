#!/usr/bin/env python3
"""Render Lesson 69 from its retained native Lyapunov-certificate trace.

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
GREEN = "#16a34a"
PAPER = "#f7f9fc"

LEFT, RIGHT = 75.0, 855.0


def read_trace() -> dict[str, dict[str, list[tuple[float, float]]]]:
    """Group the shipped trace CSV as panel -> series -> [(point, y), ...]."""
    panels: dict[str, dict[str, list[tuple[float, float]]]] = {}
    with (ROOT / "pendulum-lyapunov-evidence-trace.csv").open(newline="", encoding="utf-8") as handle:
        for row in csv.DictReader(handle):
            series = panels.setdefault(row["panel"], {})
            series.setdefault(row["series"], []).append(
                (float(row["point"]), float(row["y"]))
            )
    return panels


def axis_block(top: float, bottom: float, y_max: float, title: str, case_count: int) -> list[str]:
    """Grid, gridline labels, integer case ticks, and the panel title."""
    sy = lambda value: bottom - (value / y_max) * (bottom - top)
    sx = lambda point: LEFT + (point - 1.0) / (case_count - 1) * (RIGHT - LEFT)
    body = [f'<text x="{LEFT:.0f}" y="{top - 12:.0f}" font-size="16" font-weight="700">{title}</text>']
    for step in range(5):
        value = y_max * (4 - step) / 4
        y = sy(value)
        body.append(f'<line class="grid" x1="{LEFT:.0f}" y1="{y:.2f}" x2="{RIGHT:.0f}" y2="{y:.2f}" />')
        body.append(f'<text x="{LEFT - 8:.0f}" y="{y + 4:.2f}" text-anchor="end" font-size="11">{value:.2f}</text>')
    for point in range(1, case_count + 1):
        x = sx(point)
        body.append(f'<line class="grid" x1="{x:.2f}" y1="{top:.0f}" x2="{x:.2f}" y2="{bottom:.0f}" />')
        body.append(f'<text x="{x:.2f}" y="{bottom + 18:.0f}" text-anchor="middle" font-size="11">{point}</text>')
    body.append(f'<rect class="axis" x="{LEFT:.0f}" y="{top:.0f}" width="{RIGHT - LEFT:.0f}" height="{bottom - top:.0f}" fill="none" />')
    body.append(f'<text x="{(LEFT + RIGHT) / 2:.0f}" y="{bottom + 36:.0f}" text-anchor="middle" font-size="12">case (1-8 certified, 9-11 observational; see Evidence table)</text>')
    return body


def series_polyline(points: list[tuple[float, float]], top: float, bottom: float, y_max: float, case_count: int, colour: str, dash: str = "") -> str:
    sy = lambda value: bottom - (value / y_max) * (bottom - top)
    sx = lambda point: LEFT + (point - 1.0) / (case_count - 1) * (RIGHT - LEFT)
    ordered = sorted(points, key=lambda item: item[0])
    encoded = " ".join(f"{sx(point):.2f},{sy(value):.2f}" for point, value in ordered)
    dashed = f' stroke-dasharray="{dash}"' if dash else ""
    return f'<polyline fill="none" stroke="{colour}" stroke-width="2.2"{dashed} points="{encoded}" />'


def render() -> str:
    panels = read_trace()
    v_panel = panels["Initial Lyapunov value"]
    t_panel = panels["Peak applied torque"]
    case_count = len(v_panel["Initial V"])

    v_max = max(y for _, y in v_panel["Initial V"]) * 1.1
    t_max = max(y for _, y in t_panel["Applied torque"]) * 1.1

    v_top, v_bottom = 157.0, 317.0
    t_top, t_bottom = 407.0, 567.0

    body: list[str] = []
    body += axis_block(v_top, v_bottom, v_max, "Initial Lyapunov value (V)", case_count)
    body.append(series_polyline(v_panel["Initial V"], v_top, v_bottom, v_max, case_count, BLUE))
    body.append(series_polyline(v_panel["Certificate rho"], v_top, v_bottom, v_max, case_count, RED, dash="7 5"))
    body += axis_block(t_top, t_bottom, t_max, "Peak applied torque (N m)", case_count)
    body.append(series_polyline(t_panel["Applied torque"], t_top, t_bottom, t_max, case_count, GREEN))

    legend = (
        f'<line x1="55" y1="108" x2="83" y2="108" stroke="{BLUE}" stroke-width="3"/>'
        '<text x="89" y="113" font-size="12">Initial V</text>'
        f'<line x1="163" y1="108" x2="191" y2="108" stroke="{RED}" stroke-width="3" stroke-dasharray="7 5"/>'
        '<text x="197" y="113" font-size="12">Certificate rho</text>'
    )

    return f'''<svg xmlns="http://www.w3.org/2000/svg" role="img" aria-labelledby="title desc" viewBox="0 0 900 650">
<title id="title">Certified region is smaller than simulated recovery</title>
<desc id="desc">Cases 1-8 start inside the certificate; 9-11 are observations only. Case numbers match the shipped trace and summary CSVs and the Evidence table.</desc>
<defs><marker id="arrow" markerWidth="8" markerHeight="8" refX="7" refY="3" orient="auto"><path d="M0,0 L0,6 L8,3 z" fill="{INK}"/></marker></defs>
<rect width="900" height="650" fill="{PAPER}" />
<style>text{{font-family:system-ui,-apple-system,sans-serif;fill:{INK}}} .axis{{stroke:{AXIS};stroke-width:1}} .grid{{stroke:{GRID};stroke-width:1}}</style>
<text x="55" y="42" font-size="24" font-weight="700">Certified region is smaller than simulated recovery</text>
<text x="55" y="70" font-size="14">Cases 1-8 start inside the certificate; 9-11 are observations only.</text>
{legend}
{"".join(body)}
</svg>
'''


def main() -> None:
    (ROOT / "pendulum-lyapunov-evidence.svg").write_text(render(), encoding="utf-8")


if __name__ == "__main__":
    main()
