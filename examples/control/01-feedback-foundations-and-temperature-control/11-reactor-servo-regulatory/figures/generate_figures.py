#!/usr/bin/env python3
"""Render Lesson 11 directly from its retained native execution trace."""

from __future__ import annotations

import csv
from pathlib import Path


ROOT = Path(__file__).resolve().parent
PAPER = "#f8fafc"
INK = "#172033"
MUTED = "#526078"
GRID = "#cbd5e1"
PV = "#1268b3"
SP = "#172033"
MV = "#16803c"
EVENT = "#b4232f"


def text(x: float, y: float, value: str, css: str = "note", anchor: str = "start") -> str:
    return f'<text x="{x:.1f}" y="{y:.1f}" class="{css}" text-anchor="{anchor}">{value}</text>'


def line(x1: float, y1: float, x2: float, y2: float, css: str) -> str:
    return f'<line x1="{x1:.1f}" y1="{y1:.1f}" x2="{x2:.1f}" y2="{y2:.1f}" class="{css}" />'


def polyline(points: list[tuple[float, float]], colour: str, dash: str = "") -> str:
    encoded = " ".join(f"{x:.1f},{y:.1f}" for x, y in points)
    dashed = f' stroke-dasharray="{dash}"' if dash else ""
    return f'<polyline points="{encoded}" fill="none" stroke="{colour}" stroke-width="2.4"{dashed} />'


def panel(
    rows: list[dict[str, str]],
    x0: float,
    x1: float,
    y0: float,
    y1: float,
    event_time: float,
    duration: float,
    field: str,
    y_min: float,
    y_max: float,
    y_ticks: list[float],
    title_value: str,
    y_label: str,
    show_setpoint: bool,
) -> list[str]:
    sx = lambda time_s: x0 + ((time_s - event_time) / duration) * (x1 - x0)
    sy = lambda value: y1 - ((value - y_min) / (y_max - y_min)) * (y1 - y0)
    body = [text(x0, y0 - 13, title_value, "panel-title")]
    for value in y_ticks:
        body.extend([line(x0, sy(value), x1, sy(value), "grid"), text(x0 - 9, sy(value) + 4, f"{value:g}", anchor="end")])
    for elapsed in range(0, int(duration) + 1, 10):
        x = sx(event_time + elapsed)
        body.extend([line(x, y0, x, y1, "grid"), text(x, y1 + 19, str(elapsed), anchor="middle")])
    body.extend([line(x0, y0, x0, y1, "axis"), line(x0, y1, x1, y1, "axis")])
    body.append(line(sx(event_time), y0, sx(event_time), y1, "event"))
    sampled = rows[::2]
    body.append(polyline([(sx(float(row["time_s"])), sy(float(row[field]))) for row in sampled], PV if field == "pv_degC" else MV))
    if show_setpoint:
        body.append(polyline([(sx(float(row["time_s"])), sy(float(row["setpoint_degC"]))) for row in sampled], SP, "7 5"))
    body.append(text((x0 + x1) / 2, y1 + 38, "Time after event (s)", "label", "middle"))
    body.append(f'<text x="{x0 - 47:.1f}" y="{(y0 + y1) / 2:.1f}" class="label" text-anchor="middle" transform="rotate(-90 {x0 - 47:.1f} {(y0 + y1) / 2:.1f})">{y_label}</text>')
    return body


def render() -> str:
    with (ROOT / "servo-regulatory-trace.csv").open(newline="", encoding="utf-8") as handle:
        all_rows = list(csv.DictReader(handle))
    servo = [row for row in all_rows if row["objective"] == "servo" and row["auto_mode"] == "true" and 5.0 <= float(row["time_s"]) <= 55.0]
    regulatory = [row for row in all_rows if row["objective"] == "regulatory" and row["auto_mode"] == "true" and 60.0 <= float(row["time_s"]) <= 120.0]
    body: list[str] = []
    body += panel(servo, 82, 422, 118, 294, 5.0, 50.0, "pv_degC", 79.0, 87.0, [80, 82, 84, 86], "Servo: setpoint tracking", "Temperature (°C)", True)
    body += panel(servo, 522, 862, 118, 294, 5.0, 50.0, "applied_mv_percent", 40.0, 100.0, [40, 60, 80, 100], "Servo: applied valve", "Valve opening (%)", False)
    body += panel(regulatory, 82, 422, 426, 602, 60.0, 60.0, "pv_degC", 78.0, 81.0, [78, 79, 80, 81], "Regulatory: disturbance rejection", "Temperature (°C)", True)
    body += panel(regulatory, 522, 862, 426, 602, 60.0, 60.0, "applied_mv_percent", 45.0, 80.0, [45, 55, 65, 75], "Regulatory: applied valve", "Valve opening (%)", False)
    body += [
        line(92, 78, 117, 78, "sp-key"), text(124, 82, "Setpoint (SP)"),
        f'<line x1="260" y1="78" x2="285" y2="78" stroke="{PV}" stroke-width="3" />', text(292, 82, "Process value (PV)"),
        f'<line x1="450" y1="78" x2="475" y2="78" stroke="{MV}" stroke-width="3" />', text(482, 82, "Applied valve (MV_applied)"),
        line(700, 68, 700, 88, "event"), text(710, 82, "Setpoint/load event"),
        text(450, 684, "The same PI fails servo overshoot but passes regulatory recovery", "conclusion", "middle"),
        text(450, 711, "Simulation evidence · two fresh runs · 100 ms · seed 20260827", "subtitle", "middle"),
    ]
    return f'''<svg xmlns="http://www.w3.org/2000/svg" width="900" height="760" viewBox="0 0 900 760" role="img" style="forced-color-adjust: none; color-scheme: light;">
<title>Servo and regulatory evidence for one unchanged reactor PI controller</title>
<desc>Four panels compare reactor temperature and applied valve opening for a setpoint step and a separate cold-feed-equivalent disturbance.</desc>
<style>
  text {{ font-family: Arial, sans-serif; font-size: 12px; }}
  .title {{ font-size: 22px; font-weight: 700; fill: {INK}; }} .subtitle {{ font-size: 13px; fill: {MUTED}; }}
  .panel-title {{ font-size: 14px; font-weight: 700; fill: {INK}; }} .label {{ font-size: 12px; font-weight: 700; fill: {INK}; }}
  .note {{ fill: {MUTED}; }} .conclusion {{ font-size: 16px; font-weight: 700; fill: {INK}; }}
  .axis {{ stroke: #8792a5; stroke-width: 1.4; }} .grid {{ stroke: {GRID}; stroke-width: 1; stroke-dasharray: 3 5; }}
  .event {{ stroke: {EVENT}; stroke-width: 1.5; stroke-dasharray: 5 4; }}
  .sp-key {{ stroke: {SP}; stroke-width: 2.4; stroke-dasharray: 7 5; }}
</style>
<rect width="900" height="760" rx="12" fill="{PAPER}" stroke="{GRID}" />
{text(48, 34, "One tuning must be judged against two different objectives", "title")}
{text(48, 55, "Setpoint tracking and disturbance rejection use separate runs, windows, and requirements", "subtitle")}
{"".join(body)}
</svg>
'''


def main() -> None:
    (ROOT / "servo-regulatory-response.svg").write_text(render(), encoding="utf-8")


if __name__ == "__main__":
    main()
