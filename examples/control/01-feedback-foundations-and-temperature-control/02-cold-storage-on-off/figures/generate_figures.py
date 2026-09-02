#!/usr/bin/env python3
"""Generate deterministic Lesson 2 teaching figures from native run evidence."""

from __future__ import annotations

import csv
from pathlib import Path

ROOT = Path(__file__).resolve().parent

STYLE = """
  svg { forced-color-adjust: none; color-scheme: light; }
  .surface { fill: #f8fafc; stroke: #cbd5e1; stroke-width: 2; }
  text { font-family: system-ui, -apple-system, BlinkMacSystemFont, 'Segoe UI', sans-serif; fill: #172033; }
  .title { font-size: 20px; font-weight: 650; }
  .subtitle { font-size: 12px; fill: #4b5568; }
  .axis { stroke: #526076; stroke-width: 1.2; }
  .grid { stroke: #cbd3df; stroke-width: 0.8; stroke-dasharray: 3 5; }
  .pv { stroke: #1769aa; stroke-width: 2.2; fill: none; }
  .sp { stroke: #526076; stroke-width: 1.7; fill: none; stroke-dasharray: 7 4; }
  .mv { stroke: #c64b16; stroke-width: 1.8; fill: none; }
  .event { stroke: #9b3152; stroke-width: 1.8; fill: none; stroke-dasharray: 5 4; }
  .door-band { fill: #f4d9e2; opacity: 0.42; }
  .tick { font-size: 11px; fill: #465268; }
  .label { font-size: 12px; font-weight: 600; }
  .note { font-size: 11px; fill: #374258; }
  .on { fill: #c64b16; }
  .off { fill: #526076; }
""".strip()


def read_csv(name: str) -> list[dict[str, str]]:
    with (ROOT / name).open(newline="", encoding="utf-8") as handle:
        return list(csv.DictReader(handle))


def path(points: list[tuple[float, float]]) -> str:
    return " ".join(
        ("M" if index == 0 else "L") + f"{x:.2f},{y:.2f}"
        for index, (x, y) in enumerate(points)
    )


def response_figure() -> None:
    rows = [row for row in read_csv("on-off-response.csv") if row["phase"] == "experiment"]
    if len(rows) != 1200:
        raise ValueError(f"expected 1200 response samples, found {len(rows)}")

    left, right = 82.0, 850.0
    top_a, bottom_a = 86.0, 274.0
    top_b, bottom_b = 350.0, 538.0
    width = right - left

    def x(time_s: float) -> float:
        return left + width * time_s / 120.0

    def y_pv(value: float) -> float:
        return bottom_a - (value - 1.5) * (bottom_a - top_a) / (4.2 - 1.5)

    def y_mv(value: float) -> float:
        return bottom_b - value * (bottom_b - top_b) / 100.0

    pv_points = [(x(float(row["time_s"])), y_pv(float(row["pv_degC"]))) for row in rows]
    mv_points = [(x(float(row["time_s"])), y_mv(float(row["mv_percent"]))) for row in rows]
    lines = [
        '<svg xmlns="http://www.w3.org/2000/svg" width="900" height="610" viewBox="0 0 900 610" role="img" aria-labelledby="title desc">',
        '  <title id="title">Cold-room response under single-threshold ON/OFF control</title>',
        '  <desc id="desc">Executable simulation evidence shows temperature held near two degrees during a door disturbance while the compressor command chatters between zero and one hundred percent.</desc>',
        f"  <style>\n{STYLE}\n  </style>",
        '  <rect x="1" y="1" width="898" height="608" rx="8" class="surface" />',
        '  <text x="48" y="31" class="title">Single threshold regulates temperature but chatters</text>',
        '  <text x="48" y="52" class="subtitle">Simulation evidence · shipped ST · 100 ms samples · seed 20260828</text>',
        f'  <rect x="{x(60):.1f}" y="{top_a:.1f}" width="{x(90)-x(60):.1f}" height="{bottom_a-top_a:.1f}" class="door-band" />',
    ]
    for value in [1.5, 2.0, 2.5, 3.0, 3.5, 4.0]:
        y = y_pv(value)
        lines.extend([
            f'  <line x1="{left}" y1="{y:.1f}" x2="{right}" y2="{y:.1f}" class="grid" />',
            f'  <text x="72" y="{y+4:.1f}" class="tick" text-anchor="end">{value:.1f}</text>',
        ])
    for value in [0, 50, 100]:
        y = y_mv(float(value))
        lines.extend([
            f'  <line x1="{left}" y1="{y:.1f}" x2="{right}" y2="{y:.1f}" class="grid" />',
            f'  <text x="72" y="{y+4:.1f}" class="tick" text-anchor="end">{value}</text>',
        ])
    for value in range(0, 121, 20):
        px = x(float(value))
        lines.extend([
            f'  <line x1="{px:.1f}" y1="{top_a}" x2="{px:.1f}" y2="{bottom_b}" class="grid" />',
            f'  <text x="{px:.1f}" y="558" class="tick" text-anchor="middle">{value}</text>',
        ])
    lines.extend([
        f'  <line x1="{left}" y1="{top_a}" x2="{left}" y2="{bottom_a}" class="axis" />',
        f'  <line x1="{left}" y1="{bottom_a}" x2="{right}" y2="{bottom_a}" class="axis" />',
        f'  <line x1="{left}" y1="{top_b}" x2="{left}" y2="{bottom_b}" class="axis" />',
        f'  <line x1="{left}" y1="{bottom_b}" x2="{right}" y2="{bottom_b}" class="axis" />',
        '  <text x="20" y="180" class="label" text-anchor="middle" transform="rotate(-90 20 180)">Cold-room temperature (°C)</text>',
        '  <text x="20" y="444" class="label" text-anchor="middle" transform="rotate(-90 20 444)">Compressor demand (%)</text>',
        '  <text x="466" y="580" class="label" text-anchor="middle">Time (s)</text>',
        f'  <path d="M{left:.2f},{y_pv(2.0):.2f} L{right:.2f},{y_pv(2.0):.2f}" class="sp" />',
        f'  <path d="{path(pv_points)}" class="pv" />',
        f'  <path d="{path(mv_points)}" class="mv" />',
        f'  <line x1="{x(60):.1f}" y1="{top_a}" x2="{x(60):.1f}" y2="{bottom_a}" class="event" />',
        f'  <text x="{x(60)+7:.1f}" y="266" class="note">Door opens</text>',
        f'  <line x1="{x(90):.1f}" y1="{top_a}" x2="{x(90):.1f}" y2="{bottom_a}" class="event" />',
        f'  <text x="{x(90)+7:.1f}" y="266" class="note">Door closes</text>',
        '  <line x1="585" y1="68" x2="615" y2="68" class="pv" />',
        '  <text x="622" y="72" class="note">PV</text>',
        '  <line x1="665" y1="68" x2="695" y2="68" class="sp" />',
        '  <text x="702" y="72" class="note">SP</text>',
        '  <line x1="745" y1="68" x2="775" y2="68" class="mv" />',
        '  <text x="782" y="72" class="note">MV</text>',
        '</svg>',
    ])
    (ROOT / "on-off-response.svg").write_text("\n".join(lines) + "\n", encoding="utf-8")


def chatter_figure() -> None:
    rows = read_csv("threshold-chatter.csv")
    if len(rows) != 8:
        raise ValueError(f"expected 8 threshold samples, found {len(rows)}")
    left, right = 90.0, 850.0
    top_a, bottom_a = 92.0, 244.0
    top_b, bottom_b = 292.0, 378.0

    def x(sample: float) -> float:
        return left + (sample - 1.0) * (right - left) / 7.0

    def y_pv(value: float) -> float:
        return bottom_a - (value - 1.90) * (bottom_a - top_a) / 0.20

    pv_points = [
        (x(float(row["sample"])), y_pv(float(row["measured_pv_degC"])))
        for row in rows
    ]
    lines = [
        '<svg xmlns="http://www.w3.org/2000/svg" width="900" height="430" viewBox="0 0 900 430" role="img" aria-labelledby="title desc">',
        '  <title id="title">Noisy threshold crossings cause compressor chatter</title>',
        '  <desc id="desc">Eight alternating temperature samples around a two degree threshold cause seven compressor output transitions.</desc>',
        f"  <style>\n{STYLE}\n  </style>",
        '  <rect x="1" y="1" width="898" height="428" rx="8" class="surface" />',
        '  <text x="48" y="34" class="title">Small threshold crossings reverse the compressor command</text>',
        '  <text x="48" y="55" class="subtitle">Simulation evidence · constant underlying temperature · declared sensor noise · seven transitions</text>',
        '  <line x1="430" y1="76" x2="458" y2="76" class="pv" />',
        '  <text x="466" y="80" class="note">Measured PV</text>',
        '  <line x1="570" y1="76" x2="598" y2="76" class="sp" />',
        '  <text x="606" y="80" class="note">Switching threshold</text>',
        '  <rect x="760" y="68" width="18" height="12" class="on" />',
        '  <text x="786" y="80" class="note">Applied MV</text>',
    ]
    for value in [1.90, 1.95, 2.00, 2.05, 2.10]:
        y = y_pv(value)
        lines.extend([
            f'  <line x1="{left}" y1="{y:.1f}" x2="{right}" y2="{y:.1f}" class="grid" />',
            f'  <text x="80" y="{y+4:.1f}" class="tick" text-anchor="end">{value:.2f}</text>',
        ])
    lines.extend([
        f'  <line x1="{left}" y1="{top_a}" x2="{left}" y2="{bottom_a}" class="axis" />',
        f'  <line x1="{left}" y1="{bottom_a}" x2="{right}" y2="{bottom_a}" class="axis" />',
        f'  <path d="{path(pv_points)}" class="pv" />',
        f'  <path d="M{left:.1f},{y_pv(2.0):.1f} L{right:.1f},{y_pv(2.0):.1f}" class="sp" />',
        '  <text x="20" y="168" class="label" text-anchor="middle" transform="rotate(-90 20 168)">Temperature (°C)</text>',
        f'  <line x1="{left}" y1="{top_b}" x2="{left}" y2="{bottom_b}" class="axis" />',
        f'  <line x1="{left}" y1="{bottom_b}" x2="{right}" y2="{bottom_b}" class="axis" />',
        '  <text x="20" y="335" class="label" text-anchor="middle" transform="rotate(-90 20 335)">MV (%)</text>',
    ])
    bar_width = 48.0
    for row in rows:
        sample = float(row["sample"])
        output = float(row["mv_percent"])
        px = x(sample)
        height = (bottom_b - top_b) * output / 100.0
        lines.append(
            f'  <rect x="{px-bar_width/2:.1f}" y="{bottom_b-height:.1f}" width="{bar_width:.1f}" height="{height:.1f}" class="on" />'
        )
        lines.append(f'  <text x="{px:.1f}" y="402" class="tick" text-anchor="middle">{int(sample)}</text>')
    lines.extend([
        '  <text x="470" y="420" class="label" text-anchor="middle">Recorded sample</text>',
        '</svg>',
    ])
    (ROOT / "threshold-chatter.svg").write_text("\n".join(lines) + "\n", encoding="utf-8")


if __name__ == "__main__":
    response_figure()
    chatter_figure()
