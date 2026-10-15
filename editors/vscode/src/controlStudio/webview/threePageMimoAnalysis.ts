import type { RecordedMimoAnalysis } from "../threePageMimoAnalysis";
import type { MimoSettledTrial } from "../threePageMimoAnalysis";
import { automaticFractionDigits, formatEngineeringNumber } from "../chartContract";
import { displayEngineeringUnit } from "../unitDisplay";
import type { AnalysisEnvelope, AnalysisInputs, ToolOffer } from "../generated/AnalyzeAnswer";
import type { Failure } from "../generated/Failure";
import type { AnalysisTool } from "../generated/AnalyzeRequest";
import type { AnalysisPage } from "../generated/PageRequest";
import {
  ANALYSIS_KIND, ENGINE_MODEL_CHANGED_EVENT, answerOf, type AnalysisAnswers, type AnalysisUpdate, type EngineAnalysisEvents,
} from "./engineAnalysis";

/** The engine's measured-paths tool (C7): a served loop's measured MIMO paths are its answer, with its window rules and sentences. */
const MEASURED_PATHS = "measuredPaths" as const;

/** The answer of `measuredPaths`: the gain matrix, the relative gain array, the static decoupler, the pairing and the sentences. */
type MeasuredPathsAnswer = AnalysisAnswers["measuredPaths"];

/** The page's post of an analysis message to the host (`control-studio.analysis.*`). */
type MimoPost = (kind: string, payload?: Record<string, unknown>) => void;

export interface ThreePageMimoAnalysisPresenter extends EngineAnalysisEvents {
  setContext(value: ThreePageMimoAnalysisContext | undefined): void;
  setSelected(value: boolean): void;
  /** Takes a parked loop's TypeScript result (V7.17 debt). A served loop's answer arrives through the engine events. */
  receive(value: RecordedMimoAnalysis | undefined): void;
  actionLabel(): string;
  run(): void;
  clear(): void;
  isSelected(): boolean;
  /** The engine's label of the measured paths, when the engine offers them. */
  label(): string | undefined;
  /** Whether the engine offers the measured paths for the loop. */
  offered(): boolean;
  /** The analysis the section shows with the inputs it holds, for the page's record (`controlStudio/page`, B9.9). */
  shown(): AnalysisPage | undefined;
}

export interface ThreePageMimoAnalysisContext {
  /** The loop's declared inputs: one row of settled windows for each. */
  inputIds: readonly string[];
  /** Parked loops only (V7.17 debt): the retained recordings the TypeScript route analyses. */
  recordings?: readonly { id: string; label: string }[];
  /**
   * Parked loops only (V7.17 debt): the TypeScript route's calculation. A served loop has none; the engine calculates the study's
   * last kept recording (C7).
   */
  onAnalyze?: (recordingId: string, trials: readonly MimoSettledTrial[], options?: { maximumUnexcitedInputDrift?: number }) => void;
}

/** Unwrap the host completion envelope while preserving the retained result contract (V7.17 debt: a parked loop's route only). */
export function decodeMimoAnalysisResult(payload: unknown): RecordedMimoAnalysis | undefined {
  if (!payload || typeof payload !== "object" || Array.isArray(payload)) return undefined;
  const value = payload as Record<string, unknown>;
  if (value.status === "failed") return { status: "rejected", reason: String(value.reason ?? "MIMO analysis failed") };
  const result = value.status === "complete" ? value.result : payload;
  if (!result || typeof result !== "object" || Array.isArray(result)) return undefined;
  const candidate = result as Record<string, unknown>;
  return candidate.status === "qualified" || candidate.status === "rejected" ? candidate as unknown as RecordedMimoAnalysis : undefined;
}

/**
 * The page's measured-paths section (specification 35, C7 and B9.9). A served loop's section draws the engine's `measuredPaths` answer
 * and sends the window entries it reads; it checks nothing and computes nothing. A parked loop keeps the TypeScript route, marked V7.17.
 */
export function initializeThreePageMimoAnalysis(root: HTMLElement, initialContext?: ThreePageMimoAnalysisContext,
  post: MimoPost = () => undefined, changed: (selected: boolean) => void = () => undefined): ThreePageMimoAnalysisPresenter {
  const section = root.querySelector<HTMLElement>("[data-three-page-mimo-analysis]");
  return section ? new MimoAnalysisSection(section, initialContext, post, changed) : inertPresenter();
}

class MimoAnalysisSection implements ThreePageMimoAnalysisPresenter {
  private readonly status: HTMLElement;
  private readonly summary: HTMLElement;
  private readonly controls: HTMLElement;
  private readonly recordingField: HTMLElement;
  private readonly recording: HTMLSelectElement;
  private readonly windows: HTMLTableSectionElement;
  private readonly drift: HTMLInputElement;
  private readonly refusalLine: HTMLElement;
  private readonly gainHost: HTMLElement;
  private readonly rgaHost: HTMLElement;
  private readonly decouplerHost: HTMLElement;
  private readonly pairingHost: HTMLElement;
  private readonly detailsHost: HTMLElement;
  private context: ThreePageMimoAnalysisContext | undefined;
  /** Parked loops only (V7.17 debt): the TypeScript route's result. */
  private evidence: RecordedMimoAnalysis | undefined;
  private selected = false;
  private draftWindows = new Map<string, string>();
  /** The engine's side (C7): the offer, the running operation, the refusal and the answer of the measured paths. */
  private offer: ToolOffer | undefined;
  private pending: { operationId?: string; progress?: string } | undefined;
  private refusal: Failure | undefined;
  private notice: string | undefined;
  private engineAnswer: MeasuredPathsAnswer | undefined;

  constructor(private readonly section: HTMLElement, initialContext: ThreePageMimoAnalysisContext | undefined,
    private readonly post: MimoPost, private readonly changed: (selected: boolean) => void) {
    this.status = section.querySelector<HTMLElement>("[data-three-page-mimo-analysis-status]")!;
    this.summary = section.querySelector<HTMLElement>("[data-three-page-mimo-analysis-summary]")!;
    this.controls = section.querySelector<HTMLElement>("[data-three-page-mimo-analysis-controls]")!;
    this.recordingField = section.querySelector<HTMLElement>("[data-three-page-mimo-analysis-recording-field]")!;
    this.recording = section.querySelector<HTMLSelectElement>("[data-three-page-mimo-analysis-recording]")!;
    this.windows = section.querySelector<HTMLTableSectionElement>("[data-three-page-mimo-analysis-windows] tbody")!;
    this.drift = section.querySelector<HTMLInputElement>("[data-three-page-mimo-analysis-drift]")!;
    this.refusalLine = section.querySelector<HTMLElement>("[data-three-page-mimo-analysis-refusal]")!;
    this.gainHost = section.querySelector<HTMLElement>("[data-three-page-mimo-analysis-gain]")!;
    this.rgaHost = section.querySelector<HTMLElement>("[data-three-page-mimo-analysis-rga]")!;
    this.decouplerHost = section.querySelector<HTMLElement>("[data-three-page-mimo-analysis-decoupler]")!;
    this.pairingHost = section.querySelector<HTMLElement>("[data-three-page-mimo-analysis-pairing]")!;
    this.detailsHost = section.querySelector<HTMLElement>("[data-three-page-mimo-analysis-details]")!;
    this.context = initialContext;
    this.renderControls();
    // A model change clears the answer calculated for the model before it (B9.9); the engine's tool list follows the new model.
    window.addEventListener(ENGINE_MODEL_CHANGED_EVENT, () => this.modelChanged());
  }

  setContext(value: ThreePageMimoAnalysisContext | undefined): void {
    this.context = value;
    this.renderControls();
    this.render();
  }

  setSelected(value: boolean): void {
    const was = this.selected;
    this.selected = value;
    if (value !== was) { this.refusal = undefined; this.notice = undefined; }
    this.render();
    // The page tells the host which analysis it shows, so that reopening shows it again (B9.9).
    if (value !== was && this.engineMode()) this.post(ANALYSIS_KIND.show, value ? { tool: MEASURED_PATHS, inputs: this.engineInputs() } : {});
  }

  receive(value: RecordedMimoAnalysis | undefined): void {
    this.evidence = value;
    this.render();
  }

  actionLabel(): string {
    if (!this.engineMode()) return "Calculate measured paths";
    if (this.pending && this.offer?.operation) return this.offer.abortAction ?? "Abort analysis";
    return this.offer?.action ?? "Calculate measured paths";
  }

  run(): void {
    if (!this.context) return;
    if (!this.engineMode()) { this.runParked(); return; }
    // The Tune button aborts a running calculation as it starts one (B9.9): Abort sends the engine's cancel.
    if (this.pending) {
      if (this.pending.operationId && this.offer?.operation) this.post(ANALYSIS_KIND.cancel);
      return;
    }
    this.calculate(this.engineInputs());
  }

  clear(): void {
    this.evidence = undefined;
    this.engineAnswer = undefined;
    this.refusal = undefined;
    this.notice = undefined;
    this.pending = undefined;
    this.render();
  }

  isSelected(): boolean { return this.selected; }

  label(): string | undefined { return this.offer?.label; }

  offered(): boolean { return this.offer !== undefined; }

  shown(): AnalysisPage | undefined {
    return this.selected && this.engineMode() ? { tool: MEASURED_PATHS, inputs: this.engineInputs() } : undefined;
  }

  tools(offers: readonly ToolOffer[] | undefined, failure: Failure | undefined): void {
    this.offer = offers?.find(item => item.tool === MEASURED_PATHS);
    if (failure && this.selected) this.refusal = failure;
    this.refresh();
  }

  started(tool: AnalysisTool, operationId: string): void {
    if (tool !== MEASURED_PATHS) return;
    this.pending = { operationId };
    this.refresh();
  }

  answer(envelope: AnalysisEnvelope): void {
    if (envelope.tool === MEASURED_PATHS) this.receiveAnswer(envelope);
  }

  refused(tool: AnalysisTool | undefined, failure: Failure): void {
    if (tool !== undefined && tool !== MEASURED_PATHS) return;
    if (tool === MEASURED_PATHS) { this.pending = undefined; this.engineAnswer = undefined; }
    this.refusal = failure;
    this.notice = undefined;
    this.refresh();
  }

  update(update: AnalysisUpdate): void {
    if (update.state === "running") {
      this.pending = { ...this.pending, operationId: update.operationId, ...(update.progress ? { progress: update.progress } : {}) };
      this.refresh();
      return;
    }
    this.pending = undefined;
    if (update.state === "finished" && update.envelope) { this.receiveAnswer(update.envelope); return; }
    if (update.state === "failed") { this.engineAnswer = undefined; this.refusal = update.failure; }
    else if (update.state === "cancelled") { this.refusal = undefined; this.notice = "Analysis cancelled."; }
    this.refresh();
  }

  /** The inputs the engine recorded for the analysis (A8.6) fill the windows, and the analysis is calculated again. */
  restore(tool: AnalysisTool, inputs: AnalysisInputs): void {
    if (tool !== MEASURED_PATHS || !this.context) return;
    this.fillFields(inputs);
    this.selected = true;
    this.refusal = undefined;
    this.notice = undefined;
    if (!this.pending && this.engineMode()) this.calculate(inputs);
    this.refresh();
  }

  /** A loop with a context and no TypeScript route is the engine's: its measured paths are the engine's answer (C7). */
  private engineMode(): boolean {
    return this.context !== undefined && this.context.onAnalyze === undefined;
  }

  /** Shows what the engine's events changed, and tells the page so that the Tune button names the state (Abort while running). */
  private refresh(): void {
    this.render();
    this.changed(this.selected);
  }

  private render(): void {
    if (this.engineMode()) this.renderEngine();
    else this.renderParked(this.evidence);
  }

  /** Asks the engine to calculate the measured paths with these inputs; the section shows them until the engine's answer comes. */
  private calculate(inputs: AnalysisInputs): void {
    this.pending = {};
    this.refusal = undefined;
    this.notice = undefined;
    this.engineAnswer = undefined;
    this.render();
    this.post(ANALYSIS_KIND.run, { tool: MEASURED_PATHS, inputs });
  }

  private receiveAnswer(envelope: AnalysisEnvelope): void {
    this.pending = undefined;
    this.notice = undefined;
    this.refusal = envelope.failure ?? undefined;
    this.engineAnswer = envelope.failure ? undefined : answerOf(envelope.result ?? undefined, MEASURED_PATHS);
    this.refresh();
  }

  private modelChanged(): void {
    this.engineAnswer = undefined;
    this.refusal = undefined;
    this.notice = undefined;
    this.render();
  }

  /**
   * The inputs the engine calculates from: one settled trial for each input whose four windows are all entered, and the drift tolerance
   * when one is entered. The engine checks the windows and the tolerance and names what is wrong (C7); an incomplete trial is not sent.
   */
  private engineInputs(): AnalysisInputs {
    const settledTrials: NonNullable<AnalysisInputs["settledTrials"]> = [];
    for (const inputId of this.context?.inputIds ?? []) {
      const entry = (key: string) => numberField(this.windowText(inputId, key));
      const baselineStart = entry("baseline-start"), baselineEnd = entry("baseline-end");
      const settledStart = entry("settled-start"), settledEnd = entry("settled-end");
      if (baselineStart === undefined || baselineEnd === undefined || settledStart === undefined || settledEnd === undefined) continue;
      settledTrials.push({ inputId, baseline: { startSeconds: baselineStart, endSeconds: baselineEnd },
        settled: { startSeconds: settledStart, endSeconds: settledEnd } });
    }
    const tolerance = numberField(this.drift.value.trim());
    return { settledTrials, ...(tolerance === undefined ? {} : { maximumUnexcitedInputDrift: tolerance }) };
  }

  /** The text typed in one window of one input's row. */
  private windowText(inputId: string, key: string): string {
    const entry = Array.from(this.section.querySelectorAll<HTMLInputElement>("[data-mimo-input]"))
      .find(input => input.dataset.mimoInput === inputId && input.dataset.mimoWindow === key);
    return entry?.value.trim() ?? "";
  }

  private fillFields(inputs: AnalysisInputs): void {
    this.draftWindows = new Map<string, string>();
    for (const trial of inputs.settledTrials ?? []) {
      this.draftWindows.set(`${trial.inputId}:baseline-start`, String(trial.baseline.startSeconds));
      this.draftWindows.set(`${trial.inputId}:baseline-end`, String(trial.baseline.endSeconds));
      this.draftWindows.set(`${trial.inputId}:settled-start`, String(trial.settled.startSeconds));
      this.draftWindows.set(`${trial.inputId}:settled-end`, String(trial.settled.endSeconds));
    }
    const tolerance = inputs.maximumUnexcitedInputDrift;
    this.drift.value = tolerance === undefined || tolerance === null ? "" : String(tolerance);
    this.renderControls();
  }

  private renderControls(): void {
    const context = this.context;
    this.controls.hidden = !context;
    if (!context) return;
    this.recordingField.hidden = context.onAnalyze === undefined;
    this.recording.replaceChildren(...(context.recordings ?? []).map(item => {
      const option = document.createElement("option"); option.value = item.id; option.textContent = item.label; return option;
    }));
    this.windows.replaceChildren(...context.inputIds.map(inputId => this.windowRow(inputId)));
  }

  private windowRow(inputId: string): HTMLTableRowElement {
    const row = document.createElement("tr");
    const label = document.createElement("th");
    label.textContent = inputId;
    row.append(label);
    for (const key of ["baseline-start", "baseline-end", "settled-start", "settled-end"] as const) {
      const cell = document.createElement("td");
      const input = document.createElement("input");
      input.type = "number";
      input.step = "any";
      input.dataset.mimoInput = inputId;
      input.dataset.mimoWindow = key;
      input.value = this.draftWindows.get(`${inputId}:${key}`) ?? "";
      input.addEventListener("input", () => this.draftWindows.set(`${inputId}:${key}`, input.value));
      cell.append(input);
      row.append(cell);
    }
    return row;
  }

  /**
   * The engine's answer. Each sentence is shown as the engine wrote it; the matrices take one precision each, and the labels are the
   * channels' symbols and units. A refusal's sentence is shown under the windows and the tolerance it concerns.
   */
  private renderEngine(): void {
    this.section.hidden = !this.selected || !this.context;
    this.renderControls();
    this.status.textContent = this.pending ? (this.pending.progress ?? "Calculating measured paths…") : (this.notice ?? "");
    this.refusalLine.hidden = this.refusal === undefined;
    this.refusalLine.textContent = this.refusal ? (this.refusal.detail ?? this.refusal.message) : "";
    const answer = this.engineAnswer;
    if (!answer) { this.clearAnswer(); return; }
    this.summary.replaceChildren(...answer.summary.map(paragraph));
    const outputs = answer.outputs.map(channelName), inputs = answer.inputs.map(channelName);
    renderMatrix(this.gainHost, answer.gainMatrix, outputs, inputs);
    renderMatrix(this.rgaHost, answer.relativeGainArray, outputs, inputs);
    renderMatrix(this.decouplerHost, answer.decoupler, inputs, outputs);
    this.pairingHost.textContent = pairingText(answer);
    this.detailsHost.replaceChildren(...answer.details.map(paragraph));
  }

  /** V7.17 debt: a parked loop's result, calculated by the TypeScript route. A served loop's section never shows it. */
  private renderParked(value: RecordedMimoAnalysis | undefined): void {
    this.section.hidden = !this.selected || (!value && !this.context);
    if (!value) { this.clearAnswer(); this.status.textContent = ""; this.renderControls(); return; }
    if (value.status === "rejected") { this.clearAnswer(); this.status.textContent = `MIMO analysis unavailable: ${rejectionText(value.reason)}`; this.renderControls(); return; }
    // A qualified fit has full-rank, well-conditioned input steps; poorer excitation is rejected before this point.
    this.status.textContent = `${value.sampleCount} good samples from ${value.inputs.length} independent input steps.`;
    this.summary.textContent = `${value.inputs.map(channelName).join(" · ")} → ${value.outputs.map(channelName).join(" · ")}`;
    renderMatrix(this.gainHost, value.gainMatrix, value.outputIds, value.inputIds);
    renderMatrix(this.rgaHost, value.rga, value.outputIds, value.inputIds);
    renderMatrix(this.decouplerHost, value.decoupler, value.inputIds, value.outputIds);
    this.pairingHost.textContent = `Pairing candidates: ${value.pairing.map(pair => `${pair.outputId} ← ${pair.inputId || "rejected"} (${pair.status})`).join(", ")}`;
    for (const trial of value.trials) {
      for (const key of ["baseline-start", "baseline-end", "settled-start", "settled-end"]) {
        const span = key.startsWith("baseline") ? trial.baseline : trial.settled;
        this.draftWindows.set(`${trial.inputId}:${key}`, String(key.endsWith("start") ? span.startSeconds : span.endSeconds));
      }
    }
    this.renderControls();
  }

  private clearAnswer(): void {
    this.summary.replaceChildren();
    this.gainHost.replaceChildren();
    this.rgaHost.replaceChildren();
    this.decouplerHost.replaceChildren();
    this.pairingHost.replaceChildren();
    this.detailsHost.replaceChildren();
  }

  /**
   * V7.17 debt: a parked loop's calculation on the TypeScript route, with its window checks and its sentences. A served loop's
   * calculation is the engine's (`run`), and its checks are the engine's too.
   */
  private runParked(): void {
    const analyze = this.context?.onAnalyze;
    if (!this.context || !analyze || !this.recording.value) return;
    const trials: MimoSettledTrial[] = [];
    for (const inputId of this.context.inputIds) {
      const value = (key: string) => {
        const raw = this.windowText(inputId, key);
        return raw ? Number(raw) : Number.NaN;
      };
      const trial = { inputId, baseline: { startSeconds: value("baseline-start"), endSeconds: value("baseline-end") }, settled: { startSeconds: value("settled-start"), endSeconds: value("settled-end") } };
      if (![trial.baseline.startSeconds, trial.baseline.endSeconds, trial.settled.startSeconds, trial.settled.endSeconds].every(Number.isFinite) || trial.baseline.startSeconds >= trial.baseline.endSeconds || trial.settled.startSeconds >= trial.settled.endSeconds) {
        this.status.textContent = "Enter finite, ordered baseline and settled windows for every declared input.";
        return;
      }
      trials.push(trial);
    }
    const rawDrift = this.drift.value.trim();
    if (rawDrift && (!Number.isFinite(Number(rawDrift)) || Number(rawDrift) < 0)) {
      this.status.textContent = "Enter a finite nonnegative drift tolerance or leave it blank.";
      return;
    }
    analyze(this.recording.value, trials, rawDrift ? { maximumUnexcitedInputDrift: Number(rawDrift) } : undefined);
  }
}

/**
 * Routes the engine's analysis messages (B9.9): the `measuredPaths` tool, its operations and its answers, to the measured-paths section;
 * every other tool to the analysis presenter. An update names no tool, so its operation is recognised by the `started` message that
 * named the measured paths, or by its finished envelope.
 */
export function routeMeasuredPaths(analysis: EngineAnalysisEvents, measured: EngineAnalysisEvents): EngineAnalysisEvents {
  const operations = new Set<string>();
  const owner = (tool: AnalysisTool): EngineAnalysisEvents => (tool === MEASURED_PATHS ? measured : analysis);
  return {
    tools(offers, failure) {
      measured.tools(offers, failure);
      analysis.tools(offers, failure);
    },
    started(tool, operationId) {
      if (tool === MEASURED_PATHS) operations.add(operationId);
      owner(tool).started(tool, operationId);
    },
    answer(envelope) {
      owner(envelope.tool).answer(envelope);
    },
    refused(tool, failure) {
      if (tool === undefined) { measured.refused(tool, failure); analysis.refused(tool, failure); return; }
      owner(tool).refused(tool, failure);
    },
    update(update) {
      const measuredOperation = operations.has(update.operationId) || update.envelope?.tool === MEASURED_PATHS;
      if (update.state !== "running") operations.delete(update.operationId);
      (measuredOperation ? measured : analysis).update(update);
    },
    restore(tool, inputs) {
      owner(tool).restore(tool, inputs);
    },
  };
}

/** A number typed in a window or tolerance field: a blank or unreadable entry is none. */
function numberField(text: string): number | undefined {
  if (!text.trim()) return undefined;
  const value = Number(text);
  return Number.isFinite(value) ? value : undefined;
}

/** A channel as its symbol and unit, in a matrix's header and in a summary. */
function channelName(channel: { symbol: string; unit: string }): string {
  return `${channel.symbol} (${displayEngineeringUnit(channel.unit)})`;
}

/** The named pairs of the relative gain array, or the engine's sentence of its refusal (N4.5). */
function pairingText(answer: MeasuredPathsAnswer): string {
  const pairing = answer.pairing;
  if (pairing.status !== "named") return pairing.reason ?? "No pairing is named.";
  const names = new Map([...answer.outputs, ...answer.inputs].map(channel => [channel.id, channel.symbol] as const));
  const pairs = pairing.pairs.map(pair => `${names.get(pair.outputId) ?? pair.outputId} ← ${names.get(pair.inputId) ?? pair.inputId} (relative gain ${formatEngineeringNumber(pair.relativeGain)})`);
  return `Pairing: ${pairs.join(", ")}`;
}

function paragraph(text: string): HTMLParagraphElement {
  const element = document.createElement("p");
  element.textContent = text;
  return element;
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

/** V7.17 debt: the reasons of a parked loop's TypeScript route, in plain words; an unrecognised reason is shown as reported. */
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

/** A section that is not in the page: it takes every message and shows nothing. */
function inertPresenter(): ThreePageMimoAnalysisPresenter {
  return {
    setContext: () => undefined, setSelected: () => undefined, receive: () => undefined, actionLabel: () => "Calculate measured paths",
    run: () => undefined, clear: () => undefined, isSelected: () => false, label: () => undefined, offered: () => false, shown: () => undefined,
    tools: () => undefined, started: () => undefined, answer: () => undefined, refused: () => undefined, update: () => undefined, restore: () => undefined,
  };
}
