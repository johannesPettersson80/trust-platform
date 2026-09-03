#!/usr/bin/env python3
"""Render Lesson 80 from its retained native gravity-tank capstone trace.

Reads the checked-in trace CSV (long "panel,series,point,x,y,x_unit,y_unit"
form) and writes gravity-tank-capstone-evidence.svg. Pure Python, no
third-party dependencies, so any learner can regenerate the figure after the
underlying evidence changes.
"""

from __future__ import annotations

import csv
from collections import OrderedDict
from pathlib import Path

ROOT = Path(__file__).resolve().parent
TRACE = ROOT / "gravity-tank-capstone-evidence-trace.csv"
OUTPUT = ROOT / "gravity-tank-capstone-evidence.svg"

INK = "#172033"
MUTED = "#526078"
GRID = "#cbd5e1"
AXIS = "#8792a5"
PAPER = "#f8fafc"

COLOURS = {"Fixed": "#7a5300", "Scheduled": "#1268b3",
           "Worst regional IAE": "#7a5300", "Maximum departure": "#7a5300"}


def read_trace(path: Path):
    data: "OrderedDict[str, OrderedDict[str, list[tuple[float, float]]]]" = OrderedDict()
    y_units: dict[str, str] = {}
    with path.open(newline="", encoding="utf-8") as handle:
        for row in csv.DictReader(handle):
            data.setdefault(row["panel"], OrderedDict())
            data[row["panel"]].setdefault(row["series"], []).append((float(row["x"]), float(row["y"])))
            y_units[row["panel"]] = row["y_unit"]
    for panel in data:
        for series in data[panel]:
            data[panel][series].sort(key=lambda point: point[0])
    return data, y_units


def text(x: float, y: float, value: str, css: str = "note", anchor: str = "start") -> str:
    return f'<text x="{x:.1f}" y="{y:.1f}" class="{css}" text-anchor="{anchor}">{value}</text>'


def nice_ticks(minimum: float, maximum: float, count: int = 5) -> list[float]:
    if maximum <= minimum:
        maximum = minimum + 1.0
    step = (maximum - minimum) / (count - 1)
    return [minimum + step * i for i in range(count)]


def render_grouped_bars(series_map, category_labels, axis_title, top, bottom, left, right) -> list[str]:
    n_categories = len(next(iter(series_map.values())))
    n_series = len(series_map)
    y_max = max(y for points in series_map.values() for _, y in points) * 1.2
    slot = (right - left) / n_categories
    sy = lambda v: bottom - v / y_max * (bottom - top)
    body = [text(left, top - 12, axis_title, "label")]
    for value in nice_ticks(0, y_max):
        body.append(f'<line x1="{left:.1f}" y1="{sy(value):.1f}" x2="{right:.1f}" y2="{sy(value):.1f}" class="grid" />')
        body.append(text(left - 8, sy(value) + 4, f"{value:.2f}", "note", "end"))
    body.append(f'<line x1="{left:.1f}" y1="{top}" x2="{left:.1f}" y2="{bottom}" class="axis" />')
    body.append(f'<line x1="{left:.1f}" y1="{bottom}" x2="{right:.1f}" y2="{bottom}" class="axis" />')
    bar_width = slot * 0.7 / n_series
    for cat in range(n_categories):
        cx = left + slot * cat
        body.append(text(cx + slot / 2, bottom + 18, category_labels[cat], "note", "middle"))
        for s_index, (name, points) in enumerate(series_map.items()):
            _, y = points[cat]
            bx = cx + slot * 0.15 + s_index * bar_width
            # With one series, colour by category (Fixed/Scheduled) instead of
            # by the single series name, so the two bars stay distinguishable.
            colour = COLOURS.get(category_labels[cat], "#1268b3") if n_series == 1 else COLOURS.get(name, "#1268b3")
            body.append(f'<rect x="{bx:.1f}" y="{sy(y):.1f}" width="{bar_width * 0.9:.1f}" '
                         f'height="{sy(0) - sy(y):.1f}" fill="{colour}" />')
            body.append(text(bx + bar_width * 0.45, sy(y) - 6, f"{y:.3f}", "note", "middle"))
    return body


def legend(entries: list[tuple[str, str]], x: float, y: float) -> list[str]:
    body = []
    for i, (name, colour) in enumerate(entries):
        row_y = y + i * 18
        body.append(f'<rect x="{x:.1f}" y="{row_y - 9:.1f}" width="14" height="10" fill="{colour}" />')
        body.append(text(x + 20, row_y, name))
    return body


def render() -> str:
    data, y_units = read_trace(TRACE)
    left, right = 92.0, 520.0
    legend_x = right + 24
    top_margin = 60
    panel_height = 210
    gap = 60

    svg_parts: list[str] = []
    y_cursor = top_margin

    for panel, axis_title, categories in [
        ("Worst regional integrated absolute error", "Worst regional IAE (m*s)", ["Fixed", "Scheduled"]),
        ("Maximum level departure", "Maximum level departure (m)", ["Fixed", "Scheduled"]),
    ]:
        series = data[panel]
        (only_name, points), = series.items()
        top, bottom = y_cursor, y_cursor + panel_height
        svg_parts.append(text(left, top - 34, panel, "title"))
        svg_parts += render_grouped_bars(
            {only_name: points}, categories, axis_title, top, bottom, left, right,
        )
        y_cursor = bottom + gap

    top, bottom = y_cursor, y_cursor + panel_height
    svg_parts.append(text(left, top - 34, "Regional IAE", "title"))
    svg_parts += render_grouped_bars(
        data["Regional IAE"], ["0.30-0.55", "0.55-0.80", "0.80-0.55", "0.55-0.30"],
        "Regional IAE (m*s)", top, bottom, left, right,
    )
    entries = [(name, COLOURS.get(name, "#1268b3")) for name in data["Regional IAE"]]
    svg_parts += legend(entries, legend_x, top + 10)
    y_cursor = bottom + gap

    total_height = y_cursor + 20
    total_width = legend_x + 160
    header = f'''<svg xmlns="http://www.w3.org/2000/svg" width="{total_width:.0f}" height="{total_height:.0f}" viewBox="0 0 {total_width:.0f} {total_height:.0f}" role="img">
<title>Worst regional tracking error and maximum departure for fixed and scheduled gravity-tank control</title>
<desc>Simulation evidence: worst regional integrated absolute level error and maximum level departure for the fixed high-level PI and the gain-scheduled PI, and the four per-transition regional IAE values behind the worst-regional comparison.</desc>
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
