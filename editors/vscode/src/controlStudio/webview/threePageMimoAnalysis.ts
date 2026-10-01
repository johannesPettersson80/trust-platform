import type { RecordedMimoAnalysis } from "../threePageMimoAnalysis";
import type { MimoSettledTrial } from "../threePageMimoAnalysis";
import { automaticFractionDigits, formatEngineeringNumber } from "../chartContract";
import { displayEngineeringUnit } from "../unitDisplay";

export interface ThreePageMimoAnalysisPresenter {
  setContext(value: ThreePageMimoAnalysisContext | undefined): void;
  setSelected(value: boolean): void;
  receive(value: RecordedMimoAnalysis | undefined): void;
  actionLabel(): string;
  run(): void;
  clear(): void;
}

export interface ThreePageMimoAnalysisContext {
  inputIds: readonly string[];
  recordings: readonly { id: string; label: string }[];
  onAnalyze: (recordingId: string, trials: readonly MimoSettledTrial[], options?: { maximumUnexcitedInputDrift?: number }) => void;
}

/** Unwrap the host completion envelope while preserving the retained result contract. */
export function decodeMimoAnalysisResult(payload: unknown): RecordedMimoAnalysis | undefined {
  if (!payload || typeof payload !== "object" || Array.isArray(payload)) return undefined;
  const value = payload as Record<string, unknown>;
  if (value.status === "failed") return { status: "rejected", reason: String(value.reason ?? "MIMO analysis failed") };
  const result = value.status === "complete" ? value.result : payload;
  if (!result || typeof result !== "object" || Array.isArray(result)) return undefined;
  const candidate = result as Record<string, unknown>;
  return candidate.status === "qualified" || candidate.status === "rejected" ? candidate as unknown as RecordedMimoAnalysis : undefined;
}

export function initializeThreePageMimoAnalysis(root: HTMLElement, initialContext?: ThreePageMimoAnalysisContext): ThreePageMimoAnalysisPresenter {
  const section = root.querySelector<HTMLElement>("[data-three-page-mimo-analysis]");
  if (!section) return { setContext: () => undefined, setSelected: () => undefined, receive: () => undefined, actionLabel: () => "Calculate measured paths", run: () => undefined, clear: () => undefined };
  const status = section.querySelector<HTMLElement>("[data-three-page-mimo-analysis-status]")!;
  const summary = section.querySelector<HTMLElement>("[data-three-page-mimo-analysis-summary]")!;
  const controls = section.querySelector<HTMLElement>("[data-three-page-mimo-analysis-controls]")!;
  const recording = section.querySelector<HTMLSelectElement>("[data-three-page-mimo-analysis-recording]")!;
  const windows = section.querySelector<HTMLTableSectionElement>("[data-three-page-mimo-analysis-windows] tbody")!;
  const drift = section.querySelector<HTMLInputElement>("[data-three-page-mimo-analysis-drift]")!;
  let context = initialContext;
  let evidence: RecordedMimoAnalysis | undefined;
  let selected = false;
  let draftWindows = new Map<string, string>();
  const renderControls = (): void => {
    controls.hidden = !context;
    if (!context) return;
    recording.replaceChildren(...context.recordings.map(item => { const option = document.createElement("option"); option.value = item.id; option.textContent = item.label; return option; }));
    windows.replaceChildren(...context.inputIds.map(inputId => {
      const row = document.createElement("tr"); const label = document.createElement("th"); label.textContent = inputId; row.append(label);
      for (const key of ["baseline-start", "baseline-end", "settled-start", "settled-end"] as const) {
        const cell = document.createElement("td"); const input = document.createElement("input"); input.type = "number"; input.step = "any"; input.dataset.mimoInput = inputId; input.dataset.mimoWindow = key; input.value = draftWindows.get(`${inputId}:${key}`) ?? ""; input.addEventListener("input", () => draftWindows.set(`${inputId}:${key}`, input.value)); cell.append(input); row.append(cell);
      }
      return row;
    }));
  };
  const runAnalysis = (): void => {
    if (!context || !recording.value) return;
    const trials: MimoSettledTrial[] = [];
    for (const inputId of context.inputIds) {
      const value = (key: string) => { const raw = Array.from(section.querySelectorAll<HTMLInputElement>("[data-mimo-input]")
        ).find(input => input.dataset.mimoInput === inputId && input.dataset.mimoWindow === key)?.value.trim() ?? ""; return raw ? Number(raw) : Number.NaN; };
      const trial = { inputId, baseline: { startSeconds: value("baseline-start"), endSeconds: value("baseline-end") }, settled: { startSeconds: value("settled-start"), endSeconds: value("settled-end") } };
      if (![trial.baseline.startSeconds, trial.baseline.endSeconds, trial.settled.startSeconds, trial.settled.endSeconds].every(Number.isFinite) || trial.baseline.startSeconds >= trial.baseline.endSeconds || trial.settled.startSeconds >= trial.settled.endSeconds) { status.textContent = "Enter finite, ordered baseline and settled windows for every declared input."; return; }
      trials.push(trial);
    }
    const rawDrift = drift.value.trim();
    if (rawDrift && (!Number.isFinite(Number(rawDrift)) || Number(rawDrift) < 0)) { status.textContent = "Enter a finite nonnegative drift tolerance or leave it blank."; return; }
    context.onAnalyze(recording.value, trials, rawDrift ? { maximumUnexcitedInputDrift: Number(rawDrift) } : undefined);
  };
  const clearResult = (): void => {
    summary.replaceChildren();
    section.querySelector<HTMLElement>("[data-three-page-mimo-analysis-gain]")?.replaceChildren();
    section.querySelector<HTMLElement>("[data-three-page-mimo-analysis-rga]")?.replaceChildren();
    section.querySelector<HTMLElement>("[data-three-page-mimo-analysis-decoupler]")?.replaceChildren();
    section.querySelector<HTMLElement>("[data-three-page-mimo-analysis-pairing]")?.replaceChildren();
  };
  const render = (value: RecordedMimoAnalysis | undefined): void => {
    section.hidden = !selected || (!value && !context);
    if (!value) { clearResult(); status.textContent = ""; renderControls(); return; }
    if (value.status === "rejected") { clearResult(); status.textContent = `MIMO analysis unavailable: ${rejectionText(value.reason)}`; renderControls(); return; }
    // A qualified fit has full-rank, well-conditioned input steps; poorer excitation is rejected before this point.
    status.textContent = `${value.sampleCount} good samples from ${value.inputs.length} independent input steps.`;
    summary.textContent = `${value.inputs.map(input => `${input.symbol} (${displayEngineeringUnit(input.unit)})`).join(" · ")} → ${value.outputs.map(output => `${output.symbol} (${displayEngineeringUnit(output.unit)})`).join(" · ")}`;
    renderMatrix(section.querySelector<HTMLElement>("[data-three-page-mimo-analysis-gain]")!, value.gainMatrix, value.outputIds, value.inputIds);
    renderMatrix(section.querySelector<HTMLElement>("[data-three-page-mimo-analysis-rga]")!, value.rga, value.outputIds, value.inputIds);
    renderMatrix(section.querySelector<HTMLElement>("[data-three-page-mimo-analysis-decoupler]")!, value.decoupler, value.inputIds, value.outputIds);
    const pairing = section.querySelector<HTMLElement>("[data-three-page-mimo-analysis-pairing]")!;
    pairing.textContent = `Pairing candidates: ${value.pairing.map(pair => `${pair.outputId} ← ${pair.inputId || "rejected"} (${pair.status})`).join(", ")}`;
    value.trials.forEach(trial => ["baseline-start", "baseline-end", "settled-start", "settled-end"].forEach(key => {
      const window = key.startsWith("baseline") ? trial.baseline : trial.settled; const value = key.endsWith("start") ? window.startSeconds : window.endSeconds;
      draftWindows.set(`${trial.inputId}:${key}`, String(value));
    }));
    renderControls();
  };
  const setContext = (value: ThreePageMimoAnalysisContext | undefined): void => { context = value; renderControls(); render(evidence); };
  const setSelected = (value: boolean): void => { selected = value; render(evidence); };
  renderControls();
  return { setContext, setSelected, receive: value => { evidence = value; render(value); }, actionLabel: () => "Calculate measured paths", run: runAnalysis, clear: () => { evidence = undefined; render(undefined); } };
}

/** Writes a matrix entry at the matrix's shared precision, keeping trailing zeros so that precision stays visible. */
function matrixEntryText(value: number, digits: number | undefined): string {
  if (digits === undefined || !Number.isFinite(value) || (value !== 0 && Math.abs(value) < 10 ** -digits)) return formatEngineeringNumber(value, digits);
  return value.toFixed(digits);
}

function renderMatrix(host: HTMLElement, matrix: readonly (readonly number[])[], rows: readonly string[], columns: readonly string[]): void {
  const table = document.createElement("table");
  const head = table.createTHead().insertRow();
  head.append(document.createElement("th"));
  columns.forEach(column => { const cell = document.createElement("th"); cell.textContent = column; head.append(cell); });
  const body = table.createTBody();
  // One precision per matrix, set by its smallest entry, so rows and columns can be read against each other.
  const magnitudes = matrix.flat().map(Math.abs).filter(value => Number.isFinite(value) && value > 0);
  const digits = magnitudes.length ? automaticFractionDigits(Math.min(...magnitudes)) : undefined;
  matrix.forEach((row, index) => { const tr = body.insertRow(); const label = tr.insertCell(); label.textContent = rows[index] ?? String(index + 1); row.forEach(value => { const cell = tr.insertCell(); cell.textContent = matrixEntryText(value, digits); }); });
  host.replaceChildren(table);
}

/** The measured-path analysis names its reason in plain words; an unrecognised reason is shown as reported. */
const REJECTION_TEXT: Readonly<Record<string, string>> = {
  "square-multichannel-binding-required": "Declare as many bound inputs as outputs.",
  "pairing-dimension-exceeds-bounded-analysis": "At most eight input/output pairs can be compared.",
  "one-settled-trial-per-input-required": "Enter one baseline and one settled window for each input.",
  "no-retained-samples": "The selected recording has no samples.",
  "insufficient-quality-valid-mimo-samples": "A window lies outside the recording or has no good sample of every channel.",
  "unexcited-input-drift": "An input that was not stepped moved during another input's trial.",
  "window-straddles-input-change": "A window contains the input step. Place the baseline before the step and the settled window after it.",
};
function rejectionText(reason: string): string { return REJECTION_TEXT[reason] ?? reason; }
