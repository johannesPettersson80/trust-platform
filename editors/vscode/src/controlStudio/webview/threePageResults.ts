import type { LoopDefinition } from "../generated/OpenResult";
import { displayEngineeringUnit, formatEngineeringNumber } from "../chartContract";
import { formatMatrixRows } from "./numberListText";
import { familyLabelOf } from "./openedOffers";
import { structureResultView } from "./structureResults";

export function renderTunedController(root: HTMLElement, loop: LoopDefinition, payload: Record<string, unknown>): void {
  if (payload.status !== "candidate" || !payload.values || typeof payload.values !== "object") return;
  const values = payload.values as Record<string, number>;
  const area = root.querySelector<HTMLElement>("[data-studio-tuning-result]")!;
  const family = String(payload.family ?? loop.controller.type);
  // The five structures the engine serves answer their own section (structureResults.ts, C3, C6, C7, C9, C10); the rows below serve the PID path.
  const served = structureResultView(loop, payload);
  const words = payload.words as TuneWordsView | undefined;
  const title = document.createElement("p"); title.textContent = served ? served.title : `${familyLabelOf(family)} controller · ${loop.controller.type}`;
  // The engine's description of the method is read on hover, so the page carries no permanent text for it (B9.1).
  if (typeof payload.methodDescription === "string") title.title = payload.methodDescription;
  const list = document.createElement("dl"); list.className = "studio-setting-values";
  // A Keep toggle keeps the calculated value fixed for the next tuning run; it uses the pins that Tune already sends.
  const pinned = (payload.settingsPins as { values?: Record<string, number> } | undefined)?.values ?? {};
  // The PID path's rows are the engine's setting rows (TuneWords.settingLabels, tune_words.rs), in its order, each with its label. A served
  // structure's section replaces them, so none of its values passes through this loop.
  for (const setting of served ? [] : settingRows(words)) {
    if (!Number.isFinite(values[setting.symbol])) continue;
    // The row's unit is the engine's (SettingLabel.unit, tune_words.rs): a row without one is shown without a unit.
    const label = document.createElement("dt"); label.textContent = setting.label;
    const value = document.createElement("dd"); value.textContent = `${formatEngineeringNumber(values[setting.symbol])}${unitSuffix(setting.unit ?? "")}`;
    value.dataset.studioParameter = setting.symbol;
    const keepLabel = document.createElement("label"); keepLabel.className = "studio-keep"; keepLabel.title = "Keep this value fixed for the next tuning run";
    const keep = document.createElement("input"); keep.type = "checkbox"; keep.dataset.studioKeep = setting.symbol; keep.checked = Number.isFinite(pinned[setting.symbol]);
    keepLabel.append(keep, " Keep");
    const row = document.createElement("div"); row.append(label, value, keepLabel);
    list.append(row);
  }
  area.replaceChildren(title, ...(served ? served.elements : [list]));
  const margins = payload.robustness as CandidateMarginsView | undefined;
  if (margins) {
    // A margin the engine names in words (unbounded, or not observed in the range) is shown as those words (B9.5, TuneWords).
    const text = (value: number | string | undefined, unit: string) => typeof value === "number" ? `${formatEngineeringNumber(value)}${unit}` : value ?? "—";
    const line = document.createElement("p"); line.dataset.studioRobustness = "";
    line.textContent = `Gain margin ${text(margins.gainMarginDb, " dB")} · phase margin ${text(margins.phaseMarginDegrees, "°")}`
      + ` · delay margin ${text(margins.delayMarginSeconds, " s")} · peak sensitivity Ms ${formatEngineeringNumber(margins.peakSensitivity)}`;
    area.append(line);
  }
  // The engine's outcome against the mandatory requirements, in its own sentence (TuneWords.outcome, B4.4, B4.5).
  const outcome = payload.outcome as { status?: unknown } | undefined;
  const sentence = outcomeText(words);
  if (outcome && sentence !== undefined) {
    const line = document.createElement("p"); line.dataset.studioOutcome = String(outcome.status); line.textContent = sentence;
    area.append(line);
  }
  // The closed-loop poles of a state law (C4), on the sampled model (N7.11), as the engine gives them.
  const state = payload.state as StateAnswerView | undefined;
  if (state && Array.isArray(state.closedLoopPoles) && state.closedLoopPoles.length > 0) {
    const poles = document.createElement("p"); poles.dataset.studioClosedLoopPoles = "";
    poles.textContent = `Closed-loop poles: ${state.closedLoopPoles.map(poleText).join(" · ")}`;
    area.append(poles);
  }
  if (Array.isArray(payload.unavailable)) for (const note of payload.unavailable) {
    const line = document.createElement("p"); line.dataset.studioTuneNote = ""; line.textContent = String(note); area.append(line);
  }
  if (Array.isArray(payload.comparison) && payload.comparison.length > 0) area.append(comparisonTable(payload.comparison));
  // The Tune answer's feedforward is the one design the PID path draws beside its rows; a group's and a schedule's are the structure views' (structureResults.ts).
  const design = payload.design as { feedforward?: FeedforwardAnswerView } | undefined;
  if (design?.feedforward) area.append(...feedforwardDesignView(loop, design.feedforward));
  const horizon = state?.horizon;
  if (horizon) {
    // The line that names the horizon's steps and policy, and the heading of its stages, are the engine's words (TuneWords.horizonPolicy, horizonStages).
    const policy = document.createElement("p"); policy.dataset.studioDesignPolicy = "finite";
    policy.textContent = typeof words?.horizonPolicy === "string" ? words.horizonPolicy : "";
    const details = document.createElement("details"); const summary = document.createElement("summary");
    summary.textContent = typeof words?.horizonStages === "string" ? words.horizonStages : "";
    details.append(summary);
    const table = document.createElement("table"); table.className = "trust-table";
    const header = table.createTHead().insertRow();
    for (const name of ["Stage", "Gain K", "Cost P"]) { const th = document.createElement("th"); th.textContent = name; header.append(th); }
    const body = table.createTBody();
    const matrix = (value: number[][] | undefined) => value ? formatMatrixRows(value) : "—";
    horizon.riccatiSequence.forEach((cost, stage) => { const row = body.insertRow(); row.dataset.studioRiccatiStage = String(stage);
      for (const value of [String(stage), matrix(horizon.gainSequence[stage]), matrix(cost)]) row.insertCell().textContent = value;
    });
    details.append(table); area.append(policy, details);
  }
  area.hidden = false;
}

/** The engine's state answer as the page reads it (C4): the closed-loop poles and the finite-horizon stages. */
interface StateAnswerView {
  closedLoopPoles?: { real: number; imaginary: number }[];
  horizon?: { steps: number; policy: string; gainSequence: number[][][]; riccatiSequence: number[][][] };
}

/** A closed-loop pole in words: its real part, and its imaginary part with its sign and `i` when the pole is not real. */
function poleText(pole: { real: number; imaginary: number }): string {
  if (pole.imaginary === 0) return formatEngineeringNumber(pole.real);
  return `${formatEngineeringNumber(pole.real)} ${pole.imaginary < 0 ? "−" : "+"} ${formatEngineeringNumber(Math.abs(pole.imaginary))}i`;
}

/** The candidate's margins as the page holds them (engineTune.ts): each number, or the engine's word for a margin that is not one (B9.5). */
interface CandidateMarginsView {
  gainMarginDb?: number | string;
  phaseMarginDegrees?: number | string;
  delayMarginSeconds?: number | string;
  peakSensitivity: number;
}

/** The engine's words of a Tune answer as the page reads them (TuneWords, tune_words.rs). */
interface TuneWordsView { outcome?: unknown; settingLabels?: unknown; horizonPolicy?: unknown; horizonStages?: unknown }
/** One setting row as the engine gives it: the setting's symbol, the label of its row and its unit (SettingLabel, tune_words.rs). */
interface SettingLabelView { symbol: string; label: string; unit?: string | null }

/** The setting rows of the PID path, in the engine's order (TuneWords.settingLabels). */
function settingRows(words: TuneWordsView | undefined): SettingLabelView[] {
  const settings = words?.settingLabels;
  return Array.isArray(settings) ? settings as SettingLabelView[] : [];
}

/** The outcome sentence in the engine's words (TuneWords.outcome); the page composes no sentence of its own (B4.4, B4.5). */
function outcomeText(words: TuneWordsView | undefined): string | undefined {
  const sentence = words?.outcome;
  return typeof sentence === "string" ? sentence : undefined;
}

/** The constrained comparison around the SIMC candidate (N5.10): each candidate's structure, gains and outcome, as the engine gives them. */
function comparisonTable(entries: unknown[]): HTMLTableElement {
  const table = document.createElement("table"); table.className = "trust-table"; table.dataset.studioComparison = "";
  const header = table.createTHead().insertRow();
  for (const name of ["Candidate", "Structure", "Kp", "Ki", "Kd", "Outcome"]) { const th = document.createElement("th"); th.textContent = name; header.append(th); }
  const body = table.createTBody();
  for (const item of entries) {
    const entry = item as { label?: unknown; structure?: unknown; gains?: Record<string, number>; outcomeText?: unknown };
    const gains = entry.gains ?? {};
    const row = body.insertRow(); row.dataset.studioComparisonStructure = String(entry.structure);
    // The outcome's sentence is the engine's (ComparisonEntry.outcomeText, messages.rs); the page composes none.
    const outcome = typeof entry.outcomeText === "string" ? entry.outcomeText : "";
    // A gain the answer does not hold is shown as none, never as zero (the engine's candidate holds all three).
    for (const text of [String(entry.label), String(entry.structure), gainText(gains.proportionalGain),
      gainText(gains.integralGain), gainText(gains.derivativeGain), outcome]) row.insertCell().textContent = text;
  }
  return table;
}

/** A feedforward path as the engine answers it: its gain with the unit of it, its time constant and its dead time (`FeedforwardPath`, messages.rs). */
interface FeedforwardPathView { gain: number; gainUnit?: string | null; timeConstantSeconds: number; delaySeconds: number }
/** The engine's answer of a measured-disturbance feedforward (C8, `FeedforwardAnswer`), in the names of its JSON, with the unit of each value it states. */
interface FeedforwardAnswerView {
  outputPath: FeedforwardPathView; disturbancePath: FeedforwardPathView;
  gain: number; gainUnit?: string | null; leadSeconds?: number; lagSeconds?: number;
  disturbanceStep: number; disturbanceUnit?: string | null; measurementUnit?: string | null; feedbackDeviation: number; feedforwardDeviation: number;
}

/**
 * Specification 35 §17.12: both paths, the feedforward values and the two largest deviations after the disturbance step, as the engine answers
 * them. Every value comes with the engine's unit; the page composes none. The signals' symbols name the rows, as the loop declares them.
 */
function feedforwardDesignView(loop: LoopDefinition, design: FeedforwardAnswerView): HTMLElement[] {
  const measurement = loop.signals.processVariable, output = loop.signals.manipulatedVariable;
  const disturbance = loop.signals.additional?.find(signal => signal.role === "disturbance");
  const table = document.createElement("table"); table.className = "trust-table"; table.dataset.studioFeedforwardDesign = "";
  const header = table.createTHead().insertRow();
  for (const name of [`Path to ${measurement.symbol}`, "Gain K", "Time constant τ", "Delay θ"]) { const th = document.createElement("th"); th.textContent = name; header.append(th); }
  const body = table.createTBody();
  for (const [label, path] of [[`Controller output ${output.symbol}`, design.outputPath], [`Disturbance ${disturbance?.symbol ?? ""}`, design.disturbancePath]] as const) {
    const row = body.insertRow();
    for (const text of [label, `${formatEngineeringNumber(path.gain)}${unitSuffix(path.gainUnit ?? "")}`,
      `${formatEngineeringNumber(path.timeConstantSeconds)} s`, `${formatEngineeringNumber(path.delaySeconds)} s`]) row.insertCell().textContent = text;
  }
  // K_ff is in the engine's unit; the lead and the lag are the program's times, in seconds.
  const values = document.createElement("p"); values.dataset.studioFeedforwardValues = "";
  values.textContent = `Feedforward gain K_ff ${formatEngineeringNumber(design.gain)}${unitSuffix(design.gainUnit ?? "")}`
    + (typeof design.leadSeconds === "number" && typeof design.lagSeconds === "number"
      ? ` · lead time T_lead ${formatEngineeringNumber(design.leadSeconds)} s · lag time T_lag ${formatEngineeringNumber(design.lagSeconds)} s` : "");
  const deviation = document.createElement("p"); deviation.dataset.studioFeedforwardDeviation = "";
  const unit = unitSuffix(design.measurementUnit ?? "");
  deviation.textContent = `Largest deviation after a ${formatEngineeringNumber(design.disturbanceStep)}${unitSuffix(design.disturbanceUnit ?? "")} disturbance step, on the model: `
    + `${formatEngineeringNumber(design.feedbackDeviation)}${unit} with the PI alone, ${formatEngineeringNumber(design.feedforwardDeviation)}${unit} with feedforward`;
  return [table, values, deviation];
}

/** A unit after a value, as the engine gives it: none for a number without a unit (`1`) and for a unit the engine gave none. */
function unitSuffix(unit: string): string { return unit === "" || unit === "1" ? "" : ` ${displayEngineeringUnit(unit)}`; }

/** A gain of the comparison, or a dash when the answer holds none: a gain is never shown as zero for want of a value. */
function gainText(value: number | undefined): string {
  return typeof value === "number" && Number.isFinite(value) ? formatEngineeringNumber(value) : "—";
}
