#!/usr/bin/env python3
"""Render Lesson 6 from its retained executable comparison trace."""

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


def text(x: float, y: float, value: str, css: str = "note", anchor: str = "start") -> str:
    return f'<text x="{x:.1f}" y="{y:.1f}" class="{css}" text-anchor="{anchor}">{value}</text>'


def line(x1: float, y1: float, x2: float, y2: float, css: str) -> str:
    return f'<line x1="{x1:.1f}" y1="{y1:.1f}" x2="{x2:.1f}" y2="{y2:.1f}" class="{css}" />'


def polyline(points: list[tuple[float, float]], colour: str, width: float = 2.2, dash: str = "") -> str:
    encoded = " ".join(f"{x:.1f},{y:.1f}" for x, y in points)
    dashed = f' stroke-dasharray="{dash}"' if dash else ""
    return f'<polyline points="{encoded}" fill="none" stroke="{colour}" stroke-width="{width}"{dashed} />'


def comparison_figure() -> str:
    with (ROOT / "integral-comparison.csv").open(newline="", encoding="utf-8") as handle:
        data = list(csv.DictReader(handle))
    grouped: dict[str, list[dict[str, str]]] = {}
    for row in data:
        if row["phase"] != "safe_restoration" and float(row["time_s"]) >= 50.0:
            grouped.setdefault(row["case"], []).append(row)

    left, right = 86.0, 850.0
    top1, bottom1 = 115.0, 355.0
    top2, bottom2 = 425.0, 585.0
    sx = lambda value: left + (value - 50.0) / 70.0 * (right - left)
    sy_pv = lambda value: bottom1 - (value - 32.5) / 3.5 * (bottom1 - top1)
    sy_mv = lambda value: bottom2 - (value - 70.0) / 25.0 * (bottom2 - top2)
    body: list[str] = [
        f'<rect x="{sx(60):.1f}" y="{top1}" width="{right - sx(60):.1f}" height="{bottom2 - top1}" fill="#f6e8ea" />',
        line(sx(60), top1, sx(60), bottom2, "event"),
        text(sx(60) + 8, top1 + 18, "Cold-water load increases"),
    ]
    for value in [33, 34, 35, 36]:
        body += [line(left, sy_pv(value), right, sy_pv(value), "grid"), text(left - 12, sy_pv(value) + 4, str(value), anchor="end")]
    for value in [70, 75, 80, 85, 90, 95]:
        body += [line(left, sy_mv(value), right, sy_mv(value), "grid"), text(left - 12, sy_mv(value) + 4, str(value), anchor="end")]
    for value in [50, 60, 70, 80, 90, 100, 110, 120]:
        body += [line(sx(value), top1, sx(value), bottom2, "grid"), text(sx(value), bottom2 + 24, str(value), anchor="middle")]
    body += [
        line(left, top1, left, bottom1, "axis"), line(left, bottom1, right, bottom1, "axis"),
        line(left, top2, left, bottom2, "axis"), line(left, bottom2, right, bottom2, "axis"),
        polyline([(left, sy_pv(35.0)), (right, sy_pv(35.0))], INK, 1.4, "7 5"),
    ]
    baseline = grouped["biased_p"]
    integral = grouped["integral_only"]
    body += [
        polyline([(sx(float(row["time_s"])), sy_pv(float(row["pv_degC"]))) for row in baseline[::2]], GREY, 2.0),
        polyline([(sx(float(row["time_s"])), sy_pv(float(row["pv_degC"]))) for row in integral[::2]], BLUE),
        polyline([(sx(float(row["time_s"])), sy_mv(float(row["applied_mv_percent"]))) for row in baseline[::2]], GREY, 1.8, "6 4"),
        polyline([(sx(float(row["time_s"])), sy_mv(float(row["requested_mv_percent"]))) for row in integral[::2]], ORANGE),
        polyline([(sx(float(row["time_s"])), sy_mv(float(row["applied_mv_percent"]))) for row in integral[::2]], GREEN, 1.6, "5 3"),
        text(468, 635, "Time (s)", "label", "middle"),
        '<text x="24" y="238" class="label" text-anchor="middle" transform="rotate(-90 24 238)">Outlet temperature (°C)</text>',
        '<text x="24" y="510" class="label" text-anchor="middle" transform="rotate(-90 24 510)">Heater output (%)</text>',
        line(360, 88, 390, 88, "baseline"), text(397, 92, "Biased P"),
        line(475, 88, 505, 88, "integral"), text(512, 92, "Integral-only PV"),
        line(625, 88, 655, 88, "request"), text(662, 92, "I request"),
        line(755, 88, 785, 88, "applied"), text(792, 92, "I applied"),
    ]
    return f'''<svg xmlns="http://www.w3.org/2000/svg" width="900" height="660" viewBox="0 0 900 660" role="img">
<title>Integral-only and biased proportional water-heater response</title>
<desc>Simulation comparison showing prompt biased proportional response with offset and slower integral-only response that removes offset.</desc>
<style>
  text {{ font-family: Arial, sans-serif; font-size: 12px; }}
  .title {{ font-family: Arial, sans-serif; font-size: 22px; font-weight: 700; fill: {INK}; }}
  .subtitle {{ font-family: Arial, sans-serif; font-size: 13px; fill: {MUTED}; }}
  .axis {{ stroke: {GREY}; stroke-width: 1.4; }}
  .grid {{ stroke: {GRID}; stroke-width: 1; stroke-dasharray: 3 5; }}
  .event {{ stroke: #a51c30; stroke-width: 1.5; stroke-dasharray: 5 4; }}
  .note {{ font-family: Arial, sans-serif; font-size: 12px; fill: {MUTED}; }}
  .label {{ font-family: Arial, sans-serif; font-size: 13px; font-weight: 700; fill: {INK}; }}
  .baseline {{ stroke: {GREY}; stroke-width: 2; }}
  .integral {{ stroke: {BLUE}; stroke-width: 2.2; }}
  .request {{ stroke: {ORANGE}; stroke-width: 2.2; }}
  .applied {{ stroke: {GREEN}; stroke-width: 1.6; stroke-dasharray: 5 3; }}
</style>
<rect width="900" height="660" rx="12" fill="{PAPER}" stroke="{GRID}" />
{text(48, 42, "Integral memory removes the changed-load offset", "title")}
{text(48, 65, "Simulation evidence · shipped ST · 100 ms samples · seed 20260828", "subtitle")}
{"".join(body)}
</svg>
'''


def main() -> None:
    (ROOT / "integral-comparison.svg").write_text(comparison_figure(), encoding="utf-8")


if __name__ == "__main__":
    main()
