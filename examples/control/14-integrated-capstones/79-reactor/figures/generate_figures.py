#!/usr/bin/env python3
"""Render Lesson 79 from its retained native reactor capstone trace.

Reads the checked-in trace CSV (long "panel,series,point,x,y,x_unit,y_unit"
form) and writes reactor-capstone-evidence.svg. Pure Python, no third-party
dependencies, so any learner can regenerate the figure after the underlying
evidence changes.

Each (panel, y_unit) combination becomes its own small chart so that no axis
ever mixes two physically different units (a panel such as "Fault recovery"
carries seven different y_unit values across its series). Every axis title is
built by capitalising the CSV's own y_unit string exactly once -- never by
concatenating a panel name onto an already self-describing unit such as
"temperature (degC)" -- so no title can end up nested like
"Reactor temperature (temperature (degC))".
"""

from __future__ import annotations

import csv
from collections import OrderedDict, defaultdict
from pathlib import Path

ROOT = Path(__file__).resolve().parent
TRACE = ROOT / "reactor-capstone-evidence-trace.csv"
OUTPUT = ROOT / "reactor-capstone-evidence.svg"

INK = "#172033"
MUTED = "#526078"
GRID = "#cbd5e1"
AXIS = "#8792a5"
PAPER = "#f8fafc"
PALETTE = ["#1268b3", "#b4232f", "#16803c", "#7a5300", "#7c3aed", "#0e7490", "#be185d"]


def axis_title(unit: str) -> str:
    """Capitalise the CSV's own unit string once; never re-wrap it."""
    return unit[:1].upper() + unit[1:]


def read_trace(path: Path):
    # sub_charts: ordered dict keyed by (panel, y_unit) -> ordered dict of
    # series name -> list[(x, y)]; x_units tracked per sub-chart.
    sub_charts: "OrderedDict[tuple[str, str], OrderedDict[str, list[tuple[float, float]]]]" = OrderedDict()
    x_units: dict[tuple[str, str], str] = {}
    with path.open(newline="", encoding="utf-8") as handle:
        for row in csv.DictReader(handle):
            key = (row["panel"], row["y_unit"])
            sub_charts.setdefault(key, OrderedDict())
            sub_charts[key].setdefault(row["series"], []).append((float(row["x"]), float(row["y"])))
            x_units[key] = row["x_unit"]
    for key in sub_charts:
        for series in sub_charts[key]:
            sub_charts[key][series].sort(key=lambda point: point[0])
    return sub_charts, x_units


def text(x: float, y: float, value: str, css: str = "note", anchor: str = "start") -> str:
    return f'<text x="{x:.1f}" y="{y:.1f}" class="{css}" text-anchor="{anchor}">{value}</text>'


def polyline(points: list[tuple[float, float]], colour: str) -> str:
    encoded = " ".join(f"{x:.1f},{y:.1f}" for x, y in points)
    return f'<polyline points="{encoded}" fill="none" stroke="{colour}" stroke-width="1.9" />'


def nice_ticks(minimum: float, maximum: float, count: int = 4) -> list[float]:
    if maximum <= minimum:
        maximum = minimum + 1.0
    step = (maximum - minimum) / (count - 1)
    return [minimum + step * i for i in range(count)]


def render_chart(series_map, x_title, y_title, top, bottom, left, right, stride) -> list[str]:
    all_x = [x for series in series_map.values() for x, _ in series]
    all_y = [y for series in series_map.values() for _, y in series]
    x_min, x_max = min(all_x), max(all_x)
    y_min, y_max = min(all_y), max(all_y)
    pad = max((y_max - y_min) * 0.10, 0.05)
    y_min, y_max = y_min - pad, y_max + pad
    sx = lambda v: left + (v - x_min) / (x_max - x_min) * (right - left)
    sy = lambda v: bottom - (v - y_min) / (y_max - y_min) * (bottom - top)

    body = [text(left, top - 8, y_title, "label")]
    for value in nice_ticks(y_min, y_max):
        body.append(f'<line x1="{left:.1f}" y1="{sy(value):.1f}" x2="{right:.1f}" y2="{sy(value):.1f}" class="grid" />')
        body.append(text(left - 6, sy(value) + 3, f"{value:.2f}", "tick", "end"))
    body.append(f'<line x1="{left:.1f}" y1="{top}" x2="{left:.1f}" y2="{bottom}" class="axis" />')
    body.append(f'<line x1="{left:.1f}" y1="{bottom}" x2="{right:.1f}" y2="{bottom}" class="axis" />')
    body.append(text((left + right) / 2, bottom + 22, x_title, "tick", "middle"))
    for i, (name, points) in enumerate(series_map.items()):
        colour = PALETTE[i % len(PALETTE)]
        body.append(polyline([(sx(x), sy(y)) for x, y in points[::stride]], colour))
    return body


def legend(entries: list[tuple[str, str]], x: float, y: float, max_width: float) -> tuple[list[str], float]:
    body = []
    row_y = y
    cursor_x = x
    line_height = 13
    for name, colour in entries:
        entry_width = 14 + 5.6 * len(name)
        if cursor_x + entry_width > x + max_width:
            cursor_x = x
            row_y += line_height
        body.append(f'<line x1="{cursor_x:.1f}" y1="{row_y - 3:.1f}" x2="{cursor_x + 12:.1f}" y2="{row_y - 3:.1f}" stroke="{colour}" stroke-width="2.4" />')
        body.append(text(cursor_x + 16, row_y, name, "tick"))
        cursor_x += entry_width
    return body, row_y + line_height


def render() -> str:
    sub_charts, x_units = read_trace(TRACE)

    columns = 3
    cell_width = 310.0
    cell_height = 200.0
    col_gap = 34.0
    row_gap = 46.0
    left_margin = 56.0
    top_margin = 56.0

    svg_parts: list[str] = []
    index = 0
    max_bottom = 0.0
    current_panel = None
    for (panel, y_unit), series_map in sub_charts.items():
        col = index % columns
        row = index // columns
        cell_left = left_margin + col * (cell_width + col_gap)
        cell_top = top_margin + row * (cell_height + row_gap)
        chart_top = cell_top + 34
        chart_bottom = cell_top + cell_height - 24
        chart_right = cell_left + cell_width - 20

        label = panel if panel != current_panel else f"{panel} (continued)"
        current_panel = panel
        svg_parts.append(text(cell_left, cell_top, label, "title"))
        svg_parts += render_chart(
            series_map, axis_title(x_units[(panel, y_unit)]), axis_title(y_unit),
            chart_top, chart_bottom, cell_left, chart_right, 6,
        )
        entries = [(name, PALETTE[i % len(PALETTE)]) for i, name in enumerate(series_map)]
        legend_parts, legend_bottom = legend(entries, cell_left, chart_bottom + 34, cell_width)
        svg_parts += legend_parts
        max_bottom = max(max_bottom, legend_bottom)
        index += 1

    total_rows = (index + columns - 1) // columns
    total_height = top_margin + total_rows * (cell_height + row_gap) + 10
    total_width = left_margin + columns * (cell_width + col_gap)

    header = f'''<svg xmlns="http://www.w3.org/2000/svg" width="{total_width:.0f}" height="{total_height:.0f}" viewBox="0 0 {total_width:.0f} {total_height:.0f}" role="img">
<title>Integrated jacketed-reactor journey</title>
<desc>Simulation evidence: reactor and jacket temperatures, applied and requested utilities, the feedforward path, the protection selector, authority and mode, controller revision, the abnormal-event timeline, the identification step response, and fault-recovery signals for the feedback-only and integrated reactor architectures.</desc>
<style>
  text {{ font-family: Arial, sans-serif; font-size: 11px; fill: {INK}; }}
  .title {{ font-size: 13px; font-weight: 700; }}
  .label {{ font-size: 11px; font-weight: 700; }}
  .tick {{ fill: {MUTED}; font-size: 10px; }}
  .grid {{ stroke: {GRID}; stroke-width: 1; stroke-dasharray: 2 3; }}
  .axis {{ stroke: {AXIS}; stroke-width: 1.1; }}
</style>
<rect width="{total_width:.0f}" height="{total_height:.0f}" fill="{PAPER}" />
'''
    return header + "".join(svg_parts) + "\n</svg>\n"


def main() -> None:
    OUTPUT.write_text(render(), encoding="utf-8")


if __name__ == "__main__":
    main()
