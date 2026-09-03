#!/usr/bin/env python3
"""Render Lesson 36's topology diagram.

main() writes figures/two-tank-state-space-topology.svg -- pump, upstream
tank, inter-tank flow, downstream tank, drain; no time-series data source.
(render_hidden_prediction() below can still regenerate
figures/two-tank-hidden-prediction.svg -- the pre-commit prompt, from
two-tank-hidden-prediction-summary.csv -- but it is not wired into main().)

figures/two-tank-state-space.svg is owned by the native test
control_curriculum_modelling_progression
(crates/trust-runtime/tests/control_curriculum_modelling_progression.rs).
Refresh it with: TRUST_UPDATE_CONTROL_CURRICULUM_ARTIFACTS=1 cargo test -p trust-runtime --test control_curriculum_modelling_progression
"""

from __future__ import annotations

import csv
from pathlib import Path

ROOT = Path(__file__).resolve().parent
INK = "#243044"
MUTED = "#526078"
GRID = "#d8e0eb"
PAPER = "#f7f9fc"
BLUE = "#0072b2"
ORANGE = "#d55e00"
GREEN = "#009e73"


def render_hidden_prediction() -> str:
    with (ROOT / "two-tank-hidden-prediction-summary.csv").open(newline="", encoding="utf-8") as handle:
        record = next(csv.DictReader(handle))
    case_a = float(record["visible_case_a_downstream_cm"])
    case_b = float(record["visible_case_b_downstream_cm"])

    def case_block(x: float, label: str, value: float) -> str:
        return (
            f'<text x="{x + 110:.0f}" y="112" text-anchor="middle" font-size="18" font-weight="700">{label}</text>'
            f'<rect x="{x:.0f}" y="135" width="220" height="85" rx="8" class="hidden"/>'
            f'<text x="{x + 110:.0f}" y="183" text-anchor="middle" font-size="16">upstream state hidden</text>'
            f'<line x1="{x + 110:.0f}" y1="220" x2="{x + 110:.0f}" y2="255" class="line"/>'
            f'<rect x="{x:.0f}" y="255" width="220" height="85" rx="8" class="tank"/>'
            f'<text x="{x + 110:.0f}" y="296" text-anchor="middle" font-size="16">downstream level</text>'
            f'<text x="{x + 110:.0f}" y="322" text-anchor="middle" font-size="20" font-weight="700">{value:.3f} cm</text>'
        )

    body = case_block(135, "Case A", case_a) + case_block(545, "Case B", case_b)
    return f'''<svg xmlns="http://www.w3.org/2000/svg" role="img" aria-labelledby="title desc" data-stage="pre-commit" viewBox="0 0 900 420">
<title id="title">Commit a prediction from identical visible readings</title>
<desc id="desc">Cases A and B both show zero centimetres downstream. Their upstream states and future traces remain hidden.</desc>
<rect width="900" height="420" fill="{PAPER}"/>
<style>text{{font-family:system-ui,-apple-system,sans-serif;fill:{INK}}}.tank{{fill:#eaf2fb;stroke:#526078;stroke-width:2}}.hidden{{fill:#eef0f4;stroke:#8792a5;stroke-width:2;stroke-dasharray:7 5}}.line{{stroke:#526078;stroke-width:3}}.blank{{fill:#fff;stroke:#8792a5;stroke-width:1.5}}</style>
<text x="55" y="45" font-size="24" font-weight="700">Do identical current outputs imply identical future outputs?</text>
<text x="55" y="72" font-size="14">Only the current downstream level is visible. Record your prediction before opening the reveal.</text>
{body}
<rect x="135" y="365" width="630" height="36" rx="5" class="blank"/><text x="150" y="389" font-size="14">Prediction and reason:</text>
</svg>
'''


def render_topology() -> str:
    body: list[str] = [
        f'<line x1="30" y1="120" x2="90" y2="120" stroke="{BLUE}" stroke-width="3" marker-end="url(#arrow-blue)"/>',
        f'<text x="20" y="108" font-size="13" fill="{BLUE}">pump inlet (u)</text>',
        '<rect x="90" y="70" width="130" height="150" rx="8" fill="#eaf2fb" stroke="#526078" stroke-width="2"/>',
        '<text x="155" y="55" text-anchor="middle" font-size="15" font-weight="700">Upstream tank</text>',
        '<text x="155" y="150" text-anchor="middle" font-size="13">level h1</text>',
        f'<line x1="220" y1="150" x2="290" y2="150" stroke="{ORANGE}" stroke-width="3" marker-end="url(#arrow-orange)"/>',
        f'<text x="222" y="140" font-size="12" fill="{ORANGE}">inter-tank flow (k12·h1)</text>',
        '<rect x="290" y="90" width="130" height="150" rx="8" fill="#eaf2fb" stroke="#526078" stroke-width="2"/>',
        '<text x="355" y="75" text-anchor="middle" font-size="15" font-weight="700">Downstream tank</text>',
        '<text x="355" y="170" text-anchor="middle" font-size="13">level h2</text>',
        f'<line x1="420" y1="200" x2="480" y2="200" stroke="{GREEN}" stroke-width="3" marker-end="url(#arrow-green)"/>',
        f'<text x="422" y="190" font-size="12" fill="{GREEN}">drain (k2·h2)</text>',
        '<text x="355" y="245" text-anchor="middle" font-size="12" fill="#8792a5">level transmitter (measured)</text>',
        '<text x="155" y="240" text-anchor="middle" font-size="12" fill="#8792a5">level transmitter (measured)</text>',
    ]
    return f'''<svg xmlns="http://www.w3.org/2000/svg" width="520" height="270" viewBox="0 0 520 270" role="img" aria-labelledby="title2 desc2">
<title id="title2">Pump, upstream tank, inter-tank flow, downstream tank, drain</title>
<desc id="desc2">An inlet pump feeds the upstream tank. Inter-tank flow carries water into the downstream tank, which drains to waste. Both tanks have level transmitters.</desc>
<defs>
  <marker id="arrow-blue" viewBox="0 0 10 10" refX="9" refY="5" markerWidth="7" markerHeight="7" orient="auto-start-reverse"><path d="M 0 0 L 10 5 L 0 10 z" fill="{BLUE}"/></marker>
  <marker id="arrow-orange" viewBox="0 0 10 10" refX="9" refY="5" markerWidth="7" markerHeight="7" orient="auto-start-reverse"><path d="M 0 0 L 10 5 L 0 10 z" fill="{ORANGE}"/></marker>
  <marker id="arrow-green" viewBox="0 0 10 10" refX="9" refY="5" markerWidth="7" markerHeight="7" orient="auto-start-reverse"><path d="M 0 0 L 10 5 L 0 10 z" fill="{GREEN}"/></marker>
</defs>
<rect width="520" height="270" rx="14" fill="{PAPER}" stroke="{GRID}"/>
<g font-family="system-ui, sans-serif" fill="{INK}">
{"".join(body)}
</g>
</svg>
'''


def main() -> None:
    (ROOT / "two-tank-state-space-topology.svg").write_text(render_topology(), encoding="utf-8")


if __name__ == "__main__":
    main()
