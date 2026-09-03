#!/usr/bin/env python3
"""Render Lesson 29's atomic-update figure from its retained trace."""

from __future__ import annotations

import csv
from pathlib import Path

ROOT = Path(__file__).resolve().parent
INK = "#243044"
GRID = "#d8e0eb"
AXIS = "#526078"
BLUE = "#0072b2"
ORANGE = "#d55e00"
GREEN = "#009e73"
DASH = "#555555"
PAPER = "#f7f9fc"

LEFT, RIGHT = 70.0, 860.0
P1_TOP, P1_BOTTOM = 155.0, 315.0
P2_TOP, P2_BOTTOM = 360.0, 520.0


def text(x: float, y: float, value: str, size: int = 11, anchor: str = "start", weight: str = "400") -> str:
    w = f' font-weight="{weight}"' if weight != "400" else ""
    return f'<text x="{x:.2f}" y="{y:.2f}" font-size="{size}" text-anchor="{anchor}"{w}>{value}</text>'


def hline(y: float, css: str = "grid") -> str:
    return f'<line class="{css}" x1="{LEFT}" y1="{y:.2f}" x2="{RIGHT}" y2="{y:.2f}" />'


def vline(x: float, top: float, bottom: float, css: str = "grid") -> str:
    return f'<line class="{css}" x1="{x:.2f}" y1="{top}" x2="{x:.2f}" y2="{bottom}" />'


def polyline(points: list[tuple[float, float]], colour: str, dash: bool = False) -> str:
    encoded = " ".join(f"{x:.2f},{y:.2f}" for x, y in points)
    da = ' stroke-dasharray="7 5"' if dash else ""
    return f'<polyline fill="none" stroke="{colour}" stroke-width="2.2"{da} points="{encoded}" />'


def y_ticks(lo: float, hi: float, n: int = 5) -> list[float]:
    if hi <= lo:
        hi = lo + 1.0
    pad = (hi - lo) * 0.15 or 1.0
    lo, hi = lo - pad, hi + pad
    return [hi - (hi - lo) * i / (n - 1) for i in range(n)]


def panel(top: float, bottom: float, title: str, x_values: list[float], x_ticks: list[str], series: list[tuple[str, list[float], bool]]) -> list[str]:
    values = [v for _, vals, _ in series for v in vals]
    lo, hi = min(values), max(values)
    ticks = y_ticks(lo, hi)
    body = [text(LEFT, top - 12, title, 16, weight="700")]
    for t in ticks:
        y = bottom - (t - ticks[-1]) / (ticks[0] - ticks[-1]) * (bottom - top)
        body += [hline(y), text(LEFT - 8, y + 4, f"{t:.2f}", anchor="end")]

    x_lo, x_hi = min(x_values), max(x_values)

    def sx(v: float) -> float:
        return LEFT if x_hi <= x_lo else LEFT + (v - x_lo) / (x_hi - x_lo) * (RIGHT - LEFT)

    def sy(v: float) -> float:
        return bottom - (v - ticks[-1]) / (ticks[0] - ticks[-1]) * (bottom - top)

    for i, label in enumerate(x_ticks):
        xv = x_lo + i / (len(x_ticks) - 1) * (x_hi - x_lo)
        body += [vline(sx(xv), top, bottom), text(sx(xv), bottom + 18, label, anchor="middle")]
    for colour, vals, dashed in series:
        body.append(polyline([(sx(x_values[i]), sy(v)) for i, v in enumerate(vals)], colour, dashed))
    body.append(f'<rect class="axis" x="{LEFT}" y="{top}" width="{RIGHT - LEFT}" height="{bottom - top}" fill="none" />')
    return body


def legend(items: list[tuple[float, float, str, str, bool]]) -> list[str]:
    body = []
    for x, y, colour, label, dashed in items:
        da = ' stroke-dasharray="7 5"' if dashed else ""
        body.append(f'<line x1="{x:.1f}" y1="{y:.1f}" x2="{x + 28:.1f}" y2="{y:.1f}" stroke="{colour}" stroke-width="3"{da} />')
        body.append(text(x + 34, y + 5, label, 12))
    return body


def render() -> str:
    with (ROOT / "atomic-update-trace.csv").open(newline="", encoding="utf-8") as handle:
        rows = list(csv.DictReader(handle))
    times = [float(row["time_s"]) for row in rows]
    x_ticks = [f"{t:.1f}" for t in [0.0, 0.1, 0.2, 0.3, 0.4, 0.5, 0.6]]

    previous_applied = [float(row["previous_applied"]) for row in rows]
    rate_lower = [float(row["rate_lower_bound"]) for row in rows]
    rate_upper = [float(row["rate_upper_bound"]) for row in rows]
    applied = [float(row["applied_output"]) for row in rows]
    diagnostic = [float(row["unconstrained_target_diagnostic"]) for row in rows]
    rejected_sum = [float(row["rejected_post-limit_sum"]) for row in rows]

    body: list[str] = []
    body += legend([
        (55, 96, DASH, "previous applied", True),
        (212, 96, ORANGE, "rate lower bound", True),
        (369, 96, GREEN, "rate upper bound", True),
        (527, 96, BLUE, "applied output", False),
        (55, 116, BLUE, "unconstrained target diagnostic", False),
        (320, 116, ORANGE, "rejected post-limit sum", True),
    ])
    body += panel(P1_TOP, P1_BOTTOM, "Previous, permitted and applied valve output (%)", times, x_ticks, [
        (DASH, previous_applied, True),
        (ORANGE, rate_lower, True),
        (GREEN, rate_upper, True),
        (BLUE, applied, False),
    ])
    body += panel(P2_TOP, P2_BOTTOM, "Independent target diagnostic and rejected external sum (%)", times, x_ticks, [
        (BLUE, diagnostic, False),
        (ORANGE, rejected_sum, True),
    ])
    body.append(text(465, 598, "time (s)", 12, anchor="middle"))

    title = "Every rate interval starts at the prior applied output"
    desc = "Configured Simulation evidence · 100 ms scans; internal feedforward stays inside constraints"
    return f'''<svg xmlns="http://www.w3.org/2000/svg" role="img" aria-labelledby="title desc" viewBox="0 0 900 610">
<title id="title">{title}</title>
<desc id="desc">{desc}</desc>
<rect width="900" height="610" fill="{PAPER}"/>
<style>text{{font-family:system-ui,-apple-system,sans-serif;fill:{INK}}} .axis{{stroke:{AXIS};stroke-width:1}} .grid{{stroke:{GRID};stroke-width:1}} .marker{{stroke:#b43c32;stroke-width:1.5;stroke-dasharray:4 4}}</style>
{text(55, 42, title, 24, weight="700")}
{text(55, 68, desc, 14)}
{"".join(body)}
</svg>
'''


def main() -> None:
    (ROOT / "atomic-update.svg").write_text(render(), encoding="utf-8")


if __name__ == "__main__":
    main()
