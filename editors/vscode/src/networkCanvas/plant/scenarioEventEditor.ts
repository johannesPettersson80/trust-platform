export const PLANT_SCENARIO_EVENT_KINDS = [
  "set_state",
  "add_state",
  "set_parameter",
  "set_sensor_bias",
  "stick_actuator",
  "release_actuator",
  "reset_plant",
] as const;

export type PlantScenarioEventKind = typeof PLANT_SCENARIO_EVENT_KINDS[number];

/** Decimal seconds are converted exactly; no rounding can change an event's due time. */
export function scenarioMillisecondsFromSeconds(raw: string): number {
  const match = /^(\d{1,16})(?:\.(\d*))?$/.exec(raw.trim());
  if (!match) return Number.NaN;
  const fraction = (match[2] ?? "").replace(/0+$/, "");
  if (fraction.length > 3) return Number.NaN;
  const milliseconds = BigInt(match[1]) * 1000n + BigInt(fraction.padEnd(3, "0"));
  // The runtime stores elapsed nanoseconds in i64 (events.rs).
  return milliseconds <= 9_223_372_036_854n ? Number(milliseconds) : Number.NaN;
}

export interface PlantScenarioEventDraft {
  enabled: boolean;
  atMs: number;
  /** Unsaved seconds text, including invalid edits; never serialized to runtime configuration. */
  timeSecondsText?: string;
  kind: PlantScenarioEventKind;
  channel?: number;
  parameter?: string;
  value?: number;
}

export interface PlantScenarioEventIssue {
  row: number;
  field: "at_ms" | "kind" | "channel" | "parameter" | "value";
  message: string;
}

export interface PlantScenarioEventContext {
  enabled: boolean;
  stateCount: number;
  outputCount: number;
  scalarActuator: boolean;
  parameters: readonly string[];
  stateLabels?: readonly string[];
  outputLabels?: readonly string[];
}

export function createPlantScenarioEvent(_sequence: number): PlantScenarioEventDraft {
  return { enabled: true, atMs: 0, kind: "set_state", channel: 0, value: 0 };
}

export function applyPlantScenarioEventKind(
  current: PlantScenarioEventDraft,
  kind: PlantScenarioEventKind,
  parameters: readonly string[]
): PlantScenarioEventDraft {
  const common = { enabled: current.enabled, atMs: current.atMs, kind,
    ...(current.timeSecondsText === undefined ? {} : { timeSecondsText: current.timeSecondsText }) };
  switch (kind) {
    case "set_state":
    case "add_state":
    case "set_sensor_bias":
      return { ...common, channel: 0, value: 0 };
    case "set_parameter":
      return { ...common, parameter: parameters[0] ?? "", value: 0 };
    case "stick_actuator":
    case "release_actuator":
    case "reset_plant":
      return common;
  }
}

/** Declaration order is behavioral only for events with the same due time. */
export function movePlantScenarioEvent(
  rows: readonly PlantScenarioEventDraft[],
  index: number,
  delta: -1 | 1
): PlantScenarioEventDraft[] {
  const target = index + delta;
  if (
    index < 0 || index >= rows.length || target < 0 || target >= rows.length ||
    rows[index].atMs !== rows[target].atMs
  ) {
    return [...rows];
  }
  const next = [...rows];
  [next[index], next[target]] = [next[target], next[index]];
  return next;
}

export function validatePlantScenarioEvents(
  rows: readonly PlantScenarioEventDraft[],
  context: PlantScenarioEventContext
): PlantScenarioEventIssue[] {
  const issues: PlantScenarioEventIssue[] = [];
  const add = (row: number, field: PlantScenarioEventIssue["field"], message: string) =>
    issues.push({ row, field, message });
  rows.forEach((event, row) => {
    if (!event.enabled) return;
    if (!context.enabled) {
      add(row, "kind", "Enable the plant or remove its scenario events.");
    }
    if (!Number.isSafeInteger(event.atMs) || event.atMs < 0) {
      add(row, "at_ms", event.timeSecondsText === undefined
        ? "Time must be a nonnegative whole number of milliseconds."
        : "Enter nonnegative decimal seconds with at most 0.001 s precision; the value must fit the supported time range.");
    }
    const finiteValue = () => {
      if (typeof event.value !== "number" || !Number.isFinite(event.value)) {
        add(row, "value", "Enter a finite event value.");
      }
    };
    switch (event.kind) {
      case "set_state":
      case "add_state":
        if (!Number.isSafeInteger(event.channel) || event.channel! < 0 || event.channel! >= context.stateCount) {
          add(row, "channel", channelRange("State", context.stateCount));
        }
        finiteValue();
        break;
      case "set_parameter":
        if (!event.parameter || !context.parameters.includes(event.parameter)) {
          add(row, "parameter", "Choose a parameter owned by the selected plant model.");
        }
        finiteValue();
        break;
      case "set_sensor_bias":
        if (!Number.isSafeInteger(event.channel) || event.channel! < 0 || event.channel! >= context.outputCount) {
          add(row, "channel", channelRange("Sensor", context.outputCount));
        }
        finiteValue();
        break;
      case "stick_actuator":
      case "release_actuator":
        if (!context.scalarActuator) {
          add(row, "kind", "Actuator fault events require a plant with one command signal.");
        }
        break;
      case "reset_plant":
        break;
      default:
        add(row, "kind", "Choose a supported scenario action.");
    }
  });
  return issues;
}

export function persistedPlantScenarioEvents(
  rows: readonly PlantScenarioEventDraft[]
): Record<string, unknown>[] {
  return rows.filter((event) => event.enabled).map((event) => {
    const result: Record<string, unknown> = { at_ms: event.atMs, kind: event.kind };
    if (["set_state", "add_state", "set_sensor_bias"].includes(event.kind)) result.channel = event.channel;
    if (event.kind === "set_parameter") result.parameter = event.parameter;
    if (["set_state", "add_state", "set_parameter", "set_sensor_bias"].includes(event.kind)) result.value = event.value;
    return result;
  });
}

export function scenarioEventsForPlant(
  savedConfiguration: unknown,
  plantId: string
): PlantScenarioEventDraft[] {
  if (!savedConfiguration || typeof savedConfiguration !== "object" || Array.isArray(savedConfiguration)) return [];
  const events = (savedConfiguration as Record<string, unknown>).plant_events;
  if (!Array.isArray(events)) return [];
  return events.flatMap((entry) => {
    if (!entry || typeof entry !== "object" || Array.isArray(entry)) return [];
    const event = entry as Record<string, unknown>;
    if (event.plant !== plantId || !PLANT_SCENARIO_EVENT_KINDS.includes(event.kind as PlantScenarioEventKind)) return [];
    const row: PlantScenarioEventDraft = {
      enabled: true,
      atMs: typeof event.at_ms === "number" ? event.at_ms : Number.NaN,
      kind: event.kind as PlantScenarioEventKind,
    };
    if (typeof event.channel === "number") row.channel = event.channel;
    if (typeof event.parameter === "string") row.parameter = event.parameter;
    if (typeof event.value === "number") row.value = event.value;
    return [row];
  });
}

function channelRange(name: string, count: number): string {
  return count > 0
    ? `${name} channel must be between 0 and ${count - 1}.`
    : `${name} channel is unavailable for this model.`;
}
