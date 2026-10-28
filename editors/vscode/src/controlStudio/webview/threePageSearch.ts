import type { SearchAnswer } from "../generated/SearchAnswer";
import type { SearchCandidate } from "../generated/SearchCandidate";
import type { SearchTrial } from "../generated/SearchTrial";
import type { LoopDefinition } from "../generated/OpenResult";
import { displayEngineeringUnit, formatEngineeringNumber } from "../chartContract";

/**
 * The finite search as the Tune area shows it (specification 35, C5): the engine's answer, or the kept candidate a reopened study holds.
 * A reopened candidate has no status and no sentence.
 */
export type SearchView = Partial<Pick<SearchAnswer, "message" | "selected" | "status">> & Pick<SearchCandidate, "scoreUnit" | "trials">;

/** The words of a trial's bounds outcome (C5). A trial that was not attempted has none. */
const BOUNDS_WORDS: Readonly<Record<string, string>> = { met: "Met", notMet: "Not met", missingEvidence: "Missing evidence" };

/**
 * Shows a finite search in the Tune area (C5): the engine's sentence when the search did not complete, then one row per trial in the
 * engine's order. A row gives the trial's coefficient values, whether it was attempted (with its reason when it was refused), its bounds
 * outcome and J in its unit, and it marks the selected trial. The engine decides every value; the page only lays them out.
 */
export function renderSearchResult(root: HTMLElement, loop: LoopDefinition, search: SearchView): void {
  const area = root.querySelector<HTMLElement>("[data-studio-tuning-result]")!;
  const owner = area.ownerDocument;
  const title = owner.createElement("p");
  title.textContent = `Finite search · ${search.trials.length} ${search.trials.length === 1 ? "trial" : "trials"}`;
  area.replaceChildren(title);
  if (search.status !== undefined && search.status !== "completed" && typeof search.message === "string") {
    const sentence = owner.createElement("p");
    sentence.dataset.studioSearchStatus = search.status;
    sentence.textContent = search.message;
    area.append(sentence);
  }
  area.append(trialTable(owner, loop, search));
  area.hidden = false;
}

/** The trials as one table: a column per coefficient the trials set, then the attempt, the bounds outcome and J. */
function trialTable(owner: Document, loop: LoopDefinition, search: SearchView): HTMLTableElement {
  const symbols = coefficientSymbols(search.trials);
  const table = owner.createElement("table");
  table.className = "trust-table";
  table.dataset.studioSearchTrials = "";
  const names = ["Trial", ...symbols.map(symbol => parameterLabel(loop, symbol)), "Attempted", "Bounds", `J (${search.scoreUnit})`];
  const header = table.createTHead().insertRow();
  for (const name of names) {
    const heading = owner.createElement("th");
    heading.textContent = name;
    header.append(heading);
  }
  const body = table.createTBody();
  search.trials.forEach((trial, index) => body.append(trialRow(owner, trial, index, symbols, search.selected)));
  return table;
}

function trialRow(owner: Document, trial: SearchTrial, index: number, symbols: readonly string[], selected: number | null | undefined): HTMLTableRowElement {
  const row = owner.createElement("tr");
  row.dataset.studioSearchTrial = String(index + 1);
  const isSelected = index === selected;
  if (isSelected) row.dataset.studioSearchSelected = "";
  row.append(
    cellWith(owner, isSelected ? `${index + 1} · selected` : String(index + 1)),
    ...symbols.map(symbol => cellWith(owner, numberText(trial.parameters[symbol]))),
    attemptCell(owner, trial),
    cellWith(owner, trial.bounds ? BOUNDS_WORDS[trial.bounds] : "—"),
    cellWith(owner, numberText(trial.score)),
  );
  return row;
}

/** Whether the trial was attempted; a trial refused before the search started names its reason beneath. */
function attemptCell(owner: Document, trial: SearchTrial): HTMLTableCellElement {
  const cell = cellWith(owner, trial.attempted ? "Attempted" : "Not attempted");
  if (trial.reason) {
    const reason = owner.createElement("span");
    reason.dataset.studioSearchReason = "";
    reason.textContent = trial.reason;
    cell.append(owner.createElement("br"), reason);
  }
  return cell;
}

function cellWith(owner: Document, text: string): HTMLTableCellElement {
  const cell = owner.createElement("td");
  cell.textContent = text;
  return cell;
}

function numberText(value: number | null | undefined): string {
  return typeof value === "number" ? formatEngineeringNumber(value) : "—";
}

/** The coefficient symbols the trials set, in the order they first appear. */
function coefficientSymbols(trials: readonly SearchTrial[]): string[] {
  return Array.from(new Set(trials.flatMap(trial => Object.keys(trial.parameters))));
}

/** A coefficient's name as the page shows it: its symbol, with the unit the loop declares for it. */
function parameterLabel(loop: LoopDefinition, symbol: string): string {
  const unit = loop.parameterBinding?.parameters.find(parameter => parameter.symbol === symbol)?.unit;
  return unit ? `${symbol} (${displayEngineeringUnit(unit)})` : symbol;
}
