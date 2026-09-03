#!/usr/bin/env python3
"""Render Lesson 39 from its retained estimation, comparison, and residual traces."""

from __future__ import annotations

import csv
from pathlib import Path


ROOT = Path(__file__).resolve().parent
INK = "#243044"
GRID = "#d8e0eb"
PAPER = "#f7f9fc"
BLUE = "#0072b2"
ORANGE = "#d55e00"
GREEN = "#009e73"
PINK = "#cc79a7"


def text(x: float, y: float, value: str, size: int = 11, weight: str = "400", anchor: str = "start") -> str:
    return f'<text x="{x:.2f}" y="{y:.2f}" font-size="{size}" font-weight="{weight}" text-anchor="{anchor}">{value}</text>'


def polyline(points: list[tuple[float, float]], colour: str, dash: str = "") -> str:
    encoded = " ".join(f"{x:.2f},{y:.2f}" for x, y in points)
    dashed = f' stroke-dasharray="{dash}"' if dash else ""
    return f'<polyline fill="none" stroke="{colour}" stroke-width="2.2"{dashed} points="{encoded}" />'


def frame(title: str, subtitle: str, legend: list[tuple[str, str, str]], body: str,
          out_label: str, cmd_label: str | None) -> str:
    legend_x = 55
    legend_body = ""
    for label, colour, dash in legend:
        legend_body += f'<line x1="{legend_x}" y1="96" x2="{legend_x + 28}" y2="96" stroke="{colour}" stroke-width="3"{f" stroke-dasharray={chr(34)}{dash}{chr(34)}" if dash else ""}/>{text(legend_x + 34, 101, label)}'
        legend_x += 40 + 7 * len(label)
    labels = text(70, 143, out_label, size=16, weight="700")
    if cmd_label:
        labels += text(70, 348, cmd_label, size=16, weight="700")
    return f'''<svg xmlns="http://www.w3.org/2000/svg" role="img" aria-labelledby="title desc" viewBox="0 0 900 610">
<title id="title">{title}</title>
<desc id="desc">{subtitle}</desc>
<rect width="900" height="610" fill="{PAPER}"/>
<style>text{{font-family:system-ui,-apple-system,sans-serif;fill:{INK}}} .axis{{stroke:#526078;stroke-width:1}} .grid{{stroke:{GRID};stroke-width:1}}</style>
{text(55, 42, title, size=24, weight="700")}
{text(55, 68, subtitle, size=14)}
{legend_body}
{labels}
{body}
{text(465, 598, "time (s)", size=12, anchor="middle")}
</svg>
'''


def two_panel_body(rows: list[dict], t_end: float,
                    out_cols: list[tuple[str, str, str]], out_range: tuple[float, float],
                    cmd_cols: list[tuple[str, str, str]], cmd_range: tuple[float, float]) -> str:
    left, right = 70.0, 860.0
    top1, bottom1 = 155.0, 315.0
    top2, bottom2 = 360.0, 520.0
    sx = lambda t: left + t / t_end * (right - left)
    sy1 = lambda v: bottom1 - (v - out_range[0]) / (out_range[1] - out_range[0]) * (bottom1 - top1)
    sy2 = lambda v: bottom2 - (v - cmd_range[0]) / (cmd_range[1] - cmd_range[0]) * (bottom2 - top2)

    body: list[str] = []
    for i in range(5):
        frac = i / 4
        v1 = out_range[1] - frac * (out_range[1] - out_range[0])
        y1 = top1 + frac * (bottom1 - top1)
        body.append(f'<line class="grid" x1="{left}" y1="{y1:.2f}" x2="{right}" y2="{y1:.2f}"/>{text(left - 8, y1 + 4, f"{v1:.2f}", anchor="end")}')
        v2 = cmd_range[1] - frac * (cmd_range[1] - cmd_range[0])
        y2 = top2 + frac * (bottom2 - top2)
        body.append(f'<line class="grid" x1="{left}" y1="{y2:.2f}" x2="{right}" y2="{y2:.2f}"/>{text(left - 8, y2 + 4, f"{v2:.2f}", anchor="end")}')
    for i in range(7):
        tv = t_end * i / 6
        x = sx(tv)
        body.append(f'<line class="grid" x1="{x:.2f}" y1="{top1}" x2="{x:.2f}" y2="{bottom1}"/>{text(x, bottom1 + 18, f"{tv:.1f}", anchor="middle")}')
        body.append(f'<line class="grid" x1="{x:.2f}" y1="{top2}" x2="{x:.2f}" y2="{bottom2}"/>{text(x, bottom2 + 18, f"{tv:.1f}", anchor="middle")}')

    for column, colour, dash in out_cols:
        body.append(polyline([(sx(float(r["time_s"])), sy1(float(r[column]))) for r in rows if r[column] not in ("", None)], colour, dash))
    body.append(f'<rect x="{left}" y="{top1}" width="{right - left}" height="{bottom1 - top1}" fill="none" stroke="#526078"/>')
    for column, colour, dash in cmd_cols:
        body.append(polyline([(sx(float(r["time_s"])), sy2(float(r[column]))) for r in rows], colour, dash))
    body.append(f'<rect x="{left}" y="{top2}" width="{right - left}" height="{bottom2 - top2}" fill="none" stroke="#526078"/>')
    return "".join(body)


def render_estimation() -> str:
    with (ROOT / "dryer-model-estimation-trace.csv").open(newline="", encoding="utf-8") as handle:
        rows = list(csv.DictReader(handle))
    t_end = float(rows[-1]["time_s"])
    body = two_panel_body(
        rows, t_end,
        [("measured", INK, ""), ("first_order", ORANGE, "7 5"), ("fopdt", BLUE, ""),
         ("second_order", GREEN, "7 5"), ("integrating", PINK, "7 5")],
        (79.5, 90.0),
        [("applied_command", INK, "")], (29.0, 41.0),
    )
    legend = [("measured", INK, ""), ("first order", ORANGE, "7 5"), ("FOPDT", BLUE, ""),
              ("second order", GREEN, "7 5"), ("integrating", PINK, "7 5")]
    return frame("Dryer candidates on estimation data",
                 "Configured Simulation evidence · common estimation step and candidate predictions",
                 legend, body, "Product-zone temperature (degC)", "Applied heater demand (%)")


def render_comparison() -> str:
    with (ROOT / "dryer-model-comparison-trace.csv").open(newline="", encoding="utf-8") as handle:
        rows = list(csv.DictReader(handle))
    t_end = float(rows[-1]["time_s"])
    body = two_panel_body(
        rows, t_end,
        [("measured", INK, ""), ("first_order", ORANGE, "7 5"), ("fopdt", BLUE, ""), ("second_order", GREEN, "7 5")],
        (79.5, 82.0),
        [("applied_command", PINK, "")], (29.0, 36.0),
    )
    legend = [("measured", INK, ""), ("first order", ORANGE, "7 5"), ("FOPDT", BLUE, ""),
              ("second order", GREEN, "7 5"), ("applied command", PINK, "")]
    return frame("Dryer candidates on withheld data",
                 "Configured Simulation measurement · predictions use frozen candidates and withheld pulse data",
                 legend, body, "Product-zone temperature (degC)", "Applied heater demand (%)")


def render_residual() -> str:
    with (ROOT / "dryer-model-residual-trace.csv").open(newline="", encoding="utf-8") as handle:
        rows = list(csv.DictReader(handle))
    t_end = float(rows[-1]["time_s"])
    left, right = 70.0, 860.0
    top, bottom = 155.0, 520.0
    r_range = (-1.4, 0.6)
    sx = lambda t: left + t / t_end * (right - left)
    sy = lambda v: bottom - (v - r_range[0]) / (r_range[1] - r_range[0]) * (bottom - top)

    body: list[str] = []
    for i in range(9):
        frac = i / 8
        v = r_range[1] - frac * (r_range[1] - r_range[0])
        y = top + frac * (bottom - top)
        body.append(f'<line class="grid" x1="{left}" y1="{y:.2f}" x2="{right}" y2="{y:.2f}"/>{text(left - 8, y + 4, f"{v:.2f}", anchor="end")}')
    for i in range(7):
        tv = t_end * i / 6
        x = sx(tv)
        body.append(f'<line class="grid" x1="{x:.2f}" y1="{top}" x2="{x:.2f}" y2="{bottom}"/>{text(x, bottom + 18, f"{tv:.1f}", anchor="middle")}')
    body.append(f'<line x1="{left}" y1="{sy(0.0):.2f}" x2="{right}" y2="{sy(0.0):.2f}" class="axis"/>')

    columns = [("first_order_residual", ORANGE, "7 5"), ("fopdt_residual", BLUE, ""),
               ("second-order_residual", GREEN, "7 5"), ("integrating_residual", PINK, "")]
    for column, colour, dash in columns:
        body.append(polyline([(sx(float(r["time_s"])), sy(float(r[column]))) for r in rows], colour, dash))
    body.append(f'<rect x="{left}" y="{top}" width="{right - left}" height="{bottom - top}" fill="none" stroke="#526078"/>')

    legend = [("first order residual", ORANGE, "7 5"), ("FOPDT residual", BLUE, ""),
              ("second-order residual", GREEN, "7 5"), ("integrating residual", PINK, "")]
    return frame("Dryer residuals reveal structure errors",
                 "Residual equals measured temperature minus each frozen candidate prediction",
                 legend, "".join(body), "Finite-equilibrium candidate residuals (degC)", None)


def main() -> None:
    (ROOT / "dryer-model-estimation.svg").write_text(render_estimation(), encoding="utf-8")
    (ROOT / "dryer-model-comparison.svg").write_text(render_comparison(), encoding="utf-8")
    (ROOT / "dryer-model-residual.svg").write_text(render_residual(), encoding="utf-8")


if __name__ == "__main__":
    main()
