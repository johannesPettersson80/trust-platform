#!/usr/bin/env python3
"""Render Lesson 5 figures from retained executable CSV evidence."""

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
GREY = "#8792a5"
PAPER = "#f8fafc"


def rows(name: str) -> list[dict[str, str]]:
    with (ROOT / name).open(newline="", encoding="utf-8") as handle:
        return list(csv.DictReader(handle))


def text(x: float, y: float, value: str, css: str = "note", anchor: str = "start") -> str:
    return f'<text x="{x:.1f}" y="{y:.1f}" class="{css}" text-anchor="{anchor}">{value}</text>'


def line(x1: float, y1: float, x2: float, y2: float, css: str) -> str:
    return f'<line x1="{x1:.1f}" y1="{y1:.1f}" x2="{x2:.1f}" y2="{y2:.1f}" class="{css}" />'


def polyline(points: list[tuple[float, float]], css: str) -> str:
    encoded = " ".join(f"{x:.1f},{y:.1f}" for x, y in points)
    strokes = {
        "blue": (BLUE, 2.2, ""),
        "orange": (ORANGE, 2.2, ""),
        "green": (GREEN, 2.2, ""),
        "grey": (GREY, 1.7, ""),
        "setpoint": (INK, 1.4, ' stroke-dasharray="7 5"'),
    }
    stroke, width, extra = strokes[css]
    return (
        f'<polyline points="{encoded}" fill="none" stroke="{stroke}" '
        f'stroke-width="{width}"{extra} />'
    )


def shell(width: int, height: int, title: str, subtitle: str, body: str, styles: str) -> str:
    return f'''<svg xmlns="http://www.w3.org/2000/svg" width="{width}" height="{height}" viewBox="0 0 {width} {height}" role="img">
<title>{title}</title>
<desc>{subtitle}</desc>
<style>
  text {{ font-family: Arial, sans-serif; font-size: 12px; }}
  .title {{ font-family: Arial, sans-serif; font-size: 22px; font-weight: 700; fill: {INK}; }}
  .subtitle {{ font-family: Arial, sans-serif; font-size: 13px; fill: {MUTED}; }}
  .axis {{ stroke: {GREY}; stroke-width: 1.4; }}
  .grid {{ stroke: {GRID}; stroke-width: 1; stroke-dasharray: 3 5; }}
  .note {{ font-family: Arial, sans-serif; font-size: 12px; fill: {MUTED}; }}
  .label {{ font-family: Arial, sans-serif; font-size: 13px; font-weight: 700; fill: {INK}; }}
  .blue {{ fill: none; stroke: {BLUE}; stroke-width: 2.2; }}
  .orange {{ fill: none; stroke: {ORANGE}; stroke-width: 2.2; }}
  .green {{ fill: none; stroke: {GREEN}; stroke-width: 2.2; }}
  .grey {{ fill: none; stroke: {GREY}; stroke-width: 1.7; }}
  .setpoint {{ fill: none; stroke: {INK}; stroke-width: 1.4; stroke-dasharray: 7 5; }}
  {styles}
</style>
<rect width="{width}" height="{height}" rx="12" fill="{PAPER}" stroke="{GRID}" />
{text(48, 42, title, "title")}
{text(48, 65, subtitle, "subtitle")}
{body}
</svg>
'''


def holding_figure() -> str:
    data = rows("holding-output-trials.csv")
    grouped: dict[str, list[dict[str, str]]] = {}
    for row in data:
        grouped.setdefault(row["trial"], []).append(row)
    left, right, top, bottom = 86.0, 850.0, 105.0, 410.0
    sx = lambda value: left + value / 60.0 * (right - left)
    sy = lambda value: bottom - (value - 18.0) / 20.0 * (bottom - top)
    body: list[str] = []
    body.append(
        f'<rect x="{sx(50):.1f}" y="{top}" width="{right - sx(50):.1f}" '
        f'height="{bottom - top:.1f}" fill="#e8f3ec" />'
    )
    body.append(text(sx(50) + 8, top + 18, "50–60 s evaluation window"))
    for value in [20, 25, 30, 34, 35, 36, 37]:
        body += [line(left, sy(value), right, sy(value), "grid"), text(left - 12, sy(value) + 4, f"{value}", anchor="end")]
    for value in [0, 10, 20, 30, 40, 50, 60]:
        body += [line(sx(value), top, sx(value), bottom, "grid"), text(sx(value), bottom + 24, str(value), anchor="middle")]
    body += [line(left, top, left, bottom, "axis"), line(left, bottom, right, bottom, "axis")]
    body += [polyline([(left, sy(35.0)), (right, sy(35.0))], "setpoint")]
    for trial, css in [("manual_70_percent", "blue"), ("manual_75_percent", "green"), ("manual_80_percent", "orange")]:
        trace = grouped[trial]
        points = [(sx(float(row["time_s"])), sy(float(row["pv_degC"]))) for row in trace[::3]]
        body.append(polyline(points, css))
    body += [
        text(468, 470, "Time (s)", "label", "middle"),
        f'<text x="24" y="258" class="label" text-anchor="middle" transform="rotate(-90 24 258)">Outlet temperature (°C)</text>',
        line(535, 86, 565, 86, "blue"), text(572, 90, "70% trial"),
        line(640, 86, 670, 86, "green"), text(677, 90, "75% trial"),
        line(745, 86, 775, 86, "orange"), text(782, 90, "80% trial"),
        line(385, 86, 415, 86, "setpoint"), text(422, 90, "35 °C SP"),
    ]
    return shell(
        900,
        500,
        "Manual trials measure the nominal holding output",
        "Simulation evidence · shipped ST · 100 ms samples · identical nominal plant",
        "\n".join(body),
        "",
    )


def comparison_figure() -> str:
    data = rows("bias-comparison.csv")
    grouped: dict[str, list[dict[str, str]]] = {}
    for row in data:
        if row["auto_mode"] == "true":
            grouped.setdefault(row["case"], []).append(row)
    left, right = 86.0, 850.0
    top1, bottom1 = 115.0, 355.0
    top2, bottom2 = 425.0, 585.0
    sx = lambda value: left + value / 120.0 * (right - left)
    sy_pv = lambda value: bottom1 - (value - 18.0) / 18.0 * (bottom1 - top1)
    sy_mv = lambda value: bottom2 - value / 210.0 * (bottom2 - top2)
    body: list[str] = [
        f'<rect x="{sx(60):.1f}" y="{top1}" width="{right - sx(60):.1f}" height="{bottom2 - top1}" fill="#f6e8ea" />',
        line(sx(60), top1, sx(60), bottom2, "event"),
        text(sx(60) + 8, top1 + 18, "Cold-water load increases"),
    ]
    for value in [20, 25, 30, 35]:
        body += [line(left, sy_pv(value), right, sy_pv(value), "grid"), text(left - 12, sy_pv(value) + 4, str(value), anchor="end")]
    for value in [0, 50, 100, 150, 200]:
        body += [line(left, sy_mv(value), right, sy_mv(value), "grid"), text(left - 12, sy_mv(value) + 4, str(value), anchor="end")]
    for value in [0, 20, 40, 60, 80, 100, 120]:
        body += [line(sx(value), top1, sx(value), bottom2, "grid"), text(sx(value), bottom2 + 24, str(value), anchor="middle")]
    body += [line(left, top1, left, bottom1, "axis"), line(left, bottom1, right, bottom1, "axis")]
    body += [line(left, top2, left, bottom2, "axis"), line(left, bottom2, right, bottom2, "axis")]
    body += [polyline([(left, sy_pv(35.0)), (right, sy_pv(35.0))], "setpoint")]
    zero = grouped["zero_bias"]
    biased = grouped["measured_bias"]
    body += [
        polyline([(sx(float(row["time_s"])), sy_pv(float(row["pv_degC"]))) for row in zero[::3]], "grey"),
        polyline([(sx(float(row["time_s"])), sy_pv(float(row["pv_degC"]))) for row in biased[::3]], "blue"),
        polyline([(sx(float(row["time_s"])), sy_mv(float(row["requested_mv_percent"]))) for row in biased[::3]], "orange"),
        polyline([(sx(float(row["time_s"])), sy_mv(float(row["applied_mv_percent"]))) for row in biased[::3]], "green"),
        text(468, 635, "Time (s)", "label", "middle"),
        f'<text x="24" y="238" class="label" text-anchor="middle" transform="rotate(-90 24 238)">Outlet temperature (°C)</text>',
        f'<text x="24" y="510" class="label" text-anchor="middle" transform="rotate(-90 24 510)">Heater output (%)</text>',
        line(455, 88, 485, 88, "grey"), text(492, 92, "Zero bias PV"),
        line(580, 88, 610, 88, "blue"), text(617, 92, "75% bias PV"),
        line(706, 88, 736, 88, "orange"), text(743, 92, "Requested MV"),
        line(706, 104, 736, 104, "green"), text(743, 108, "Applied MV"),
    ]
    return shell(
        900,
        660,
        "Measured bias removes nominal offset, not changed-load offset",
        "Simulation evidence · shipped ST · Kp 8 %/°C · 100 ms samples · seed 20260828",
        "\n".join(body),
        ".event { stroke: #a51c30; stroke-width: 1.5; stroke-dasharray: 5 4; }",
    )


def main() -> None:
    (ROOT / "holding-output-trials.svg").write_text(holding_figure(), encoding="utf-8")
    (ROOT / "bias-comparison.svg").write_text(comparison_figure(), encoding="utf-8")


if __name__ == "__main__":
    main()
