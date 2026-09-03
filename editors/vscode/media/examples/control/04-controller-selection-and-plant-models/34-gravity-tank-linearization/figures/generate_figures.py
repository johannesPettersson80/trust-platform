#!/usr/bin/env python3
"""Render Lesson 34's topology diagram.

Writes figures/gravity-tank-linearization-topology.svg (pump inlet at the
top, fixed gravity outlet at the bottom); that diagram has no time-series
data source.

figures/gravity-tank-linearization.svg is owned by the native test
control_curriculum_modelling_progression
(crates/trust-runtime/tests/control_curriculum_modelling_progression.rs).
Refresh it with: TRUST_UPDATE_CONTROL_CURRICULUM_ARTIFACTS=1 cargo test -p trust-runtime --test control_curriculum_modelling_progression
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
        '<path d="M120 90 L120 320 L380 320 L380 90" fill="#eaf2fb" stroke="#526078" stroke-width="2.5"/>',
        '<line x1="120" y1="150" x2="380" y2="150" stroke="#1268b3" stroke-width="1.5" stroke-dasharray="5 4"/>',
        '<text x="392" y="154" font-size="13" fill="#1268b3">level (h)</text>',
        f'<line x1="60" y1="70" x2="130" y2="70" stroke="{BLUE}" stroke-width="3" marker-end="url(#arrow-blue)"/>',
        '<line x1="130" y1="70" x2="130" y2="110" stroke="' + BLUE + '" stroke-width="3" marker-end="url(#arrow-blue)"/>',
        f'<text x="30" y="60" font-size="14" font-weight="700" fill="{BLUE}">variable-speed pump</text>',
        f'<text x="30" y="78" font-size="13" fill="{BLUE}">inlet flow (q)</text>',
        '<rect x="230" y="320" width="40" height="18" fill="#526078"/>',
        f'<line x1="250" y1="338" x2="250" y2="390" stroke="{ORANGE}" stroke-width="3" marker-end="url(#arrow-orange)"/>',
        f'<text x="258" y="360" font-size="13" fill="{ORANGE}">gravity outlet flow</text>',
        f'<text x="258" y="378" font-size="13" fill="{ORANGE}">(c·sqrt(h)), fixed area</text>',
    ]
    return f'''<svg xmlns="http://www.w3.org/2000/svg" width="620" height="420" viewBox="0 0 620 420" role="img" aria-labelledby="title2 desc2">
<title id="title2">Pump inlet at the top, gravity outlet at the bottom</title>
<desc id="desc2">A variable-speed pump supplies the tank from above; liquid drains through a fixed-area outlet at the bottom under gravity, so outlet flow grows with the square root of level.</desc>
<defs>
  <marker id="arrow-blue" viewBox="0 0 10 10" refX="9" refY="5" markerWidth="7" markerHeight="7" orient="auto-start-reverse"><path d="M 0 0 L 10 5 L 0 10 z" fill="{BLUE}"/></marker>
  <marker id="arrow-orange" viewBox="0 0 10 10" refX="9" refY="5" markerWidth="7" markerHeight="7" orient="auto-start-reverse"><path d="M 0 0 L 10 5 L 0 10 z" fill="{ORANGE}"/></marker>
</defs>
<rect width="620" height="420" rx="14" fill="{PAPER}" stroke="{GRID}"/>
<text x="40" y="410" font-family="system-ui, sans-serif" font-size="13" fill="{MUTED}">Outlet flow depends on level: doubling level does not double outlet flow.</text>
<g font-family="system-ui, sans-serif" fill="{INK}">
{"".join(body)}
</g>
</svg>
'''


def main() -> None:
    (ROOT / "gravity-tank-linearization-topology.svg").write_text(render_topology(), encoding="utf-8")


if __name__ == "__main__":
    main()
