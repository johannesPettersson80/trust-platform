#!/usr/bin/env python3
"""Render Lesson 24 from its retained native combined-case trace.

Pure Python, no third-party dependencies. Reads
mechanical-nonidealities-trace.csv from this directory and writes
mechanical-nonidealities.svg. Plots the ideal-load position against the
combined-nonideality load position, and the requested-versus-applied torque
of the combined case, since overlaying all seven isolated cases at once
would obscure rather than clarify the comparison the table already carries.
"""

from __future__ import annotations

import csv
from pathlib import Path

ROOT = Path(__file__).resolve().parent
INK = "#243044"
GRID = "#d8e0eb"
AXIS = "#526078"
GREEN = "#009e73"
RED = "#b4232f"
BLUE = "#0072b2"
PAPER = "#f7f9fc"

LEFT, RIGHT = 70.0, 860.0
POS_TOP, POS_BOTTOM = 96.0, 300.0
TRQ_TOP, TRQ_BOTTOM = 345.0, 505.0
T_MIN, T_MAX = 0.0, 20.0
POS_MIN, POS_MAX = -0.6, 0.6
TRQ_MIN, TRQ_MAX = -3.0, 3.5


def sx(t: float) -> float:
    return LEFT + (t - T_MIN) / (T_MAX - T_MIN) * (RIGHT - LEFT)


def sy_pos(v: float) -> float:
    return POS_BOTTOM - (v - POS_MIN) / (POS_MAX - POS_MIN) * (POS_BOTTOM - POS_TOP)


def sy_trq(v: float) -> float:
    return TRQ_BOTTOM - (v - TRQ_MIN) / (TRQ_MAX - TRQ_MIN) * (TRQ_BOTTOM - TRQ_TOP)


def polyline(points: list[tuple[float, float]], colour: str) -> str:
    encoded = " ".join(f"{x:.2f},{y:.2f}" for x, y in points)
    return f'<polyline fill="none" stroke="{colour}" stroke-width="2" points="{encoded}" />'


def render() -> str:
    with (ROOT / "mechanical-nonidealities-trace.csv").open(newline="", encoding="utf-8") as handle:
        rows = list(csv.DictReader(handle))

    body: list[str] = []
    for value in [-0.5, -0.25, 0.0, 0.25, 0.5]:
        y = sy_pos(value)
        body.append(f'<line class="grid" x1="{LEFT}" y1="{y:.2f}" x2="{RIGHT}" y2="{y:.2f}"/>'
                     f'<text x="{LEFT - 8:.1f}" y="{y + 4:.1f}" text-anchor="end" font-size="11">{value:.2f}</text>')
    for value in [-2, -1, 0, 1, 2, 3]:
        y = sy_trq(value)
        body.append(f'<line class="grid" x1="{LEFT}" y1="{y:.2f}" x2="{RIGHT}" y2="{y:.2f}"/>'
                     f'<text x="{LEFT - 8:.1f}" y="{y + 4:.1f}" text-anchor="end" font-size="11">{value}</text>')
    for value in [0, 5, 10, 15, 20]:
        x = sx(value)
        body.append(f'<line class="grid" x1="{x:.2f}" y1="{POS_TOP}" x2="{x:.2f}" y2="{POS_BOTTOM}"/>')
        body.append(f'<line class="grid" x1="{x:.2f}" y1="{TRQ_TOP}" x2="{x:.2f}" y2="{TRQ_BOTTOM}"/>')
        body.append(f'<text x="{x:.2f}" y="{TRQ_BOTTOM + 18:.1f}" text-anchor="middle" font-size="11">{value}</text>')

    ideal_pos = [(sx(float(r["time_s"])), sy_pos(float(r["ideal_load"]))) for r in rows]
    combined_pos = [(sx(float(r["time_s"])), sy_pos(float(r["nonideal_load"]))) for r in rows]
    req_trq = [(sx(float(r["time_s"])), sy_trq(float(r["requested"]))) for r in rows]
    app_trq = [(sx(float(r["time_s"])), sy_trq(float(r["applied"]))) for r in rows]

    body += [
        f'<rect class="axis" x="{LEFT}" y="{POS_TOP}" width="{RIGHT-LEFT:.1f}" height="{POS_BOTTOM-POS_TOP:.1f}" fill="none"/>',
        f'<rect class="axis" x="{LEFT}" y="{TRQ_TOP}" width="{RIGHT-LEFT:.1f}" height="{TRQ_BOTTOM-TRQ_TOP:.1f}" fill="none"/>',
        polyline(ideal_pos, GREEN),
        polyline(combined_pos, RED),
        polyline(req_trq, RED),
        polyline(app_trq, BLUE),
        f'<line x1="55" y1="96" x2="83" y2="96" stroke="{GREEN}" stroke-width="2"/><text x="89" y="101" font-size="12">ideal load position</text>',
        f'<line x1="255" y1="96" x2="283" y2="96" stroke="{RED}" stroke-width="2"/><text x="289" y="101" font-size="12">combined load position / requested torque</text>',
        f'<line x1="620" y1="96" x2="648" y2="96" stroke="{BLUE}" stroke-width="2"/><text x="654" y="101" font-size="12">applied torque</text>',
        f'<text x="{LEFT}" y="{POS_TOP - 12:.1f}" font-size="16" font-weight="700">Load position (rad)</text>',
        f'<text x="{LEFT}" y="{TRQ_TOP - 12:.1f}" font-size="16" font-weight="700">Combined-case torque (N m)</text>',
        f'<text x="{(LEFT+RIGHT)/2:.1f}" y="{TRQ_BOTTOM + 38:.1f}" text-anchor="middle" font-size="13" font-weight="700">Time after start (s)</text>',
    ]
    return f'''<svg xmlns="http://www.w3.org/2000/svg" role="img" aria-labelledby="title desc" viewBox="0 0 900 560">
<title id="title">Mechanical limits can look like poor controller tuning</title>
<desc id="desc">Configured Simulation evidence &#183; identical controller and reversal; only plant nonidealities change</desc>
<rect width="900" height="560" fill="{PAPER}"/>
<style>text{{font-family:system-ui,-apple-system,sans-serif;fill:{INK}}} .axis{{stroke:{AXIS};stroke-width:1}} .grid{{stroke:{GRID};stroke-width:1}}</style>
<text x="55" y="42" font-size="22" font-weight="700">Mechanical limits can look like poor controller tuning</text>
<text x="55" y="66" font-size="13">Configured Simulation evidence &#183; identical controller and reversal; only plant nonidealities change</text>
{"".join(body)}
</svg>
'''


def main() -> None:
    (ROOT / "mechanical-nonidealities.svg").write_text(render(), encoding="utf-8")


if __name__ == "__main__":
    main()
