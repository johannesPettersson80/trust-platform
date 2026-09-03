#!/usr/bin/env python3
"""Render Lesson 55 from its retained native ratio-comparison trace.

Reads the shipped panel/series samples from
``blending-ratio-evidence-trace.csv`` and redraws
``blending-ratio-evidence.svg`` with the same two panels shown to the
learner: blend ratio and additive request for the feedback-only and
ratio-plus-trim blenders, including the ratio-demand series in the additive
panel. Pure Python, no third-party dependencies.

The axis title for the additive panel is corrected here (finding L55-9):
the shipped SVG named the panel "Additive request — ratio demand green
dashed (flow (L/min))", burying the third series' identity inside a doubled,
nested-parenthesis axis title instead of a legend entry. This generator
gives the panel a clean "Quantity (unit)" title and a proper legend swatch
for the ratio-demand series instead.
"""

from __future__ import annotations

import csv
from pathlib import Path

ROOT = Path(__file__).resolve().parent
TRACE = ROOT / "blending-ratio-evidence-trace.csv"
OUTPUT = ROOT / "blending-ratio-evidence.svg"

INK = "#243044"
GRID = "#d8e0eb"
AXIS = "#526078"
PAPER = "#f7f9fc"

# name -> (colour, dash-array or "" for solid)
SERIES_STYLE = {
    "Feedback only": ("#6b7280", ""),
    "Ratio plus trim": ("#2563eb", ""),
    "Ratio demand": ("#059669", "7 5"),
}

# (CSV panel name, clean "Quantity (unit)" axis title, series to plot in draw order)
PANELS = [
    ("Blend ratio", "Blend ratio (additive/feed ratio)", ["Feedback only", "Ratio plus trim"]),
    ("Additive request — ratio demand green dashed", "Additive request (L/min)",
     ["Feedback only", "Ratio plus trim", "Ratio demand"]),
]

LEGEND_SERIES = ["Feedback only", "Ratio plus trim", "Ratio demand"]

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
    for name in LEGEND_SERIES:
        colour, dash = SERIES_STYLE[name]
        dasharray = f' stroke-dasharray="{dash}"' if dash else ""
        body.append(f'<line x1="{legend_x:.1f}" y1="108" x2="{legend_x + 28:.1f}" y2="108" stroke="{colour}" stroke-width="3"{dasharray}/>')
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
            body.append(text(LEFT - 8, yy + 4, f"{value:.3f}" if y_hi - y_lo < 2 else f"{value:.2f}", anchor="end"))
        for value in nice_ticks(x_lo, x_hi):
            xx = sx(value)
            body.append(f'<line class="grid" x1="{xx:.2f}" y1="{top:.2f}" x2="{xx:.2f}" y2="{bottom:.2f}"/>')
            body.append(text(xx, bottom + 18, f"{value:.2f}", anchor="middle"))
        for name in series:
            colour, dash = SERIES_STYLE[name]
            dasharray = f' stroke-dasharray="{dash}"' if dash else ""
            points = points_by_series[name]
            step = max(1, len(points) // 450)
            encoded = " ".join(f"{sx(x):.2f},{sy(y):.2f}" for x, y in points[::step])
            body.append(f'<polyline fill="none" stroke="{colour}" stroke-width="2.2"{dasharray} points="{encoded}"/>')
        body.append(f'<rect x="{LEFT:.2f}" y="{top:.2f}" width="{RIGHT - LEFT:.2f}" height="{PANEL_HEIGHT:.2f}" fill="none" class="axis"/>')

    body.append(text(465, PANEL_TOP[-1] + PANEL_HEIGHT + 36, "Time after start (s)", size=12, anchor="middle"))

    return f'''<svg xmlns="http://www.w3.org/2000/svg" role="img" aria-labelledby="title desc" viewBox="0 0 900 650">
<title id="title">Ratio action moves with the wild stream</title>
<desc id="desc">Feed-ratio action prevents the composition error that feedback must otherwise correct.</desc>
<rect width="900" height="650" fill="{PAPER}"/>
<style>text{{font-family:system-ui,-apple-system,sans-serif;fill:{INK}}} .axis{{stroke:{AXIS};stroke-width:1}} .grid{{stroke:{GRID};stroke-width:1}}</style>
{text(55, 42, "Ratio action moves with the wild stream", size=24, weight="700")}
{text(55, 70, "Feed-ratio action prevents the composition error that feedback must otherwise correct.", size=14)}
{"".join(body)}
</svg>
'''


def main() -> None:
    OUTPUT.write_text(render(), encoding="utf-8")


if __name__ == "__main__":
    main()
