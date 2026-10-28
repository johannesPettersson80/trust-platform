// What a symbol's bindings say (specification 39, §5.3 item 4): the shared pieces each symbol
// reads its state with.

import { formatValue } from "../format";
import { withValueFormat } from "../valueFormats";
import type { DrawFrame, HmiObjectModel, LiveValue, StructuredValue } from "../model";
import { say } from "../speech";
import { isOn } from "../values";
import type { SymbolState, SymbolView } from "./index";
import { observed, turnDelayMs } from "./motion";

/** A binding's value, by property name. */
export type ValueOf = (name: string) => LiveValue | undefined;

/** What a symbol shows of its state: the state for styles and tests, the word under the tag, a
 * fault, and the view its parts are drawn from. */
export interface SymbolReading {
  /** None while nothing that decides it is bound. */
  state?: string;
  word: string;
  fault: boolean;
  view: SymbolView;
}

/** A symbol's reader: from its bindings' values, the frame and its fault, what it shows. */
export type SymbolReader = (
  value: ValueOf,
  frame: DrawFrame,
  faulted: boolean,
  object: HmiObjectModel,
) => SymbolReading;

/** Whether a boolean value is known and TRUE. */
export function on(value: LiveValue | undefined): boolean {
  return value !== undefined && value.state !== "unknown" && isOn(value.value);
}

/** The motion of a part that turns while `value` says so and is a current observation. */
export function turning(value: LiveValue | undefined, frame: DrawFrame): { delayMs: number } | undefined {
  return on(value) && observed(value) ? { delayMs: turnDelayMs(frame.nowMs) } : undefined;
}

/** Machines that run and stop, as the pump does: the moving part turns while `running` is TRUE,
 * also during a fault; the word says Fault first, else Running or Stopped. */
export const readRunning: SymbolReader = (value, frame, faulted) => {
  const running = value("running");
  const runs = on(running);
  const word = running ? say(runs ? "Running" : "Stopped") : "";
  return {
    state: faulted ? "fault" : running ? (runs ? "running" : "stopped") : undefined,
    word: faulted ? say("Fault") : word,
    fault: faulted,
    view: { unknown: false, turning: turning(running, frame) },
  };
};

/** A number for the word: the value with the object's decimals and unit, or nothing. */
export function shownNumber(value: LiveValue | undefined, object: HmiObjectModel): string | undefined {
  const number = Number(value?.value);
  if (value === undefined || value.value === null || !Number.isFinite(number)) return undefined;
  const text = formatValue(number, { decimals: object.decimals });
  return object.unit ? `${text} ${object.unit}` : text;
}

/** A number in a state, given as a share of the object's range, so the state reads right whatever
 * the range. */
export interface Share {
  share: number;
}

/** A binding's value in a state: a value, or a share of the object's range. */
export type StateValue = StructuredValue | Share;

function isShare(value: StateValue): value is Share {
  return (
    typeof value === "object" &&
    value !== null &&
    !Array.isArray(value) &&
    Object.keys(value).length === 1 &&
    typeof (value as Share).share === "number"
  );
}

/** A state's value for an object: a share taken of the object's range (default 0 to 100). */
export function resolveState(value: StateValue, object: HmiObjectModel): StructuredValue {
  if (!isShare(value)) return value;
  const [min, max] = [object.min ?? 0, object.max ?? 100];
  return Number((min + (max - min) * value.share).toFixed(6));
}

type Flag = "fault" | "manual" | "out-of-service";

/** The states of a machine that runs and stops (§5.3 item 4): Running, Stopped, Fault, Manual and
 * Out of service; `extra` gives its other bindings' values as it runs or stands. */
export function runningStates(extra: (running: boolean) => Record<string, StateValue> = () => ({})): SymbolState[] {
  const state = (
    id: string,
    name: string,
    running: boolean,
    flags: Partial<Record<Flag, boolean>> = {},
  ): SymbolState => {
    const set = Object.keys(flags);
    return {
      id,
      name,
      values: { running, fault: false, manual: false, "out-of-service": false, ...extra(running), ...flags },
      needs: set.length > 0 ? set : ["running"],
    };
  };
  return [
    state("running", "Running", true),
    state("stopped", "Stopped", false),
    state("fault", "Fault", false, { fault: true }),
    state("manual", "Manual", true, { manual: true }),
    state("out-of-service", "Out of service", false, { "out-of-service": true }),
  ];
}

/** A number in percent for the word, or nothing. */
export function percentWord(value: LiveValue | undefined): string | undefined {
  const number = Number(value?.value);
  if (value === undefined || value.value === null || !Number.isFinite(number)) return undefined;
  return say("{value} %", { value: formatValue(Math.round(number), {}) });
}

/** A machine that gives a number while it runs: a pump's speed in percent, a dosing pump's rate
 * in its unit. Stopped or faulted, it says so. */
export function readRunningWith(property: string, percent: boolean): SymbolReader {
  return (value, frame, faulted, object) => {
    const reading = readRunning(value, frame, faulted, object);
    if (reading.state !== "running") return reading;
    const shown = percent ? percentWord(value(property)) : shownBoundNumber(value, property, frame, object);
    return shown ? { ...reading, word: shown } : reading;
  };
}

/** Equipment that shows numbers (§5.3 item 5, show a value): the word gives them with the
 * object's decimals and unit, joined when there are two. */
export function readNumbers(...properties: string[]): SymbolReader {
  return (value, frame, faulted, object) => {
    const shown = properties
      .map((property) => shownBoundNumber(value, property, frame, object))
      .filter((text): text is string => text !== undefined);
    return {
      state: faulted ? "fault" : undefined,
      word: faulted ? say("Fault") : shown.join(" · "),
      fault: faulted,
      view: { unknown: false },
    };
  };
}

/** Format a symbol property with the same path defaults as a standalone readout. */
export function shownBoundNumber(
  value: ValueOf,
  property: string,
  frame: DrawFrame,
  object: HmiObjectModel,
): string | undefined {
  return shownNumber(value(property), withValueFormat(object, frame, property));
}
