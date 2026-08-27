# Learning Paths

Example routes for programming by doing.

## Recommended paths

| Goal | Open | Then continue with |
| --- | --- | --- |
| first success with truST | [Tutorials](tutorials.md) | [Build, Validate, Test](../operate/build-validate-test.md) |
| connect to external systems or hardware | [Connectivity](connectivity.md) | [Program / Communication](../connect/protocol-matrix.md) |
| migrate from vendor-specific tooling | [Vendor Profiles](vendor-profiles.md) | [Program / Migration](../migrate/index.md) |
| learn visual editors | [Visual Editors](visual-editors.md) | [Program / Visual Programming](../develop/visual-editors/index.md) |
| build HMI/operator workflows | [HMI](hmi.md) | [Run / Operator HMI](../operate/hmi-and-web-ui.md) |
| learn PID and control engineering | [Control System Engineering](../develop/control-system-engineering.md) | [Control Studio](../operate/control-studio.md) |
| motion and performance work | [Libraries And Motion](libraries-and-motion.md) | [Benchmarks](../reference/benchmarks.md) |
| federation/fleet workflows | [Runtime Cloud](runtime-cloud.md) | [Run / Runtime Cloud](../operate/runtime-cloud.md) |
| larger end-to-end systems | [Capstones](capstones.md) | [What Is truST?](../concepts/index.md) |

## Default recommendation

If you are unsure, do this order:

1. [Tutorials](tutorials.md)
2. [Connectivity](connectivity.md)
3. [Vendor Profiles](vendor-profiles.md) or [HMI](hmi.md), depending on your
   actual project

## Control engineering path

Open these projects from the existing truST example gallery. They use the real
`Control.PID`, ordinary simulated I/O, Control Studio, and the benchmark limits
from the control performance specification. All results are simulation
evidence.

Start with the foundation lessons:

1. `control-nominal-fopdt`
2. `control-sampling-interval`
3. `control-load-disturbance`
4. `control-saturation-antiwindup`
5. `control-measurement-noise`
6. `control-rate-limit`

Continue with the practitioner lessons:

1. `control-integrating-plant`
2. `control-underdamped-pid`
3. `control-delay-dominant`
4. `control-nonlinear-actuator`
5. `control-bounded-variation`

Use the advanced lessons after those foundations:

1. `control-open-loop-unstable`
2. `control-scheduled-gain`
3. `control-severe-jitter`
4. `control-coupled-two-loop`
5. `control-evidence-integrity`

Each project README follows the same Predict, Configure, Monitor, Test,
Identify, Tune, Validate, Explain, and Export journey and gives the exact native
expected-result command.
