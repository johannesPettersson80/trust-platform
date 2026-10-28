#!/usr/bin/env python3
"""Generate the Lesson 25 aliasing diagram and the annotated plot of the load trials Control Studio recorded with four timings."""

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

# The aliasing diagram keeps its own curve styles.
STYLE += """
  .fast {{ fill: none; stroke: #c64b16; stroke-width: 1.6; }}
  .slow {{ fill: none; stroke: #1769aa; stroke-width: 2.8; }}
  .sample {{ fill: #172033; stroke: #f8fafc; stroke-width: 1.5; }}
""".replace("{{", "{").replace("}}", "}")
SAMPLE_INTERVAL = 0.1


def sine(frequency: float, time: float) -> float:
    return math.sin(2.0 * math.pi * frequency * time)


def panel(top: float, bottom: float, fast_hz: float, slow: callable, labels: tuple[str, str, str]) -> list[str]:
    left, right = 82.0, 850.0
    sx = lambda value: left + value * (right - left)
    sy = lambda value: bottom - (value + 1.2) / 2.4 * (bottom - top)
    fine = [index / 1_000 for index in range(1_001)]
    samples = [index * SAMPLE_INTERVAL for index in range(11)]
    elements = []
    for value in (-1, 0, 1):
        elements.extend([line(left, sy(value), right, sy(value), "grid"), text(left - 10, sy(value) + 4, str(value), "tick", "end")])
    for value in samples:
        elements.extend([line(sx(value), top, sx(value), bottom, "grid"), text(sx(value), bottom + 18, f"{value:.1f}", "tick", "middle")])
    elements.extend([
        line(left, top, left, bottom, "axis"),
        line(left, sy(0), right, sy(0), "axis"),
        path([(sx(t), sy(sine(fast_hz, t))) for t in fine], "fast"),
        path([(sx(t), sy(slow(t))) for t in fine], "slow"),
    ])
    for t in samples:
        # The sample the controller stores lies on both curves.
        assert abs(sine(fast_hz, t) - slow(t)) < 1e-9
        elements.append(f'  <circle cx="{sx(t):.2f}" cy="{sy(slow(t)):.2f}" r="5" class="sample" />')
    elements.extend([
        line(left + 10, top - 12, left + 40, top - 12, "fast"), text(left + 47, top - 8, labels[0], "note"),
        line(left + 200, top - 12, left + 230, top - 12, "slow"), text(left + 237, top - 8, labels[1], "note"),
        f'  <circle cx="{left + 415:.2f}" cy="{top - 12:.2f}" r="5" class="sample" />',
        text(left + 427, top - 8, labels[2], "note"),
    ])
    return elements


def aliasing_figure() -> None:
    elements = [
        text(52, 32, "Different signals, identical samples", "title"),
        text(52, 53, "Sampled every 0.1 s (10 Hz), the controller receives only the dots", "subtitle"),
    ]
    elements += panel(100.0, 290.0, 8.0, lambda t: -sine(2.0, t),
                      ("8 Hz sine", "inverted 2 Hz sine", "samples: the same for both"))
    elements += panel(372.0, 562.0, 10.0, lambda t: 0.0,
                      ("10 Hz sine", "constant 0", "samples: all zero"))
    elements.append(text(466.0, 612.0, "Time (s)", "label", "middle"))
    (ROOT / "sampling-aliasing.svg").write_text(
        svg_document(
            "Aliasing at 10 Hz sampling",
            "Top: an 8 hertz sine and an inverted 2 hertz sine pass through the same eleven samples taken every 0.1 seconds. "
            "Bottom: a 10 hertz sine sampled once per cycle is zero at every sample, like a constant signal.",
            "\n".join(elements),
            636,
        ),
        encoding="utf-8",
    )


def response_plot() -> None:
    trials = [("load-trial", "signal", "Normal, 0.1 s"), ("delayed-update", "applied", "One update 3 s late"),
              ("slow-sampling", "signal-d", "Slow sampling, 1 s"), ("assumed-interval", "signal-c", "Assumed interval")]
    runs = {name: read_rows(f"timing-{name}.csv") for name, _, _ in trials}
    plot = Plot(
        "Timing faults during the 30 kW load trial",
        "Control Studio validation trials of this project · set B, β = 0.8, Tf = 1 s · numbers answer the predictions",
        60.0,
        [0, 10, 20, 30, 40, 50, 60],
        [
            {"label": "Water temperature (°C)", "lo": 32.9, "hi": 35.3, "ticks": [33, 33.5, 34, 34.5, 35], "format": "{:g}", "height": 220.0},
            {"label": "Heater command (%)", "lo": 70.0, "hi": 100.0, "ticks": [70, 80, 90, 100], "height": 130.0},
        ],
    )
    plot.band(0, 34.8, 35.2, 30.0, 60.0, "Required from 30 s: within 0.2 °C")
    plot.hline(0, 33.0, "limit", "Lowest at least 33 °C", at=40.0, below=True)
    for name, css, legend in trials:
        rows = runs[name]
        plot.series(0, column(rows, "time_s"), column(rows, "P1.TemperatureC"), css, legend)
        plot.series(1, column(rows, "time_s"), column(rows, "P1.AppliedPowerPct"), css, step=(name in ("slow-sampling", "assumed-interval")))
    low = min(runs["delayed-update"], key=lambda row: float(row["P1.TemperatureC"]))
    plot.mark(0, float(low["time_s"]), float(low["P1.TemperatureC"]), 3, "One late update: lowest 33.21 °C", 70, 0)
    slow = min(runs["slow-sampling"], key=lambda row: abs(float(row["time_s"]) - 18.9))
    plot.mark(0, 18.9, float(slow["P1.TemperatureC"]), 4, "1 s sampling settles in 18.9 s", 60, 30)
    at30 = min(runs["assumed-interval"], key=lambda row: abs(float(row["time_s"]) - 30.0))
    plot.mark(0, 30.0, float(at30["P1.TemperatureC"]), 5, "Assumed interval: still 0.78 °C low at 30 s", 50, 40)
    plot.render(
        "timing-load-trials.svg",
        "Timing faults during the load trial",
        "Under the same 30 kilowatt load, one update 3 seconds late lets the water fall to 33.21 degrees Celsius, 1 second sampling settles in 18.9 seconds like the normal trial, and a program that tells the controller 0.1 seconds while it updates every 0.5 seconds leaves the water 0.78 degrees low after 30 seconds.",
    )


if __name__ == "__main__":
    aliasing_figure()
    response_plot()
