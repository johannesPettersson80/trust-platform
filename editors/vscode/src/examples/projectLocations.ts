/** Persistable identity for one copied control lesson project. */
export interface ControlLessonProjectLocation {
  readonly lessonId: string;
  readonly projectPath: string;
}

export const CONTROL_LESSON_PROJECTS_KEY = "trust.examples.controlLessonProjects";

interface ProjectLocationState {
  get<T>(key: string, fallback?: T): T;
  update(key: string, value: unknown): Thenable<void> | Promise<void>;
}

export type ControlLessonProjectRecovery =
  | { readonly kind: "present"; readonly projectPath: string }
  | { readonly kind: "missing"; readonly projectPath: string };

/** Add one location immutably; an exact duplicate is retained once. */
export function recordControlLessonProject(
  records: ReadonlyArray<ControlLessonProjectLocation>,
  lessonId: string,
  projectPath: string
): ControlLessonProjectLocation[] {
  const duplicate = records.some((record) =>
    record.lessonId === lessonId && record.projectPath === projectPath
  );
  return duplicate ? [...records] : [...records, { lessonId, projectPath }];
}

export function readControlLessonProjects(
  state: Pick<ProjectLocationState, "get">
): ControlLessonProjectLocation[] {
  const raw = state.get<unknown>(CONTROL_LESSON_PROJECTS_KEY, []);
  if (!Array.isArray(raw)) return [];
  return raw.filter((record): record is ControlLessonProjectLocation =>
    Boolean(record) && typeof record === "object" &&
    typeof (record as { lessonId?: unknown }).lessonId === "string" &&
    (record as { lessonId: string }).lessonId.length > 0 &&
    typeof (record as { projectPath?: unknown }).projectPath === "string" &&
    (record as { projectPath: string }).projectPath.length > 0
  );
}

export async function persistControlLessonProject(
  state: ProjectLocationState,
  lessonId: string,
  projectPath: string
): Promise<void> {
  await state.update(
    CONTROL_LESSON_PROJECTS_KEY,
    recordControlLessonProject(readControlLessonProjects(state), lessonId, projectPath)
  );
}

/** Classify a retained location without deleting it or guessing another copy. */
export async function controlLessonProjectRecovery(
  record: ControlLessonProjectLocation,
  exists: (projectPath: string) => Promise<boolean>
): Promise<ControlLessonProjectRecovery> {
  return {
    kind: await exists(record.projectPath) ? "present" : "missing",
    projectPath: record.projectPath,
  };
}
