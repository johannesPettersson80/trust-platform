import * as vscode from "vscode";

import { evaluateControlLessonAvailability } from "./controlLesson";
import { parseManifest, type ExampleEntry } from "./model";

export async function loadExamples(
  context: vscode.ExtensionContext
): Promise<ExampleEntry[]> {
  const root = vscode.Uri.joinPath(context.extensionUri, "media", "examples");
  const data = await vscode.workspace.fs.readFile(
    vscode.Uri.joinPath(root, "manifest.json")
  );
  const entries = parseManifest(JSON.parse(Buffer.from(data).toString("utf8")));
  return Promise.all(
    entries.map(async (entry) => {
      if (!entry.tags?.includes("control")) {
        return entry;
      }
      const lessonData = await vscode.workspace.fs.readFile(
        vscode.Uri.joinPath(root, entry.path, "lesson.json")
      );
      const availability = evaluateControlLessonAvailability(
        JSON.parse(Buffer.from(lessonData).toString("utf8")),
        entry.id
      );
      return {
        ...entry,
        lesson: availability.lesson,
        lessonAvailability: availability,
      };
    })
  );
}
