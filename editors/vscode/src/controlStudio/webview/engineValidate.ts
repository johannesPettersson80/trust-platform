import type { Failure } from "../generated/Failure";
import type { PlotSeries, RequirementOutcome, ValidateResult, ValidationPlot, ValueRange } from "../generated/ValidateResult";
import { displayEngineeringUnit, formatEngineeringNumber } from "../chartContract";
import type { EngineHostPorts } from "./engineOpened";
import { record } from "./engineShared";
import type { EngineStageReceiver } from "./engineStages";
import type { SignalResponseGroup } from "./threePageLiveSignals";

/**
 * Shows the engine's Validate answer on the Validate page (specification 35, B5.4, B9.6, B9.9). The engine decides every verdict,
 * label, value, range and plot; this module lays the answer's rows out in the table, formats numbers with their units, draws the
 * engine's plot on the chart, and passes the engine's Apply answer to the page's stage state, which gates Apply and shows a refusal
 * with its next action (B5.5).
 */
export const validateReceiver: EngineStageReceiver = {
  /**
   * A reopened loop shows the last Validate result the study holds while it is current (A8.4, A8.6): the same table, and whether Apply
   * is available. The study keeps no plot, so a reopened loop shows none (B5.4).
   */
  opened(root, opened, ports) {
    const validation = record(opened.validation);
    if (validation) showResult(root, validation, ports);
  },
  result(root, result, ports) {
    showResult(root, result, ports);
  },
};

/** One Validate result: its table, its plot and Apply's answer, which the page's engineer stage state takes. */
function showResult(root: HTMLElement, result: Record<string, unknown>, ports: EngineHostPorts): void {
  const answer = result as unknown as ValidateResult;
  renderValidation(root, answer);
  showValidationPlot(root, answer.plot);
  relayApplyAnswer(answer, ports);
}

/**
 * Apply is available only when the engine says so (B5.4); a refused Apply shows the engine's reason and next action (B5.5). The page's
 * gates take the answer's stage evidence, which the engine computed for it, whether the validation passed or not (B9.4).
 */
function relayApplyAnswer(answer: ValidateResult, ports: EngineHostPorts): void {
  if (answer.applyAvailable) {
    ports.receive("engineer", { status: "verified", stageEvidence: answer.stageEvidence });
    return;
  }
  ports.receive("engineer", { status: "verification-failed", stageEvidence: answer.stageEvidence, ...(answer.failure ? { failure: pageFailure(answer.failure) } : {}) });
}

/** The page's failure shape (webview/studioFailure.ts), from the engine's failure with the words its next action has. */
function pageFailure(failure: Failure): Record<string, unknown> {
  return {
    message: failure.message,
    ...(failure.detail ? { detail: failure.detail } : {}),
    nextAction: { kind: failure.nextAction, label: failure.nextActionLabel ?? "" },
  };
}

/** The verdicts in the order the table shows them: failures first, then inconclusive rows, then passing rows (B9.9). */
const VERDICT_ORDER: Record<RequirementOutcome["status"], number> = { Fail: 0, Inconclusive: 1, Pass: 2 };

/**
 * A requirement row of the table, with the words its run is named by (the scenario and the run), if any. The independent model
 * check is the first row of the table unless it fails (B5.4), so it is marked; `rank` puts it first.
 */
interface RequirementEntry {
  readonly requirement: RequirementOutcome;
  readonly prefix: string;
  readonly modelAgreement: boolean;
}

/**
 * The rows of the requirement table (B5.2 to B5.4, B9.9): the independent model check, each scenario's current settings and candidate
 * runs, and the Manual-to-Auto entry of an Auto commissioning. Each requirement shows its label, its channel by role, its measured
 * value or range, its limit or declared bounds and its source. The model check stays first unless it fails; the other rows that fail
 * come first, then the inconclusive ones, then the passing ones, each group in the engine's order. The passing guards (`guard`) are
 * counted in one row beneath them, and the candidate's overall result closes the table. The words come from the engine's `labels`;
 * the verdicts and the guard flags are the engine's.
 */
function renderValidation(root: HTMLElement, answer: ValidateResult): void {
  const table = root.querySelector<HTMLTableElement>("[data-studio-validation-result]");
  if (!table) return;
  const doc = root.ownerDocument;
  const labels = answer.labels;
  const scenarios = Array.isArray(answer.scenarios) ? answer.scenarios : [];
  const entries: RequirementEntry[] = [];
  if (answer.modelAgreement) entries.push({ requirement: answer.modelAgreement, prefix: "", modelAgreement: true });
  for (const scenario of scenarios) {
    const scenarioPrefix = scenarios.length > 1 ? `${scenario.label} · ` : "";
    for (const [run, runName] of [[scenario.current, labels.currentSettings], [scenario.candidate, labels.candidate]] as const) {
      for (const requirement of run.requirements) entries.push({ requirement, prefix: `${scenarioPrefix}${runName} · `, modelAgreement: false });
    }
  }
  // The Manual-to-Auto rows carry the engine's words for the case, as a scenario's rows carry their run's words (B9.9).
  const initialization = answer.initialization;
  for (const requirement of initialization?.requirements ?? []) entries.push({ requirement, prefix: `${initialization?.label ?? ""} · `, modelAgreement: false });
  const passingGuards = entries.filter(({ requirement }) => isPassingGuard(requirement)).length;
  const rows = entries
    .filter(({ requirement }) => !isPassingGuard(requirement))
    .sort((left, right) => rank(left) - rank(right))
    .map(({ requirement, prefix }) => requirementRow(doc, prefix, requirement, labels.automatic));
  if (passingGuards > 0) rows.push(guardRow(doc, labels.guards, labels.guardsLimit, passingGuards));
  if (scenarios.length > 0) rows.push(overallRow(doc, labels.candidateOverall, answer.status));
  table.tBodies[0].replaceChildren(...rows);
  table.hidden = rows.length === 0;
  showPlantReason(root, table, answer.plantReason);
}

/** A generated check that passed: the engine counts it in the guard row, not in a row of its own (B9.9). */
function isPassingGuard(requirement: RequirementOutcome): boolean {
  return requirement.guard === true && requirement.status === "Pass";
}

/** The place of a row in the table: the model check first unless it fails, then the verdicts in `VERDICT_ORDER`. */
function rank(entry: RequirementEntry): number {
  return entry.modelAgreement && entry.requirement.status !== "Fail" ? -1 : VERDICT_ORDER[entry.requirement.status];
}

/**
 * The passing guards as one row (B9.9): the engine's words for the row, the number of checks it counts, and Pass. The row has no
 * channel and no single limit; its limit column is the engine's words that the counted checks stayed within their declared limits.
 */
function guardRow(doc: Document, label: string, limit: string, count: number): HTMLTableRowElement {
  const row = doc.createElement("tr");
  row.dataset.studioRequirement = "limit-and-controller-checks";
  row.append(cell(doc, label), cell(doc, `${count} ${count === 1 ? "check" : "checks"}`), cell(doc, limit), verdictCell(doc, "Pass"));
  return row;
}

/**
 * One requirement as a row. Its title is its name, marked Automatic when Automatic generated its limit (B4.3, B5.4), with its
 * channel named by its role and unit; its source, the plant it ran against, is shown under the title (B5.4).
 */
function requirementRow(doc: Document, prefix: string, requirement: RequirementOutcome, automaticWord: string): HTMLTableRowElement {
  const row = doc.createElement("tr");
  row.dataset.studioRequirement = requirement.id;
  // The full symbol is the hover text; the row names the channel by its role (B9.9).
  row.title = requirement.channel.symbol;
  const mark = requirement.automatic ? ` · ${automaticWord}` : "";
  const title = cell(doc, `${prefix}${requirement.label}${mark}${channelText(requirement)}`);
  if (requirement.source) {
    const source = doc.createElement("small");
    source.textContent = requirement.source;
    title.append(doc.createElement("br"), source);
  }
  row.append(title, cell(doc, measuredText(requirement)), cell(doc, limitText(requirement)), verdictCell(doc, requirement.status, requirement.reason));
  return row;
}

/** The overall row: the candidate's result, which decides Apply (N8.5, B5.4); it has no measured value and no limit. */
function overallRow(doc: Document, label: string, status: ValidateResult["status"]): HTMLTableRowElement {
  const row = doc.createElement("tr");
  row.dataset.studioRequirement = "overall";
  row.append(cell(doc, label), cell(doc, "—"), cell(doc, "—"), verdictCell(doc, status));
  return row;
}

/** The channel's role with its unit in parentheses, for example "Measurement (°C)"; the symbol is never shown (B9.9). */
function channelText(requirement: RequirementOutcome): string {
  const { role, unit } = requirement.channel;
  return role ? ` · ${role}${unitParentheses(unit)}` : "";
}

/**
 * The measured value of a check. An operating-range or state-range check shows the measured range of its channel, from the
 * lowest to the highest good value (B5.4); "—" when no good value was measured.
 */
function measuredText(requirement: RequirementOutcome): string {
  const unit = unitSuffix(requirement.unit);
  if (requirement.measuredRange) return rangeText(requirement.measuredRange, unit);
  if (requirement.limitRange) return "—";
  return typeof requirement.value === "number" ? `${formatEngineeringNumber(requirement.value)}${unit}` : "—";
}

/** The limit of a check: its declared bounds for an operating-range or state-range check (B5.4), or else its one-sided limit. */
function limitText(requirement: RequirementOutcome): string {
  const unit = unitSuffix(requirement.unit);
  const bounds = requirement.limitRange;
  if (bounds) return `${bounds.minimum === bounds.maximum ? "= " : ""}${rangeText(bounds, unit)}`;
  if (typeof requirement.limit !== "number") return "—";
  const side = requirement.limitSide === "atLeast" ? "≥" : "≤";
  return `${side} ${formatEngineeringNumber(requirement.limit)}${unit}`;
}

/** A range as one number when its ends are equal, and as "lowest–highest" otherwise, with its unit. */
function rangeText(range: ValueRange, unit: string): string {
  return range.minimum === range.maximum
    ? `${formatEngineeringNumber(range.minimum)}${unit}`
    : `${formatEngineeringNumber(range.minimum)}–${formatEngineeringNumber(range.maximum)}${unit}`;
}

function unitSuffix(unit: string): string {
  return unit.trim() && unit.trim() !== "1" ? ` ${displayEngineeringUnit(unit)}` : "";
}

function unitParentheses(unit: string): string {
  return unit.trim() && unit.trim() !== "1" ? ` (${displayEngineeringUnit(unit)})` : "";
}

function cell(doc: Document, text: string): HTMLTableCellElement {
  const element = doc.createElement("td");
  element.textContent = text;
  return element;
}

/** A verdict keeps its word as the text, with the engine's reason when it is not a pass; the icon and colour come from the verdict attribute. */
function verdictCell(doc: Document, status: string, reason?: string): HTMLTableCellElement {
  const element = cell(doc, status !== "Pass" && reason ? `${status} · ${reason}` : status);
  const verdict = status.toLowerCase();
  if (verdict === "pass" || verdict === "fail" || verdict === "inconclusive") {
    element.classList.add("studio-verdict");
    element.dataset.verdict = verdict;
  }
  return element;
}

/** Why the model replaced the configured plant for every case (B5.3), in the engine's words, above the table's scroll region. */
function showPlantReason(root: HTMLElement, table: HTMLTableElement, reason: string | null | undefined): void {
  const existing = root.querySelector<HTMLElement>("[data-studio-plant-reason]");
  if (!reason) {
    existing?.remove();
    return;
  }
  const note: HTMLElement = existing ?? root.ownerDocument.createElement("p");
  note.dataset.studioPlantReason = "";
  note.textContent = reason;
  if (!existing) (table.parentElement ?? table).before(note);
}

/**
 * Hands the engine's Validate plot to the chart, which reads the plot of the engineer's events (B5.4, B9.6). A plot the engine
 * does not give is withdrawn, so that a result with no samples shows no older curve.
 */
function showValidationPlot(root: HTMLElement, plot: ValidationPlot | null | undefined): void {
  const view = root.ownerDocument.defaultView;
  if (!view) return;
  view.dispatchEvent(new view.CustomEvent("trust-control-studio-engineer", { detail: { validationPlot: plot ? measuredPlot(plot) : null } }));
}

/**
 * The engine's plot in the chart's shape (B9.6): the measured output as one group, with the candidate's run and the current
 * settings' run as its two series, each on its own time base. The names and the title are the engine's.
 */
export function measuredPlot(plot: ValidationPlot): { title: string; groups: SignalResponseGroup[] } {
  const series = (run: PlotSeries): SignalResponseGroup["series"][number] => ({
    name: run.name,
    points: run.timeSeconds.map((time, index) => [time, run.values[index] ?? null]),
  });
  return {
    title: plot.title,
    groups: [{
      name: plot.role,
      unit: plot.unit,
      timeStart: 0,
      series: [series(plot.tuned), ...(plot.current ? [{ ...series(plot.current), dashed: true }] : [])],
    }],
  };
}
