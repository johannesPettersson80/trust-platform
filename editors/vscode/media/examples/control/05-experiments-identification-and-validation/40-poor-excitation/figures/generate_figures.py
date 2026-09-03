#!/usr/bin/env python3
"""Render Lesson 40 from its retained weak/qualified/feedback comparison trace."""

from __future__ import annotations

import csv
from pathlib import Path


ROOT = Path(__file__).resolve().parent
INK = "#243044"
GRID = "#d8e0eb"
PAPER = "#f7f9fc"
BLUE = "#0072b2"
ORANGE = "#d55e00"
PINK = "#cc79a7"


def text(x: float, y: float, value: str, size: int = 11, weight: str = "400", anchor: str = "start") -> str:
    return f'<text x="{x:.2f}" y="{y:.2f}" font-size="{size}" font-weight="{weight}" text-anchor="{anchor}">{value}</text>'


def polyline(points: list[tuple[float, float]], colour: str, dash: str = "") -> str:
    encoded = " ".join(f"{x:.2f},{y:.2f}" for x, y in points)
    dashed = f' stroke-dasharray="{dash}"' if dash else ""
    return f'<polyline fill="none" stroke="{colour}" stroke-width="2.2"{dashed} points="{encoded}" />'


def render() -> str:
    with (ROOT / "poor-excitation-comparison-trace.csv").open(newline="", encoding="utf-8") as handle:
        rows = list(csv.DictReader(handle))
    t_end = float(rows[-1]["time_s"])

    left, right = 70.0, 860.0
    top1, bottom1 = 155.0, 315.0
    top2, bottom2 = 360.0, 520.0
    t_range = (49.0, 56.0)
    i_range = (18.0, 31.0)
    sx = lambda t: left + t / t_end * (right - left)
    sy1 = lambda v: bottom1 - (v - t_range[0]) / (t_range[1] - t_range[0]) * (bottom1 - top1)
    sy2 = lambda v: bottom2 - (v - i_range[0]) / (i_range[1] - i_range[0]) * (bottom2 - top2)

    body: list[str] = []
    for i, value in enumerate([56.00, 54.25, 52.50, 50.75, 49.00]):
        y = top1 + i * (bottom1 - top1) / 4
        body.append(f'<line class="grid" x1="{left}" y1="{y:.2f}" x2="{right}" y2="{y:.2f}"/>{text(left - 8, y + 4, f"{value:.2f}", anchor="end")}')
    for i, value in enumerate([31.00, 27.75, 24.50, 21.25, 18.00]):
        y = top2 + i * (bottom2 - top2) / 4
        body.append(f'<line class="grid" x1="{left}" y1="{y:.2f}" x2="{right}" y2="{y:.2f}"/>{text(left - 8, y + 4, f"{value:.2f}", anchor="end")}')
    for i in range(7):
        tv = t_end * i / 6
        x = sx(tv)
        body.append(f'<line class="grid" x1="{x:.2f}" y1="{top1}" x2="{x:.2f}" y2="{bottom1}"/>{text(x, bottom1 + 18, f"{tv:.1f}", anchor="middle")}')
        body.append(f'<line class="grid" x1="{x:.2f}" y1="{top2}" x2="{x:.2f}" y2="{bottom2}"/>{text(x, bottom2 + 18, f"{tv:.1f}", anchor="middle")}')

    for column, colour in [("weak_open_loop", ORANGE), ("qualified_open_loop", BLUE), ("ordinary_feedback", PINK)]:
        body.append(polyline([(sx(float(r["time_s"])), sy1(float(r[column]))) for r in rows], colour))
    body.append(f'<rect x="{left}" y="{top1}" width="{right - left}" height="{bottom1 - top1}" fill="none" stroke="#526078"/>')

    for column, colour in [("weak_input", ORANGE), ("qualified_input", BLUE), ("feedback-created_input", PINK)]:
        body.append(polyline([(sx(float(r["time_s"])), sy2(float(r[column]))) for r in rows], colour))
    body.append(f'<rect x="{left}" y="{top2}" width="{right - left}" height="{bottom2 - top2}" fill="none" stroke="#526078"/>')

    legend = [("weak open loop", ORANGE), ("qualified open loop", BLUE), ("ordinary feedback", PINK),
              ("weak input", ORANGE), ("qualified input", BLUE), ("feedback-created input", PINK)]
    legend_x = 55
    legend_body = ""
    for label, colour in legend:
        legend_body += f'<line x1="{legend_x}" y1="96" x2="{legend_x + 28}" y2="96" stroke="{colour}" stroke-width="3"/>{text(legend_x + 34, 101, label)}'
        legend_x += 40 + 7 * len(label)

    return f'''<svg xmlns="http://www.w3.org/2000/svg" role="img" aria-labelledby="title desc" viewBox="0 0 900 610">
<title id="title">Identification data quality</title>
<desc id="desc">Configured Simulation evidence · three independent copies of the same heat exchanger</desc>
<rect width="900" height="610" fill="{PAPER}"/>
<style>text{{font-family:system-ui,-apple-system,sans-serif;fill:{INK}}} .axis{{stroke:#526078;stroke-width:1}} .grid{{stroke:{GRID};stroke-width:1}}</style>
{text(55, 42, "Identification data quality", size=24, weight="700")}
{text(55, 68, "Configured Simulation evidence &#183; three independent copies of the same heat exchanger", size=14)}
{legend_body}
{text(70, 143, "Measured outlet temperature (degC)", size=16, weight="700")}
{text(70, 348, "Applied valve position (%)", size=16, weight="700")}
{"".join(body)}
{text(465, 598, "time (s)", size=12, anchor="middle")}
</svg>
'''


def main() -> None:
    (ROOT / "poor-excitation-comparison.svg").write_text(render(), encoding="utf-8")


if __name__ == "__main__":
    main()
