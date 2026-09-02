#!/usr/bin/env python3
"""Regenerate Lesson 15 teaching figures from retained executable evidence."""

from __future__ import annotations

import csv
from pathlib import Path


ROOT = Path(__file__).resolve().parent
SEED = 20260824


def esc(text: object) -> str:
    return (str(text).replace("&", "&amp;").replace("<", "&lt;")
            .replace(">", "&gt;").replace('"', "&quot;"))


def svg_document(width: int, height: int, body: str, title: str, description: str) -> str:
    return f'''<svg xmlns="http://www.w3.org/2000/svg" width="{width}" height="{height}" viewBox="0 0 {width} {height}" role="img" aria-labelledby="title desc">
<title id="title">{esc(title)}</title>
<desc id="desc">{esc(description)}</desc>
<style>
  svg {{ color: #1f2937; background: #ffffff; font-family: system-ui, -apple-system, BlinkMacSystemFont, "Segoe UI", sans-serif; forced-color-adjust: none; }}
  .panel {{ fill: #f8fafc; stroke: #94a3b8; stroke-width: 1.5; }}
  .grid {{ stroke: #d7dee8; stroke-width: 1; }}
  .axis {{ stroke: #64748b; stroke-width: 1.3; }}
  .actual {{ fill: none; stroke: #1473e6; stroke-width: 3; }}
  .model {{ fill: none; stroke: #d97706; stroke-width: 2.4; stroke-dasharray: 9 5; }}
  .request {{ fill: none; stroke: #7c3aed; stroke-width: 3; }}
  .applied {{ fill: none; stroke: #059669; stroke-width: 2.2; stroke-dasharray: 7 4; }}
  .reference {{ stroke: #475569; stroke-width: 1.4; stroke-dasharray: 5 5; }}
  .event {{ stroke: #b91c1c; stroke-width: 1.4; stroke-dasharray: 4 4; }}
  .label {{ fill: currentColor; font-size: 15px; }}
  .small {{ fill: currentColor; font-size: 13px; }}
  .muted {{ fill: #52606d; font-size: 13px; }}
  .title {{ fill: currentColor; font-size: 23px; font-weight: 650; }}
  .subtitle {{ fill: #52606d; font-size: 14px; }}
  .heading {{ fill: currentColor; font-size: 17px; font-weight: 650; }}
  .water {{ fill: #dbeafe; stroke: #1473e6; stroke-width: 2; }}
  .heat {{ stroke: #dc2626; stroke-width: 4; fill: none; marker-end: url(#arrow-red); }}
  .loss {{ stroke: #64748b; stroke-width: 3; fill: none; marker-end: url(#arrow-grey); }}
  .point {{ fill: #ffffff; stroke: #1473e6; stroke-width: 3; }}
  @media (prefers-color-scheme: dark) {{
    svg {{ color: #e5e7eb; background: #1e1e1e; }}
    .panel {{ fill: #252526; stroke: #6b7280; }}
    .grid {{ stroke: #3f4650; }} .axis {{ stroke: #9ca3af; }}
    .muted, .subtitle {{ fill: #b8c0cc; }} .water {{ fill: #173a5e; }}
    .point {{ fill: #1e1e1e; }}
  }}
  @media (forced-colors: active) {{
    svg {{ color: CanvasText; background: Canvas; }}
    .panel, .water {{ fill: Canvas; stroke: CanvasText; }}
    .grid, .axis, .reference, .event, .actual, .model, .request, .applied, .heat, .loss {{ stroke: CanvasText; }}
    .label, .small, .muted, .title, .subtitle, .heading {{ fill: CanvasText; }}
    .point {{ fill: Canvas; stroke: CanvasText; }}
  }}
</style>
{body}
</svg>
'''


def energy_boundary() -> str:
    body = '''
<defs>
  <marker id="arrow-red" markerWidth="10" markerHeight="10" refX="8" refY="3" orient="auto"><path d="M0,0 L0,6 L9,3 z" fill="#dc2626"/></marker>
  <marker id="arrow-grey" markerWidth="10" markerHeight="10" refX="8" refY="3" orient="auto"><path d="M0,0 L0,6 L9,3 z" fill="#64748b"/></marker>
</defs>
<text x="40" y="42" class="title">Water-heater energy boundary</text>
<text x="40" y="68" class="subtitle">Explain the physical flows before writing the conservation equation</text>
<rect x="330" y="105" width="250" height="180" rx="18" class="panel"/>
<rect x="350" y="155" width="210" height="110" rx="10" class="water"/>
<text x="455" y="190" text-anchor="middle" class="heading">Well-mixed water</text>
<text x="455" y="218" text-anchor="middle" class="label">water temperature (T)</text>
<text x="455" y="235" text-anchor="middle" class="small">stored energy</text>
<text x="455" y="253" text-anchor="middle" class="small">thermal capacitance (C)</text>
<path d="M120 195 H330" class="heat"/>
<text x="120" y="150" class="heading">Electric heater</text>
<text x="120" y="174" class="small">applied command (MV_applied)</text>
<text x="150" y="218" class="small">heater heat input (Qin)</text>
<path d="M580 195 H790" class="loss"/>
<text x="630" y="150" class="heading">Surroundings + flow</text>
<text x="630" y="174" class="small">ambient temperature (Tambient)</text>
<text x="625" y="218" class="small">heat loss: H × (T - Tambient)</text>
<text x="40" y="326" class="label">Stored-energy rate = heater power entering - heat leaving</text>
<text x="860" y="326" text-anchor="end" class="muted">Explanatory diagram · not measured evidence</text>
'''
    return svg_document(
        900, 360, body, "Water-heater energy boundary",
        "A well-mixed water vessel stores thermal energy. Heater power enters from the left and ambient and recirculation heat loss leaves to the right.",
    )


def polyline(rows: list[dict[str, str]], x, y, key: str) -> str:
    return " ".join(f"{x(float(row['time_s'])):.2f},{y(float(row[key])):.2f}" for row in rows)


def response_figure(rows: list[dict[str, str]], summary: dict[str, str]) -> str:
    left, right = 78.0, 858.0
    top_a, bottom_a = 130.0, 410.0
    top_b, bottom_b = 500.0, 675.0
    x = lambda value: left + value / 30.1 * (right - left)
    y_temp = lambda value: bottom_a - (value - 19.5) / 5.0 * (bottom_a - top_a)
    y_power = lambda value: bottom_b - value / 25.0 * (bottom_b - top_b)
    grid = []
    for tick in (0, 5, 10, 15, 20, 25, 30):
        px = x(tick)
        grid.append(f'<line x1="{px:.2f}" y1="{top_a}" x2="{px:.2f}" y2="{bottom_a}" class="grid"/>')
        grid.append(f'<line x1="{px:.2f}" y1="{top_b}" x2="{px:.2f}" y2="{bottom_b}" class="grid"/>')
        grid.append(f'<text x="{px:.2f}" y="700" text-anchor="middle" class="small">{tick}</text>')
    for tick in (20, 21, 22, 23, 24):
        py = y_temp(tick)
        grid.append(f'<line x1="{left}" y1="{py:.2f}" x2="{right}" y2="{py:.2f}" class="grid"/>')
        grid.append(f'<text x="66" y="{py + 5:.2f}" text-anchor="end" class="small">{tick}</text>')
    for tick in (0, 10, 20):
        py = y_power(tick)
        grid.append(f'<line x1="{left}" y1="{py:.2f}" x2="{right}" y2="{py:.2f}" class="grid"/>')
        grid.append(f'<text x="66" y="{py + 5:.2f}" text-anchor="end" class="small">{tick}</text>')

    six = next(row for row in rows if abs(float(row["time_s"]) - 6.0) < 1e-9)
    restoration_time = float(summary["restoration_time_s"])
    body = f'''
<text x="40" y="40" class="title">Derived prediction versus configured simulation truth</text>
<text x="40" y="66" class="subtitle">Lesson 15 · 0 to 20% heater step · 100 ms samples · seed {SEED}</text>
<rect x="{left}" y="{top_a}" width="{right-left}" height="{bottom_a-top_a}" class="panel"/>
<rect x="{left}" y="{top_b}" width="{right-left}" height="{bottom_b-top_b}" class="panel"/>
{''.join(grid)}
<text x="78" y="118" class="heading">Outlet-water temperature (degC)</text>
<text x="78" y="485" class="heading">Heater command (%)</text>
<line x1="{left}" y1="{y_temp(24):.2f}" x2="{right}" y2="{y_temp(24):.2f}" class="reference"/>
<text x="850" y="{y_temp(24)-8:.2f}" text-anchor="end" class="small">predicted equilibrium 24.00 degC</text>
<line x1="{x(1):.2f}" y1="{top_a}" x2="{x(1):.2f}" y2="{bottom_b}" class="event"/>
<text x="{x(1)+6:.2f}" y="151" class="small">response begins after L = 1.00 s</text>
<line x1="{x(restoration_time):.2f}" y1="{top_b}" x2="{x(restoration_time):.2f}" y2="{bottom_b}" class="event"/>
<text x="{x(restoration_time)-6:.2f}" y="488" text-anchor="end" class="small">safe restoration at {restoration_time:.2f} s</text>
<polyline points="{polyline(rows, x, y_temp, 'pv_degc')}" class="actual"/>
<polyline points="{polyline(rows, x, y_temp, 'model_prediction_degc')}" class="model"/>
<polyline points="{polyline(rows, x, y_power, 'requested_power_percent')}" class="request"/>
<polyline points="{polyline(rows, x, y_power, 'applied_power_percent')}" class="applied"/>
<circle cx="{x(6):.2f}" cy="{y_temp(float(six['pv_degc'])):.2f}" r="5" class="point"/>
<text x="{x(6)+12:.2f}" y="{y_temp(float(six['pv_degc']))-10:.2f}" class="small">6.00 s: 22.53 degC</text>
<line x1="100" y1="84" x2="132" y2="84" class="actual"/><text x="140" y="89" class="small">configured simulation truth</text>
<line x1="370" y1="84" x2="402" y2="84" class="model"/><text x="410" y="89" class="small">derived model prediction</text>
<line x1="100" y1="724" x2="132" y2="724" class="request"/><text x="140" y="729" class="small">requested power</text>
<line x1="340" y1="724" x2="372" y2="724" class="applied"/><text x="380" y="729" class="small">applied power</text>
<text x="858" y="729" text-anchor="end" class="small">time (s)</text>
<text x="78" y="752" class="muted">6 s error {float(summary['six_second_absolute_error_degc']):.3f} degC · final-window mean error {float(summary['final_window_mean_absolute_equilibrium_error_degc']):.3f} degC · final-window range {float(summary['final_window_pv_range_degc']):.3f} degC</text>
'''
    return svg_document(
        900, 760, body, "Water-heater model prediction and simulation response",
        "Two plots compare derived and simulated outlet temperature, then requested and applied heater power. The curves agree under configured simulation truth and finish with safe zero-power restoration.",
    )


def main() -> None:
    with (ROOT / "heater-model-trace.csv").open(newline="", encoding="utf-8") as handle:
        rows = list(csv.DictReader(handle))
    with (ROOT / "heater-model-summary.csv").open(newline="", encoding="utf-8") as handle:
        summary = next(csv.DictReader(handle))
    (ROOT / "heater-energy-boundary.svg").write_text(energy_boundary(), encoding="utf-8")
    (ROOT / "heater-model-response.svg").write_text(
        response_figure(rows, summary), encoding="utf-8"
    )


if __name__ == "__main__":
    main()
