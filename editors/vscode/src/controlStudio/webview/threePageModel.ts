import type { GeneralModel } from "../generalModel";
import { formatEngineeringNumber } from "../chartContract";

/** Presentation of the host's selected model; never fits or changes its coefficients. */
export function renderIdentifiedModel(root: HTMLElement, model: GeneralModel): void {
  const type = root.querySelector<HTMLElement>("[data-studio-model-type]")!;
  const equation = root.querySelector<HTMLElement>("[data-studio-model-equation]")!;
  if (model.kind === "transfer-function") {
    const order = model.denominator.length - 1;
    const integrator = model.denominator[model.denominator.length - 1] === 0;
    type.textContent = `${integrator ? "Integrating" : order === 0 ? "Static gain" : order === 1 ? "First-order" : order === 2 ? "Second-order" : `${order}-order`} model${model.delaySeconds ? " with delay" : ""}`;
    equation.textContent = `G(s) = (${polynomial(model.numerator)}) / (${polynomial(model.denominator)})${model.delaySeconds ? ` · e^(−${number(model.delaySeconds)}s)` : ""}`;
  } else {
    const discrete = model.kind === "discrete-state-space";
    type.textContent = `${discrete ? "Discrete" : "Continuous"} state-space model${model.inputs.length > 1 || model.outputs.length > 1 ? ` · ${model.inputs.length} inputs, ${model.outputs.length} outputs` : ""}`;
    const inputOffset = model.operatingPoint?.inputs;
    const outputOffset = model.operatingPoint?.outputs;
    const input = inputOffset?.some(value => value !== 0) ? `(u${discrete ? "[k]" : ""} − ${vector(inputOffset)})` : discrete ? "u[k]" : "u";
    const state = discrete ? "x[k]" : "x";
    const output = outputOffset?.some(value => value !== 0) ? `${vector(outputOffset)} + ` : "";
    equation.textContent = `${discrete ? "x[k+1]" : "dx/dt"} = ${matrix(model.a)} ${state} + ${matrix(model.b)} ${input}\n${discrete ? "y[k]" : "y"} = ${output}${matrix(model.c)} ${state} + ${matrix(model.d)} ${input}${discrete ? `\nSample period = ${number(model.sampleTimeSeconds!)} s` : ""}${model.delaySeconds ? `\nInput delay = ${number(model.delaySeconds)} s` : ""}`;
  }
  if (model.validation === "unvalidated") type.textContent += " · Provisional";
  root.querySelector<HTMLElement>("[data-studio-model-result]")!.hidden = false;
}
const number = (value: number) => formatEngineeringNumber(value);
const matrix = (values: readonly (readonly number[])[]) => `[${values.map(row => row.map(number).join(", ")).join("; ")}]`;
const vector = (values: readonly number[]) => values.length === 1 ? number(values[0]) : `[${values.map(number).join(", ")}]`;
function polynomial(values: readonly number[]): string {
  return values.map((value, index) => {
    if (!value) return "";
    const power = values.length - index - 1;
    return `${value < 0 ? "−" : "+"}${number(Math.abs(value))}${power ? power === 1 ? "s" : `s^${power}` : ""}`;
  }).filter(Boolean).join(" ").replace(/^\+/, "") || "0";
}
