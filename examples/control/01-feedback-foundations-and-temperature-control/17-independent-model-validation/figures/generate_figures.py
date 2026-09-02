#!/usr/bin/env python3
"""Generate Lesson 17 figures from the retained native-runtime recording."""

from __future__ import annotations

import csv
from pathlib import Path


ROOT = Path(__file__).resolve().parent
TRACE = list(csv.DictReader((ROOT / "validation-trace.csv").open()))
SUMMARY = {row["metric"]: row for row in csv.DictReader((ROOT / "validation-summary.csv").open())}

STYLE = """
<style>
text{font-family:-apple-system,BlinkMacSystemFont,'Segoe UI',sans-serif;fill:#1f2937}
.title{font-size:25px;font-weight:700}.heading{font-size:18px;font-weight:700}
.body{font-size:14px}.small{font-size:13px}.muted{fill:#536174}
.panel{fill:#f7f9fc;stroke:#9fb2cc;stroke-width:1.4}.grid{stroke:#d4deea;stroke-width:1}
.measured{fill:none;stroke:#1473e6;stroke-width:3}.model{fill:none;stroke:#e46c0a;stroke-width:2.4;stroke-dasharray:8 5}
.envelope{fill:#e8dfff;stroke:none;opacity:.8}.residual{fill:none;stroke:#7c3aed;stroke-width:2.2}
.event{stroke:#c24135;stroke-width:1.5;stroke-dasharray:4 4}.zero{stroke:#64748b;stroke-width:1.3}
.accepted{fill:#e5f7ee;stroke:#14804a}.restricted{fill:#fff4d6;stroke:#a16207}
svg{background:#fff;forced-color-adjust:none}
</style>
"""


def path(rows, x, y, key):
    return " ".join(f'{x(float(row["time_s"])):.2f},{y(float(row[key])):.2f}' for row in rows)


def common_grid(x, top, height):
    return "".join(
        f'<line x1="{x(t):.2f}" y1="{top}" x2="{x(t):.2f}" y2="{top+height}" class="grid"/>'
        for t in range(0, 61, 10)
    )


def response():
    left, top, width, height = 78.0, 130.0, 780.0, 430.0
    x = lambda value: left + value / 60.0 * width
    y = lambda value: top + height - (value - 29.7) / 1.6 * height
    upper = list(reversed(TRACE))
    envelope = " ".join(
        [f'{x(float(row["time_s"])):.2f},{y(float(row["envelope_upper_degc"])):.2f}' for row in TRACE]
        + [f'{x(float(row["time_s"])):.2f},{y(float(row["envelope_lower_degc"])):.2f}' for row in upper]
    )
    ticks = "".join(f'<text x="{x(t):.2f}" y="582" text-anchor="middle" class="small">{t}</text>' for t in range(0, 61, 10))
    y_ticks = "".join(f'<text x="70" y="{y(v)+4:.2f}" text-anchor="end" class="small">{v:.1f}</text>' for v in (30.0, 30.5, 31.0))
    return f'''<svg xmlns="http://www.w3.org/2000/svg" width="900" height="760" viewBox="0 0 900 760">
{STYLE}
<text x="48" y="40" class="title">Independent prediction on data excluded from fitting</text>
<text x="48" y="66" class="body muted">Measured Simulation evidence and frozen identified-model prediction · no refitting</text>
<line x1="80" y1="94" x2="114" y2="94" class="measured"/><text x="122" y="99" class="small">measured outlet temperature</text>
<line x1="353" y1="94" x2="387" y2="94" class="model"/><text x="395" y="99" class="small">frozen model prediction</text>
<rect x="610" y="82" width="25" height="18" class="envelope"/><text x="642" y="98" class="small">parameter envelope</text>
<text x="{left}" y="120" class="heading">Outlet-water temperature (degC)</text>
<rect x="{left}" y="{top}" width="{width}" height="{height}" class="panel"/>{common_grid(x, top, height)}{ticks}{y_ticks}
<polygon points="{envelope}" class="envelope"/>
<polyline points="{path(TRACE, x, y, 'measured_pv_degc')}" class="measured"/>
<polyline points="{path(TRACE, x, y, 'predicted_pv_degc')}" class="model"/>
<line x1="{x(10.1):.2f}" y1="{top}" x2="{x(10.1):.2f}" y2="{top+height}" class="event"/>
<line x1="{x(30.0):.2f}" y1="{top}" x2="{x(30.0):.2f}" y2="{top+height}" class="event"/>
<text x="{x(10.1)+7:.2f}" y="{top+22}" class="small">new 20-to-25% validation step</text>
<text x="{x(30)+7:.2f}" y="{top+46}" class="small">unmodelled cold-flow load</text>
<text x="{left}" y="620" class="body">The model predicts the new fixed-flow step, then diverges systematically when the omitted load path changes.</text>
<rect x="{left}" y="650" width="300" height="55" rx="8" class="accepted"/>
<text x="{left+150}" y="674" text-anchor="middle" class="heading">ACCEPTED</text><text x="{left+150}" y="696" text-anchor="middle" class="small">local fixed-flow command response</text>
<rect x="{left+330}" y="650" width="300" height="55" rx="8" class="restricted"/>
<text x="{left+480}" y="674" text-anchor="middle" class="heading">RESTRICTED</text><text x="{left+480}" y="696" text-anchor="middle" class="small">cold-flow disturbance prediction</text>
<text x="790" y="735" class="small">time (s)</text>
</svg>'''


def residual():
    left, top, width, height = 78.0, 105.0, 780.0, 235.0
    x = lambda value: left + value / 60.0 * width
    y = lambda value: top + height / 2 - value / 1.2 * height / 2
    ticks = "".join(f'<text x="{x(t):.2f}" y="363" text-anchor="middle" class="small">{t}</text>' for t in range(0, 61, 10))
    fixed_rmse = float(SUMMARY["fixed_flow_rmse"]["value"])
    bias = float(SUMMARY["post_load_residual_bias"]["value"])
    rho = float(SUMMARY["fixed_flow_lag_one_correlation"]["value"])
    return f'''<svg xmlns="http://www.w3.org/2000/svg" width="900" height="440" viewBox="0 0 900 440">
{STYLE}
<text x="48" y="40" class="title">Residual structure determines the model's valid purpose</text>
<text x="48" y="65" class="body muted">Prediction residual = measured temperature minus frozen-model prediction</text>
<text x="{left}" y="94" class="heading">Prediction residual (degC)</text>
<rect x="{left}" y="{top}" width="{width}" height="{height}" class="panel"/>{common_grid(x, top, height)}{ticks}
<line x1="{left}" y1="{y(0):.2f}" x2="{left+width}" y2="{y(0):.2f}" class="zero"/>
<polyline points="{path(TRACE, x, y, 'residual_degc')}" class="residual"/>
<line x1="{x(30):.2f}" y1="{top}" x2="{x(30):.2f}" y2="{top+height}" class="event"/>
<text x="{x(30)+7:.2f}" y="{top+22}" class="small">cold-flow load begins</text>
<text x="70" y="{y(1.0)+4:.2f}" text-anchor="end" class="small">+1.0</text><text x="70" y="{y(0)+4:.2f}" text-anchor="end" class="small">0.0</text><text x="70" y="{y(-1.0)+4:.2f}" text-anchor="end" class="small">-1.0</text>
<text x="{left}" y="402" class="body">Fixed-flow RMSE {fixed_rmse:.3f} degC · lag-one correlation {rho:.3f} (diagnostic) · post-load bias {bias:.3f} degC</text>
</svg>'''


(ROOT / "validation-response.svg").write_text(response())
(ROOT / "validation-residual.svg").write_text(residual())
