# Learning Paths

Example routes for programming by doing.

## Recommended paths

| Goal | Open | Then continue with |
| --- | --- | --- |
| first success with truST | [Tutorials](tutorials.md) | [Build, Validate, Test](../operate/build-validate-test.md) |
| connect to external systems or hardware | [Connectivity](connectivity.md) | [Program / Communication](../connect/protocol-matrix.md) |
| migrate from vendor-specific tooling | [Vendor Profiles](vendor-profiles.md) | [Program / Migration](../migrate/index.md) |
| learn visual editors | [Visual Editors](visual-editors.md) | [Program / Visual Programming](../develop/visual-editors/index.md) |
| learn PID and control engineering | [Control System Engineering](../develop/control-system-engineering.md) | [Control Studio](../operate/control-studio.md) |
| motion and performance work | [Libraries And Motion](libraries-and-motion.md) | [Benchmarks](../reference/benchmarks.md) |
| federation/fleet workflows | [Runtime Cloud](runtime-cloud.md) | [Run / Runtime Cloud](../operate/runtime-cloud.md) |
| larger end-to-end systems | [Capstones](capstones.md) | [What Is truST?](../concepts/index.md) |

## Default recommendation

If you are unsure, do this order:

1. [Tutorials](tutorials.md)
2. [Connectivity](connectivity.md)
3. [Vendor Profiles](vendor-profiles.md), when your project comes from
   vendor-specific tooling

## Control engineering path

Open these 84 projects in order from the truST example gallery. Each link opens
the canonical learner page, where the course position, prerequisite, real
equipment, objective, safety boundary, theory, executable Structured Text,
experiment, evidence, and next lesson are stated. Every practical result is
Simulation evidence unless its learner page explicitly says otherwise.

1. [`control-lesson-01`](https://github.com/johannesPettersson80/trust-platform/tree/main/examples/control/01-feedback-foundations-and-temperature-control/01-cold-storage-loop-anatomy)
2. [`control-lesson-02`](https://github.com/johannesPettersson80/trust-platform/tree/main/examples/control/01-feedback-foundations-and-temperature-control/02-cold-storage-on-off)
3. [`control-lesson-03`](https://github.com/johannesPettersson80/trust-platform/tree/main/examples/control/01-feedback-foundations-and-temperature-control/03-cold-storage-hysteresis-dwell)
4. [`control-lesson-04`](https://github.com/johannesPettersson80/trust-platform/tree/main/examples/control/01-feedback-foundations-and-temperature-control/04-water-heater-p)
5. [`control-lesson-05`](https://github.com/johannesPettersson80/trust-platform/tree/main/examples/control/01-feedback-foundations-and-temperature-control/05-water-heater-pi)
6. [`control-lesson-06`](https://github.com/johannesPettersson80/trust-platform/tree/main/examples/control/01-feedback-foundations-and-temperature-control/06-water-heater-saturation-windup)
7. [`control-lesson-07`](https://github.com/johannesPettersson80/trust-platform/tree/main/examples/control/01-feedback-foundations-and-temperature-control/07-water-heater-anti-windup)
8. [`control-lesson-08`](https://github.com/johannesPettersson80/trust-platform/tree/main/examples/control/01-feedback-foundations-and-temperature-control/08-heating-valve-rate-tracking)
9. [`control-lesson-09`](https://github.com/johannesPettersson80/trust-platform/tree/main/examples/control/01-feedback-foundations-and-temperature-control/09-setpoint-and-disturbance-response)
10. [`control-lesson-10`](https://github.com/johannesPettersson80/trust-platform/tree/main/examples/control/01-feedback-foundations-and-temperature-control/10-proportional-setpoint-weighting)
11. [`control-lesson-11`](https://github.com/johannesPettersson80/trust-platform/tree/main/examples/control/01-feedback-foundations-and-temperature-control/11-measurement-filtering)
12. [`control-lesson-12`](https://github.com/johannesPettersson80/trust-platform/tree/main/examples/control/01-feedback-foundations-and-temperature-control/12-elapsed-time-and-sampling)
13. [`control-lesson-13`](https://github.com/johannesPettersson80/trust-platform/tree/main/examples/control/01-feedback-foundations-and-temperature-control/13-process-response-classes)
14. [`control-lesson-14`](https://github.com/johannesPettersson80/trust-platform/tree/main/examples/control/01-feedback-foundations-and-temperature-control/14-conservation-and-accumulation)
15. [`control-water-heater-energy-balance`](https://github.com/johannesPettersson80/trust-platform/tree/main/examples/control/01-feedback-foundations-and-temperature-control/15-water-heater-energy-balance)
16. [`control-lesson-16`](https://github.com/johannesPettersson80/trust-platform/tree/main/examples/control/01-feedback-foundations-and-temperature-control/16-gravity-tank-nonlinearity)
17. [`control-lesson-17`](https://github.com/johannesPettersson80/trust-platform/tree/main/examples/control/01-feedback-foundations-and-temperature-control/17-gravity-tank-linearization)
18. [`control-lesson-18`](https://github.com/johannesPettersson80/trust-platform/tree/main/examples/control/01-feedback-foundations-and-temperature-control/18-transfer-function-from-physics)
19. [`control-lesson-19`](https://github.com/johannesPettersson80/trust-platform/tree/main/examples/control/02-derivative-action-and-dynamic-response/19-poles-zeros-and-response)
20. [`control-lesson-20`](https://github.com/johannesPettersson80/trust-platform/tree/main/examples/control/02-derivative-action-and-dynamic-response/20-state-equations-from-stores)
21. [`control-lesson-21`](https://github.com/johannesPettersson80/trust-platform/tree/main/examples/control/05-experiments-identification-and-validation/21-safe-informative-experiment)
22. [`control-lesson-22`](https://github.com/johannesPettersson80/trust-platform/tree/main/examples/control/05-experiments-identification-and-validation/22-excitation-for-dynamics)
23. [`control-lesson-23`](https://github.com/johannesPettersson80/trust-platform/tree/main/examples/control/05-experiments-identification-and-validation/23-step-response-identification)
24. [`control-lesson-24`](https://github.com/johannesPettersson80/trust-platform/tree/main/examples/control/05-experiments-identification-and-validation/24-choose-model-structure)
25. [`control-lesson-25`](https://github.com/johannesPettersson80/trust-platform/tree/main/examples/control/05-experiments-identification-and-validation/25-parameter-identifiability)
26. [`control-lesson-26`](https://github.com/johannesPettersson80/trust-platform/tree/main/examples/control/05-experiments-identification-and-validation/26-independent-model-validation)
27. [`control-lesson-27`](https://github.com/johannesPettersson80/trust-platform/tree/main/examples/control/04-controller-selection-and-plant-models/27-controller-selection)
28. [`control-lesson-28`](https://github.com/johannesPettersson80/trust-platform/tree/main/examples/control/06-pid-tuning-methods/28-manual-pi-tuning)
29. [`control-relay-tuning`](https://github.com/johannesPettersson80/trust-platform/tree/main/examples/control/06-pid-tuning-methods/29-relay-tuning)
30. [`control-model-based-tuning`](https://github.com/johannesPettersson80/trust-platform/tree/main/examples/control/06-pid-tuning-methods/30-model-based-pi-tuning)
31. [`control-tuning-method-comparison`](https://github.com/johannesPettersson80/trust-platform/tree/main/examples/control/06-pid-tuning-methods/31-tuning-method-comparison)
32. [`control-proportional-position-stiffness`](https://github.com/johannesPettersson80/trust-platform/tree/main/examples/control/02-derivative-action-and-dynamic-response/32-proportional-position-stiffness)
33. [`control-derivative-motion-damping`](https://github.com/johannesPettersson80/trust-platform/tree/main/examples/control/02-derivative-action-and-dynamic-response/33-derivative-motion-damping)
34. [`control-loaded-position-pid`](https://github.com/johannesPettersson80/trust-platform/tree/main/examples/control/02-derivative-action-and-dynamic-response/34-servo-pid-holding-torque)
36. [`control-servo-filtered-derivative`](https://github.com/johannesPettersson80/trust-platform/tree/main/examples/control/02-derivative-action-and-dynamic-response/36-filtered-derivative)
37. [`control-discrete-integral-derivative`](https://github.com/johannesPettersson80/trust-platform/tree/main/examples/control/03-production-quality-digital-pid/37-discrete-integral-derivative)
38. [`control-pid-forms`](https://github.com/johannesPettersson80/trust-platform/tree/main/examples/control/03-production-quality-digital-pid/38-positional-incremental-pid)
39. [`control-booster-pump-modes`](https://github.com/johannesPettersson80/trust-platform/tree/main/examples/control/03-production-quality-digital-pid/39-controller-mode-initialization)
40. [`control-booster-pump-bumpless-transfer`](https://github.com/johannesPettersson80/trust-platform/tree/main/examples/control/03-production-quality-digital-pid/40-bumpless-transfer)
41. [`control-booster-pump-atomic-update`](https://github.com/johannesPettersson80/trust-platform/tree/main/examples/control/03-production-quality-digital-pid/41-atomic-parameter-update)
42. [`control-booster-pump-invalid-data`](https://github.com/johannesPettersson80/trust-platform/tree/main/examples/control/03-production-quality-digital-pid/42-invalid-stale-data)
43. [`control-characteristic-equation`](https://github.com/johannesPettersson80/trust-platform/tree/main/examples/control/07-classical-stability-and-frequency-design/43-characteristic-equation)
44. [`control-routh-hurwitz`](https://github.com/johannesPettersson80/trust-platform/tree/main/examples/control/07-classical-stability-and-frequency-design/44-routh-hurwitz)
45. [`control-root-locus`](https://github.com/johannesPettersson80/trust-platform/tree/main/examples/control/07-classical-stability-and-frequency-design/45-root-locus)
46. [`control-bode-frequency-response`](https://github.com/johannesPettersson80/trust-platform/tree/main/examples/control/07-classical-stability-and-frequency-design/46-bode-frequency-response)
47. [`control-nyquist-stability`](https://github.com/johannesPettersson80/trust-platform/tree/main/examples/control/07-classical-stability-and-frequency-design/47-nyquist-stability)
48. [`control-sensitivity-functions`](https://github.com/johannesPettersson80/trust-platform/tree/main/examples/control/07-classical-stability-and-frequency-design/48-sensitivity-functions)
49. [`control-loop-shaping`](https://github.com/johannesPettersson80/trust-platform/tree/main/examples/control/07-classical-stability-and-frequency-design/49-loop-shaping)
50. [`control-position-z-plane-sampling`](https://github.com/johannesPettersson80/trust-platform/tree/main/examples/control/07-classical-stability-and-frequency-design/50-position-z-plane)
51. [`control-feedforward`](https://github.com/johannesPettersson80/trust-platform/tree/main/examples/control/09-industrial-control-structures/51-feedforward)
52. [`control-cascade`](https://github.com/johannesPettersson80/trust-platform/tree/main/examples/control/09-industrial-control-structures/52-cascade)
53. [`control-ratio-control`](https://github.com/johannesPettersson80/trust-platform/tree/main/examples/control/09-industrial-control-structures/53-ratio-control)
54. [`control-split-range`](https://github.com/johannesPettersson80/trust-platform/tree/main/examples/control/09-industrial-control-structures/54-split-range)
55. [`control-override-control`](https://github.com/johannesPettersson80/trust-platform/tree/main/examples/control/09-industrial-control-structures/55-override-control)
56. [`control-gain-scheduling`](https://github.com/johannesPettersson80/trust-platform/tree/main/examples/control/09-industrial-control-structures/56-gain-scheduling)
57. [`control-smith-predictor`](https://github.com/johannesPettersson80/trust-platform/tree/main/examples/control/09-industrial-control-structures/57-smith-predictor)
58. [`control-valve-characterization`](https://github.com/johannesPettersson80/trust-platform/tree/main/examples/control/09-industrial-control-structures/58-valve-characterization)
59. [`control-valve-stiction`](https://github.com/johannesPettersson80/trust-platform/tree/main/examples/control/09-industrial-control-structures/59-valve-stiction)
60. [`control-loop-diagnosis`](https://github.com/johannesPettersson80/trust-platform/tree/main/examples/control/09-industrial-control-structures/60-loop-diagnosis)
61. [`control-mimo-paths`](https://github.com/johannesPettersson80/trust-platform/tree/main/examples/control/10-multivariable-control/61-mimo-paths)
62. [`control-relative-gain-array`](https://github.com/johannesPettersson80/trust-platform/tree/main/examples/control/10-multivariable-control/62-relative-gain-array)
63. [`control-decentralized-pi`](https://github.com/johannesPettersson80/trust-platform/tree/main/examples/control/10-multivariable-control/63-decentralized-pi)
64. [`control-decoupling`](https://github.com/johannesPettersson80/trust-platform/tree/main/examples/control/10-multivariable-control/64-decoupling)
65. [`control-robust-mimo-selection`](https://github.com/johannesPettersson80/trust-platform/tree/main/examples/control/10-multivariable-control/65-robust-mimo-selection)
66. [`control-controllability`](https://github.com/johannesPettersson80/trust-platform/tree/main/examples/control/11-state-space-estimation-and-predictive-control/66-controllability)
67. [`control-observability`](https://github.com/johannesPettersson80/trust-platform/tree/main/examples/control/11-state-space-estimation-and-predictive-control/67-observability)
68. [`control-state-feedback`](https://github.com/johannesPettersson80/trust-platform/tree/main/examples/control/11-state-space-estimation-and-predictive-control/68-state-feedback)
69. [`control-lqr`](https://github.com/johannesPettersson80/trust-platform/tree/main/examples/control/11-state-space-estimation-and-predictive-control/69-lqr)
70. [`control-state-observer`](https://github.com/johannesPettersson80/trust-platform/tree/main/examples/control/11-state-space-estimation-and-predictive-control/70-state-observer)
71. [`control-kalman-filter`](https://github.com/johannesPettersson80/trust-platform/tree/main/examples/control/11-state-space-estimation-and-predictive-control/71-kalman-filter)
72. [`control-lqg`](https://github.com/johannesPettersson80/trust-platform/tree/main/examples/control/11-state-space-estimation-and-predictive-control/72-lqg)
73. [`control-mpc`](https://github.com/johannesPettersson80/trust-platform/tree/main/examples/control/11-state-space-estimation-and-predictive-control/73-mpc)
74. [`control-lyapunov-stability`](https://github.com/johannesPettersson80/trust-platform/tree/main/examples/control/12-nonlinear-adaptive-and-robust-control/74-lyapunov-stability)
75. [`control-feedback-linearization`](https://github.com/johannesPettersson80/trust-platform/tree/main/examples/control/12-nonlinear-adaptive-and-robust-control/75-feedback-linearization)
76. [`control-adaptive-control`](https://github.com/johannesPettersson80/trust-platform/tree/main/examples/control/12-nonlinear-adaptive-and-robust-control/76-adaptive-control)
77. [`control-robust-control`](https://github.com/johannesPettersson80/trust-platform/tree/main/examples/control/12-nonlinear-adaptive-and-robust-control/77-robust-control)
78. [`control-networked-control`](https://github.com/johannesPettersson80/trust-platform/tree/main/examples/control/13-networked-fault-aware-and-diagnostic-control/78-networked-control)
79. [`control-fault-aware-control`](https://github.com/johannesPettersson80/trust-platform/tree/main/examples/control/13-networked-fault-aware-and-diagnostic-control/79-fault-aware-control)
80. [`control-capstone-cold-storage`](https://github.com/johannesPettersson80/trust-platform/tree/main/examples/control/14-integrated-capstones/80-cold-storage)
81. [`control-capstone-water-heater`](https://github.com/johannesPettersson80/trust-platform/tree/main/examples/control/14-integrated-capstones/81-water-heater)
82. [`control-capstone-servo`](https://github.com/johannesPettersson80/trust-platform/tree/main/examples/control/14-integrated-capstones/82-servo)
83. [`control-capstone-reactor`](https://github.com/johannesPettersson80/trust-platform/tree/main/examples/control/14-integrated-capstones/83-reactor-control)
84. [`control-capstone-independent`](https://github.com/johannesPettersson80/trust-platform/tree/main/examples/control/14-integrated-capstones/84-independent)

Completing the course teaches the declared simulation and introductory design
scope. It does not replace plant-specific commissioning, machinery protection,
safety lifecycle work, regulatory qualification, or approval for a physical
experiment.
