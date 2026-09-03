#!/usr/bin/env python3
"""Render Lesson 34 from its retained native trace and the tank's topology.

Reads figures/gravity-tank-linearization-trace.csv and writes
figures/gravity-tank-linearization.svg. Also writes the new physical diagram
figures/gravity-tank-linearization-topology.svg (pump inlet at the top,
fixed gravity outlet at the bottom); that diagram has no time-series data
source.
"""

from __future__ import annotations

import csv
import math
from pathlib import Path

ROOT = Path(__file__).resolve().parent
INK = "#243044"
MUTED = "#526078"
GRID = "#d8e0eb"
PAPER = "#f7f9fc"
BLUE = "#0072b2"
ORANGE = "#d55e00"
GREEN = "#009e73"

STYLE = (
    "text{font-family:system-ui,-apple-system,sans-serif;fill:#243044} "
    ".axis{stroke:#526078;stroke-width:1} .grid{stroke:#d8e0eb;stroke-width:1} "
    ".marker{stroke:#b43c32;stroke-width:1.5;stroke-dasharray:4 4}"
)


def nice_num(span: float, round_result: bool) -> float:
    if span <= 0:
        return 1.0
    exponent = math.floor(math.log10(span))
    fraction = span / (10 ** exponent)
    if round_result:
        nice_fraction = 1 if fraction < 1.5 else 2 if fraction < 3 else 5 if fraction < 7 else 10
    else:
        nice_fraction = 1 if fraction <= 1 else 2 if fraction <= 2 else 5 if fraction <= 5 else 10
    return nice_fraction * (10 ** exponent)


def nice_ticks(min_val: float, max_val: float, count: int = 5) -> list[float]:
    if min_val == max_val:
        min_val, max_val = min_val - 1.0, max_val + 1.0
    pad = (max_val - min_val) * 0.04
    min_val, max_val = min_val - pad, max_val + pad
    span = nice_num(max_val - min_val, False)
    step = nice_num(span / (count - 1), True)
    niced_min = math.floor(min_val / step) * step
    return [niced_min + i * step for i in range(count)]


def draw_panel(body: list[str], left: float, right: float, top: float, bottom: float,
               title: str, y_ticks: list[float], x_ticks: list[float], x_min: float, x_max: float,
               series: list[tuple[list[float], list[float], str, str]]) -> None:
    body.append(f'<text x="{left:.0f}" y="{top - 12:.0f}" font-size="16" font-weight="700">{title}</text>')

    def sy(v: float) -> float:
        return bottom - (v - y_ticks[0]) / (y_ticks[-1] - y_ticks[0]) * (bottom - top)

    def sx(v: float) -> float:
        return left + (v - x_min) / (x_max - x_min) * (right - left)

    for yv in y_ticks:
        body.append(f'<line class="grid" x1="{left:.2f}" y1="{sy(yv):.2f}" x2="{right:.2f}" y2="{sy(yv):.2f}"/>')
        body.append(f'<text x="{left - 8:.2f}" y="{sy(yv) + 4:.2f}" text-anchor="end" font-size="11">{yv:.2f}</text>')
    for xv in x_ticks:
        body.append(f'<line class="grid" x1="{sx(xv):.2f}" y1="{top:.2f}" x2="{sx(xv):.2f}" y2="{bottom:.2f}"/>')
        body.append(f'<text x="{sx(xv):.2f}" y="{bottom + 18:.0f}" text-anchor="middle" font-size="11">{xv:.1f}</text>')
    for xs, ys, colour, dash in series:
        pts = " ".join(f"{sx(x):.2f},{sy(y):.2f}" for x, y in zip(xs, ys))
        dash_attr = f' stroke-dasharray="{dash}"' if dash else ""
        body.append(f'<polyline fill="none" stroke="{colour}" stroke-width="2.2"{dash_attr} points="{pts}"/>')
    body.append(f'<rect class="axis" x="{left:.2f}" y="{top:.2f}" width="{right - left:.2f}" height="{bottom - top:.2f}" fill="none"/>')


def legend_row(body: list[str], y: float, items: list[tuple[str, str, str]]) -> None:
    x = 55.0
    for label, colour, dash in items:
        dash_attr = f' stroke-dasharray="{dash}"' if dash else ""
        body.append(f'<line x1="{x:.1f}" y1="{y}" x2="{x + 28:.1f}" y2="{y}" stroke="{colour}" stroke-width="3"{dash_attr}/>')
        body.append(f'<text x="{x + 34:.1f}" y="{y + 5}" font-size="12">{label}</text>')
        x += 34 + 7 * len(label) + 26


def render_results() -> str:
    with (ROOT / "gravity-tank-linearization-trace.csv").open(newline="", encoding="utf-8") as handle:
        rows = list(csv.DictReader(handle))
    t = [float(r["time_s"]) for r in rows]
    low_err = [float(r["25_cm_operating_point"]) for r in rows]
    mid_err = [float(r["50_cm_operating_point"]) for r in rows]
    high_err = [float(r["81_cm_operating_point"]) for r in rows]
    nonlinear = [float(r["nonlinear_tank"]) for r in rows]
    local = [float(r["local_tangent_model"]) for r in rows]
    x_ticks = [round(i * max(t) / 6, 1) for i in range(7)]

    body: list[str] = []
    legend_row(body, 101, [
        ("25 cm operating point", BLUE, ""), ("50 cm operating point", ORANGE, ""), ("81 cm operating point", GREEN, ""),
    ])
    legend_row(body, 121, [("nonlinear tank", BLUE, ""), ("local tangent model", ORANGE, "7 5")])
    y1 = nice_ticks(0.0, max(max(low_err), max(mid_err), max(high_err)))
    draw_panel(body, 70, 860, 155, 315, "Small-step nonlinear minus local-model error (cm)", y1, x_ticks, 0, max(t), [
        (t, low_err, BLUE, ""), (t, mid_err, ORANGE, ""), (t, high_err, GREEN, ""),
    ])
    y2 = nice_ticks(min(min(nonlinear), min(local)), max(max(nonlinear), max(local)))
    draw_panel(body, 70, 860, 360, 520, "Low-level large-step extrapolation (cm)", y2, x_ticks, 0, max(t), [
        (t, nonlinear, BLUE, ""), (t, local, ORANGE, "7 5"),
    ])
    body.append('<text x="465" y="598" text-anchor="middle" font-size="12">time (s)</text>')

    return f'''<svg xmlns="http://www.w3.org/2000/svg" role="img" aria-labelledby="title desc" viewBox="0 0 900 610">
<title id="title">Linear models are local to one operating point</title>
<desc id="desc">Configured Simulation evidence · three small steps qualify; the restored large step exposes extrapolation</desc>
<rect width="900" height="610" fill="{PAPER}"/>
<style>{STYLE}</style>
<text x="55" y="42" font-size="24" font-weight="700">Linear models are local to one operating point</text>
<text x="55" y="68" font-size="14">Configured Simulation evidence · three small steps qualify; the restored large step exposes extrapolation</text>
{"".join(body)}
</svg>
'''


def render_topology() -> str:
    body: list[str] = [
        '<path d="M120 90 L120 320 L380 320 L380 90" fill="#eaf2fb" stroke="#526078" stroke-width="2.5"/>',
        '<line x1="120" y1="150" x2="380" y2="150" stroke="#1268b3" stroke-width="1.5" stroke-dasharray="5 4"/>',
        '<text x="392" y="154" font-size="13" fill="#1268b3">level (h)</text>',
        f'<line x1="60" y1="70" x2="130" y2="70" stroke="{BLUE}" stroke-width="3" marker-end="url(#arrow-blue)"/>',
        '<line x1="130" y1="70" x2="130" y2="110" stroke="' + BLUE + '" stroke-width="3" marker-end="url(#arrow-blue)"/>',
        f'<text x="30" y="60" font-size="14" font-weight="700" fill="{BLUE}">variable-speed pump</text>',
        f'<text x="30" y="78" font-size="13" fill="{BLUE}">inlet flow (q)</text>',
        '<rect x="230" y="320" width="40" height="18" fill="#526078"/>',
        f'<line x1="250" y1="338" x2="250" y2="390" stroke="{ORANGE}" stroke-width="3" marker-end="url(#arrow-orange)"/>',
        f'<text x="258" y="360" font-size="13" fill="{ORANGE}">gravity outlet flow</text>',
        f'<text x="258" y="378" font-size="13" fill="{ORANGE}">(c·sqrt(h)), fixed area</text>',
    ]
    return f'''<svg xmlns="http://www.w3.org/2000/svg" width="620" height="420" viewBox="0 0 620 420" role="img" aria-labelledby="title2 desc2">
<title id="title2">Pump inlet at the top, gravity outlet at the bottom</title>
<desc id="desc2">A variable-speed pump supplies the tank from above; liquid drains through a fixed-area outlet at the bottom under gravity, so outlet flow grows with the square root of level.</desc>
<defs>
  <marker id="arrow-blue" viewBox="0 0 10 10" refX="9" refY="5" markerWidth="7" markerHeight="7" orient="auto-start-reverse"><path d="M 0 0 L 10 5 L 0 10 z" fill="{BLUE}"/></marker>
  <marker id="arrow-orange" viewBox="0 0 10 10" refX="9" refY="5" markerWidth="7" markerHeight="7" orient="auto-start-reverse"><path d="M 0 0 L 10 5 L 0 10 z" fill="{ORANGE}"/></marker>
</defs>
<rect width="620" height="420" rx="14" fill="{PAPER}" stroke="{GRID}"/>
<text x="40" y="410" font-family="system-ui, sans-serif" font-size="13" fill="{MUTED}">Outlet flow depends on level: doubling level does not double outlet flow.</text>
<g font-family="system-ui, sans-serif" fill="{INK}">
{"".join(body)}
</g>
</svg>
'''


def main() -> None:
    (ROOT / "gravity-tank-linearization.svg").write_text(render_results(), encoding="utf-8")
    (ROOT / "gravity-tank-linearization-topology.svg").write_text(render_topology(), encoding="utf-8")


if __name__ == "__main__":
    main()
