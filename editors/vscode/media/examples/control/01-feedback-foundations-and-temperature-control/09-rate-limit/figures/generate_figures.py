#!/usr/bin/env python3
"""Render Lesson 9 from its retained native valve-rate trace."""

from __future__ import annotations

import csv
from pathlib import Path


ROOT = Path(__file__).resolve().parent
INK = "#172033"
MUTED = "#526078"
GRID = "#cbd5e1"
BLUE = "#1268b3"
ORANGE = "#d94801"
GREEN = "#16803c"
RED = "#b4232f"
PURPLE = "#6f42c1"
PAPER = "#f8fafc"


def text(x: float, y: float, value: str, css: str = "note", anchor: str = "start") -> str:
    return f'<text x="{x:.1f}" y="{y:.1f}" class="{css}" text-anchor="{anchor}">{value}</text>'


def line(x1: float, y1: float, x2: float, y2: float, css: str) -> str:
    return f'<line x1="{x1:.1f}" y1="{y1:.1f}" x2="{x2:.1f}" y2="{y2:.1f}" class="{css}" />'


def polyline(points: list[tuple[float, float]], colour: str, width: float = 2.2, dash: str = "") -> str:
    encoded = " ".join(f"{x:.1f},{y:.1f}" for x, y in points)
    dashed = f' stroke-dasharray="{dash}"' if dash else ""
    return f'<polyline points="{encoded}" fill="none" stroke="{colour}" stroke-width="{width}"{dashed} />'


def render() -> str:
    with (ROOT / "rate-limit-trace.csv").open(newline="", encoding="utf-8") as handle:
        rows = [row for row in csv.DictReader(handle) if float(row["time_s"]) <= 55.0]

    left, right = 92.0, 850.0
    flow_top, flow_bottom = 118.0, 300.0
    position_top, position_bottom = 374.0, 556.0
    rate_top, rate_bottom = 630.0, 775.0
    sx = lambda value: left + value / 55.0 * (right - left)
    sy_flow = lambda value: flow_bottom - value / 20.0 * (flow_bottom - flow_top)
    sy_position = lambda value: position_bottom - value / 35.0 * (position_bottom - position_top)
    sy_rate = lambda value: rate_bottom - (value + 0.75) / 2.5 * (rate_bottom - rate_top)
    body: list[str] = []

    for value in [0, 5, 10, 15, 20]:
        body += [line(left, sy_flow(value), right, sy_flow(value), "grid"), text(left - 12, sy_flow(value) + 4, str(value), anchor="end")]
    for value in [0, 10, 20, 30]:
        body += [line(left, sy_position(value), right, sy_position(value), "grid"), text(left - 12, sy_position(value) + 4, str(value), anchor="end")]
    for value in [-0.5, 0, 0.5, 1.0, 1.5]:
        body += [line(left, sy_rate(value), right, sy_rate(value), "grid"), text(left - 12, sy_rate(value) + 4, f"{value:.1f}", anchor="end")]
    for value in [0, 5, 15, 25, 35, 45, 55]:
        body += [line(sx(value), flow_top, sx(value), flow_bottom, "grid"), line(sx(value), position_top, sx(value), position_bottom, "grid"), line(sx(value), rate_top, sx(value), rate_bottom, "grid"), text(sx(value), rate_bottom + 24, str(value), anchor="middle")]
    for event, label in [(0.2, "SP raised to 10 L/min"), (15.2, "SP restored to 0 L/min"), (49.7, "Valve reaches closed")]:
        for top, bottom in [(flow_top, flow_bottom), (position_top, position_bottom), (rate_top, rate_bottom)]:
            body.append(line(sx(event), top, sx(event), bottom, "event"))
        body.append(text(sx(event) + 5, flow_top + 17, label))

    body += [
        line(left, flow_top, left, flow_bottom, "axis"), line(left, flow_bottom, right, flow_bottom, "axis"),
        line(left, position_top, left, position_bottom, "axis"), line(left, position_bottom, right, position_bottom, "axis"),
        line(left, rate_top, left, rate_bottom, "axis"), line(left, rate_bottom, right, rate_bottom, "axis"),
        polyline([(sx(float(row["time_s"])), sy_flow(float(row["setpoint_L_min"]))) for row in rows[::2]], INK, 1.7, "7 5"),
        polyline([(sx(float(row["time_s"])), sy_flow(float(row["pv_L_min"]))) for row in rows[::2]], BLUE, 2.5),
        polyline([(sx(float(row["time_s"])), sy_position(float(row["requested_mv_percent"]))) for row in rows[::2]], RED, 2.2),
        polyline([(sx(float(row["time_s"])), sy_position(float(row["applied_mv_percent"]))) for row in rows[::2]], GREEN, 2.5),
        polyline([(sx(float(row["time_s"])), sy_rate(float(row["measured_travel_rate_pp_s"]))) for row in rows[::2]], PURPLE, 2.2),
        line(left, sy_rate(1.5), right, sy_rate(1.5), "limit"),
        line(left, sy_rate(-0.5), right, sy_rate(-0.5), "limit"),
        text(470, 821, "Time after start (s)", "label", "middle"),
        '<text x="25" y="209" class="label" text-anchor="middle" transform="rotate(-90 25 209)">Flow (L/min)</text>',
        '<text x="25" y="465" class="label" text-anchor="middle" transform="rotate(-90 25 465)">Valve position (%)</text>',
        '<text x="25" y="703" class="label" text-anchor="middle" transform="rotate(-90 25 703)">Travel rate (pp/s)</text>',
        line(252, 88, 282, 88, "sp"), text(289, 92, "SP"), line(350, 88, 380, 88, "pv"), text(387, 92, "PV"),
        line(214, 344, 244, 344, "request"), text(251, 348, "Requested valve position"),
        line(504, 344, 534, 344, "applied"), text(541, 348, "Applied valve position"),
        text(sx(19), sy_position(29), "The request reverses first; the geared valve closes at only 0.5 pp/s", "annotation"),
    ]
    return f'''<svg xmlns="http://www.w3.org/2000/svg" width="900" height="850" viewBox="0 0 900 850" role="img">
<title>Requested and rate-limited motor-valve motion</title>
<desc>Simulation evidence showing heating-water flow, requested and applied valve position, and asymmetric measured opening and closing rates during a bounded setpoint experiment.</desc>
<style>
  text {{ font-family: Arial, sans-serif; font-size: 12px; }}
  .title {{ font-size: 22px; font-weight: 700; fill: {INK}; }} .subtitle {{ font-size: 13px; fill: {MUTED}; }}
  .axis {{ stroke: #8792a5; stroke-width: 1.4; }} .grid {{ stroke: {GRID}; stroke-width: 1; stroke-dasharray: 3 5; }}
  .event {{ stroke: #a51c30; stroke-width: 1.4; stroke-dasharray: 5 4; }} .limit {{ stroke: {ORANGE}; stroke-width: 1.5; stroke-dasharray: 7 4; }}
  .note {{ fill: {MUTED}; }} .label {{ font-size: 13px; font-weight: 700; fill: {INK}; }} .annotation {{ font-size: 12px; font-weight: 700; fill: {RED}; }}
  .sp {{ stroke: {INK}; stroke-width: 1.7; stroke-dasharray: 7 5; }} .pv {{ stroke: {BLUE}; stroke-width: 2.5; }}
  .request {{ stroke: {RED}; stroke-width: 2.2; }} .applied {{ stroke: {GREEN}; stroke-width: 2.5; }}
</style>
<rect width="900" height="850" rx="12" fill="{PAPER}" stroke="{GRID}" />
{text(48, 42, "A valid position request can still be physically unreachable this scan", "title")}
{text(48, 65, "Simulation evidence · shipped ST · 100 ms samples · seed 20260824", "subtitle")}
{"".join(body)}
</svg>
'''


def main() -> None:
    (ROOT / "rate-limit-response.svg").write_text(render(), encoding="utf-8")


if __name__ == "__main__":
    main()
