#!/usr/bin/env python3
"""Render Lesson 43 from its retained native feedback-relay trace."""

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

# case -> (label mv, label pv, color, dashed)
SERIES = [
    ("qualified", "Qualified", "#0072b2", False),
    ("low_amplitude", "Rejected", "#d55e00", True),
]


def text(x: float, y: float, value: str, size: int = 12, weight: str = "400", anchor: str = "start") -> str:
    return (f'<text x="{x:.1f}" y="{y:.1f}" font-size="{size}" font-weight="{weight}" '
            f'text-anchor="{anchor}">{value}</text>')


def polyline(points, colour: str, dashed: bool) -> str:
    encoded = " ".join(f"{x:.1f},{y:.1f}" for x, y in points)
    dash = ' stroke-dasharray="6 4" ' if dashed else ""
    return f'<polyline points="{encoded}" fill="none" stroke="{colour}" stroke-width="2.0"{dash} />'


def panel(rows_by_case, value_key, minimum, maximum, top, bottom, left, right,
          t_max, gridlines, unit_label, relay_start: float | None = None) -> list[str]:
    body: list[str] = [text(left, top - 12, unit_label, 15, "700")]
    sy = lambda value: bottom - (value - minimum) / (maximum - minimum) * (bottom - top)
    for value in gridlines:
        body.append(f'<line class="grid" x1="{left:.1f}" y1="{sy(value):.2f}" x2="{right:.1f}" y2="{sy(value):.2f}" />')
        body.append(text(left - 8, sy(value) + 4, f"{value:.2f}", 11, anchor="end"))
    sx = lambda t: left + t / t_max * (right - left)
    if relay_start is not None:
        x = sx(relay_start)
        body.append(f'<line x1="{x:.1f}" y1="{top}" x2="{x:.1f}" y2="{bottom}" stroke="{MARKER}" '
                     f'stroke-width="1.4" stroke-dasharray="4 4" />')
    body.append(f'<line x1="{left:.1f}" y1="{top}" x2="{left:.1f}" y2="{bottom}" stroke="{AXIS}" stroke-width="1" />')
    body.append(f'<line x1="{left:.1f}" y1="{bottom}" x2="{right:.1f}" y2="{bottom}" stroke="{AXIS}" stroke-width="1" />')
    for case, _label, colour, dashed in SERIES:
        points = [(sx(float(r["time_s"])), sy(float(r[value_key]))) for r in rows_by_case[case][::4]]
        body.append(polyline(points, colour, dashed))
    return body


def render() -> str:
    rows_by_case: dict[str, list[dict]] = defaultdict(list)
    with (ROOT / "relay-autotuning-comparison-trace.csv").open(newline="", encoding="utf-8") as handle:
        for row in csv.DictReader(handle):
            rows_by_case[row["case"]].append(row)
    t_max = max(float(r["time_s"]) for rows in rows_by_case.values() for r in rows)

    legend: list[str] = []
    x = 55.0
    for _case, label, colour, dashed in SERIES:
        dash = ' stroke-dasharray="6 4"' if dashed else ""
        legend.append(f'<line x1="{x:.1f}" y1="96" x2="{x + 28:.1f}" y2="96" stroke="{colour}" stroke-width="3"{dash}/>')
        legend.append(text(x + 34, 101, f"{label} PV", 12))
        x += 34 + 8 * len(label + " PV") + 34
    for _case, label, colour, dashed in SERIES:
        dash = ' stroke-dasharray="6 4"' if dashed else ""
        legend.append(f'<line x1="{x:.1f}" y1="96" x2="{x + 28:.1f}" y2="96" stroke="{colour}" stroke-width="3"{dash}/>')
        legend.append(text(x + 34, 101, f"{label} MV", 12))
        x += 34 + 8 * len(label + " MV") + 34

    top_panel = panel(rows_by_case, "pv_degC", 48.0, 52.0, 155.0, 315.0, 70.0, 860.0, t_max,
                       [52, 51, 50, 49, 48], "Outlet temperature (degC)", relay_start=10.0)
    bottom_panel = panel(rows_by_case, "applied_mv_percent", 0.0, 40.0, 390.0, 550.0, 70.0, 860.0, t_max,
                          [40, 30, 20, 10, 0], "Applied valve position (%)", relay_start=10.0)
    for value in range(0, int(t_max) + 1, 30):
        x = 70.0 + value / t_max * 790.0
        top_panel.append(text(x, 570.0, f"{value}", 11, anchor="middle"))
    top_panel.append(text(465.0, 592.0, "Time (s)", 13, "700", "middle"))

    body = "".join(legend) + "".join(top_panel) + "".join(bottom_panel)
    return f'''<svg xmlns="http://www.w3.org/2000/svg" role="img" aria-labelledby="title desc" viewBox="0 0 900 610">
<title id="title">Qualified and rejected feedback-relay tests</title>
<desc id="desc">Feedback thresholds drive switching; the low-amplitude record remains rejected</desc>
<rect width="900" height="610" fill="{PAPER}"/>
<style>text{{font-family:system-ui,-apple-system,sans-serif;fill:{INK}}} .grid{{stroke:{GRID};stroke-width:1}}</style>
<text x="55" y="42" font-size="24" font-weight="700">Qualified and rejected feedback-relay tests</text>
<text x="55" y="68" font-size="14">Feedback thresholds drive switching; the low-amplitude record remains rejected</text>
{body}
</svg>
'''


def main() -> None:
    (ROOT / "relay-autotuning-comparison.svg").write_text(render(), encoding="utf-8")


if __name__ == "__main__":
    main()
