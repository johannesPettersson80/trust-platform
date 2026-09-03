#!/usr/bin/env python3
"""Render Lesson 33's topology diagram.

Writes figures/tank-heater-conservation-topology.svg (labeled inlet/outlet
flow arrows for the tank and heater-power/heat-loss arrows for the
vessel); that diagram has no time-series data source.

figures/tank-heater-conservation.svg is owned by the native test
control_curriculum_selection
(crates/trust-runtime/tests/control_curriculum_selection.rs). Refresh it
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


def render_topology() -> str:
    body: list[str] = [
        '<rect x="55" y="140" width="250" height="230" rx="10" fill="#eaf2fb" stroke="#526078" stroke-width="2"/>',
        '<text x="180" y="172" text-anchor="middle" font-size="18" font-weight="700">Buffer tank</text>',
        f'<line x1="20" y1="200" x2="105" y2="200" stroke="{BLUE}" stroke-width="3" marker-end="url(#arrow-blue)"/>',
        f'<text x="25" y="192" font-size="13" fill="{BLUE}">inlet flow (q_in)</text>',
        f'<line x1="255" y1="330" x2="340" y2="330" stroke="{ORANGE}" stroke-width="3" marker-end="url(#arrow-orange)"/>',
        f'<text x="255" y="322" font-size="13" fill="{ORANGE}">outlet flow (q_out)</text>',
        '<rect x="90" y="230" width="180" height="90" fill="#dbeafe" stroke="#526078" stroke-width="1.5"/>',
        '<text x="180" y="280" text-anchor="middle" font-size="14">stored volume (level h)</text>',

        '<rect x="405" y="140" width="290" height="230" rx="10" fill="#fff7ed" stroke="#9a3412" stroke-width="2"/>',
        '<text x="550" y="172" text-anchor="middle" font-size="18" font-weight="700">Heated vessel</text>',
        f'<line x1="370" y1="330" x2="450" y2="330" stroke="{ORANGE}" stroke-width="3" marker-end="url(#arrow-orange)"/>',
        f'<text x="365" y="322" font-size="13" fill="{ORANGE}">heater power (P)</text>',
        f'<line x1="650" y1="180" x2="730" y2="180" stroke="#8792a5" stroke-width="3" marker-end="url(#arrow-gray)"/>',
        f'<text x="655" y="172" font-size="13" fill="{MUTED}">heat loss to ambient (H)</text>',
        '<rect x="440" y="230" width="220" height="90" fill="#fef3e2" stroke="#9a3412" stroke-width="1.5"/>',
        '<text x="550" y="280" text-anchor="middle" font-size="14">stored energy (temperature T)</text>',
    ]
    return f'''<svg xmlns="http://www.w3.org/2000/svg" width="800" height="400" viewBox="0 0 800 400" role="img" aria-labelledby="title2 desc2">
<title id="title2">Labeled flow and heat paths for the tank and vessel</title>
<desc id="desc2">Inlet flow adds stored volume and outlet flow removes it in the buffer tank; heater power adds stored energy and heat loss to ambient removes it in the heated vessel.</desc>
<defs>
  <marker id="arrow-blue" viewBox="0 0 10 10" refX="9" refY="5" markerWidth="7" markerHeight="7" orient="auto-start-reverse"><path d="M 0 0 L 10 5 L 0 10 z" fill="{BLUE}"/></marker>
  <marker id="arrow-orange" viewBox="0 0 10 10" refX="9" refY="5" markerWidth="7" markerHeight="7" orient="auto-start-reverse"><path d="M 0 0 L 10 5 L 0 10 z" fill="{ORANGE}"/></marker>
  <marker id="arrow-gray" viewBox="0 0 10 10" refX="9" refY="5" markerWidth="7" markerHeight="7" orient="auto-start-reverse"><path d="M 0 0 L 10 5 L 0 10 z" fill="#8792a5"/></marker>
</defs>
<rect width="800" height="400" rx="14" fill="{PAPER}" stroke="{GRID}"/>
<text x="48" y="45" font-family="system-ui, sans-serif" font-size="20" font-weight="700" fill="{INK}">Accumulation equals rate entering minus rate leaving</text>
<g font-family="system-ui, sans-serif" fill="{INK}">
{"".join(body)}
</g>
</svg>
'''


def main() -> None:
    (ROOT / "tank-heater-conservation-topology.svg").write_text(render_topology(), encoding="utf-8")


if __name__ == "__main__":
    main()
