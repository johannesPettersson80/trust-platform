#!/usr/bin/env python3
"""Render Lesson 37 from its retained reviewed-plan comparison trace."""

from __future__ import annotations

import csv
from pathlib import Path


ROOT = Path(__file__).resolve().parent
INK = "#243044"
MUTED = "#526078"
GRID = "#d8e0eb"
BLUE = "#0072b2"
GREEN = "#009e73"
ORANGE = "#d55e00"
PAPER = "#f7f9fc"


def text(x: float, y: float, value: str, size: int = 11, weight: str = "400", anchor: str = "start", fill: str = "") -> str:
    fill_attr = f' fill="{fill}"' if fill else ""
    return f'<text x="{x:.2f}" y="{y:.2f}" font-size="{size}" font-weight="{weight}" text-anchor="{anchor}"{fill_attr}>{value}</text>'


def polyline(points: list[tuple[float, float]], colour: str, dash: str = "") -> str:
    encoded = " ".join(f"{x:.2f},{y:.2f}" for x, y in points)
    dashed = f' stroke-dasharray="{dash}"' if dash else ""
    return f'<polyline fill="none" stroke="{colour}" stroke-width="2.2"{dashed} points="{encoded}" />'


def render() -> str:
    with (ROOT / "safe-experiment-comparison-trace.csv").open(newline="", encoding="utf-8") as handle:
        rows = list(csv.DictReader(handle))

    left, right = 70.0, 860.0
    t_top, t_bottom = 155.0, 315.0
    v_top, v_bottom = 360.0, 520.0
    t_min, t_max = 49.0, 56.0
    v_min, v_max = 55.0, 75.0
    t_end = float(rows[-1]["time_s"])

    sx = lambda t: left + t / t_end * (right - left)
    sy_t = lambda v: t_bottom - (v - t_min) / (t_max - t_min) * (t_bottom - t_top)
    sy_v = lambda v: v_bottom - (v - v_min) / (v_max - v_min) * (v_bottom - v_top)

    body: list[str] = []
    for i, value in enumerate([56.00, 54.25, 52.50, 50.75, 49.00]):
        y = t_top + i * (t_bottom - t_top) / 4
        body.append(f'<line class="grid" x1="{left}" y1="{y:.2f}" x2="{right}" y2="{y:.2f}"/>{text(left - 8, y + 4, f"{value:.2f}", anchor="end")}')
    for i, value in enumerate([75.00, 70.00, 65.00, 60.00, 55.00]):
        y = v_top + i * (v_bottom - v_top) / 4
        body.append(f'<line class="grid" x1="{left}" y1="{y:.2f}" x2="{right}" y2="{y:.2f}"/>{text(left - 8, y + 4, f"{value:.2f}", anchor="end")}')
    for i in range(7):
        tv = t_end * i / 6
        x = sx(tv)
        body.append(f'<line class="grid" x1="{x:.2f}" y1="{t_top}" x2="{x:.2f}" y2="{t_bottom}"/>{text(x, t_bottom + 18, f"{tv:.1f}", anchor="middle")}')
        body.append(f'<line class="grid" x1="{x:.2f}" y1="{v_top}" x2="{x:.2f}" y2="{v_bottom}"/>{text(x, v_bottom + 18, f"{tv:.1f}", anchor="middle")}')

    bad_quality_time = next((float(r["time_s"]) for r in rows if float(r["bad-quality_applied"]) < 65.0), None)
    if bad_quality_time is not None:
        bx = sx(bad_quality_time)
        body.append(f'<line class="marker" x1="{bx:.2f}" y1="{t_top}" x2="{bx:.2f}" y2="{t_bottom}"/>')
        body.append(text(bx + 5, t_top + 14, "bad quality", fill="#8c2f28"))

    body.append(polyline([(sx(float(r["time_s"])), sy_t(float(r["accepted"]))) for r in rows[::2]], BLUE))
    body.append(polyline([(sx(float(r["time_s"])), sy_t(float(r["bad-quality_abort"]))) for r in rows[::2]], ORANGE))
    body.append(polyline([(sx(float(r["time_s"])), sy_t(float(r["rejected_before_motion"]))) for r in rows[::2]], INK, "7 5"))
    body.append(f'<rect x="{left}" y="{t_top}" width="{right - left}" height="{t_bottom - t_top}" fill="none" stroke="#526078"/>')

    body.append(polyline([(sx(float(r["time_s"])), sy_v(float(r["accepted_requested"]))) for r in rows[::2]], BLUE, "7 5"))
    body.append(polyline([(sx(float(r["time_s"])), sy_v(float(r["accepted_applied"]))) for r in rows[::2]], GREEN))
    body.append(polyline([(sx(float(r["time_s"])), sy_v(float(r["bad-quality_applied"]))) for r in rows[::2]], ORANGE))
    body.append(polyline([(sx(float(r["time_s"])), sy_v(float(r["rejected_applied"]))) for r in rows[::2]], INK, "7 5"))
    body.append(f'<rect x="{left}" y="{v_top}" width="{right - left}" height="{v_bottom - v_top}" fill="none" stroke="#526078"/>')

    legend = (
        f'<line x1="55" y1="96" x2="83" y2="96" stroke="{BLUE}" stroke-width="3"/>{text(89, 101, "accepted")}'
        f'<line x1="155" y1="96" x2="183" y2="96" stroke="{ORANGE}" stroke-width="3"/>{text(189, 101, "bad-quality abort")}'
        f'<line x1="319" y1="96" x2="347" y2="96" stroke="{INK}" stroke-width="3"/>{text(353, 101, "rejected before motion")}'
        f'<line x1="55" y1="116" x2="83" y2="116" stroke="{GREEN}" stroke-width="3"/>{text(89, 121, "accepted applied")}'
        f'<line x1="212" y1="116" x2="240" y2="116" stroke="{ORANGE}" stroke-width="3"/>{text(246, 121, "bad-quality applied")}'
        f'<line x1="422" y1="116" x2="450" y2="116" stroke="{INK}" stroke-width="3"/>{text(456, 121, "rejected applied")}'
    )

    return f'''<svg xmlns="http://www.w3.org/2000/svg" role="img" aria-labelledby="title desc" viewBox="0 0 900 610">
<title id="title">Reviewed heat-exchanger outcomes</title>
<desc id="desc">Configured Simulation evidence: accepted, rejected-before-motion, and bad-quality abort valve plans and the resulting outlet-temperature traces</desc>
<rect width="900" height="610" fill="{PAPER}"/>
<style>text{{font-family:system-ui,-apple-system,sans-serif;fill:{INK}}} .axis{{stroke:#526078;stroke-width:1}} .grid{{stroke:{GRID};stroke-width:1}} .marker{{stroke:#b43c32;stroke-width:1.5;stroke-dasharray:4 4}}</style>
{text(55, 42, "Reviewed heat-exchanger outcomes", size=24, weight="700")}
{text(55, 68, "Configured Simulation evidence &#183; accepted, rejected-before-motion, and bad-quality abort", size=14)}
{legend}
{text(70, 143, "Outlet temperature (degC)", size=16, weight="700")}
{text(70, 348, "Valve position (%)", size=16, weight="700")}
{"".join(body)}
{text(465, 598, "time (s)", size=12, anchor="middle")}
</svg>
'''


def main() -> None:
    (ROOT / "safe-experiment-comparison.svg").write_text(render(), encoding="utf-8")


if __name__ == "__main__":
    main()
