import { automaticFractionDigits, formatEngineeringNumber } from "../chartContract";
import { displayEngineeringUnit } from "../unitDisplay";
import type { AnalysisEnvelope, AnalysisInputs, ToolOffer } from "../generated/AnalyzeAnswer";
import type { Failure } from "../generated/Failure";
import type { AnalysisTool } from "../generated/AnalyzeRequest";
import type { AnalysisPage } from "../generated/PageRequest";
import {
  ANALYSIS_KIND, answerOf, type AnalysisAnswers, type AnalysisUpdate, type EngineAnalysisEvents,
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
  actionLabel(): string;
  run(): void;
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
}

/**
 * The page's measured-paths section (specification 35, C7 and B9.9). A served loop's section draws the engine's `measuredPaths` answer
 * and sends the window entries it reads; it checks nothing and computes nothing.
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
  private readonly windows: HTMLTableSectionElement;
  private readonly drift: HTMLInputElement;
  private readonly refusalLine: HTMLElement;
  private readonly gainHost: HTMLElement;
  private readonly rgaHost: HTMLElement;
  private readonly decouplerHost: HTMLElement;
  private readonly pairingHost: HTMLElement;
  private readonly detailsHost: HTMLElement;
  private context: ThreePageMimoAnalysisContext | undefined;
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
    // The engine takes the study's last recording (C7), so the page names none: the template's recording choice stays hidden.
    section.querySelector<HTMLElement>("[data-three-page-mimo-analysis-recording-field]")!.hidden = true;
    this.windows = section.querySelector<HTMLTableSectionElement>("[data-three-page-mimo-analysis-windows] tbody")!;
    this.drift = section.querySelector<HTMLInputElement>("[data-three-page-mimo-analysis-drift]")!;
    // Text, as the windows are: the engine reads the tolerance and refuses what no number reads.
    this.drift.type = "text";
    this.drift.inputMode = "decimal";
    this.refusalLine = section.querySelector<HTMLElement>("[data-three-page-mimo-analysis-refusal]")!;
    this.gainHost = section.querySelector<HTMLElement>("[data-three-page-mimo-analysis-gain]")!;
    this.rgaHost = section.querySelector<HTMLElement>("[data-three-page-mimo-analysis-rga]")!;
    this.decouplerHost = section.querySelector<HTMLElement>("[data-three-page-mimo-analysis-decoupler]")!;
    this.pairingHost = section.querySelector<HTMLElement>("[data-three-page-mimo-analysis-pairing]")!;
    this.detailsHost = section.querySelector<HTMLElement>("[data-three-page-mimo-analysis-details]")!;
    this.context = initialContext;
    this.renderControls();
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

  actionLabel(): string {
    if (this.pending && this.offer?.operation) return this.offer.abortAction ?? "Abort analysis";
    return this.offer?.action ?? "Calculate measured paths";
  }

  run(): void {
    if (!this.context) return;
    // The Tune button aborts a running calculation as it starts one (B9.9): Abort sends the engine's cancel.
    if (this.pending) {
      if (this.pending.operationId && this.offer?.operation) this.post(ANALYSIS_KIND.cancel);
      return;
    }
    this.calculate(this.engineInputs());
  }

  label(): string | undefined { return this.offer?.label; }

  offered(): boolean { return this.offer !== undefined; }

  shown(): AnalysisPage | undefined {
    return this.selected && this.engineMode() ? { tool: MEASURED_PATHS, inputs: this.engineInputs() } : undefined;
  }

  tools(offers: readonly ToolOffer[] | undefined, failure: Failure | undefined, stale?: readonly AnalysisTool[]): void {
    this.offer = offers?.find(item => item.tool === MEASURED_PATHS);
    // The engine lists the answers that are no longer current (B9.9). The measured paths read a recording, not the model, so a model change
    // does not list them; the section drops its answer only when the engine does.
    if (stale?.includes(MEASURED_PATHS)) { this.engineAnswer = undefined; this.notice = undefined; }
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
    else if (update.state === "cancelled") { this.refusal = undefined; this.notice = update.message; }
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

  /** A loop with a context is the engine's: its measured paths are the engine's answer (C7). */
  private engineMode(): boolean {
    return this.context !== undefined;
  }

  /** Shows what the engine's events changed, and tells the page so that the Tune button names the state (Abort while running). */
  private refresh(): void {
    this.render();
    this.changed(this.selected);
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

  /**
   * The inputs the engine calculates from: one settled trial for each input, with each window end as the engineer typed it (a blank end is
   * left out), and the drift tolerance as typed when one is entered. The engine reads each entry and names what is missing or unreadable (C7).
   */
  private engineInputs(): AnalysisInputs {
    const settledTrials: NonNullable<AnalysisInputs["settledTrials"]> = [];
    for (const inputId of this.context?.inputIds ?? []) {
      const entry = (key: string) => this.windowText(inputId, key) || undefined;
      const baselineStart = entry("baseline-start"), baselineEnd = entry("baseline-end");
      const settledStart = entry("settled-start"), settledEnd = entry("settled-end");
      settledTrials.push({ inputId, baseline: { startSeconds: baselineStart, endSeconds: baselineEnd },
        settled: { startSeconds: settledStart, endSeconds: settledEnd } });
    }
    const tolerance = this.drift.value.trim();
    return { settledTrials, ...(tolerance ? { maximumUnexcitedInputDrift: tolerance } : {}) };
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
      this.draftWindows.set(`${trial.inputId}:baseline-start`, String(trial.baseline.startSeconds ?? ""));
      this.draftWindows.set(`${trial.inputId}:baseline-end`, String(trial.baseline.endSeconds ?? ""));
      this.draftWindows.set(`${trial.inputId}:settled-start`, String(trial.settled.startSeconds ?? ""));
      this.draftWindows.set(`${trial.inputId}:settled-end`, String(trial.settled.endSeconds ?? ""));
    }
    const tolerance = inputs.maximumUnexcitedInputDrift;
    this.drift.value = tolerance === undefined || tolerance === null ? "" : String(tolerance);
    this.renderControls();
  }

  private renderControls(): void {
    const context = this.context;
    this.controls.hidden = !context;
    if (!context) return;
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
      // Text, so that an entry no number reads reaches the engine, which refuses it under this field (C7).
      input.type = "text";
      input.inputMode = "decimal";
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
  private render(): void {
    this.section.hidden = !this.selected || !this.context;
    this.renderControls();
    this.status.textContent = this.pending ? (this.pending.progress ?? "Calculating measured paths…") : (this.notice ?? "");
    this.refusalLine.hidden = this.refusal === undefined;
    this.refusalLine.textContent = this.refusal ? (this.refusal.detail ?? this.refusal.message) : "";
    this.markRefusedField();
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

  /** Marks the window that the engine's refusal names (`field`: `<inputId>:<window key>`, C7); every other window is unmarked. */
  private markRefusedField(): void {
    Array.from(this.section.querySelectorAll<HTMLInputElement>("[data-mimo-input][data-mimo-window]")).forEach(input => {
      const refused = `${input.dataset.mimoInput}:${input.dataset.mimoWindow}` === this.refusal?.field;
      if (refused) input.setAttribute("aria-invalid", "true");
      else input.removeAttribute("aria-invalid");
      input.classList.toggle("trust-input--error", refused);
    });
  }

  private clearAnswer(): void {
    this.summary.replaceChildren();
    this.gainHost.replaceChildren();
    this.rgaHost.replaceChildren();
    this.decouplerHost.replaceChildren();
    this.pairingHost.replaceChildren();
    this.detailsHost.replaceChildren();
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
    tools(offers, failure, stale) {
      measured.tools(offers, failure, stale);
      analysis.tools(offers, failure, stale);
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

/** A section that is not in the page: it takes every message and shows nothing. */
function inertPresenter(): ThreePageMimoAnalysisPresenter {
  return {
    setContext: () => undefined, setSelected: () => undefined, actionLabel: () => "Calculate measured paths",
    run: () => undefined, label: () => undefined, offered: () => false, shown: () => undefined,
    tools: () => undefined, started: () => undefined, answer: () => undefined, refused: () => undefined, update: () => undefined, restore: () => undefined,
  };
}
