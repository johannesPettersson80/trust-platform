#!/usr/bin/env python3
"""Render Lesson 44 from its retained native SIMC response-target trace."""

from __future__ import annotations

import csv
from collections import defaultdict
from pathlib import Path

ROOT = Path(__file__).resolve().parent
INK = "#243044"
GRID = "#d8e0eb"
AXIS = "#526078"
PAPER = "#f7f9fc"
MARKER = "#b43c32"

# (plant, candidate) -> (label, color, dashed)
SERIES = [
    (("water_heater", "Fast"), "Heater Fast", "#0072b2", False),
    (("water_heater", "Balanced"), "Heater Balanced", "#0072b2", True),
    (("water_heater", "Robust"), "Heater Robust", "#56b4e9", True),
    (("buffer_tank", "Fast"), "Tank Fast", "#009e73", False),
    (("buffer_tank", "Balanced"), "Tank Balanced", "#009e73", True),
    (("buffer_tank", "Robust"), "Tank Robust", "#66c2a5", True),
    (("belt_dryer", "Fast"), "Dryer Fast", "#d55e00", False),
    (("belt_dryer", "Balanced"), "Dryer Balanced", "#d55e00", True),
    (("belt_dryer", "Robust"), "Dryer Robust", "#e69f00", True),
]


def text(x: float, y: float, value: str, size: int = 12, weight: str = "400", anchor: str = "start") -> str:
    return (f'<text x="{x:.1f}" y="{y:.1f}" font-size="{size}" font-weight="{weight}" '
            f'text-anchor="{anchor}">{value}</text>')


def polyline(points, colour: str, dashed: bool) -> str:
    encoded = " ".join(f"{x:.1f},{y:.1f}" for x, y in points)
    dash = ' stroke-dasharray="6 4"' if dashed else ""
    return f'<polyline points="{encoded}" fill="none" stroke="{colour}" stroke-width="1.8"{dash} />'


def panel(rows_by_key, value_key, minimum, maximum, top, bottom, left, right,
          t_max, gridlines, unit_label) -> list[str]:
    body: list[str] = [text(left, top - 12, unit_label, 15, "700")]
    sy = lambda value: bottom - (value - minimum) / (maximum - minimum) * (bottom - top)
    for value in gridlines:
        body.append(f'<line class="grid" x1="{left:.1f}" y1="{sy(value):.2f}" x2="{right:.1f}" y2="{sy(value):.2f}" />')
        body.append(text(left - 8, sy(value) + 4, f"{value:.2f}", 11, anchor="end"))
    sx = lambda t: left + t / t_max * (right - left)
    x = sx(10.0)
    body.append(f'<line x1="{x:.1f}" y1="{top}" x2="{x:.1f}" y2="{bottom}" stroke="{MARKER}" '
                 f'stroke-width="1.4" stroke-dasharray="4 4" />')
    body.append(f'<line x1="{left:.1f}" y1="{top}" x2="{left:.1f}" y2="{bottom}" stroke="{AXIS}" stroke-width="1" />')
    body.append(f'<line x1="{left:.1f}" y1="{bottom}" x2="{right:.1f}" y2="{bottom}" stroke="{AXIS}" stroke-width="1" />')
    for key, _label, colour, dashed in SERIES:
        points = [(sx(float(r["time_s"])), sy(float(r[value_key]))) for r in rows_by_key[key][::4]]
        body.append(polyline(points, colour, dashed))
    return body


def render() -> str:
    rows_by_key: dict[tuple[str, str], list[dict]] = defaultdict(list)
    with (ROOT / "model-based-tuning-comparison-trace.csv").open(newline="", encoding="utf-8") as handle:
        for row in csv.DictReader(handle):
            rows_by_key[(row["plant"], row["candidate"])].append(row)
    t_max = max(float(r["time_s"]) for rows in rows_by_key.values() for r in rows)

    legend: list[str] = []
    x, y = 55.0, 90.0
    for i, (_key, label, colour, dashed) in enumerate(SERIES):
        if i == 5:
            x, y = 55.0, 108.0
        dash = ' stroke-dasharray="6 4"' if dashed else ""
        legend.append(f'<line x1="{x:.1f}" y1="{y}" x2="{x + 24:.1f}" y2="{y}" stroke="{colour}" stroke-width="3"{dash}/>')
        legend.append(text(x + 30, y + 5, label, 11))
        x += 30 + 7 * len(label) + 20

    top_panel = panel(rows_by_key, "normalized_pv", 0.0, 1.25, 155.0, 315.0, 70.0, 860.0, t_max,
                       [1.25, 1.0, 0.75, 0.5, 0.25, 0.0], "Normalized process response (fraction of setpoint change)")
    bottom_panel = panel(rows_by_key, "applied_mv_percent", 0.0, 100.0, 390.0, 550.0, 70.0, 860.0, t_max,
                          [100, 75, 50, 25, 0], "Applied actuator output (%)")
    for value in range(0, int(t_max) + 1, 30):
        x = 70.0 + value / t_max * 790.0
        top_panel.append(text(x, 570.0, f"{value}", 11, anchor="middle"))
    top_panel.append(text(465.0, 592.0, "Time (s)", 13, "700", "middle"))

    body = "".join(legend) + "".join(top_panel) + "".join(bottom_panel)
    return f'''<svg xmlns="http://www.w3.org/2000/svg" role="img" aria-labelledby="title desc" viewBox="0 0 900 610">
<title id="title">SIMC response targets on three process types</title>
<desc id="desc">The same color and dash key applies to normalized PV and applied MV</desc>
<rect width="900" height="610" fill="{PAPER}"/>
<style>text{{font-family:system-ui,-apple-system,sans-serif;fill:{INK}}} .grid{{stroke:{GRID};stroke-width:1}}</style>
<text x="55" y="42" font-size="24" font-weight="700">SIMC response targets on three process types</text>
<text x="55" y="65" font-size="14">The same color and dash key applies to normalized PV and applied MV</text>
{body}
</svg>
'''


def main() -> None:
    (ROOT / "model-based-tuning-comparison.svg").write_text(render(), encoding="utf-8")


if __name__ == "__main__":
    main()
