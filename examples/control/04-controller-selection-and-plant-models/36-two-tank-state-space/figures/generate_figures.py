#!/usr/bin/env python3
"""Render Lesson 36's figures from retained native data.

Writes:
  * two-tank-state-space.svg            -- from two-tank-state-space-trace.csv
  * two-tank-hidden-prediction.svg      -- from two-tank-hidden-prediction-summary.csv
    (the pre-commit prompt; by definition it shows only the two identical
    zero downstream readings and no future trace)
  * two-tank-state-space-topology.svg   -- pump, upstream tank, inter-tank
    flow, downstream tank, drain; no time-series data source.
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
               series: list[tuple[list[float], list[float], str, str]]) -> None:
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
    for xs, ys, colour, dash in series:
        pts = " ".join(f"{sx(x):.2f},{sy(y):.2f}" for x, y in zip(xs, ys))
        dash_attr = f' stroke-dasharray="{dash}"' if dash else ""
        body.append(f'<polyline fill="none" stroke="{colour}" stroke-width="2.2"{dash_attr} points="{pts}"/>')
    body.append(f'<rect class="axis" x="{left:.2f}" y="{top:.2f}" width="{right - left:.2f}" height="{bottom - top:.2f}" fill="none"/>')


def legend_row(body: list[str], y: float, items: list[tuple[str, str, str]]) -> None:
    x = 55.0
    for label, colour, dash in items:
        dash_attr = f' stroke-dasharray="{dash}"' if dash else ""
        body.append(f'<line x1="{x:.1f}" y1="{y}" x2="{x + 28:.1f}" y2="{y}" stroke="{colour}" stroke-width="3"{dash_attr}/>')
        body.append(f'<text x="{x + 34:.1f}" y="{y + 5}" font-size="12">{label}</text>')
        x += 34 + 7 * len(label) + 26


def render_results() -> str:
    with (ROOT / "two-tank-state-space-trace.csv").open(newline="", encoding="utf-8") as handle:
        rows = list(csv.DictReader(handle))
    t = [float(r["time_s"]) for r in rows]
    upstream = [float(r["upstream_level"]) for r in rows]
    downstream = [float(r["downstream_level"]) for r in rows]
    empty = [float(r["empty_upstream_state"]) for r in rows]
    charged = [float(r["charged_upstream_state"]) for r in rows]
    x_ticks = [round(i * max(t) / 6, 1) for i in range(7)]

    body: list[str] = []
    legend_row(body, 101, [
        ("upstream level", BLUE, ""), ("downstream level", ORANGE, ""),
        ("empty upstream state", GREEN, "7 5"), ("charged upstream state", PINK, ""),
    ])
    y1 = nice_ticks(0.0, max(max(upstream), max(downstream)))
    draw_panel(body, 70, 860, 155, 315, "Main two-tank response (cm)", y1, x_ticks, 0, max(t), [
        (t, upstream, BLUE, ""), (t, downstream, ORANGE, ""),
    ])
    y2 = nice_ticks(0.0, max(max(empty), max(charged)))
    draw_panel(body, 70, 860, 360, 520, "Zero-input downstream reveal (cm)", y2, x_ticks, 0, max(t), [
        (t, empty, GREEN, "7 5"), (t, charged, PINK, ""),
    ])
    body.append('<text x="465" y="598" text-anchor="middle" font-size="12">time (s)</text>')

    return f'''<svg xmlns="http://www.w3.org/2000/svg" role="img" aria-labelledby="title desc" viewBox="0 0 900 610">
<title id="title">One output can hide different internal states</title>
<desc id="desc">Configured Simulation evidence · both hidden cases start at zero downstream level</desc>
<rect width="900" height="610" fill="{PAPER}"/>
<style>{STYLE}</style>
<text x="55" y="42" font-size="24" font-weight="700">One output can hide different internal states</text>
<text x="55" y="68" font-size="14">Configured Simulation evidence · both hidden cases start at zero downstream level</text>
{"".join(body)}
</svg>
'''


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
    (ROOT / "two-tank-state-space.svg").write_text(render_results(), encoding="utf-8")
    (ROOT / "two-tank-hidden-prediction.svg").write_text(render_hidden_prediction(), encoding="utf-8")
    (ROOT / "two-tank-state-space-topology.svg").write_text(render_topology(), encoding="utf-8")


if __name__ == "__main__":
    main()
