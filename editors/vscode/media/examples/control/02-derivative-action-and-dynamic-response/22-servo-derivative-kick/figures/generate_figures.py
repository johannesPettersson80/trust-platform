#!/usr/bin/env python3
"""Render Lesson 22 from its retained native three-weight trace.

Pure Python, no third-party dependencies. Reads derivative-kick-trace.csv
from this directory and writes derivative-kick.svg.
"""

from __future__ import annotations

import csv
from pathlib import Path

ROOT = Path(__file__).resolve().parent
INK = "#243044"
GRID = "#d8e0eb"
AXIS = "#526078"
RED = "#b4232f"
ORANGE = "#d55e00"
BLUE = "#0072b2"
PAPER = "#f7f9fc"

LEFT, RIGHT = 70.0, 860.0
REQ_TOP, REQ_BOTTOM = 96.0, 300.0
APP_TOP, APP_BOTTOM = 345.0, 505.0
T_MIN, T_MAX = 0.0, 15.0
REQ_MIN, REQ_MAX = 0.0, 85.0
APP_MIN, APP_MAX = 0.0, 11.0


def sx(t: float) -> float:
    return LEFT + (t - T_MIN) / (T_MAX - T_MIN) * (RIGHT - LEFT)


def sy_req(v: float) -> float:
    return REQ_BOTTOM - (v - REQ_MIN) / (REQ_MAX - REQ_MIN) * (REQ_BOTTOM - REQ_TOP)


def sy_app(v: float) -> float:
    return APP_BOTTOM - (v - APP_MIN) / (APP_MAX - APP_MIN) * (APP_BOTTOM - APP_TOP)


def polyline(points: list[tuple[float, float]], colour: str) -> str:
    encoded = " ".join(f"{x:.2f},{y:.2f}" for x, y in points)
    return f'<polyline fill="none" stroke="{colour}" stroke-width="2.2" points="{encoded}" />'


def render() -> str:
    with (ROOT / "derivative-kick-trace.csv").open(newline="", encoding="utf-8") as handle:
        rows = list(csv.DictReader(handle))

    body: list[str] = []
    for value in [0, 20, 40, 60, 80]:
        y = sy_req(value)
        body.append(f'<line class="grid" x1="{LEFT}" y1="{y:.2f}" x2="{RIGHT}" y2="{y:.2f}"/>'
                     f'<text x="{LEFT - 8:.1f}" y="{y + 4:.1f}" text-anchor="end" font-size="11">{value}</text>')
    for value in [0, 5, 10]:
        y = sy_app(value)
        body.append(f'<line class="grid" x1="{LEFT}" y1="{y:.2f}" x2="{RIGHT}" y2="{y:.2f}"/>'
                     f'<text x="{LEFT - 8:.1f}" y="{y + 4:.1f}" text-anchor="end" font-size="11">{value}</text>')
    for value in [0.0, 2.5, 5.0, 7.5, 10.0, 12.5, 15.0]:
        x = sx(value)
        body.append(f'<line class="grid" x1="{x:.2f}" y1="{REQ_TOP}" x2="{x:.2f}" y2="{REQ_BOTTOM}"/>')
        body.append(f'<line class="grid" x1="{x:.2f}" y1="{APP_TOP}" x2="{x:.2f}" y2="{APP_BOTTOM}"/>')
        body.append(f'<text x="{x:.2f}" y="{APP_BOTTOM + 18:.1f}" text-anchor="middle" font-size="11">{value:g}</text>')

    series = [("c=1_request", "c=1_applied", RED, "c=1"), ("c=.5_request", "c=.5_applied", ORANGE, "c=0.5"), ("c=0_request", "c=0_applied", BLUE, "c=0")]
    body += [
        f'<rect class="axis" x="{LEFT}" y="{REQ_TOP}" width="{RIGHT-LEFT:.1f}" height="{REQ_BOTTOM-REQ_TOP:.1f}" fill="none"/>',
        f'<rect class="axis" x="{LEFT}" y="{APP_TOP}" width="{RIGHT-LEFT:.1f}" height="{APP_BOTTOM-APP_TOP:.1f}" fill="none"/>',
        f'<line x1="{sx(5):.2f}" y1="{REQ_TOP}" x2="{sx(5):.2f}" y2="{REQ_BOTTOM}" stroke="#8a8f98" stroke-width="1" stroke-dasharray="3 4"/>',
        f'<text x="{sx(5)+4:.1f}" y="{REQ_TOP+14:.1f}" font-size="10" fill="#526078">SP step</text>',
    ]
    legend_x = 55.0
    for req_key, app_key, colour, label in series:
        body.append(polyline([(sx(float(r["time_s"])), sy_req(float(r[req_key]))) for r in rows], colour))
        body.append(polyline([(sx(float(r["time_s"])), sy_app(float(r[app_key]))) for r in rows], colour))
        body.append(f'<line x1="{legend_x:.1f}" y1="96" x2="{legend_x+28:.1f}" y2="96" stroke="{colour}" stroke-width="3"/>'
                     f'<text x="{legend_x+34:.1f}" y="101" font-size="12">{label} requested / applied</text>')
        legend_x += 34 + 11 * len(f"{label} requested / applied") + 30

    body += [
        f'<text x="{LEFT}" y="{REQ_TOP - 12:.1f}" font-size="16" font-weight="700">Requested torque (N m)</text>',
        f'<text x="{LEFT}" y="{APP_TOP - 12:.1f}" font-size="16" font-weight="700">Applied torque (N m)</text>',
        f'<text x="{(LEFT+RIGHT)/2:.1f}" y="{APP_BOTTOM + 38:.1f}" text-anchor="middle" font-size="13" font-weight="700">Time after start (s)</text>',
    ]
    return f'''<svg xmlns="http://www.w3.org/2000/svg" role="img" aria-labelledby="title desc" viewBox="0 0 900 560">
<title id="title">Derivative setpoint handling removes the torque-command kick</title>
<desc id="desc">Configured Simulation evidence &#183; same PID, positioning table, step, limits and axes</desc>
<rect width="900" height="560" fill="{PAPER}"/>
<style>text{{font-family:system-ui,-apple-system,sans-serif;fill:{INK}}} .axis{{stroke:{AXIS};stroke-width:1}} .grid{{stroke:{GRID};stroke-width:1}}</style>
<text x="55" y="42" font-size="22" font-weight="700">Derivative setpoint handling removes the torque-command kick</text>
<text x="55" y="66" font-size="13">Configured Simulation evidence &#183; same PID, positioning table, step, limits and axes</text>
{"".join(body)}
</svg>
'''


def main() -> None:
    (ROOT / "derivative-kick.svg").write_text(render(), encoding="utf-8")


if __name__ == "__main__":
    main()
