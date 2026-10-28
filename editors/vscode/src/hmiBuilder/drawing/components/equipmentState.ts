// What the bindings of a symbol say (specification 39, §5.3, item 4): its state word, what the
// symbol shows, its signs, and the values that decide them. Each symbol reads its own state; an
// unknown deciding value makes it unknown, and manual and out of service are signs of every one.

import type { DrawFrame, HmiObjectModel, LiveValue } from "../model";
import type { SymbolDefinition } from "../symbols";
import { on, SymbolReading } from "../symbols/reading";
import { applyRules } from "../symbols/rules";
import { liveValueOf } from "../values";

export interface EquipmentReading extends SymbolReading {
  manual: boolean;
  outOfService: boolean;
  /** The deciding value that styles and marks the object: the stalest, else an unknown one. */
  live?: LiveValue;
}

export function readEquipment(
  definition: SymbolDefinition,
  object: HmiObjectModel,
  frame: DrawFrame,
): EquipmentReading {
  const value = (name: string): LiveValue | undefined => liveValueOf(object, name, frame);
  const values = definition.deciding.map(value).filter((candidate): candidate is LiveValue => candidate !== undefined);
  const base = {
    manual: on(value("manual")),
    outOfService: on(value("out-of-service")),
    live: stalest(values) ?? values.find((candidate) => candidate.state === "unknown") ?? values[0],
  };
  // The object's own variant and tag stand in every view, an unknown one too.
  const own = { ...(object.variant ? { variant: object.variant } : {}), ...(object.tag ? { tag: object.tag } : {}) };
  if (values.some((candidate) => candidate.state === "unknown")) {
    return { ...base, word: "?", fault: false, view: { unknown: true, ...own } };
  }
  const reading = definition.read(value, frame, on(value("fault")), object);
  const parts = applyRules(definition.rules ?? [], value, frame, object);
  const view = { ...reading.view, ...(Object.keys(parts).length > 0 ? { parts } : {}), ...own };
  return { ...base, ...reading, view };
}

function stalest(values: LiveValue[]): LiveValue | undefined {
  return values.filter((value) => value.staleMs !== undefined).sort((a, b) => (b.staleMs ?? 0) - (a.staleMs ?? 0))[0];
}
