#!/usr/bin/env python3
"""Render Lesson 78 from its retained native servo capstone trace.

Reads the checked-in trace CSV (long "panel,series,point,x,y,x_unit,y_unit"
form) and writes servo-capstone-evidence.svg. Pure Python, no third-party
dependencies, so any learner can regenerate the figure after the underlying
evidence changes.
"""

from __future__ import annotations

import csv
from collections import defaultdict
from pathlib import Path

ROOT = Path(__file__).resolve().parent
TRACE = ROOT / "servo-capstone-evidence-trace.csv"
OUTPUT = ROOT / "servo-capstone-evidence.svg"

INK = "#172033"
MUTED = "#526078"
GRID = "#cbd5e1"
AXIS = "#8792a5"
PAPER = "#f8fafc"

SERIES_STYLE = {
    "Setpoint": ("#172033", "6 4"),
    "Filtered PID": ("#1268b3", ""),
    "Delayed PID": ("#b4232f", ""),
}
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


def render_line_panel(series_map, axis_title, top, bottom, left, right, stride) -> list[str]:
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
    body.append(text((left + right) / 2, bottom + 34, TIME_TITLE, "label", "middle"))
    for name, points in series_map.items():
        colour, dash = SERIES_STYLE.get(name, ("#526078", ""))
        body.append(polyline([(sx(x), sy(y)) for x, y in points[::stride]], colour, dash))
    return body


def render_bar_panel(points, axis_title, top, bottom, left, right) -> list[str]:
    labels = ["P", "PD", "Filtered PID", "Delayed PID"]
    colours = ["#7a5300", "#b4232f", "#1268b3", "#16803c"]
    y_max = max(y for _, y in points) * 1.2
    n = len(points)
    slot = (right - left) / n
    sy = lambda v: bottom - v / y_max * (bottom - top)
    body = [text(left, top - 12, axis_title, "label")]
    for value in nice_ticks(0, y_max):
        body.append(f'<line x1="{left:.1f}" y1="{sy(value):.1f}" x2="{right:.1f}" y2="{sy(value):.1f}" class="grid" />')
        body.append(text(left - 8, sy(value) + 4, f"{value:.3f}", "note", "end"))
    body.append(f'<line x1="{left:.1f}" y1="{top}" x2="{left:.1f}" y2="{bottom}" class="axis" />')
    body.append(f'<line x1="{left:.1f}" y1="{bottom}" x2="{right:.1f}" y2="{bottom}" class="axis" />')
    body.append(text((left + right) / 2, bottom + 34, "Structure", "label", "middle"))
    for i, (x, y) in enumerate(points):
        cx = left + slot * (i + 0.5)
        colour = colours[i % len(colours)]
        body.append(f'<rect x="{cx - slot * 0.28:.1f}" y="{sy(y):.1f}" width="{slot * 0.56:.1f}" '
                     f'height="{sy(0) - sy(y):.1f}" fill="{colour}" />')
        body.append(text(cx, sy(y) - 8, f"{y:.4f}", "note", "middle"))
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
    panel_height = 220
    bar_panel_height = 210
    gap = 70
    left, right = 92.0, 760.0
    legend_x = right + 16

    svg_parts: list[str] = []
    y_cursor = top_margin

    for panel, axis_title, stride in [
        ("Indexing-table position", "Payload position (rad)", 8),
        ("Applied motor torque", "Applied motor torque (N*m)", 8),
    ]:
        series_map = data[panel]
        top, bottom = y_cursor, y_cursor + panel_height
        svg_parts.append(text(left, top - 34, panel, "title"))
        svg_parts += render_line_panel(series_map, axis_title, top, bottom, left, right, stride)
        entries = [(name, *SERIES_STYLE.get(name, ("#526078", ""))) for name in series_map]
        svg_parts += legend(entries, legend_x, top + 10)
        y_cursor = bottom + gap

    top, bottom = y_cursor, y_cursor + bar_panel_height
    svg_parts.append(text(left, top - 34, "Architecture comparison", "title"))
    points = data["Architecture comparison"]["Final load error"]
    svg_parts += render_bar_panel(points, "Final load error (rad)", top, bottom, left, left + 420)
    y_cursor = bottom + gap

    total_height = y_cursor + 20
    total_width = legend_x + 260
    header = f'''<svg xmlns="http://www.w3.org/2000/svg" width="{total_width:.0f}" height="{total_height:.0f}" viewBox="0 0 {total_width:.0f} {total_height:.0f}" role="img">
<title>Indexing-table position and applied motor torque for the selected and delayed PID designs</title>
<desc>Simulation evidence: setpoint and payload position for the filtered PID and unchanged delayed PID candidates, their applied motor torque on the same time axis, and the retained final-window load-error comparison across P, PD, filtered PID, and delayed PID structures.</desc>
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
