import * as vscode from "vscode";

import { openExistingProjectFolder } from "../projectOpening";
import {
  controlLessonProjectRecovery,
  type ControlLessonProjectLocation,
} from "./projectLocations";

export type RetainedProjectReopenResult =
  | { readonly kind: "opened"; readonly projectPath: string }
  | { readonly kind: "missing"; readonly projectPath: string }
  | { readonly kind: "cancelled" };

export async function openRetainedControlLessonProject(
  _context: unknown,
  lessonId: string,
  records: ReadonlyArray<ControlLessonProjectLocation>,
  choose: (paths: ReadonlyArray<string>) => Promise<string | undefined> = async (paths) => {
    const selected = await vscode.window.showQuickPick(paths, { placeHolder: "Choose a saved lesson project" });
    return selected;
  },
  exists: (projectPath: string) => Promise<boolean> = async (projectPath) => {
    try {
      await vscode.workspace.fs.stat(vscode.Uri.file(projectPath));
      return true;
    } catch {
      return false;
    }
  },
  open: (projectPath: string) => Promise<boolean> = (projectPath) => openExistingProjectFolder(vscode.Uri.file(projectPath)),
): Promise<RetainedProjectReopenResult> {
  const paths = records.filter((record) => record.lessonId === lessonId).map((record) => record.projectPath);
  const selected = await choose(paths);
  if (!selected) return { kind: "cancelled" };
  if (!paths.includes(selected)) return { kind: "cancelled" };
  const recovery = await controlLessonProjectRecovery({ lessonId, projectPath: selected }, exists);
  if (recovery.kind === "missing") return recovery;
  return (await open(selected))
    ? { kind: "opened", projectPath: selected }
    : { kind: "cancelled" };
}
