#!/usr/bin/env python3
"""Render Lesson 23 from its retained native filter-comparison trace.

Pure Python, no third-party dependencies. Reads filtered-derivative-trace.csv
from this directory and writes filtered-derivative.svg. Shows the raw
(noisy) position against the bypass and selected (first-order, 0.05 s)
filtered positions, and the matching applied-torque activity, since plotting
all six retained candidates at once would obscure rather than clarify the
selection.
"""

from __future__ import annotations

import csv
from pathlib import Path

ROOT = Path(__file__).resolve().parent
INK = "#243044"
GRID = "#d8e0eb"
AXIS = "#526078"
GREY = "#8a8f98"
RED = "#b4232f"
BLUE = "#0072b2"
PAPER = "#f7f9fc"

LEFT, RIGHT = 70.0, 860.0
POS_TOP, POS_BOTTOM = 96.0, 300.0
TRQ_TOP, TRQ_BOTTOM = 345.0, 505.0
T_MIN, T_MAX = 0.0, 20.0
POS_MIN, POS_MAX = -0.02, 0.55
TRQ_MIN, TRQ_MAX = -1.5, 6.0


def sx(t: float) -> float:
    return LEFT + (t - T_MIN) / (T_MAX - T_MIN) * (RIGHT - LEFT)


def sy_pos(v: float) -> float:
    return POS_BOTTOM - (v - POS_MIN) / (POS_MAX - POS_MIN) * (POS_BOTTOM - POS_TOP)


def sy_trq(v: float) -> float:
    return TRQ_BOTTOM - (v - TRQ_MIN) / (TRQ_MAX - TRQ_MIN) * (TRQ_BOTTOM - TRQ_TOP)


def polyline(points: list[tuple[float, float]], colour: str, dash: str = "") -> str:
    encoded = " ".join(f"{x:.2f},{y:.2f}" for x, y in points)
    dashed = f' stroke-dasharray="{dash}"' if dash else ""
    return f'<polyline fill="none" stroke="{colour}" stroke-width="2" points="{encoded}"{dashed} />'


def render() -> str:
    with (ROOT / "filtered-derivative-trace.csv").open(newline="", encoding="utf-8") as handle:
        rows = [r for r in csv.DictReader(handle) if float(r["time_s"]) <= 20.0]

    body: list[str] = []
    for value in [0.0, 0.1, 0.2, 0.3, 0.4, 0.5]:
        y = sy_pos(value)
        body.append(f'<line class="grid" x1="{LEFT}" y1="{y:.2f}" x2="{RIGHT}" y2="{y:.2f}"/>'
                     f'<text x="{LEFT - 8:.1f}" y="{y + 4:.1f}" text-anchor="end" font-size="11">{value:.2f}</text>')
    for value in [-1, 0, 1, 2, 3, 4, 5, 6]:
        y = sy_trq(value)
        body.append(f'<line class="grid" x1="{LEFT}" y1="{y:.2f}" x2="{RIGHT}" y2="{y:.2f}"/>'
                     f'<text x="{LEFT - 8:.1f}" y="{y + 4:.1f}" text-anchor="end" font-size="11">{value:g}</text>')
    for value in [0, 5, 10, 15, 20]:
        x = sx(value)
        body.append(f'<line class="grid" x1="{x:.2f}" y1="{POS_TOP}" x2="{x:.2f}" y2="{POS_BOTTOM}"/>')
        body.append(f'<line class="grid" x1="{x:.2f}" y1="{TRQ_TOP}" x2="{x:.2f}" y2="{TRQ_BOTTOM}"/>')
        body.append(f'<text x="{x:.2f}" y="{TRQ_BOTTOM + 18:.1f}" text-anchor="middle" font-size="11">{value}</text>')

    raw_pos = [(sx(float(r["time_s"])), sy_pos(float(r["raw_position"]))) for r in rows]
    bypass_pos = [(sx(float(r["time_s"])), sy_pos(float(r["bypass"]))) for r in rows]
    sel_pos = [(sx(float(r["time_s"])), sy_pos(float(r["first_τ=.05"]))) for r in rows]
    bypass_trq = [(sx(float(r["time_s"])), sy_trq(float(r["bypass_torque"]))) for r in rows]
    sel_trq = [(sx(float(r["time_s"])), sy_trq(float(r["τ=.05_torque"]))) for r in rows]

    body += [
        f'<rect class="axis" x="{LEFT}" y="{POS_TOP}" width="{RIGHT-LEFT:.1f}" height="{POS_BOTTOM-POS_TOP:.1f}" fill="none"/>',
        f'<rect class="axis" x="{LEFT}" y="{TRQ_TOP}" width="{RIGHT-LEFT:.1f}" height="{TRQ_BOTTOM-TRQ_TOP:.1f}" fill="none"/>',
        polyline(raw_pos, GREY, "3 3"),
        polyline(bypass_pos, RED),
        polyline(sel_pos, BLUE),
        polyline(bypass_trq, RED),
        polyline(sel_trq, BLUE),
        f'<line x1="55" y1="96" x2="83" y2="96" stroke="{GREY}" stroke-width="2" stroke-dasharray="3 3"/><text x="89" y="101" font-size="12">raw position</text>',
        f'<line x1="215" y1="96" x2="243" y2="96" stroke="{RED}" stroke-width="2"/><text x="249" y="101" font-size="12">bypass position / torque</text>',
        f'<line x1="455" y1="96" x2="483" y2="96" stroke="{BLUE}" stroke-width="2"/><text x="489" y="101" font-size="12">selected 0.05 s position / torque</text>',
        f'<text x="{LEFT}" y="{POS_TOP - 12:.1f}" font-size="16" font-weight="700">Table position (rad)</text>',
        f'<text x="{LEFT}" y="{TRQ_TOP - 12:.1f}" font-size="16" font-weight="700">Applied torque (N m)</text>',
        f'<text x="{(LEFT+RIGHT)/2:.1f}" y="{TRQ_BOTTOM + 38:.1f}" text-anchor="middle" font-size="13" font-weight="700">Time after start (s)</text>',
    ]
    return f'''<svg xmlns="http://www.w3.org/2000/svg" role="img" aria-labelledby="title desc" viewBox="0 0 900 560">
<title id="title">Derivative filtering trades encoder-noise activity for lag</title>
<desc id="desc">Configured Simulation evidence &#183; same servo, noise seed, controller and axes</desc>
<rect width="900" height="560" fill="{PAPER}"/>
<style>text{{font-family:system-ui,-apple-system,sans-serif;fill:{INK}}} .axis{{stroke:{AXIS};stroke-width:1}} .grid{{stroke:{GRID};stroke-width:1}}</style>
<text x="55" y="42" font-size="22" font-weight="700">Derivative filtering trades encoder-noise activity for lag</text>
<text x="55" y="66" font-size="13">Configured Simulation evidence &#183; same servo, noise seed, controller and axes</text>
{"".join(body)}
</svg>
'''


def main() -> None:
    (ROOT / "filtered-derivative.svg").write_text(render(), encoding="utf-8")


if __name__ == "__main__":
    main()
