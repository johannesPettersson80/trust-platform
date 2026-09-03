#!/usr/bin/env python3
"""Render Lesson 19 from its retained native gain-sweep trace.

Pure Python, no third-party dependencies. Reads position-p-trace.csv and
position-p-summary.csv from this directory and writes position-p.svg.
"""

from __future__ import annotations

import csv
from pathlib import Path

ROOT = Path(__file__).resolve().parent
INK = "#243044"
GRID = "#d8e0eb"
AXIS = "#526078"
ORANGE = "#d55e00"
BLUE = "#0072b2"
PAPER = "#f7f9fc"

LEFT, RIGHT = 70.0, 860.0
POS_TOP, POS_BOTTOM = 96.0, 300.0
TRQ_TOP, TRQ_BOTTOM = 345.0, 505.0
T_MIN, T_MAX = 0.0, 15.0
POS_MIN, POS_MAX = -0.2, 1.8
TRQ_MIN, TRQ_MAX = -10.0, 10.0


def sx(t: float) -> float:
    return LEFT + (t - T_MIN) / (T_MAX - T_MIN) * (RIGHT - LEFT)


def sy_pos(v: float) -> float:
    return POS_BOTTOM - (v - POS_MIN) / (POS_MAX - POS_MIN) * (POS_BOTTOM - POS_TOP)


def sy_trq(v: float) -> float:
    return TRQ_BOTTOM - (v - TRQ_MIN) / (TRQ_MAX - TRQ_MIN) * (TRQ_BOTTOM - TRQ_TOP)


def polyline(points: list[tuple[float, float]], colour: str) -> str:
    encoded = " ".join(f"{x:.2f},{y:.2f}" for x, y in points)
    return f'<polyline fill="none" stroke="{colour}" stroke-width="2.2" points="{encoded}" />'


def render() -> str:
    with (ROOT / "position-p-trace.csv").open(newline="", encoding="utf-8") as handle:
        rows = list(csv.DictReader(handle))

    body: list[str] = []
    # Position panel grid (rad).
    for value in [0.0, 0.5, 1.0, 1.5]:
        y = sy_pos(value)
        body.append(f'<line class="grid" x1="{LEFT}" y1="{y:.2f}" x2="{RIGHT}" y2="{y:.2f}"/>'
                     f'<text x="{LEFT - 8:.1f}" y="{y + 4:.1f}" text-anchor="end" font-size="11">{value:.2f}</text>')
    body.append(f'<line x1="{sx(0):.2f}" y1="{POS_TOP}" x2="{sx(0):.2f}" y2="{sy_pos(1.0):.2f}" stroke="#8a8f98" stroke-width="1" stroke-dasharray="3 4"/>')
    body.append(f'<text x="{sx(0)+4:.1f}" y="{sy_pos(1.0)-4:.1f}" font-size="10" fill="#526078">SP 1 rad</text>')
    # Torque panel grid (N m).
    for value in [-10.0, -5.0, 0.0, 5.0, 10.0]:
        y = sy_trq(value)
        body.append(f'<line class="grid" x1="{LEFT}" y1="{y:.2f}" x2="{RIGHT}" y2="{y:.2f}"/>'
                     f'<text x="{LEFT - 8:.1f}" y="{y + 4:.1f}" text-anchor="end" font-size="11">{value:.2f}</text>')
    # Shared time axis ticks.
    for value in [0.0, 2.5, 5.0, 7.5, 10.0, 12.5, 15.0]:
        x = sx(value)
        body.append(f'<line class="grid" x1="{x:.2f}" y1="{POS_TOP}" x2="{x:.2f}" y2="{POS_BOTTOM}"/>')
        body.append(f'<line class="grid" x1="{x:.2f}" y1="{TRQ_TOP}" x2="{x:.2f}" y2="{TRQ_BOTTOM}"/>')
        body.append(f'<text x="{x:.2f}" y="{TRQ_BOTTOM + 18:.1f}" text-anchor="middle" font-size="11">{value:g}</text>')

    kp2_pos = [(sx(float(r["time_s"])), sy_pos(float(r["kp_2_position"]))) for r in rows]
    kp8_pos = [(sx(float(r["time_s"])), sy_pos(float(r["kp_8_position"]))) for r in rows]
    kp2_trq = [(sx(float(r["time_s"])), sy_trq(float(r["kp_2_torque"]))) for r in rows]
    kp8_trq = [(sx(float(r["time_s"])), sy_trq(float(r["kp_8_torque"]))) for r in rows]

    body += [
        f'<rect class="axis" x="{LEFT}" y="{POS_TOP}" width="{RIGHT-LEFT:.1f}" height="{POS_BOTTOM-POS_TOP:.1f}" fill="none"/>',
        f'<rect class="axis" x="{LEFT}" y="{TRQ_TOP}" width="{RIGHT-LEFT:.1f}" height="{TRQ_BOTTOM-TRQ_TOP:.1f}" fill="none"/>',
        polyline(kp2_pos, ORANGE),
        polyline(kp8_pos, BLUE),
        polyline(kp2_trq, ORANGE),
        polyline(kp8_trq, BLUE),
        f'<text x="{LEFT}" y="{POS_TOP - 12:.1f}" font-size="16" font-weight="700">Table position (rad)</text>',
        f'<text x="{LEFT}" y="{TRQ_TOP - 12:.1f}" font-size="16" font-weight="700">Applied motor torque (N m)</text>',
        f'<text x="{(LEFT+RIGHT)/2:.1f}" y="{TRQ_BOTTOM + 38:.1f}" text-anchor="middle" font-size="13" font-weight="700">Time after start (s)</text>',
        f'<line x1="55" y1="96" x2="83" y2="96" stroke="{ORANGE}" stroke-width="3"/><text x="89" y="101" font-size="12">Kp 2 position / torque</text>',
        f'<line x1="245" y1="96" x2="273" y2="96" stroke="{BLUE}" stroke-width="3"/><text x="279" y="101" font-size="12">Kp 8 position / torque</text>',
    ]
    return f'''<svg xmlns="http://www.w3.org/2000/svg" role="img" aria-labelledby="title desc" viewBox="0 0 900 560">
<title id="title">Position stiffness and damping from one gain sweep</title>
<desc id="desc">Configured Simulation evidence · same table, torque limit, timing and axes</desc>
<rect width="900" height="560" fill="{PAPER}"/>
<style>text{{font-family:system-ui,-apple-system,sans-serif;fill:{INK}}} .axis{{stroke:{AXIS};stroke-width:1}} .grid{{stroke:{GRID};stroke-width:1}}</style>
<text x="55" y="42" font-size="22" font-weight="700">Position stiffness and damping from one gain sweep</text>
<text x="55" y="66" font-size="13">Configured Simulation evidence &#183; same table, torque limit, timing and axes</text>
{"".join(body)}
</svg>
'''


def main() -> None:
    (ROOT / "position-p.svg").write_text(render(), encoding="utf-8")


if __name__ == "__main__":
    main()
