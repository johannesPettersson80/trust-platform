/**
 * A scenario event as the plant editor's draft holds it (specification 47 §12). Rust owns its rules:
 * the saved events a plant opens with (`plant.topology`), the time read from the seconds text, the
 * checks, the defaults of a new event and of a new action, which events may change places, and the
 * entries a plant saves.
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
