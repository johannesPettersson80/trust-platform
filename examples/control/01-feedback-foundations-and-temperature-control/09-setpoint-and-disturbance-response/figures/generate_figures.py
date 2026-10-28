#!/usr/bin/env python3
"""Generate the Lesson 9 annotated plot from its retained simulator trace."""

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


def response_plot() -> None:
    rows = read_rows("response-trials.csv")
    target = [row for row in rows if row["trial"] == "setpoint"]
    load = [row for row in rows if row["trial"] == "load"]
    assert len(target) == len(load) == 600
    t = column(target, "time_s")
    pv_target, pv_load = column(target, "pv_degC"), column(load, "pv_degC")
    plot = Plot(
        "Setpoint change and load change on the water heater",
        "Simulator runs of this project · two separate 60 s trials from 35 °C · numbers answer the predictions",
        60.0,
        [0, 10, 20, 30, 40, 50, 60],
        [
            {"label": "Setpoint trial (°C)", "lo": 34.5, "hi": 39.5, "ticks": [35, 36, 37, 38, 39], "height": 140.0},
            {"label": "Load trial (°C)", "lo": 32.5, "hi": 35.5, "ticks": [33, 34, 35], "height": 120.0},
            {"label": "Heater power (%)", "lo": 70.0, "hi": 95.0, "ticks": [70, 75, 80, 85, 90, 95], "height": 130.0},
        ],
    )
    plot.band(0, 37.8, 38.2, 30.0, 60.0, "Required: within 0.2 °C from 30 s")
    plot.band(1, 34.8, 35.2, 30.0, 60.0, "Required: within 0.2 °C from 30 s")
    plot.hline(0, 39.0, "limit", "Peak at most 39 °C", at=40.0)
    plot.hline(1, 33.0, "limit", "Lowest at least 33 °C", at=40.0, below=True)
    plot.series(0, t, column(target, "setpoint_degC"), "reference", "Setpoint")
    plot.series(0, t, pv_target, "signal", "Setpoint trial")
    plot.series(1, t, pv_load, "signal-b", "Load trial")
    plot.series(2, t, column(target, "applied_percent"), "signal")
    plot.series(2, t, column(load, "applied_percent"), "signal-b")
    plot.mark(2, 0.0, float(target[0]["applied_percent"]), 1, "Setpoint trial: 81.15 % at once", 300, 30)
    plot.mark(2, 0.0, float(load[0]["applied_percent"]), 2, "Load trial: still 75 %", 300, 20)
    last_target = max(float(row["time_s"]) for row in target if abs(float(row["pv_degC"]) - 38.0) > 0.2)
    plot.mark(0, last_target, pv_target[round(last_target * 10)], 3, "Inside 0.2 °C from 23.4 s: passes", 30, 34)
    low = min(range(600), key=lambda index: pv_load[index])
    plot.mark(1, low * 0.1, pv_load[low], 4, "Lowest 33.32 °C", 40, 24)
    last_load = max(float(row["time_s"]) for row in load if abs(float(row["pv_degC"]) - 35.0) > 0.2)
    plot.mark(1, last_load, pv_load[round(last_load * 10)], 5, "Outside 0.2 °C until 30.4 s: fails by 0.4 s", 30, 30)
    plot.render(
        "setpoint-and-load-trials.svg",
        "Setpoint change and load change on the water heater",
        "In the setpoint trial the heater command jumps to 81.15 percent before the water moves, and the temperature is inside 0.2 degrees Celsius of 38 from 23.4 seconds. In the load trial the command stays at 75 percent until the water cools, the temperature falls to 33.32 degrees and is outside 0.2 degrees of 35 until 30.4 seconds.",
    )


if __name__ == "__main__":
    response_plot()
