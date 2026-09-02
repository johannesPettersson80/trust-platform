#!/usr/bin/env python3
"""Render Lesson 7 from retained native PI comparison evidence."""

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
    with (ROOT / "pi-comparison.csv").open(newline="", encoding="utf-8") as handle:
        data = list(csv.DictReader(handle))
    with (ROOT / "qualification-summary.csv").open(newline="", encoding="utf-8") as handle:
        summaries = {row["case"]: row for row in csv.DictReader(handle)}
    grouped: dict[str, list[dict[str, str]]] = {}
    for row in data:
        if row["phase"] != "safe_restoration" and float(row["time_s"]) >= 50.0:
            grouped.setdefault(row["case"], []).append(row)

    left, right = 92.0, 850.0
    top, bottom = 125.0, 385.0
    sx = lambda value: left + (value - 50.0) / 70.0 * (right - left)
    sy = lambda value: bottom - (value - 32.5) / 3.5 * (bottom - top)
    body: list[str] = [
        f'<rect x="{sx(60):.1f}" y="{top}" width="{right - sx(60):.1f}" height="{bottom - top}" fill="#f6e8ea" />',
        line(sx(60), top, sx(60), bottom, "event"),
        text(sx(60) + 8, top + 18, "Cold-water load increases"),
    ]
    for value in [33, 34, 35, 36]:
        body += [line(left, sy(value), right, sy(value), "grid"), text(left - 12, sy(value) + 4, str(value), anchor="end")]
    for value in [50, 60, 70, 80, 90, 100, 110, 120]:
        body += [line(sx(value), top, sx(value), bottom, "grid"), text(sx(value), bottom + 24, str(value), anchor="middle")]
    body += [
        line(left, top, left, bottom, "axis"),
        line(left, bottom, right, bottom, "axis"),
        polyline([(left, sy(35.0)), (right, sy(35.0))], INK, 1.4, "7 5"),
        polyline([(sx(float(row["time_s"])), sy(float(row["pv_degC"]))) for row in grouped["biased_p"][::2]], GREY, 2.0),
        polyline([(sx(float(row["time_s"])), sy(float(row["pv_degC"]))) for row in grouped["integral_only"][::2]], BLUE, 2.0),
        polyline([(sx(float(row["time_s"])), sy(float(row["pv_degC"]))) for row in grouped["pi_ki_4"][::2]], GREEN, 2.4),
        text(470, 427, "Time after start (s)", "label", "middle"),
        '<text x="26" y="255" class="label" text-anchor="middle" transform="rotate(-90 26 255)">Outlet temperature (°C)</text>',
        line(355, 94, 385, 94, "baseline"), text(392, 98, "Biased P"),
        line(470, 94, 500, 94, "integral"), text(507, 98, "Integral-only"),
        line(610, 94, 640, 94, "selected"), text(647, 98, "Selected PI, Ki 4"),
    ]

    candidates = ["pi_ki_0_5", "pi_ki_2", "pi_ki_4", "pi_ki_8"]
    labels = ["0.5", "2", "4", "8"]
    x_positions = [210.0, 390.0, 570.0, 750.0]
    recovery_top, recovery_bottom = 485.0, 575.0
    movement_top, movement_bottom = 625.0, 715.0
    recovery_y = lambda value: recovery_bottom - value / 50.0 * (recovery_bottom - recovery_top)
    movement_y = lambda value: movement_bottom - value / 70.0 * (movement_bottom - movement_top)
    for value in [0, 15, 30, 45]:
        body += [line(left, recovery_y(value), right, recovery_y(value), "grid"), text(left - 12, recovery_y(value) + 4, str(value), anchor="end")]
    for value in [0, 20, 40, 60]:
        body += [line(left, movement_y(value), right, movement_y(value), "grid"), text(left - 12, movement_y(value) + 4, str(value), anchor="end")]
    body += [
        line(left, recovery_top, left, recovery_bottom, "axis"), line(left, recovery_bottom, right, recovery_bottom, "axis"),
        line(left, movement_top, left, movement_bottom, "axis"), line(left, movement_bottom, right, movement_bottom, "axis"),
        '<text x="26" y="530" class="label" text-anchor="middle" transform="rotate(-90 26 530)">Recovery (s)</text>',
        '<text x="26" y="670" class="label" text-anchor="middle" transform="rotate(-90 26 670)">MV movement (pp)</text>',
    ]
    for case, label, x in zip(candidates, labels, x_positions):
        row = summaries[case]
        recovery = float(row["recovery_time_s"])
        movement = float(row["cumulative_applied_movement_pp"])
        selected = row["selected"] == "true"
        eligible = row["eligible"] == "true"
        colour = GREEN if selected else BLUE if eligible else RED
        body += [
            f'<circle cx="{x:.1f}" cy="{recovery_y(recovery):.1f}" r="7" fill="{colour}" />',
            text(x, recovery_y(recovery) - 12, f"{recovery:.1f} s", "value", "middle"),
            f'<rect x="{x - 7:.1f}" y="{movement_y(movement) - 7:.1f}" width="14" height="14" fill="{colour}" />',
            text(x, movement_y(movement) - 13, f"{movement:.1f} pp", "value", "middle"),
            text(x, 747, f"Ki {label}", "label", "middle"),
            text(x, 766, "selected" if selected else "eligible" if eligible else "rejected", "status", "middle"),
        ]

    return f'''<svg xmlns="http://www.w3.org/2000/svg" width="900" height="790" viewBox="0 0 900 790" role="img">
<title>Water-heater PI response and integral-gain tuning trade-off</title>
<desc>Simulation evidence comparing biased proportional, integral-only, and selected PI temperature responses, followed by recovery time and actuator movement for four integral-gain candidates.</desc>
<style>
  text {{ font-family: Arial, sans-serif; font-size: 12px; }}
  .title {{ font-family: Arial, sans-serif; font-size: 22px; font-weight: 700; fill: {INK}; }}
  .subtitle {{ font-family: Arial, sans-serif; font-size: 13px; fill: {MUTED}; }}
  .axis {{ stroke: {GREY}; stroke-width: 1.4; }}
  .grid {{ stroke: {GRID}; stroke-width: 1; stroke-dasharray: 3 5; }}
  .event {{ stroke: #a51c30; stroke-width: 1.5; stroke-dasharray: 5 4; }}
  .note {{ font-family: Arial, sans-serif; font-size: 12px; fill: {MUTED}; }}
  .label {{ font-family: Arial, sans-serif; font-size: 13px; font-weight: 700; fill: {INK}; }}
  .value {{ font-family: Arial, sans-serif; font-size: 12px; font-weight: 700; fill: {INK}; }}
  .status {{ font-family: Arial, sans-serif; font-size: 11px; fill: {MUTED}; }}
  .baseline {{ stroke: {GREY}; stroke-width: 2; }}
  .integral {{ stroke: {BLUE}; stroke-width: 2; }}
  .selected {{ stroke: {GREEN}; stroke-width: 2.4; }}
</style>
<rect width="900" height="790" rx="12" fill="{PAPER}" stroke="{GRID}" />
{text(48, 42, "PI combines prompt correction with stored action", "title")}
{text(48, 65, "Simulation evidence · shipped ST · 100 ms samples · seed 20260828", "subtitle")}
{text(48, 465, "One-factor tuning: Kp remains 8 while only Ki changes", "label")}
{"".join(body)}
</svg>
'''


def main() -> None:
    (ROOT / "pi-comparison.svg").write_text(comparison_figure(), encoding="utf-8")


if __name__ == "__main__":
    main()
