#!/usr/bin/env python3
"""Render Lesson 32 from its retained native trace and the skid's topology.

Reads figures/plant-response-classes-trace.csv and writes the full-window
comparison figures/plant-response-classes.svg plus the deliberately ambiguous
figures/plant-response-classes-first-second.svg (same data, first second
only, relabeled Response A-D so the identities stay hidden). Also writes the
new physical block diagram figures/plant-response-classes-topology.svg, which
has no time-series data source.
"""

from __future__ import annotations

import csv
import math
from pathlib import Path

ROOT = Path(__file__).resolve().parent
INK = "#243044"
MUTED = "#526078"
GRID = "#d8e0eb"
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
               series: list[tuple[list[float], list[float], str]]) -> None:
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
    for xs, ys, colour in series:
        pts = " ".join(f"{sx(x):.2f},{sy(y):.2f}" for x, y in zip(xs, ys))
        body.append(f'<polyline fill="none" stroke="{colour}" stroke-width="2.2" points="{pts}"/>')
    body.append(f'<rect class="axis" x="{left:.2f}" y="{top:.2f}" width="{right - left:.2f}" height="{bottom - top:.2f}" fill="none"/>')


def legend_row(body: list[str], y: float, items: list[tuple[str, str]]) -> None:
    x = 55.0
    for label, colour in items:
        body.append(f'<line x1="{x:.1f}" y1="{y}" x2="{x + 28:.1f}" y2="{y}" stroke="{colour}" stroke-width="3"/>')
        body.append(f'<text x="{x + 34:.1f}" y="{y + 5}" font-size="12">{label}</text>')
        x += 34 + 7 * len(label) + 26


def read_trace() -> list[dict[str, str]]:
    with (ROOT / "plant-response-classes-trace.csv").open(newline="", encoding="utf-8") as handle:
        return list(csv.DictReader(handle))


def two_panel(rows: list[dict[str, str]], title: str, desc: str, legend: list[tuple[str, str]],
              panel1_title: str, panel2_title: str, cols: list[str]) -> str:
    t = [float(r["time_s"]) for r in rows]
    a = [float(r[cols[0]]) for r in rows]
    b = [float(r[cols[1]]) for r in rows]
    c = [float(r[cols[2]]) for r in rows]
    d = [float(r[cols[3]]) for r in rows]
    x_ticks = [round(i * max(t) / 6, 2) for i in range(7)]

    body: list[str] = []
    legend_row(body, 101, legend)
    y1 = nice_ticks(min(0.0, min(a), min(b)), max(max(a), max(b)))
    draw_panel(body, 70, 860, 155, 315, panel1_title, y1, x_ticks, 0, max(t), [
        (t, a, BLUE), (t, b, ORANGE),
    ])
    y2 = nice_ticks(min(0.0, min(c), min(d)), max(max(c), max(d)))
    draw_panel(body, 70, 860, 360, 520, panel2_title, y2, x_ticks, 0, max(t), [
        (t, c, GREEN), (t, d, PINK),
    ])
    body.append('<text x="465" y="598" text-anchor="middle" font-size="12">time (s)</text>')

    return f'''<svg xmlns="http://www.w3.org/2000/svg" role="img" aria-labelledby="title desc" viewBox="0 0 900 610">
<title id="title">{title}</title>
<desc id="desc">{desc}</desc>
<rect width="900" height="610" fill="{PAPER}"/>
<style>{STYLE}</style>
<text x="55" y="42" font-size="24" font-weight="700">{title}</text>
<text x="55" y="68" font-size="14">{desc}</text>
{"".join(body)}
</svg>
'''


def render_results(rows: list[dict[str, str]]) -> str:
    return two_panel(
        rows,
        "Open-loop steps reveal four response classes",
        "Configured Simulation evidence · plotted responses are normalized; the retained run preserves physical units",
        [("self-regulating vessel", BLUE), ("integrating tank", ORANGE), ("delayed oven", GREEN), ("flexible drive", PINK)],
        "Stored thermal and material responses (% of declared change)",
        "Delay and oscillation signatures (% of declared change)",
        ["self-regulating_vessel", "integrating_tank", "delayed_oven", "flexible_drive"],
    )


def render_first_second(rows: list[dict[str, str]]) -> str:
    short = [r for r in rows if float(r["time_s"]) <= 1.0]
    return two_panel(
        short,
        "One second is not enough to classify every response",
        "Unlabeled prediction view · record what remains ambiguous before revealing more data",
        [("Response A", BLUE), ("Response B", ORANGE), ("Response C", GREEN), ("Response D", PINK)],
        "Responses A and B during the first second (% of declared change)",
        "Responses C and D during the first second (% of declared change)",
        ["self-regulating_vessel", "integrating_tank", "delayed_oven", "flexible_drive"],
    )


def module_box(x: float, label: str, actuator: str, sensor: str, colour: str) -> str:
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
    modules = [
        (55, "Heated vessel", "heater command (%)", "temperature", BLUE),
        (270, "Buffer tank", "net-inflow trim (%)", "level", ORANGE),
        (485, "Conveyor oven", "heater command (%)", "product temperature", GREEN),
        (700, "Flexible drive", "torque (N·m)", "position", PINK),
    ]
    body: list[str] = [module_box(x, label, actuator, sensor, colour) for x, label, actuator, sensor, colour in modules]
    for x, *_ in modules:
        cx = x + 195 / 2
        body.append(f'<line x1="{cx:.1f}" y1="345" x2="{cx:.1f}" y2="380" stroke="{MUTED}" stroke-width="2" stroke-dasharray="6 4"/>')
    body.append(f'<line x1="152" y1="380" x2="798" y2="380" stroke="{MUTED}" stroke-width="2" stroke-dasharray="6 4"/>')
    body.append(f'<rect x="330" y="405" width="240" height="46" rx="8" fill="#fff7ed" stroke="{MUTED}" stroke-width="1.5"/>')
    body.append('<text x="450" y="433" text-anchor="middle" font-size="15" font-weight="700">Normalized experiment command</text>')

    return f'''<svg xmlns="http://www.w3.org/2000/svg" width="1120" height="480" viewBox="0 0 1120 480" role="img" aria-labelledby="title2 desc2">
<title id="title2">Four modules share one normalized command and nothing else</title>
<desc id="desc2">Heated vessel, buffer tank, conveyor oven, and flexible drive each keep a separate actuator and sensor. Only the shared normalized experiment command reaches all four.</desc>
<rect width="1120" height="480" rx="14" fill="{PAPER}" stroke="{GRID}"/>
<text x="48" y="48" font-family="system-ui, sans-serif" font-size="22" font-weight="700" fill="{INK}">Four modules, one shared command, four different mechanisms</text>
<text x="48" y="75" font-family="system-ui, sans-serif" font-size="15" fill="{MUTED}">Each module keeps its own actuator and sensor; only the normalized command is common.</text>
<g font-family="system-ui, sans-serif" text-anchor="middle" fill="{INK}">
{"".join(body)}
</g>
</svg>
'''


def main() -> None:
    rows = read_trace()
    (ROOT / "plant-response-classes.svg").write_text(render_results(rows), encoding="utf-8")
    (ROOT / "plant-response-classes-first-second.svg").write_text(render_first_second(rows), encoding="utf-8")
    (ROOT / "plant-response-classes-topology.svg").write_text(render_topology(), encoding="utf-8")


if __name__ == "__main__":
    main()
