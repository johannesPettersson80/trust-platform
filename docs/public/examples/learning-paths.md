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

Open the course's projects in order from the truST example gallery. Each link opens
the canonical learner page, where the course position, prerequisite, real
equipment, objective, safety boundary, theory, executable Structured Text,
experiment, evidence, and next lesson are stated. Every practical result is
Simulation evidence unless its learner page explicitly says otherwise.

1. [`control-lesson-01`](https://github.com/johannesPettersson80/trust-platform/tree/main/examples/control/01-feedback-foundations-and-temperature-control/01-cold-storage-loop-anatomy)
2. [`control-lesson-02`](https://github.com/johannesPettersson80/trust-platform/tree/main/examples/control/01-feedback-foundations-and-temperature-control/02-cold-storage-on-off)
3. [`control-lesson-03`](https://github.com/johannesPettersson80/trust-platform/tree/main/examples/control/01-feedback-foundations-and-temperature-control/03-cold-storage-hysteresis-dwell)
4. [`control-capstone-cold-storage`](https://github.com/johannesPettersson80/trust-platform/tree/main/examples/control/14-integrated-capstones/80-cold-storage)
5. [`control-lesson-04`](https://github.com/johannesPettersson80/trust-platform/tree/main/examples/control/01-feedback-foundations-and-temperature-control/04-water-heater-p)
6. [`control-lesson-05`](https://github.com/johannesPettersson80/trust-platform/tree/main/examples/control/01-feedback-foundations-and-temperature-control/05-water-heater-pi)
7. [`control-lesson-06`](https://github.com/johannesPettersson80/trust-platform/tree/main/examples/control/01-feedback-foundations-and-temperature-control/06-water-heater-saturation-windup)
8. [`control-lesson-07`](https://github.com/johannesPettersson80/trust-platform/tree/main/examples/control/01-feedback-foundations-and-temperature-control/07-water-heater-anti-windup)
9. [`control-lesson-09`](https://github.com/johannesPettersson80/trust-platform/tree/main/examples/control/01-feedback-foundations-and-temperature-control/09-setpoint-and-disturbance-response)
10. [`control-loop-diagnosis`](https://github.com/johannesPettersson80/trust-platform/tree/main/examples/control/09-industrial-control-structures/60-loop-diagnosis)
11. [`control-lesson-28`](https://github.com/johannesPettersson80/trust-platform/tree/main/examples/control/06-pid-tuning-methods/28-manual-pi-tuning)
12. [`control-lesson-13`](https://github.com/johannesPettersson80/trust-platform/tree/main/examples/control/01-feedback-foundations-and-temperature-control/13-process-response-classes)
13. [`control-water-heater-energy-balance`](https://github.com/johannesPettersson80/trust-platform/tree/main/examples/control/01-feedback-foundations-and-temperature-control/15-water-heater-energy-balance)
14. [`control-lesson-21`](https://github.com/johannesPettersson80/trust-platform/tree/main/examples/control/05-experiments-identification-and-validation/21-safe-informative-experiment)
15. [`control-lesson-23`](https://github.com/johannesPettersson80/trust-platform/tree/main/examples/control/05-experiments-identification-and-validation/23-step-response-identification)
16. [`control-lesson-24`](https://github.com/johannesPettersson80/trust-platform/tree/main/examples/control/05-experiments-identification-and-validation/24-choose-model-structure)
17. [`control-lesson-25`](https://github.com/johannesPettersson80/trust-platform/tree/main/examples/control/05-experiments-identification-and-validation/25-parameter-identifiability)
18. [`control-lesson-26`](https://github.com/johannesPettersson80/trust-platform/tree/main/examples/control/05-experiments-identification-and-validation/26-independent-model-validation)
19. [`control-model-based-tuning`](https://github.com/johannesPettersson80/trust-platform/tree/main/examples/control/06-pid-tuning-methods/30-model-based-pi-tuning)
20. [`control-relay-tuning`](https://github.com/johannesPettersson80/trust-platform/tree/main/examples/control/06-pid-tuning-methods/29-relay-tuning)
21. [`control-tuning-method-comparison`](https://github.com/johannesPettersson80/trust-platform/tree/main/examples/control/06-pid-tuning-methods/31-tuning-method-comparison)
22. [`control-tuning-to-requirements`](https://github.com/johannesPettersson80/trust-platform/tree/main/examples/control/06-pid-tuning-methods/85-tuning-to-requirements)
23. [`control-lesson-10`](https://github.com/johannesPettersson80/trust-platform/tree/main/examples/control/01-feedback-foundations-and-temperature-control/10-proportional-setpoint-weighting)
24. [`control-lesson-11`](https://github.com/johannesPettersson80/trust-platform/tree/main/examples/control/01-feedback-foundations-and-temperature-control/11-measurement-filtering)
25. [`control-lesson-12`](https://github.com/johannesPettersson80/trust-platform/tree/main/examples/control/01-feedback-foundations-and-temperature-control/12-elapsed-time-and-sampling)
26. [`control-booster-pump-modes`](https://github.com/johannesPettersson80/trust-platform/tree/main/examples/control/03-production-quality-digital-pid/39-controller-mode-initialization)
27. [`control-booster-pump-bumpless-transfer`](https://github.com/johannesPettersson80/trust-platform/tree/main/examples/control/03-production-quality-digital-pid/40-bumpless-transfer)
28. [`control-booster-pump-atomic-update`](https://github.com/johannesPettersson80/trust-platform/tree/main/examples/control/03-production-quality-digital-pid/41-atomic-parameter-update)
29. [`control-booster-pump-invalid-data`](https://github.com/johannesPettersson80/trust-platform/tree/main/examples/control/03-production-quality-digital-pid/42-invalid-stale-data)
30. [`control-capstone-water-heater`](https://github.com/johannesPettersson80/trust-platform/tree/main/examples/control/14-integrated-capstones/81-water-heater)
31. [`control-lesson-08`](https://github.com/johannesPettersson80/trust-platform/tree/main/examples/control/01-feedback-foundations-and-temperature-control/08-heating-valve-rate-tracking)
32. [`control-valve-characterization`](https://github.com/johannesPettersson80/trust-platform/tree/main/examples/control/09-industrial-control-structures/58-valve-characterization)
33. [`control-valve-stiction`](https://github.com/johannesPettersson80/trust-platform/tree/main/examples/control/09-industrial-control-structures/59-valve-stiction)
34. [`control-proportional-position-stiffness`](https://github.com/johannesPettersson80/trust-platform/tree/main/examples/control/02-derivative-action-and-dynamic-response/32-proportional-position-stiffness)
35. [`control-derivative-motion-damping`](https://github.com/johannesPettersson80/trust-platform/tree/main/examples/control/02-derivative-action-and-dynamic-response/33-derivative-motion-damping)
36. [`control-loaded-position-pid`](https://github.com/johannesPettersson80/trust-platform/tree/main/examples/control/02-derivative-action-and-dynamic-response/34-servo-pid-holding-torque)
37. [`control-servo-filtered-derivative`](https://github.com/johannesPettersson80/trust-platform/tree/main/examples/control/02-derivative-action-and-dynamic-response/36-filtered-derivative)
38. [`control-discrete-integral-derivative`](https://github.com/johannesPettersson80/trust-platform/tree/main/examples/control/03-production-quality-digital-pid/37-discrete-integral-derivative)
39. [`control-pid-forms`](https://github.com/johannesPettersson80/trust-platform/tree/main/examples/control/03-production-quality-digital-pid/38-positional-incremental-pid)
40. [`control-own-pid-law`](https://github.com/johannesPettersson80/trust-platform/tree/main/examples/control/03-production-quality-digital-pid/86-own-pid-law)
41. [`control-capstone-servo`](https://github.com/johannesPettersson80/trust-platform/tree/main/examples/control/14-integrated-capstones/82-servo)
42. [`control-controller-families`](https://github.com/johannesPettersson80/trust-platform/tree/main/examples/control/04-controller-selection-and-plant-models/27-controller-selection)
43. [`control-lesson-14`](https://github.com/johannesPettersson80/trust-platform/tree/main/examples/control/01-feedback-foundations-and-temperature-control/14-conservation-and-accumulation)
44. [`control-level-control`](https://github.com/johannesPettersson80/trust-platform/tree/main/examples/control/01-feedback-foundations-and-temperature-control/87-level-control)
45. [`control-lesson-16`](https://github.com/johannesPettersson80/trust-platform/tree/main/examples/control/01-feedback-foundations-and-temperature-control/16-gravity-tank-nonlinearity)
46. [`control-lesson-17`](https://github.com/johannesPettersson80/trust-platform/tree/main/examples/control/01-feedback-foundations-and-temperature-control/17-gravity-tank-linearization)
47. [`control-gain-scheduling`](https://github.com/johannesPettersson80/trust-platform/tree/main/examples/control/09-industrial-control-structures/56-gain-scheduling)
48. [`control-lesson-18`](https://github.com/johannesPettersson80/trust-platform/tree/main/examples/control/01-feedback-foundations-and-temperature-control/18-transfer-function-from-physics)
49. [`control-lesson-19`](https://github.com/johannesPettersson80/trust-platform/tree/main/examples/control/02-derivative-action-and-dynamic-response/19-poles-zeros-and-response)
50. [`control-characteristic-equation`](https://github.com/johannesPettersson80/trust-platform/tree/main/examples/control/07-classical-stability-and-frequency-design/43-characteristic-equation)
51. [`control-root-locus`](https://github.com/johannesPettersson80/trust-platform/tree/main/examples/control/07-classical-stability-and-frequency-design/45-root-locus)
52. [`control-routh-hurwitz`](https://github.com/johannesPettersson80/trust-platform/tree/main/examples/control/07-classical-stability-and-frequency-design/44-routh-hurwitz)
53. [`control-lesson-22`](https://github.com/johannesPettersson80/trust-platform/tree/main/examples/control/05-experiments-identification-and-validation/22-excitation-for-dynamics)
54. [`control-bode-frequency-response`](https://github.com/johannesPettersson80/trust-platform/tree/main/examples/control/07-classical-stability-and-frequency-design/46-bode-frequency-response)
55. [`control-sensitivity-functions`](https://github.com/johannesPettersson80/trust-platform/tree/main/examples/control/07-classical-stability-and-frequency-design/48-sensitivity-functions)
56. [`control-loop-shaping`](https://github.com/johannesPettersson80/trust-platform/tree/main/examples/control/07-classical-stability-and-frequency-design/49-loop-shaping)
57. [`control-nyquist-stability`](https://github.com/johannesPettersson80/trust-platform/tree/main/examples/control/07-classical-stability-and-frequency-design/47-nyquist-stability)
58. [`control-robustness-margins`](https://github.com/johannesPettersson80/trust-platform/tree/main/examples/control/07-classical-stability-and-frequency-design/88-robustness-margins)
59. [`control-mechanical-resonance`](https://github.com/johannesPettersson80/trust-platform/tree/main/examples/control/07-classical-stability-and-frequency-design/89-mechanical-resonance)
60. [`control-position-z-plane-sampling`](https://github.com/johannesPettersson80/trust-platform/tree/main/examples/control/07-classical-stability-and-frequency-design/50-position-z-plane)
61. [`control-trajectory-feedforward`](https://github.com/johannesPettersson80/trust-platform/tree/main/examples/control/09-industrial-control-structures/90-trajectory-feedforward)
62. [`control-feedforward`](https://github.com/johannesPettersson80/trust-platform/tree/main/examples/control/09-industrial-control-structures/51-feedforward)
63. [`control-cascade`](https://github.com/johannesPettersson80/trust-platform/tree/main/examples/control/09-industrial-control-structures/52-cascade)
64. [`control-ratio-control`](https://github.com/johannesPettersson80/trust-platform/tree/main/examples/control/09-industrial-control-structures/53-ratio-control)
65. [`control-split-range`](https://github.com/johannesPettersson80/trust-platform/tree/main/examples/control/09-industrial-control-structures/54-split-range)
66. [`control-override-control`](https://github.com/johannesPettersson80/trust-platform/tree/main/examples/control/09-industrial-control-structures/55-override-control)
67. [`control-smith-predictor`](https://github.com/johannesPettersson80/trust-platform/tree/main/examples/control/09-industrial-control-structures/57-smith-predictor)
68. [`control-capstone-reactor`](https://github.com/johannesPettersson80/trust-platform/tree/main/examples/control/14-integrated-capstones/83-reactor-control)
69. [`control-mimo-paths`](https://github.com/johannesPettersson80/trust-platform/tree/main/examples/control/10-multivariable-control/61-mimo-paths)
70. [`control-relative-gain-array`](https://github.com/johannesPettersson80/trust-platform/tree/main/examples/control/10-multivariable-control/62-relative-gain-array)
71. [`control-decentralized-pi`](https://github.com/johannesPettersson80/trust-platform/tree/main/examples/control/10-multivariable-control/63-decentralized-pi)
72. [`control-decoupling`](https://github.com/johannesPettersson80/trust-platform/tree/main/examples/control/10-multivariable-control/64-decoupling)
73. [`control-robust-mimo-selection`](https://github.com/johannesPettersson80/trust-platform/tree/main/examples/control/10-multivariable-control/65-robust-mimo-selection)
74. [`control-lesson-20`](https://github.com/johannesPettersson80/trust-platform/tree/main/examples/control/02-derivative-action-and-dynamic-response/20-state-equations-from-stores)
75. [`control-controllability`](https://github.com/johannesPettersson80/trust-platform/tree/main/examples/control/11-state-space-estimation-and-predictive-control/66-controllability)
76. [`control-observability`](https://github.com/johannesPettersson80/trust-platform/tree/main/examples/control/11-state-space-estimation-and-predictive-control/67-observability)
77. [`control-state-feedback`](https://github.com/johannesPettersson80/trust-platform/tree/main/examples/control/11-state-space-estimation-and-predictive-control/68-state-feedback)
78. [`control-state-observer`](https://github.com/johannesPettersson80/trust-platform/tree/main/examples/control/11-state-space-estimation-and-predictive-control/70-state-observer)
79. [`control-kalman-filter`](https://github.com/johannesPettersson80/trust-platform/tree/main/examples/control/11-state-space-estimation-and-predictive-control/71-kalman-filter)
80. [`control-lqr`](https://github.com/johannesPettersson80/trust-platform/tree/main/examples/control/11-state-space-estimation-and-predictive-control/69-lqr)
81. [`control-lqg`](https://github.com/johannesPettersson80/trust-platform/tree/main/examples/control/11-state-space-estimation-and-predictive-control/72-lqg)
82. [`control-mpc`](https://github.com/johannesPettersson80/trust-platform/tree/main/examples/control/11-state-space-estimation-and-predictive-control/73-mpc)
83. [`control-lyapunov-stability`](https://github.com/johannesPettersson80/trust-platform/tree/main/examples/control/12-nonlinear-adaptive-and-robust-control/74-lyapunov-stability)
84. [`control-feedback-linearization`](https://github.com/johannesPettersson80/trust-platform/tree/main/examples/control/12-nonlinear-adaptive-and-robust-control/75-feedback-linearization)
85. [`control-adaptive-control`](https://github.com/johannesPettersson80/trust-platform/tree/main/examples/control/12-nonlinear-adaptive-and-robust-control/76-adaptive-control)
86. [`control-robust-control`](https://github.com/johannesPettersson80/trust-platform/tree/main/examples/control/12-nonlinear-adaptive-and-robust-control/77-robust-control)
87. [`control-networked-control`](https://github.com/johannesPettersson80/trust-platform/tree/main/examples/control/13-networked-fault-aware-and-diagnostic-control/78-networked-control)
88. [`control-fault-aware-control`](https://github.com/johannesPettersson80/trust-platform/tree/main/examples/control/13-networked-fault-aware-and-diagnostic-control/79-fault-aware-control)
89. [`control-capstone-independent`](https://github.com/johannesPettersson80/trust-platform/tree/main/examples/control/14-integrated-capstones/84-independent)

Completing the course teaches the declared simulation and introductory design
scope. It does not replace plant-specific commissioning, machinery protection,
safety lifecycle work, regulatory qualification, or approval for a physical
experiment.
