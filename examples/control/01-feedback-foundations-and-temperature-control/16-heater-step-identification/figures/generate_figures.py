#!/usr/bin/env python3
"""Generate Lesson 16 teaching figures from retained executable evidence."""

from __future__ import annotations

import csv
import json
from pathlib import Path


ROOT = Path(__file__).resolve().parent
TRACE = list(csv.DictReader((ROOT / "heater-identification-trace.csv").open()))
SUMMARY = next(csv.DictReader((ROOT / "heater-identification-run-summary.csv").open()))
MODEL = json.loads((ROOT / "heater-identification-model-evidence.json").read_text())
SEED = TRACE[0]["seed"]


STYLE = """
<style>
text{font-family:-apple-system,BlinkMacSystemFont,'Segoe UI',sans-serif;fill:#1f2937}
.title{font-size:25px;font-weight:700}.heading{font-size:19px;font-weight:700}
.body{font-size:14px}.small{font-size:13px}.muted{fill:#536174}
.panel{fill:#f5f8fb;stroke:#9fb2cc;stroke-width:1.4}.grid{stroke:#d4deea;stroke-width:1}
.measured{fill:none;stroke:#1473e6;stroke-width:3}.model{fill:none;stroke:#e46c0a;stroke-width:2.5;stroke-dasharray:8 5}
.residual{fill:none;stroke:#7c3aed;stroke-width:2}.event{stroke:#d52b1e;stroke-width:1.5;stroke-dasharray:4 4}
.guide{stroke:#64748b;stroke-width:1.3;stroke-dasharray:5 4}.step{stroke:#0f9d75;stroke-width:3;fill:none}
.marker{fill:white;stroke:#1473e6;stroke-width:3}.badge{fill:#fff4d6;stroke:#a16207;stroke-width:1.2}
svg{background:#fff;forced-color-adjust:none}
</style>
"""


def points(rows: list[dict[str, str]], x, y, x_key: str, y_key: str) -> str:
    return " ".join(f"{x(float(row[x_key])):.2f},{y(float(row[y_key])):.2f}" for row in rows)


def construction() -> str:
    active = TRACE[:500]
    baseline = float(SUMMARY["baseline_degc"])
    final = baseline + float(SUMMARY["delta_y_degc"])
    level_2 = baseline + 0.02 * (final - baseline)
    level_63 = float(SUMMARY["level_63_degc"])
    step_time = 10.1
    response_time = step_time + float(SUMMARY["estimated_delay_s"])
    crossing_time = step_time + float(SUMMARY["crossing_63_upper_s"])
    left, top, width, height = 80.0, 100.0, 750.0, 245.0
    x = lambda value: left + value / 50.0 * width
    y_min, y_max = 29.8, 32.2
    y = lambda value: top + height - (value - y_min) / (y_max - y_min) * height
    grid = "".join(
        f'<line x1="{x(t):.2f}" y1="{top}" x2="{x(t):.2f}" y2="{top+height}" class="grid"/>'
        for t in range(0, 51, 10)
    ) + "".join(
        f'<line x1="{left}" y1="{y(v):.2f}" x2="{left+width}" y2="{y(v):.2f}" class="grid"/>'
        for v in (30.0, 31.0, 32.0)
    )
    x_ticks = "".join(
        f'<text x="{x(t):.2f}" y="361" text-anchor="middle" class="small">{t}</text>'
        for t in range(0, 51, 10)
    )
    y_ticks = "".join(
        f'<text x="72" y="{y(v)+4:.2f}" text-anchor="end" class="small">{v:.0f}</text>'
        for v in (30.0, 31.0, 32.0)
    )
    return f'''<svg xmlns="http://www.w3.org/2000/svg" width="900" height="440" viewBox="0 0 900 440">
{STYLE}
<text x="55" y="40" class="title">How the reaction curve defines gain, delay, and response time</text>
<text x="55" y="65" class="body muted">Measured Simulation evidence · 500 samples · 100 ms interval · seed {SEED}</text>
<rect x="{left}" y="{top}" width="{width}" height="{height}" class="panel"/>{grid}{x_ticks}{y_ticks}
<polyline points="{points(active, x, y, 'time_s', 'pv_degc')}" class="measured"/>
<line x1="{left}" y1="{y(baseline):.2f}" x2="{left+width}" y2="{y(baseline):.2f}" class="guide"/>
<line x1="{left}" y1="{y(final):.2f}" x2="{left+width}" y2="{y(final):.2f}" class="guide"/>
<line x1="{x(step_time):.2f}" y1="{top}" x2="{x(step_time):.2f}" y2="{top+height}" class="event"/>
<line x1="{x(response_time):.2f}" y1="{top}" x2="{x(response_time):.2f}" y2="{top+height}" class="event"/>
<line x1="{x(crossing_time):.2f}" y1="{top}" x2="{x(crossing_time):.2f}" y2="{top+height}" class="event"/>
<circle cx="{x(response_time):.2f}" cy="{y(level_2):.2f}" r="5" class="marker"/>
<circle cx="{x(crossing_time):.2f}" cy="{y(level_63):.2f}" r="5" class="marker"/>
<path d="M {x(0):.2f} {top+height-18:.2f} H {x(step_time):.2f} V {top+height-52:.2f} H {x(50):.2f}" class="step"/>
<text x="{x(1):.2f}" y="{y(baseline)-10:.2f}" class="small">baseline: y0 = {baseline:.2f} degC at u0 = 20.00%</text>
<text x="{x(step_time)+6:.2f}" y="{top+height-62:.2f}" class="small">applied step: u1 = 30.00%, delta_u = 10.00 pp</text>
<text x="{x(response_time)+18:.2f}" y="{top+height-10:.2f}" class="small">2% crossing bounds apparent delay (L)</text>
<text x="{x(crossing_time)+7:.2f}" y="{y(level_63)-10:.2f}" class="small">63.2% crossing bounds time constant (T)</text>
<text x="{x(31):.2f}" y="{y(final)-9:.2f}" class="small">final mean: y1 = {final:.3f} degC</text>
<text x="26" y="{top+height/2:.2f}" text-anchor="middle" class="small" transform="rotate(-90 26 {top+height/2:.2f})">outlet temperature (degC)</text>
<text x="760" y="374" class="small">time (s)</text>
<text x="80" y="405" class="body">First qualify baseline, timing, excitation, quality, clipping, and saturation. Only then construct the model.</text>
</svg>'''


def fit_figure() -> str:
    active = TRACE[:500]
    fit_points = MODEL["fit"]["points"]
    selected = MODEL["selected"]
    uncertainty = selected["uncertainty"]
    left, width = 78.0, 780.0
    top_a, height_a = 125.0, 275.0
    top_b, height_b = 480.0, 170.0
    x = lambda value: left + value / 50.0 * width
    pv_min, pv_max = 29.8, 32.2
    y_pv = lambda value: top_a + height_a - (value - pv_min) / (pv_max - pv_min) * height_a
    residuals = [float(point["residual"]) for point in fit_points]
    residual_limit = max(0.01, max(abs(value) for value in residuals) * 1.15)
    y_r = lambda value: top_b + height_b / 2 - value / residual_limit * height_b / 2
    grid = "".join(
        f'<line x1="{x(t):.2f}" y1="{top_a}" x2="{x(t):.2f}" y2="{top_a+height_a}" class="grid"/>'
        f'<line x1="{x(t):.2f}" y1="{top_b}" x2="{x(t):.2f}" y2="{top_b+height_b}" class="grid"/>'
        for t in range(0, 51, 10)
    )
    top_x_ticks = "".join(
        f'<text x="{x(t):.2f}" y="418" text-anchor="middle" class="small">{t}</text>'
        for t in range(0, 51, 10)
    )
    pv_ticks = "".join(
        f'<text x="70" y="{y_pv(v)+4:.2f}" text-anchor="end" class="small">{v:.0f}</text>'
        for v in (30.0, 31.0, 32.0)
    )
    residual_ticks = "".join(
        f'<text x="70" y="{y_r(v)+4:.2f}" text-anchor="end" class="small">{v:+.3f}</text>'
        for v in (residual_limit, 0.0, -residual_limit)
    )
    measured = points(active, x, y_pv, "time_s", "pv_degc")
    predicted = " ".join(
        f'{x(float(point["timeSeconds"])):.2f},{y_pv(float(point["predicted"])):.2f}'
        for point in fit_points
    )
    residual = " ".join(
        f'{x(float(point["timeSeconds"])):.2f},{y_r(float(point["residual"])):.2f}'
        for point in fit_points
    )
    gain = uncertainty["gain"]
    tau = uncertainty["timeConstantSeconds"]
    delay = uncertainty["deadTimeSeconds"]
    return f'''<svg xmlns="http://www.w3.org/2000/svg" width="900" height="760" viewBox="0 0 900 760">
{STYLE}
<text x="55" y="38" class="title">Measured response and identified model</text>
<text x="55" y="64" class="body muted">Identified from Simulation data · validation required · not accepted for Tune</text>
<rect x="650" y="20" width="195" height="58" rx="8" class="badge"/>
<text x="747" y="44" text-anchor="middle" class="heading">VALIDATION REQUIRED</text>
<text x="747" y="64" text-anchor="middle" class="small">same-run fit only</text>
<line x1="100" y1="94" x2="132" y2="94" class="measured"/><text x="140" y="99" class="small">measured Simulation PV</text>
<line x1="355" y1="94" x2="387" y2="94" class="model"/><text x="395" y="99" class="small">identified FOPDT prediction</text>
<text x="{left}" y="116" class="heading">Outlet-water temperature (degC)</text>
<rect x="{left}" y="{top_a}" width="{width}" height="{height_a}" class="panel"/>{grid}{top_x_ticks}{pv_ticks}
<polyline points="{measured}" class="measured"/><polyline points="{predicted}" class="model"/>
<line x1="{x(10.1):.2f}" y1="{top_a}" x2="{x(10.1):.2f}" y2="{top_a+height_a}" class="event"/>
<text x="{x(10.1)+7:.2f}" y="{top_a+18:.2f}" class="small">20 to 30% applied step</text>
<text x="{left}" y="445" class="body">K {selected['gain']:.4f} degC/% [{gain['minimum']:.4f}, {gain['maximum']:.4f}] · T {selected['timeConstantSeconds']:.2f} s [{tau['minimum']:.2f}, {tau['maximum']:.2f}] · L {selected['deadTimeSeconds']:.2f} s [{delay['minimum']:.2f}, {delay['maximum']:.2f}]</text>
<text x="{left}" y="470" class="heading">Prediction residual (measured minus predicted, degC)</text>
<rect x="{left}" y="{top_b}" width="{width}" height="{height_b}" class="panel"/>
<line x1="{left}" y1="{y_r(0):.2f}" x2="{left+width}" y2="{y_r(0):.2f}" class="guide"/>
{residual_ticks}
<polyline points="{residual}" class="residual"/>
<text x="{left}" y="685" class="body">RMSE {MODEL['fit']['rootMeanSquareError']:.4f} degC · normalized error {selected['normalizedError']:.4f} · no warnings</text>
<text x="{left}" y="712" class="small muted">Sample-resolution envelope only; not statistical confidence. Configured truth was withheld until after fitting.</text>
<text x="760" y="742" class="small">time (s)</text>
</svg>'''


(ROOT / "heater-identification-construction.svg").write_text(construction())
(ROOT / "heater-identification-fit.svg").write_text(fit_figure())
