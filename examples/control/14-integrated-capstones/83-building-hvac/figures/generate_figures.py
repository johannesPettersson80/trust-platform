#!/usr/bin/env python3
"""Render Lesson 83's figures.

Pure Python, no third-party dependencies.

- figures/building-hvac-capstone-evidence.svg is rebuilt from the retained
  native trace figures/building-hvac-capstone-evidence-trace.csv, keeping the
  same two panels, series, and axes the shipped figure uses.
- figures/building-hvac-capstone-topology.svg is a structural box-and-arrow
  diagram of the two coupled zones, their shared controller, and the
  forecast and outdoor-loss paths. It is not derived from CSV data (it has
  no panels, series, or axes to reproduce); it is written directly in the
  visual style of examples/control/01-feedback-foundations-and-temperature-
  control/08-water-heater-saturation/figures/heater-signal-path.svg.
"""

from __future__ import annotations

import csv
from pathlib import Path

ROOT = Path(__file__).resolve().parent

# --- Evidence chart -------------------------------------------------------

PANELS = [
    {
        "name": "Temperature integrated absolute error",
        "series": "Absolute error",
        "axis_title": "Temperature integrated absolute error (degC*s)",
        "y_min": 0.0,
        "y_max": 240.0,
        "colour": "#2563eb",
        "top": 195.0,
        "bottom": 355.0,
        "x_axis_label_y": 391.0,
        "legend_x": 55,
    },
    {
        "name": "Applied-heater movement",
        "series": "Movement",
        "axis_title": "Applied-heater movement (percentage points)",
        "y_min": 0.0,
        "y_max": 180.0,
        "colour": "#dc2626",
        "top": 445.0,
        "bottom": 605.0,
        "x_axis_label_y": 641.0,
        "legend_x": 220,
    },
]

LEFT, RIGHT = 75.0, 855.0
X_MIN, X_MAX = -0.10, 2.10


def read_trace() -> dict[str, list[tuple[float, float]]]:
    with (ROOT / "building-hvac-capstone-evidence-trace.csv").open(newline="", encoding="utf-8") as handle:
        rows = list(csv.DictReader(handle))
    by_panel: dict[str, list[tuple[float, float]]] = {}
    for row in rows:
        by_panel.setdefault(row["panel"], []).append((float(row["x"]), float(row["y"])))
    return by_panel


def sx(index: float) -> float:
    return LEFT + (index - X_MIN) / (X_MAX - X_MIN) * (RIGHT - LEFT)


def sy(value: float, y_min: float, y_max: float, top: float, bottom: float) -> float:
    return bottom - (value - y_min) / (y_max - y_min) * (bottom - top)


def render_evidence(by_panel: dict[str, list[tuple[float, float]]]) -> str:
    body: list[str] = []
    for panel in PANELS:
        lx = panel["legend_x"]
        body.append(
            f'<line x1="{lx}" y1="146" x2="{lx + 28}" y2="146" stroke="{panel["colour"]}" stroke-width="3"/>'
            f'<text x="{lx + 34}" y="151" font-size="12">{panel["series"]}</text>'
        )
    for panel in PANELS:
        points = by_panel[panel["name"]]
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
        for index, value in points:
            body.append(
                f'<circle cx="{sx(index):.2f}" cy="{sy(value, y_min, y_max, top, bottom):.2f}" r="4" fill="{panel["colour"]}"/>'
            )
        body.append(f'<rect class="axis" x="75" y="{top}" width="780" height="{bottom - top}" fill="none"/>')
        body.append(f'<text x="465" y="{panel["x_axis_label_y"]:.2f}" text-anchor="middle" font-size="12">architecture index</text>')

    return f'''<svg xmlns="http://www.w3.org/2000/svg" role="img" aria-labelledby="title desc" viewBox="0 0 900 650">
<title id="title">Predictive control does not earn application when actuator movement fails the requirement</title>
<desc id="desc">Index 0 is strong PI, index 1 balanced MPC, and index 2 actuator-conservative MPC; every candidate meets comfort, but both MPC designs exceed the movement limit.</desc>
<defs><marker id="arrow" markerWidth="8" markerHeight="8" refX="7" refY="3" orient="auto"><path d="M0,0 L0,6 L8,3 z" fill="#243044"/></marker></defs>
<rect width="900" height="650" fill="#f7f9fc"/>
<style>text{{font-family:system-ui,-apple-system,sans-serif;fill:#243044}} .axis{{stroke:#526078;stroke-width:1}} .grid{{stroke:#d8e0eb;stroke-width:1}}</style>
<text x="55" y="42" font-size="24" font-weight="700">Predictive control does not earn application when actuator</text>
<text x="55" y="70" font-size="24" font-weight="700">movement fails the requirement</text>
<text x="55" y="98" font-size="14">Index 0 is strong PI, index 1 balanced MPC, and index 2 actuator-conservative MPC; every</text>
<text x="55" y="116" font-size="14">candidate meets comfort, but both MPC designs exceed the movement limit.</text>
{"".join(body)}
</svg>
'''


# --- Topology diagram (structural, not CSV-derived) ------------------------

def render_topology() -> str:
    return '''<svg xmlns="http://www.w3.org/2000/svg" width="1120" height="480" viewBox="0 0 1120 480" role="img" aria-labelledby="title desc">
  <title id="title">Two coupled zones share one controller and one heating-power limit</title>
  <desc id="desc">Weather, occupancy, and tariff forecasts feed the controller, which sends independent heater commands to two thermally coupled zones and reads both measured temperatures back; each zone also loses heat outdoors and exchanges heat with the other zone.</desc>
  <defs>
    <marker id="arrow-blue" viewBox="0 0 10 10" refX="9" refY="5" markerWidth="7" markerHeight="7" orient="auto-start-reverse">
      <path d="M 0 0 L 10 5 L 0 10 z" fill="#2563eb"/>
    </marker>
    <marker id="arrow-green" viewBox="0 0 10 10" refX="9" refY="5" markerWidth="7" markerHeight="7" orient="auto-start-reverse">
      <path d="M 0 0 L 10 5 L 0 10 z" fill="#16a34a"/>
    </marker>
    <marker id="arrow-gray" viewBox="0 0 10 10" refX="9" refY="5" markerWidth="7" markerHeight="7" orient="auto-start-reverse">
      <path d="M 0 0 L 10 5 L 0 10 z" fill="#475569"/>
    </marker>
    <marker id="arrow-orange" viewBox="0 0 10 10" refX="9" refY="5" markerWidth="7" markerHeight="7" orient="auto-start-reverse">
      <path d="M 0 0 L 10 5 L 0 10 z" fill="#ea580c"/>
    </marker>
  </defs>

  <rect width="1120" height="480" rx="18" fill="#f8fafc"/>
  <text x="48" y="42" fill="#0f172a" font-family="system-ui, sans-serif" font-size="22" font-weight="700">One controller commands two thermally coupled zones</text>
  <text x="48" y="66" fill="#475569" font-family="system-ui, sans-serif" font-size="15">Forecasts inform the decision; outdoor loss and inter-zone exchange act on both zones regardless of the controller.</text>

  <g font-family="system-ui, sans-serif" text-anchor="middle">
    <rect x="40" y="112" width="260" height="72" rx="10" fill="#f8fafc" stroke="#64748b" stroke-width="2"/>
    <text x="170" y="142" fill="#0f172a" font-size="16" font-weight="700">Weather, occupancy,</text>
    <text x="170" y="163" fill="#0f172a" font-size="16" font-weight="700">tariff forecast</text>

    <rect x="400" y="112" width="320" height="72" rx="10" fill="#eff6ff" stroke="#2563eb" stroke-width="2"/>
    <text x="560" y="142" fill="#0f172a" font-size="17" font-weight="700">Controller</text>
    <text x="560" y="163" fill="#1d4ed8" font-size="14">qualified PI or finite-grid MPC</text>

    <rect x="90" y="290" width="280" height="86" rx="10" fill="#f0fdf4" stroke="#16a34a" stroke-width="2"/>
    <text x="230" y="326" fill="#0f172a" font-size="18" font-weight="700">Zone 1</text>
    <text x="230" y="348" fill="#166534" font-size="14">measured temperature T1</text>

    <rect x="750" y="290" width="280" height="86" rx="10" fill="#f0fdf4" stroke="#16a34a" stroke-width="2"/>
    <text x="890" y="326" fill="#0f172a" font-size="18" font-weight="700">Zone 2</text>
    <text x="890" y="348" fill="#166534" font-size="14">measured temperature T2</text>

    <rect x="470" y="392" width="180" height="64" rx="10" fill="#fff7ed" stroke="#ea580c" stroke-width="2"/>
    <text x="560" y="418" fill="#0f172a" font-size="15" font-weight="700">Outdoor</text>
    <text x="560" y="437" fill="#9a3412" font-size="13">temperature</text>
  </g>

  <path d="M300 148 H393" fill="none" stroke="#475569" stroke-width="3" marker-end="url(#arrow-gray)"/>
  <text x="346" y="133" fill="#334155" font-family="system-ui, sans-serif" font-size="13" font-weight="600" text-anchor="middle">three-step forecast</text>

  <path d="M480 184 V240 H230 V283" fill="none" stroke="#2563eb" stroke-width="3" marker-end="url(#arrow-blue)"/>
  <text x="300" y="234" fill="#1d4ed8" font-family="system-ui, sans-serif" font-size="13" font-weight="600" text-anchor="middle">heater 1 command</text>

  <path d="M640 184 V240 H890 V283" fill="none" stroke="#2563eb" stroke-width="3" marker-end="url(#arrow-blue)"/>
  <text x="820" y="234" fill="#1d4ed8" font-family="system-ui, sans-serif" font-size="13" font-weight="600" text-anchor="middle">heater 2 command</text>

  <path d="M270 290 V260 H440 V184" fill="none" stroke="#16a34a" stroke-width="3" marker-end="url(#arrow-green)"/>
  <path d="M850 290 V268 H680 V184" fill="none" stroke="#16a34a" stroke-width="3" marker-end="url(#arrow-green)"/>
  <text x="560" y="256" fill="#166534" font-family="system-ui, sans-serif" font-size="13" font-weight="600" text-anchor="middle">measured T1, T2 feedback</text>

  <path d="M370 333 H750" fill="none" stroke="#475569" stroke-width="3" marker-start="url(#arrow-gray)" marker-end="url(#arrow-gray)"/>
  <text x="560" y="322" fill="#334155" font-family="system-ui, sans-serif" font-size="13" font-weight="600" text-anchor="middle">inter-zone heat exchange</text>

  <path d="M505 392 L280 379" fill="none" stroke="#ea580c" stroke-width="3" marker-end="url(#arrow-orange)"/>
  <path d="M615 392 L840 379" fill="none" stroke="#ea580c" stroke-width="3" marker-end="url(#arrow-orange)"/>
  <text x="345" y="463" fill="#9a3412" font-family="system-ui, sans-serif" font-size="13" font-weight="600" text-anchor="middle">outdoor heat loss</text>
  <text x="775" y="463" fill="#9a3412" font-family="system-ui, sans-serif" font-size="13" font-weight="600" text-anchor="middle">outdoor heat loss</text>
</svg>
'''


def main() -> None:
    by_panel = read_trace()
    (ROOT / "building-hvac-capstone-evidence.svg").write_text(render_evidence(by_panel), encoding="utf-8")
    (ROOT / "building-hvac-capstone-topology.svg").write_text(render_topology(), encoding="utf-8")


if __name__ == "__main__":
    main()
