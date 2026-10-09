import type { ControlLoopDefinition } from "./loopDefinition";
import { draftStorageKey } from "./engineerDraftStorage";
import { navigationStorageKey } from "./engineerNavigationStorage";
import { mimoAnalysisStorageKey, workflowResultStorageKey } from "./panelPersistence";
import { isRecord } from "./recordingFile";
import { expandStoredEvidence } from "./studyStorageCompaction";
import {
  archiveStudyState, openProjectStudy, projectStudyState, updateProjectStudy, type StudySection, type StudyState,
} from "./studyStore";

/** The part of the VS Code workspace state that Control Studio used before the study moved into the project. */
export interface WorkspaceMemento {
  get(key: string): unknown;
  update(key: string, value: unknown): PromiseLike<void>;
}

interface LegacySection {
  readonly key: string;
  readonly section: StudySection;
  /** The binding revision the value belongs to; undefined means the value is not tied to one. */
  readonly revision?: string;
  readonly value: unknown;
}

/**
 * Opens the loop's project study, moves any study still held in workspace state into it, and records the pointer to
 * the current study. Opening starts no test, tuning, validation or write to the runtime (A8.6).
 */
export async function openWorkspaceStudy(memento: WorkspaceMemento, projectRoot: string,
  loop: ControlLoopDefinition): Promise<StudyState> {
  await openProjectStudy(projectRoot, loop);
  await migrateWorkspaceStudy(memento, loop);
  await memento.update(studyPointerKey(loop), { schemaVersion: 1, projectRoot, bindingRevision: loop.revision });
  return projectStudyState(loop);
}

/**
 * Writes each legacy value into the project first and removes its workspace copy only after that write succeeded.
 * A value the project already holds is left in place. Returns how many values were moved.
 */
export async function migrateWorkspaceStudy(memento: WorkspaceMemento, loop: ControlLoopDefinition): Promise<number> {
  let moved = 0;
  for (const legacy of legacySections(memento, loop)) {
    if (legacy.revision === undefined || legacy.revision === loop.revision) {
      if (projectStudyState(loop)[legacy.section] !== undefined) continue;
      await updateProjectStudy(loop, legacy.section, legacy.value);
    } else {
      await archiveStudyState(loop, legacy.revision, singleSection(legacy.section, legacy.value));
    }
    await memento.update(legacy.key, undefined);
    moved += 1;
  }
  return moved;
}

export function studyPointerKey(loop: ControlLoopDefinition): string {
  return `trust.controlStudio.study.${loop.projectId}.${loop.id}`;
}

function legacySections(memento: WorkspaceMemento, loop: ControlLoopDefinition): LegacySection[] {
  const found: LegacySection[] = [];
  const draftKey = draftStorageKey(loop);
  const draft = memento.get(draftKey);
  if (draft !== undefined) {
    const expanded = expandStoredEvidence(draft);
    found.push({ key: draftKey, section: "draft", value: expanded,
      revision: isRecord(expanded) && typeof expanded.loopRevision === "string" ? expanded.loopRevision : undefined });
  }
  const historyKey = workflowResultStorageKey(loop);
  const history = memento.get(historyKey);
  if (history !== undefined) found.push({ key: historyKey, section: "history", value: history });
  const pageKey = navigationStorageKey(loop);
  const page = memento.get(pageKey);
  if (page !== undefined) found.push({ key: pageKey, section: "page", value: page, revision: loop.revision });
  const mimoKey = mimoAnalysisStorageKey(loop);
  const mimo = memento.get(mimoKey);
  if (mimo !== undefined) {
    found.push({ key: mimoKey, section: "mimo", value: mimo,
      revision: isRecord(mimo) && typeof mimo.loopRevision === "string" ? mimo.loopRevision : undefined });
  }
  return found;
}

function singleSection(section: StudySection, value: unknown): StudyState {
  switch (section) {
    case "draft": return { draft: value };
    case "history": return { history: value };
    case "page": return { page: value };
    case "mimo": return { mimo: value };
  }
}
