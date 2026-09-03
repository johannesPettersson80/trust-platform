#!/usr/bin/env python3
"""Render Lesson 35's topology diagram.

main() writes figures/pole-zero-plant-classes-topology.svg -- simple
schematics of the crane, magnetic bearing, and boiler drum; that diagram
has no time-series data source. (render_map() below can still regenerate
figures/pole-zero-plant-classes-map.svg from
pole-zero-plant-classes-matches.csv -- poles/zeros only, the equipment
identity columns are never read into the rendered worksheet -- but it is
not wired into main().)

figures/pole-zero-plant-classes.svg is owned by the native test
control_curriculum_modelling_progression
(crates/trust-runtime/tests/control_curriculum_modelling_progression.rs).
Refresh it with: TRUST_UPDATE_CONTROL_CURRICULUM_ARTIFACTS=1 cargo test -p trust-runtime --test control_curriculum_modelling_progression
"""

from __future__ import annotations

import csv
import re
from pathlib import Path

ROOT = Path(__file__).resolve().parent
INK = "#243044"
MUTED = "#526078"
GRID = "#d8e0eb"
PAPER = "#f7f9fc"
BLUE = "#0072b2"
ORANGE = "#d55e00"


def parse_complex(token: str) -> tuple[float, float]:
    token = token.strip()
    match = re.match(r"^([+-]?\d+\.?\d*)?([+-]j\d+\.?\d*)$", token)
    if match:
        real = float(match.group(1)) if match.group(1) else 0.0
        imag_token = match.group(2)
        sign = -1.0 if imag_token[0] == "-" else 1.0
        imag = sign * float(imag_token[2:])
        return real, imag
    return float(token), 0.0


REAL_SCALE = 115.0  # px per unit (1/s)
IMAG_SCALE = 17.5  # px per unit (1/s)


def draw_map_panel(body: list[str], x: float, y: float, label: str,
                    poles: list[tuple[float, float]], zeros: list[tuple[float, float]]) -> None:
    width, height = 230.0, 175.0
    axis_x = x + 0.75 * width
    axis_y = y + 0.5 * height
    body.append(f'<text x="{x:.1f}" y="{y - 12.0:.1f}" font-size="16" font-weight="700">Map {label}</text>')
    body.append(f'<rect x="{x:.1f}" y="{y:.1f}" width="{width:.1f}" height="{height:.1f}" class="frame"/>')
    body.append(f'<line x1="{x + 0.25 * width:.1f}" y1="{y:.1f}" x2="{x + 0.25 * width:.1f}" y2="{y + height:.1f}" class="grid"/>')
    body.append(f'<line x1="{axis_x:.1f}" y1="{y:.1f}" x2="{axis_x:.1f}" y2="{y + height:.1f}" class="axis"/>')
    body.append(f'<line x1="{x:.1f}" y1="{y + 0.9 * height:.1f}" x2="{x + width:.1f}" y2="{y + 0.9 * height:.1f}" class="grid"/>')
    body.append(f'<line x1="{x:.1f}" y1="{axis_y:.1f}" x2="{x + width:.1f}" y2="{axis_y:.1f}" class="axis"/>')
    body.append(f'<line x1="{x:.1f}" y1="{y + 0.1 * height:.1f}" x2="{x + width:.1f}" y2="{y + 0.1 * height:.1f}" class="grid"/>')
    body.append(f'<text x="{x + width:.1f}" y="{y + height - 6.0:.1f}" font-size="11" text-anchor="end">Re(s)</text>')
    body.append(f'<text x="{axis_x + 5.0:.1f}" y="{y + 13.0:.1f}" font-size="11">Im(s)</text>')
    for real, imag in poles:
        cx = axis_x + real * REAL_SCALE
        cy = axis_y - imag * IMAG_SCALE
        body.append(
            f'<g data-symbol="pole" class="pole">'
            f'<line x1="{cx - 6:.1f}" y1="{cy - 6:.1f}" x2="{cx + 6:.1f}" y2="{cy + 6:.1f}"/>'
            f'<line x1="{cx - 6:.1f}" y1="{cy + 6:.1f}" x2="{cx + 6:.1f}" y2="{cy - 6:.1f}"/></g>'
        )
    for real, imag in zeros:
        cx = axis_x + real * REAL_SCALE
        cy = axis_y - imag * IMAG_SCALE
        body.append(f'<circle data-symbol="zero" cx="{cx:.1f}" cy="{cy:.1f}" r="7" class="zero"/>')


def render_map() -> str:
    with (ROOT / "pole-zero-plant-classes-matches.csv").open(newline="", encoding="utf-8") as handle:
        maps = list(csv.DictReader(handle))
    body: list[str] = []
    positions = [(55.0, 115.0), (335.0, 115.0), (615.0, 115.0), (55.0, 360.0), (335.0, 360.0), (615.0, 360.0)]
    for (x, y), row in zip(positions, maps):
        poles = [parse_complex(tok) for tok in row["poles_per_s"].split(";")]
        zeros = [] if row["zeros_per_s"].strip().lower() == "none" else [parse_complex(tok) for tok in row["zeros_per_s"].split(";")]
        draw_map_panel(body, x, y, row["map_id"], poles, zeros)

    return f'''<svg xmlns="http://www.w3.org/2000/svg" role="img" aria-labelledby="title desc" viewBox="0 0 900 620">
<title id="title">Hidden pole-zero matching worksheet</title>
<desc id="desc">Six unlabeled complex-plane maps generated from the declared executable model parameters. Crosses are poles and circles are zeros.</desc>
<rect width="900" height="620" fill="{PAPER}"/>
<style>text{{font-family:system-ui,-apple-system,sans-serif;fill:{INK}}}.axis{{stroke:#526078;stroke-width:1.2}}.grid{{stroke:{GRID};stroke-width:1}}.pole{{stroke:#b43c32;stroke-width:3}}.zero{{stroke:{BLUE};stroke-width:3;fill:{PAPER}}}.frame{{stroke:#8792a5;stroke-width:1;fill:none}}</style>
<text x="55" y="42" font-size="24" font-weight="700">Match each pole-zero map to an executed response</text>
<text x="55" y="68" font-size="14">Cross = pole · circle = zero · locations use inverse seconds</text>
{"".join(body)}
</svg>
'''


def render_topology() -> str:
    body: list[str] = [
        # Overhead crane: trolley on a rail with a suspended payload.
        '<line x1="60" y1="90" x2="270" y2="90" stroke="#526078" stroke-width="4"/>',
        '<rect x="140" y="80" width="50" height="26" fill="#eff6ff" stroke="#1268b3" stroke-width="2"/>',
        '<line x1="165" y1="106" x2="165" y2="160" stroke="#526078" stroke-width="2" stroke-dasharray="4 3"/>',
        '<circle cx="165" cy="175" r="16" fill="#fff7ed" stroke="#d55e00" stroke-width="2"/>',
        '<text x="165" y="55" text-anchor="middle" font-size="15" font-weight="700">Overhead crane</text>',
        '<text x="165" y="205" text-anchor="middle" font-size="12" fill="#526078">trolley force -&gt; payload position</text>',

        # Magnetic bearing: rotor centered by opposing electromagnets.
        '<circle cx="450" cy="140" r="34" fill="#eff6ff" stroke="#1268b3" stroke-width="2"/>',
        '<rect x="405" y="128" width="20" height="24" fill="#fff7ed" stroke="#d55e00" stroke-width="2"/>',
        '<rect x="475" y="128" width="20" height="24" fill="#fff7ed" stroke="#d55e00" stroke-width="2"/>',
        '<text x="450" y="55" text-anchor="middle" font-size="15" font-weight="700">Magnetic bearing</text>',
        '<text x="450" y="205" text-anchor="middle" font-size="12" fill="#526078">coil force -&gt; rotor displacement</text>',

        # Boiler drum: feedwater in, steam out, level sensed.
        '<rect x="640" y="95" width="180" height="90" rx="16" fill="#eff6ff" stroke="#1268b3" stroke-width="2"/>',
        '<line x1="730" y1="95" x2="730" y2="185" stroke="#8792a5" stroke-width="1.5" stroke-dasharray="4 3"/>',
        '<line x1="600" y1="130" x2="645" y2="130" stroke="#d55e00" stroke-width="3" marker-end="url(#arrow-o)"/>',
        '<text x="598" y="122" text-anchor="end" font-size="12" fill="#d55e00">feedwater</text>',
        '<line x1="815" y1="105" x2="850" y2="90" stroke="#8792a5" stroke-width="3" marker-end="url(#arrow-g)"/>',
        '<text x="820" y="82" font-size="12" fill="#526078">steam out</text>',
        '<text x="730" y="55" text-anchor="middle" font-size="15" font-weight="700">Boiler drum</text>',
        '<text x="730" y="205" text-anchor="middle" font-size="12" fill="#526078">feedwater -&gt; drum level</text>',
    ]
    return f'''<svg xmlns="http://www.w3.org/2000/svg" width="900" height="230" viewBox="0 0 900 230" role="img" aria-labelledby="title2 desc2">
<title id="title2">Overhead crane, magnetic bearing, and boiler drum</title>
<desc id="desc2">Simple schematics of the three machines appearing for the first time in this course: an overhead crane with a suspended payload, a rotor centered by opposing electromagnet coils, and a boiler drum with feedwater in and steam out.</desc>
<defs>
  <marker id="arrow-o" viewBox="0 0 10 10" refX="9" refY="5" markerWidth="7" markerHeight="7" orient="auto-start-reverse"><path d="M 0 0 L 10 5 L 0 10 z" fill="{ORANGE}"/></marker>
  <marker id="arrow-g" viewBox="0 0 10 10" refX="9" refY="5" markerWidth="7" markerHeight="7" orient="auto-start-reverse"><path d="M 0 0 L 10 5 L 0 10 z" fill="#8792a5"/></marker>
</defs>
<rect width="900" height="230" rx="14" fill="{PAPER}" stroke="{GRID}"/>
<g font-family="system-ui, sans-serif" fill="{INK}">
{"".join(body)}
</g>
</svg>
'''


def main() -> None:
    (ROOT / "pole-zero-plant-classes-topology.svg").write_text(render_topology(), encoding="utf-8")


if __name__ == "__main__":
    main()
