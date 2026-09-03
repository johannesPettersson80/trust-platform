#!/usr/bin/env python3
"""Render Lesson 46 evidence from the retained servo-root-locus-evidence CSVs.

Pure Python, no third-party dependencies. Reads the long-format trace CSV
(panel, series, point, x, y, x_unit, y_unit) and draws the root-locus and
carriage-position panels used by the README. Axis titles are derived once
from the CSV's own x_unit/y_unit text, so a title never repeats a word
inside a nested parenthesis. The CSV panel "Table position" is relabelled
"Carriage position" on the figure only (README and lesson.json already use
"carriage position"; the CSV itself is left untouched).
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
BLUE = "#2673b8"
AMBER = "#c9660a"
REQUIREMENT = "#7c8494"
HIGHLIGHT_BLUE = "#0b3d68"
HIGHLIGHT_AMBER = "#7a3c05"

# One explicit colour and stroke style per drawn series -> exactly one legend entry each.
SERIES_STYLE = {
    "P open-loop poles": (BLUE, "", "marker-x"),
    "P root-locus upper": (BLUE, "", "line"),
    "P root-locus lower": (BLUE, "", "line"),
    "P asymptote": (BLUE, "5 4", "line"),
    "Selected P poles": (HIGHLIGHT_BLUE, "", "marker-dot"),
    "Lead open-loop poles": (AMBER, "", "marker-x"),
    "Lead zero": (AMBER, "", "marker-o"),
    "Lead root-locus branch 1": (AMBER, "", "line"),
    "Lead root-locus branch 2": (AMBER, "", "line"),
    "Lead root-locus branch 3": (AMBER, "", "line"),
    "Lead asymptote": (AMBER, "5 4", "line"),
    "Selected lead poles": (HIGHLIGHT_AMBER, "", "marker-dot"),
    "Damping ratio 0.7": (REQUIREMENT, "3 4", "line"),
    "Damping ratio -0.7": (REQUIREMENT, "3 4", "line"),
    "P position": (BLUE, "", "line"),
    "Lead position": (AMBER, "", "line"),
}
PANEL_TITLES = {"Table position": "Carriage position"}
LEGEND_ORDER = [
    "P open-loop poles", "P root-locus upper", "P root-locus lower", "P asymptote",
    "Selected P poles", "Lead open-loop poles", "Lead zero", "Lead root-locus branch 1",
    "Lead root-locus branch 2", "Lead root-locus branch 3", "Lead asymptote",
    "Selected lead poles", "Damping ratio 0.7", "Damping ratio -0.7",
]


def load_trace(stem: str) -> "OrderedDict[str, OrderedDict[str, list[tuple[float, float, str, str]]]]":
    panels: "OrderedDict[str, OrderedDict[str, list[tuple[float, float, str, str]]]]" = OrderedDict()
    with (ROOT / f"{stem}-trace.csv").open(newline="", encoding="utf-8") as handle:
        for row in csv.DictReader(handle):
            panel = panels.setdefault(row["panel"], OrderedDict())
            series = panel.setdefault(row["series"], [])
            series.append((float(row["x"]), float(row["y"]), row["x_unit"], row["y_unit"]))
    return panels


def axis_title(unit_text: str) -> str:
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


def marker(x: float, y: float, colour: str, kind: str) -> str:
    arm = 6.0
    if kind == "marker-x":
        return (
            f'<line x1="{x - arm:.1f}" y1="{y - arm:.1f}" x2="{x + arm:.1f}" y2="{y + arm:.1f}" stroke="{colour}" stroke-width="2.6" />'
            f'<line x1="{x - arm:.1f}" y1="{y + arm:.1f}" x2="{x + arm:.1f}" y2="{y - arm:.1f}" stroke="{colour}" stroke-width="2.6" />'
        )
    if kind == "marker-o":
        return f'<circle cx="{x:.1f}" cy="{y:.1f}" r="6.5" fill="none" stroke="{colour}" stroke-width="2.6" />'
    return f'<circle cx="{x:.1f}" cy="{y:.1f}" r="5.5" fill="{colour}" stroke="white" stroke-width="1.2" />'


def draw_panel(
    origin_x: float, top: float, width: float, height: float,
    series_map: "OrderedDict[str, list[tuple[float, float, str, str]]]",
    panel_label: str, downsample: bool,
    domain: "tuple[float, float, float, float] | None" = None,
    clip_id: str = "",
) -> tuple[list[str], list[str]]:
    if domain:
        lo_x, hi_x, lo_y, hi_y = domain
    else:
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
    if clip_id:
        body.append(f'<clipPath id="{clip_id}"><rect x="{left:.1f}" y="{top:.1f}" width="{width:.1f}" height="{height:.1f}" /></clipPath>')
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

    legend: list[str] = []
    drawn = ['<g clip-path="url(#' + clip_id + ')">'] if clip_id else ["<g>"]
    for name, points in series_map.items():
        colour, dash, kind = SERIES_STYLE.get(name, (INK, "", "line"))
        screen = [(sx(x), sy(y)) for x, y, _, _ in points]
        if kind.startswith("marker"):
            for px, py in screen:
                drawn.append(marker(px, py, colour, kind))
        else:
            step = max(1, len(screen) // 300) if downsample else 1
            reduced = screen[::step]
            if reduced[-1] != screen[-1]:
                reduced = reduced + [screen[-1]]
            drawn.append(polyline(reduced, colour, 2.3, dash))
        legend.append((name, colour, dash, kind))
    drawn.append("</g>")
    body += drawn

    x_unit = next(iter(series_map.values()))[0][2]
    y_unit = next(iter(series_map.values()))[0][3]
    body.append(text((left + right) / 2, bottom + 38, axis_title(x_unit), "label", "middle"))
    body.append(
        f'<text x="{left - 46:.1f}" y="{(top + bottom) / 2:.1f}" class="label" '
        f'text-anchor="middle" transform="rotate(-90 {left - 46:.1f} {(top + bottom) / 2:.1f})">{axis_title(y_unit)}</text>'
    )
    body.append(text(left, top - 10, panel_label, "label"))
    return body, legend


def legend_block(entries: list[tuple[str, str, str, str]], x0: float, y0: float, per_row: int, row_h: float, col_w: float) -> list[str]:
    out: list[str] = []
    for i, (name, colour, dash, kind) in enumerate(entries):
        col, row = i % per_row, i // per_row
        lx = x0 + col * col_w
        ly = y0 + row * row_h
        if kind.startswith("marker"):
            out.append(marker(lx + 10, ly - 4, colour, kind))
        else:
            dashed = f' stroke-dasharray="{dash}"' if dash else ""
            out.append(f'<line x1="{lx:.1f}" y1="{ly - 4:.1f}" x2="{lx + 20:.1f}" y2="{ly - 4:.1f}" stroke="{colour}" stroke-width="3"{dashed} />')
        out.append(text(lx + 26, ly, name))
    return out


def render() -> str:
    panels = load_trace("servo-root-locus-evidence")
    locus = panels["Root locus and requirement region"]
    position = OrderedDict((PANEL_TITLES.get(k, k), v) for k, v in panels["Table position"].items())

    ordered_locus = OrderedDict((name, locus[name]) for name in LEGEND_ORDER)
    locus_body, _ = draw_panel(
        80, 158, 760, 225, ordered_locus, "Root locus and requirement region",
        downsample=False, domain=(-16.0, 1.0, -13.0, 13.0), clip_id="locusClip",
    )
    pos_body, pos_legend = draw_panel(80, 460, 760, 150, position, "Carriage position vs. time", downsample=True)

    legend_svg = legend_block(
        [(n, *SERIES_STYLE[n]) for n in LEGEND_ORDER], 80, 94, per_row=5, row_h=19, col_w=155,
    )
    pos_legend_svg = legend_block([(n, c, d, k) for n, c, d, k in pos_legend], 80, 672, per_row=4, row_h=20, col_w=175)

    return f'''<svg xmlns="http://www.w3.org/2000/svg" role="img" aria-labelledby="title desc" viewBox="0 0 900 700">
<title id="title">Servo pole placement and measured position</title>
<desc id="desc">Proportional-only root locus cannot leave real part -1; the lead compensator's zero and farther-left pole move the dominant poles into the required region, and the measured carriage position confirms both responses.</desc>
<rect width="900" height="700" fill="{PAPER}" />
<style>
  text {{ font-family: system-ui, -apple-system, sans-serif; fill: {INK}; font-size: 12px; }}
  .grid {{ stroke: {GRID}; stroke-width: 1; }}
  .axiszero {{ stroke: {MUTED}; stroke-width: 1.2; }}
  .label {{ font-size: 13px; font-weight: 700; fill: {INK}; }}
  .note {{ fill: {MUTED}; }}
</style>
<text x="55" y="42" font-size="22" font-weight="700">Servo pole placement and measured position</text>
<text x="55" y="66" font-size="13" fill="{MUTED}">Simulation evidence &#183; two equivalent local servo copies &#183; seed 20260829</text>
{"".join(legend_svg)}
{"".join(locus_body)}
{"".join(pos_body)}
{"".join(pos_legend_svg)}
</svg>
'''


def main() -> None:
    (ROOT / "servo-root-locus-evidence.svg").write_text(render(), encoding="utf-8")


if __name__ == "__main__":
    main()
