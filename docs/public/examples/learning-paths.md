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

Open these 84 projects in order from the truST example gallery. Each link opens
the canonical learner page, where the course position, prerequisite, real
equipment, objective, safety boundary, theory, executable Structured Text,
experiment, evidence, and next lesson are stated. Every practical result is
Simulation evidence unless its learner page explicitly says otherwise.

1. [`control-lesson-01`](../../../examples/control/01-feedback-foundations-and-temperature-control/01-cold-storage-loop-anatomy/README.md)
2. [`control-lesson-02`](../../../examples/control/01-feedback-foundations-and-temperature-control/02-cold-storage-on-off/README.md)
3. [`control-lesson-03`](../../../examples/control/01-feedback-foundations-and-temperature-control/03-cold-storage-hysteresis-dwell/README.md)
4. [`control-lesson-04`](../../../examples/control/01-feedback-foundations-and-temperature-control/04-water-heater-p/README.md)
5. [`control-lesson-05`](../../../examples/control/01-feedback-foundations-and-temperature-control/05-water-heater-pi/README.md)
6. [`control-lesson-06`](../../../examples/control/01-feedback-foundations-and-temperature-control/06-water-heater-saturation-windup/README.md)
7. [`control-lesson-07`](../../../examples/control/01-feedback-foundations-and-temperature-control/07-water-heater-anti-windup/README.md)
8. [`control-lesson-08`](../../../examples/control/01-feedback-foundations-and-temperature-control/08-heating-valve-rate-tracking/README.md)
9. [`control-lesson-09`](../../../examples/control/01-feedback-foundations-and-temperature-control/09-setpoint-and-disturbance-response/README.md)
10. [`control-lesson-10`](../../../examples/control/01-feedback-foundations-and-temperature-control/10-proportional-setpoint-weighting/README.md)
11. [`control-lesson-11`](../../../examples/control/01-feedback-foundations-and-temperature-control/11-measurement-filtering/README.md)
12. [`control-lesson-12`](../../../examples/control/01-feedback-foundations-and-temperature-control/12-elapsed-time-and-sampling/README.md)
13. [`control-lesson-13`](../../../examples/control/01-feedback-foundations-and-temperature-control/13-process-response-classes/README.md)
14. [`control-lesson-14`](../../../examples/control/01-feedback-foundations-and-temperature-control/14-conservation-and-accumulation/README.md)
15. [`control-water-heater-energy-balance`](../../../examples/control/01-feedback-foundations-and-temperature-control/15-water-heater-energy-balance/README.md)
16. [`control-lesson-16`](../../../examples/control/01-feedback-foundations-and-temperature-control/16-gravity-tank-nonlinearity/README.md)
17. [`control-lesson-17`](../../../examples/control/01-feedback-foundations-and-temperature-control/17-gravity-tank-linearization/README.md)
18. [`control-lesson-18`](../../../examples/control/01-feedback-foundations-and-temperature-control/18-transfer-function-from-physics/README.md)
19. [`control-lesson-19`](../../../examples/control/02-derivative-action-and-dynamic-response/19-poles-zeros-and-response/README.md)
20. [`control-lesson-20`](../../../examples/control/02-derivative-action-and-dynamic-response/20-state-equations-from-stores/README.md)
21. [`control-lesson-21`](../../../examples/control/05-experiments-identification-and-validation/21-safe-informative-experiment/README.md)
22. [`control-lesson-22`](../../../examples/control/05-experiments-identification-and-validation/22-excitation-for-dynamics/README.md)
23. [`control-lesson-23`](../../../examples/control/05-experiments-identification-and-validation/23-step-response-identification/README.md)
24. [`control-lesson-24`](../../../examples/control/05-experiments-identification-and-validation/24-choose-model-structure/README.md)
25. [`control-lesson-25`](../../../examples/control/05-experiments-identification-and-validation/25-parameter-identifiability/README.md)
26. [`control-lesson-26`](../../../examples/control/05-experiments-identification-and-validation/26-independent-model-validation/README.md)
27. [`control-lesson-27`](../../../examples/control/04-controller-selection-and-plant-models/27-controller-selection/README.md)
28. [`control-lesson-28`](../../../examples/control/06-pid-tuning-methods/28-manual-pi-tuning/README.md)
29. [`control-relay-tuning`](../../../examples/control/06-pid-tuning-methods/29-relay-tuning/README.md)
30. [`control-model-based-tuning`](../../../examples/control/06-pid-tuning-methods/30-model-based-pi-tuning/README.md)
31. [`control-tuning-method-comparison`](../../../examples/control/06-pid-tuning-methods/31-tuning-method-comparison/README.md)
32. [`control-proportional-position-stiffness`](../../../examples/control/02-derivative-action-and-dynamic-response/32-proportional-position-stiffness/README.md)
33. [`control-derivative-motion-damping`](../../../examples/control/02-derivative-action-and-dynamic-response/33-derivative-motion-damping/README.md)
34. [`control-loaded-position-pid`](../../../examples/control/02-derivative-action-and-dynamic-response/34-servo-pid-holding-torque/README.md)
36. [`control-servo-filtered-derivative`](../../../examples/control/02-derivative-action-and-dynamic-response/36-filtered-derivative/README.md)
37. [`control-discrete-integral-derivative`](../../../examples/control/03-production-quality-digital-pid/37-discrete-integral-derivative/README.md)
38. [`control-pid-forms`](../../../examples/control/03-production-quality-digital-pid/38-positional-incremental-pid/README.md)
39. [`control-booster-pump-modes`](../../../examples/control/03-production-quality-digital-pid/39-controller-mode-initialization/README.md)
40. [`control-booster-pump-bumpless-transfer`](../../../examples/control/03-production-quality-digital-pid/40-bumpless-transfer/README.md)
41. [`control-booster-pump-atomic-update`](../../../examples/control/03-production-quality-digital-pid/41-atomic-parameter-update/README.md)
42. [`control-booster-pump-invalid-data`](../../../examples/control/03-production-quality-digital-pid/42-invalid-stale-data/README.md)
43. [`control-characteristic-equation`](../../../examples/control/07-classical-stability-and-frequency-design/43-characteristic-equation/README.md)
44. [`control-routh-hurwitz`](../../../examples/control/07-classical-stability-and-frequency-design/44-routh-hurwitz/README.md)
45. [`control-root-locus`](../../../examples/control/07-classical-stability-and-frequency-design/45-root-locus/README.md)
46. [`control-bode-frequency-response`](../../../examples/control/07-classical-stability-and-frequency-design/46-bode-frequency-response/README.md)
47. [`control-nyquist-stability`](../../../examples/control/07-classical-stability-and-frequency-design/47-nyquist-stability/README.md)
48. [`control-sensitivity-functions`](../../../examples/control/07-classical-stability-and-frequency-design/48-sensitivity-functions/README.md)
49. [`control-loop-shaping`](../../../examples/control/07-classical-stability-and-frequency-design/49-loop-shaping/README.md)
50. [`control-position-z-plane-sampling`](../../../examples/control/07-classical-stability-and-frequency-design/50-position-z-plane/README.md)
51. [`control-feedforward`](../../../examples/control/09-industrial-control-structures/51-feedforward/README.md)
52. [`control-cascade`](../../../examples/control/09-industrial-control-structures/52-cascade/README.md)
53. [`control-ratio-control`](../../../examples/control/09-industrial-control-structures/53-ratio-control/README.md)
54. [`control-split-range`](../../../examples/control/09-industrial-control-structures/54-split-range/README.md)
55. [`control-override-control`](../../../examples/control/09-industrial-control-structures/55-override-control/README.md)
56. [`control-gain-scheduling`](../../../examples/control/09-industrial-control-structures/56-gain-scheduling/README.md)
57. [`control-smith-predictor`](../../../examples/control/09-industrial-control-structures/57-smith-predictor/README.md)
58. [`control-valve-characterization`](../../../examples/control/09-industrial-control-structures/58-valve-characterization/README.md)
59. [`control-valve-stiction`](../../../examples/control/09-industrial-control-structures/59-valve-stiction/README.md)
60. [`control-loop-diagnosis`](../../../examples/control/09-industrial-control-structures/60-loop-diagnosis/README.md)
61. [`control-mimo-paths`](../../../examples/control/10-multivariable-control/61-mimo-paths/README.md)
62. [`control-relative-gain-array`](../../../examples/control/10-multivariable-control/62-relative-gain-array/README.md)
63. [`control-decentralized-pi`](../../../examples/control/10-multivariable-control/63-decentralized-pi/README.md)
64. [`control-decoupling`](../../../examples/control/10-multivariable-control/64-decoupling/README.md)
65. [`control-robust-mimo-selection`](../../../examples/control/10-multivariable-control/65-robust-mimo-selection/README.md)
66. [`control-controllability`](../../../examples/control/11-state-space-estimation-and-predictive-control/66-controllability/README.md)
67. [`control-observability`](../../../examples/control/11-state-space-estimation-and-predictive-control/67-observability/README.md)
68. [`control-state-feedback`](../../../examples/control/11-state-space-estimation-and-predictive-control/68-state-feedback/README.md)
69. [`control-lqr`](../../../examples/control/11-state-space-estimation-and-predictive-control/69-lqr/README.md)
70. [`control-state-observer`](../../../examples/control/11-state-space-estimation-and-predictive-control/70-state-observer/README.md)
71. [`control-kalman-filter`](../../../examples/control/11-state-space-estimation-and-predictive-control/71-kalman-filter/README.md)
72. [`control-lqg`](../../../examples/control/11-state-space-estimation-and-predictive-control/72-lqg/README.md)
73. [`control-mpc`](../../../examples/control/11-state-space-estimation-and-predictive-control/73-mpc/README.md)
74. [`control-lyapunov-stability`](../../../examples/control/12-nonlinear-adaptive-and-robust-control/74-lyapunov-stability/README.md)
75. [`control-feedback-linearization`](../../../examples/control/12-nonlinear-adaptive-and-robust-control/75-feedback-linearization/README.md)
76. [`control-adaptive-control`](../../../examples/control/12-nonlinear-adaptive-and-robust-control/76-adaptive-control/README.md)
77. [`control-robust-control`](../../../examples/control/12-nonlinear-adaptive-and-robust-control/77-robust-control/README.md)
78. [`control-networked-control`](../../../examples/control/13-networked-fault-aware-and-diagnostic-control/78-networked-control/README.md)
79. [`control-fault-aware-control`](../../../examples/control/13-networked-fault-aware-and-diagnostic-control/79-fault-aware-control/README.md)
80. [`control-capstone-cold-storage`](../../../examples/control/14-integrated-capstones/80-cold-storage/README.md)
81. [`control-capstone-water-heater`](../../../examples/control/14-integrated-capstones/81-water-heater/README.md)
82. [`control-capstone-servo`](../../../examples/control/14-integrated-capstones/82-servo/README.md)
83. [`control-capstone-reactor`](../../../examples/control/14-integrated-capstones/83-reactor-control/README.md)
84. [`control-capstone-independent`](../../../examples/control/14-integrated-capstones/84-independent/README.md)

Completing the course teaches the declared simulation and introductory design
scope. It does not replace plant-specific commissioning, machinery protection,
safety lifecycle work, regulatory qualification, or approval for a physical
experiment.
