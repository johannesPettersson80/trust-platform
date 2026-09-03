#!/usr/bin/env python3
"""Render Lesson 42 from its retained native manual/historical tuning trace."""

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

# case -> (label, color, dashed)
SERIES = [
    ("manual_p", "Manual P", "#0072b2", False),
    ("manual_pi", "Manual PI", "#009e73", False),
    ("manual_pi_rejected", "Rejected Ki=0.05", "#cc79a7", True),
    ("critical_p", "Critical P", "#d55e00", True),
    ("historical_pi", "Historical PI", "#e69f00", False),
]
MARKERS = [(10.0, "setpoint step"), (100.0, "load disturbance"), (180.0, "restoration")]


def text(x: float, y: float, value: str, size: int = 12, weight: str = "400", anchor: str = "start") -> str:
    return (f'<text x="{x:.1f}" y="{y:.1f}" font-size="{size}" font-weight="{weight}" '
            f'text-anchor="{anchor}">{value}</text>')


def polyline(points, colour: str, dashed: bool) -> str:
    encoded = " ".join(f"{x:.1f},{y:.1f}" for x, y in points)
    dash = ' stroke-dasharray="7 5"' if dashed else ""
    return f'<polyline points="{encoded}" fill="none" stroke="{colour}" stroke-width="2.2"{dash} />'


def panel(rows_by_case, value_key, minimum, maximum, top, bottom, left, right,
          t_max, gridlines, unit_label) -> list[str]:
    body: list[str] = [text(left, top - 12, unit_label, 15, "700")]
    sy = lambda value: bottom - (value - minimum) / (maximum - minimum) * (bottom - top)
    for value in gridlines:
        body.append(f'<line class="grid" x1="{left:.1f}" y1="{sy(value):.2f}" x2="{right:.1f}" y2="{sy(value):.2f}" />')
        body.append(text(left - 8, sy(value) + 4, f"{value:.2f}", 11, anchor="end"))
    sx = lambda t: left + t / t_max * (right - left)
    for at_seconds, label in MARKERS:
        x = sx(at_seconds)
        body.append(f'<line x1="{x:.1f}" y1="{top}" x2="{x:.1f}" y2="{bottom}" stroke="{MARKER}" '
                     f'stroke-width="1.4" stroke-dasharray="4 4" />')
    body.append(f'<line x1="{left:.1f}" y1="{top}" x2="{left:.1f}" y2="{bottom}" stroke="{AXIS}" stroke-width="1" />')
    body.append(f'<line x1="{left:.1f}" y1="{bottom}" x2="{right:.1f}" y2="{bottom}" stroke="{AXIS}" stroke-width="1" />')
    for case, _label, colour, dashed in SERIES:
        points = [(sx(float(r["time_s"])), sy(float(r[value_key]))) for r in rows_by_case[case][::5]]
        body.append(polyline(points, colour, dashed))
    return body


def render() -> str:
    rows_by_case: dict[str, list[dict]] = defaultdict(list)
    with (ROOT / "manual-historical-tuning-comparison-trace.csv").open(newline="", encoding="utf-8") as handle:
        for row in csv.DictReader(handle):
            rows_by_case[row["case"]].append(row)
    t_max = max(float(r["time_s"]) for rows in rows_by_case.values() for r in rows)

    legend: list[str] = []
    x = 55.0
    for _case, label, colour, dashed in SERIES:
        dash = ' stroke-dasharray="7 5"' if dashed else ""
        legend.append(f'<line x1="{x:.1f}" y1="96" x2="{x + 28:.1f}" y2="96" stroke="{colour}" stroke-width="3"{dash}/>')
        legend.append(text(x + 34, 101, label, 12))
        x += 34 + 8 * len(label) + 34

    top_panel = panel(rows_by_case, "pv_degC", 48.0, 56.0, 155.0, 315.0, 70.0, 860.0, t_max,
                       [56, 54, 52, 50, 48], "Outlet temperature (degC)")
    bottom_panel = panel(rows_by_case, "applied_mv_percent", 0.0, 100.0, 390.0, 550.0, 70.0, 860.0, t_max,
                          [100, 75, 50, 25, 0], "Applied valve position (%)")
    for value in range(0, int(t_max) + 1, 30):
        x = 70.0 + value / t_max * 790.0
        top_panel.append(text(x, 570.0, f"{value}", 11, anchor="middle"))
    top_panel.append(text(465.0, 592.0, "Time (s)", 13, "700", "middle"))

    body = "".join(legend) + "".join(top_panel) + "".join(bottom_panel)
    return f'''<svg xmlns="http://www.w3.org/2000/svg" role="img" aria-labelledby="title desc" viewBox="0 0 900 610">
<title id="title">Manual and historical heat-exchanger tuning</title>
<desc id="desc">Same plant and events; the rejected manual trial and critical experiment remain visible</desc>
<rect width="900" height="610" fill="{PAPER}"/>
<style>text{{font-family:system-ui,-apple-system,sans-serif;fill:{INK}}} .grid{{stroke:{GRID};stroke-width:1}}</style>
<text x="55" y="42" font-size="24" font-weight="700">Manual and historical heat-exchanger tuning</text>
<text x="55" y="68" font-size="14">Same plant and events; the rejected manual trial and critical experiment remain visible</text>
{body}
</svg>
'''


def main() -> None:
    (ROOT / "manual-historical-tuning-comparison.svg").write_text(render(), encoding="utf-8")


if __name__ == "__main__":
    main()
