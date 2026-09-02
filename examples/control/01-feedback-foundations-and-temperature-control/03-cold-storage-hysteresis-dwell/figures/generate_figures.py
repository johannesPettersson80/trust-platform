#!/usr/bin/env python3
"""Generate deterministic Lesson 3 teaching figures from native run evidence."""

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
  .single { stroke: #7c8799; stroke-width: 1.5; fill: none; }
  .hysteresis { stroke: #1769aa; stroke-width: 2.0; fill: none; }
  .protected { stroke: #c64b16; stroke-width: 2.0; fill: none; }
  .requested { stroke: #1769aa; stroke-width: 2.2; fill: none; }
  .applied { stroke: #c64b16; stroke-width: 2.2; fill: none; }
  .inhibit { fill: #9b3152; opacity: 0.18; }
  .event { stroke: #9b3152; stroke-width: 1.5; stroke-dasharray: 5 4; }
  .door-band { fill: #f4d9e2; opacity: 0.42; }
  .tick { font-size: 11px; fill: #465268; }
  .label { font-size: 12px; font-weight: 600; }
  .note { font-size: 11px; fill: #374258; }
""".strip()


def read_csv(name: str) -> list[dict[str, str]]:
    with (ROOT / name).open(newline="", encoding="utf-8") as handle:
        return list(csv.DictReader(handle))


def svg_path(points: list[tuple[float, float]]) -> str:
    return " ".join(
        ("M" if index == 0 else "L") + f"{x:.2f},{y:.2f}"
        for index, (x, y) in enumerate(points)
    )


def comparison_figure() -> None:
    rows = read_csv("hysteresis-dwell-comparison.csv")
    cases = {
        name: [row for row in rows if row["case"] == name]
        for name in ("single_threshold", "hysteresis", "hysteresis_dwell")
    }
    if any(len(case) != 1200 for case in cases.values()):
        raise ValueError("expected 1,200 samples for each comparison candidate")

    left, right = 92.0, 850.0
    top_pv, bottom_pv = 96.0, 330.0
    top_mv, bottom_mv = 400.0, 642.0
    width = right - left

    def x(time_s: float) -> float:
        return left + width * time_s / 120.0

    def y_pv(value: float) -> float:
        return bottom_pv - (value - 1.25) * (bottom_pv - top_pv) / (4.25 - 1.25)

    lane_centres = {"single_threshold": 438.0, "hysteresis": 520.0, "hysteresis_dwell": 602.0}

    def y_state(case: str, applied: bool) -> float:
        return lane_centres[case] - (22.0 if applied else -22.0)

    lines = [
        '<svg xmlns="http://www.w3.org/2000/svg" width="900" height="700" viewBox="0 0 900 700" role="img" aria-labelledby="title desc">',
        '  <title id="title">Cold-room comparison of a single threshold, hysteresis, and dwell protection</title>',
        '  <desc id="desc">Executable simulation evidence compares cold-room temperature and applied compressor state for three equivalent 120-second runs.</desc>',
        f"  <style>\n{STYLE}\n  </style>",
        '  <rect x="1" y="1" width="898" height="698" rx="8" class="surface" />',
        '  <text x="48" y="32" class="title">Hysteresis removes chatter; dwell makes postponed reversals visible</text>',
        '  <text x="48" y="54" class="subtitle">Simulation evidence · shipped ST · 100 ms samples · seed 20260828 · equivalent runs</text>',
        f'  <rect x="{x(60):.1f}" y="{top_pv}" width="{x(90)-x(60):.1f}" height="{bottom_mv-top_pv}" class="door-band" />',
    ]
    for value in (1.5, 2.0, 2.5, 3.0, 3.5, 4.0):
        y = y_pv(value)
        lines.extend([
            f'  <line x1="{left}" y1="{y:.1f}" x2="{right}" y2="{y:.1f}" class="grid" />',
            f'  <text x="82" y="{y+4:.1f}" class="tick" text-anchor="end">{value:.1f}</text>',
        ])
    for value in range(0, 121, 20):
        px = x(float(value))
        lines.extend([
            f'  <line x1="{px:.1f}" y1="{top_pv}" x2="{px:.1f}" y2="{bottom_mv}" class="grid" />',
            f'  <text x="{px:.1f}" y="666" class="tick" text-anchor="middle">{value}</text>',
        ])
    lines.extend([
        f'  <line x1="{left}" y1="{top_pv}" x2="{left}" y2="{bottom_pv}" class="axis" />',
        f'  <line x1="{left}" y1="{bottom_pv}" x2="{right}" y2="{bottom_pv}" class="axis" />',
        f'  <line x1="{left}" y1="{top_mv}" x2="{left}" y2="{bottom_mv}" class="axis" />',
        f'  <line x1="{left}" y1="{bottom_mv}" x2="{right}" y2="{bottom_mv}" class="axis" />',
        '  <text x="22" y="213" class="label" text-anchor="middle" transform="rotate(-90 22 213)">Cold-room temperature (°C)</text>',
        '  <text x="22" y="521" class="label" text-anchor="middle" transform="rotate(-90 22 521)">Applied compressor state</text>',
        '  <text x="471" y="687" class="label" text-anchor="middle">Time (s)</text>',
        f'  <line x1="{x(60):.1f}" y1="{top_pv}" x2="{x(60):.1f}" y2="{bottom_mv}" class="event" />',
        f'  <text x="{x(60)+7:.1f}" y="322" class="note">Door opens</text>',
        f'  <line x1="{x(90):.1f}" y1="{top_pv}" x2="{x(90):.1f}" y2="{bottom_mv}" class="event" />',
        f'  <text x="{x(90)+7:.1f}" y="322" class="note">Door closes</text>',
    ])
    labels = {
        "single_threshold": ("Single threshold · 760 transitions", "single"),
        "hysteresis": ("Hysteresis · 12 transitions", "hysteresis"),
        "hysteresis_dwell": ("Hysteresis + dwell · 12 transitions", "protected"),
    }
    for case, (label, css_class) in labels.items():
        trace = cases[case]
        pv_points = [(x(float(row["time_s"])), y_pv(float(row["pv_degC"]))) for row in trace]
        state_points = [(x(float(row["time_s"])), y_state(case, row["applied_cooling"] == "1")) for row in trace]
        lines.extend([
            f'  <path d="{svg_path(pv_points)}" class="{css_class}" />',
            f'  <path d="{svg_path(state_points)}" class="{css_class}" />',
            f'  <text x="98" y="{lane_centres[case]-29:.1f}" class="note">{label}</text>',
        ])
    lines.extend([
        '  <line x1="530" y1="76" x2="560" y2="76" class="single" />',
        '  <text x="567" y="80" class="note">Single threshold</text>',
        '  <line x1="666" y1="76" x2="696" y2="76" class="hysteresis" />',
        '  <text x="703" y="80" class="note">Hysteresis</text>',
        '  <line x1="782" y1="76" x2="812" y2="76" class="protected" />',
        '  <text x="819" y="80" class="note">Dwell</text>',
        '</svg>',
    ])
    (ROOT / "hysteresis-dwell-comparison.svg").write_text("\n".join(lines) + "\n", encoding="utf-8")


def memory_figure() -> None:
    rows = read_csv("threshold-memory.csv")
    if len(rows) != 4:
        raise ValueError(f"expected four state-memory samples, found {len(rows)}")
    left, right = 110.0, 830.0
    sample_x = [left + index * (right - left) / 3.0 for index in range(4)]

    lines = [
        '<svg xmlns="http://www.w3.org/2000/svg" width="900" height="450" viewBox="0 0 900 450" role="img" aria-labelledby="title desc">',
        '  <title id="title">Hysteresis state memory and delayed dwell transition</title>',
        '  <desc id="desc">Four declared temperature samples show cooling retained inside the hysteresis band and an off request postponed by minimum on-time.</desc>',
        f"  <style>\n{STYLE}\n  </style>",
        '  <rect x="1" y="1" width="898" height="448" rx="8" class="surface" />',
        '  <text x="48" y="32" class="title">Request changes at the threshold; applied state waits for dwell</text>',
        '  <text x="48" y="54" class="subtitle">Simulation evidence · deterministic inputs · requested and applied states retained separately</text>',
        '  <text x="92" y="112" class="label" text-anchor="end">PV (°C)</text>',
        '  <text x="92" y="212" class="label" text-anchor="end">Requested</text>',
        '  <text x="92" y="292" class="label" text-anchor="end">Applied</text>',
        '  <text x="92" y="372" class="label" text-anchor="end">Inhibit</text>',
    ]
    for index, row in enumerate(rows):
        px = sample_x[index]
        requested = row["requested_cooling"] == "1"
        applied = row["applied_cooling"] == "1"
        inhibited = row["dwell_inhibit"] == "1"
        if inhibited:
            lines.append(f'  <rect x="{px-70:.1f}" y="76" width="140" height="316" class="inhibit" />')
        lines.extend([
            f'  <line x1="{px:.1f}" y1="80" x2="{px:.1f}" y2="392" class="grid" />',
            f'  <text x="{px:.1f}" y="113" class="note" text-anchor="middle">{float(row["pv_degC"]):.2f}</text>',
            f'  <circle cx="{px:.1f}" cy="212" r="9" class="{("requested" if requested else "single")}" />',
            f'  <text x="{px:.1f}" y="238" class="note" text-anchor="middle">{("on" if requested else "off")}</text>',
            f'  <circle cx="{px:.1f}" cy="292" r="9" class="{("protected" if applied else "single")}" />',
            f'  <text x="{px:.1f}" y="318" class="note" text-anchor="middle">{("on" if applied else "off")}</text>',
            f'  <text x="{px:.1f}" y="376" class="note" text-anchor="middle">{("true" if inhibited else "false")}</text>',
            f'  <text x="{px:.1f}" y="420" class="tick" text-anchor="middle">Sample {index+1}</text>',
        ])
    lines.extend([
        '  <text x="230" y="150" class="note" text-anchor="middle">Above 2.50 °C: request on</text>',
        '  <text x="470" y="150" class="note" text-anchor="middle">Inside band: retain on</text>',
        '  <text x="710" y="150" class="note" text-anchor="middle">Below 1.50 °C: request off</text>',
        '</svg>',
    ])
    (ROOT / "threshold-memory.svg").write_text("\n".join(lines) + "\n", encoding="utf-8")


if __name__ == "__main__":
    comparison_figure()
    memory_figure()
