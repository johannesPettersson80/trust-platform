import * as vscode from "vscode";

import { projectRequest } from "../projectRequest";
import type {
  ControlLesson,
  ControlLessonAvailability,
  ControlLessonMetadata,
  ControlLessonSummary,
} from "./controlLesson";
import type { ExampleEntry } from "./model";

// The gallery's entries come from the language server (`trust-lsp.examplesCatalog`, specification 25 §3.1): it reads
// the packaged manifest and every lesson's lesson.json, decides each lesson's availability and which entries can be
// copied, and which stored lesson ids the learner's progress keeps. This module asks and maps the answer.

interface CatalogEntry {
  readonly id: string;
  readonly title: string;
  readonly description: string;
  readonly path: string;
  readonly hardware: string;
  readonly tags?: ReadonlyArray<string>;
  readonly lesson?: ControlLessonMetadata;
  readonly lessonAvailability?: { readonly kind: "available" } | { readonly kind: "unavailable"; readonly reason: string };
  readonly copyable: boolean;
}

interface CatalogAnswer {
  readonly entries: ReadonlyArray<CatalogEntry>;
  readonly createdLessonIds: ReadonlyArray<string>;
}

/** The gallery and the lesson ids, of those stored as `created`, that the learner's progress keeps. */
export interface ExampleCatalog {
  readonly entries: ExampleEntry[];
  readonly createdLessonIds: string[];
}

/** Loads the gallery. A manifest that cannot be read rejects; one broken lesson is only unavailable. */
export async function loadExampleCatalog(
  context: vscode.ExtensionContext,
  created: ReadonlyArray<string> = []
): Promise<ExampleCatalog> {
  const answer = await projectRequest<CatalogAnswer>("trust-lsp.examplesCatalog", {
    root: vscode.Uri.joinPath(context.extensionUri, "media", "examples").fsPath,
    created,
  });
  if (!answer.ok) {
    throw new Error(answer.error);
  }
  return {
    entries: answer.entries.map(toExampleEntry),
    createdLessonIds: [...answer.createdLessonIds],
  };
}

/** The gallery's entries. */
export async function loadExamples(context: vscode.ExtensionContext): Promise<ExampleEntry[]> {
  return (await loadExampleCatalog(context)).entries;
}

function toExampleEntry(entry: CatalogEntry): ExampleEntry {
  return {
    id: entry.id,
    title: entry.title,
    description: entry.description,
    path: entry.path,
    hardware: entry.hardware,
    tags: entry.tags,
    lesson: entry.lesson,
    lessonAvailability: availabilityOf(entry),
    copyable: entry.copyable,
  };
}

function availabilityOf(entry: CatalogEntry): ControlLessonAvailability | undefined {
  const availability = entry.lessonAvailability;
  if (!availability) {
    return undefined;
  }
  return availability.kind === "available"
    ? { kind: "available", lesson: entry.lesson as ControlLesson }
    : { kind: "unavailable", reason: availability.reason, lesson: entry.lesson as ControlLessonSummary | undefined };
}
