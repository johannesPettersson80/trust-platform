import type { LoopDefinition } from "../generated/OpenResult";
import type { TuningRequirements } from "../generated/InputsResult";
import { displayEngineeringUnit } from "../chartContract";
import { openCompactNumberEditor, type CompactEditorIssue, type CompactNumberField } from "../../webview/compactEditor";
import { openedOffers } from "./openedOffers";
import { mountThreePageScenarios, type ThreePageScenarioOptions } from "./threePageScenarios";
import { signalRoleLabel } from "./signalRoles";

/** A signal of the loop as the requirements rows show it: its variable and its unit. */
interface NamedSignal {
  symbol: string;
  unit: string;
}

/** The test design that sits under More…; an automatic value in any of these marks the section Automatic. */
const MORE_KEYS = ["referenceChange", "durationSeconds", "transferTolerance", "measurementNoiseBand", "gainUncertaintyFraction"] as const;

/** The rows of one controlled output, by the names of the engine's record (B4.3, C7). A loop with several outputs names them `<row>:<channel>`. */
const OUTPUT_ROWS = ["referenceChange", "maximumError", "settlingBand", "settlingTimeSeconds", "maximumOvershootPercent"] as const;

/** The values an output's rows start from, by the names of the engine's record. */
type RowValues = { readonly [row in (typeof OUTPUT_ROWS)[number]]?: number | string | null };

/** The rows the engineer typed for one output, as the engine reads them (B4.3): the output's channel and its values, numbers or the text of a duration. */
export interface TypedOutput {
  channel: string;
  referenceChange?: number | string;
  maximumError?: number | string;
  settlingBand?: number | string;
  settlingTimeSeconds?: number | string;
  maximumOvershootPercent?: number | string;
}

/**
 * The requirements the page holds when it opens the editor: the record the engine last answered or, until the engine answers a change, the change
 * the page posted. A change for several outputs holds their rows as typed (`outputRequirements`), so the engine's refusal of it shows them again.
 */
export type HeldRequirements = Partial<TuningRequirements> & { outputRequirements?: readonly TypedOutput[] };

/**
 * What the engine answers about the requirements (B4.3): whether the measurement noise band is required (a declared low-pass or notch
 * filter needs it), and the refusal of the last requirements, which the editor shows under the field the refusal names.
 */
export interface RequirementEngineNotes {
  measurementNoiseBandRequired?: boolean;
  refusal?: CompactEditorIssue;
}

/** The rows the engineer typed for the output of `channel`, taken from the editor's values, which name each row `<row>:<channel>`. */
function typedRows(channel: string, values: Readonly<Record<string, number | string>>): TypedOutput {
  const rows: TypedOutput = { channel };
  for (const row of OUTPUT_ROWS) {
    const value = values[`${row}:${channel}`];
    if (value !== undefined) rows[row] = value;
  }
  return rows;
}

/**
 * Done sends what was typed, then runs `resumeTune`; only the editor that a paused Tune opened receives it. Cancel changes nothing. The engine
 * assembles the requirement record from the rows (B4.3): for one output they are the record's own values; for several, the page sends the rows
 * of each output under its channel (`outputRequirements`, in the editor's order) with the shared values beside them, and the engine writes
 * each output's record, the first output's copy and the shared values. A blank optional row is left out, and declares no value.
 */
export function editControlRequirements(loop: LoopDefinition, current: HeldRequirements,
  commit: (next: TuningRequirements) => void, scenarios?: ThreePageScenarioOptions, resumeTune?: () => void,
  notes: RequirementEngineNotes = {}): void {
  const output = loop.signals.processVariable;
  // The engine lists the editor's controlled outputs and what their rows start from (`requirementOutputs`, B4.3), and names the signal of each
  // output channel (`liveSides`, B3.6). A listed output whose signal the live sides do not name is dropped here and gets no row. A loop with one
  // output names its rows by the loop's process variable, and the input disturbance is named by the loop's plant input.
  const offers = openedOffers();
  const sides = offers.liveSides?.outputs ?? [];
  const outputs = (offers.requirementOutputs ?? []).flatMap(offered => {
    const signal = sides.find(side => side.signalId === offered.signalId);
    return signal ? [{ offered, signal }] : [];
  });
  const multichannel = outputs.length > 1;
  const channelFields = (modelChannelId: string, signal: NamedSignal, values: RowValues | undefined): CompactNumberField[] => {
    const unit = displayEngineeringUnit(signal.unit), suffix = multichannel ? `:${modelChannelId}` : "";
    const label = (key: string, text: string) => current.automaticCriteria?.fields.includes(`${key}${suffix}`) ? `${text} · Automatic` : text;
    const value = (key: (typeof OUTPUT_ROWS)[number]) => values?.[key] ?? undefined;
    const role = signalRoleLabel(offers.liveSides, signal.symbol, signal.unit);
    return [
      { key: `settlingTimeSeconds${suffix}`, label: label("settlingTimeSeconds", "Maximum settling time"), signal: signal.symbol, signalLabel: role, value: value("settlingTimeSeconds"), duration: true },
      { key: `settlingBand${suffix}`, label: label("settlingBand", `Settling band (±${unit})`), signal: signal.symbol, signalLabel: role, value: value("settlingBand") },
      // Overshoot is never generated: a blank row declares no limit, so it is not labelled Automatic.
      { key: `maximumOvershootPercent${suffix}`, label: "Maximum overshoot (% of setpoint change)", signal: signal.symbol, signalLabel: role,
        value: value("maximumOvershootPercent"), optional: true, placeholder: "None" },
      { key: `maximumError${suffix}`, label: label("maximumError", `Maximum steady-state error (${unit})`), signal: signal.symbol, signalLabel: role, value: value("maximumError") },
      { key: `referenceChange${suffix}`, label: label("referenceChange", `Test setpoint change (${unit})`), signal: signal.symbol, signalLabel: role, value: value("referenceChange"), more: true },
    ];
  };
  // An output's rows start from the newest rows the page holds for its channel: the engine's record or the change just posted (a refusal brings
  // its rows back); until the study holds either, from what the engine offered for the output when the loop opened.
  const startOf = ({ offered }: (typeof outputs)[number]): RowValues | undefined =>
    current.channelRequirements?.[offered.channelId] ?? current.outputRequirements?.find(rows => rows.channel === offered.channelId) ?? offered.requirements ?? undefined;
  const fields: CompactNumberField[] = multichannel
    ? outputs.flatMap(item => channelFields(item.offered.channelId, item.signal, startOf(item)))
    : channelFields("", output, current);
  const unit = displayEngineeringUnit(output.unit);
  // A constant load added to the plant input in its own trial with the setpoint unchanged; blank declares none.
  const command = loop.signals.manipulatedVariable;
  const commandRole = signalRoleLabel(offers.liveSides, command.symbol, command.unit);
  fields.push({ key: "constantInputDisturbance", label: `Input disturbance (${displayEngineeringUnit(command.unit)})`, signal: command.symbol, signalLabel: commandRole,
    value: current.constantInputDisturbance ?? undefined, optional: true, placeholder: "None" });
  fields.push({ key: "durationSeconds", label: `Validation duration${current.automaticCriteria?.fields.includes("durationSeconds") ? " · Automatic" : ""}`, value: current.durationSeconds,
    duration: true, more: true });
  // The engine says whether a declared filter needs the band (B4.3); a band that is already given is always shown.
  if (current.measurementNoiseBand !== undefined || notes.measurementNoiseBandRequired) fields.push({ key: "measurementNoiseBand", label: `Measurement noise (±${unit})`, value: current.measurementNoiseBand ?? undefined, more: true });
  if (current.gainUncertaintyFraction !== undefined) fields.push({ key: "gainUncertaintyFraction", label: "Gain uncertainty (0–1)", value: current.gainUncertaintyFraction ?? undefined, more: true });
  // Auto commissioning checks the controller's first Auto output against this tolerance; blank uses the automatic allowance.
  fields.push({ key: "transferTolerance", label: `Auto-entry tolerance (${displayEngineeringUnit(command.unit)})`, signal: command.symbol, signalLabel: commandRole,
    value: current.transferTolerance ?? undefined, optional: true, placeholder: "Automatic", more: true });
  const moreAutomatic = current.automaticCriteria?.fields.some(key => (MORE_KEYS as readonly string[]).some(name => key === name || key.startsWith(`${name}:`))) ?? false;
  const done = (next: TuningRequirements): void => { commit(next); resumeTune?.(); };
  let scenarioEditor: ReturnType<typeof mountThreePageScenarios> | undefined;
  openCompactNumberEditor({ title: "Control requirements", fields, fieldAttribute: "data-studio-requirement", rowLayout: "requirements",
    showSignals: multichannel, moreLabel: `More…${moreAutomatic || current.automaticCriteria?.fields.includes("referenceChange") ? " · Automatic" : ""}`,
    // The editor checks no range and no entry: the engine reads each entry as typed and refuses a value under its field (B4.3, requirements.rs).
    // The engine's refusal of the scenario list names the scenarios, not a requirement field: the scenario area shows it (B4.3).
    initialIssue: notes.refusal?.field === "scenarios" ? undefined : notes.refusal,
    commit: values => {
      // What the editor does not edit goes back as the engine answered it: the declared response and the record of the generated values, which
      // the engine keeps Automatic on only for the values that the change leaves as they were (B4.3).
      const origin = {
        ...(current.tuningObjective ? { tuningObjective: current.tuningObjective } : {}),
        ...(current.automaticCriteria ? { automaticCriteria: current.automaticCriteria } : {}),
      };
      const scenarioValues = scenarioEditor?.read();
      const scenarioPayload = scenarioValues ? { scenarioDefinitions: scenarioValues } : {};
      if (!multichannel) return done({ ...values, ...origin, ...scenarioPayload } as unknown as TuningRequirements);
      // The channel-suffixed rows are the editor's names for its fields: the engine reads the rows of each output under its channel.
      const typed = outputs.map(({ offered }) => typedRows(offered.channelId, values));
      const shared = Object.fromEntries(Object.entries(values).filter(([key]) => !key.includes(":")));
      done({ outputRequirements: typed, ...shared, ...origin, ...scenarioPayload } as unknown as TuningRequirements);
    } });
  if (scenarios) {
    const dialog = document.querySelector<HTMLElement>("[data-trust-compact-editor]");
    if (dialog) scenarioEditor = mountThreePageScenarios(dialog, scenarios);
    if (notes.refusal?.field === "scenarios") scenarioEditor?.refuse(notes.refusal.message);
  }
}
