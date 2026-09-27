import * as nodePath from "path";
import * as vscode from "vscode";

import type { ExampleEntry } from "./model";

type ManifestEntry = { title: string; path: string };

/**
 * Adapt only catalog-resolved sibling lesson links for a copied lesson. A copied
 * project has no sibling folders, while external, anchor, and unresolved links
 * remain authored markdown and must not be rewritten.
 */
export async function adaptCopiedLessonReadme(
  context: vscode.ExtensionContext,
  entry: ExampleEntry,
  destination: vscode.Uri
): Promise<void> {
  const readme = vscode.Uri.joinPath(destination, "README.md");
  const source = Buffer.from(await vscode.workspace.fs.readFile(readme)).toString("utf8");
  const matches = Array.from(source.matchAll(/(?<!!)\[([^\]\n]+)\]\(([^)\n]+)\)/g));
  if (!matches.length) return;

  const manifestUri = vscode.Uri.joinPath(context.extensionUri, "media", "examples", "manifest.json");
  const manifest = JSON.parse(Buffer.from(await vscode.workspace.fs.readFile(manifestUri)).toString("utf8")) as ManifestEntry[];
  let output = "";
  let cursor = 0;
  for (const match of matches) {
    const rawTarget = match[2];
    const targetPath = resolveRelativeLessonPath(entry.path, rawTarget);
    const target = targetPath ? manifest.find((candidate) => candidate.path === targetPath) : undefined;
    if (!target) continue;

    const number = await lessonNumber(context, target.path, /^Lesson\s+(\d+)$/i.exec(match[1])?.[1]);
    if (!number) continue;
    output += source.slice(cursor, match.index);
    output += `${match[1]} (to open it, use truST > Start from example and select Lesson ${number})`;
    cursor = (match.index ?? 0) + match[0].length;
  }
  if (cursor === 0) return;
  output += source.slice(cursor);
  await vscode.workspace.fs.writeFile(readme, Buffer.from(output, "utf8"));
}

function resolveRelativeLessonPath(entryPath: string, rawTarget: string): string | undefined {
  if (!rawTarget || rawTarget.startsWith("#") || rawTarget.startsWith("/") || /^[a-z][a-z\d+.-]*:/i.test(rawTarget)) {
    return undefined;
  }
  const normalized = nodePath.posix.normalize(
    nodePath.posix.join(entryPath, rawTarget.replace(/^\.\//, ""))
  ).replace(/\/README\.md$/, "");
  return normalized.startsWith("control/") ? normalized : undefined;
}

async function lessonNumber(context: vscode.ExtensionContext, relativePath: string, fallback?: string): Promise<string | undefined> {
  try {
    const uri = vscode.Uri.joinPath(context.extensionUri, "media", "examples", relativePath, "lesson.json");
    const metadata = JSON.parse(Buffer.from(await vscode.workspace.fs.readFile(uri)).toString("utf8")) as { sequence?: number };
    return typeof metadata.sequence === "number" ? String(metadata.sequence) : fallback;
  } catch {
    return fallback;
  }
}
