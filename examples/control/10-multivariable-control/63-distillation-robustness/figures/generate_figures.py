#!/usr/bin/env python3
"""Render Lesson 63 from its retained native robustness-comparison trace.

Reads the checked-in trace CSV (panel, series, point, x, y, x_unit, y_unit)
and redraws the two-panel evidence SVG with correct numbers and a single,
unambiguous axis title per panel (no repeated quantity name, no nested
parenthesis). Requested/applied pairs in the second panel share one colour
per architecture and are distinguished only by a dashed (requested) versus
solid (applied) stroke, with a dedicated legend explaining that convention.
"""

from __future__ import annotations

import csv
from pathlib import Path

ROOT = Path(__file__).resolve().parent
INK = "#243044"
MUTED = "#526078"
GRID = "#d8e0eb"
PAPER = "#f7f9fc"

LEFT, RIGHT = 75.0, 855.0
PANEL_HEIGHT = 160.0
PANEL_GAP = 90.0
TOP = 167.0

ARCH_COLOR = {"Direct": "#6b7280", "Static": "#059669", "Centralized": "#2563eb"}

PANEL1 = ("Strong-interaction cross response", ["Direct", "Static", "Centralized"])
PANEL2_SERIES = [
    ("Static requested", "#059669", "7 5"),
    ("Static applied", "#059669", ""),
    ("Central requested", "#2563eb", "7 5"),
    ("Central applied", "#2563eb", ""),
]
PANEL2_NAME = "Infeasible reflux request"


def read_trace() -> dict[tuple[str, str], list[tuple[float, float]]]:
    with (ROOT / "distillation-robustness-evidence-trace.csv").open(newline="", encoding="utf-8") as handle:
        rows = list(csv.DictReader(handle))
    data: dict[tuple[str, str], list[tuple[float, float]]] = {}
    for row in rows:
        key = (row["panel"], row["series"])
        data.setdefault(key, []).append((float(row["x"]), float(row["y"])))
    return data


def nice_ticks(lo: float, hi: float, count: int = 5) -> list[float]:
    if hi <= lo:
        hi = lo + 1.0
    span = hi - lo
    return [lo + span * i / (count - 1) for i in range(count)]


def text(x: float, y: float, value: str, css: str = "note", anchor: str = "start") -> str:
    return f'<text x="{x:.1f}" y="{y:.1f}" class="{css}" text-anchor="{anchor}">{value}</text>'


def polyline(points: list[tuple[float, float]], colour: str, dash: str = "") -> str:
    encoded = " ".join(f"{x:.2f},{y:.2f}" for x, y in points)
    dashed = f' stroke-dasharray="{dash}"' if dash else ""
    return (f'<polyline points="{encoded}" fill="none" stroke="{colour}" '
            f'stroke-width="2.2"{dashed} />')


def scales(points: list[tuple[float, float]], top: float, bottom: float):
    xs = [x for x, _ in points]
    ys = [y for _, y in points]
    x_lo, x_hi = min(xs), max(xs)
    pad = max(1e-6, (max(ys) - min(ys)) * 0.12)
    y_lo, y_hi = min(ys) - pad, max(ys) + pad
    sx = lambda v: LEFT + (v - x_lo) / (x_hi - x_lo) * (RIGHT - LEFT)
    sy = lambda v: bottom - (v - y_lo) / (y_hi - y_lo) * (bottom - top)
    return x_lo, x_hi, y_lo, y_hi, sx, sy


def axes_and_ticks(top: float, bottom: float, x_lo: float, x_hi: float,
                    y_lo: float, y_hi: float, sx, sy, y_label: str) -> list[str]:
    body: list[str] = []
    for yt in nice_ticks(y_lo, y_hi):
        body.append(f'<line class="grid" x1="{LEFT}" y1="{sy(yt):.1f}" x2="{RIGHT}" y2="{sy(yt):.1f}" />')
        body.append(text(LEFT - 8, sy(yt) + 4, f"{yt:.2f}", "note", "end"))
    for xt in nice_ticks(x_lo, x_hi):
        body.append(f'<line class="grid" x1="{sx(xt):.1f}" y1="{top}" x2="{sx(xt):.1f}" y2="{bottom}" />')
        body.append(text(sx(xt), bottom + 18, f"{xt:.1f}", "note", "middle"))
    body.append(f'<line class="axis" x1="{LEFT}" y1="{top}" x2="{LEFT}" y2="{bottom}" />')
    body.append(f'<line class="axis" x1="{LEFT}" y1="{bottom}" x2="{RIGHT}" y2="{bottom}" />')
    body.append(text((LEFT + RIGHT) / 2, bottom + 36, "Time after start (s)", "label", "middle"))
    mid = (top + bottom) / 2
    body.append(
        f'<text x="26" y="{mid:.1f}" class="label" text-anchor="middle" '
        f'transform="rotate(-90 26 {mid:.1f})">{y_label}</text>'
    )
    return body


def render() -> str:
    data = read_trace()

    top1 = TOP
    bottom1 = top1 + PANEL_HEIGHT
    p1_points = [pt for name in PANEL1[1] for pt in data[(PANEL1[0], name)]]
    x_lo, x_hi, y_lo, y_hi, sx, sy = scales(p1_points, top1, bottom1)
    body1 = [text(LEFT, top1 - 12, PANEL1[0], "paneltitle")]
    body1 += axes_and_ticks(top1, bottom1, x_lo, x_hi, y_lo, y_hi, sx, sy, "Bottoms deviation (pp)")
    for name in PANEL1[1]:
        pts = sorted(data[(PANEL1[0], name)])
        pts = pts[::4] + [pts[-1]]
        body1.append(polyline([(sx(x), sy(y)) for x, y in pts], ARCH_COLOR[name]))

    top2 = bottom1 + 90.0
    bottom2 = top2 + PANEL_HEIGHT
    p2_points = [pt for name, _, _ in PANEL2_SERIES for pt in data[(PANEL2_NAME, name)]]
    x2_lo, x2_hi, y2_lo, y2_hi, sx2, sy2 = scales(p2_points, top2, bottom2)
    body2 = [text(LEFT, top2 - 12, PANEL2_NAME, "paneltitle")]
    body2 += axes_and_ticks(top2, bottom2, x2_lo, x2_hi, y2_lo, y2_hi, sx2, sy2, "Valve command (pp)")
    for name, colour, dash in PANEL2_SERIES:
        pts = sorted(data[(PANEL2_NAME, name)])
        pts = pts[::4] + [pts[-1]]
        body2.append(polyline([(sx2(x), sy2(y)) for x, y in pts], colour, dash))

    legend = []
    lx = 55.0
    for name, colour in ARCH_COLOR.items():
        legend.append(f'<line x1="{lx:.1f}" y1="118" x2="{lx + 28:.1f}" y2="118" stroke="{colour}" stroke-width="3"/>')
        legend.append(text(lx + 34, 123, name))
        lx += 34 + 11 * len(name) + 30

    legend2_y = top2 - 32
    legend2 = [
        text(LEFT, legend2_y, "Requested (dashed) vs applied (solid); colour keeps the same Static/Centralized mapping as above:", "note"),
    ]

    height = bottom2 + 80
    return f'''<svg xmlns="http://www.w3.org/2000/svg" role="img" aria-labelledby="title desc" viewBox="0 0 900 {height:.0f}">
<title id="title">Robustness must be checked in coupled directions</title>
<desc id="desc">Static decoupling passes the declared normal envelope; coordinated scaling preserves direction but cannot make an infeasible target achievable.</desc>
<rect width="900" height="{height:.0f}" fill="{PAPER}"/>
<style>
  text {{ font-family: system-ui, -apple-system, sans-serif; fill: {INK}; font-size: 12px; }}
  .axis {{ stroke: #8792a5; stroke-width: 1.4; }}
  .grid {{ stroke: {GRID}; stroke-width: 1; stroke-dasharray: 3 5; }}
  .note {{ fill: {MUTED}; font-size: 11px; }}
  .label {{ font-size: 13px; font-weight: 700; fill: {INK}; }}
  .paneltitle {{ font-size: 16px; font-weight: 700; fill: {INK}; }}
  .title {{ font-size: 24px; font-weight: 700; fill: {INK}; }}
  .subtitle {{ font-size: 14px; fill: {MUTED}; }}
</style>
{text(55, 42, "Robustness must be checked in coupled directions", "title")}
{text(55, 70, "Static decoupling passes the declared normal envelope; coordinated scaling preserves", "subtitle")}
{text(55, 88, "direction but cannot make an infeasible target achievable.", "subtitle")}
{"".join(legend)}
{"".join(body1)}
{"".join(legend2)}
{"".join(body2)}
</svg>
'''


def main() -> None:
    (ROOT / "distillation-robustness-evidence.svg").write_text(render(), encoding="utf-8")


if __name__ == "__main__":
    main()
