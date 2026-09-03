#!/usr/bin/env python3
"""Render Lesson 33 from its retained native trace and the flow topology.

Reads figures/tank-heater-conservation-trace.csv and writes
figures/tank-heater-conservation.svg. Also writes the new physical diagram
figures/tank-heater-conservation-topology.svg (labeled inlet/outlet flow
arrows for the tank and heater-power/heat-loss arrows for the vessel); that
diagram has no time-series data source.
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
PINK = "#cc79a7"

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
               series: list[tuple[list[float], list[float], str, str]], marker: float | None,
               marker_label: str | None) -> None:
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
    if marker is not None:
        body.append(f'<line class="marker" x1="{sx(marker):.2f}" y1="{top:.2f}" x2="{sx(marker):.2f}" y2="{bottom:.2f}"/>')
        if marker_label:
            body.append(f'<text x="{sx(marker) + 6:.2f}" y="{top + 18:.2f}" font-size="11" fill="#8c2f28">{marker_label}</text>')
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
    with (ROOT / "tank-heater-conservation-trace.csv").open(newline="", encoding="utf-8") as handle:
        rows = list(csv.DictReader(handle))
    t = [float(r["time_s"]) for r in rows]
    level = [float(r["simulated_level"]) for r in rows]
    level_pred = [float(r["mass-balance_prediction"]) for r in rows]
    temp = [float(r["simulated_temperature"]) for r in rows]
    temp_pred = [float(r["energy-balance_prediction"]) for r in rows]
    x_ticks = [round(i * max(t) / 6, 1) for i in range(7)]

    body: list[str] = []
    legend_row(body, 101, [
        ("simulated level", BLUE, ""), ("mass-balance prediction", ORANGE, "7 5"),
        ("simulated temperature", GREEN, ""), ("energy-balance prediction", PINK, "7 5"),
    ])
    y1 = nice_ticks(min(level), max(level))
    draw_panel(body, 70, 860, 155, 315, "Tank level (cm)", y1, x_ticks, 0, max(t), [
        (t, level, BLUE, ""), (t, level_pred, ORANGE, "7 5"),
    ], marker=1.0, marker_label="heater delay ends")
    y2 = nice_ticks(min(temp), max(temp))
    draw_panel(body, 70, 860, 360, 520, "Heater temperature (degC)", y2, x_ticks, 0, max(t), [
        (t, temp, GREEN, ""), (t, temp_pred, PINK, "7 5"),
    ], marker=1.0, marker_label=None)
    body.append('<text x="465" y="598" text-anchor="middle" font-size="12">time (s)</text>')

    return f'''<svg xmlns="http://www.w3.org/2000/svg" role="img" aria-labelledby="title desc" viewBox="0 0 900 610">
<title id="title">Balances predict stored level and heat</title>
<desc id="desc">Configured Simulation evidence · analytical predictions are compared with independently executed plants</desc>
<rect width="900" height="610" fill="{PAPER}"/>
<style>{STYLE}</style>
<text x="55" y="42" font-size="24" font-weight="700">Balances predict stored level and heat</text>
<text x="55" y="68" font-size="14">Configured Simulation evidence · analytical predictions are compared with independently executed plants</text>
{"".join(body)}
</svg>
'''


def render_topology() -> str:
    body: list[str] = [
        '<rect x="55" y="140" width="250" height="230" rx="10" fill="#eaf2fb" stroke="#526078" stroke-width="2"/>',
        '<text x="180" y="172" text-anchor="middle" font-size="18" font-weight="700">Buffer tank</text>',
        f'<line x1="20" y1="200" x2="105" y2="200" stroke="{BLUE}" stroke-width="3" marker-end="url(#arrow-blue)"/>',
        f'<text x="25" y="192" font-size="13" fill="{BLUE}">inlet flow (q_in)</text>',
        f'<line x1="255" y1="330" x2="340" y2="330" stroke="{ORANGE}" stroke-width="3" marker-end="url(#arrow-orange)"/>',
        f'<text x="255" y="322" font-size="13" fill="{ORANGE}">outlet flow (q_out)</text>',
        '<rect x="90" y="230" width="180" height="90" fill="#dbeafe" stroke="#526078" stroke-width="1.5"/>',
        '<text x="180" y="280" text-anchor="middle" font-size="14">stored volume (level h)</text>',

        '<rect x="405" y="140" width="290" height="230" rx="10" fill="#fff7ed" stroke="#9a3412" stroke-width="2"/>',
        '<text x="550" y="172" text-anchor="middle" font-size="18" font-weight="700">Heated vessel</text>',
        f'<line x1="370" y1="330" x2="450" y2="330" stroke="{ORANGE}" stroke-width="3" marker-end="url(#arrow-orange)"/>',
        f'<text x="365" y="322" font-size="13" fill="{ORANGE}">heater power (P)</text>',
        f'<line x1="650" y1="180" x2="730" y2="180" stroke="#8792a5" stroke-width="3" marker-end="url(#arrow-gray)"/>',
        f'<text x="655" y="172" font-size="13" fill="{MUTED}">heat loss to ambient (H)</text>',
        '<rect x="440" y="230" width="220" height="90" fill="#fef3e2" stroke="#9a3412" stroke-width="1.5"/>',
        '<text x="550" y="280" text-anchor="middle" font-size="14">stored energy (temperature T)</text>',
    ]
    return f'''<svg xmlns="http://www.w3.org/2000/svg" width="800" height="400" viewBox="0 0 800 400" role="img" aria-labelledby="title2 desc2">
<title id="title2">Labeled flow and heat paths for the tank and vessel</title>
<desc id="desc2">Inlet flow adds stored volume and outlet flow removes it in the buffer tank; heater power adds stored energy and heat loss to ambient removes it in the heated vessel.</desc>
<defs>
  <marker id="arrow-blue" viewBox="0 0 10 10" refX="9" refY="5" markerWidth="7" markerHeight="7" orient="auto-start-reverse"><path d="M 0 0 L 10 5 L 0 10 z" fill="{BLUE}"/></marker>
  <marker id="arrow-orange" viewBox="0 0 10 10" refX="9" refY="5" markerWidth="7" markerHeight="7" orient="auto-start-reverse"><path d="M 0 0 L 10 5 L 0 10 z" fill="{ORANGE}"/></marker>
  <marker id="arrow-gray" viewBox="0 0 10 10" refX="9" refY="5" markerWidth="7" markerHeight="7" orient="auto-start-reverse"><path d="M 0 0 L 10 5 L 0 10 z" fill="#8792a5"/></marker>
</defs>
<rect width="800" height="400" rx="14" fill="{PAPER}" stroke="{GRID}"/>
<text x="48" y="45" font-family="system-ui, sans-serif" font-size="20" font-weight="700" fill="{INK}">Accumulation equals rate entering minus rate leaving</text>
<g font-family="system-ui, sans-serif" fill="{INK}">
{"".join(body)}
</g>
</svg>
'''


def main() -> None:
    (ROOT / "tank-heater-conservation.svg").write_text(render_results(), encoding="utf-8")
    (ROOT / "tank-heater-conservation-topology.svg").write_text(render_topology(), encoding="utf-8")


if __name__ == "__main__":
    main()
