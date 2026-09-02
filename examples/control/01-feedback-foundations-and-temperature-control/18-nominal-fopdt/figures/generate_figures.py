#!/usr/bin/env python3
"""Generate Lesson 18 figures from retained native-runtime evidence."""

from __future__ import annotations

import csv
from pathlib import Path


ROOT = Path(__file__).resolve().parent
TRACE = list(csv.DictReader((ROOT / "lambda-comparison.csv").open()))
SUMMARY = {row["candidate"]: row for row in csv.DictReader((ROOT / "lambda-summary.csv").open())}

STYLE = """
<style>
text{font-family:-apple-system,BlinkMacSystemFont,'Segoe UI',sans-serif;fill:#1f2937}
.title{font-size:25px;font-weight:700}.heading{font-size:18px;font-weight:700}
.body{font-size:14px}.small{font-size:13px}.muted{fill:#536174}
.panel{fill:#f7f9fc;stroke:#9fb2cc;stroke-width:1.4}.grid{stroke:#d4deea;stroke-width:1}
.sp{fill:none;stroke:#475569;stroke-width:1.8;stroke-dasharray:5 4}.empirical{fill:none;stroke:#e46c0a;stroke-width:2.4}
.lambda{fill:none;stroke:#1473e6;stroke-width:3}.perturbed{fill:none;stroke:#7c3aed;stroke-width:2.3}
.event{stroke:#c24135;stroke-width:1.5;stroke-dasharray:4 4}.limit{stroke:#14804a;stroke-width:1.3;stroke-dasharray:5 4}
.pass{fill:#e5f7ee;stroke:#14804a}.note{fill:#eef4ff;stroke:#3769a8}
svg{background:#fff;forced-color-adjust:none}
</style>
"""


def points(key, x, y):
    return " ".join(f'{x(float(row["time_s"])):.2f},{y(float(row[key])):.2f}' for row in TRACE)


def grid(x, top, height):
    return "".join(
        f'<line x1="{x(t):.2f}" y1="{top}" x2="{x(t):.2f}" y2="{top+height}" class="grid"/>'
        for t in range(0, 121, 20)
    )


def comparison():
    left, width = 78.0, 780.0
    top_pv, height_pv = 130.0, 300.0
    top_mv, height_mv = 520.0, 145.0
    x = lambda value: left + value / 120.0 * width
    y_pv = lambda value: top_pv + height_pv - (value - 30.0) / 1.0 * height_pv
    y_mv = lambda value: top_mv + height_mv - (value - 20.0) / 5.0 * height_mv
    ticks = "".join(f'<text x="{x(t):.2f}" y="687" text-anchor="middle" class="small">{t}</text>' for t in range(0, 121, 20))
    emp = SUMMARY["empirical"]
    lam = SUMMARY["lambda"]
    return f'''<svg xmlns="http://www.w3.org/2000/svg" width="900" height="760" viewBox="0 0 900 760">
{STYLE}
<text x="48" y="40" class="title">Empirical PI and model-based PI on one identical plan</text>
<text x="48" y="66" class="body muted">Configured Simulation evidence · same plant, event, limits, timing, seed, and axes</text>
<line x1="82" y1="95" x2="116" y2="95" class="sp"/><text x="124" y="100" class="small">setpoint</text>
<line x1="230" y1="95" x2="264" y2="95" class="empirical"/><text x="272" y="100" class="small">empirical PI</text>
<line x1="410" y1="95" x2="444" y2="95" class="lambda"/><text x="452" y="100" class="small">Lambda/IMC PI</text>
<text x="{left}" y="120" class="heading">Outlet-water temperature (degC)</text>
<rect x="{left}" y="{top_pv}" width="{width}" height="{height_pv}" class="panel"/>{grid(x, top_pv, height_pv)}
<polyline points="{points('setpoint_degc', x, y_pv)}" class="sp"/><polyline points="{points('empirical_pv_degc', x, y_pv)}" class="empirical"/><polyline points="{points('lambda_pv_degc', x, y_pv)}" class="lambda"/>
<line x1="{x(60):.2f}" y1="{top_pv}" x2="{x(60):.2f}" y2="{top_pv+height_pv}" class="event"/><text x="{x(60)+7:.2f}" y="{top_pv+22}" class="small">cold-flow load</text>
<text x="70" y="{y_pv(30)+4:.2f}" text-anchor="end" class="small">30.0</text><text x="70" y="{y_pv(30.5)+4:.2f}" text-anchor="end" class="small">30.5</text><text x="70" y="{y_pv(31)+4:.2f}" text-anchor="end" class="small">31.0</text>
<text x="{left}" y="463" class="body">Tracking IAE: empirical {float(emp['tracking_iae_degc_s']):.2f}, Lambda {float(lam['tracking_iae_degc_s']):.2f} degC*s</text>
<text x="{left}" y="487" class="body">Load IAE: empirical {float(emp['load_iae_degc_s']):.2f}, Lambda {float(lam['load_iae_degc_s']):.2f} degC*s</text>
<text x="{left}" y="510" class="heading">Applied heater power (%)</text>
<rect x="{left}" y="{top_mv}" width="{width}" height="{height_mv}" class="panel"/>{grid(x, top_mv, height_mv)}
<polyline points="{points('empirical_mv_percent', x, y_mv)}" class="empirical"/><polyline points="{points('lambda_mv_percent', x, y_mv)}" class="lambda"/>
<line x1="{x(60):.2f}" y1="{top_mv}" x2="{x(60):.2f}" y2="{top_mv+height_mv}" class="event"/>{ticks}
<text x="70" y="{y_mv(20)+4:.2f}" text-anchor="end" class="small">20.0</text><text x="70" y="{y_mv(22.5)+4:.2f}" text-anchor="end" class="small">22.5</text><text x="70" y="{y_mv(25)+4:.2f}" text-anchor="end" class="small">25.0</text>
<text x="790" y="730" class="small">time (s)</text>
</svg>'''


def uncertainty():
    left, top, width, height = 78.0, 110.0, 780.0, 205.0
    x = lambda value: left + value / 120.0 * width
    y = lambda value: top + height - (value - 30.0) / 1.0 * height
    pert = SUMMARY["lambda_perturbed_noisy"]
    return f'''<svg xmlns="http://www.w3.org/2000/svg" width="900" height="440" viewBox="0 0 900 440">
{STYLE}
<text x="48" y="40" class="title">The selected tuning must also survive declared uncertainty</text>
<text x="48" y="66" class="body muted">K 0.19 degC/% · T 6.0 s · L 1.3 s · deterministic measurement noise 0.03 degC</text>
<text x="{left}" y="100" class="heading">Perturbed outlet-water temperature (degC)</text>
<rect x="{left}" y="{top}" width="{width}" height="{height}" class="panel"/>{grid(x, top, height)}
<polyline points="{points('setpoint_degc', x, y)}" class="sp"/><polyline points="{points('perturbed_pv_degc', x, y)}" class="perturbed"/>
<line x1="{x(60):.2f}" y1="{top}" x2="{x(60):.2f}" y2="{top+height}" class="event"/><text x="{x(60)+7:.2f}" y="{top+22}" class="small">cold-flow load</text>
<text x="70" y="{y(30)+4:.2f}" text-anchor="end" class="small">30.0</text><text x="70" y="{y(30.5)+4:.2f}" text-anchor="end" class="small">30.5</text><text x="70" y="{y(31)+4:.2f}" text-anchor="end" class="small">31.0</text>
<rect x="78" y="342" width="310" height="58" rx="8" class="pass"/><text x="233" y="366" text-anchor="middle" class="heading">PASS</text><text x="233" y="389" text-anchor="middle" class="small">final error {float(pert['final_mean_absolute_error_degc']):.3f} degC ≤ 0.20 degC</text>
<rect x="414" y="342" width="390" height="58" rx="8" class="note"/><text x="609" y="366" text-anchor="middle" class="heading">ACTUATOR COST REMAINS VISIBLE</text><text x="609" y="389" text-anchor="middle" class="small">accumulated MV travel {float(pert['mv_travel_percentage_points']):.2f} percentage points</text>
<text x="790" y="425" class="small">time (s)</text>
</svg>'''


(ROOT / "lambda-comparison.svg").write_text(comparison())
(ROOT / "lambda-uncertainty.svg").write_text(uncertainty())
