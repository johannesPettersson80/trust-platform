/**
 * A scenario event as the plant editor's draft holds it (specification 47 §12). Rust owns its rules:
 * the time read from the seconds text, the checks, the defaults of a new event and of a new action,
 * which events may change places, and the entries a plant saves.
 */
export interface PlantScenarioEventDraft {
  enabled: boolean;
  atMs: number;
  /** The seconds text as typed; once present, Rust reads the event time from it. */
  timeSecondsText?: string;
  kind: string;
  channel?: number;
  parameter?: string;
  value?: number;
}

/** The saved events of one plant as draft rows; the saved file holds only included events. */
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
    if (event.plant !== plantId) return [];
    const row: PlantScenarioEventDraft = {
      enabled: true,
      atMs: typeof event.at_ms === "number" ? event.at_ms : Number.NaN,
      kind: typeof event.kind === "string" ? event.kind : "",
    };
    if (typeof event.channel === "number") row.channel = event.channel;
    if (typeof event.parameter === "string") row.parameter = event.parameter;
    if (typeof event.value === "number") row.value = event.value;
    return [row];
  });
}
