#!/usr/bin/env python3
"""Render Lesson 31 from its retained native trace and the lesson's topology.

Reads figures/controller-selection-trace.csv (the normalized comparison plot)
and writes figures/controller-selection.svg. Also writes the new physical
block diagram figures/controller-selection-topology.svg from the four loops
declared in lesson.json; that diagram has no time-series data source.
"""

from __future__ import annotations

import csv
import math
from pathlib import Path

ROOT = Path(__file__).resolve().parent
INK = "#243044"
MUTED = "#526078"
GRID = "#d8e0eb"
AXIS = "#526078"
PAPER = "#f7f9fc"
BLUE = "#0072b2"
ORANGE = "#d55e00"
GREEN = "#009e73"
PINK = "#cc79a7"

STYLE = (
    "text{font-family:system-ui,-apple-system,sans-serif;fill:#243044} "
    ".axis{stroke:#526078;stroke-width:1} .grid{stroke:#d8e0eb;stroke-width:1} "
    ".marker{stroke:#b43c32;stroke-width:1.5;stroke-dasharray:4 4}"
)


def nice_num(span: float, round_result: bool) -> float:
    if span <= 0:
        return 1.0
    exponent = math.floor(math.log10(span))
    fraction = span / (10 ** exponent)
    if round_result:
        nice_fraction = 1 if fraction < 1.5 else 2 if fraction < 3 else 5 if fraction < 7 else 10
    else:
        nice_fraction = 1 if fraction <= 1 else 2 if fraction <= 2 else 5 if fraction <= 5 else 10
    return nice_fraction * (10 ** exponent)


def nice_ticks(min_val: float, max_val: float, count: int = 5) -> list[float]:
    if min_val == max_val:
        min_val, max_val = min_val - 1.0, max_val + 1.0
    pad = (max_val - min_val) * 0.04
    min_val, max_val = min_val - pad, max_val + pad
    span = nice_num(max_val - min_val, False)
    step = nice_num(span / (count - 1), True)
    niced_min = math.floor(min_val / step) * step
    return [niced_min + i * step for i in range(count)]


def draw_panel(body: list[str], left: float, right: float, top: float, bottom: float,
               title: str, y_ticks: list[float], x_ticks: list[float], x_min: float, x_max: float,
               series: list[tuple[list[float], list[float], str, str]]) -> None:
    body.append(f'<text x="{left:.0f}" y="{top - 12:.0f}" font-size="16" font-weight="700">{title}</text>')

    def sy(v: float) -> float:
        return bottom - (v - y_ticks[0]) / (y_ticks[-1] - y_ticks[0]) * (bottom - top)

    def sx(v: float) -> float:
        return left + (v - x_min) / (x_max - x_min) * (right - left)

    for yv in y_ticks:
        body.append(f'<line class="grid" x1="{left:.2f}" y1="{sy(yv):.2f}" x2="{right:.2f}" y2="{sy(yv):.2f}"/>')
        body.append(f'<text x="{left - 8:.2f}" y="{sy(yv) + 4:.2f}" text-anchor="end" font-size="11">{yv:.2f}</text>')
    for xv in x_ticks:
        body.append(f'<line class="grid" x1="{sx(xv):.2f}" y1="{top:.2f}" x2="{sx(xv):.2f}" y2="{bottom:.2f}"/>')
        body.append(f'<text x="{sx(xv):.2f}" y="{bottom + 18:.0f}" text-anchor="middle" font-size="11">{xv:.1f}</text>')
    for xs, ys, colour, dash in series:
        pts = " ".join(f"{sx(x):.2f},{sy(y):.2f}" for x, y in zip(xs, ys))
        dash_attr = f' stroke-dasharray="{dash}"' if dash else ""
        body.append(f'<polyline fill="none" stroke="{colour}" stroke-width="2.2"{dash_attr} points="{pts}"/>')
    body.append(f'<rect class="axis" x="{left:.2f}" y="{top:.2f}" width="{right - left:.2f}" height="{bottom - top:.2f}" fill="none"/>')


def legend_row(body: list[str], y: float, items: list[tuple[str, str, str]]) -> None:
    x = 55.0
    for label, colour, dash in items:
        dash_attr = f' stroke-dasharray="{dash}"' if dash else ""
        body.append(f'<line x1="{x:.1f}" y1="{y}" x2="{x + 28:.1f}" y2="{y}" stroke="{colour}" stroke-width="3"{dash_attr}/>')
        body.append(f'<text x="{x + 34:.1f}" y="{y + 5}" font-size="12">{label}</text>')
        x += 34 + 7 * len(label) + 26


def read_trace() -> tuple[list[float], dict[str, list[float]]]:
    """Read the trace CSV positionally: the header repeats "heater" for both
    the PV-relative and MV-relative series, so DictReader would collide."""
    with (ROOT / "controller-selection-trace.csv").open(newline="", encoding="utf-8") as handle:
        rows = list(csv.reader(handle))
    header, data = rows[0], rows[1:]
    columns = {name: [] for name in
               ["time_s", "cold_room", "heater_pv", "tank", "table",
                "compressor", "heater_mv", "tank_pump", "table_torque"]}
    keys = ["time_s", "cold_room", "heater_pv", "tank", "table",
            "compressor", "heater_mv", "tank_pump", "table_torque"]
    for row in data:
        for key, value in zip(keys, row[1:]):
            columns[key].append(float(value))
    return columns["time_s"], columns


def render_results() -> str:
    time_s, c = read_trace()
    x_ticks = [round(i * max(time_s) / 6, 1) for i in range(7)]

    body: list[str] = []
    legend_row(body, 101, [("cold room", BLUE, ""), ("heater", ORANGE, ""), ("tank", GREEN, ""), ("table", PINK, "")])
    legend_row(body, 121, [("compressor", BLUE, ""), ("heater", ORANGE, ""), ("tank pump", GREEN, ""), ("table torque", PINK, "")])

    y1 = nice_ticks(0.0, max(max(c["cold_room"]), max(c["heater_pv"]), max(c["tank"]), max(c["table"])))
    draw_panel(body, 70, 860, 155, 315, "Process value relative to setpoint (%)", y1, x_ticks, 0, max(time_s), [
        (time_s, c["cold_room"], BLUE, ""),
        (time_s, c["heater_pv"], ORANGE, ""),
        (time_s, c["tank"], GREEN, ""),
        (time_s, c["table"], PINK, ""),
    ])
    y2 = nice_ticks(min(min(c["compressor"]), min(c["heater_mv"]), min(c["tank_pump"]), min(c["table_torque"])),
                     max(max(c["compressor"]), max(c["heater_mv"]), max(c["tank_pump"]), max(c["table_torque"])))
    draw_panel(body, 70, 860, 360, 520, "Actuator use relative to declared range (%)", y2, x_ticks, 0, max(time_s), [
        (time_s, c["compressor"], BLUE, ""),
        (time_s, c["heater_mv"], ORANGE, ""),
        (time_s, c["tank_pump"], GREEN, ""),
        (time_s, c["table_torque"], PINK, ""),
    ])
    body.append('<text x="465" y="598" text-anchor="middle" font-size="12">time (s)</text>')

    return f'''<svg xmlns="http://www.w3.org/2000/svg" role="img" aria-labelledby="title desc" viewBox="0 0 900 610">
<title id="title">Choose the simplest controller that meets requirements</title>
<desc id="desc">Configured Simulation evidence · normalized PV and actuator use; physical values remain in the retained run</desc>
<rect width="900" height="610" fill="{PAPER}"/>
<style>{STYLE}</style>
<text x="55" y="42" font-size="24" font-weight="700">Choose the simplest controller that meets requirements</text>
<text x="55" y="68" font-size="14">Configured Simulation evidence · normalized PV and actuator use; physical values remain in the retained run</text>
{"".join(body)}
</svg>
'''


def loop_box(x: float, label: str, actuator: str, sensor: str, colour: str) -> str:
    width = 195
    return (
        f'<rect x="{x:.1f}" y="150" width="{width}" height="180" rx="10" fill="#eff6ff" stroke="{colour}" stroke-width="2"/>'
        f'<text x="{x + width / 2:.1f}" y="182" text-anchor="middle" font-size="18" font-weight="700">{label}</text>'
        f'<line x1="{x + 20:.1f}" y1="200" x2="{x + width - 20:.1f}" y2="200" stroke="{MUTED}" stroke-width="1.5"/>'
        f'<text x="{x + width / 2:.1f}" y="228" text-anchor="middle" font-size="14" fill="{MUTED}">actuator</text>'
        f'<text x="{x + width / 2:.1f}" y="248" text-anchor="middle" font-size="13">{actuator}</text>'
        f'<text x="{x + width / 2:.1f}" y="278" text-anchor="middle" font-size="14" fill="{MUTED}">sensor</text>'
        f'<text x="{x + width / 2:.1f}" y="298" text-anchor="middle" font-size="13">{sensor}</text>'
    )


def render_topology() -> str:
    loops = [
        (55, "Cold room", "compressor (binary)", "temperature", BLUE),
        (270, "Water heater", "heater power (%)", "temperature", ORANGE),
        (485, "Buffer tank", "inlet pump (%)", "level", GREEN),
        (700, "Positioning table", "motor torque (N·m)", "position", PINK),
    ]
    body: list[str] = [loop_box(x, label, actuator, sensor, colour) for x, label, actuator, sensor, colour in loops]
    for x, *_ in loops:
        cx = x + 195 / 2
        body.append(f'<line x1="{cx:.1f}" y1="345" x2="{cx:.1f}" y2="380" stroke="{MUTED}" stroke-width="2" stroke-dasharray="6 4"/>')
    body.append(f'<line x1="152" y1="380" x2="798" y2="380" stroke="{MUTED}" stroke-width="2" stroke-dasharray="6 4"/>')
    body.append(f'<rect x="330" y="405" width="240" height="46" rx="8" fill="#fff7ed" stroke="{MUTED}" stroke-width="1.5"/>')
    body.append('<text x="450" y="433" text-anchor="middle" font-size="15" font-weight="700">P1.ExperimentEnabled</text>')

    return f'''<svg xmlns="http://www.w3.org/2000/svg" width="1120" height="480" viewBox="0 0 1120 480" role="img" aria-labelledby="title2 desc2">
<title id="title2">Four independent loops share one experiment authority</title>
<desc id="desc2">Cold room, water heater, buffer tank, and positioning table each keep a separate actuator and sensor. Only the reviewed Boolean P1.ExperimentEnabled is common to all four.</desc>
<rect width="1120" height="480" rx="14" fill="{PAPER}" stroke="{GRID}"/>
<text x="48" y="48" font-family="system-ui, sans-serif" font-size="22" font-weight="700" fill="{INK}">Four loops, one shared authority, no shared signal path</text>
<text x="48" y="75" font-family="system-ui, sans-serif" font-size="15" fill="{MUTED}">Each plant keeps its own actuator and sensor; only the reviewed experiment authority is common.</text>
<g font-family="system-ui, sans-serif" text-anchor="middle" fill="{INK}">
{"".join(body)}
</g>
</svg>
'''


def main() -> None:
    (ROOT / "controller-selection.svg").write_text(render_results(), encoding="utf-8")
    (ROOT / "controller-selection-topology.svg").write_text(render_topology(), encoding="utf-8")


if __name__ == "__main__":
    main()
