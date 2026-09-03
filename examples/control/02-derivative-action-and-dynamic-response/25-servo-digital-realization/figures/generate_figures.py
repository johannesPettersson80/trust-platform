#!/usr/bin/env python3
"""Render Lesson 25's four figures from their retained native traces.

Pure Python, no third-party dependencies. Reads the four
digital-*-trace.csv files in this directory and writes the matching SVGs.
"""

from __future__ import annotations

import csv
from pathlib import Path

ROOT = Path(__file__).resolve().parent
INK = "#243044"
GRID = "#d8e0eb"
AXIS = "#526078"
GREY = "#8a8f98"
RED = "#b4232f"
ORANGE = "#d55e00"
BLUE = "#0072b2"
GREEN = "#009e73"
PAPER = "#f7f9fc"

LEFT, RIGHT = 70.0, 860.0
TOP_A, BOT_A = 96.0, 300.0
TOP_B, BOT_B = 345.0, 505.0


def scale(lo: float, hi: float, top: float, bottom: float):
    def s(v: float) -> float:
        return bottom - (v - lo) / (hi - lo) * (bottom - top)
    return s


def sx(t: float, tmax: float) -> float:
    return LEFT + t / tmax * (RIGHT - LEFT)


def polyline(points: list[tuple[float, float]], colour: str, dash: str = "") -> str:
    encoded = " ".join(f"{x:.2f},{y:.2f}" for x, y in points)
    d = f' stroke-dasharray="{dash}"' if dash else ""
    return f'<polyline fill="none" stroke="{colour}" stroke-width="2" points="{encoded}"{d} />'


def frame(title: str, desc: str, label_a: str, label_b: str, body: str) -> str:
    return f'''<svg xmlns="http://www.w3.org/2000/svg" role="img" aria-labelledby="title desc" viewBox="0 0 900 560">
<title id="title">{title}</title>
<desc id="desc">{desc}</desc>
<rect width="900" height="560" fill="{PAPER}"/>
<style>text{{font-family:system-ui,-apple-system,sans-serif;fill:{INK}}} .axis{{stroke:{AXIS};stroke-width:1}} .grid{{stroke:{GRID};stroke-width:1}}</style>
<text x="55" y="42" font-size="22" font-weight="700">{title}</text>
<text x="55" y="66" font-size="13">{desc}</text>
<rect class="axis" x="{LEFT}" y="{TOP_A}" width="{RIGHT-LEFT:.1f}" height="{BOT_A-TOP_A:.1f}" fill="none"/>
<rect class="axis" x="{LEFT}" y="{TOP_B}" width="{RIGHT-LEFT:.1f}" height="{BOT_B-TOP_B:.1f}" fill="none"/>
<text x="{LEFT}" y="{TOP_A - 12:.1f}" font-size="16" font-weight="700">{label_a}</text>
<text x="{LEFT}" y="{TOP_B - 12:.1f}" font-size="16" font-weight="700">{label_b}</text>
<text x="{(LEFT+RIGHT)/2:.1f}" y="{BOT_B + 38:.1f}" text-anchor="middle" font-size="13" font-weight="700">Time after start (s)</text>
{body}
</svg>
'''


def grid_lines(values: list[float], top: float, bottom: float, s) -> str:
    out = []
    for v in values:
        y = s(v)
        out.append(f'<line class="grid" x1="{LEFT}" y1="{y:.2f}" x2="{RIGHT}" y2="{y:.2f}"/>'
                    f'<text x="{LEFT - 8:.1f}" y="{y + 4:.1f}" text-anchor="end" font-size="11">{v:g}</text>')
    return "".join(out)


def time_grid(tmax: float, ticks: list[float]) -> str:
    out = []
    for t in ticks:
        x = sx(t, tmax)
        out.append(f'<line class="grid" x1="{x:.2f}" y1="{TOP_A}" x2="{x:.2f}" y2="{BOT_A}"/>'
                    f'<line class="grid" x1="{x:.2f}" y1="{TOP_B}" x2="{x:.2f}" y2="{BOT_B}"/>'
                    f'<text x="{x:.2f}" y="{BOT_B + 18:.1f}" text-anchor="middle" font-size="11">{t:g}</text>')
    return "".join(out)


def render_realization() -> None:
    with (ROOT / "digital-realization-trace.csv").open(newline="", encoding="utf-8") as h:
        rows = list(csv.DictReader(h))
    tmax = 20.0
    sp = scale(-0.05, 0.55, TOP_A, BOT_A)
    st = scale(-1.0, 10.0, TOP_B, BOT_B)
    sp_pts = [(sx(float(r["time_s"]), tmax), sp(float(r["sp"]))) for r in rows]
    ideal_pts = [(sx(float(r["time_s"]), tmax), sp(float(r["ideal"]))) for r in rows]
    slow_pts = [(sx(float(r["time_s"]), tmax), sp(float(r["100_ms"]))) for r in rows]
    ideal_trq = [(sx(float(r["time_s"]), tmax), st(min(float(r["10_ms_torque"]), 10.0))) for r in rows]
    slow_trq = [(sx(float(r["time_s"]), tmax), st(min(float(r["100_ms_torque"]), 10.0))) for r in rows]
    body = (
        grid_lines([0.0, 0.25, 0.5], TOP_A, BOT_A, sp)
        + grid_lines([0, 2, 4, 6, 8, 10], TOP_B, BOT_B, st)
        + time_grid(tmax, [0, 5, 10, 15, 20])
        + polyline(sp_pts, GREY, "3 3")
        + polyline(ideal_pts, GREEN)
        + polyline(slow_pts, RED)
        + polyline(ideal_trq, GREEN)
        + polyline(slow_trq, RED)
        + f'<line x1="55" y1="96" x2="83" y2="96" stroke="{GREY}" stroke-width="2" stroke-dasharray="3 3"/><text x="89" y="101" font-size="12">SP</text>'
        + f'<line x1="130" y1="96" x2="158" y2="96" stroke="{GREEN}" stroke-width="2"/><text x="164" y="101" font-size="12">10 ms position/torque</text>'
        + f'<line x1="400" y1="96" x2="428" y2="96" stroke="{RED}" stroke-width="2"/><text x="434" y="101" font-size="12">100 ms position/torque</text>'
    )
    (ROOT / "digital-realization.svg").write_text(
        frame("Digital choices change the implemented controller",
              "Configured Simulation evidence &#183; same PID, plant, and 0.5 rad step",
              "Table position (rad)", "Applied torque (N m)", body),
        encoding="utf-8")


def render_method_comparison() -> None:
    with (ROOT / "digital-method-comparison-trace.csv").open(newline="", encoding="utf-8") as h:
        rows = list(csv.DictReader(h))
    tmax = 20.0
    sp = scale(-0.05, 0.55, TOP_A, BOT_A)
    st = scale(-1.0, 10.0, TOP_B, BOT_B)
    series = [("fe_position", "fe_torque", RED, "Forward Euler"), ("be_position", "be_torque", BLUE, "Backward Euler"), ("tustin_position", "tustin_torque", GREEN, "Tustin")]
    body = grid_lines([0.0, 0.25, 0.5], TOP_A, BOT_A, sp) + grid_lines([0, 2, 4, 6, 8, 10], TOP_B, BOT_B, st) + time_grid(tmax, [0, 5, 10, 15, 20])
    legend_x = 55.0
    for pk, tk, colour, label in series:
        pos_pts = [(sx(float(r["time_s"]), tmax), sp(float(r[pk]))) for r in rows]
        trq_pts = [(sx(float(r["time_s"]), tmax), st(min(float(r[tk]), 10.0))) for r in rows]
        body += polyline(pos_pts, colour) + polyline(trq_pts, colour)
        body += f'<line x1="{legend_x:.1f}" y1="96" x2="{legend_x+28:.1f}" y2="96" stroke="{colour}" stroke-width="2"/><text x="{legend_x+34:.1f}" y="101" font-size="12">{label}</text>'
        legend_x += 34 + 11 * len(label) + 30
    (ROOT / "digital-method-comparison.svg").write_text(
        frame("Three integral realizations at 20 ms",
              "Configured Simulation evidence &#183; same PID, plant, 20 ms interval and 0.5 rad step",
              "Table position (rad)", "Applied torque (N m)", body),
        encoding="utf-8")


def render_update_timing() -> None:
    with (ROOT / "digital-update-timing-trace.csv").open(newline="", encoding="utf-8") as h:
        rows = [r for r in csv.DictReader(h) if float(r["time_s"]) <= 2.0]
    tmax = 2.0
    sp = scale(-0.1, 1.1, TOP_A, BOT_A)
    st = scale(-1.0, 10.0, TOP_B, BOT_B)
    pulses10 = [(sx(float(r["time_s"]), tmax), sp(float(r["10_ms_updates"]))) for r in rows]
    pulses100 = [(sx(float(r["time_s"]), tmax), sp(float(r["100_ms_updates"]))) for r in rows]
    trq10 = [(sx(float(r["time_s"]), tmax), st(min(float(r["10_ms_torque"]), 10.0))) for r in rows]
    trq100 = [(sx(float(r["time_s"]), tmax), st(min(float(r["100_ms_torque"]), 10.0))) for r in rows]
    body = (
        grid_lines([0.0, 0.5, 1.0], TOP_A, BOT_A, sp)
        + grid_lines([0, 2, 4, 6, 8, 10], TOP_B, BOT_B, st)
        + time_grid(tmax, [0, 0.5, 1.0, 1.5, 2.0])
        + polyline(pulses10, ORANGE)
        + polyline(pulses100, BLUE)
        + polyline(trq10, ORANGE)
        + polyline(trq100, BLUE)
        + f'<line x1="55" y1="96" x2="83" y2="96" stroke="{ORANGE}" stroke-width="2"/><text x="89" y="101" font-size="12">10 ms update pulse / torque</text>'
        + f'<line x1="330" y1="96" x2="358" y2="96" stroke="{BLUE}" stroke-width="2"/><text x="364" y="101" font-size="12">100 ms update pulse / torque</text>'
    )
    (ROOT / "digital-update-timing.svg").write_text(
        frame("Update-pulse timing showing held torque between scans",
              "Configured Simulation evidence &#183; first 2 s, 10 ms and 100 ms candidates",
              "Update pulse (1/0)", "Applied torque (N m)", body),
        encoding="utf-8")


def render_alias_spectrum() -> None:
    with (ROOT / "digital-alias-spectrum-trace.csv").open(newline="", encoding="utf-8") as h:
        rows = list(csv.DictReader(h))
    fmax = 5.0
    amax = max(float(r["measured_amplitude"]) for r in rows) * 1.15 or 0.003
    sf = lambda f: LEFT + f / fmax * (RIGHT - LEFT)
    sa = scale(0.0, amax, TOP_A, BOT_A)
    pts = [(sf(float(r["frequency_hz"])), sa(float(r["measured_amplitude"]))) for r in rows]
    body = [
        f'<rect class="axis" x="{LEFT}" y="{TOP_A}" width="{RIGHT-LEFT:.1f}" height="{BOT_A-TOP_A:.1f}" fill="none"/>',
        polyline(pts, BLUE),
    ]
    for v in [0.0, amax / 2, amax]:
        y = sa(v)
        body.append(f'<line class="grid" x1="{LEFT}" y1="{y:.2f}" x2="{RIGHT}" y2="{y:.2f}"/>'
                     f'<text x="{LEFT-8:.1f}" y="{y+4:.1f}" text-anchor="end" font-size="11">{v:.4f}</text>')
    for v in [0, 1, 2, 3, 4, 5]:
        x = sf(v)
        body.append(f'<line class="grid" x1="{x:.2f}" y1="{TOP_A}" x2="{x:.2f}" y2="{BOT_A}"/>'
                     f'<text x="{x:.2f}" y="{BOT_A+18:.1f}" text-anchor="middle" font-size="11">{v}</text>')
    body.append(f'<line x1="{sf(2.0):.2f}" y1="{TOP_A}" x2="{sf(2.0):.2f}" y2="{BOT_A}" stroke="{RED}" stroke-width="1.5" stroke-dasharray="5 4"/>')
    body.append(f'<text x="{sf(2.0)+4:.1f}" y="{TOP_A+14:.1f}" font-size="10" fill="{RED}">predicted 2 Hz alias</text>')
    body.append(f'<text x="{(LEFT+RIGHT)/2:.1f}" y="{BOT_A+38:.1f}" text-anchor="middle" font-size="13" font-weight="700">Frequency (Hz)</text>')
    body.append(f'<text x="{LEFT}" y="{TOP_A-12:.1f}" font-size="16" font-weight="700">Residual amplitude (rad)</text>')
    svg = f'''<svg xmlns="http://www.w3.org/2000/svg" role="img" aria-labelledby="title desc" viewBox="0 0 900 360">
<title id="title">Sampled residual spectrum at 100 ms</title>
<desc id="desc">Configured Simulation evidence &#183; 5-to-20 s window, 8 Hz interference sampled at 10 Hz</desc>
<rect width="900" height="360" fill="{PAPER}"/>
<style>text{{font-family:system-ui,-apple-system,sans-serif;fill:{INK}}} .axis{{stroke:{AXIS};stroke-width:1}} .grid{{stroke:{GRID};stroke-width:1}}</style>
<text x="55" y="42" font-size="22" font-weight="700">Sampled residual spectrum at 100 ms</text>
<text x="55" y="66" font-size="13">Configured Simulation evidence &#183; 5-to-20 s window, 8 Hz interference sampled at 10 Hz</text>
{"".join(body)}
</svg>
'''
    (ROOT / "digital-alias-spectrum.svg").write_text(svg, encoding="utf-8")


def main() -> None:
    render_realization()
    render_method_comparison()
    render_update_timing()
    render_alias_spectrum()


if __name__ == "__main__":
    main()
