#!/usr/bin/env python3
"""Render Lesson 41 from its retained response and residual validation traces."""

from __future__ import annotations

import csv
from pathlib import Path


ROOT = Path(__file__).resolve().parent
INK = "#243044"
GRID = "#d8e0eb"
PAPER = "#f7f9fc"
BLUE = "#0072b2"
GREEN = "#009e73"
ORANGE = "#d55e00"
PINK = "#cc79a7"


def text(x: float, y: float, value: str, size: int = 11, weight: str = "400", anchor: str = "start") -> str:
    return f'<text x="{x:.2f}" y="{y:.2f}" font-size="{size}" font-weight="{weight}" text-anchor="{anchor}">{value}</text>'


def polyline(points: list[tuple[float, float]], colour: str, dash: str = "") -> str:
    encoded = " ".join(f"{x:.2f},{y:.2f}" for x, y in points)
    dashed = f' stroke-dasharray="{dash}"' if dash else ""
    return f'<polyline fill="none" stroke="{colour}" stroke-width="2.2"{dashed} points="{encoded}" />'


def single_panel(title: str, subtitle: str, legend: list[tuple[str, str, str]],
                  rows: list[dict], t_end: float, series: list[tuple[str, str, str]],
                  v_range: tuple[float, float], out_label: str, event_time: float | None = None) -> str:
    left, right = 70.0, 860.0
    top, bottom = 155.0, 520.0
    sx = lambda t: left + t / t_end * (right - left)
    sy = lambda v: bottom - (v - v_range[0]) / (v_range[1] - v_range[0]) * (bottom - top)

    body: list[str] = []
    for i in range(9):
        frac = i / 8
        v = v_range[1] - frac * (v_range[1] - v_range[0])
        y = top + frac * (bottom - top)
        body.append(f'<line class="grid" x1="{left}" y1="{y:.2f}" x2="{right}" y2="{y:.2f}"/>{text(left - 8, y + 4, f"{v:.2f}", anchor="end")}')
    for i in range(9):
        tv = t_end * i / 8
        x = sx(tv)
        body.append(f'<line class="grid" x1="{x:.2f}" y1="{top}" x2="{x:.2f}" y2="{bottom}"/>{text(x, bottom + 18, f"{tv:.0f}", anchor="middle")}')
    if event_time is not None:
        ex = sx(event_time)
        body.append(f'<line class="marker" x1="{ex:.2f}" y1="{top}" x2="{ex:.2f}" y2="{bottom}"/>')
        body.append(text(ex + 5, top + 14, "cold-flow load", size=11))

    for column, colour, dash in series:
        body.append(polyline([(sx(float(r["time_s"])), sy(float(r[column]))) for r in rows], colour, dash))
    body.append(f'<rect x="{left}" y="{top}" width="{right - left}" height="{bottom - top}" fill="none" stroke="#526078"/>')

    legend_x = 55
    legend_body = ""
    for label, colour, dash in legend:
        legend_body += f'<line x1="{legend_x}" y1="96" x2="{legend_x + 28}" y2="96" stroke="{colour}" stroke-width="3"{f" stroke-dasharray={chr(34)}{dash}{chr(34)}" if dash else ""}/>{text(legend_x + 34, 101, label)}'
        legend_x += 40 + 7 * len(label)
        if legend_x > 760:
            legend_x = 55

    return f'''<svg xmlns="http://www.w3.org/2000/svg" role="img" aria-labelledby="title desc" viewBox="0 0 900 610">
<title id="title">{title}</title>
<desc id="desc">{subtitle}</desc>
<rect width="900" height="610" fill="{PAPER}"/>
<style>text{{font-family:system-ui,-apple-system,sans-serif;fill:{INK}}} .axis{{stroke:#526078;stroke-width:1}} .grid{{stroke:{GRID};stroke-width:1}} .marker{{stroke:#b43c32;stroke-width:1.5;stroke-dasharray:4 4}}</style>
{text(55, 42, title, size=24, weight="700")}
{text(55, 68, subtitle, size=14)}
{legend_body}
{text(70, 143, out_label, size=16, weight="700")}
{"".join(body)}
{text(465, 598, "time (s)", size=12, anchor="middle")}
</svg>
'''


def render_response() -> str:
    with (ROOT / "heat-exchanger-validation-response-trace.csv").open(newline="", encoding="utf-8") as handle:
        rows = list(csv.DictReader(handle))
    t_end = float(rows[-1]["time_s"])
    series = [
        ("local_measured", INK, ""), ("frozen_prediction", BLUE, ""),
        ("envelope_lower", GREEN, "7 5"), ("envelope_upper", GREEN, "7 5"),
        ("cold-flow_measured", ORANGE, ""), ("high-flow_measured", PINK, ""),
        ("unchanged_prediction", BLUE, "7 5"),
    ]
    legend = [
        ("local measured", INK, ""), ("frozen prediction", BLUE, ""),
        ("envelope lower", GREEN, "7 5"), ("envelope upper", GREEN, "7 5"),
        ("cold-flow measured", ORANGE, ""), ("high-flow measured", PINK, ""),
        ("unchanged prediction", BLUE, "7 5"),
    ]
    return single_panel(
        "Independent heat-exchanger validation",
        "Configured Simulation measurements · one frozen local prediction and predeclared envelope",
        legend, rows, t_end, series, (49.0, 53.5),
        "Local holdout and frozen prediction (degC)", event_time=90.0,
    )


def render_residual() -> str:
    with (ROOT / "heat-exchanger-validation-residual-trace.csv").open(newline="", encoding="utf-8") as handle:
        rows = list(csv.DictReader(handle))
    t_end = float(rows[-1]["time_s"])
    series = [("local_residual", BLUE, ""), ("cold-flow_residual", ORANGE, ""), ("high-flow_residual", PINK, "")]
    legend = [("local residual", BLUE, ""), ("cold-flow residual", ORANGE, ""), ("high-flow residual", PINK, "")]
    return single_panel(
        "Validation residuals reveal model limits",
        "Residual equals measured outlet temperature minus the unchanged frozen-model prediction",
        legend, rows, t_end, series, (-2.5, 0.6),
        "Local and cold-flow residuals (degC)", event_time=90.0,
    )


def main() -> None:
    (ROOT / "heat-exchanger-validation-response.svg").write_text(render_response(), encoding="utf-8")
    (ROOT / "heat-exchanger-validation-residual.svg").write_text(render_residual(), encoding="utf-8")


if __name__ == "__main__":
    main()
