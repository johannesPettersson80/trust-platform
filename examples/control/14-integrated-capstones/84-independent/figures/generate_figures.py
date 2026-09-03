#!/usr/bin/env python3
"""Render Lesson 84's topology diagram.

Pure Python, no third-party dependencies.

figures/independent-capstone-topology.svg is a structural box-and-arrow
diagram of the feedback-plus-feedforward signal path onto the shared
receiver. It is not derived from CSV data (it has no panels, series, or
axes to reproduce); it is written directly in the visual style of
examples/control/01-feedback-foundations-and-temperature-control/
08-water-heater-saturation/figures/heater-signal-path.svg.

figures/independent-capstone-evidence.svg is owned by the native test
control_curriculum_independent_capstone
(crates/trust-runtime/tests/control_curriculum_independent_capstone.rs).
Refresh it with: TRUST_UPDATE_CONTROL_CURRICULUM_ARTIFACTS=1 cargo test -p trust-runtime --test control_curriculum_independent_capstone
"""

from __future__ import annotations

from pathlib import Path

ROOT = Path(__file__).resolve().parent

# --- Topology diagram (structural, not CSV-derived) ------------------------

def render_topology() -> str:
    return '''<svg xmlns="http://www.w3.org/2000/svg" width="1200" height="420" viewBox="0 0 1200 420" role="img" aria-labelledby="title desc">
  <title id="title">Feedback and measured-demand feedforward share one actuator path</title>
  <desc id="desc">Pressure error drives a PI controller; measured plant demand separately drives a one-for-one feedforward gain. Both requested contributions sum before one amplitude and slew limit, and only the applied delivery request reaches the compressor and receiver; measured header pressure returns to the PI controller, and plant demand also acts directly on the receiver as physical flow.</desc>
  <defs>
    <marker id="arrow-blue" viewBox="0 0 10 10" refX="9" refY="5" markerWidth="7" markerHeight="7" orient="auto-start-reverse">
      <path d="M 0 0 L 10 5 L 0 10 z" fill="#2563eb"/>
    </marker>
    <marker id="arrow-orange" viewBox="0 0 10 10" refX="9" refY="5" markerWidth="7" markerHeight="7" orient="auto-start-reverse">
      <path d="M 0 0 L 10 5 L 0 10 z" fill="#ea580c"/>
    </marker>
    <marker id="arrow-gray" viewBox="0 0 10 10" refX="9" refY="5" markerWidth="7" markerHeight="7" orient="auto-start-reverse">
      <path d="M 0 0 L 10 5 L 0 10 z" fill="#475569"/>
    </marker>
    <marker id="arrow-purple" viewBox="0 0 10 10" refX="9" refY="5" markerWidth="7" markerHeight="7" orient="auto-start-reverse">
      <path d="M 0 0 L 10 5 L 0 10 z" fill="#a21caf"/>
    </marker>
    <marker id="arrow-green" viewBox="0 0 10 10" refX="9" refY="5" markerWidth="7" markerHeight="7" orient="auto-start-reverse">
      <path d="M 0 0 L 10 5 L 0 10 z" fill="#16a34a"/>
    </marker>
  </defs>

  <rect width="1200" height="420" rx="18" fill="#f8fafc"/>
  <text x="48" y="40" fill="#0f172a" font-family="system-ui, sans-serif" font-size="21" font-weight="700">Feedback and measured-demand feedforward share one actuator path</text>
  <text x="48" y="63" fill="#475569" font-family="system-ui, sans-serif" font-size="14">Both requested contributions pass through the same amplitude and slew limit before the compressor.</text>

  <g font-family="system-ui, sans-serif" text-anchor="middle">
    <rect x="40" y="100" width="220" height="72" rx="10" fill="#eff6ff" stroke="#2563eb" stroke-width="2"/>
    <text x="150" y="130" fill="#0f172a" font-size="16" font-weight="700">PI controller</text>
    <text x="150" y="151" fill="#1d4ed8" font-size="13">pressure error e = SP - PV</text>

    <rect x="40" y="220" width="220" height="72" rx="10" fill="#fdf4ff" stroke="#a21caf" stroke-width="2"/>
    <text x="150" y="250" fill="#0f172a" font-size="16" font-weight="700">Feedforward gain</text>
    <text x="150" y="271" fill="#86198f" font-size="13">one-for-one on measured demand</text>

    <rect x="340" y="160" width="230" height="72" rx="10" fill="#fff7ed" stroke="#ea580c" stroke-width="2"/>
    <text x="455" y="190" fill="#0f172a" font-size="16" font-weight="700">Amplitude / slew limit</text>
    <text x="455" y="211" fill="#9a3412" font-size="13">MV requested to MV applied</text>

    <rect x="650" y="160" width="220" height="72" rx="10" fill="#f8fafc" stroke="#64748b" stroke-width="2"/>
    <text x="760" y="190" fill="#0f172a" font-size="16" font-weight="700">Compressor</text>
    <text x="760" y="211" fill="#475569" font-size="13">5-second delivery lag</text>

    <rect x="950" y="160" width="220" height="72" rx="10" fill="#f0fdf4" stroke="#16a34a" stroke-width="2"/>
    <text x="1060" y="190" fill="#0f172a" font-size="16" font-weight="700">Receiver</text>
    <text x="1060" y="211" fill="#166534" font-size="13">header pressure PV</text>

    <rect x="340" y="300" width="230" height="72" rx="10" fill="#f8fafc" stroke="#64748b" stroke-width="2"/>
    <text x="455" y="330" fill="#0f172a" font-size="16" font-weight="700">Plant air demand</text>
    <text x="455" y="351" fill="#475569" font-size="13">measured disturbance q_d</text>
  </g>

  <path d="M260 136 L335 190" fill="none" stroke="#2563eb" stroke-width="3" marker-end="url(#arrow-blue)"/>
  <path d="M260 256 L335 205" fill="none" stroke="#a21caf" stroke-width="3" marker-end="url(#arrow-purple)"/>
  <text x="300" y="245" fill="#334155" font-family="system-ui, sans-serif" font-size="12" font-weight="600" text-anchor="middle">MV_feedback, MV_feedforward</text>

  <path d="M570 196 H645" fill="none" stroke="#ea580c" stroke-width="3" marker-end="url(#arrow-orange)"/>
  <path d="M870 196 H945" fill="none" stroke="#475569" stroke-width="3" marker-end="url(#arrow-gray)"/>
  <text x="607" y="181" fill="#9a3412" font-family="system-ui, sans-serif" font-size="12" font-weight="600" text-anchor="middle">MV applied</text>

  <path d="M455 300 V232" fill="none" stroke="#a21caf" stroke-width="2.5" stroke-dasharray="6 4" marker-end="url(#arrow-purple)"/>
  <text x="455" y="292" fill="#86198f" font-family="system-ui, sans-serif" font-size="12" font-weight="600" text-anchor="middle">measured signal</text>

  <path d="M570 336 H1060 V236" fill="none" stroke="#475569" stroke-width="2.5" stroke-dasharray="3 3" marker-end="url(#arrow-gray)"/>
  <text x="815" y="351" fill="#334155" font-family="system-ui, sans-serif" font-size="12" font-weight="600" text-anchor="middle">physical flow imbalance, not a controller signal</text>

  <path d="M1060 232 V380 H150 V172" fill="none" stroke="#16a34a" stroke-width="3" marker-end="url(#arrow-green)"/>
  <text x="600" y="398" fill="#166534" font-family="system-ui, sans-serif" font-size="13" font-weight="600" text-anchor="middle">measured header pressure (PV) returns to the PI controller</text>
</svg>
'''


def main() -> None:
    (ROOT / "independent-capstone-topology.svg").write_text(render_topology(), encoding="utf-8")


if __name__ == "__main__":
    main()
