#!/usr/bin/env python3
"""Render Lesson 45 evidence from the retained crane-routh-evidence CSVs.

Pure Python, no third-party dependencies. Reads the long-format trace CSV
(panel, series, point, x, y, x_unit, y_unit) and draws the pole-location and
time-domain panels used by the README. Axis titles are derived once from the
CSV's own x_unit/y_unit text, so a title never repeats a word inside a nested
parenthesis.
"""

from __future__ import annotations

import csv
from collections import OrderedDict
from pathlib import Path

ROOT = Path(__file__).resolve().parent
INK = "#243044"
MUTED = "#526078"
GRID = "#d8e0eb"
PAPER = "#f7f9fc"
COLOURS = {"K = 4": "#2673b8", "K = 6": "#8a6d1d", "K = 8": "#b43c32"}
DASH = {"K = 4": "", "K = 6": "7 5", "K = 8": ""}


def colour_for(name: str) -> str:
    for prefix, colour in COLOURS.items():
        if name == prefix or name.startswith(prefix + " "):
            return colour
    return INK


def dash_for(name: str) -> str:
    for prefix, dash in DASH.items():
        if name == prefix or name.startswith(prefix + " "):
            return dash
    return ""


def load_trace(stem: str) -> "OrderedDict[str, OrderedDict[str, list[tuple[float, float, str, str]]]]":
    panels: "OrderedDict[str, OrderedDict[str, list[tuple[float, float, str, str]]]]" = OrderedDict()
    with (ROOT / f"{stem}-trace.csv").open(newline="", encoding="utf-8") as handle:
        for row in csv.DictReader(handle):
            panel = panels.setdefault(row["panel"], OrderedDict())
            series = panel.setdefault(row["series"], [])
            series.append((float(row["x"]), float(row["y"]), row["x_unit"], row["y_unit"]))
    return panels


def axis_title(unit_text: str) -> str:
    """'position (m)' -> 'Position (m)'; never nests a second parenthesis."""
    return unit_text[:1].upper() + unit_text[1:]


def scale(value: float, lo: float, hi: float, a: float, b: float) -> float:
    if hi <= lo:
        return (a + b) / 2
    return a + (value - lo) / (hi - lo) * (b - a)


def nice_ticks(lo: float, hi: float, count: int = 5) -> list[float]:
    if hi <= lo:
        hi = lo + 1.0
    step = (hi - lo) / (count - 1)
    return [lo + i * step for i in range(count)]


def fmt(value: float) -> str:
    if abs(value) >= 100:
        return f"{value:.0f}"
    if abs(value) >= 10:
        return f"{value:.1f}"
    return f"{value:.2f}"


def text(x: float, y: float, value: str, css: str = "note", anchor: str = "start") -> str:
    return f'<text x="{x:.1f}" y="{y:.1f}" class="{css}" text-anchor="{anchor}">{value}</text>'


def polyline(points: list[tuple[float, float]], colour: str, width: float = 2.2, dash: str = "") -> str:
    encoded = " ".join(f"{x:.1f},{y:.1f}" for x, y in points)
    dashed = f' stroke-dasharray="{dash}"' if dash else ""
    return f'<polyline points="{encoded}" fill="none" stroke="{colour}" stroke-width="{width}"{dashed} />'


def marker(x: float, y: float, colour: str) -> str:
    arm = 5.5
    return (
        f'<line x1="{x - arm:.1f}" y1="{y - arm:.1f}" x2="{x + arm:.1f}" y2="{y + arm:.1f}" '
        f'stroke="{colour}" stroke-width="2.4" />'
        f'<line x1="{x - arm:.1f}" y1="{y + arm:.1f}" x2="{x + arm:.1f}" y2="{y - arm:.1f}" '
        f'stroke="{colour}" stroke-width="2.4" />'
    )


def draw_panel(
    origin_x: float, top: float, width: float, height: float,
    series_map: "OrderedDict[str, list[tuple[float, float, str, str]]]",
    downsample: int, use_markers: bool, extra: list[str],
    shade: "list[tuple[float, float, str]] | None" = None,
) -> tuple[list[str], list[str]]:
    all_x = [p[0] for pts in series_map.values() for p in pts]
    all_y = [p[1] for pts in series_map.values() for p in pts]
    pad_x = (max(all_x) - min(all_x)) * 0.08 or 1.0
    pad_y = (max(all_y) - min(all_y)) * 0.12 or 1.0
    lo_x, hi_x = min(all_x) - pad_x, max(all_x) + pad_x
    lo_y, hi_y = min(all_y) - pad_y, max(all_y) + pad_y
    left, right = origin_x, origin_x + width
    bottom = top + height
    sx = lambda v: scale(v, lo_x, hi_x, left, right)
    sy = lambda v: scale(v, lo_y, hi_y, bottom, top)

    body = [f'<rect x="{left:.1f}" y="{top:.1f}" width="{width:.1f}" height="{height:.1f}" fill="white" stroke="{GRID}" />']
    for x0, x1, colour in shade or []:
        body.append(f'<rect x="{sx(x0):.1f}" y="{top:.1f}" width="{sx(x1) - sx(x0):.1f}" height="{height:.1f}" fill="{colour}" />')
    for tick in nice_ticks(lo_y, hi_y):
        body.append(f'<line x1="{left:.1f}" y1="{sy(tick):.1f}" x2="{right:.1f}" y2="{sy(tick):.1f}" class="grid" />')
        body.append(text(left - 10, sy(tick) + 4, fmt(tick), anchor="end"))
    for tick in nice_ticks(lo_x, hi_x, 6):
        body.append(f'<line x1="{sx(tick):.1f}" y1="{top:.1f}" x2="{sx(tick):.1f}" y2="{bottom:.1f}" class="grid" />')
        body.append(text(sx(tick), bottom + 18, fmt(tick), anchor="middle"))
    if lo_x < 0 < hi_x:
        body.append(f'<line x1="{sx(0):.1f}" y1="{top:.1f}" x2="{sx(0):.1f}" y2="{bottom:.1f}" class="axiszero" />')
    if lo_y < 0 < hi_y:
        body.append(f'<line x1="{left:.1f}" y1="{sy(0):.1f}" x2="{right:.1f}" y2="{sy(0):.1f}" class="axiszero" />')
    body += extra

    legend: list[str] = []
    for name, points in series_map.items():
        colour = colour_for(name)
        screen = [(sx(x), sy(y)) for x, y, _, _ in points]
        if use_markers:
            for px, py in screen:
                body.append(marker(px, py, colour))
        else:
            step = max(1, len(screen) // 400) if downsample else 1
            reduced = screen[::step]
            if reduced[-1] != screen[-1]:
                reduced = reduced + [screen[-1]]
            body.append(polyline(reduced, colour, 2.3, dash_for(name)))
        legend.append((name, colour))

    x_unit = next(iter(series_map.values()))[0][2]
    y_unit = next(iter(series_map.values()))[0][3]
    body.append(text((left + right) / 2, bottom + 38, axis_title(x_unit), "label", "middle"))
    body.append(
        f'<text x="{left - 46:.1f}" y="{(top + bottom) / 2:.1f}" class="label" '
        f'text-anchor="middle" transform="rotate(-90 {left - 46:.1f} {(top + bottom) / 2:.1f})">{axis_title(y_unit)}</text>'
    )
    return body, legend


def render() -> str:
    panels = load_trace("crane-routh-evidence")
    poles = panels["Closed-loop poles"]
    position = panels["Crane position"]

    early0, early1, late0, late1 = 2.01, 12.00, 30.01, 40.00
    window_shade = [(early0, early1, "#e7edf6"), (late0, late1, "#fbeee2")]

    pole_body, pole_legend = draw_panel(80, 130, 760, 210, poles, downsample=False, use_markers=True, extra=[])
    pos_body, pos_legend = draw_panel(80, 400, 760, 190, position, downsample=True, use_markers=False, extra=[], shade=window_shade)

    def legend_row(entries: list[tuple[str, str]], y: float) -> list[str]:
        row: list[str] = []
        lx = 80
        for name, colour in entries:
            row.append(f'<line x1="{lx}" y1="{y - 4:.1f}" x2="{lx + 26}" y2="{y - 4:.1f}" stroke="{colour}" stroke-width="3" />')
            row.append(text(lx + 32, y, name))
            lx += 175
        return row

    legend_svg = legend_row(pole_legend, 112) + legend_row(pos_legend, 655)

    return f'''<svg xmlns="http://www.w3.org/2000/svg" role="img" aria-labelledby="title desc" viewBox="0 0 900 680">
<title id="title">Crane stability prediction and measured motion</title>
<desc id="desc">Routh-Hurwitz predicts stable, marginal, and unstable classes for gains 4, 6, and 8; the retained simulator trace confirms decay, persistence, and growth.</desc>
<rect width="900" height="680" fill="{PAPER}" />
<style>
  text {{ font-family: system-ui, -apple-system, sans-serif; fill: {INK}; font-size: 12px; }}
  .grid {{ stroke: {GRID}; stroke-width: 1; }}
  .axiszero {{ stroke: {MUTED}; stroke-width: 1.2; }}
  .label {{ font-size: 13px; font-weight: 700; fill: {INK}; }}
  .note {{ fill: {MUTED}; }}
</style>
<text x="55" y="42" font-size="22" font-weight="700">Crane stability prediction and measured motion</text>
<text x="55" y="66" font-size="13" fill="{MUTED}">Simulation evidence &#183; three equivalent local crane copies &#183; seed 20260829</text>
{"".join(legend_svg)}
<text x="80" y="120" class="label">Closed-loop poles: real part vs. imaginary part</text>
{"".join(pole_body)}
<text x="80" y="390" class="label">Trolley position vs. time (shaded: early 2.01-12.00 s, late 30.01-40.00 s windows)</text>
{"".join(pos_body)}
</svg>
'''


def main() -> None:
    (ROOT / "crane-routh-evidence.svg").write_text(render(), encoding="utf-8")


if __name__ == "__main__":
    main()
