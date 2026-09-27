import * as vscode from "vscode";
import * as fs from "fs/promises";
import * as path from "path";
import { createHash } from "crypto";
import type { ExampleEntry } from "./model";

export const BUNDLED_EXAMPLE_SCHEME = "trust-example-source";
const MAX_PREVIEW_BYTES = 2 * 1024 * 1024;

/** Immutable, read-only source previews; project creation stays with copyExample. */
export class BundledExamplePreviews implements vscode.TextDocumentContentProvider {
  private readonly snapshots = new Map<string, string>();

  constructor(private readonly examplesRoot: vscode.Uri) {}

  async prepare(entry: Pick<ExampleEntry, "id" | "path">, target: string, allowedTargets: readonly string[]): Promise<vscode.Uri> {
    if (!allowedTargets.includes(target) || !relativeFile(target) || !relativeFile(entry.path)) {
      throw new Error("This file is not an allowed reference in the selected bundled example.");
    }
    const root = await fs.realpath(this.examplesRoot.fsPath);
    const project = await fs.realpath(path.join(root, entry.path));
    if (!within(root, project)) throw new Error("The bundled example is outside the examples directory.");
    const file = await fs.realpath(path.join(project, target));
    if (!within(project, file)) throw new Error("The referenced file is outside the selected example.");
    const stat = await fs.stat(file);
    if (!stat.isFile() || stat.size > MAX_PREVIEW_BYTES) throw new Error("This file cannot be shown as a text preview.");
    const bytes = await fs.readFile(file);
    if (bytes.length > MAX_PREVIEW_BYTES || bytes.includes(0)) throw new Error("This file cannot be shown as a text preview.");
    const content = new TextDecoder("utf-8", { fatal: true }).decode(bytes);
    const revision = createHash("sha256").update(bytes).digest("hex");
    const uri = vscode.Uri.from({ scheme: BUNDLED_EXAMPLE_SCHEME, authority: "bundled",
      path: `/${entry.id}/${target}`, query: `revision=${revision}` });
    this.snapshots.set(uri.toString(), content);
    return uri;
  }

  provideTextDocumentContent(uri: vscode.Uri): string {
    const content = this.snapshots.get(uri.toString());
    if (content === undefined) throw new Error("Open this bundled source from its lesson page first.");
    return content;
  }
}

function relativeFile(value: string): boolean {
  return value.length > 0 && !/[\\:%?#\u0000-\u001f]/.test(value) && !path.isAbsolute(value) &&
    value.split("/").every(part => part.length > 0 && part !== "." && part !== "..");
}

function within(root: string, file: string): boolean {
  const relative = path.relative(root, file);
  return relative !== "" && relative !== ".." && !relative.startsWith(`..${path.sep}`) && !path.isAbsolute(relative);
}
