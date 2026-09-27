import { formatEngineeringNumber } from "../chartContract";

export type ParameterDescriptor = { symbol: string; unit?: string };
export type ParameterComparisonInput = {
  previous?: unknown;
  next?: unknown;
  parameterDescriptors?: unknown;
};

type ParameterRow = {
  symbol: string;
  unit?: string;
  current: unknown;
  reviewed: unknown;
};

export function renderParameterComparison(input: ParameterComparisonInput): HTMLElement[] {
  const rows = parameterRows(input);
  if (!rows.length) return [];
  const changed = rows.filter(row => !sameValue(row.current, row.reviewed));
  const unchanged = rows.filter(row => sameValue(row.current, row.reviewed));
  const result: HTMLElement[] = [];
  if (changed.length) result.push(parameterTable(changed));
  if (unchanged.length) {
    const details = document.createElement("details");
    const summary = document.createElement("summary");
    summary.textContent = `Unchanged settings (${unchanged.length})`;
    details.append(summary, parameterTable(unchanged));
    result.push(details);
  }
  return result;
}

export function renderHistoricalParameterComparison(input: ParameterComparisonInput): HTMLElement | undefined {
  const rows = renderParameterComparison(input);
  if (!rows.length) return undefined;
  const details = document.createElement("details");
  const summary = document.createElement("summary");
  summary.textContent = "Previous parameter comparison";
  details.append(summary, ...rows);
  return details;
}

export function renderObservedValues(value: unknown, parameterDescriptors?: unknown): HTMLElement | undefined {
  const entries = isRecord(value) ? Object.entries(value) : [["Observed", value] as [string, unknown]];
  if (!entries.length) return undefined;
  const table = document.createElement("table");
  table.className = "engineer-evidence-table";
  table.dataset.parameterObserved = "";
  const caption = document.createElement("caption");
  caption.className = "parameter-observed-caption";
  caption.textContent = "Observed values";
  const body = document.createElement("tbody");
  for (const [symbol, observed] of entries) {
    const row = document.createElement("tr");
    const heading = document.createElement("th");
    heading.scope = "row";
    const descriptor = Array.isArray(parameterDescriptors) ? parameterDescriptors.find(item => isDescriptor(item) && item.symbol === symbol) : undefined;
    heading.textContent = descriptor?.unit ? `${humanLabel(symbol)} (${descriptor.unit})` : humanLabel(symbol);
    heading.title = symbol;
    const cell = document.createElement("td");
    cell.textContent = formatValue(observed);
    row.append(heading, cell);
    body.append(row);
  }
  table.append(caption, body);
  return table;
}

function parameterRows(input: ParameterComparisonInput): ParameterRow[] {
  const previous = isRecord(input.previous) ? input.previous : {};
  const next = isRecord(input.next) ? input.next : {};
  const descriptors = Array.isArray(input.parameterDescriptors)
    ? input.parameterDescriptors.filter(isDescriptor)
    : [];
  const keys = [...new Set([...Object.keys(previous), ...Object.keys(next)])];
  const symbols = [...new Set([...descriptors.map(descriptor => descriptor.symbol).filter(symbol => keys.includes(symbol)), ...keys])];
  return symbols.map(symbol => {
    const descriptor = descriptors.find(item => item.symbol === symbol);
    return { symbol, unit: descriptor?.unit, current: previous[symbol], reviewed: next[symbol] };
  });
}

function parameterTable(rows: ParameterRow[]): HTMLElement {
  const table = document.createElement("table");
  table.className = "engineer-evidence-table";
  table.dataset.parameterComparison = "";
  const caption = document.createElement("caption");
  caption.className = "sr-only";
  caption.textContent = "Current / reviewed";
  const head = document.createElement("thead");
  const header = document.createElement("tr");
  header.append(columnHeading("Parameter"), columnHeading("Current"), columnHeading("Reviewed"));
  head.append(header);
  const body = document.createElement("tbody");
  for (const item of rows) {
    const row = document.createElement("tr");
    row.dataset.parameterSymbol = item.symbol;
    const label = item.unit ? `${humanLabel(item.symbol)} (${item.unit})` : humanLabel(item.symbol);
    const heading = document.createElement("th");
    heading.scope = "row";
    heading.textContent = label;
    heading.title = item.symbol;
    const current = document.createElement("td");
    current.textContent = formatValue(item.current);
    const reviewed = document.createElement("td");
    reviewed.textContent = formatValue(item.reviewed);
    row.append(heading, current, reviewed);
    body.append(row);
  }
  table.append(caption, head, body);
  return table;
}

function columnHeading(value: string): HTMLElement {
  const heading = document.createElement("th");
  heading.scope = "col";
  heading.textContent = value;
  return heading;
}

function humanLabel(symbol: string): string {
  const segment = symbol.split(/[.:/]/).filter(Boolean).pop() ?? symbol;
  return segment.replace(/([a-z0-9])([A-Z])/g, "$1 $2").replace(/[_-]+/g, " ");
}

function formatValue(value: unknown): string {
  return typeof value === "number" ? formatEngineeringNumber(value) : value === undefined || value === null ? "Unavailable" : String(value);
}

function sameValue(left: unknown, right: unknown): boolean {
  return Object.is(left, right) || (typeof left === "number" && typeof right === "number" && Number.isNaN(left) && Number.isNaN(right));
}

function isDescriptor(value: unknown): value is ParameterDescriptor {
  return isRecord(value) && typeof value.symbol === "string";
}

function isRecord(value: unknown): value is Record<string, unknown> {
  return Boolean(value && typeof value === "object" && !Array.isArray(value));
}
