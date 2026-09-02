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

1. [`control-cold-storage-loop-anatomy`](../../../examples/control/01-feedback-foundations-and-temperature-control/01-cold-storage-loop-anatomy/README.md)
2. [`control-cold-storage-on-off`](../../../examples/control/01-feedback-foundations-and-temperature-control/02-cold-storage-on-off/README.md)
3. [`control-cold-storage-hysteresis-dwell`](../../../examples/control/01-feedback-foundations-and-temperature-control/03-cold-storage-hysteresis-dwell/README.md)
4. [`control-water-heater-p`](../../../examples/control/01-feedback-foundations-and-temperature-control/04-water-heater-p/README.md)
5. [`control-water-heater-bias`](../../../examples/control/01-feedback-foundations-and-temperature-control/05-water-heater-bias/README.md)
6. [`control-water-heater-integral`](../../../examples/control/01-feedback-foundations-and-temperature-control/06-water-heater-integral/README.md)
7. [`control-water-heater-pi`](../../../examples/control/01-feedback-foundations-and-temperature-control/07-water-heater-pi/README.md)
8. [`control-water-heater-saturation`](../../../examples/control/01-feedback-foundations-and-temperature-control/08-water-heater-saturation/README.md)
9. [`control-rate-limit`](../../../examples/control/01-feedback-foundations-and-temperature-control/09-rate-limit/README.md)
10. [`control-saturation-antiwindup`](../../../examples/control/01-feedback-foundations-and-temperature-control/10-saturation-antiwindup/README.md)
11. [`control-reactor-servo-regulatory`](../../../examples/control/01-feedback-foundations-and-temperature-control/11-reactor-servo-regulatory/README.md)
12. [`control-reactor-setpoint-weighting`](../../../examples/control/01-feedback-foundations-and-temperature-control/12-reactor-setpoint-weighting/README.md)
13. [`control-measurement-noise`](../../../examples/control/01-feedback-foundations-and-temperature-control/13-measurement-noise/README.md)
14. [`control-sampling-interval`](../../../examples/control/01-feedback-foundations-and-temperature-control/14-sampling-interval/README.md)
15. [`control-water-heater-energy-balance`](../../../examples/control/01-feedback-foundations-and-temperature-control/15-water-heater-energy-balance/README.md)
16. [`control-heater-step-identification`](../../../examples/control/01-feedback-foundations-and-temperature-control/16-heater-step-identification/README.md)
17. [`control-independent-heater-model-validation`](../../../examples/control/01-feedback-foundations-and-temperature-control/17-independent-model-validation/README.md)
18. [`control-nominal-fopdt`](../../../examples/control/01-feedback-foundations-and-temperature-control/18-nominal-fopdt/README.md)
19. [`control-servo-position-p`](../../../examples/control/02-derivative-action-and-dynamic-response/19-servo-position-p/README.md)
20. [`control-servo-position-pd`](../../../examples/control/02-derivative-action-and-dynamic-response/20-servo-position-pd/README.md)
21. [`control-loaded-position-pid`](../../../examples/control/02-derivative-action-and-dynamic-response/21-underdamped-pid/README.md)
22. [`control-servo-derivative-kick`](../../../examples/control/02-derivative-action-and-dynamic-response/22-servo-derivative-kick/README.md)
23. [`control-servo-filtered-derivative`](../../../examples/control/02-derivative-action-and-dynamic-response/23-servo-filtered-derivative/README.md)
24. [`control-servo-mechanical-nonidealities`](../../../examples/control/02-derivative-action-and-dynamic-response/24-servo-mechanical-nonidealities/README.md)
25. [`control-servo-digital-realization`](../../../examples/control/02-derivative-action-and-dynamic-response/25-servo-digital-realization/README.md)
26. [`control-pid-forms`](../../../examples/control/03-production-quality-digital-pid/26-pid-forms/README.md)
27. [`control-booster-pump-modes`](../../../examples/control/03-production-quality-digital-pid/27-booster-pump-modes/README.md)
28. [`control-booster-pump-bumpless-transfer`](../../../examples/control/03-production-quality-digital-pid/28-booster-pump-bumpless-transfer/README.md)
29. [`control-booster-pump-atomic-update`](../../../examples/control/03-production-quality-digital-pid/29-booster-pump-atomic-update/README.md)
30. [`control-booster-pump-invalid-data`](../../../examples/control/03-production-quality-digital-pid/30-booster-pump-invalid-data/README.md)
31. [`control-controller-selection`](../../../examples/control/04-controller-selection-and-plant-models/31-controller-selection/README.md)
32. [`control-plant-response-classes`](../../../examples/control/04-controller-selection-and-plant-models/32-plant-response-classes/README.md)
33. [`control-tank-heater-conservation`](../../../examples/control/04-controller-selection-and-plant-models/33-tank-heater-conservation/README.md)
34. [`control-gravity-tank-linearization`](../../../examples/control/04-controller-selection-and-plant-models/34-gravity-tank-linearization/README.md)
35. [`control-pole-zero-plant-classes`](../../../examples/control/04-controller-selection-and-plant-models/35-pole-zero-plant-classes/README.md)
36. [`control-two-tank-state-space`](../../../examples/control/04-controller-selection-and-plant-models/36-two-tank-state-space/README.md)
37. [`control-safe-heat-exchanger-experiment`](../../../examples/control/05-experiments-identification-and-validation/37-safe-heat-exchanger-experiment/README.md)
38. [`control-excitation-methods`](../../../examples/control/05-experiments-identification-and-validation/38-excitation-methods/README.md)
39. [`control-dryer-model-selection`](../../../examples/control/05-experiments-identification-and-validation/39-dryer-model-selection/README.md)
40. [`control-poor-excitation`](../../../examples/control/05-experiments-identification-and-validation/40-poor-excitation/README.md)
41. [`control-independent-model-validation`](../../../examples/control/05-experiments-identification-and-validation/41-heat-exchanger-validation-uncertainty/README.md)
42. [`control-manual-historical-tuning`](../../../examples/control/06-pid-tuning-methods/42-heat-exchanger-tuning-methods/README.md)
43. [`control-relay-autotuning`](../../../examples/control/06-pid-tuning-methods/43-relay-autotuning/README.md)
44. [`control-model-based-tuning`](../../../examples/control/06-pid-tuning-methods/44-model-based-tuning/README.md)
45. [`control-crane-routh-stability`](../../../examples/control/07-classical-stability-and-frequency-design/45-crane-routh-stability/README.md)
46. [`control-servo-root-locus`](../../../examples/control/07-classical-stability-and-frequency-design/46-servo-root-locus/README.md)
47. [`control-flexible-drive-bode`](../../../examples/control/07-classical-stability-and-frequency-design/47-flexible-drive-bode/README.md)
48. [`control-delayed-thermal-nyquist`](../../../examples/control/07-classical-stability-and-frequency-design/48-delayed-thermal-nyquist/README.md)
49. [`control-motor-sensitivity-loop-shaping`](../../../examples/control/07-classical-stability-and-frequency-design/49-motor-sensitivity/README.md)
50. [`control-position-z-plane-sampling`](../../../examples/control/07-classical-stability-and-frequency-design/50-position-z-plane/README.md)
51. [`control-tuning-method-comparison`](../../../examples/control/08-tuning-qualification-and-automation/51-method-comparison/README.md)
52. [`control-automatic-tuning-sequence`](../../../examples/control/08-tuning-qualification-and-automation/52-automatic-water-heater/README.md)
53. [`control-conveyor-oven-feedforward`](../../../examples/control/09-industrial-control-structures/53-conveyor-oven-feedforward/README.md)
54. [`control-reactor-cascade`](../../../examples/control/09-industrial-control-structures/54-reactor-cascade/README.md)
55. [`control-blending-ratio`](../../../examples/control/09-industrial-control-structures/55-blending-ratio/README.md)
56. [`control-reactor-split-range`](../../../examples/control/09-industrial-control-structures/56-reactor-split-range/README.md)
57. [`control-compressor-override`](../../../examples/control/09-industrial-control-structures/57-compressor-override/README.md)
58. [`control-gravity-tank-scheduling`](../../../examples/control/09-industrial-control-structures/58-gravity-tank-scheduling/README.md)
59. [`control-heated-air-duct-smith`](../../../examples/control/09-industrial-control-structures/59-heated-air-duct-smith/README.md)
60. [`control-valve-stiction`](../../../examples/control/09-industrial-control-structures/60-valve-stiction/README.md)
61. [`control-distillation-mimo`](../../../examples/control/10-multivariable-control/61-distillation-mimo/README.md)
62. [`control-distillation-decoupling`](../../../examples/control/10-multivariable-control/62-distillation-decoupling/README.md)
63. [`control-distillation-robustness`](../../../examples/control/10-multivariable-control/63-distillation-robustness/README.md)
64. [`control-two-tank-observability`](../../../examples/control/11-state-space-estimation-and-predictive-control/64-two-tank-observability/README.md)
65. [`control-magnetic-bearing-state-feedback`](../../../examples/control/11-state-space-estimation-and-predictive-control/65-magnetic-bearing-state-feedback/README.md)
66. [`control-weigh-feeder-kalman`](../../../examples/control/11-state-space-estimation-and-predictive-control/66-weigh-feeder-kalman/README.md)
67. [`control-suspension-lqr`](../../../examples/control/11-state-space-estimation-and-predictive-control/67-suspension-lqr/README.md)
68. [`control-building-hvac-mpc`](../../../examples/control/11-state-space-estimation-and-predictive-control/68-building-hvac-mpc/README.md)
69. [`control-pendulum-lyapunov`](../../../examples/control/12-nonlinear-adaptive-and-robust-control/69-pendulum-lyapunov/README.md)
70. [`control-robot-joint-nonlinear`](../../../examples/control/12-nonlinear-adaptive-and-robust-control/70-robot-joint-nonlinear/README.md)
71. [`control-extruder-adaptive`](../../../examples/control/12-nonlinear-adaptive-and-robust-control/71-extruder-adaptive/README.md)
72. [`control-flexible-airframe-robust`](../../../examples/control/12-nonlinear-adaptive-and-robust-control/72-flexible-airframe-robust/README.md)
73. [`control-networked-pumping`](../../../examples/control/13-networked-fault-aware-and-diagnostic-control/73-networked-pumping/README.md)
74. [`control-pumping-fault-aware`](../../../examples/control/13-networked-fault-aware-and-diagnostic-control/74-pumping-fault-aware/README.md)
75. [`control-loop-diagnosis`](../../../examples/control/13-networked-fault-aware-and-diagnostic-control/75-loop-diagnosis/README.md)
76. [`control-capstone-cold-storage`](../../../examples/control/14-integrated-capstones/76-cold-storage/README.md)
77. [`control-capstone-water-heater`](../../../examples/control/14-integrated-capstones/77-water-heater/README.md)
78. [`control-capstone-servo`](../../../examples/control/14-integrated-capstones/78-servo/README.md)
79. [`control-capstone-reactor`](../../../examples/control/14-integrated-capstones/79-reactor/README.md)
80. [`control-capstone-gravity-tank`](../../../examples/control/14-integrated-capstones/80-gravity-tank/README.md)
81. [`control-capstone-conveyor-oven`](../../../examples/control/14-integrated-capstones/81-conveyor-oven/README.md)
82. [`control-capstone-distillation`](../../../examples/control/14-integrated-capstones/82-distillation-column/README.md)
83. [`control-capstone-building-hvac`](../../../examples/control/14-integrated-capstones/83-building-hvac/README.md)
84. [`control-capstone-independent`](../../../examples/control/14-integrated-capstones/84-independent/README.md)

Completing the course teaches the declared simulation and introductory design
scope. It does not replace plant-specific commissioning, machinery protection,
safety lifecycle work, regulatory qualification, or approval for a physical
experiment.
