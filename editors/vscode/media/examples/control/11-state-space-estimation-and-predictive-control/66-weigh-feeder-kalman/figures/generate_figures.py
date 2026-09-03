#!/usr/bin/env python3
"""Render Lesson 66's evidence figure from its retained native trace.

Pure Python, no third-party dependencies. Reads
weigh-feeder-kalman-evidence-trace.csv and writes
weigh-feeder-kalman-evidence.svg. Each axis title is written once (quantity
and unit are not duplicated), and every drawn series has a legend entry: the
mass-flow panel's four traces share the top legend, and the innovation
panel's two traces have their own legend directly above that panel.
"""

from __future__ import annotations

import csv
from pathlib import Path

ROOT = Path(__file__).resolve().parent
TRACE_CSV = ROOT / "weigh-feeder-kalman-evidence-trace.csv"
OUTPUT_SVG = ROOT / "weigh-feeder-kalman-evidence.svg"

INK = "#243044"
GRID = "#d8e0eb"


def load_rows() -> list[dict[str, str]]:
    with TRACE_CSV.open(newline="", encoding="utf-8") as handle:
        return list(csv.DictReader(handle))


def nice_ticks(lo: float, hi: float, count: int = 5) -> list[float]:
    if hi <= lo:
        hi = lo + 1.0
    span = hi - lo
    pad = span * 0.08
    lo -= pad
    hi += pad
    step = (hi - lo) / (count - 1)
    return [lo + i * step for i in range(count)]


def fmt(value: float) -> str:
    return f"{value:.2f}"


def render_panel(rows, panel_name, series_specs, x0, y0, w, h, title, x_label="time (s)") -> str:
    points = {
        name: [(float(r["x"]), float(r["y"])) for r in rows if r["panel"] == panel_name and r["series"] == name]
        for name, _color, _dash in series_specs
    }
    all_x = [x for pts in points.values() for x, _ in pts]
    all_y = [y for pts in points.values() for _, y in pts]
    x_ticks = nice_ticks(min(all_x), max(all_x))
    y_ticks = nice_ticks(min(all_y), max(all_y))
    x_lo, x_hi = x_ticks[0], x_ticks[-1]
    y_lo, y_hi = y_ticks[0], y_ticks[-1]

    def sx(v: float) -> float:
        return x0 + (v - x_lo) / (x_hi - x_lo) * w

    def sy(v: float) -> float:
        return y0 + h - (v - y_lo) / (y_hi - y_lo) * h

    body = [f'<text x="{x0}" y="{y0 - 12}" font-size="16" font-weight="700">{title}</text>']
    for yt in y_ticks:
        body.append(
            f'<line class="grid" x1="{x0}" y1="{sy(yt):.2f}" x2="{x0 + w}" y2="{sy(yt):.2f}"/>'
            f'<text x="{x0 - 8}" y="{sy(yt) + 4:.2f}" text-anchor="end" font-size="11">{fmt(yt)}</text>'
        )
    for xt in x_ticks:
        body.append(
            f'<line class="grid" x1="{sx(xt):.2f}" y1="{y0}" x2="{sx(xt):.2f}" y2="{y0 + h}"/>'
            f'<text x="{sx(xt):.2f}" y="{y0 + h + 18}" text-anchor="middle" font-size="11">{fmt(xt)}</text>'
        )
    for name, color, dash in series_specs:
        encoded = " ".join(f"{sx(x):.2f},{sy(y):.2f}" for x, y in points[name])
        dash_attr = f' stroke-dasharray="{dash}"' if dash else ""
        body.append(f'<polyline fill="none" stroke="{color}" stroke-width="2.2"{dash_attr} points="{encoded}"/>')
    body.append(
        f'<rect class="axis" x="{x0}" y="{y0}" width="{w}" height="{h}" fill="none"/>'
        f'<text x="{x0 + w / 2}" y="{y0 + h + 36}" text-anchor="middle" font-size="12">{x_label}</text>'
    )
    return "\n".join(body)


def render_legend(items, x: float, y: float) -> str:
    body = []
    cx = x
    for label, color, dash in items:
        dash_attr = f' stroke-dasharray="{dash}"' if dash else ""
        body.append(
            f'<line x1="{cx:.1f}" y1="{y}" x2="{cx + 28:.1f}" y2="{y}" stroke="{color}" stroke-width="3"{dash_attr}/>'
            f'<text x="{cx + 34:.1f}" y="{y + 5}" font-size="12">{label}</text>'
        )
        cx += 34 + len(label) * 6.6 + 26
    return "\n".join(body)


def render() -> str:
    rows = load_rows()

    mass_flow_series = [
        ("Simulation truth", "#111827", None),
        ("Load cell", "#9ca3af", None),
        ("Fixed filter", "#b45309", None),
        ("Kalman", "#2563eb", None),
    ]
    innovation_series = [
        ("Nominal", "#2563eb", None),
        ("Biased load cell", "#dc2626", None),
    ]

    top = render_panel(
        rows, "Mass-flow estimate", mass_flow_series,
        x0=75, y0=164, w=780, h=150, title="Mass-flow estimate (kg/s)",
    )
    bottom = render_panel(
        rows, "Innovation diagnosis", innovation_series,
        x0=75, y0=416, w=780, h=150, title="Innovation diagnosis (kg/s)",
    )
    legend_top = render_legend(mass_flow_series, x=55, y=118)
    legend_bottom = render_legend(
        [("Nominal innovation", c, d) for (_n, c, d) in [innovation_series[0]]]
        + [("Biased-sensor innovation", c, d) for (_n, c, d) in [innovation_series[1]]],
        x=55, y=378,
    )

    return f'''<svg xmlns="http://www.w3.org/2000/svg" role="img" aria-labelledby="title desc" viewBox="0 0 900 630">
<title id="title">Covariance choices change trust and diagnostic behavior</title>
<desc id="desc">The nominal Kalman estimate balances noisy load-cell measurements against the feeder model; persistent innovation reveals bias.</desc>
<defs><marker id="arrow" markerWidth="8" markerHeight="8" refX="7" refY="3" orient="auto"><path d="M0,0 L0,6 L8,3 z" fill="{INK}"/></marker></defs>
<rect width="900" height="630" fill="#f7f9fc"/>
<style>text{{font-family:system-ui,-apple-system,sans-serif;fill:{INK}}} .axis{{stroke:#526078;stroke-width:1}} .grid{{stroke:{GRID};stroke-width:1}}</style>
<text x="55" y="42" font-size="24" font-weight="700">Covariance choices change trust and diagnostic behavior</text>
<text x="55" y="70" font-size="14">The nominal Kalman estimate balances noisy load-cell measurements against the feeder model;</text>
<text x="55" y="88" font-size="14">persistent innovation reveals bias.</text>
{legend_top}
{top}
{legend_bottom}
{bottom}
</svg>
'''


def main() -> None:
    OUTPUT_SVG.write_text(render(), encoding="utf-8")


if __name__ == "__main__":
    main()
