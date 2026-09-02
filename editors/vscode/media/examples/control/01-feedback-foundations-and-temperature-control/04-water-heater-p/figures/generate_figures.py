#!/usr/bin/env python3
"""Regenerate the lesson's comparison plot from retained simulator evidence."""

from __future__ import annotations

import csv
from pathlib import Path


ROOT = Path(__file__).resolve().parent
FONT = "system-ui, -apple-system, BlinkMacSystemFont, 'Segoe UI', sans-serif"
STYLE = f"""
  svg {{ forced-color-adjust: none; color-scheme: light; }}
  .surface {{ fill: #f8fafc; stroke: #cbd5e1; stroke-width: 2; }}
  text {{ font-family: {FONT}; fill: #172033; }}
  .title {{ font-size: 20px; font-weight: 650; }}
  .subtitle {{ font-size: 12px; fill: #4b5568; }}
  .axis {{ stroke: #526076; stroke-width: 1.2; }}
  .grid {{ stroke: #cbd3df; stroke-width: 0.8; stroke-dasharray: 3 5; }}
  .tick {{ font-size: 11px; fill: #465268; }}
  .label {{ font-size: 12px; font-weight: 600; }}
  .note {{ font-size: 11px; fill: #374258; }}
  .low {{ fill: none; stroke: #1769aa; stroke-width: 2.4; }}
  .high {{ fill: none; stroke: #c64b16; stroke-width: 2.4; stroke-dasharray: 8 4; }}
  .aggressive {{ fill: none; stroke: #7a3eb1; stroke-width: 2.0; stroke-dasharray: 2 3; }}
  .sp {{ fill: none; stroke: #526076; stroke-width: 1.5; stroke-dasharray: 2 4; }}
  .event {{ stroke: #9b3152; stroke-width: 1.8; stroke-dasharray: 5 4; }}
"""


def svg_document(title: str, description: str, body: str, height: int) -> str:
    return f"""<svg xmlns="http://www.w3.org/2000/svg" width="900" height="{height}" viewBox="0 0 900 {height}" role="img" aria-labelledby="title desc">
  <title id="title">{title}</title>
  <desc id="desc">{description}</desc>
  <style>{STYLE}</style>
  <rect x="1" y="1" width="898" height="{height - 2}" rx="8" class="surface" />
{body}
</svg>
"""


def line(x1: float, y1: float, x2: float, y2: float, css: str) -> str:
    return f'  <line x1="{x1:.2f}" y1="{y1:.2f}" x2="{x2:.2f}" y2="{y2:.2f}" class="{css}" />'


def text(x: float, y: float, value: str, css: str, anchor: str = "start") -> str:
    return f'  <text x="{x:.2f}" y="{y:.2f}" class="{css}" text-anchor="{anchor}">{value}</text>'


def path(points: list[tuple[float, float]], css: str) -> str:
    commands = " ".join(("M" if index == 0 else "L") + f"{x:.2f},{y:.2f}" for index, (x, y) in enumerate(points))
    return f'  <path d="{commands}" class="{css}" />'


def comparison_plot() -> None:
    with (ROOT / "p-gain-comparison.csv").open(newline="", encoding="utf-8") as handle:
        rows = list(csv.DictReader(handle))
    assert len(rows) == 1_200
    values = [{key: (value if key == "load_event" else float(value)) for key, value in row.items()} for row in rows]
    plotted = values[::5]
    if values[-1] not in plotted:
        plotted.append(values[-1])

    left, right = 82.0, 850.0
    top1, bottom1 = 82.0, 212.0
    top2, bottom2 = 270.0, 400.0
    top3, bottom3 = 458.0, 588.0
    sx = lambda value: left + value / 120.0 * (right - left)
    sy_pv = lambda value: bottom1 - (value - 15.0) / 22.0 * (bottom1 - top1)
    sy_requested = lambda value: bottom2 - (value + 100.0) / 750.0 * (bottom2 - top2)
    sy_applied = lambda value: bottom3 - value / 100.0 * (bottom3 - top3)
    elements = [
        text(52, 30, "What proportional gain changes — and what it cannot remove", "title"),
        text(52, 51, "Simulation evidence · same plant, initial state, load event, duration, sample time and seed", "subtitle"),
    ]
    for value in [15, 20, 25, 30, 35]:
        elements.extend([line(left, sy_pv(value), right, sy_pv(value), "grid"), text(left - 10, sy_pv(value) + 4, str(value), "tick", "end")])
    for value in [-100, 0, 100, 300, 500]:
        elements.extend([line(left, sy_requested(value), right, sy_requested(value), "grid"), text(left - 10, sy_requested(value) + 4, str(value), "tick", "end")])
    for value in [0, 25, 50, 75, 100]:
        elements.extend([line(left, sy_applied(value), right, sy_applied(value), "grid"), text(left - 10, sy_applied(value) + 4, str(value), "tick", "end")])
    for value in [0, 20, 40, 60, 80, 100, 120]:
        elements.extend([line(sx(value), top1, sx(value), bottom3, "grid"), text(sx(value), bottom3 + 20, str(value), "tick", "middle")])
    elements.extend([
        line(left, top1, left, bottom1, "axis"), line(left, bottom1, right, bottom1, "axis"),
        line(left, top2, left, bottom2, "axis"), line(left, bottom2, right, bottom2, "axis"),
        line(left, top3, left, bottom3, "axis"), line(left, bottom3, right, bottom3, "axis"),
        f'  <text x="20" y="147" class="label" text-anchor="middle" transform="rotate(-90 20 147)">Temperature (°C)</text>',
        f'  <text x="20" y="335" class="label" text-anchor="middle" transform="rotate(-90 20 335)">Requested power (%)</text>',
        f'  <text x="20" y="523" class="label" text-anchor="middle" transform="rotate(-90 20 523)">Applied power (%)</text>',
        text((left + right) / 2, 630, "Time (s)", "label", "middle"),
        path([(sx(row["time_s"]), sy_pv(row["setpoint_degC"])) for row in plotted], "sp"),
        path([(sx(row["time_s"]), sy_pv(row["low_kp_pv_degC"])) for row in plotted], "low"),
        path([(sx(row["time_s"]), sy_pv(row["high_kp_pv_degC"])) for row in plotted], "high"),
        path([(sx(row["time_s"]), sy_pv(row["aggressive_kp_pv_degC"])) for row in plotted], "aggressive"),
        path([(sx(row["time_s"]), sy_requested(row["low_kp_mv_requested_percent"])) for row in plotted], "low"),
        path([(sx(row["time_s"]), sy_requested(row["high_kp_mv_requested_percent"])) for row in plotted], "high"),
        path([(sx(row["time_s"]), sy_requested(row["aggressive_kp_mv_requested_percent"])) for row in plotted], "aggressive"),
        path([(sx(row["time_s"]), sy_applied(row["low_kp_mv_applied_percent"])) for row in plotted], "low"),
        path([(sx(row["time_s"]), sy_applied(row["high_kp_mv_applied_percent"])) for row in plotted], "high"),
        path([(sx(row["time_s"]), sy_applied(row["aggressive_kp_mv_applied_percent"])) for row in plotted], "aggressive"),
        line(sx(60), top1, sx(60), bottom3, "event"),
        text(sx(60) + 7, top1 + 16, "Cold-water load at 60 s", "note"),
        line(430, 67, 460, 67, "sp"), text(467, 71, "SP 35 °C", "note"),
        line(555, 67, 585, 67, "low"), text(592, 71, "Kp 2", "note"),
        line(650, 67, 680, 67, "high"), text(687, 71, "Kp 8", "note"),
        line(745, 67, 775, 67, "aggressive"), text(782, 71, "Kp 40", "note"),
        text(right - 4, sy_pv(values[-1]["low_kp_pv_degC"]) + 16, "Kp 2 retains 12.9 °C error", "note", "end"),
        text(right - 4, sy_pv(values[-1]["high_kp_pv_degC"]) - 8, "Kp 8 retains 6.9 °C error", "note", "end"),
        line(left, sy_requested(100), right, sy_requested(100), "sp"),
        text(right - 4, sy_requested(100) - 6, "Physical maximum 100%", "note", "end"),
        text(right - 4, sy_applied(100) + 15, "Applied output remains within 0–100%", "note", "end"),
        text(sx(76), sy_applied(14), "Reduced damping: repeated PV and MV movement", "note", "middle"),
    ])
    (ROOT / "p-gain-comparison.svg").write_text(
        svg_document(
            "Water-heater responses for three proportional gains",
            "Three actual simulator runs show process value, unclamped requested heater power, and clamped applied heater power. Gain eight reduces temperature offset compared with gain two, while aggressive gain forty causes repeated movement because plant delay reduces damping.",
            "\n".join(elements),
            660,
        ),
        encoding="utf-8",
    )


if __name__ == "__main__":
    comparison_plot()
