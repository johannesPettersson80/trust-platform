#!/usr/bin/env python3
"""Render Lesson 59's figures from retained data.

Pure standard library; no third-party dependencies.

``render_evidence()`` reads the checked-in
``heated-air-duct-smith-evidence-trace.csv`` (panel, series, point, x, y,
x_unit, y_unit rows) and regenerates ``heated-air-duct-smith-evidence.svg``
deterministically so the figure can never drift from the retained data.

``render_signal_path()`` regenerates the static Smith-predictor block diagram
``heated-air-duct-smith-signal-path.svg`` in the same hand-drawn box-and-arrow
style as Lesson 8's ``figures/heater-signal-path.svg``. It has no data
dependency; it is retained here so the diagram is a checked-in, deterministic
generation output rather than an unreproducible drawing.
"""

from __future__ import annotations

import csv
from pathlib import Path

ROOT = Path(__file__).resolve().parent
TRACE = ROOT / "heated-air-duct-smith-evidence-trace.csv"
EVIDENCE_OUTPUT = ROOT / "heated-air-duct-smith-evidence.svg"
SIGNAL_PATH_OUTPUT = ROOT / "heated-air-duct-smith-signal-path.svg"

INK = "#243044"
PAPER = "#f7f9fc"
GRID = "#d8e0eb"
AXIS = "#526078"

SVG_TITLE = "Smith prediction handles transport delay"
SVG_DESC = (
    "The nominal model improves the delayed duct response; mismatch "
    "correction exposes when fallback is required."
)

# (legend label, CSV series name, colour, dash pattern or None)
SERIES = [
    ("Ordinary PI", "Ordinary PI", "#6b7280", None),
    ("Nominal Smith", "Nominal Smith", "#2563eb", None),
    ("Moderate mismatch", "Moderate mismatch", "#b45309", "7 5"),
    ("Severe mismatch", "Severe mismatch", "#dc2626", "7 5"),
]

# (CSV panel name, displayed axis title "Quantity (unit)")
# The panel title names the same quantity the README defines: the
# downstream product temperature (y). It no longer repeats the word inside
# a nested parenthesis ("Outlet temperature (temperature (degC))").
PANEL_1 = ("Outlet temperature", "Downstream product temperature (degC)")
PANEL_2 = ("Heater command", "Heater command (%)")

LEFT, RIGHT = 75.0, 855.0
PANEL_TOP = [167.0, 417.0]
PANEL_HEIGHT = 160.0
CHAR_WIDTH = 7.3
LEGEND_GAP = 24.0


def load_series(path: Path) -> dict[tuple[str, str], list[tuple[float, float]]]:
    data: dict[tuple[str, str], list[tuple[float, float]]] = {}
    with path.open(newline="", encoding="utf-8") as handle:
        for row in csv.DictReader(handle):
            key = (row["panel"], row["series"])
            data.setdefault(key, []).append((float(row["x"]), float(row["y"])))
    for points in data.values():
        points.sort()
    return data


def text(x: float, y: float, value: str, size: int = 11, weight: str = "400",
          anchor: str = "start") -> str:
    weight_attr = f' font-weight="{weight}"' if weight != "400" else ""
    return (f'<text x="{x:.2f}" y="{y:.2f}" font-size="{size}"{weight_attr} '
            f'text-anchor="{anchor}">{value}</text>')


def grid_line(x1: float, y1: float, x2: float, y2: float) -> str:
    return f'<line class="grid" x1="{x1:.2f}" y1="{y1:.2f}" x2="{x2:.2f}" y2="{y2:.2f}"/>'


def polyline(points: list[tuple[float, float]], colour: str, dash: str | None) -> str:
    dash_attr = f' stroke-dasharray="{dash}"' if dash else ""
    encoded = " ".join(f"{x:.2f},{y:.2f}" for x, y in points)
    return (f'<polyline fill="none" stroke="{colour}" stroke-width="2.2"{dash_attr} '
            f'points="{encoded}"/>')


def render_legend(y: float) -> str:
    parts: list[str] = []
    x = 55.0
    for label, _series, colour, dash in SERIES:
        dash_attr = f' stroke-dasharray="{dash}"' if dash else ""
        parts.append(f'<line x1="{x:.1f}" y1="{y:.1f}" x2="{x + 28:.1f}" y2="{y:.1f}" '
                      f'stroke="{colour}" stroke-width="3"{dash_attr}/>')
        text_x = x + 34
        parts.append(text(text_x, y + 5, label, size=12))
        x = text_x + len(label) * CHAR_WIDTH + LEGEND_GAP
    return "".join(parts)


def render_panel(data: dict[tuple[str, str], list[tuple[float, float]]],
                  panel_csv_name: str, axis_title: str, top: float,
                  x_min: float, x_max: float) -> str:
    bottom = top + PANEL_HEIGHT
    series_points = {name: data[(panel_csv_name, name)] for _label, name, _c, _d in SERIES
                      if (panel_csv_name, name) in data}
    all_y = [y for points in series_points.values() for _x, y in points]
    y_min, y_max = min(all_y), max(all_y)
    if y_max == y_min:
        y_max = y_min + 1.0

    def sx(value: float) -> float:
        return LEFT + (value - x_min) / (x_max - x_min) * (RIGHT - LEFT)

    def sy(value: float) -> float:
        return bottom - (value - y_min) / (y_max - y_min) * PANEL_HEIGHT

    parts = [text(LEFT, top - 12, axis_title, size=16, weight="700")]
    for step in range(5):
        y_value = y_min + step * (y_max - y_min) / 4
        y_px = bottom - step * PANEL_HEIGHT / 4
        parts.append(grid_line(LEFT, y_px, RIGHT, y_px))
        parts.append(text(LEFT - 8, y_px + 4, f"{y_value:.2f}", anchor="end"))
    for step in range(5):
        x_value = x_min + step * (x_max - x_min) / 4
        x_px = LEFT + step * (RIGHT - LEFT) / 4
        parts.append(grid_line(x_px, top, x_px, bottom))
        parts.append(text(x_px, bottom + 18, f"{x_value:.2f}", anchor="middle"))
    for _label, name, colour, dash in SERIES:
        points = series_points.get(name)
        if not points:
            continue
        parts.append(polyline([(sx(x), sy(y)) for x, y in points], colour, dash))
    parts.append(f'<rect class="axis" x="{LEFT:.0f}" y="{top:.0f}" '
                  f'width="{RIGHT - LEFT:.0f}" height="{PANEL_HEIGHT:.0f}" fill="none"/>')
    parts.append(text((LEFT + RIGHT) / 2, bottom + 36, "time (s)", size=12, anchor="middle"))
    return "".join(parts)


def render_evidence() -> str:
    data = load_series(TRACE)
    all_x = [x for (panel, _series), points in data.items()
             for x, _y in points if panel in (PANEL_1[0], PANEL_2[0])]
    x_min, x_max = 0.0, max(all_x)

    body = [render_legend(118.0)]
    body.append(render_panel(data, PANEL_1[0], PANEL_1[1], PANEL_TOP[0], x_min, x_max))
    body.append(render_panel(data, PANEL_2[0], PANEL_2[1], PANEL_TOP[1], x_min, x_max))

    return f'''<svg xmlns="http://www.w3.org/2000/svg" role="img" aria-labelledby="title desc" viewBox="0 0 900 650">
<title id="title">{SVG_TITLE}</title>
<desc id="desc">{SVG_DESC}</desc>
<defs><marker id="arrow" markerWidth="8" markerHeight="8" refX="7" refY="3" orient="auto"><path d="M0,0 L0,6 L8,3 z" fill="{INK}"/></marker></defs>
<rect width="900" height="650" fill="{PAPER}"/>
<style>text{{font-family:system-ui,-apple-system,sans-serif;fill:{INK}}} .axis{{stroke:{AXIS};stroke-width:1}} .grid{{stroke:{GRID};stroke-width:1}}</style>
{text(55, 42, SVG_TITLE, size=24, weight="700")}
{text(55, 70, "The nominal model improves the delayed duct response; mismatch correction exposes when", size=14)}
{text(55, 88, "fallback is required.", size=14)}
{"".join(body)}
</svg>
'''


# --- Smith-predictor block diagram (hand-drawn style, matches Lesson 8) ---

def _box(x: float, y: float, w: float, h: float, fill: str, stroke: str,
          line1: str, line2: str = "") -> str:
    cx = x + w / 2
    parts = [f'<rect x="{x:.0f}" y="{y:.0f}" width="{w:.0f}" height="{h:.0f}" rx="10" '
              f'fill="{fill}" stroke="{stroke}" stroke-width="2"/>']
    if line2:
        parts.append(f'<text x="{cx:.0f}" y="{y + h / 2 - 6:.0f}" fill="#0f172a" '
                      f'font-size="17" font-weight="700" text-anchor="middle">{line1}</text>')
        parts.append(f'<text x="{cx:.0f}" y="{y + h / 2 + 18:.0f}" fill="#475569" '
                      f'font-size="13" text-anchor="middle">{line2}</text>')
    else:
        parts.append(f'<text x="{cx:.0f}" y="{y + h / 2 + 6:.0f}" fill="#0f172a" '
                      f'font-size="17" font-weight="700" text-anchor="middle">{line1}</text>')
    return "".join(parts)


def _hline(x1: float, y: float, x2: float, marker: bool = True) -> str:
    end = ' marker-end="url(#arrow-g)"' if marker else ""
    return f'<path d="M{x1:.0f} {y:.0f} H{x2:.0f}" fill="none" stroke="#475569" stroke-width="2.6"{end}/>'


def _elbow(points: list[tuple[float, float]], marker: bool = True, dash: bool = False) -> str:
    d = f"M{points[0][0]:.0f} {points[0][1]:.0f} " + " ".join(
        (f"H{x:.0f}" if points[i][1] == points[i - 1][1] else f"V{y:.0f}")
        for i, (x, y) in enumerate(points[1:], start=1)
    )
    end = ' marker-end="url(#arrow-g)"' if marker else ""
    dash_attr = ' stroke-dasharray="6 5"' if dash else ""
    return f'<path d="{d}" fill="none" stroke="#475569" stroke-width="2.6"{end}{dash_attr}/>'


def _label(x: float, y: float, value: str, colour: str = "#334155") -> str:
    return (f'<text x="{x:.0f}" y="{y:.0f}" fill="{colour}" font-family="system-ui, sans-serif" '
            f'font-size="13" font-weight="600" text-anchor="middle">{value}</text>')


def render_signal_path() -> str:
    blue_fill, blue_stroke = "#eff6ff", "#2563eb"
    green_fill, green_stroke = "#f0fdf4", "#16a34a"
    purple_fill, purple_stroke = "#f5f3ff", "#7c3aed"
    predictor_fill, predictor_stroke = "#ede9fe", "#6d28d9"
    orange_fill, orange_stroke = "#fff7ed", "#ea580c"

    controller = _box(30, 240, 190, 180, blue_fill, blue_stroke, "PID controller", "requests heater power")
    oven = _box(310, 130, 250, 90, green_fill, green_stroke, "Physical oven", "16 s transport delay")
    model0 = _box(310, 285, 250, 90, purple_fill, purple_stroke, "Delay-free model", "estimates y_m0 now")
    modelL = _box(310, 440, 250, 90, purple_fill, purple_stroke, "Delayed model", "estimates y_mL, 16 s late")
    predictor = (
        f'<rect x="700" y="250" width="280" height="220" rx="10" fill="{predictor_fill}" '
        f'stroke="{predictor_stroke}" stroke-width="2"/>'
        f'<text x="840" y="278" fill="#0f172a" font-size="17" font-weight="700" '
        f'text-anchor="middle">Predictor</text>'
    )
    predictor_lines = (
        f'{_label(840, 395, "c_m = y_meas − y_mL", "#4c1d95")}'
        f'{_label(840, 423, "y_pred = y_m0 + c_m", "#4c1d95")}'
        f'{_label(840, 451, "|c_m| ≥ 5 degC: fallback", "#7c3aed")}'
    )
    switch = _box(1040, 285, 230, 90, orange_fill, orange_stroke, "Fallback switch", "predicted or measured")

    bus_x = 260.0
    body = [
        controller, oven, model0, modelL, predictor, predictor_lines, switch,
        # heater command bus: controller -> bus -> oven / model0 / modelL
        _hline(220, 330, bus_x, marker=False),
        f'<path d="M{bus_x:.0f} 175 V485" fill="none" stroke="#475569" stroke-width="2.6"/>',
        _elbow([(bus_x, 175), (310, 175)]),
        _elbow([(bus_x, 330), (310, 330)]),
        _elbow([(bus_x, 485), (310, 485)]),
        _label(bus_x + 55, 218, "heater command (u)", "#166534"),
        # physical oven measured temperature branches to the predictor (y_meas)
        # and, separately, straight into the fallback switch's measured input.
        _elbow([(560, 175), (630, 175), (630, 295), (700, 295)]),
        _label(645, 240, "y_meas", "#166534"),
        _elbow([(630, 175), (1155, 175), (1155, 285)]),
        _label(900, 165, "y_meas (measured, always available)", "#166534"),
        # delay-free model -> predictor (y_m0)
        _elbow([(560, 330), (700, 330)]),
        _label(630, 320, "y_m0", "#5b21b6"),
        # delayed model -> predictor (y_mL)
        _elbow([(560, 485), (640, 485), (640, 365), (700, 365)]),
        _label(610, 473, "y_mL", "#5b21b6"),
        # predictor -> fallback switch (y_pred)
        _elbow([(980, 330), (1040, 330)]),
        _label(1010, 320, "y_pred", "#5b21b6"),
        # fallback switch -> controller (selected process value), long return path
        f'<path d="M1155 375 V555 H125 V420" fill="none" stroke="#ea580c" stroke-width="2.6" '
        f'marker-end="url(#arrow-orange)"/>',
        _label(640, 573, "selected process value (predicted, or measured on fallback) returns to the controller",
               "#9a3412"),
    ]

    return f'''<svg xmlns="http://www.w3.org/2000/svg" width="1300" height="620" viewBox="0 0 1300 620" role="img" aria-labelledby="title desc">
  <title id="title">Smith-predictor signal path for the heated-air duct</title>
  <desc id="desc">Heater command drives the physical oven and two internal models. Measured-minus-delayed-model correction produces a predicted current temperature. A fallback switch returns either that prediction or the raw measurement to the controller.</desc>
  <defs>
    <marker id="arrow-g" viewBox="0 0 10 10" refX="9" refY="5" markerWidth="7" markerHeight="7" orient="auto-start-reverse"><path d="M 0 0 L 10 5 L 0 10 z" fill="#475569"/></marker>
    <marker id="arrow-green" viewBox="0 0 10 10" refX="9" refY="5" markerWidth="7" markerHeight="7" orient="auto-start-reverse"><path d="M 0 0 L 10 5 L 0 10 z" fill="#16a34a"/></marker>
    <marker id="arrow-orange" viewBox="0 0 10 10" refX="9" refY="5" markerWidth="7" markerHeight="7" orient="auto-start-reverse"><path d="M 0 0 L 10 5 L 0 10 z" fill="#ea580c"/></marker>
  </defs>
  <rect width="1300" height="620" rx="18" fill="#f8fafc"/>
  <text x="40" y="47" fill="#0f172a" font-family="system-ui, sans-serif" font-size="24" font-weight="700">Two internal models predict what the sensor cannot show yet</text>
  <text x="40" y="76" fill="#475569" font-family="system-ui, sans-serif" font-size="16">Measured-minus-delayed-model correction anchors the estimate; a fallback switch always keeps measured feedback available.</text>
  {"".join(body)}
</svg>
'''


def main() -> None:
    EVIDENCE_OUTPUT.write_text(render_evidence(), encoding="utf-8")
    SIGNAL_PATH_OUTPUT.write_text(render_signal_path(), encoding="utf-8")


if __name__ == "__main__":
    main()
