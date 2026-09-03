#!/usr/bin/env python3
"""Render Lesson 59's topology diagram.

Pure standard library; no third-party dependencies.

``render_signal_path()`` regenerates the static Smith-predictor block diagram
``heated-air-duct-smith-signal-path.svg`` in the same hand-drawn box-and-arrow
style as Lesson 8's ``figures/heater-signal-path.svg``. It has no data
dependency; it is retained here so the diagram is a checked-in, deterministic
generation output rather than an unreproducible drawing.

``heated-air-duct-smith-evidence.svg`` is owned by the native test
control_curriculum_smith
(crates/trust-runtime/tests/control_curriculum_smith.rs). Refresh it with:
TRUST_UPDATE_CONTROL_CURRICULUM_ARTIFACTS=1 cargo test -p trust-runtime --test control_curriculum_smith
"""

from __future__ import annotations

from pathlib import Path

ROOT = Path(__file__).resolve().parent
SIGNAL_PATH_OUTPUT = ROOT / "heated-air-duct-smith-signal-path.svg"


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
    SIGNAL_PATH_OUTPUT.write_text(render_signal_path(), encoding="utf-8")


if __name__ == "__main__":
    main()
