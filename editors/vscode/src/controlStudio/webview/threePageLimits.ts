import type { AcquisitionSettings, ExperimentLimits } from "../generated/OpenResult";
import { displayEngineeringUnit } from "../chartContract";
import { openCompactNumberEditor, type CompactEditorIssue } from "../../webview/compactEditor";

/**
 * One signal's range in the Limits editor. A bound is the number the engineer committed, or the text the engineer typed when it is no
 * number, which the engine reads and refuses under the field of the range's maximum (specification 35, B3.2).
 */
export interface StudioRange {
  symbol: string;
  unit: string;
  minimum: number | string;
  maximum: number | string;
}

/**
 * The Limits editor's values (specification 35, B3.2): each plant input and measured output with the range the experiment may use, and the
 * maximum duration. The starting values are the engine's limits (`experimentLimits` of the open answer, which holds the study's own
 * record over the loop's declared defaults); the page edits a copy and sends it back. A duration is text: the engine's words for it
 * (`durationWords`), or the text the page committed (`15 min`), which the engine reads and checks; it is absent while neither the loop nor the
 * engineer has given one.
 */
export interface StudioExperimentLimits {
  durationSeconds?: string;
  inputs: StudioRange[];
  outputs: StudioRange[];
  /** The loop's declared amplitude and relay direction (B3.3): the engine's values, which the page carries and does not edit. */
  amplitude: number;
  relayDirection?: ExperimentLimits["relayDirection"];
}

/** The limits the engine gave, as the editor starts from them (B3.2). */
export function limitsOf(held: ExperimentLimits): StudioExperimentLimits {
  const range = ({ symbol, unit, minimum, maximum }: ExperimentLimits["inputs"][number]): StudioRange => ({ symbol, unit, minimum, maximum });
  return {
    // The engine's words for the duration read back as exactly the duration they show, so the editor shows them as they are and the page never
    // rounds a number into the text it sends (B9.9).
    durationSeconds: held.durationWords ?? undefined,
    inputs: held.inputs.map(range),
    outputs: held.outputs.map(range),
    amplitude: held.amplitude,
    relayDirection: held.relayDirection ?? undefined,
  };
}

/**
 * Edit a copy; only Done commits the complete set. The engine checks every entry (B3.2): it refuses a range beyond the loop's declared
 * limits, a bound that no number reads and a duration that no time reads, each under its field, so the editor checks none and commits
 * what was typed. `refusal` is the engine's sentence for the last values, shown under its field: a refusal of a limit, or the sentence with
 * which a run asked for a missing limit (B8, `needsMessage`). The editor writes no sentence of its own.
 */
export function editExperimentLimits(values: StudioExperimentLimits,
  commit: (limits: StudioExperimentLimits) => void, cancelled?: () => void, refusal?: CompactEditorIssue): void {
  const multichannel = values.inputs.length > 1 || values.outputs.length > 1;
  const fields = [
    ...values.inputs.flatMap(signal => rangeFields("input", signal, multichannel)),
    ...values.outputs.flatMap(signal => rangeFields("output", signal, multichannel)),
    { key: "durationSeconds", label: "Maximum duration", value: values.durationSeconds, duration: true },
  ];
  // The editor checks no range and no entry: the engine reads what was typed and refuses a value under its field (B3.2, `inputs_action.rs`).
  openCompactNumberEditor({ title: "Experiment limits", fieldAttribute: "data-studio-limit", cancelled,
    initialIssue: refusal, fields,
    commit: next => {
      commit({
        ...values,
        durationSeconds: String(next.durationSeconds),
        inputs: values.inputs.map(signal => ({ ...signal, minimum: next[fieldKey("inputMinimum", signal.symbol, multichannel)],
          maximum: next[fieldKey("inputMaximum", signal.symbol, multichannel)] })),
        outputs: values.outputs.map(signal => ({ ...signal, minimum: next[fieldKey("outputMinimum", signal.symbol, multichannel)],
          maximum: next[fieldKey("outputMaximum", signal.symbol, multichannel)] })),
      });
    } });
}

/** The Minimum and Maximum fields of one signal's range. They carry no bounds of their own: the engine checks each value (B3.2). */
function rangeFields(kind: "input" | "output", signal: StudioRange, multichannel: boolean) {
  const group = `${kind === "input" ? "Plant input" : "Measured output"} · ${signal.symbol} (${displayEngineeringUnit(signal.unit)})`;
  return [
    { key: fieldKey(`${kind}Minimum`, signal.symbol, multichannel), label: "Minimum", group, signal: signal.symbol, value: signal.minimum },
    { key: fieldKey(`${kind}Maximum`, signal.symbol, multichannel), label: "Maximum", group, signal: signal.symbol, value: signal.maximum },
  ];
}

function fieldKey(prefix: string, symbol: string, multichannel: boolean): string { return multichannel ? `${prefix}:${symbol}` : prefix; }

/**
 * The request's acquisition settings for the limits the page holds (B3.2): the range of every plant input and measured output by its
 * symbol, as the editor lists them, and the amplitude and relay direction the engine gave. The duration and every bound go as the editor
 * holds them: the number, or the text typed, which the engine reads and checks. A duration that is not given goes as the blank text of its
 * field, and the engine refuses it under the duration's field.
 */
export function acquisitionPreferences(method: AcquisitionSettings["method"], limits: StudioExperimentLimits): AcquisitionSettings {
  return { method, durationSeconds: limits.durationSeconds ?? "", amplitude: limits.amplitude,
    inputRanges: bySymbol(limits.inputs), outputRanges: bySymbol(limits.outputs),
    ...(limits.relayDirection ? { relayDirection: limits.relayDirection } : {}) };
}

function bySymbol(ranges: readonly StudioRange[]): Record<string, { minimum: number | string; maximum: number | string }> {
  return Object.fromEntries(ranges.map(range => [range.symbol, { minimum: range.minimum, maximum: range.maximum }]));
}
