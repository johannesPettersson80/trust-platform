#!/usr/bin/env python3
"""Render Lesson 13 evidence from the retained native trace and filter law."""

import csv
import math
from pathlib import Path


ROOT = Path(__file__).resolve().parent
TRACE = ROOT / "measurement-filter-trace.csv"
OUTPUT = ROOT / "measurement-filter-response.svg"
WIDTH = 900
HEIGHT = 760
COLORS = {"bypass": "#d97706", "first_order": "#2563eb", "second_order": "#15803d"}
DASHES = {"bypass": "", "first_order": "", "second_order": "7 4"}


def load_trace():
    with TRACE.open(newline="", encoding="utf-8") as stream:
        rows = list(csv.DictReader(stream))
    grouped = {name: [] for name in COLORS}
    for row in rows:
        grouped[row["filter"]].append({
            "time": float(row["time_s"]),
            "sp": float(row["setpoint_bar"]),
            "truth": float(row["simulator_true_pressure_bar"]),
            "raw": float(row["raw_pv_bar"]),
            "controller": float(row["controller_pv_bar"]),
            "mv": float(row["applied_mv_pp"]),
        })
    return grouped


def line_points(rows, x_key, y_key, x0, x1, y0, y1, box):
    left, top, width, height = box
    points = []
    for row in rows:
        x = row[x_key]
        if x0 <= x <= x1:
            y = row[y_key]
            px = left + (x - x0) * width / (x1 - x0)
            py = top + height - (y - y0) * height / (y1 - y0)
            points.append(f"{px:.2f},{py:.2f}")
    return " ".join(points)


def axes(svg, box, title, x_label, y_label, x_ticks, y_ticks):
    left, top, width, height = box
    svg.append(f'<rect x="{left}" y="{top}" width="{width}" height="{height}" class="plot" fill="#ffffff" stroke="#9ca3af"/>')
    svg.append(f'<text x="{left}" y="{top - 12}" class="panel-title">{title}</text>')
    for value, label in x_ticks:
        x = left + value * width
        svg.append(f'<line x1="{x:.2f}" y1="{top}" x2="{x:.2f}" y2="{top + height}" class="grid" stroke="#e5e7eb"/>')
        svg.append(f'<text x="{x:.2f}" y="{top + height + 18}" class="tick" text-anchor="middle">{label}</text>')
    for value, label in y_ticks:
        y = top + height - value * height
        svg.append(f'<line x1="{left}" y1="{y:.2f}" x2="{left + width}" y2="{y:.2f}" class="grid" stroke="#e5e7eb"/>')
        svg.append(f'<text x="{left - 8}" y="{y + 4:.2f}" class="tick" text-anchor="end">{label}</text>')
    svg.append(f'<text x="{left + width / 2}" y="{top + height + 38}" class="axis" text-anchor="middle">{x_label}</text>')
    svg.append(f'<text x="{left - 43}" y="{top + height / 2}" class="axis" text-anchor="middle" transform="rotate(-90 {left - 43} {top + height / 2})">{y_label}</text>')


def frequency_response(order, frequency_hz, coefficient=2.0 / 3.0, dt=0.1):
    omega = 2.0 * math.pi * frequency_hz * dt
    denominator = math.sqrt(1.0 + (1.0 - coefficient) ** 2
                            - 2.0 * (1.0 - coefficient) * math.cos(omega))
    magnitude = coefficient / denominator
    return 20.0 * math.log10(magnitude ** order)


def render():
    traces = load_trace()
    svg = [
        f'<svg xmlns="http://www.w3.org/2000/svg" width="{WIDTH}" height="{HEIGHT}" viewBox="0 0 {WIDTH} {HEIGHT}">',
        '<style>svg{forced-color-adjust:none;background:#fff;color:#111827;font-family:system-ui,-apple-system,"Segoe UI",sans-serif}.title{font-size:22px;font-weight:700}.subtitle{font-size:12px;fill:#4b5563}.panel-title{font-size:14px;font-weight:650;fill:#111827}.axis,.tick,.legend,.note{font-size:11px;fill:#374151}.plot{fill:#fff;stroke:#9ca3af;stroke-width:1}.grid{stroke:#e5e7eb;stroke-width:1}.series{fill:none;stroke-width:2}.thin{fill:none;stroke-width:1.3}.event{stroke:#7c3aed;stroke-width:1.2;stroke-dasharray:4 4}.selected{fill:#eff6ff;stroke:#2563eb}</style>',
        '<rect width="900" height="760" fill="#ffffff"/>',
        '<text x="50" y="34" class="title">Pump-pressure measurement filtering</text>',
        '<text x="50" y="55" class="subtitle">Native deterministic simulation · selected: first-order filter · Simulation evidence</text>',
    ]

    boxes = [(75, 100, 340, 200), (525, 100, 300, 200), (75, 400, 340, 190), (525, 400, 300, 190)]
    axes(svg, boxes[0], "Setpoint response and measurement paths", "Time (s)", "Pressure deviation (bar)",
         [(0, "0"), (1/3, "5"), (2/3, "10"), (1, "15")],
         [(0, "-0.1"), (5/12, "0.4"), (10/12, "0.9")])
    for name, rows in traces.items():
        points = line_points(rows, "time", "controller", 0, 15, -0.1, 1.1, boxes[0])
        dash = f' stroke-dasharray="{DASHES[name]}"' if DASHES[name] else ""
        svg.append(f'<polyline points="{points}" class="series" fill="none" stroke-width="2" stroke="{COLORS[name]}"{dash}/>')
    selected = traces["first_order"]
    svg.append(f'<polyline points="{line_points(selected, "time", "raw", 0, 15, -0.1, 1.1, boxes[0])}" class="thin" fill="none" stroke-width="1.3" stroke="#9ca3af"/>')
    svg.append(f'<polyline points="{line_points(selected, "time", "truth", 0, 15, -0.1, 1.1, boxes[0])}" class="thin" fill="none" stroke-width="1.3" stroke="#111827" stroke-dasharray="2 3"/>')
    svg.append(f'<polyline points="{line_points(selected, "time", "sp", 0, 15, -0.1, 1.1, boxes[0])}" class="thin" fill="none" stroke-width="1.3" stroke="#7c3aed" stroke-dasharray="5 4"/>')
    event_x = boxes[0][0] + 5.1 / 15 * boxes[0][2]
    svg.append(f'<line x1="{event_x:.2f}" y1="{boxes[0][1]}" x2="{event_x:.2f}" y2="{boxes[0][1] + boxes[0][3]}" class="event"/>')

    axes(svg, boxes[1], "Steady-noise detail", "Time (s)", "Controller PV (bar)",
         [(0, "20"), (0.5, "22"), (1, "24")], [(0, "0.96"), (0.5, "1.00"), (1, "1.04")])
    for name, rows in traces.items():
        points = line_points(rows, "time", "controller", 20, 24, 0.96, 1.04, boxes[1])
        dash = f' stroke-dasharray="{DASHES[name]}"' if DASHES[name] else ""
        svg.append(f'<polyline points="{points}" class="series" fill="none" stroke-width="2" stroke="{COLORS[name]}"{dash}/>')

    axes(svg, boxes[2], "Noise-driven recycle-valve activity", "Time (s)", "Applied trim (pp)",
         [(0, "20"), (0.5, "22"), (1, "24")], [(0, "0.95"), (0.5, "1.00"), (1, "1.05")])
    for name, rows in traces.items():
        points = line_points(rows, "time", "mv", 20, 24, 0.95, 1.05, boxes[2])
        dash = f' stroke-dasharray="{DASHES[name]}"' if DASHES[name] else ""
        svg.append(f'<polyline points="{points}" class="series" fill="none" stroke-width="2" stroke="{COLORS[name]}"{dash}/>')

    axes(svg, boxes[3], "Executed discrete magnitude response", "Frequency (Hz)", "Magnitude (dB)",
         [(0, "0"), (0.4, "2"), (0.8, "4"), (1, "5")], [(0, "-12"), (0.5, "-6"), (1, "0")])
    frequencies = [index * 5.0 / 160 for index in range(161)]
    for name, order in [("first_order", 1), ("second_order", 2)]:
        rows = [{"f": f, "db": frequency_response(order, f)} for f in frequencies]
        points = line_points(rows, "f", "db", 0, 5, -12, 0, boxes[3])
        dash = f' stroke-dasharray="{DASHES[name]}"' if DASHES[name] else ""
        svg.append(f'<polyline points="{points}" class="series" fill="none" stroke-width="2" stroke="{COLORS[name]}"{dash}/>')
    svg.append(f'<line x1="{boxes[3][0]}" y1="{boxes[3][1]}" x2="{boxes[3][0] + boxes[3][2]}" y2="{boxes[3][1]}" class="series" stroke-width="2" stroke="{COLORS["bypass"]}"/>')

    legend_y = 680
    for index, (name, label) in enumerate([("bypass", "Bypass"), ("first_order", "First order (selected)"), ("second_order", "Second order")]):
        x = 75 + index * 190
        dash = f' stroke-dasharray="{DASHES[name]}"' if DASHES[name] else ""
        svg.append(f'<line x1="{x}" y1="{legend_y}" x2="{x + 30}" y2="{legend_y}" class="series" stroke-width="2" stroke="{COLORS[name]}"{dash}/>')
        svg.append(f'<text x="{x + 38}" y="{legend_y + 4}" class="legend">{label}</text>')
    svg.append('<line x1="640" y1="680" x2="670" y2="680" class="thin" stroke-width="1.3" stroke="#9ca3af"/><text x="678" y="684" class="legend">Raw (selected run)</text>')
    svg.append('<line x1="75" y1="708" x2="105" y2="708" class="thin" stroke-width="1.3" stroke="#111827" stroke-dasharray="2 3"/><text x="113" y="712" class="legend">Simulator-only true pressure (not a controller input)</text>')
    svg.append('<rect x="525" y="698" width="300" height="34" rx="5" class="selected"/><text x="538" y="719" class="note">First order passes attenuation, lag and bias requirements.</text>')
    svg.append('</svg>')
    OUTPUT.write_text("\n".join(svg) + "\n", encoding="utf-8")


if __name__ == "__main__":
    render()
