import type { Failure } from "../generated/Failure";
import type { PlotSeries, ValidateResult, ValidateRow, ValidationPlot } from "../generated/ValidateResult";
import type { EngineHostPorts } from "./engineOpened";
import { record } from "./engineShared";
import type { EngineStageReceiver } from "./engineStages";
import type { SignalResponseGroup } from "./threePageLiveSignals";

/**
 * Shows the engine's Validate answer on the Validate page (specification 35, B5.4, B9.6, B9.9). The engine decides every row, word,
 * value, verdict and plot, and it puts the rows in their order and counts the passing guards. This module writes the answer's rows
 * into the table in that order, draws the engine's plot on the chart, passes the engine's Apply answer to the page's stage state (which
 * gates Apply and shows a refusal with its next action, B5.5), and hides the table when a stage change leaves the validation no longer
 * current (A8.4). It computes none of the words, numbers or verdicts.
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
  followStageChanges(root);
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

/**
 * The table's rows (B9.9): the engine's rows in the order it gives them, one `<tr>` each. The engine names each row, writes its
 * measured value, limit and verdict, puts the failing rows first, counts the passing guards in one row and closes the table with the
 * candidate's overall row. The table is hidden when the answer has no rows.
 */
function renderValidation(root: HTMLElement, answer: ValidateResult): void {
  const table = root.querySelector<HTMLTableElement>("[data-studio-validation-result]");
  if (!table) return;
  const doc = root.ownerDocument;
  const rows = (answer.table ?? []).map(row => validateRow(doc, row));
  table.tBodies[0].replaceChildren(...rows);
  table.hidden = rows.length === 0;
  showPlantReason(root, table, answer.plantReason);
}

/**
 * One row as the engine gives it (B9.9). Its title is the engine's name for the row, with the plant it ran against shown under the title
 * (B5.4); the symbol is the hover text, and the row keeps the engine's identity for it (`data-studio-requirement`).
 */
function validateRow(doc: Document, row: ValidateRow): HTMLTableRowElement {
  const element = doc.createElement("tr");
  element.dataset.studioRequirement = row.requirement;
  // The full symbol is the hover text; the row names the channel by its role (B9.9).
  element.title = row.symbol;
  const title = cell(doc, row.title);
  if (row.source) {
    const source = doc.createElement("small");
    source.textContent = row.source;
    title.append(doc.createElement("br"), source);
  }
  element.append(title, cell(doc, row.measured), cell(doc, row.limit), verdictCell(doc, row.verdictText, row.verdictClass));
  return element;
}

function cell(doc: Document, text: string): HTMLTableCellElement {
  const element = doc.createElement("td");
  element.textContent = text;
  return element;
}

/** A verdict keeps the engine's words as the text; the icon and colour come from the engine's verdict class, which the page styles. */
function verdictCell(doc: Document, text: string, verdictClass: string): HTMLTableCellElement {
  const element = cell(doc, text);
  if (verdictClass === "pass" || verdictClass === "fail" || verdictClass === "inconclusive") {
    element.classList.add("studio-verdict");
    element.dataset.verdict = verdictClass;
  }
  return element;
}

/** The roots whose stage changes the Validate table already follows, so that each page adds one listener (see `followStageChanges`). */
const followedRoots = new WeakSet<HTMLElement>();

/**
 * Follows the page's stage changes (`trust-control-studio-engineer`, threePageHostState.ts dispatches every engineer payload as it
 * reaches the page). A payload whose stage evidence holds no current validation result hides the table and drops its rows; a reset drops
 * the plant reason with them (A8.4). The engine's evidence says whether a result is current (`validationHasResult`), and the page decides
 * nothing. The listener is added when the first result is shown, because the table holds rows only after a result.
 */
function followStageChanges(root: HTMLElement): void {
  const view = root.ownerDocument.defaultView;
  if (!view || followedRoots.has(root)) return;
  followedRoots.add(root);
  view.addEventListener("trust-control-studio-engineer", event => {
    const payload = record((event as CustomEvent).detail);
    if (payload) gateValidation(root, payload);
  });
}

function gateValidation(root: HTMLElement, payload: Record<string, unknown>): void {
  const reset = payload.status === "reset";
  // A change the engine says invalidated the validation, or evidence that names no current result, hides the table; a payload with neither leaves it as it is.
  const invalidated = payload.validationInvalidated === true || record(payload.stageEvidence)?.validationHasResult === false;
  if (!reset && !invalidated) return;
  const table = root.querySelector<HTMLTableElement>("[data-studio-validation-result]");
  if (!table) return;
  table.tBodies[0].replaceChildren();
  table.hidden = true;
  // A reset drops the plant reason with the results it explained (A8.4).
  if (reset) root.querySelector<HTMLElement>("[data-studio-plant-reason]")?.remove();
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
