#!/usr/bin/env python3
"""Render Lesson 8 from its retained native saturation trace."""

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
    with (ROOT / "saturation-trace.csv").open(newline="", encoding="utf-8") as handle:
        rows = [row for row in csv.DictReader(handle) if 50.0 <= float(row["time_s"]) <= 95.0]

    left, right = 92.0, 850.0
    temperature_top, temperature_bottom = 120.0, 345.0
    output_top, output_bottom = 420.0, 650.0
    sx = lambda value: left + (value - 50.0) / 45.0 * (right - left)
    sy_temperature = lambda value: temperature_bottom - (value - 32.0) / 14.0 * (temperature_bottom - temperature_top)
    sy_output = lambda value: output_bottom - value / 450.0 * (output_bottom - output_top)
    body: list[str] = [
        f'<rect x="{sx(55):.1f}" y="{temperature_top}" width="{sx(65) - sx(55):.1f}" height="{temperature_bottom - temperature_top}" fill="#f6e8ea" />',
        f'<rect x="{sx(55):.1f}" y="{output_top}" width="{sx(65) - sx(55):.1f}" height="{output_bottom - output_top}" fill="#f6e8ea" />',
    ]
    for value in [32, 35, 40, 45]:
        body += [line(left, sy_temperature(value), right, sy_temperature(value), "grid"), text(left - 12, sy_temperature(value) + 4, str(value), anchor="end")]
    for value in [0, 100, 200, 300, 400]:
        body += [line(left, sy_output(value), right, sy_output(value), "grid"), text(left - 12, sy_output(value) + 4, str(value), anchor="end")]
    for value in [50, 55, 65, 75, 85, 95]:
        body += [line(sx(value), temperature_top, sx(value), temperature_bottom, "grid"), line(sx(value), output_top, sx(value), output_bottom, "grid"), text(sx(value), output_bottom + 24, str(value), anchor="middle")]
    for event, label in [(55, "SP raised to 45 °C"), (65, "SP restored to 35 °C")]:
        body += [line(sx(event), temperature_top, sx(event), temperature_bottom, "event"), line(sx(event), output_top, sx(event), output_bottom, "event"), text(sx(event) + 6, temperature_top + 18, label)]
    body += [
        line(left, temperature_top, left, temperature_bottom, "axis"), line(left, temperature_bottom, right, temperature_bottom, "axis"),
        line(left, output_top, left, output_bottom, "axis"), line(left, output_bottom, right, output_bottom, "axis"),
        polyline([(sx(float(row["time_s"])), sy_temperature(float(row["setpoint_degC"]))) for row in rows[::2]], INK, 1.7, "7 5"),
        polyline([(sx(float(row["time_s"])), sy_temperature(float(row["pv_degC"]))) for row in rows[::2]], BLUE, 2.5),
        polyline([(sx(float(row["time_s"])), sy_output(float(row["requested_mv_percent"]))) for row in rows[::2]], RED, 2.3),
        polyline([(sx(float(row["time_s"])), sy_output(float(row["applied_mv_percent"]))) for row in rows[::2]], GREEN, 2.5),
        line(left, sy_output(100.0), right, sy_output(100.0), "limit"),
        text(470, 694, "Time after start (s)", "label", "middle"),
        '<text x="26" y="233" class="label" text-anchor="middle" transform="rotate(-90 26 233)">Temperature (°C)</text>',
        '<text x="26" y="535" class="label" text-anchor="middle" transform="rotate(-90 26 535)">Heater power (%)</text>',
        line(245, 90, 275, 90, "sp"), text(282, 94, "SP"), line(345, 90, 375, 90, "pv"), text(382, 94, "PV"),
        line(220, 391, 250, 391, "request"), text(257, 395, "Requested heater power"),
        line(500, 391, 530, 391, "applied"), text(537, 395, "Applied heater power"),
        text(sx(67), sy_output(315), "Stored demand keeps the physical heater saturated", "annotation"),
        text(sx(67), sy_output(287), "for 12.7 s after the achievable SP returns", "annotation"),
    ]
    return f'''<svg xmlns="http://www.w3.org/2000/svg" width="900" height="720" viewBox="0 0 900 720" role="img">
<title>Water-heater requested and applied output during amplitude saturation</title>
<desc>Simulation evidence showing a temporarily unreachable temperature setpoint, the process response, a controller request above 400 percent, the physical heater clamped at 100 percent, and delayed release after setpoint restoration.</desc>
<style>
  text {{ font-family: Arial, sans-serif; font-size: 12px; }}
  .title {{ font-size: 22px; font-weight: 700; fill: {INK}; }} .subtitle {{ font-size: 13px; fill: {MUTED}; }}
  .axis {{ stroke: #8792a5; stroke-width: 1.4; }} .grid {{ stroke: {GRID}; stroke-width: 1; stroke-dasharray: 3 5; }}
  .event {{ stroke: #a51c30; stroke-width: 1.5; stroke-dasharray: 5 4; }} .limit {{ stroke: {ORANGE}; stroke-width: 1.5; stroke-dasharray: 7 4; }}
  .note {{ fill: {MUTED}; }} .label {{ font-size: 13px; font-weight: 700; fill: {INK}; }} .annotation {{ font-size: 12px; font-weight: 700; fill: {RED}; }}
  .sp {{ stroke: {INK}; stroke-width: 1.7; stroke-dasharray: 7 5; }} .pv {{ stroke: {BLUE}; stroke-width: 2.5; }}
  .request {{ stroke: {RED}; stroke-width: 2.3; }} .applied {{ stroke: {GREEN}; stroke-width: 2.5; }}
</style>
<rect width="900" height="720" rx="12" fill="{PAPER}" stroke="{GRID}" />
{text(48, 42, "The clamp protects the heater, but stored demand delays recovery", "title")}
{text(48, 65, "Simulation evidence · shipped ST · 100 ms samples · seed 20260828", "subtitle")}
{"".join(body)}
</svg>
'''


def main() -> None:
    (ROOT / "saturation-response.svg").write_text(render(), encoding="utf-8")


if __name__ == "__main__":
    main()
