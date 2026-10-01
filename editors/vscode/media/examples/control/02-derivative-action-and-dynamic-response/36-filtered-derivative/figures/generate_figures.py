#!/usr/bin/env python3
"""Generate the Lesson 37 annotated plot from its Control Studio validation runs."""

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
    rows = read_rows("filtered-derivative-response.csv")
    pick = lambda scenario, case: [row for row in rows if row["scenario"] == scenario and row["case"] == case]
    hold0, hold1 = pick("loaded-hold", "tf0"), pick("loaded-hold", "tf01")
    step0, step5, step1 = pick("noisy-step", "tf0"), pick("noisy-step", "tf05"), pick("noisy-step", "tf01")
    assert len(hold0) == len(hold1) == len(step0) == len(step5) == len(step1) == 401
    t = column(hold0, "time_s")
    plot = Plot(
        "Filtering the speed that D uses",
        "Control Studio validation runs on the actual ST · same angle noise in every run · numbers answer the predictions",
        8.0,
        [0, 1, 2, 3, 4, 5, 6, 7, 8],
        [
            {"label": "Hold: torque (N·m)", "lo": 0.0, "hi": 4.2, "ticks": [0, 1, 2, 3, 4], "height": 160.0},
            {"label": "Noisy step: angle (rad)", "lo": 0.0, "hi": 1.45, "ticks": [0, 0.5, 1], "height": 180.0},
        ],
    )
    plot.series(0, t, column(hold0, "torque_nm"), "signal-b", "No filter (Tf 0)", step=True)
    plot.series(0, t, column(hold1, "torque_nm"), "signal", "Tf 0.1 s", step=True)
    plot.hline(1, 1.0, "reference")
    plot.series(1, t, column(step0, "angle_rad"), "signal-b")
    plot.series(1, t, column(step5, "angle_rad"), "signal-c", "Tf 0.5 s")
    plot.series(1, t, column(step1, "angle_rad"), "signal")
    plot.mark(0, 6.4, float(hold0[320]["torque_nm"]), 1, "No filter: 1.31–2.68 N·m while standing still", -30, -44)
    plot.mark(0, 2.6, float(hold1[130]["torque_nm"]), 2, "Tf 0.1 s: 1.89–2.05 N·m;\nactivity 20 instead of 177 N·m", 30, 50)
    peak5 = max(range(401), key=lambda i: float(step5[i]["angle_rad"]))
    plot.mark(1, t[peak5], float(step5[peak5]["angle_rad"]), 3, "Tf 0.5 s: D brakes too late, overshoot 0.27 rad", 34, -14)
    plot.mark(1, 5.0, float(step1[250]["angle_rad"]), 4, "Tf 0.1 s moves like no filter: overshoot 0.007 rad", 10, 64)
    plot.render(
        "filtered-derivative-response.svg",
        "Filtering the speed that D uses",
        "Without a filter the holding torque flickers between 1.31 and 2.68 newton-metres; a 0.1 second filter keeps it between 1.89 and 2.05 and moves the shaft as before, while a 0.5 second filter delays the braking so much that the step overshoots 0.27 radians.",
    )


if __name__ == "__main__":
    response_plot()
