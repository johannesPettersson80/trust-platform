import { ENGINE_MODEL_CHANGED_EVENT } from "./engineAnalysis";
import { showEngineRequirements } from "./engineRequirements";

/**
 * Shows the engine's identification on the Identify page (specification 35, B3.8, B9.1 and B9.6). The model type, the equation,
 * the key numbers and the Provisional label are the engine's words, written into the page's model fields. The response plot goes
 * to the page's one chart, with the applied plant input the experiment held drawn beside the measurement and the prediction. The
 * requirements go to the Requirements editor. The page lays these values out and computes none of them.
 */

/** Shows the identification the engine answered: an Identify answer, a run's identification, or a reopened study's (A8.6). */
export function showEngineIdentification(root: HTMLElement, result: Record<string, unknown>): void {
  const type = root.querySelector<HTMLElement>("[data-studio-model-type]");
  const equation = root.querySelector<HTMLElement>("[data-studio-model-equation]");
  const numbers = root.querySelector<HTMLElement>("[data-studio-model-numbers]");
  const warnings = root.querySelector<HTMLElement>("[data-studio-model-warnings]");
  const provisional = result.provisional === true;
  if (type) type.textContent = `${textOf(result.modelType)}${provisional ? " · Provisional" : ""}`;
  if (equation) equation.textContent = textOf(result.equation);
  if (numbers) {
    numbers.textContent = textOf(result.keyNumbers);
    numbers.hidden = numbers.textContent === "";
  }
  // The engine states no warnings with its result; the previous model's warnings do not stay.
  if (warnings) {
    warnings.replaceChildren();
    warnings.hidden = true;
  }
  root.querySelector<HTMLElement>("[data-studio-model-result]")?.removeAttribute("hidden");
  showPlot(root, result.plot);
  showEngineRequirements(root, result.requirements);
  // The status line is the engine's sentence for this identification (`statusSentence`, B3.8, B9.9); the page writes none.
  identificationStatus(root, textOf(result.statusSentence));
  dispatchOnView(root, ENGINE_MODEL_CHANGED_EVENT);
}

/**
 * Clears the model of the study the engine has reset (A8.4): a new working study holds no identification yet, and the status line
 * says so in the engine's sentence (`sentence`, the reset answer's `statusSentence`, B9.9). The chart drops its model comparison and resumes
 * live observation from the reset's own answer (engineStages.ts), as the Identify page specification's Reset requires.
 */
export function clearEngineIdentification(root: HTMLElement, sentence: string): void {
  const type = root.querySelector<HTMLElement>("[data-studio-model-type]");
  const equation = root.querySelector<HTMLElement>("[data-studio-model-equation]");
  const numbers = root.querySelector<HTMLElement>("[data-studio-model-numbers]");
  const warnings = root.querySelector<HTMLElement>("[data-studio-model-warnings]");
  if (type) type.textContent = "";
  if (equation) equation.textContent = "";
  if (numbers) {
    numbers.textContent = "";
    numbers.hidden = true;
  }
  if (warnings) {
    warnings.replaceChildren();
    warnings.hidden = true;
  }
  root.querySelector<HTMLElement>("[data-studio-model-result]")?.setAttribute("hidden", "");
  identificationStatus(root, sentence);
  dispatchOnView(root, ENGINE_MODEL_CHANGED_EVENT);
}

/**
 * Tells the page that the engine's answer accepted a model into the study (the Recorded-data fit's accepted step, B3.3, B3.7): the Analyze…
 * list asks the engine which tools apply to it (B9.9), as it does when an answer shows or clears an identification.
 */
export function announceModelChange(root: HTMLElement): void {
  dispatchOnView(root, ENGINE_MODEL_CHANGED_EVENT);
}

/** Sends one of the page's window events on the view that holds the page, so that the webview's own listeners hear it. */
function dispatchOnView(root: HTMLElement, name: string, detail?: Record<string, unknown>): void {
  const view = root.ownerDocument.defaultView;
  view?.dispatchEvent(new view.CustomEvent(name, detail ? { detail } : undefined));
}

/** Writes the Identify page's status line. */
export function identificationStatus(root: HTMLElement, message: string): void {
  const status = root.querySelector<HTMLElement>("[data-three-page-identification-status]");
  if (status) status.textContent = message;
}

/**
 * The response plot: the measurement and the model's prediction on the engine's one time base (B9.6), and the applied plant input
 * the experiment held, which the chart draws from the experiment it is given. The chart's `predictedResponse` pairs each time with
 * both values, and a null value is a gap, never an interpolated sample (B10.3).
 */
function showPlot(root: HTMLElement, plot: unknown): void {
  const data = recordOf(plot);
  const times = arrayOf(data?.timeSeconds);
  const traces = arrayOf(data?.traces).map(recordOf).filter((trace): trace is Record<string, unknown> => trace !== undefined);
  const measured = traces.find((trace) => trace.role === "measurement");
  const predicted = traces.find((trace) => trace.role === "prediction");
  if (!measured || !predicted) {
    // An answer with no response (a supplied model, B3.7) drops the comparison of the model before it: the chart resumes live observation.
    dispatchOnView(root, "trust-control-studio-engineer", { liveIdentification: true });
    return;
  }
  const input = traces.find((trace) => trace.role === "plant-input");
  const measurement = arrayOf(measured.values);
  const prediction = arrayOf(predicted.values);
  const output = textOf(measured.symbol);
  // The applied input and the measurement share the time base; the experiment carries both as the chart's recorded traces.
  const experiment = input ? {
    inputs: [{ id: textOf(input.symbol), unit: textOf(input.unit), points: pointsOf(times, arrayOf(input.values)) }],
    outputs: [{ id: output, unit: textOf(measured.unit), points: pointsOf(times, measurement) }],
  } : undefined;
  dispatchOnView(root, "trust-control-studio-engineer", { identificationPresentation: {
    predictedResponse: times.map((timeSeconds, index) => ({
      timeSeconds, measured: measurement[index] ?? null, predicted: prediction[index] ?? null,
    })),
    ...(output ? { outputId: output } : {}),
    ...(experiment ? { experiment } : {}),
    units: { input: textOf(input?.unit), output: textOf(measured.unit), gain: "", time: "s" },
  } });
}

/** A trace's points on the time base: a value the trace does not hold is a gap (null), never an interpolated sample (B10.3). */
function pointsOf(times: readonly unknown[], values: readonly unknown[]): { timeSeconds: unknown; value: unknown }[] {
  return times.map((timeSeconds, index) => ({ timeSeconds, value: values[index] ?? null }));
}

function recordOf(value: unknown): Record<string, unknown> | undefined {
  return value && typeof value === "object" && !Array.isArray(value) ? (value as Record<string, unknown>) : undefined;
}

function arrayOf(value: unknown): unknown[] {
  return Array.isArray(value) ? value : [];
}

export function textOf(value: unknown): string {
  return typeof value === "string" ? value : "";
}
