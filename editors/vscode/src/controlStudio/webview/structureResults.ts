/**
 * The Tune result of the five structures the engine serves and that no lesson uses (specification 35, C3, C6, C7, C9, C10, B4.5): a
 * cascade's two PIs, a Smith predictor's PI and model terms, a gain schedule's table, an ON/OFF controller's settings and a PID group's
 * members and decoupler. Each value is the engine's answer, each gain's name is the engine's (`words.gainLabels`), each channel of a group is
 * named by the engine (`inputName`, `outputName`, `inputNames`), and each symbol and unit is the loop's declared one. This module computes no
 * value. It picks the section to draw by what the answer and the loop declare: a group and a schedule by the answer's `design`, and a cascade,
 * a Smith predictor and an ON/OFF controller by the loop's own `controller` declaration (`structureResultView`). The PID path keeps its rows in
 * threePageResults.ts.
 */
import type { LoopDefinition } from "../generated/OpenResult";
import { displayEngineeringUnit, formatEngineeringNumber } from "../chartContract";

/** A cascade controller as its seed names it (C6): its instance path and the variables of its gains. */
interface CascadeGainsView { instance: string; proportionalSymbol: string; integralSymbol: string }

/** The fields of a `controller.project` seed that this module reads: a cascade's two controllers (C6), a Smith predictor's variables (C9). */
interface SeedView {
  kind: string;
  inner?: CascadeGainsView;
  outer?: CascadeGainsView;
  proportionalSymbol?: string;
  integralSymbol?: string;
  gainSymbol?: string;
  timeConstantSymbol?: string;
  delaySymbol?: string;
  operatingInputSymbol?: string;
  operatingOutputSymbol?: string;
}

/** A PID group member's design as the engine answers it (C7, `GroupMemberDesign`), with the names of its channels (`inputName`, `outputName`). */
interface GroupMemberView {
  instance: string;
  inputChannelId: string;
  outputChannelId: string;
  inputName?: string | null;
  outputName?: string | null;
  relativeGain: number;
  proportionalSymbol: string;
  integralSymbol: string;
  gains: { proportionalGain: number; integralGain: number };
}

/** A PID group's answer (C7, `GroupCandidate`): its members in the group's order and, when the program applies one, its decoupler. */
interface GroupView {
  members: GroupMemberView[];
  decoupler?: { matrix: number[][]; inputChannelIds: string[]; inputNames?: string[] | null; memberInstances: string[] };
}

/** One operating point of a gain schedule as the engine answers it (C10), in the table's order. */
interface SchedulePointView {
  name: string;
  value: number;
  proportionalSymbol: string;
  integralSymbol: string;
  proportionalGain: number;
  integralGain: number;
}

/** A gain schedule's answer (C10): the engine's choice and sentence, the scheduling variable and every point. */
interface ScheduleView {
  choice: "fixed" | "scheduled";
  reason: string;
  scheduling: { symbol: string; unit: string };
  points: SchedulePointView[];
}

/** The names of the three gains, as the engine words them in the Tune answer (`words.gainLabels`). */
interface GainLabelsView { proportional: string; integral: string; derivative: string }

/** The section of a Tune result that one of the five structures shows: its title and the elements of its body. */
export interface StructureResultView { title: string; elements: HTMLElement[] }

/**
 * The section of a Tune result for a loop of one of the five structures, or undefined for every other loop and answer. The answer's
 * group (`design.group`) and schedule (`design.schedule`) are read from the payload; a cascade, a Smith predictor and an ON/OFF controller
 * are read from the loop's own declaration, whose symbols name the values the answer holds.
 */
export function structureResultView(loop: LoopDefinition, payload: Record<string, unknown>): StructureResultView | undefined {
  const values = numbersOf(payload.values);
  const design = recordOf(payload.design);
  const labels = gainLabelsOf(payload);
  const group = design?.group as GroupView | undefined;
  if (group) return { title: "PID group", elements: groupElements(loop, group, labels) };
  const schedule = design?.schedule as ScheduleView | undefined;
  if (schedule) return { title: "Gain schedule", elements: scheduleElements(loop, schedule, labels) };
  const seed = loop.controller.project?.seed as unknown as SeedView | undefined;
  if (seed?.kind === "cascade-pid" && seed.inner && seed.outer) {
    return { title: "Cascade", elements: cascadeElements(loop, seed.inner, seed.outer, values, labels) };
  }
  if (seed?.kind === "smith-pid") return { title: "Smith predictor", elements: smithElements(loop, seed, values, labels) };
  if (loop.controller.onOff) return { title: "ON/OFF controller", elements: onOffElements(loop, loop.controller.onOff as OnOffView, values) };
  return undefined;
}

/** The gain names that the engine gives with the Tune answer (`words.gainLabels`, C6, C7, C9, C10); a missing name reads as blank. */
function gainLabelsOf(payload: Record<string, unknown>): GainLabelsView {
  const gains = recordOf(recordOf(payload.words)?.gainLabels);
  return { proportional: String(gains?.proportional ?? ""), integral: String(gains?.integral ?? ""), derivative: String(gains?.derivative ?? "") };
}

/** Specification 35 C6: the inner controller first, then the outer, each with its instance and its proportional and integral gain. */
function cascadeElements(loop: LoopDefinition, inner: CascadeGainsView, outer: CascadeGainsView, values: Numbers, labels: GainLabelsView): HTMLElement[] {
  const element = tableWith(["Controller", labels.proportional, labels.integral], "data-studio-cascade-design");
  const body = element.createTBody();
  for (const [role, gains] of [["Inner", inner], ["Outer", outer]] as const) {
    const row = body.insertRow();
    row.dataset.studioCascadeController = role.toLowerCase();
    row.insertCell().textContent = `${role} · ${gains.instance}`;
    row.insertCell().textContent = valueText(values[gains.proportionalSymbol], unitOf(loop, gains.proportionalSymbol));
    row.insertCell().textContent = valueText(values[gains.integralSymbol], unitOf(loop, gains.integralSymbol));
  }
  return [element];
}

/** Specification 35 C9: the PI, the model's gain, time constant and delay, and the operating point when the library block is used. */
function smithElements(loop: LoopDefinition, seed: SeedView, values: Numbers, labels: GainLabelsView): HTMLElement[] {
  const list = settingList("data-studio-smith-design");
  const rows: [string, string | undefined][] = [
    [labels.proportional, seed.proportionalSymbol],
    [labels.integral, seed.integralSymbol],
    ["Model gain K", seed.gainSymbol],
    ["Model time constant T", seed.timeConstantSymbol],
    ["Model delay θ", seed.delaySymbol],
    ["Operating input", seed.operatingInputSymbol],
    ["Operating output", seed.operatingOutputSymbol],
  ];
  for (const [label, symbol] of rows) if (symbol !== undefined) settingRow(list, label, symbol, loop, values[symbol]);
  return [list];
}

/** Specification 35 C3: the hysteresis and the two minimum times from the answer, and the declared output levels. */
function onOffElements(loop: LoopDefinition, block: OnOffView, values: Numbers): HTMLElement[] {
  const list = settingList("data-studio-on-off-design");
  settingRow(list, "Hysteresis", block.hysteresisSymbol, loop, values[block.hysteresisSymbol]);
  settingRow(list, "Minimum ON time", block.minimumOnTimeSymbol, loop, values[block.minimumOnTimeSymbol]);
  settingRow(list, "Minimum OFF time", block.minimumOffTimeSymbol, loop, values[block.minimumOffTimeSymbol]);
  const term = document.createElement("dt");
  term.textContent = "Output levels";
  const detail = document.createElement("dd");
  detail.dataset.studioOnOffLevels = "";
  const unit = unitSuffix(loop.signals.manipulatedVariable.unit);
  detail.textContent = `OFF at ${formatEngineeringNumber(block.lowOutput)}${unit} · ON at ${formatEngineeringNumber(block.highOutput)}${unit}`;
  list.append(term, detail);
  return [list];
}

/** Specification 35 C7: each member's PI with its paired channels and its relative gain, then the decoupler D when the group has one. */
function groupElements(loop: LoopDefinition, group: GroupView, labels: GainLabelsView): HTMLElement[] {
  const element = tableWith(["Controller", "Paired channels", labels.proportional, labels.integral, "Relative gain λ"],
    "data-studio-group-members");
  const body = element.createTBody();
  for (const member of group.members) {
    const row = body.insertRow();
    row.dataset.studioGroupMember = member.instance;
    row.insertCell().textContent = member.instance;
    // The engine names each channel by the symbol of the signal the loop binds to it, else by the channel's identifier (GroupMemberDesign).
    row.insertCell().textContent = `${member.inputName ?? member.inputChannelId} → ${member.outputName ?? member.outputChannelId}`;
    row.insertCell().textContent = valueText(member.gains.proportionalGain, unitOf(loop, member.proportionalSymbol));
    row.insertCell().textContent = valueText(member.gains.integralGain, unitOf(loop, member.integralSymbol));
    row.insertCell().textContent = formatEngineeringNumber(member.relativeGain);
  }
  return group.decoupler ? [element, decouplerTable(group.decoupler)] : [element];
}

/** Specification 35 C7: `D` by plant input (row) and member (column), with the channels the engine names (GroupDecoupler.inputNames). */
function decouplerTable(decoupler: NonNullable<GroupView["decoupler"]>): HTMLElement {
  const element = tableWith(["Decoupler D: plant input", ...decoupler.memberInstances.map(instance => `from ${instance}`)],
    "data-studio-group-decoupler");
  const body = element.createTBody();
  decoupler.matrix.forEach((row, index) => {
    const cells = body.insertRow();
    cells.insertCell().textContent = decoupler.inputNames?.[index] ?? decoupler.inputChannelIds[index] ?? "";
    for (const value of row) cells.insertCell().textContent = formatEngineeringNumber(value);
  });
  return element;
}

/**
 * Specification 35 C10: the engine's sentence and choice. A fixed design shows its one PI; a scheduled design shows the table of every
 * operating point, with its scheduling value and its gains.
 */
function scheduleElements(loop: LoopDefinition, schedule: ScheduleView, labels: GainLabelsView): HTMLElement[] {
  const sentence = document.createElement("p");
  sentence.dataset.studioScheduleChoice = schedule.choice;
  sentence.textContent = schedule.reason;
  if (schedule.choice === "fixed") {
    const [first] = schedule.points;
    if (!first) return [sentence];
    const list = settingList("data-studio-schedule-fixed");
    settingRow(list, labels.proportional, first.proportionalSymbol, loop, first.proportionalGain);
    settingRow(list, labels.integral, first.integralSymbol, loop, first.integralGain);
    return [sentence, list];
  }
  const element = tableWith(["Operating point", `${schedule.scheduling.symbol}${unitSuffix(schedule.scheduling.unit)}`,
    labels.proportional, labels.integral], "data-studio-schedule-table");
  const body = element.createTBody();
  for (const point of schedule.points) {
    const row = body.insertRow();
    row.dataset.studioSchedulePoint = point.name;
    row.insertCell().textContent = point.name;
    row.insertCell().textContent = formatEngineeringNumber(point.value);
    row.insertCell().textContent = valueText(point.proportionalGain, unitOf(loop, point.proportionalSymbol));
    row.insertCell().textContent = valueText(point.integralGain, unitOf(loop, point.integralSymbol));
  }
  return [sentence, element];
}

type Numbers = Readonly<Record<string, number | undefined>>;

/** The ON/OFF design fields the page names: the symbols and the output levels, as the engine's answer gives them (the schema types the block loosely). */
type OnOffView = { hysteresisSymbol: string; minimumOnTimeSymbol: string; minimumOffTimeSymbol: string; lowOutput: number; highOutput: number };

/** A `dl` list of one structure's settings, marked for the page's tests. */
function settingList(marker: string): HTMLElement {
  const list = document.createElement("dl");
  list.className = "studio-setting-values";
  list.setAttribute(marker, "");
  return list;
}

/** One setting: its label, and its value with the unit the loop declares for its symbol. */
function settingRow(list: HTMLElement, label: string, symbol: string, loop: LoopDefinition, value: number | undefined): void {
  const term = document.createElement("dt");
  term.textContent = label;
  const detail = document.createElement("dd");
  detail.dataset.studioParameter = symbol;
  detail.textContent = valueText(value, unitOf(loop, symbol));
  list.append(term, detail);
}

/** A table with its header row, marked for the page's tests. */
function tableWith(headers: readonly string[], marker: string): HTMLTableElement {
  const element = document.createElement("table");
  element.className = "trust-table";
  element.setAttribute(marker, "");
  const header = element.createTHead().insertRow();
  for (const name of headers) {
    const cell = document.createElement("th");
    cell.textContent = name;
    header.append(cell);
  }
  return element;
}

/** The unit the loop declares for a symbol, or an empty text when the loop declares none. */
function unitOf(loop: LoopDefinition, symbol: string): string {
  return loop.parameterBinding?.parameters.find(parameter => parameter.symbol === symbol)?.unit ?? "";
}

/** A value with its unit, or a dash when the answer holds no finite value for the symbol. */
function valueText(value: number | undefined, unit: string): string {
  return value === undefined || !Number.isFinite(value) ? "—" : `${formatEngineeringNumber(value)}${unitSuffix(unit)}`;
}

function unitSuffix(unit: string): string {
  return unit === "" || unit === "1" ? "" : ` ${displayEngineeringUnit(unit)}`;
}

function recordOf(value: unknown): Record<string, unknown> | undefined {
  return value && typeof value === "object" && !Array.isArray(value) ? value as Record<string, unknown> : undefined;
}

function numbersOf(value: unknown): Numbers {
  return (recordOf(value) as Numbers | undefined) ?? {};
}
