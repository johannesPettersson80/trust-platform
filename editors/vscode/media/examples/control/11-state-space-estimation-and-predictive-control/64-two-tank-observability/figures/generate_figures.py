#!/usr/bin/env python3
"""Render Lesson 64 from its retained native feasibility trace.

Reads the checked-in trace CSV (panel, series, point, x, y, x_unit, y_unit)
and redraws the two-panel evidence SVG with correct numbers and a single,
unambiguous axis title per panel (no repeated quantity name, no nested
parenthesis). The two panels use different series (Tank 1/Tank 2 for the
pump step; Downstream truth/Upstream sensor for the hidden-state record),
so each panel carries its own legend.
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
PANEL_GAP = 100.0
TOP = 175.0

PANEL1 = ("Upstream-pump step", [("Tank 1", "#2563eb", ""), ("Tank 2", "#059669", "")])
PANEL2 = ("Hidden downstream initial state", [("Downstream truth", "#dc2626", ""), ("Upstream sensor", "#6b7280", "7 5")])
Y_LABEL = "Level deviation (cm)"
X_LABEL = "Time after start (s)"


def read_trace() -> dict[tuple[str, str], list[tuple[float, float]]]:
    with (ROOT / "two-tank-feasibility-evidence-trace.csv").open(newline="", encoding="utf-8") as handle:
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


def render_panel(panel_name: str, series: list[tuple[str, str, str]], top: float,
                  data: dict[tuple[str, str], list[tuple[float, float]]]) -> tuple[str, float]:
    bottom = top + PANEL_HEIGHT
    all_points = [pt for name, _, _ in series for pt in data[(panel_name, name)]]
    xs = [x for x, _ in all_points]
    ys = [y for _, y in all_points]
    x_lo, x_hi = min(xs), max(xs)
    pad = max(1e-6, (max(ys) - min(ys)) * 0.12)
    y_lo, y_hi = min(ys) - pad, max(ys) + pad
    sx = lambda v: LEFT + (v - x_lo) / (x_hi - x_lo) * (RIGHT - LEFT)
    sy = lambda v: bottom - (v - y_lo) / (y_hi - y_lo) * (bottom - top)

    body: list[str] = []
    legend = []
    lx = LEFT
    for name, colour, dash in series:
        legend.append(f'<line x1="{lx:.1f}" y1="{top - 30:.1f}" x2="{lx + 26:.1f}" y2="{top - 30:.1f}" stroke="{colour}" stroke-width="3"{f" stroke-dasharray=\"{dash}\"" if dash else ""}/>')
        legend.append(text(lx + 32, top - 26, name))
        lx += 32 + 8 * len(name) + 40
    body.append(text(LEFT, top - 12, panel_name, "paneltitle"))
    for yt in nice_ticks(y_lo, y_hi):
        body.append(f'<line class="grid" x1="{LEFT}" y1="{sy(yt):.1f}" x2="{RIGHT}" y2="{sy(yt):.1f}" />')
        body.append(text(LEFT - 8, sy(yt) + 4, f"{yt:.2f}", "note", "end"))
    for xt in nice_ticks(x_lo, x_hi):
        body.append(f'<line class="grid" x1="{sx(xt):.1f}" y1="{top}" x2="{sx(xt):.1f}" y2="{bottom}" />')
        body.append(text(sx(xt), bottom + 18, f"{xt:.1f}", "note", "middle"))
    body.append(f'<line class="axis" x1="{LEFT}" y1="{top}" x2="{LEFT}" y2="{bottom}" />')
    body.append(f'<line class="axis" x1="{LEFT}" y1="{bottom}" x2="{RIGHT}" y2="{bottom}" />')
    for name, colour, dash in series:
        pts = sorted(data[(panel_name, name)])
        pts = pts[::4] + [pts[-1]]
        body.append(polyline([(sx(x), sy(y)) for x, y in pts], colour, dash))
    body.append(text((LEFT + RIGHT) / 2, bottom + 36, X_LABEL, "label", "middle"))
    mid = (top + bottom) / 2
    body.append(
        f'<text x="26" y="{mid:.1f}" class="label" text-anchor="middle" '
        f'transform="rotate(-90 26 {mid:.1f})">{Y_LABEL}</text>'
    )
    return "".join(legend) + "".join(body), bottom


def render() -> str:
    data = read_trace()
    body1, bottom1 = render_panel(PANEL1[0], PANEL1[1], TOP, data)
    top2 = bottom1 + PANEL_GAP
    body2, bottom2 = render_panel(PANEL2[0], PANEL2[1], top2, data)

    height = bottom2 + 60
    return f'''<svg xmlns="http://www.w3.org/2000/svg" role="img" aria-labelledby="title desc" viewBox="0 0 900 {height:.0f}">
<title id="title">Actuator and sensor placement determine reachable knowledge</title>
<desc id="desc">The upstream pump reaches both tanks, while an upstream-only sensor hides a downstream initial condition.</desc>
<rect width="900" height="{height:.0f}" fill="{PAPER}"/>
<style>
  text {{ font-family: system-ui, -apple-system, sans-serif; fill: {INK}; font-size: 12px; }}
  .axis {{ stroke: #8792a5; stroke-width: 1.4; }}
  .grid {{ stroke: {GRID}; stroke-width: 1; stroke-dasharray: 3 5; }}
  .note {{ fill: {MUTED}; font-size: 11px; }}
  .label {{ font-size: 13px; font-weight: 700; fill: {INK}; }}
  .paneltitle {{ font-size: 16px; font-weight: 700; fill: {INK}; }}
  .title {{ font-size: 22px; font-weight: 700; fill: {INK}; }}
  .subtitle {{ font-size: 14px; fill: {MUTED}; }}
</style>
{text(55, 42, "Actuator and sensor placement determine reachable knowledge", "title")}
{text(55, 68, "The upstream pump reaches both tanks, while an upstream-only sensor hides a", "subtitle")}
{text(55, 86, "downstream initial condition.", "subtitle")}
{body1}
{body2}
</svg>
'''


def main() -> None:
    (ROOT / "two-tank-feasibility-evidence.svg").write_text(render(), encoding="utf-8")


if __name__ == "__main__":
    main()
