import type { GeneralModel, GeneralTransferModel } from "../generalModel";
import type { IdentifiedModelPresentation } from "../identification";
import { displayEngineeringUnit, formatEngineeringNumber } from "../chartContract";
import { powerOfS } from "../powerNotation";
import { formatDurationSeconds } from "./durationField";

/** The model's key numbers in engineering units; a value stays undefined when the model does not state it. */
interface KeyNumbers {
  integrating: boolean;
  gain?: { value: number; output: string; input: string };
  timeConstants: readonly number[];
  delaySeconds: number;
  order: number;
  states?: number;
}

/** Presentation of the host's selected model; never fits or changes its coefficients. */
export function renderIdentifiedModel(root: HTMLElement, model: GeneralModel): void {
  const type = root.querySelector<HTMLElement>("[data-studio-model-type]")!;
  const equation = root.querySelector<HTMLElement>("[data-studio-model-equation]")!;
  const doc = root.ownerDocument;
  if (model.kind === "transfer-function") {
    const order = model.denominator.length - 1;
    const integrator = model.denominator[model.denominator.length - 1] === 0;
    type.textContent = `${integrator ? "Integrating" : order === 0 ? "Static gain" : order === 1 ? "First-order" : order === 2 ? "Second-order" : `${order}-order`} model${model.delaySeconds ? " with delay" : ""}`;
    equation.replaceChildren(...transferEquation(doc, polynomial(model.numerator), polynomial(model.denominator), model.delaySeconds));
    const numbers = transferNumbers(model);
    setKeyNumbers(root, numbers ? keyNumbersText(numbers) : `Transfer function of order ${order}${model.delaySeconds ? `, delay ${formatDurationSeconds(model.delaySeconds)}` : ""}`);
  } else {
    const discrete = model.kind === "discrete-state-space";
    type.textContent = `${discrete ? "Discrete" : "Continuous"} state-space model${model.inputs.length > 1 || model.outputs.length > 1 ? ` · ${channelCount(model.inputs.length, "input")}, ${channelCount(model.outputs.length, "output")}` : ""}`;
    const inputOffset = model.operatingPoint?.inputs;
    const outputOffset = model.operatingPoint?.outputs;
    const input = inputOffset?.some(value => value !== 0) ? `(u${discrete ? "[k]" : ""} − ${vector(inputOffset)})` : discrete ? "u[k]" : "u";
    const state = discrete ? "x[k]" : "x";
    const output = outputOffset?.some(value => value !== 0) ? `${vector(outputOffset)} + ` : "";
    equation.textContent = `${discrete ? "x[k+1]" : "dx/dt"} = ${matrix(model.a)} ${state} + ${matrix(model.b)} ${input}\n${discrete ? "y[k]" : "y"} = ${output}${matrix(model.c)} ${state} + ${matrix(model.d)} ${input}${discrete ? `\nSample period = ${number(model.sampleTimeSeconds!)} s` : ""}${model.delaySeconds ? `\nInput delay = ${number(model.delaySeconds)} s` : ""}`;
    setKeyNumbers(root, `State model of order ${model.states.length}`);
  }
  if (model.validation === "unvalidated") type.textContent += " · Provisional";
  setWarnings(root, []);
  root.querySelector<HTMLElement>("[data-studio-model-result]")!.hidden = false;
}

/**
 * The identification result's presentation: its model type, its equation as a fraction when its gain and time
 * constants are known, one line of key numbers and the presentation's warnings.
 */
export function renderModelPresentation(root: HTMLElement, presentation: IdentifiedModelPresentation, typeLabel: string, model?: GeneralModel): void {
  const type = root.querySelector<HTMLElement>("[data-studio-model-type]")!;
  const equation = root.querySelector<HTMLElement>("[data-studio-model-equation]")!;
  const doc = root.ownerDocument;
  type.textContent = typeLabel;
  const numbers = model?.kind === "transfer-function" ? transferNumbers(model) : presentationNumbers(presentation);
  if (model?.kind === "transfer-function" && numbers) {
    equation.replaceChildren(...transferEquation(doc, polynomial(model.numerator), polynomial(model.denominator), model.delaySeconds));
  } else if (numbers?.gain) {
    const denominator = numbers.integrating ? "s" : numbers.timeConstants.map(value => `(${number(value)}s + 1)`).join("") || "1";
    equation.replaceChildren(...transferEquation(doc, number(numbers.gain.value), denominator, presentation.delaySeconds));
  } else {
    equation.textContent = presentation.equation;
  }
  setKeyNumbers(root, numbers ? keyNumbersText(numbers) : "");
  setWarnings(root, presentation.warnings);
  root.querySelector<HTMLElement>("[data-studio-model-result]")!.hidden = false;
}

/** G(s) = fraction · e^(−θs): the fraction is written as a stacked numerator over denominator. */
function transferEquation(doc: Document, numerator: string, denominator: string, delaySeconds: number): Node[] {
  const fraction = doc.createElement("span"); fraction.className = "studio-math-fraction";
  const top = doc.createElement("span"); top.className = "studio-math-num"; top.textContent = numerator;
  const bottom = doc.createElement("span"); bottom.className = "studio-math-den"; bottom.textContent = denominator;
  fraction.append(top, bottom);
  const nodes: Node[] = [doc.createTextNode("G(s) = "), fraction];
  if (delaySeconds > 0) {
    const exponent = doc.createElement("sup"); exponent.textContent = `−${number(delaySeconds)}s`;
    nodes.push(doc.createTextNode(" · e"), exponent);
  }
  return nodes;
}

/** Gain and time constants from the general transfer function, normalised so that its constant term is one. */
function transferNumbers(model: GeneralTransferModel): KeyNumbers | undefined {
  const { numerator: num, denominator: den } = model;
  const output = model.outputs[0]?.unit ?? "", input = model.inputs[0]?.unit ?? "";
  const order = den.length - 1;
  if (num.length !== 1) return undefined;
  if (order === 1 && den[1] === 0) return { integrating: true, gain: { value: num[0] / den[0], output, input }, timeConstants: [], delaySeconds: model.delaySeconds, order: 1 };
  if (order === 1 && den[1] !== 0) return { integrating: false, gain: { value: num[0] / den[1], output, input }, timeConstants: [den[0] / den[1]], delaySeconds: model.delaySeconds, order: 1 };
  if (order === 2 && den[2] !== 0) {
    // (τ1·s + 1)(τ2·s + 1) = τ1·τ2·s² + (τ1 + τ2)·s + 1; real roots only.
    const sum = den[1] / den[2], product = den[0] / den[2];
    const discriminant = sum * sum - 4 * product;
    if (discriminant < 0) return undefined;
    const root = Math.sqrt(discriminant);
    return { integrating: false, gain: { value: num[0] / den[2], output, input }, timeConstants: [(sum + root) / 2, (sum - root) / 2], delaySeconds: model.delaySeconds, order: 2 };
  }
  return undefined;
}

/** The same key numbers from an identification result's own parameters (Gain, Time constant n, Dead time). */
function presentationNumbers(presentation: IdentifiedModelPresentation): KeyNumbers | undefined {
  const gain = presentation.parameters.find(parameter => parameter.name === "Gain");
  if (!gain) return undefined;
  const timeConstants = presentation.parameters.filter(parameter => /^Time constant \d+$/.test(parameter.name)).map(parameter => parameter.value);
  const integrating = timeConstants.length === 0 && presentation.poles.length === 1 && presentation.poles[0] === 0;
  return { integrating, gain: { value: gain.value, output: presentation.units.output, input: presentation.units.input }, timeConstants,
    delaySeconds: presentation.delaySeconds, order: timeConstants.length || 1 };
}

/** One plain line: "Gain 0.21 °C per %, time constant 75 min, delay 12 s". */
function keyNumbersText(numbers: KeyNumbers): string {
  const parts: string[] = [];
  if (numbers.gain) {
    const unit = unitPhrase(numbers.gain.output, numbers.gain.input);
    parts.push(numbers.integrating ? `Rate gain ${number(numbers.gain.value * 60)}${unit ? ` ${unit}` : ""} per min`
      : `Gain ${number(numbers.gain.value)}${unit ? ` ${unit}` : ""}`);
  }
  if (numbers.timeConstants.length === 1) parts.push(`time constant ${formatDurationSeconds(numbers.timeConstants[0])}`);
  if (numbers.timeConstants.length === 2) parts.push(`time constants ${formatDurationSeconds(numbers.timeConstants[0])} and ${formatDurationSeconds(numbers.timeConstants[1])}`);
  if (numbers.delaySeconds > 0) parts.push(`delay ${formatDurationSeconds(numbers.delaySeconds)}`);
  return parts.join(", ");
}

/** "°C per %" from "°C/%"; a dimensionless side reads as 1. */
function unitPhrase(output: string, input: string): string {
  const out = cleanUnit(output), inp = cleanUnit(input);
  if (!out && !inp) return "";
  return `${out || "1"} per ${inp || "1"}`;
}

function cleanUnit(unit: string): string {
  const text = unit.trim();
  return text === "" || text === "1" ? "" : displayEngineeringUnit(text);
}

function setKeyNumbers(root: HTMLElement, text: string): void {
  const line = root.querySelector<HTMLElement>("[data-studio-model-numbers]");
  if (!line) return;
  line.textContent = text;
  line.hidden = !text;
}

function setWarnings(root: HTMLElement, warnings: readonly string[]): void {
  const list = root.querySelector<HTMLElement>("[data-studio-model-warnings]");
  if (!list) return;
  list.replaceChildren(...warnings.map(warning => {
    const item = root.ownerDocument.createElement("li"); item.textContent = warning; return item;
  }));
  list.hidden = warnings.length === 0;
}

/** "1 input", "2 outputs". */
export const channelCount = (count: number, noun: string): string => `${count} ${noun}${count === 1 ? "" : "s"}`;
const number = (value: number) => formatEngineeringNumber(value);
const matrix = (values: readonly (readonly number[])[]) => `[${values.map(row => row.map(number).join(", ")).join("; ")}]`;
const vector = (values: readonly number[]) => values.length === 1 ? number(values[0]) : `[${values.map(number).join(", ")}]`;
function polynomial(values: readonly number[]): string {
  return values.map((value, index) => {
    if (!value) return "";
    const power = values.length - index - 1;
    return `${value < 0 ? "−" : "+"}${number(Math.abs(value))}${power ? powerOfS(power) : ""}`;
  }).filter(Boolean).join(" ").replace(/^\+/, "") || "0";
}
