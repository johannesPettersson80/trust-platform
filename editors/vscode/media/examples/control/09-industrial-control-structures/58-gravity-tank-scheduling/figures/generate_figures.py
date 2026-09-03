#!/usr/bin/env python3
"""Render Lesson 58's evidence figure from its retained native scheduling trace.

Pure standard library; no third-party dependencies. Reads the checked-in
``gravity-tank-scheduling-evidence-trace.csv`` (panel, series, point, x, y,
x_unit, y_unit rows) and regenerates ``gravity-tank-scheduling-evidence.svg``
deterministically so the figure can never drift from the retained data.
"""

from __future__ import annotations

import csv
from pathlib import Path

ROOT = Path(__file__).resolve().parent
TRACE = ROOT / "gravity-tank-scheduling-evidence-trace.csv"
OUTPUT = ROOT / "gravity-tank-scheduling-evidence.svg"

INK = "#243044"
PAPER = "#f7f9fc"
GRID = "#d8e0eb"
AXIS = "#526078"

SVG_TITLE = "Gain scheduling follows the nonlinear gravity tank"
SVG_DESC = (
    "Scheduled gains improve the response across low, middle, and high "
    "operating regions without transition bumps."
)

# (legend label, CSV series name, colour, dash pattern or None)
SERIES = [
    ("Fixed gain", "Fixed gain", "#6b7280", None),
    ("Interpolated gain", "Interpolated gain", "#059669", None),
    ("Parallel bumpless", "Parallel bumpless", "#2563eb", None),
]

# (CSV panel name, displayed axis title "Quantity (unit)")
PANEL_1 = ("Tank level", "Tank level (m)")
PANEL_2 = ("Pump flow", "Pump flow (m3/s)")

LEFT, RIGHT = 75.0, 855.0
PANEL_TOP = [167.0, 417.0]
PANEL_HEIGHT = 160.0
CHAR_WIDTH = 7.3
LEGEND_GAP = 24.0


def load_series(path: Path) -> dict[tuple[str, str], list[tuple[float, float]]]:
    data: dict[tuple[str, str], list[tuple[float, float]]] = {}
    with path.open(newline="", encoding="utf-8") as handle:
        for row in csv.DictReader(handle):
            key = (row["panel"], row["series"])
            data.setdefault(key, []).append((float(row["x"]), float(row["y"])))
    for points in data.values():
        points.sort()
    return data


def text(x: float, y: float, value: str, size: int = 11, weight: str = "400",
          anchor: str = "start") -> str:
    weight_attr = f' font-weight="{weight}"' if weight != "400" else ""
    return (f'<text x="{x:.2f}" y="{y:.2f}" font-size="{size}"{weight_attr} '
            f'text-anchor="{anchor}">{value}</text>')


def grid_line(x1: float, y1: float, x2: float, y2: float) -> str:
    return f'<line class="grid" x1="{x1:.2f}" y1="{y1:.2f}" x2="{x2:.2f}" y2="{y2:.2f}"/>'


def polyline(points: list[tuple[float, float]], colour: str, dash: str | None) -> str:
    dash_attr = f' stroke-dasharray="{dash}"' if dash else ""
    encoded = " ".join(f"{x:.2f},{y:.2f}" for x, y in points)
    return (f'<polyline fill="none" stroke="{colour}" stroke-width="2.2"{dash_attr} '
            f'points="{encoded}"/>')


def render_legend(y: float) -> str:
    parts: list[str] = []
    x = 55.0
    for label, _series, colour, dash in SERIES:
        dash_attr = f' stroke-dasharray="{dash}"' if dash else ""
        parts.append(f'<line x1="{x:.1f}" y1="{y:.1f}" x2="{x + 28:.1f}" y2="{y:.1f}" '
                      f'stroke="{colour}" stroke-width="3"{dash_attr}/>')
        text_x = x + 34
        parts.append(text(text_x, y + 5, label, size=12))
        x = text_x + len(label) * CHAR_WIDTH + LEGEND_GAP
    return "".join(parts)


def render_panel(data: dict[tuple[str, str], list[tuple[float, float]]],
                  panel_csv_name: str, axis_title: str, top: float,
                  x_min: float, x_max: float) -> str:
    bottom = top + PANEL_HEIGHT
    series_points = {name: data[(panel_csv_name, name)] for _label, name, _c, _d in SERIES
                      if (panel_csv_name, name) in data}
    all_y = [y for points in series_points.values() for _x, y in points]
    y_min, y_max = min(all_y), max(all_y)
    if y_max == y_min:
        y_max = y_min + 1.0

    def sx(value: float) -> float:
        return LEFT + (value - x_min) / (x_max - x_min) * (RIGHT - LEFT)

    def sy(value: float) -> float:
        return bottom - (value - y_min) / (y_max - y_min) * PANEL_HEIGHT

    parts = [text(LEFT, top - 12, axis_title, size=16, weight="700")]
    for step in range(5):
        y_value = y_min + step * (y_max - y_min) / 4
        y_px = bottom - step * PANEL_HEIGHT / 4
        parts.append(grid_line(LEFT, y_px, RIGHT, y_px))
        parts.append(text(LEFT - 8, y_px + 4, f"{y_value:.2f}", anchor="end"))
    for step in range(5):
        x_value = x_min + step * (x_max - x_min) / 4
        x_px = LEFT + step * (RIGHT - LEFT) / 4
        parts.append(grid_line(x_px, top, x_px, bottom))
        parts.append(text(x_px, bottom + 18, f"{x_value:.2f}", anchor="middle"))
    for _label, name, colour, dash in SERIES:
        points = series_points.get(name)
        if not points:
            continue
        parts.append(polyline([(sx(x), sy(y)) for x, y in points], colour, dash))
    parts.append(f'<rect class="axis" x="{LEFT:.0f}" y="{top:.0f}" '
                  f'width="{RIGHT - LEFT:.0f}" height="{PANEL_HEIGHT:.0f}" fill="none"/>')
    parts.append(text((LEFT + RIGHT) / 2, bottom + 36, "time (s)", size=12, anchor="middle"))
    return "".join(parts)


def render() -> str:
    data = load_series(TRACE)
    all_x = [x for (panel, _series), points in data.items()
             for x, _y in points if panel in (PANEL_1[0], PANEL_2[0])]
    x_min, x_max = 0.0, max(all_x)

    body = [render_legend(118.0)]
    body.append(render_panel(data, PANEL_1[0], PANEL_1[1], PANEL_TOP[0], x_min, x_max))
    body.append(render_panel(data, PANEL_2[0], PANEL_2[1], PANEL_TOP[1], x_min, x_max))

    return f'''<svg xmlns="http://www.w3.org/2000/svg" role="img" aria-labelledby="title desc" viewBox="0 0 900 650">
<title id="title">{SVG_TITLE}</title>
<desc id="desc">{SVG_DESC}</desc>
<defs><marker id="arrow" markerWidth="8" markerHeight="8" refX="7" refY="3" orient="auto"><path d="M0,0 L0,6 L8,3 z" fill="{INK}"/></marker></defs>
<rect width="900" height="650" fill="{PAPER}"/>
<style>text{{font-family:system-ui,-apple-system,sans-serif;fill:{INK}}} .axis{{stroke:{AXIS};stroke-width:1}} .grid{{stroke:{GRID};stroke-width:1}}</style>
{text(55, 42, SVG_TITLE, size=24, weight="700")}
{text(55, 70, "Scheduled gains improve the response across low, middle, and high operating regions without", size=14)}
{text(55, 88, "transition bumps.", size=14)}
{"".join(body)}
</svg>
'''


def main() -> None:
    OUTPUT.write_text(render(), encoding="utf-8")


if __name__ == "__main__":
    main()
