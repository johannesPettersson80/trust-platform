#!/usr/bin/env python3
"""Generate the Lesson 1 diagram and plot from its retained simulator trace."""

from __future__ import annotations

import csv
from pathlib import Path


ROOT = Path(__file__).resolve().parent
FONT = "system-ui, -apple-system, BlinkMacSystemFont, 'Segoe UI', sans-serif"
STYLE = f"""
  svg {{ forced-color-adjust: none; color-scheme: light; }}
  .surface {{ fill: #f8fafc; stroke: #cbd5e1; stroke-width: 2; }}
  text {{ font-family: {FONT}; fill: #172033; }}
  .title {{ font-size: 20px; font-weight: 650; }}
  .subtitle {{ font-size: 12px; fill: #4b5568; }}
  .box {{ fill: #ffffff; stroke: #526076; stroke-width: 1.5; }}
  .axis {{ stroke: #526076; stroke-width: 1.2; }}
  .grid {{ stroke: #cbd3df; stroke-width: 0.8; stroke-dasharray: 3 5; }}
  .signal {{ stroke: #1769aa; stroke-width: 2.2; fill: none; }}
  .surroundings {{ stroke: #26734d; stroke-width: 2; fill: none; stroke-dasharray: 7 4; }}
  .request {{ stroke: #7a3eb1; stroke-width: 2.2; fill: none; stroke-dasharray: 2 3; }}
  .applied {{ stroke: #c64b16; stroke-width: 2.2; fill: none; stroke-dasharray: 8 4; }}
  .disturbance {{ stroke: #9b3152; stroke-width: 1.8; fill: none; stroke-dasharray: 5 4; }}
  .door-band {{ fill: #f4d9e2; opacity: 0.42; }}
  .tick {{ font-size: 11px; fill: #465268; }}
  .label {{ font-size: 12px; font-weight: 600; }}
  .note {{ font-size: 11px; fill: #374258; }}
"""


def svg_document(title: str, description: str, body: str, height: int) -> str:
    return f"""<svg xmlns="http://www.w3.org/2000/svg" width="900" height="{height}" viewBox="0 0 900 {height}" role="img" aria-labelledby="title desc">
  <title id="title">{title}</title>
  <desc id="desc">{description}</desc>
  <style>{STYLE}</style>
  <rect x="1" y="1" width="898" height="{height - 2}" rx="8" class="surface" />
{body}
</svg>
"""


def text(x: float, y: float, value: str, css: str, anchor: str = "start") -> str:
    return f'  <text x="{x:.1f}" y="{y:.1f}" class="{css}" text-anchor="{anchor}">{value}</text>'


def line(x1: float, y1: float, x2: float, y2: float, css: str, marker: bool = False) -> str:
    suffix = ' marker-end="url(#arrow)"' if marker else ""
    return f'  <line x1="{x1:.1f}" y1="{y1:.1f}" x2="{x2:.1f}" y2="{y2:.1f}" class="{css}"{suffix} />'


def path(points: list[tuple[float, float]], css: str) -> str:
    commands = " ".join(("M" if index == 0 else "L") + f"{x:.2f},{y:.2f}" for index, (x, y) in enumerate(points))
    return f'  <path d="{commands}" class="{css}" />'


def loop_diagram() -> None:
    body = [
        '  <defs><marker id="arrow" markerWidth="8" markerHeight="8" refX="7" refY="4" orient="auto"><path d="M0,0 L8,4 L0,8 Z" fill="#526076" /></marker></defs>',
        text(48, 34, "The physical signals in the cold-room loop", "title"),
        text(48, 55, "Manual observation only · feedback is not closed in this lesson", "subtitle"),
        '  <rect x="55" y="112" width="160" height="76" rx="7" class="box" />',
        text(135, 140, "Operator request", "label", "middle"),
        text(135, 164, "Requested demand · %", "note", "middle"),
        '  <rect x="285" y="112" width="160" height="76" rx="7" class="box" />',
        text(365, 140, "Compressor", "label", "middle"),
        text(365, 164, "Applied demand · 0–100%", "note", "middle"),
        '  <rect x="515" y="112" width="160" height="76" rx="7" class="box" />',
        text(595, 140, "Cold room + contents", "label", "middle"),
        text(595, 164, "Process", "note", "middle"),
        '  <rect x="735" y="112" width="112" height="76" rx="7" class="box" />',
        text(791, 140, "Sensor", "label", "middle"),
        text(791, 164, "Temperature · °C", "note", "middle"),
        line(215, 150, 285, 150, "signal", True),
        line(445, 150, 515, 150, "signal", True),
        line(675, 150, 735, 150, "signal", True),
        line(595, 78, 595, 112, "disturbance", True),
        text(595, 69, "Door + wall heat disturbance", "note", "middle"),
        line(791, 188, 791, 244, "signal"),
        line(791, 244, 135, 244, "signal"),
        text(463, 265, "Temperature is observed; Lesson 2 uses it for automatic feedback", "note", "middle"),
        text(55, 292, "Desired temperature: 2 °C · Measured temperature: sensor reading", "note"),
    ]
    (ROOT / "cold-room-loop.svg").write_text(
        svg_document(
            "Cold-storage room loop elements",
            "The operator requests compressor demand, the bounded compressor acts on the cold room, the sensor reports temperature, and door and wall heat enter as a disturbance.",
            "\n".join(body),
            320,
        ),
        encoding="utf-8",
    )


def response_plot() -> None:
    with (ROOT / "manual-cooling-response.csv").open(newline="", encoding="utf-8") as handle:
        rows = [{key: (value if key == "evidence_class" else float(value)) for key, value in row.items()} for row in csv.DictReader(handle)]
    assert len(rows) == 1200
    plotted = rows[::6] + ([rows[-1]] if rows[-1] not in rows[::6] else [])
    left, right = 82.0, 850.0
    top1, bottom1 = 86.0, 274.0
    top2, bottom2 = 350.0, 538.0
    sx = lambda value: left + value / 120.0 * (right - left)
    sy_pv = lambda value: bottom1 - value / 7.0 * (bottom1 - top1)
    sy_mv = lambda value: bottom2 - value / 160.0 * (bottom2 - top2)
    body = [
        text(48, 31, "Manual actuation and an observed door disturbance", "title"),
        text(48, 52, "Simulation evidence · shipped ST · 100 ms samples · seed 20260828", "subtitle"),
        f'  <rect x="{sx(60):.1f}" y="{top1:.1f}" width="{sx(90) - sx(60):.1f}" height="{bottom1 - top1:.1f}" class="door-band" />',
    ]
    for value in [0, 1, 2, 3, 4, 5, 6, 7]:
        body.extend([line(left, sy_pv(value), right, sy_pv(value), "grid"), text(left - 10, sy_pv(value) + 4, str(value), "tick", "end")])
    for value in [0, 50, 100, 150]:
        body.extend([line(left, sy_mv(value), right, sy_mv(value), "grid"), text(left - 10, sy_mv(value) + 4, str(value), "tick", "end")])
    for value in [0, 20, 40, 60, 80, 100, 120]:
        body.extend([line(sx(value), top1, sx(value), bottom2, "grid"), text(sx(value), bottom2 + 20, str(value), "tick", "middle")])
    body.extend([
        line(left, top1, left, bottom1, "axis"), line(left, bottom1, right, bottom1, "axis"),
        line(left, top2, left, bottom2, "axis"), line(left, bottom2, right, bottom2, "axis"),
        f'  <text x="20" y="180" class="label" text-anchor="middle" transform="rotate(-90 20 180)">Cold-room temperature (°C)</text>',
        f'  <text x="20" y="444" class="label" text-anchor="middle" transform="rotate(-90 20 444)">Compressor demand (%)</text>',
        text((left + right) / 2, 580, "Time (s)", "label", "middle"),
        path([(sx(row["time_s"]), sy_pv(row["pv_degC"])) for row in plotted], "signal"),
        path([(sx(row["time_s"]), sy_pv(row["effective_surroundings_degC"])) for row in plotted], "surroundings"),
        path([(sx(row["time_s"]), sy_mv(row["requested_mv_percent"])) for row in plotted], "request"),
        path([(sx(row["time_s"]), sy_mv(row["applied_mv_percent"])) for row in plotted], "applied"),
        line(sx(20), top1, sx(20), bottom2, "disturbance"),
        text(sx(20) + 8, top1 + 16, "Command returns to safe state at 20 s", "note"),
        line(sx(60), top1, sx(60), bottom1, "disturbance"),
        text(sx(60) + 7, bottom1 - 8, "Door opens", "note"),
        line(sx(90), top1, sx(90), bottom1, "disturbance"),
        text(sx(90) + 7, bottom1 - 8, "Door closes", "note"),
        line(420, 68, 450, 68, "signal"), text(457, 72, "PV", "note"),
        line(500, 68, 530, 68, "surroundings"), text(537, 72, "Effective surroundings", "note"),
        line(665, 68, 695, 68, "request"), text(702, 72, "MV_requested", "note"),
        line(790, 68, 820, 68, "applied"), text(827, 72, "MV_applied", "note"),
    ])
    (ROOT / "manual-cooling-response.svg").write_text(
        svg_document(
            "Cold-room response to manual actuation and a door disturbance",
            "The actual simulator trace shows a 150 percent request clamped to 100 percent applied compressor demand, safe restoration at 20 seconds, and uncontrolled warming during the observed 60 to 90 second door load.",
            "\n".join(body),
            610,
        ),
        encoding="utf-8",
    )


if __name__ == "__main__":
    loop_diagram()
    response_plot()
