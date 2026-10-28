#!/usr/bin/env python3
"""Draws the Control Studio record of the 75 to 85 % step with the two candidate heaters, the two-store probe fit and the water reference that assigns the lags."""

from __future__ import annotations

import csv
from pathlib import Path


ROOT = Path(__file__).resolve().parent
FONT = "system-ui, -apple-system, BlinkMacSystemFont, 'Segoe UI', sans-serif"
ACCENT = "#b45309"
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
  .signal-b {{ stroke: #c64b16; stroke-width: 2.2; fill: none; stroke-dasharray: 8 4; }}
  .signal-c {{ stroke: #7a3eb1; stroke-width: 2.0; fill: none; stroke-dasharray: 2 3; }}
  .signal-d {{ stroke: #26734d; stroke-width: 2.0; fill: none; stroke-dasharray: 7 4; }}
  .signal-thin {{ stroke: #1769aa; stroke-width: 1.0; fill: none; }}
  .command {{ stroke: #334155; stroke-width: 1.6; fill: none; }}
  .surroundings {{ stroke: #26734d; stroke-width: 2; fill: none; stroke-dasharray: 7 4; }}
  .request {{ stroke: #7a3eb1; stroke-width: 2.2; fill: none; stroke-dasharray: 2 3; }}
  .applied {{ stroke: #c64b16; stroke-width: 2.2; fill: none; stroke-dasharray: 8 4; }}
  .disturbance {{ stroke: #9b3152; stroke-width: 1.8; fill: none; stroke-dasharray: 5 4; }}
  .reference {{ stroke: #475569; stroke-width: 1.4; fill: none; stroke-dasharray: 2 4; }}
  .limit {{ stroke: #9b3152; stroke-width: 1.4; fill: none; stroke-dasharray: 6 3; }}
  .door-band {{ fill: #f4d9e2; opacity: 0.42; }}
  .band {{ fill: #cfe8d8; opacity: 0.55; }}
  .tick {{ font-size: 11px; fill: #465268; }}
  .label {{ font-size: 12px; font-weight: 600; }}
  .note {{ font-size: 11px; fill: #374258; }}
  .halo {{ paint-order: stroke; stroke: #f8fafc; stroke-width: 4px; stroke-linejoin: round; }}
  .mark {{ fill: {ACCENT}; stroke: #ffffff; stroke-width: 1.4; }}
  .leader {{ stroke: {ACCENT}; stroke-width: 1.2; fill: none; }}
  .badge {{ fill: {ACCENT}; }}
  .badge-text {{ font-size: 11px; font-weight: 700; fill: #ffffff; }}
  .callout {{ font-size: 12px; fill: #172033; }}
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


def read_rows(name: str) -> list[dict[str, str]]:
    with (ROOT / name).open(newline="", encoding="utf-8") as handle:
        return list(csv.DictReader(handle))


def column(rows: list[dict[str, str]], key: str) -> list[float]:
    return [float(row[key]) for row in rows]


class Plot:
    """Stacked time plots with numbered marks that answer the lesson's predictions.

    Panels share the time window 0..t_max unless a panel sets its own "t0", "t1" and "t_ticks" (a zoom)."""

    LEFT, RIGHT = 82.0, 850.0

    def __init__(self, title: str, subtitle: str, t_max: float, t_ticks: list[float], panels: list[dict]) -> None:
        self.title, self.subtitle = title, subtitle
        self.body: list[str] = []
        self.over: list[str] = []
        self.legend: list[tuple[str, str]] = []
        top = 104.0
        for index, panel in enumerate(panels):
            panel.setdefault("t0", 0.0)
            panel.setdefault("t1", t_max)
            panel.setdefault("t_ticks", t_ticks)
            panel["top"] = top
            panel["bottom"] = top + panel.get("height", 170.0)
            following = panels[index + 1] if index + 1 < len(panels) else None
            panel["time_labels"] = following is None or (following.get("t0", 0.0), following.get("t1", t_max)) != (panel["t0"], panel["t1"])
            top = panel["bottom"] + panel.get("gap", 58.0 if panel["time_labels"] else 34.0)
        self.panels = panels
        self.height = int(panels[-1]["bottom"] + 64)

    def sx(self, t: float, index: int = 0) -> float:
        panel = self.panels[index]
        return self.LEFT + (t - panel["t0"]) / (panel["t1"] - panel["t0"]) * (self.RIGHT - self.LEFT)

    def sy(self, index: int, value: float) -> float:
        panel = self.panels[index]
        return panel["bottom"] - (value - panel["lo"]) / (panel["hi"] - panel["lo"]) * (panel["bottom"] - panel["top"])

    def series(self, index: int, ts: list[float], values: list[float], css: str, legend: str | None = None, step: bool = False) -> None:
        """A step series holds each sample until the next one, as a PLC output does between updates."""
        panel = self.panels[index]
        points = [(self.sx(t, index), self.sy(index, v)) for t, v in zip(ts, values) if panel["t0"] <= t <= panel["t1"]]
        if step:
            held: list[tuple[float, float]] = []
            for (x, y), (next_x, _) in zip(points, points[1:] + [(points[-1][0], 0.0)]):
                held.extend([(x, y), (next_x, y)])
            points = held
        self.body.append(path(points, css))
        if legend:
            self.legend.append((css, legend))

    def hline(self, index: int, value: float, css: str, label: str | None = None, at: float | None = None, below: bool = False) -> None:
        y = self.sy(index, value)
        self.body.append(line(self.LEFT, y, self.RIGHT, y, css))
        if label:
            x = self.sx(at, index) if at is not None else self.RIGHT - 4
            self.over.append(text(x, y + (14 if below else -5), label, "note halo", "start" if at is not None else "end"))

    def band(self, index: int, lo: float, hi: float, t0: float, t1: float, label: str | None = None) -> None:
        x0, x1 = self.sx(t0, index), self.sx(t1, index)
        y0, y1 = self.sy(index, hi), self.sy(index, lo)
        self.body.append(f'  <rect x="{x0:.1f}" y="{y0:.1f}" width="{x1 - x0:.1f}" height="{y1 - y0:.1f}" class="band" />')
        if label:
            self.over.append(text(x0 + 4, y0 - 4, label, "note halo"))

    def span(self, t0: float, t1: float, label: str | None = None, panels: list[int] | None = None) -> None:
        chosen = panels if panels is not None else list(range(len(self.panels)))
        for index in chosen:
            panel = self.panels[index]
            x0, x1 = max(self.sx(t0, index), self.LEFT), min(self.sx(t1, index), self.RIGHT)
            if x1 > x0:
                self.body.append(f'  <rect x="{x0:.1f}" y="{panel["top"]:.1f}" width="{x1 - x0:.1f}" height="{panel["bottom"] - panel["top"]:.1f}" class="door-band" />')
        if label:
            self.over.append(text(self.sx(t0, chosen[0]) + 5, self.panels[chosen[0]]["top"] + 14, label, "note halo"))

    def vline(self, t: float, label: str | None = None, panels: list[int] | None = None, label_y: float | None = None) -> None:
        chosen = panels if panels is not None else list(range(len(self.panels)))
        x = self.sx(t, chosen[0])
        self.body.append(line(x, self.panels[chosen[0]]["top"], x, self.panels[chosen[-1]]["bottom"], "disturbance"))
        if label:
            self.over.append(text(x + 5, label_y if label_y is not None else self.panels[chosen[0]]["top"] + 14, label, "note halo"))

    def mark(self, index: int, t: float, value: float, number: int, label: str, dx: float, dy: float) -> None:
        px, py = self.sx(t, index), self.sy(index, value)
        bx, by = px + dx, py + dy
        distance = max((dx * dx + dy * dy) ** 0.5, 1.0)
        ex, ey = bx - dx / distance * 9.0, by - dy / distance * 9.0
        self.over.append(line(px, py, ex, ey, "leader"))
        self.over.append(f'  <circle cx="{px:.1f}" cy="{py:.1f}" r="4" class="mark" />')
        self.over.append(f'  <circle cx="{bx:.1f}" cy="{by:.1f}" r="9" class="badge" />')
        self.over.append(text(bx, by + 4, str(number), "badge-text", "middle"))
        lines = label.split("\n")
        anchor = "start" if dx >= 0 else "end"
        lx = bx + 14 if dx >= 0 else bx - 14
        first = by + 4 - (len(lines) - 1) * 7
        for offset, value_text in enumerate(lines):
            self.over.append(text(lx, first + offset * 14, value_text, "callout halo", anchor))

    def render(self, name: str, accessible_title: str, description: str) -> None:
        frame = [
            text(48, 31, self.title, "title"),
            text(48, 52, self.subtitle, "subtitle"),
        ]
        for index, panel in enumerate(self.panels):
            for value in panel["ticks"]:
                y = self.sy(index, value)
                frame.extend([line(self.LEFT, y, self.RIGHT, y, "grid"), text(self.LEFT - 10, y + 4, panel.get("format", "{:g}").format(value), "tick", "end")])
            for t in panel["t_ticks"]:
                x = self.sx(t, index)
                frame.append(line(x, panel["top"], x, panel["bottom"], "grid"))
                if panel["time_labels"]:
                    frame.append(text(x, panel["bottom"] + 20, f"{t:g}", "tick", "middle"))
            middle = (panel["top"] + panel["bottom"]) / 2
            frame.extend([
                f'  <text x="20" y="{middle:.1f}" class="label" text-anchor="middle" transform="rotate(-90 20 {middle:.1f})">{panel["label"]}</text>',
                line(self.LEFT, panel["top"], self.LEFT, panel["bottom"], "axis"),
                line(self.LEFT, panel["bottom"], self.RIGHT, panel["bottom"], "axis"),
            ])
            if panel["time_labels"]:
                frame.append(text((self.LEFT + self.RIGHT) / 2, panel["bottom"] + 42, panel.get("time_label", "Time (s)"), "label", "middle"))
        x = 48.0
        for css, label in self.legend:
            frame.extend([line(x, 76, x + 30, 76, css), text(x + 37, 80, label, "note")])
            x += 37 + len(label) * 6.2 + 26
        (ROOT / name).write_text(
            svg_document(accessible_title, description, "\n".join(frame + self.body + self.over), self.height),
            encoding="utf-8",
        )


import math

STEP = 5.0


def first_order(time: float, gain: float, lag: float, delay: float) -> float:
    after = time - STEP - delay
    return 35.0 if after < 0 else 35.0 + gain * 10 * (1 - math.exp(-after / lag))


def two_lags(time: float, first: float, second: float) -> float:
    """Control Studio's two-store fit of the noisy probe: gain 0.199262 °C/%, delay 1.1 s; the lags enter symmetrically."""
    after = time - STEP - 1.1
    return 35.0 if after < 0 else 35.0 + 0.199262 * 10 * (1 - (first * math.exp(-after / first) - second * math.exp(-after / second)) / (first - second))


def response_plot() -> None:
    rows = [row for row in read_rows("identifiability-record.csv") if float(row["time_s"]) <= 45.0]
    t = column(rows, "time_s")
    probe = column(rows, "probe_degC")
    water = column(rows, "water_degC")
    at = {round(time, 1): index for index, time in enumerate(t)}
    plot = Plot(
        "What the step record can tell apart",
        "Control Studio recording · step 75 → 85 % at 5.0 s · probe measured with a bounded ±0.05 °C error · numbers answer the predictions",
        45.0,
        [0, 5, 10, 15, 20, 25, 30, 35, 40, 45],
        [
            {"label": "Water: two heaters (°C)", "lo": 34.8, "hi": 39.3, "ticks": [35, 36, 37, 38, 39], "height": 150.0},
            {"label": "Probe (°C)", "lo": 34.8, "hi": 37.3, "ticks": [35, 35.5, 36, 36.5, 37], "format": "{:g}", "height": 150.0},
            {"label": "Water: whose lag (°C)", "lo": 34.8, "hi": 37.3, "ticks": [35, 35.5, 36, 36.5, 37], "format": "{:g}", "height": 150.0},
        ],
    )
    plot.vline(STEP, "Step", [0, 1, 2], label_y=plot.panels[0]["top"] + 14)
    plot.series(0, t, [first_order(time, 0.4, 10.0, 1.0) for time in t], "signal-d", "Alternative that does not fit")
    plot.series(0, t, [first_order(time, 0.2, 5.0, 1.0) for time in t], "signal-b", "Model that fits")
    plot.series(0, t, water, "signal-c", "Water reference")
    plot.series(1, t, probe, "signal-thin", "Measured probe")
    plot.series(1, t, [two_lags(time, 5.1, 1.8) for time in t], "signal-b")
    plot.series(2, t, water, "signal-c")
    plot.series(2, t, [first_order(time, 0.199262, 1.8, 1.1) for time in t], "signal-d")
    plot.series(2, t, [first_order(time, 0.199262, 5.1, 1.1) for time in t], "signal-b")
    for index, time, value, label, anchor in (
        (0, 44.5, 38.2, "Heater K 0.4 °C/%, τ 10 s", "end"),
        (0, 44.5, 36.62, "Heater K 0.2 °C/%, τ 5 s", "end"),
        (1, 44.5, 36.62, "Two-store fit: lags 5.1 s and 1.8 s, either order", "end"),
        (2, 13.5, 37.1, "Water lag 1.8 s", "start"),
        (2, 44.5, 36.62, "Water lag 5.1 s", "end"),
    ):
        plot.over.append(text(plot.sx(time), plot.sy(index, value), label, "note halo", anchor))
    plot.mark(0, 7.0, water[at[7.0]], 1, "1 s after the delay: 0.018 °C apart, too close", 70, 10)
    plot.mark(0, 16.0, water[at[16.0]], 1, "10 s after: 0.799 °C apart; the water follows K 0.2, τ 5 s", 50, -20)
    plot.mark(1, 12.0, probe[at[12.0]], 2, "Fitted lags 5.1 s and 1.8 s, delay 1.1 s: the error shifts them from 5 s and 2 s", 50, 40)
    plot.mark(1, 20.0, two_lags(20.0, 5.1, 1.8), 3, "Either order gives this curve: the fit cannot say which lag is the probe's", 30, 44)
    plot.mark(2, 11.0, water[at[11.0]], 4, "63.2 % at 6.0 s after the step: the water's lag is 5 s, so 1.8 s is the probe's", 60, 40)
    plot.render(
        "identifiability-record.svg",
        "Identifiability of the heater and probe lags",
        "Early in the step, a heater with gain 0.2 and time constant 5 seconds and one with gain 0.4 and 10 seconds differ by only 0.018 degrees; after 10 seconds they differ by 0.8 degrees and the water follows the first. The two-store fit of the noisy probe gives lags of 5.1 and 1.8 seconds that fit equally well in either order. The water reference reaches 63.2 percent of its change 6 seconds after the step, so the water's lag is 5 seconds and the shorter lag belongs to the probe.",
    )


if __name__ == "__main__":
    response_plot()
