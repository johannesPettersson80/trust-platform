#!/usr/bin/env python3
"""Render Lesson 21 from its two retained native traces.

Pure Python, no third-party dependencies. Reads loaded-position-pid-trace.csv
and loaded-position-recovery-trace.csv from this directory and writes
loaded-position-pid.svg and loaded-position-recovery.svg.
"""

from __future__ import annotations

import csv
from pathlib import Path

ROOT = Path(__file__).resolve().parent
INK = "#243044"
GRID = "#d8e0eb"
AXIS = "#526078"
ORANGE = "#d55e00"
BLUE = "#0072b2"
PAPER = "#f7f9fc"

LEFT, RIGHT = 70.0, 860.0
POS_TOP, POS_BOTTOM = 96.0, 300.0
TRQ_TOP, TRQ_BOTTOM = 345.0, 505.0
T_MIN, T_MAX = 0.0, 30.0


def make_scales(pos_min: float, pos_max: float, trq_min: float, trq_max: float):
    def sx(t: float) -> float:
        return LEFT + (t - T_MIN) / (T_MAX - T_MIN) * (RIGHT - LEFT)

    def sy_pos(v: float) -> float:
        return POS_BOTTOM - (v - pos_min) / (pos_max - pos_min) * (POS_BOTTOM - POS_TOP)

    def sy_trq(v: float) -> float:
        return TRQ_BOTTOM - (v - trq_min) / (trq_max - trq_min) * (TRQ_BOTTOM - TRQ_TOP)

    return sx, sy_pos, sy_trq


def polyline(points: list[tuple[float, float]], colour: str) -> str:
    encoded = " ".join(f"{x:.2f},{y:.2f}" for x, y in points)
    return f'<polyline fill="none" stroke="{colour}" stroke-width="2.2" points="{encoded}" />'


def frame(title: str, desc: str, pos_ticks: list[float], trq_ticks: list[float],
          pos_min: float, pos_max: float, trq_min: float, trq_max: float,
          series: list[tuple[list[tuple[float, float]], list[tuple[float, float]], str, str]]) -> str:
    sx, sy_pos, sy_trq = make_scales(pos_min, pos_max, trq_min, trq_max)
    body: list[str] = []
    for value in pos_ticks:
        y = sy_pos(value)
        body.append(f'<line class="grid" x1="{LEFT}" y1="{y:.2f}" x2="{RIGHT}" y2="{y:.2f}"/>'
                     f'<text x="{LEFT - 8:.1f}" y="{y + 4:.1f}" text-anchor="end" font-size="11">{value:g}</text>')
    for value in trq_ticks:
        y = sy_trq(value)
        body.append(f'<line class="grid" x1="{LEFT}" y1="{y:.2f}" x2="{RIGHT}" y2="{y:.2f}"/>'
                     f'<text x="{LEFT - 8:.1f}" y="{y + 4:.1f}" text-anchor="end" font-size="11">{value:g}</text>')
    for value in [0, 5, 10, 15, 20, 25, 30]:
        x = sx(value)
        body.append(f'<line class="grid" x1="{x:.2f}" y1="{POS_TOP}" x2="{x:.2f}" y2="{POS_BOTTOM}"/>')
        body.append(f'<line class="grid" x1="{x:.2f}" y1="{TRQ_TOP}" x2="{x:.2f}" y2="{TRQ_BOTTOM}"/>')
        body.append(f'<text x="{x:.2f}" y="{TRQ_BOTTOM + 18:.1f}" text-anchor="middle" font-size="11">{value:g}</text>')
    body += [
        f'<rect class="axis" x="{LEFT}" y="{POS_TOP}" width="{RIGHT-LEFT:.1f}" height="{POS_BOTTOM-POS_TOP:.1f}" fill="none"/>',
        f'<rect class="axis" x="{LEFT}" y="{TRQ_TOP}" width="{RIGHT-LEFT:.1f}" height="{TRQ_BOTTOM-TRQ_TOP:.1f}" fill="none"/>',
    ]
    legend_x = 55.0
    for pos_points, trq_points, colour, label in series:
        body.append(polyline([(sx(t), sy_pos(v)) for t, v in pos_points], colour))
        body.append(polyline([(sx(t), sy_trq(v)) for t, v in trq_points], colour))
        body.append(f'<line x1="{legend_x:.1f}" y1="96" x2="{legend_x+28:.1f}" y2="96" stroke="{colour}" stroke-width="3"/>'
                     f'<text x="{legend_x+34:.1f}" y="101" font-size="12">{label}</text>')
        legend_x += 34 + 11 * len(label) + 30
    body += [
        f'<text x="{LEFT}" y="{POS_TOP - 12:.1f}" font-size="16" font-weight="700">Table position (rad)</text>',
        f'<text x="{LEFT}" y="{TRQ_TOP - 12:.1f}" font-size="16" font-weight="700">Torque (N m)</text>',
        f'<text x="{(LEFT+RIGHT)/2:.1f}" y="{TRQ_BOTTOM + 38:.1f}" text-anchor="middle" font-size="13" font-weight="700">Time after start (s)</text>',
    ]
    return f'''<svg xmlns="http://www.w3.org/2000/svg" role="img" aria-labelledby="title desc" viewBox="0 0 900 560">
<title id="title">{title}</title>
<desc id="desc">{desc}</desc>
<rect width="900" height="560" fill="{PAPER}"/>
<style>text{{font-family:system-ui,-apple-system,sans-serif;fill:{INK}}} .axis{{stroke:{AXIS};stroke-width:1}} .grid{{stroke:{GRID};stroke-width:1}}</style>
<text x="55" y="42" font-size="22" font-weight="700">{title}</text>
<text x="55" y="66" font-size="13">{desc}</text>
{"".join(body)}
</svg>
'''


def render_pid() -> str:
    with (ROOT / "loaded-position-pid-trace.csv").open(newline="", encoding="utf-8") as handle:
        rows = list(csv.DictReader(handle))
    t = [float(r["time_s"]) for r in rows]
    pd_pos = list(zip(t, (float(r["pd_position"]) for r in rows)))
    pid_pos = list(zip(t, (float(r["pid_position"]) for r in rows)))
    pd_trq = list(zip(t, (float(r["pd_torque"]) for r in rows)))
    pid_trq = list(zip(t, (float(r["pid_torque"]) for r in rows)))
    return frame(
        "Integral action removes gravity-load position error",
        "Configured Simulation evidence · same sine-gravity table, Kp, Kd, limits, timing and axes",
        [0.0, 0.1, 0.2, 0.3, 0.4, 0.5], [0.0, 0.5, 1.0, 1.5, 2.0],
        -0.02, 0.55, -0.1, 2.1,
        [(pd_pos, pd_trq, ORANGE, "PD position / torque"), (pid_pos, pid_trq, BLUE, "PID position / torque")],
    )


def render_recovery() -> str:
    with (ROOT / "loaded-position-recovery-trace.csv").open(newline="", encoding="utf-8") as handle:
        rows = list(csv.DictReader(handle))
    t = [float(r["time_s"]) for r in rows]
    sp = list(zip(t, (float(r["sp"]) for r in rows)))
    pv = list(zip(t, (float(r["pid_position"]) for r in rows)))
    trq = list(zip(t, (float(r["applied_torque"]) for r in rows)))
    return frame(
        "Bounded PID recovers after torque saturation",
        "Configured Simulation evidence · SP is restored at 5 s; gains, load and torque limits remain fixed",
        [0.0, 0.5, 1.0, 1.5, 2.0], [-10.0, -5.0, 0.0, 5.0, 10.0],
        -0.1, 2.1, -10.5, 10.5,
        [(sp, [], "#8a8f98", "Setpoint"), (pv, trq, BLUE, "Position / applied torque")],
    )


def main() -> None:
    (ROOT / "loaded-position-pid.svg").write_text(render_pid(), encoding="utf-8")
    (ROOT / "loaded-position-recovery.svg").write_text(render_recovery(), encoding="utf-8")


if __name__ == "__main__":
    main()
