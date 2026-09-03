#!/usr/bin/env python3
"""Render Lesson 56 from its retained native split-range-comparison trace.

Reads the shipped panel/series samples from
``reactor-split-range-evidence-trace.csv`` and redraws
``reactor-split-range-evidence.svg`` with the same two panels shown to the
learner: reactor temperature and signed controller demand for the dead-zone,
overlap, and characterized reactors. Pure Python, no third-party
dependencies.
"""

from __future__ import annotations

import csv
from pathlib import Path

ROOT = Path(__file__).resolve().parent
TRACE = ROOT / "reactor-split-range-evidence-trace.csv"
OUTPUT = ROOT / "reactor-split-range-evidence.svg"

INK = "#243044"
GRID = "#d8e0eb"
AXIS = "#526078"
PAPER = "#f7f9fc"

SERIES_ORDER = ["Dead zone", "Overlap", "Characterized"]
SERIES_COLOR = {
    "Dead zone": "#b45309",
    "Overlap": "#dc2626",
    "Characterized": "#2563eb",
}

# (CSV panel name, clean "Quantity (unit)" axis title, series to plot)
PANELS = [
    ("Reactor temperature", "Reactor temperature (degC)", SERIES_ORDER),
    ("Signed controller demand", "Signed controller demand (%)", SERIES_ORDER),
]

LEFT, RIGHT = 75.0, 855.0
PANEL_TOP = [157.0, 407.0]
PANEL_HEIGHT = 160.0
TITLE_OFFSET = 12.0


def load_panels() -> dict[tuple[str, str], list[tuple[float, float]]]:
    data: dict[tuple[str, str], list[tuple[float, float]]] = {}
    with TRACE.open(newline="", encoding="utf-8") as handle:
        for row in csv.DictReader(handle):
            key = (row["panel"], row["series"])
            data.setdefault(key, []).append((float(row["x"]), float(row["y"])))
    for key in data:
        data[key].sort()
    return data


def nice_ticks(lo: float, hi: float, count: int = 5) -> list[float]:
    if hi <= lo:
        hi = lo + 1.0
    step = (hi - lo) / (count - 1)
    return [lo + step * i for i in range(count)]


def text(x: float, y: float, value: str, size: int = 11, anchor: str = "start", weight: str = "") -> str:
    fw = f' font-weight="{weight}"' if weight else ""
    return f'<text x="{x:.2f}" y="{y:.2f}" font-size="{size}"{fw} text-anchor="{anchor}">{value}</text>'


def render() -> str:
    data = load_panels()
    all_x = [x for panel, _, series in PANELS for name in series for x, _ in data[(panel, name)]]
    x_lo, x_hi = min(all_x), max(all_x)

    def sx(v: float) -> float:
        return LEFT + (v - x_lo) / (x_hi - x_lo) * (RIGHT - LEFT)

    body: list[str] = []

    legend_x = 55.0
    for name in SERIES_ORDER:
        colour = SERIES_COLOR[name]
        body.append(f'<line x1="{legend_x:.1f}" y1="108" x2="{legend_x + 28:.1f}" y2="108" stroke="{colour}" stroke-width="3"/>')
        body.append(text(legend_x + 34, 113, name))
        legend_x += 34 + 8.2 * len(name) + 30

    for (csv_panel, axis_title, series), top in zip(PANELS, PANEL_TOP):
        bottom = top + PANEL_HEIGHT
        points_by_series = {name: data[(csv_panel, name)] for name in series}
        y_values = [y for pts in points_by_series.values() for _, y in pts]
        y_lo, y_hi = min(y_values), max(y_values)
        pad = (y_hi - y_lo) * 0.08 or 1.0
        y_lo, y_hi = y_lo - pad, y_hi + pad

        def sy(v: float, y_lo=y_lo, y_hi=y_hi, top=top, bottom=bottom) -> float:
            return bottom - (v - y_lo) / (y_hi - y_lo) * (bottom - top)

        body.append(text(LEFT, top - TITLE_OFFSET - 1, axis_title, size=16, weight="700"))
        for value in nice_ticks(y_lo, y_hi):
            yy = sy(value)
            body.append(f'<line class="grid" x1="{LEFT:.2f}" y1="{yy:.2f}" x2="{RIGHT:.2f}" y2="{yy:.2f}"/>')
            body.append(text(LEFT - 8, yy + 4, f"{value:.2f}", anchor="end"))
        for value in nice_ticks(x_lo, x_hi):
            xx = sx(value)
            body.append(f'<line class="grid" x1="{xx:.2f}" y1="{top:.2f}" x2="{xx:.2f}" y2="{bottom:.2f}"/>')
            body.append(text(xx, bottom + 18, f"{value:.2f}", anchor="middle"))
        for name in series:
            points = points_by_series[name]
            step = max(1, len(points) // 450)
            encoded = " ".join(f"{sx(x):.2f},{sy(y):.2f}" for x, y in points[::step])
            body.append(f'<polyline fill="none" stroke="{SERIES_COLOR[name]}" stroke-width="2.2" points="{encoded}"/>')
        body.append(f'<rect x="{LEFT:.2f}" y="{top:.2f}" width="{RIGHT - LEFT:.2f}" height="{PANEL_HEIGHT:.2f}" fill="none" class="axis"/>')

    body.append(text(465, PANEL_TOP[-1] + PANEL_HEIGHT + 36, "Time after start (s)", size=12, anchor="middle"))

    return f'''<svg xmlns="http://www.w3.org/2000/svg" role="img" aria-labelledby="title desc" viewBox="0 0 900 650">
<title id="title">Characterized split range removes gap and overlap</title>
<desc id="desc">Equal signed demands expose dead zone, simultaneous utility, and installed-gain correction.</desc>
<rect width="900" height="650" fill="{PAPER}"/>
<style>text{{font-family:system-ui,-apple-system,sans-serif;fill:{INK}}} .axis{{stroke:{AXIS};stroke-width:1}} .grid{{stroke:{GRID};stroke-width:1}}</style>
{text(55, 42, "Characterized split range removes gap and overlap", size=24, weight="700")}
{text(55, 70, "Equal signed demands expose dead zone, simultaneous utility, and installed-gain correction.", size=14)}
{"".join(body)}
</svg>
'''


def main() -> None:
    OUTPUT.write_text(render(), encoding="utf-8")


if __name__ == "__main__":
    main()
