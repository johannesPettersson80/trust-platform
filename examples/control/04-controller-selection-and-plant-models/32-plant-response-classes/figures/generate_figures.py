#!/usr/bin/env python3
"""Render Lesson 32's topology diagram.

Writes figures/plant-response-classes-topology.svg, which has no
time-series data source.

figures/plant-response-classes.svg and
figures/plant-response-classes-first-second.svg are owned by the native
test control_curriculum_selection
(crates/trust-runtime/tests/control_curriculum_selection.rs). Refresh them
with: TRUST_UPDATE_CONTROL_CURRICULUM_ARTIFACTS=1 cargo test -p trust-runtime --test control_curriculum_selection
"""

from __future__ import annotations

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
    (ROOT / "plant-response-classes-topology.svg").write_text(render_topology(), encoding="utf-8")


if __name__ == "__main__":
    main()
