#!/usr/bin/env python3
"""Render Lesson 10 from the retained native four-strategy trace."""

from __future__ import annotations

import csv
from pathlib import Path


ROOT = Path(__file__).resolve().parent
INK = "#172033"
MUTED = "#526078"
GRID = "#cbd5e1"
PAPER = "#f8fafc"
COLOURS = {
    "none": "#b4232f",
    "signal_clamping": "#1268b3",
    "back_calculation": "#16803c",
    "combined": "#6f42c1",
}
LABELS = {
    "none": "No anti-windup",
    "signal_clamping": "Signal clamping",
    "back_calculation": "BackCalculation (selected)",
    "combined": "Combined",
}


def text(x: float, y: float, value: str, css: str = "note", anchor: str = "start") -> str:
    return f'<text x="{x:.1f}" y="{y:.1f}" class="{css}" text-anchor="{anchor}">{value}</text>'


def line(x1: float, y1: float, x2: float, y2: float, css: str) -> str:
    return f'<line x1="{x1:.1f}" y1="{y1:.1f}" x2="{x2:.1f}" y2="{y2:.1f}" class="{css}" />'


def polyline(points: list[tuple[float, float]], colour: str, width: float = 2.2, dash: str = "") -> str:
    encoded = " ".join(f"{x:.1f},{y:.1f}" for x, y in points)
    dashed = f' stroke-dasharray="{dash}"' if dash else ""
    return f'<polyline points="{encoded}" fill="none" stroke="{colour}" stroke-width="{width}"{dashed} />'


def render() -> str:
    with (ROOT / "anti-windup-trace.csv").open(newline="", encoding="utf-8") as handle:
        rows = list(csv.DictReader(handle))
    by_strategy = {
        strategy: [row for row in rows if row["strategy"] == strategy]
        for strategy in COLOURS
    }

    left, right = 92.0, 850.0
    pv_top, pv_bottom = 104.0, 234.0
    mv_top, mv_bottom = 294.0, 424.0
    gap_top, gap_bottom = 484.0, 614.0
    sx = lambda value: left + value / 50.1 * (right - left)
    sy_pv = lambda value: pv_bottom - (value + 0.5) / 5.5 * (pv_bottom - pv_top)
    sy_mv = lambda value: mv_bottom - (value + 4.0) / 8.0 * (mv_bottom - mv_top)
    sy_gap = lambda value: gap_bottom - value / 30.0 * (gap_bottom - gap_top)
    body: list[str] = []

    for value in [0, 1, 2, 3, 4, 5]:
        body += [line(left, sy_pv(value), right, sy_pv(value), "grid"), text(left - 12, sy_pv(value) + 4, str(value), anchor="end")]
    for value in [-3.5, 0, 3.5]:
        body += [line(left, sy_mv(value), right, sy_mv(value), "grid"), text(left - 12, sy_mv(value) + 4, f"{value:.1f}", anchor="end")]
    for value in [0, 10, 20, 30]:
        body += [line(left, sy_gap(value), right, sy_gap(value), "grid"), text(left - 12, sy_gap(value) + 4, str(value), anchor="end")]
    for value in [0, 1, 10, 20, 30, 40, 50]:
        for top, bottom in [(pv_top, pv_bottom), (mv_top, mv_bottom), (gap_top, gap_bottom)]:
            body.append(line(sx(value), top, sx(value), bottom, "grid"))
        body.append(text(sx(value), gap_bottom + 22, str(value), anchor="middle"))
    for event, label in [(1.0, "SP raised to 4.5 °C"), (20.0, "SP restored to 0 °C")]:
        for top, bottom in [(pv_top, pv_bottom), (mv_top, mv_bottom), (gap_top, gap_bottom)]:
            body.append(line(sx(event), top, sx(event), bottom, "event"))
        body.append(text(sx(event) + 5, pv_top + 17, label))

    for top, bottom in [(pv_top, pv_bottom), (mv_top, mv_bottom), (gap_top, gap_bottom)]:
        body += [line(left, top, left, bottom, "axis"), line(left, bottom, right, bottom, "axis")]

    baseline = by_strategy["none"]
    setpoint = [(sx(float(row["time_s"])), sy_pv(float(row["setpoint_degC"]))) for row in baseline[::2]]
    body.append(polyline(setpoint, INK, 1.7, "7 5"))
    for strategy, strategy_rows in by_strategy.items():
        colour = COLOURS[strategy]
        body.append(polyline([(sx(float(row["time_s"])), sy_pv(float(row["pv_degC"]))) for row in strategy_rows[::2]], colour))
        body.append(polyline([(sx(float(row["time_s"])), sy_mv(float(row["applied_mv_percent"]))) for row in strategy_rows[::2]], colour))
    body.append(polyline([(sx(float(row["time_s"])), sy_gap(max(0.0, float(row["requested_mv_percent"]) - float(row["applied_mv_percent"])))) for row in baseline[::2]], COLOURS["none"], 2.5))
    body += [line(left, sy_mv(3.5), right, sy_mv(3.5), "limit"), line(left, sy_mv(-3.5), right, sy_mv(-3.5), "limit")]

    legend_x = [100, 270, 450, 670]
    for x, strategy in zip(legend_x, COLOURS):
        body += [f'<line x1="{x}" y1="78" x2="{x + 25}" y2="78" stroke="{COLOURS[strategy]}" stroke-width="3" />', text(x + 32, 82, LABELS[strategy])]
    body += [
        text(470, 690, "Time after start (s)", "label", "middle"),
        '<text x="25" y="169" class="label" text-anchor="middle" transform="rotate(-90 25 169)">Temperature deviation (°C)</text>',
        '<text x="25" y="359" class="label" text-anchor="middle" transform="rotate(-90 25 359)">Applied valve trim (%)</text>',
        '<text x="25" y="549" class="label" text-anchor="middle" transform="rotate(-90 25 549)">Unavailable request (pp)</text>',
        text(110, gap_top + 20, "Unprotected baseline only: MV_requested - MV_applied", "annotation"),
    ]

    return f'''<svg xmlns="http://www.w3.org/2000/svg" width="900" height="760" viewBox="0 0 900 760" role="img" style="forced-color-adjust: none; color-scheme: light;">
<title>Four anti-windup strategies on the same steam-heated batch vessel</title>
<desc>Simulation evidence comparing temperature recovery, applied steam-valve trim, and the unprotected requested-applied gap for no anti-windup, signal clamping, BackCalculation, and Combined.</desc>
<style>
  text {{ font-family: Arial, sans-serif; font-size: 12px; }}
  .title {{ font-size: 22px; font-weight: 700; fill: {INK}; }} .subtitle {{ font-size: 13px; fill: {MUTED}; }}
  .axis {{ stroke: #8792a5; stroke-width: 1.4; }} .grid {{ stroke: {GRID}; stroke-width: 1; stroke-dasharray: 3 5; }}
  .event {{ stroke: #a51c30; stroke-width: 1.4; stroke-dasharray: 5 4; }} .limit {{ stroke: #d94801; stroke-width: 1.5; stroke-dasharray: 7 4; }}
  .note {{ fill: {MUTED}; }} .label {{ font-size: 13px; font-weight: 700; fill: {INK}; }} .annotation {{ font-size: 12px; font-weight: 700; fill: #b4232f; }}
</style>
<rect width="900" height="760" rx="12" fill="{PAPER}" stroke="{GRID}" />
{text(48, 32, "Anti-windup must be selected from equivalent recovery evidence", "title")}
{text(48, 53, "Simulation evidence · shipped PI · four fresh runs · 100 ms · seed 20260824", "subtitle")}
{"".join(body)}
</svg>
'''


def main() -> None:
    (ROOT / "anti-windup-response.svg").write_text(render(), encoding="utf-8")


if __name__ == "__main__":
    main()
