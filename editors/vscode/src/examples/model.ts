// Pure model for the bundled starter examples (§0.5.12) — NO vscode import, so it's unit-testable and the
// manifest shape is guarded standalone. The vscode-facing copy/open flow lives in ../examples.ts.

import {
  parseControlLessonContract,
  type ControlLessonAvailability,
  type ControlLessonMetadata,
} from "./controlLesson";

export type { ControlLessonMetadata } from "./controlLesson";

export type ExampleHardware = "none" | "twincat" | "raspberrypi" | string;

/**
 * Superseded by `controlModulesFromEntries` below (W-04): this hand list of five
 * levels split the fourteen real course modules across unrelated titles and no
 * longer drives the gallery. Left exported, unchanged, only because it is still
 * a load-bearing fixture for other test coverage outside this slice's ownership.
 */
export const CONTROL_COURSES = [
  { id: "foundation", title: "Feedback foundations", range: "Lessons 1–9", minutes: 25, summary: "Observe feedback, then progress from on/off and P control to practical PI limits." },
  { id: "modelling", title: "Modelling and classical analysis", range: "Lessons 10–23", minutes: 35, summary: "Build, identify, validate, and analyse dynamic models before choosing a controller." },
  { id: "practitioner", title: "Industrial PID and loop engineering", range: "Lessons 24–40", minutes: 35, summary: "Tune, commission, diagnose, and combine industrial control-loop structures." },
  { id: "advanced", title: "Digital and advanced industrial control", range: "Lessons 41–60", minutes: 40, summary: "Handle sampling, unstable and coupled plants, scheduling, delay, and constraints." },
  { id: "modern", title: "Modern control", range: "Lessons 61–84", minutes: 45, summary: "Learn multivariable, state-space, estimation, optimal, robust, adaptive, nonlinear, and predictive control." },
] as const;

export type GalleryView =
  | { readonly kind: "landing" }
  | { readonly kind: "browse" }
  | { readonly kind: "control" }
  | { readonly kind: "course"; readonly id: string }
  | { readonly kind: "lesson"; readonly id: string };

export function parseControlLesson(raw: unknown, id: string): ControlLessonMetadata {
  return parseControlLessonContract(raw, id);
}

export interface ExampleEntry {
  readonly id: string;
  readonly title: string;
  readonly description: string;
  readonly path: string; // relative project directory within media/examples
  readonly hardware: ExampleHardware;
  readonly tags?: ReadonlyArray<string>;
  readonly lesson?: ControlLessonMetadata;
  readonly lessonAvailability?: ControlLessonAvailability;
}

/** Fail closed for curriculum projects; ordinary starter examples remain copyable. */
export function isExampleCopyable(entry: ExampleEntry): boolean {
  if (!entry.tags?.includes("control")) {
    return true;
  }
  return entry.lessonAvailability?.kind === "available";
}

export function controlLessonPositionLabel(sequence: number, total: number): string {
  return `Lesson ${sequence} of ${total}`;
}

export interface ControlModuleSummary {
  readonly id: string;
  readonly title: string;
  readonly range: string;
  readonly lessonCount: number;
}

const CONTROL_MODULE_PATH = /^control\/([^/]+)\//;

/**
 * The module directory basename embedded in a lesson's catalog path (e.g.
 * "01-feedback-foundations-and-temperature-control" from
 * "control/01-feedback-foundations-and-temperature-control/01-cold-storage-loop-anatomy").
 * Returns undefined for a non-curriculum example.
 */
export function controlModuleId(entryPath: string): string | undefined {
  return CONTROL_MODULE_PATH.exec(entryPath)?.[1];
}

/**
 * Humanizes a module directory basename into its display title (sentence
 * case; no acronym-casing table). The directory tree under examples/control/
 * is the one source of truth for module names and order — this never falls
 * back to a hand-maintained title list.
 */
export function controlModuleTitle(moduleId: string): string {
  const words = moduleId.replace(/^\d+-/, "").replace(/-/g, " ");
  return words.charAt(0).toUpperCase() + words.slice(1);
}

/**
 * Derives the ordered module list — and each module's real lesson range —
 * from the loaded lesson entries (examples/catalog.json + lesson.json).
 * Grouping follows the fourteen module directories under examples/control/;
 * there is no second, hand-maintained course list to keep in sync (W-04).
 */
export function controlModulesFromEntries(
  entries: ReadonlyArray<ExampleEntry>
): ControlModuleSummary[] {
  const sequencesById = new Map<string, number[]>();
  for (const entry of entries) {
    if (!entry.lesson) {
      continue;
    }
    const id = controlModuleId(entry.path);
    if (!id) {
      continue;
    }
    const sequences = sequencesById.get(id) ?? [];
    sequences.push(entry.lesson.sequence);
    sequencesById.set(id, sequences);
  }
  return Array.from(sequencesById.entries())
    .sort(([left], [right]) => left.localeCompare(right))
    .map(([id, sequences]) => {
      const minimum = Math.min(...sequences);
      const maximum = Math.max(...sequences);
      return {
        id,
        title: controlModuleTitle(id),
        range: minimum === maximum ? `Lesson ${minimum}` : `Lessons ${minimum}–${maximum}`,
        lessonCount: sequences.length,
      };
    });
}

export type ControlLessonProgressState = "not_started" | "project_created";

/**
 * Honest per-lesson progress (W-05). Creating a project folder is real, but
 * it is not retained evidence of a finished Monitor -> Test run, so it must
 * never be reported to the learner as "completed".
 */
export function controlLessonProgressState(
  lessonId: string,
  createdProjectIds: ReadonlyArray<string>
): ControlLessonProgressState {
  return createdProjectIds.includes(lessonId) ? "project_created" : "not_started";
}

export function controlLessonProgressLabel(state: ControlLessonProgressState): string {
  return state === "project_created" ? "Project created" : "Not started";
}

export interface ExamplePick {
  readonly label: string;
  readonly description: string; // the hardware-requirement badge
  readonly detail: string;
  readonly id: string;
  readonly path: string;
}

// The user-facing hardware-requirement badge shown next to each example.
export function hardwareBadge(hardware: ExampleHardware): string {
  switch (hardware) {
    case "none":
      return "No hardware";
    case "twincat":
      return "Requires TwinCAT";
    case "raspberrypi":
      return "Requires Raspberry Pi";
    default:
      return `Requires ${hardware}`;
  }
}

/** Keep gallery summaries scannable while preserving the full catalog description on disk. */
export function compactExampleDescription(description: string, limit = 80): string {
  if (description.length <= limit) {
    return description;
  }
  const prefix = description.slice(0, limit - 1);
  const boundary = prefix.lastIndexOf(" ");
  return `${prefix.slice(0, boundary > 0 ? boundary : prefix.length).trimEnd()}…`;
}

export function exampleQuickPickItems(
  entries: ReadonlyArray<ExampleEntry>
): ExamplePick[] {
  return entries.map((entry) => ({
    label: entry.title,
    description: hardwareBadge(entry.hardware),
    detail: compactExampleDescription(entry.description),
    id: entry.id,
    path: entry.path,
  }));
}

// Validate a parsed manifest: an array of entries with the required string fields and a folder path.
export function parseManifest(raw: unknown): ExampleEntry[] {
  if (!Array.isArray(raw)) {
    throw new Error("examples manifest must be an array");
  }
  return raw.map((value, index) => {
    if (!value || typeof value !== "object") {
      throw new Error(`examples manifest entry ${index} is not an object`);
    }
    const entry = value as Record<string, unknown>;
    for (const field of ["id", "title", "description", "path", "hardware"]) {
      if (typeof entry[field] !== "string" || !(entry[field] as string).length) {
        throw new Error(`examples manifest entry ${index} missing string '${field}'`);
      }
    }
    return {
      id: entry.id as string,
      title: entry.title as string,
      description: entry.description as string,
      path: entry.path as string,
      hardware: entry.hardware as string,
      tags: Array.isArray(entry.tags) ? (entry.tags as string[]) : undefined,
    };
  });
}
