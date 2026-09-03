#!/usr/bin/env python3
"""Render Lesson 84's figures.

Pure Python, no third-party dependencies.

- figures/independent-capstone-evidence.svg is rebuilt from the retained
  native trace figures/independent-capstone-evidence-trace.csv, keeping the
  same two panels, series, and axes the shipped figure uses.
- figures/independent-capstone-topology.svg is a structural box-and-arrow
  diagram of the feedback-plus-feedforward signal path onto the shared
  receiver. It is not derived from CSV data (it has no panels, series, or
  axes to reproduce); it is written directly in the visual style of
  examples/control/01-feedback-foundations-and-temperature-control/
  08-water-heater-saturation/figures/heater-signal-path.svg.
"""

from __future__ import annotations

import csv
from pathlib import Path

ROOT = Path(__file__).resolve().parent

# --- Evidence chart -------------------------------------------------------

PANELS = [
    {
        "name": "Pressure integrated absolute error",
        "axis_title": "Pressure integrated absolute error (bar*s)",
        "y_min": 0.0,
        "y_max": 32.0,
        "top": 195.0,
        "bottom": 355.0,
        "x_axis_label_y": 391.0,
    },
    {
        "name": "Applied-compressor movement",
        "axis_title": "Applied-compressor movement (percentage points)",
        "y_min": 0.0,
        "y_max": 180.0,
        "top": 445.0,
        "bottom": 605.0,
        "x_axis_label_y": 641.0,
    },
]

SERIES_STYLE = {
    "Nominal plant": {"colour": "#2563eb", "dash": ""},
    "Uncertainty plant": {"colour": "#dc2626", "dash": "7 5"},
}

LEFT, RIGHT = 75.0, 855.0
X_MIN, X_MAX = -0.05, 1.05


def read_trace() -> dict[str, dict[str, list[tuple[float, float]]]]:
    with (ROOT / "independent-capstone-evidence-trace.csv").open(newline="", encoding="utf-8") as handle:
        rows = list(csv.DictReader(handle))
    by_panel: dict[str, dict[str, list[tuple[float, float]]]] = {}
    for row in rows:
        panel = by_panel.setdefault(row["panel"], {})
        panel.setdefault(row["series"], []).append((float(row["x"]), float(row["y"])))
    return by_panel


def sx(index: float) -> float:
    return LEFT + (index - X_MIN) / (X_MAX - X_MIN) * (RIGHT - LEFT)


def sy(value: float, y_min: float, y_max: float, top: float, bottom: float) -> float:
    return bottom - (value - y_min) / (y_max - y_min) * (bottom - top)


def render_evidence(by_panel: dict[str, dict[str, list[tuple[float, float]]]]) -> str:
    body: list[str] = []
    body.append(
        '<line x1="55" y1="146" x2="83" y2="146" stroke="#2563eb" stroke-width="3"/>'
        '<text x="89" y="151" font-size="12">Nominal plant</text>'
        '<line x1="191" y1="146" x2="219" y2="146" stroke="#dc2626" stroke-width="3" stroke-dasharray="7 5"/>'
        '<text x="225" y="151" font-size="12">Uncertainty plant</text>'
    )
    for panel in PANELS:
        series_map = by_panel[panel["name"]]
        top, bottom = panel["top"], panel["bottom"]
        y_min, y_max = panel["y_min"], panel["y_max"]
        body.append(f'<text x="75" y="{top - 12:.2f}" font-size="16" font-weight="700">{panel["axis_title"]}</text>')
        for step in range(5):
            value = y_min + (y_max - y_min) * (4 - step) / 4
            y = top + step / 4 * (bottom - top)
            body.append(f'<line class="grid" x1="75" y1="{y:.2f}" x2="855" y2="{y:.2f}"/>')
            body.append(f'<text x="67" y="{y + 4:.2f}" text-anchor="end" font-size="11">{value:.2f}</text>')
        for step in range(5):
            index = X_MIN + step / 4 * (X_MAX - X_MIN)
            x = sx(index)
            body.append(f'<line class="grid" x1="{x:.2f}" y1="{top}" x2="{x:.2f}" y2="{bottom}"/>')
            body.append(f'<text x="{x:.2f}" y="{bottom + 18:.2f}" text-anchor="middle" font-size="11">{index:.2f}</text>')
        for series_name, points in series_map.items():
            colour = SERIES_STYLE[series_name]["colour"]
            for index, value in points:
                body.append(
                    f'<circle cx="{sx(index):.2f}" cy="{sy(value, y_min, y_max, top, bottom):.2f}" r="4" fill="{colour}"/>'
                )
        body.append(f'<rect class="axis" x="75" y="{top}" width="780" height="{bottom - top}" fill="none"/>')
        body.append(f'<text x="465" y="{panel["x_axis_label_y"]:.2f}" text-anchor="middle" font-size="12">architecture index</text>')

    return f'''<svg xmlns="http://www.w3.org/2000/svg" role="img" aria-labelledby="title desc" viewBox="0 0 900 650">
<title id="title">Qualified demand feedforward improves the compressed-air receiver under uncertainty</title>
<desc id="desc">Index 0 is feedback only and index 1 is PI plus qualified demand feedforward; the same decision is tested on nominal and uncertainty plants.</desc>
<defs><marker id="arrow" markerWidth="8" markerHeight="8" refX="7" refY="3" orient="auto"><path d="M0,0 L0,6 L8,3 z" fill="#243044"/></marker></defs>
<rect width="900" height="650" fill="#f7f9fc"/>
<style>text{{font-family:system-ui,-apple-system,sans-serif;fill:#243044}} .axis{{stroke:#526078;stroke-width:1}} .grid{{stroke:#d8e0eb;stroke-width:1}}</style>
<text x="55" y="42" font-size="24" font-weight="700">Qualified demand feedforward improves the compressed-air</text>
<text x="55" y="70" font-size="24" font-weight="700">receiver under uncertainty</text>
<text x="55" y="98" font-size="14">Index 0 is feedback only and index 1 is PI plus qualified demand feedforward; the same</text>
<text x="55" y="116" font-size="14">decision is tested on nominal and uncertainty plants.</text>
{"".join(body)}
</svg>
'''


# --- Topology diagram (structural, not CSV-derived) ------------------------

def render_topology() -> str:
    return '''<svg xmlns="http://www.w3.org/2000/svg" width="1200" height="420" viewBox="0 0 1200 420" role="img" aria-labelledby="title desc">
  <title id="title">Feedback and measured-demand feedforward share one actuator path</title>
  <desc id="desc">Pressure error drives a PI controller; measured plant demand separately drives a one-for-one feedforward gain. Both requested contributions sum before one amplitude and slew limit, and only the applied delivery request reaches the compressor and receiver; measured header pressure returns to the PI controller, and plant demand also acts directly on the receiver as physical flow.</desc>
  <defs>
    <marker id="arrow-blue" viewBox="0 0 10 10" refX="9" refY="5" markerWidth="7" markerHeight="7" orient="auto-start-reverse">
      <path d="M 0 0 L 10 5 L 0 10 z" fill="#2563eb"/>
    </marker>
    <marker id="arrow-orange" viewBox="0 0 10 10" refX="9" refY="5" markerWidth="7" markerHeight="7" orient="auto-start-reverse">
      <path d="M 0 0 L 10 5 L 0 10 z" fill="#ea580c"/>
    </marker>
    <marker id="arrow-gray" viewBox="0 0 10 10" refX="9" refY="5" markerWidth="7" markerHeight="7" orient="auto-start-reverse">
      <path d="M 0 0 L 10 5 L 0 10 z" fill="#475569"/>
    </marker>
    <marker id="arrow-purple" viewBox="0 0 10 10" refX="9" refY="5" markerWidth="7" markerHeight="7" orient="auto-start-reverse">
      <path d="M 0 0 L 10 5 L 0 10 z" fill="#a21caf"/>
    </marker>
    <marker id="arrow-green" viewBox="0 0 10 10" refX="9" refY="5" markerWidth="7" markerHeight="7" orient="auto-start-reverse">
      <path d="M 0 0 L 10 5 L 0 10 z" fill="#16a34a"/>
    </marker>
  </defs>

  <rect width="1200" height="420" rx="18" fill="#f8fafc"/>
  <text x="48" y="40" fill="#0f172a" font-family="system-ui, sans-serif" font-size="21" font-weight="700">Feedback and measured-demand feedforward share one actuator path</text>
  <text x="48" y="63" fill="#475569" font-family="system-ui, sans-serif" font-size="14">Both requested contributions pass through the same amplitude and slew limit before the compressor.</text>

  <g font-family="system-ui, sans-serif" text-anchor="middle">
    <rect x="40" y="100" width="220" height="72" rx="10" fill="#eff6ff" stroke="#2563eb" stroke-width="2"/>
    <text x="150" y="130" fill="#0f172a" font-size="16" font-weight="700">PI controller</text>
    <text x="150" y="151" fill="#1d4ed8" font-size="13">pressure error e = SP - PV</text>

    <rect x="40" y="220" width="220" height="72" rx="10" fill="#fdf4ff" stroke="#a21caf" stroke-width="2"/>
    <text x="150" y="250" fill="#0f172a" font-size="16" font-weight="700">Feedforward gain</text>
    <text x="150" y="271" fill="#86198f" font-size="13">one-for-one on measured demand</text>

    <rect x="340" y="160" width="230" height="72" rx="10" fill="#fff7ed" stroke="#ea580c" stroke-width="2"/>
    <text x="455" y="190" fill="#0f172a" font-size="16" font-weight="700">Amplitude / slew limit</text>
    <text x="455" y="211" fill="#9a3412" font-size="13">MV requested to MV applied</text>

    <rect x="650" y="160" width="220" height="72" rx="10" fill="#f8fafc" stroke="#64748b" stroke-width="2"/>
    <text x="760" y="190" fill="#0f172a" font-size="16" font-weight="700">Compressor</text>
    <text x="760" y="211" fill="#475569" font-size="13">5-second delivery lag</text>

    <rect x="950" y="160" width="220" height="72" rx="10" fill="#f0fdf4" stroke="#16a34a" stroke-width="2"/>
    <text x="1060" y="190" fill="#0f172a" font-size="16" font-weight="700">Receiver</text>
    <text x="1060" y="211" fill="#166534" font-size="13">header pressure PV</text>

    <rect x="340" y="300" width="230" height="72" rx="10" fill="#f8fafc" stroke="#64748b" stroke-width="2"/>
    <text x="455" y="330" fill="#0f172a" font-size="16" font-weight="700">Plant air demand</text>
    <text x="455" y="351" fill="#475569" font-size="13">measured disturbance q_d</text>
  </g>

  <path d="M260 136 L335 190" fill="none" stroke="#2563eb" stroke-width="3" marker-end="url(#arrow-blue)"/>
  <path d="M260 256 L335 205" fill="none" stroke="#a21caf" stroke-width="3" marker-end="url(#arrow-purple)"/>
  <text x="300" y="245" fill="#334155" font-family="system-ui, sans-serif" font-size="12" font-weight="600" text-anchor="middle">MV_feedback, MV_feedforward</text>

  <path d="M570 196 H645" fill="none" stroke="#ea580c" stroke-width="3" marker-end="url(#arrow-orange)"/>
  <path d="M870 196 H945" fill="none" stroke="#475569" stroke-width="3" marker-end="url(#arrow-gray)"/>
  <text x="607" y="181" fill="#9a3412" font-family="system-ui, sans-serif" font-size="12" font-weight="600" text-anchor="middle">MV applied</text>

  <path d="M455 300 V232" fill="none" stroke="#a21caf" stroke-width="2.5" stroke-dasharray="6 4" marker-end="url(#arrow-purple)"/>
  <text x="455" y="292" fill="#86198f" font-family="system-ui, sans-serif" font-size="12" font-weight="600" text-anchor="middle">measured signal</text>

  <path d="M570 336 H1060 V236" fill="none" stroke="#475569" stroke-width="2.5" stroke-dasharray="3 3" marker-end="url(#arrow-gray)"/>
  <text x="815" y="351" fill="#334155" font-family="system-ui, sans-serif" font-size="12" font-weight="600" text-anchor="middle">physical flow imbalance, not a controller signal</text>

  <path d="M1060 232 V380 H150 V172" fill="none" stroke="#16a34a" stroke-width="3" marker-end="url(#arrow-green)"/>
  <text x="600" y="398" fill="#166534" font-family="system-ui, sans-serif" font-size="13" font-weight="600" text-anchor="middle">measured header pressure (PV) returns to the PI controller</text>
</svg>
'''


def main() -> None:
    by_panel = read_trace()
    (ROOT / "independent-capstone-evidence.svg").write_text(render_evidence(by_panel), encoding="utf-8")
    (ROOT / "independent-capstone-topology.svg").write_text(render_topology(), encoding="utf-8")


if __name__ == "__main__":
    main()
