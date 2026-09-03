#!/usr/bin/env python3
"""Render Lesson 76 from its retained native cold-storage capstone trace.

Reads the checked-in trace and summary CSVs (long "panel,series,point,x,y,
x_unit,y_unit" form) and writes cold-storage-capstone-evidence.svg. Pure
Python, no third-party dependencies, so any learner can regenerate the figure
after the underlying evidence changes.
"""

from __future__ import annotations

import csv
from collections import defaultdict
from pathlib import Path

ROOT = Path(__file__).resolve().parent
TRACE = ROOT / "cold-storage-capstone-evidence-trace.csv"
OUTPUT = ROOT / "cold-storage-capstone-evidence.svg"

INK = "#172033"
MUTED = "#526078"
GRID = "#cbd5e1"
AXIS = "#8792a5"
PAPER = "#f8fafc"

# Explicit colour/dash per series so every legend entry is unambiguous and no
# axis title is ever built by blindly wrapping an already-worded unit.
SERIES_STYLE = {
    "SP": ("#172033", "6 4"),
    "PV": ("#1268b3", ""),
    "Upper limit": ("#b4232f", "3 3"),
    "Lower limit": ("#b4232f", "3 3"),
    "On 2.5": ("#7a5300", "2 4"),
    "Off 1.5": ("#7a5300", "2 4"),
    "Requested command": ("#b4232f", "6 4"),
    "Applied command": ("#16803c", ""),
    "Quiet-window transitions": ("#1268b3", ""),
}

# panel -> (axis title, subsample stride, kind)
PANELS = [
    ("Selected cold-room temperature", "Cold-room air and setpoint temperature (degC)", 4, "line"),
    ("Requested (red dashed) and applied (green) compressor command", "Compressor command (%)", 4, "line"),
    ("Candidate comparison", "Quiet-window applied-state transitions (count)", 1, "bar"),
]
TIME_TITLE = "Time after start (s)"


def read_trace(path: Path):
    data: dict[str, dict[str, list[tuple[float, float]]]] = defaultdict(lambda: defaultdict(list))
    with path.open(newline="", encoding="utf-8") as handle:
        for row in csv.DictReader(handle):
            data[row["panel"]][row["series"]].append((float(row["x"]), float(row["y"])))
    for panel in data:
        for series in data[panel]:
            data[panel][series].sort(key=lambda point: point[0])
    return data


def text(x: float, y: float, value: str, css: str = "note", anchor: str = "start") -> str:
    return f'<text x="{x:.1f}" y="{y:.1f}" class="{css}" text-anchor="{anchor}">{value}</text>'


def polyline(points: list[tuple[float, float]], colour: str, dash: str) -> str:
    encoded = " ".join(f"{x:.1f},{y:.1f}" for x, y in points)
    dashed = f' stroke-dasharray="{dash}"' if dash else ""
    return f'<polyline points="{encoded}" fill="none" stroke="{colour}" stroke-width="2.2"{dashed} />'


def nice_ticks(minimum: float, maximum: float, count: int = 5) -> list[float]:
    if maximum <= minimum:
        maximum = minimum + 1.0
    step = (maximum - minimum) / (count - 1)
    return [minimum + step * i for i in range(count)]


def render_line_panel(series_map, x_unit_title, axis_title, top, bottom, left, right, stride) -> list[str]:
    all_x = [x for series in series_map.values() for x, _ in series]
    all_y = [y for series in series_map.values() for _, y in series]
    x_min, x_max = min(all_x), max(all_x)
    y_min, y_max = min(all_y), max(all_y)
    pad = max((y_max - y_min) * 0.08, 0.05)
    y_min, y_max = y_min - pad, y_max + pad
    sx = lambda v: left + (v - x_min) / (x_max - x_min) * (right - left)
    sy = lambda v: bottom - (v - y_min) / (y_max - y_min) * (bottom - top)

    body = [text(left, top - 12, axis_title, "label")]
    for value in nice_ticks(y_min, y_max):
        body.append(f'<line x1="{left:.1f}" y1="{sy(value):.1f}" x2="{right:.1f}" y2="{sy(value):.1f}" class="grid" />')
        body.append(text(left - 8, sy(value) + 4, f"{value:.2f}", "note", "end"))
    for value in nice_ticks(x_min, x_max):
        body.append(f'<line x1="{sx(value):.1f}" y1="{top}" x2="{sx(value):.1f}" y2="{bottom}" class="grid" />')
        body.append(text(sx(value), bottom + 18, f"{value:.0f}", "note", "middle"))
    body.append(f'<line x1="{left:.1f}" y1="{top}" x2="{left:.1f}" y2="{bottom}" class="axis" />')
    body.append(f'<line x1="{left:.1f}" y1="{bottom}" x2="{right:.1f}" y2="{bottom}" class="axis" />')
    body.append(text((left + right) / 2, bottom + 34, x_unit_title, "label", "middle"))
    for name, points in series_map.items():
        colour, dash = SERIES_STYLE.get(name, ("#526078", ""))
        body.append(polyline([(sx(x), sy(y)) for x, y in points[::stride]], colour, dash))
    return body


def render_bar_panel(series_map, x_unit_title, axis_title, top, bottom, left, right) -> list[str]:
    (only_name, points), = series_map.items()
    y_max = max(y for _, y in points) * 1.15
    n = len(points)
    slot = (right - left) / n
    sy = lambda v: bottom - v / y_max * (bottom - top)
    colour, _ = SERIES_STYLE.get(only_name, ("#1268b3", ""))
    body = [text(left, top - 12, axis_title, "label")]
    for value in nice_ticks(0, y_max):
        body.append(f'<line x1="{left:.1f}" y1="{sy(value):.1f}" x2="{right:.1f}" y2="{sy(value):.1f}" class="grid" />')
        body.append(text(left - 8, sy(value) + 4, f"{value:.0f}", "note", "end"))
    body.append(f'<line x1="{left:.1f}" y1="{top}" x2="{left:.1f}" y2="{bottom}" class="axis" />')
    body.append(f'<line x1="{left:.1f}" y1="{bottom}" x2="{right:.1f}" y2="{bottom}" class="axis" />')
    body.append(text((left + right) / 2, bottom + 34, x_unit_title, "label", "middle"))
    labels = ["Single threshold", "Hysteresis only", "Hysteresis plus dwell"]
    for i, (x, y) in enumerate(points):
        cx = left + slot * (i + 0.5)
        body.append(f'<rect x="{cx - slot * 0.28:.1f}" y="{sy(y):.1f}" width="{slot * 0.56:.1f}" '
                     f'height="{sy(0) - sy(y):.1f}" fill="{colour}" />')
        body.append(text(cx, sy(y) - 8, f"{y:.0f}", "note", "middle"))
        label = labels[i] if i < len(labels) else str(int(x))
        body.append(text(cx, bottom + 18, label, "note", "middle"))
    return body


def legend(entries: list[tuple[str, str, str]], x: float, y: float) -> list[str]:
    body = []
    for i, (name, colour, dash) in enumerate(entries):
        row_y = y + i * 18
        dashed = f' stroke-dasharray="{dash}"' if dash else ""
        body.append(f'<line x1="{x:.1f}" y1="{row_y:.1f}" x2="{x + 26:.1f}" y2="{row_y:.1f}" '
                     f'stroke="{colour}" stroke-width="2.6"{dashed} />')
        body.append(text(x + 32, row_y + 4, name))
    return body


def render() -> str:
    data = read_trace(TRACE)
    top_margin = 60
    panel_height = 230
    gap = 70
    left, right = 92.0, 760.0
    legend_x = right + 16

    svg_parts: list[str] = []
    y_cursor = top_margin
    for panel, axis_title, stride, kind in PANELS:
        series_map = data[panel]
        top = y_cursor
        bottom = y_cursor + panel_height
        svg_parts.append(text(left, top - 34, panel, "title"))
        if kind == "line":
            svg_parts += render_line_panel(series_map, TIME_TITLE, axis_title, top, bottom, left, right, stride)
        else:
            svg_parts += render_bar_panel(series_map, "Candidate", axis_title, top, bottom, left, right)
        entries = [(name, *SERIES_STYLE.get(name, ("#526078", ""))) for name in series_map]
        svg_parts += legend(entries, legend_x, top + 10)
        y_cursor = bottom + gap

    total_height = y_cursor + 20
    total_width = legend_x + 220
    header = f'''<svg xmlns="http://www.w3.org/2000/svg" width="{total_width:.0f}" height="{total_height:.0f}" viewBox="0 0 {total_width:.0f} {total_height:.0f}" role="img">
<title>Cold-room temperature and applied compressor command for the selected hysteresis-plus-dwell controller</title>
<desc>Simulation evidence: qualified cold-room air temperature against the product band and hysteresis thresholds, requested and applied compressor command, and the retained candidate comparison of quiet-window applied-state transitions for the single-threshold, hysteresis-only, and hysteresis-plus-dwell candidates.</desc>
<style>
  text {{ font-family: Arial, sans-serif; font-size: 12px; fill: {INK}; }}
  .title {{ font-size: 16px; font-weight: 700; }}
  .label {{ font-size: 12px; font-weight: 700; }}
  .note {{ fill: {MUTED}; font-size: 11px; }}
  .grid {{ stroke: {GRID}; stroke-width: 1; stroke-dasharray: 3 4; }}
  .axis {{ stroke: {AXIS}; stroke-width: 1.3; }}
</style>
<rect width="{total_width:.0f}" height="{total_height:.0f}" fill="{PAPER}" />
'''
    return header + "".join(svg_parts) + "\n</svg>\n"


def main() -> None:
    OUTPUT.write_text(render(), encoding="utf-8")


if __name__ == "__main__":
    main()
