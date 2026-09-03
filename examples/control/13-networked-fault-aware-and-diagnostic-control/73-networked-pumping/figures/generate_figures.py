#!/usr/bin/env python3
"""Render Lesson 73 from its retained native networked-pumping trace."""

from __future__ import annotations

import csv
from pathlib import Path


ROOT = Path(__file__).resolve().parent
INK = "#172033"
MUTED = "#526078"
GRID = "#cbd5e1"
RED = "#dc2626"
BLUE = "#2563eb"
PAPER = "#f8fafc"


def text(x: float, y: float, value: str, css: str = "note", anchor: str = "start") -> str:
    return f'<text x="{x:.1f}" y="{y:.1f}" class="{css}" text-anchor="{anchor}">{value}</text>'


def line(x1: float, y1: float, x2: float, y2: float, css: str) -> str:
    return f'<line x1="{x1:.1f}" y1="{y1:.1f}" x2="{x2:.1f}" y2="{y2:.1f}" class="{css}" />'


def polyline(points: list[tuple[float, float]], colour: str, width: float = 2.2, dash: str = "") -> str:
    encoded = " ".join(f"{x:.1f},{y:.1f}" for x, y in points)
    dashed = f' stroke-dasharray="{dash}"' if dash else ""
    return f'<polyline points="{encoded}" fill="none" stroke="{colour}" stroke-width="{width}"{dashed} />'


def load_rows() -> list[dict[str, str]]:
    with (ROOT / "networked-pumping-evidence-trace.csv").open(newline="", encoding="utf-8") as handle:
        return list(csv.DictReader(handle))


def series(rows: list[dict[str, str]], panel: str, name: str) -> list[tuple[float, float]]:
    return sorted(
        (float(row["x"]), float(row["y"])) for row in rows
        if row["panel"] == panel and row["series"] == name
    )


def panel_block(
    rows: list[dict[str, str]], panel_csv_name: str, axis_title: str,
    y_ticks: list[float], y_min: float, y_max: float,
    top: float, bottom: float, left: float, right: float,
    x_ticks: list[float], x_max: float, draw_x_axis_label: bool,
) -> list[str]:
    sx = lambda v: left + v / x_max * (right - left)
    sy = lambda v: bottom - (v - y_min) / (y_max - y_min) * (bottom - top)
    naive = series(rows, panel_csv_name, "Naive")
    protected = series(rows, panel_csv_name, "Protected")
    body = [text(left, top - 12, axis_title, "label")]
    for value in y_ticks:
        body += [line(left, sy(value), right, sy(value), "grid"), text(left - 12, sy(value) + 4, f"{value:g}", anchor="end")]
    for value in x_ticks:
        body += [line(sx(value), top, sx(value), bottom, "grid")]
        if draw_x_axis_label:
            body.append(text(sx(value), bottom + 24, f"{value:g}", anchor="middle"))
    body += [
        line(left, top, left, bottom, "axis"), line(left, bottom, right, bottom, "axis"),
        polyline([(sx(x), sy(y)) for x, y in naive], RED, 2.2),
        polyline([(sx(x), sy(y)) for x, y in protected], BLUE, 2.2),
    ]
    if draw_x_axis_label:
        body.append(text((left + right) / 2, bottom + 42, "Time after start (s)", "label", "middle"))
    return body


def render() -> str:
    rows = load_rows()
    left, right = 92.0, 850.0
    p_top, p_bottom = 150.0, 340.0
    c_top, c_bottom = 420.0, 610.0
    x_ticks = [0, 5, 10, 15, 20, 25]

    body: list[str] = []
    body += panel_block(
        rows, "Station discharge pressure", "Header pressure (bar)",
        y_ticks=[5.5, 6.0, 6.5], y_min=5.4, y_max=6.6,
        top=p_top, bottom=p_bottom, left=left, right=right,
        x_ticks=x_ticks, x_max=25.0, draw_x_axis_label=False,
    )
    body.append(text(left, p_bottom + 24, "0", anchor="middle"))
    for value in x_ticks[1:]:
        body.append(text(left + value / 25.0 * (right - left), p_bottom + 24, f"{value:g}", anchor="middle"))
    body += panel_block(
        rows, "Applied pump-speed command", "Applied pump-speed command (%)",
        y_ticks=[0, 25, 50, 75, 100], y_min=0.0, y_max=100.0,
        top=c_top, bottom=c_bottom, left=left, right=right,
        x_ticks=x_ticks, x_max=25.0, draw_x_axis_label=True,
    )
    body += [
        line(245, 100, 275, 100, "naive"), text(282, 104, "Naive"),
        line(345, 100, 375, 100, "protected"), text(382, 104, "Protected"),
    ]
    return f'''<svg xmlns="http://www.w3.org/2000/svg" role="img" aria-labelledby="title desc" viewBox="0 0 900 660">
<title id="title">Network validity policy prevents unsafe remote-command acceptance</title>
<desc id="desc">Naive and protected receivers replay the same native 25-second packet record; the protected header pressure and applied command stay flat while the naive path follows six ineligible packets.</desc>
<rect width="900" height="660" rx="12" fill="{PAPER}" stroke="{GRID}" />
<style>
  text {{ font-family: system-ui, -apple-system, sans-serif; fill: {INK}; }}
  .title {{ font-size: 22px; font-weight: 700; fill: {INK}; }} .subtitle {{ font-size: 13px; fill: {MUTED}; }}
  .axis {{ stroke: #8792a5; stroke-width: 1.4; }} .grid {{ stroke: {GRID}; stroke-width: 1; stroke-dasharray: 3 5; }}
  .note {{ fill: {MUTED}; font-size: 11px; }} .label {{ font-size: 13px; font-weight: 700; fill: {INK}; }}
  .naive {{ stroke: {RED}; stroke-width: 3; }} .protected {{ stroke: {BLUE}; stroke-width: 3; }}
</style>
{text(48, 42, "Header pressure and applied pump-speed command", "title")}
{text(48, 65, "Simulation evidence · shipped ST · 100 ms samples · seed 20260829 · case: naive vs protected", "subtitle")}
{"".join(body)}
</svg>
'''


def main() -> None:
    (ROOT / "networked-pumping-evidence.svg").write_text(render(), encoding="utf-8")


if __name__ == "__main__":
    main()
