#!/usr/bin/env python3
"""Render Lesson 14 timing evidence from the retained native trace."""

import csv
from pathlib import Path


ROOT = Path(__file__).resolve().parent
TRACE = ROOT / "sampling-timing-trace.csv"
OUTPUT = ROOT / "sampling-timing-response.svg"
CONCEPT_OUTPUT = ROOT / "sampling-hold-concept.svg"
WIDTH = 900
HEIGHT = 760
COLORS = {
    "measured_50_ms": "#2563eb",
    "measured_100_ms": "#15803d",
    "measured_200_ms": "#7c3aed",
    "measured_jitter": "#d97706",
    "bad_nominal_200_ms": "#b91c1c",
}


def load_trace():
    with TRACE.open(newline="", encoding="utf-8") as stream:
        rows = list(csv.DictReader(stream))
    grouped = {}
    for row in rows:
        grouped.setdefault(row["case_id"], []).append({
            "index": int(row["sample_index"]),
            "time": float(row["time_s"]),
            "actual_dt": float(row["actual_dt_s"]),
            "controller_dt": float(row["controller_dt_s"]),
            "sp": float(row["setpoint_degc"]),
            "pv": float(row["pv_degc"]),
            "requested": float(row["requested_trim_pp"]),
            "applied": float(row["applied_trim_pp"]),
            "valid": row["timing_valid"] == "true",
            "fault": row["timing_fault_latched"] == "true",
            "auto": row["auto_mode"] == "true",
            "reset": row["reset_requested"] == "true",
            "event": row["event"],
        })
    return grouped


def map_x(value, low, high, box):
    return box[0] + (value - low) * box[2] / (high - low)


def map_y(value, low, high, box):
    return box[1] + box[3] - (value - low) * box[3] / (high - low)


def axes(svg, box, title, x_label, y_label, x_range, y_range, x_ticks, y_ticks):
    left, top, width, height = box
    svg.append(
        f'<rect x="{left}" y="{top}" width="{width}" height="{height}" '
        'class="plot" fill="#ffffff" stroke="#9ca3af"/>'
    )
    svg.append(f'<text x="{left}" y="{top - 12}" class="panel-title">{title}</text>')
    for value, label in x_ticks:
        x = map_x(value, *x_range, box)
        svg.append(f'<line x1="{x:.2f}" y1="{top}" x2="{x:.2f}" y2="{top + height}" class="grid"/>')
        svg.append(f'<text x="{x:.2f}" y="{top + height + 18}" class="tick" text-anchor="middle">{label}</text>')
    for value, label in y_ticks:
        y = map_y(value, *y_range, box)
        svg.append(f'<line x1="{left}" y1="{y:.2f}" x2="{left + width}" y2="{y:.2f}" class="grid"/>')
        svg.append(f'<text x="{left - 8}" y="{y + 4:.2f}" class="tick" text-anchor="end">{label}</text>')
    svg.append(f'<text x="{left + width / 2}" y="{top + height + 38}" class="axis" text-anchor="middle">{x_label}</text>')
    svg.append(
        f'<text x="{left - 43}" y="{top + height / 2}" class="axis" text-anchor="middle" '
        f'transform="rotate(-90 {left - 43} {top + height / 2})">{y_label}</text>'
    )


def line_points(rows, x_key, y_key, x_range, y_range, box):
    return " ".join(
        f'{map_x(row[x_key], *x_range, box):.2f},{map_y(row[y_key], *y_range, box):.2f}'
        for row in rows
        if x_range[0] <= row[x_key] <= x_range[1]
    )


def step_points(rows, x_key, y_key, x_range, y_range, box):
    selected = [row for row in rows if x_range[0] <= row[x_key] <= x_range[1]]
    points = []
    for index, row in enumerate(selected):
        x = map_x(row[x_key], *x_range, box)
        y = map_y(row[y_key], *y_range, box)
        if index:
            previous_y = map_y(selected[index - 1][y_key], *y_range, box)
            points.append(f"{x:.2f},{previous_y:.2f}")
        points.append(f"{x:.2f},{y:.2f}")
    return " ".join(points)


def render():
    traces = load_trace()
    svg = [
        f'<svg xmlns="http://www.w3.org/2000/svg" width="{WIDTH}" height="{HEIGHT}" viewBox="0 0 {WIDTH} {HEIGHT}">',
        '<style>svg{forced-color-adjust:none;background:#fff;color:#111827;font-family:system-ui,-apple-system,"Segoe UI",sans-serif}.title{font-size:22px;font-weight:700}.subtitle{font-size:12px;fill:#4b5563}.panel-title{font-size:14px;font-weight:650;fill:#111827}.axis,.tick,.legend,.note{font-size:11px;fill:#374151}.plot{fill:#fff;stroke:#9ca3af;stroke-width:1}.grid{stroke:#e5e7eb;stroke-width:1}.series{fill:none;stroke-width:2}.thin{fill:none;stroke-width:1.2}.sample{stroke:#fff;stroke-width:0.8}.fault{fill:#fef2f2;stroke:#b91c1c}</style>',
        '<rect width="900" height="760" fill="#ffffff"/>',
        '<text x="50" y="34" class="title">Conveyor-oven sampling, jitter, and missed scans</text>',
        '<text x="50" y="55" class="subtitle">Native deterministic simulation · measured elapsed time selected · Simulation evidence</text>',
    ]
    boxes = [(75, 100, 340, 200), (525, 100, 300, 200), (75, 400, 340, 190), (525, 400, 300, 190)]

    axes(svg, boxes[0], "Equivalent temperature responses", "Physical time (s)",
         "Temperature deviation (degC)", (0, 8), (-0.1, 1.1),
         [(0, "0"), (2, "2"), (4, "4"), (6, "6"), (8, "8")],
         [(-0.1, "-0.1"), (0.5, "0.5"), (1.1, "1.1")])
    for case_id in COLORS:
        rows = traces[case_id]
        dash = ' stroke-dasharray="7 4"' if case_id == "bad_nominal_200_ms" else ""
        svg.append(
            f'<polyline points="{line_points(rows, "time", "pv", (0, 8), (-0.1, 1.1), boxes[0])}" '
            f'class="series" stroke="{COLORS[case_id]}"{dash}/>'
        )
    selected = traces["measured_100_ms"]
    svg.append(
        f'<polyline points="{step_points(selected, "time", "sp", (0, 8), (-0.1, 1.1), boxes[0])}" '
        'class="thin" stroke="#111827" stroke-dasharray="3 3"/>'
    )

    axes(svg, boxes[1], "Sampled PV and held heater trim", "Physical time (s)",
         "PV (degC) / trim (pp)", (0.8, 1.6), (-0.1, 1.4),
         [(0.8, "0.8"), (1.0, "1.0"), (1.2, "1.2"), (1.4, "1.4"), (1.6, "1.6")],
         [(-0.1, "-0.1"), (0.65, "0.65"), (1.4, "1.4")])
    jitter = traces["measured_jitter"]
    svg.append(
        f'<polyline points="{line_points(jitter, "time", "pv", (0.8, 1.6), (-0.1, 1.4), boxes[1])}" '
        'class="series" stroke="#2563eb"/>'
    )
    svg.append(
        f'<polyline points="{step_points(jitter, "time", "applied", (0.8, 1.6), (-0.1, 1.4), boxes[1])}" '
        'class="series" stroke="#d97706"/>'
    )
    for row in jitter:
        if 0.8 <= row["time"] <= 1.6:
            x = map_x(row["time"], 0.8, 1.6, boxes[1])
            y = map_y(row["pv"], -0.1, 1.4, boxes[1])
            svg.append(f'<circle cx="{x:.2f}" cy="{y:.2f}" r="3" fill="#2563eb" class="sample"/>')

    axes(svg, boxes[2], "Actual interval versus controller interval", "Update index", "Interval (s)",
         (0, 5), (0, 0.22), [(0, "0"), (1, "1"), (2, "2"), (3, "3"), (4, "4"), (5, "5")],
         [(0, "0"), (0.1, "0.10"), (0.2, "0.20")])
    jitter_cycle = [row for row in jitter if 0 < row["time"] <= 0.5 + 1e-9]
    bad_cycle = traces["bad_nominal_200_ms"][1:6]
    for rows, y_key, color, dash in [
        (jitter_cycle, "actual_dt", "#d97706", ""),
        (jitter_cycle, "controller_dt", "#2563eb", "6 3"),
        (bad_cycle, "actual_dt", "#b91c1c", ""),
        (bad_cycle, "controller_dt", "#111827", "3 3"),
    ]:
        normalized = [{**row, "plot_index": idx + 1} for idx, row in enumerate(rows)]
        dash_attr = f' stroke-dasharray="{dash}"' if dash else ""
        svg.append(
            f'<polyline points="{line_points(normalized, "plot_index", y_key, (0, 5), (0, 0.22), boxes[2])}" '
            f'class="series" stroke="{color}"{dash_attr}/>'
        )

    axes(svg, boxes[3], "Missed-scan fault and reviewed recovery", "Recovery step", "Heater trim (pp)",
         (0, 5), (-0.02, 0.08), [(0, "normal"), (1, "fault"), (2, "valid"), (3, "reset"), (4, "Manual"), (5, "Auto")],
         [(0, "0"), (0.04, "0.04"), (0.08, "0.08")])
    fault = traces["missed_scan_recovery"]
    svg.append(
        f'<polyline points="{line_points(fault, "index", "applied", (0, 5), (-0.02, 0.08), boxes[3])}" '
        'class="series" stroke="#b91c1c"/>'
    )
    for row in fault:
        x = map_x(row["index"], 0, 5, boxes[3])
        y = map_y(row["applied"], -0.02, 0.08, boxes[3])
        svg.append(f'<circle cx="{x:.2f}" cy="{y:.2f}" r="3.2" fill="#b91c1c" class="sample"/>')

    legends = [
        ("#2563eb", "50 ms measured"), ("#15803d", "100 ms measured"),
        ("#7c3aed", "200 ms measured"), ("#d97706", "jitter measured"),
        ("#b91c1c", "200 ms actual / 100 ms nominal (ineligible)"),
    ]
    for index, (color, label) in enumerate(legends):
        x = 75 + (index % 3) * 250
        y = 675 + (index // 3) * 25
        svg.append(f'<line x1="{x}" y1="{y}" x2="{x + 30}" y2="{y}" class="series" stroke="{color}"/>')
        svg.append(f'<text x="{x + 38}" y="{y + 4}" class="legend">{label}</text>')
    svg.append('<text x="75" y="735" class="note">PV lines connect retained samples; intersample temperature is not measured. Heater trim is held between updates.</text>')
    svg.append('</svg>')
    OUTPUT.write_text("\n".join(svg) + "\n", encoding="utf-8")


def render_concept():
    """Render the physical sample-and-hold idea before frequency notation."""
    sample_x = [190, 330, 470, 610, 750]
    sample_y = [205, 168, 132, 112, 102]
    svg = [
        '<svg xmlns="http://www.w3.org/2000/svg" width="900" height="360" viewBox="0 0 900 360">',
        '<style>svg{forced-color-adjust:none;background:#fff;font-family:system-ui,-apple-system,"Segoe UI",sans-serif}.title{font-size:21px;font-weight:700;fill:#111827}.label{font-size:13px;fill:#374151}.small{font-size:11px;fill:#4b5563}.guide{stroke:#cbd5e1;stroke-width:1;stroke-dasharray:4 4}.axis{stroke:#6b7280;stroke-width:1.2}.continuous{fill:none;stroke:#2563eb;stroke-width:3}.held{fill:none;stroke:#d97706;stroke-width:3}.sample{fill:#2563eb;stroke:#fff;stroke-width:1.5}</style>',
        '<rect width="900" height="360" fill="#ffffff"/>',
        '<text x="55" y="38" class="title">Continuous oven temperature, samples, and held output</text>',
        '<text x="55" y="62" class="small">Explanatory timing diagram · not a measured response</text>',
        '<line x1="100" y1="245" x2="820" y2="245" class="axis"/>',
        '<line x1="100" y1="85" x2="100" y2="245" class="axis"/>',
        '<path d="M100 225 C185 220 235 195 300 175 S420 125 500 115 S650 95 820 92" class="continuous"/>',
    ]
    for x, y in zip(sample_x, sample_y):
        svg.append(f'<line x1="{x}" y1="82" x2="{x}" y2="292" class="guide"/>')
        svg.append(f'<circle cx="{x}" cy="{y}" r="5" class="sample"/>')
    svg.extend([
        '<path d="M100 310 H190 V285 H330 V260 H470 V236 H610 V220 H750 V208 H820" class="held"/>',
        '<text x="610" y="86" class="label" fill="#2563eb">continuous oven temperature</text>',
        '<text x="610" y="201" class="label" fill="#d97706">held heater output</text>',
        '<text x="190" y="338" class="small" text-anchor="middle">sample instants</text>',
        '<text x="585" y="338" class="small" text-anchor="middle">blue dots are sampled; orange output is held between updates</text>',
        '</svg>',
    ])
    CONCEPT_OUTPUT.write_text("\n".join(svg) + "\n", encoding="utf-8")


if __name__ == "__main__":
    render_concept()
    render()
