import { candidateRobustness, type CandidateRobustness, type Margin } from "./candidateRobustness";
import type { EngineerRequirements } from "./engineerVerificationPlan";
import type { EngineeringPlotMessageEvidence } from "./engineeringPlots";
import type { FeedforwardDesign } from "./feedforwardPidDesign";
import type { GeneralModel } from "./generalModel";
import type { ControlLoopDefinition } from "./loopDefinition";
import { projectMarginEvidence } from "./projectCandidateMargins";
import { decodeStateControllerBinding, sampleModel } from "./stateControllerDesign";

/**
 * The margins Tune shows beside a candidate (Specification 35 §17.10): of a PID's predicted loop, of a program's own
 * linear law, of a feedforward's PI on its output path, or of a state-feedback law broken at the plant input.
 */
export function candidateMargins(model: GeneralModel | undefined, loop: ControlLoopDefinition,
  design: { candidate: unknown; feedforward?: FeedforwardDesign }, task: EngineerRequirements | undefined,
  plots: EngineeringPlotMessageEvidence | undefined): CandidateRobustness | undefined {
  if (plots) return candidateRobustness(plots);
  const parameters = (design.candidate as { parameters?: Readonly<Record<string, number>> }).parameters ?? {};
  if (loop.controller.stateFeedback) return model ? stateFeedbackMargins(loop, model, parameters) : undefined;
  const evidence = projectMarginEvidence(model, loop, design, task);
  return evidence ? candidateRobustness(evidence) : undefined;
}

interface Complex { re: number; im: number }

/** L(e^{jωT}) = K·(e^{jωT}I − A)⁻¹·B for one plant input, with the model's input delay, up to half the sampling frequency. */
function stateFeedbackMargins(loop: ControlLoopDefinition, model: GeneralModel, parameters: Readonly<Record<string, number>>): CandidateRobustness | undefined {
  const definition = decodeStateControllerBinding(loop.controller.stateFeedback, loop.parameterBinding);
  if (!definition || definition.family === "LQG" || model.kind === "transfer-function" || model.inputs.length !== 1) return undefined;
  const sampled = sampleModel(model, loop.controller.taskPeriodMs / 1000), period = sampled.sampleTimeSeconds;
  const gain = definition.gainSymbols[0].map(symbol => parameters[symbol]);
  if (gain.length !== sampled.a.length || gain.some(value => !Number.isFinite(value))) return undefined;
  const nyquist = Math.PI / period, count = 2000, lowest = nyquist * 1e-5;
  const points = Array.from({ length: count }, (_, index) => {
    const omega = lowest * (0.999 * nyquist / lowest) ** (index / (count - 1));
    const state = solve(sampled.a.map((row, i) => row.map((value, j) => ({ re: (i === j ? Math.cos(omega * period) : 0) - value,
      im: i === j ? Math.sin(omega * period) : 0 }))), sampled.b.map(row => ({ re: row[0], im: 0 })));
    const loopValue = state.reduce((sum, value, index) => ({ re: sum.re + gain[index] * value.re, im: sum.im + gain[index] * value.im }), { re: 0, im: 0 });
    const delay = -omega * model.delaySeconds;
    return { omega, value: { re: loopValue.re * Math.cos(delay) - loopValue.im * Math.sin(delay), im: loopValue.re * Math.sin(delay) + loopValue.im * Math.cos(delay) } };
  });
  let previous = 0;
  const phases = points.map(({ value }, index) => {
    let phase = Math.atan2(value.im, value.re) * 180 / Math.PI;
    if (index > 0) { while (phase - previous > 180) phase -= 360; while (phase - previous < -180) phase += 360; }
    return previous = phase;
  });
  const magnitudes = points.map(({ value }) => Math.hypot(value.re, value.im));
  const crossing = (series: readonly number[], level: number) => {
    for (let index = 1; index < series.length; index++) {
      if ((series[index - 1] - level) * (series[index] - level) <= 0 && series[index - 1] !== series[index]) {
        return { index, fraction: (level - series[index - 1]) / (series[index] - series[index - 1]) };
      }
    }
    return undefined;
  };
  const at = (series: readonly number[], found: { index: number; fraction: number }) => series[found.index - 1] + found.fraction * (series[found.index] - series[found.index - 1]);
  const frequencies = points.map(point => point.omega);
  const gainCross = crossing(magnitudes, 1), phaseCross = crossing(phases, -180);
  const phaseMargin: Margin = gainCross ? 180 + at(phases, gainCross) : "unbounded";
  const gainMargin: Margin = phaseCross ? -20 * Math.log10(at(magnitudes, phaseCross)) : "unbounded";
  const crossover = gainCross ? at(frequencies, gainCross) : undefined;
  return {
    gainMarginDb: gainMargin, phaseMarginDegrees: phaseMargin,
    delayMarginSeconds: typeof phaseMargin === "number" && crossover ? phaseMargin * Math.PI / 180 / crossover : "unbounded",
    peakSensitivity: Math.max(...points.map(({ value }) => 1 / Math.hypot(1 + value.re, value.im))),
  };
}

/** Gaussian elimination with partial pivoting on a small complex system. */
function solve(matrix: Complex[][], right: Complex[]): Complex[] {
  const size = matrix.length, a = matrix.map((row, index) => [...row, right[index]]);
  const multiply = (x: Complex, y: Complex): Complex => ({ re: x.re * y.re - x.im * y.im, im: x.re * y.im + x.im * y.re });
  const divide = (x: Complex, y: Complex): Complex => { const d = y.re * y.re + y.im * y.im; return { re: (x.re * y.re + x.im * y.im) / d, im: (x.im * y.re - x.re * y.im) / d }; };
  for (let column = 0; column < size; column++) {
    let pivot = column;
    for (let row = column + 1; row < size; row++) if (Math.hypot(a[row][column].re, a[row][column].im) > Math.hypot(a[pivot][column].re, a[pivot][column].im)) pivot = row;
    [a[column], a[pivot]] = [a[pivot], a[column]];
    for (let row = column + 1; row < size; row++) {
      const factor = divide(a[row][column], a[column][column]);
      for (let k = column; k <= size; k++) { const product = multiply(factor, a[column][k]); a[row][k] = { re: a[row][k].re - product.re, im: a[row][k].im - product.im }; }
    }
  }
  const result: Complex[] = Array(size);
  for (let row = size - 1; row >= 0; row--) {
    let sum = a[row][size];
    for (let k = row + 1; k < size; k++) { const product = multiply(a[row][k], result[k]); sum = { re: sum.re - product.re, im: sum.im - product.im }; }
    result[row] = divide(sum, a[row][row]);
  }
  return result;
}
