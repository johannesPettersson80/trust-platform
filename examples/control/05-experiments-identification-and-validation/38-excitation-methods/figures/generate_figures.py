#!/usr/bin/env python3
"""Render Lesson 38 from its retained heater and servo excitation traces."""

from __future__ import annotations

import csv
from pathlib import Path


ROOT = Path(__file__).resolve().parent
INK = "#243044"
GRID = "#d8e0eb"
PAPER = "#f7f9fc"
BLUE = "#0072b2"
PINK = "#cc79a7"
ORANGE = "#d55e00"
GREEN = "#009e73"


def text(x: float, y: float, value: str, size: int = 11, weight: str = "400", anchor: str = "start") -> str:
    return f'<text x="{x:.2f}" y="{y:.2f}" font-size="{size}" font-weight="{weight}" text-anchor="{anchor}">{value}</text>'


def polyline(points: list[tuple[float, float]], colour: str, dash: str = "") -> str:
    encoded = " ".join(f"{x:.2f},{y:.2f}" for x, y in points)
    dashed = f' stroke-dasharray="{dash}"' if dash else ""
    return f'<polyline fill="none" stroke="{colour}" stroke-width="2.2"{dashed} points="{encoded}" />'


def two_panel(title: str, subtitle: str, rows: list[dict], t_end: float,
              out_cols: list[tuple[str, str, str]], out_label: str, out_range: tuple[float, float],
              cmd_cols: list[tuple[str, str, str]], cmd_label: str, cmd_range: tuple[float, float],
              legend: list[tuple[str, str]]) -> str:
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
        body.append(polyline([(sx(float(r["time_s"])), sy1(float(r[column]))) for r in rows], colour, dash))
    body.append(f'<rect x="{left}" y="{top1}" width="{right - left}" height="{bottom1 - top1}" fill="none" stroke="#526078"/>')
    for column, colour, dash in cmd_cols:
        body.append(polyline([(sx(float(r["time_s"])), sy2(float(r[column]))) for r in rows], colour, dash))
    body.append(f'<rect x="{left}" y="{top2}" width="{right - left}" height="{bottom2 - top2}" fill="none" stroke="#526078"/>')

    legend_x = 55
    legend_body = ""
    for label, colour in legend:
        legend_body += f'<line x1="{legend_x}" y1="96" x2="{legend_x + 28}" y2="96" stroke="{colour}" stroke-width="3"/>{text(legend_x + 34, 101, label)}'
        legend_x += 40 + 7 * len(label)

    return f'''<svg xmlns="http://www.w3.org/2000/svg" role="img" aria-labelledby="title desc" viewBox="0 0 900 610">
<title id="title">{title}</title>
<desc id="desc">{subtitle}</desc>
<rect width="900" height="610" fill="{PAPER}"/>
<style>text{{font-family:system-ui,-apple-system,sans-serif;fill:{INK}}} .axis{{stroke:#526078;stroke-width:1}} .grid{{stroke:{GRID};stroke-width:1}}</style>
{text(55, 42, title, size=24, weight="700")}
{text(55, 68, subtitle, size=14)}
{legend_body}
{text(70, 143, out_label, size=16, weight="700")}
{text(70, 348, cmd_label, size=16, weight="700")}
{"".join(body)}
{text(465, 598, "time (s)", size=12, anchor="middle")}
</svg>
'''


def render_heater() -> str:
    with (ROOT / "excitation-heater-comparison-trace.csv").open(newline="", encoding="utf-8") as handle:
        rows = list(csv.DictReader(handle))
    t_end = float(rows[-1]["time_s"])
    return two_panel(
        "Excitation for a slow heater",
        "Configured Simulation evidence · identical water-heater model and 100 ms sampling",
        rows, t_end,
        [("step", BLUE, ""), ("pulse", PINK, ""), ("prbs", ORANGE, ""), ("chirp", GREEN, "")],
        "Measured heater temperature (degC)", (23.4, 25.6),
        [("step_command", BLUE, "7 5"), ("pulse_command", PINK, "7 5"), ("prbs_command", ORANGE, "7 5"), ("chirp_command", GREEN, "7 5")],
        "Requested heater command (%)", (14.0, 26.0),
        [("step", BLUE), ("pulse", PINK), ("PRBS", ORANGE), ("chirp", GREEN),
         ("step command", BLUE), ("pulse command", PINK), ("PRBS command", ORANGE), ("chirp command", GREEN)],
    )


def render_servo() -> str:
    with (ROOT / "excitation-servo-comparison-trace.csv").open(newline="", encoding="utf-8") as handle:
        rows = list(csv.DictReader(handle))
    t_end = float(rows[-1]["time_s"])
    return two_panel(
        "Excitation for a flexible drive",
        "Configured Simulation evidence · identical flexible drive and 100 ms sampling",
        rows, t_end,
        [("doublet", BLUE, ""), ("prbs", ORANGE, ""), ("chirp", GREEN, "")],
        "Measured shaft position (rad)", (-0.35, 0.35),
        [("doublet_torque", BLUE, "7 5"), ("prbs_torque", ORANGE, "7 5"), ("chirp_torque", GREEN, "7 5")],
        "Requested servo torque (N·m)", (-0.2, 0.2),
        [("doublet", BLUE), ("PRBS", ORANGE), ("chirp", GREEN),
         ("doublet torque", BLUE), ("PRBS torque", ORANGE), ("chirp torque", GREEN)],
    )


def main() -> None:
    (ROOT / "excitation-heater-comparison.svg").write_text(render_heater(), encoding="utf-8")
    (ROOT / "excitation-servo-comparison.svg").write_text(render_servo(), encoding="utf-8")


if __name__ == "__main__":
    main()
