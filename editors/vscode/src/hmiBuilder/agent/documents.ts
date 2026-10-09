import { createHash } from "crypto";
import * as fs from "fs/promises";
import * as path from "path";
import * as vscode from "vscode";

export interface DocumentSnapshot {
  file: string;
  uri: string;
  exists: boolean;
  text: string;
  /** The observed state: the editor version (or "disk") and a content hash. Opening a page
   * changes it without changing the page; compare snapshots with `sameContent`. */
  revision: string;
  version: number | null;
  dirty: boolean;
}
const MAX_FILE_BYTES = 1024 * 1024;

/** Whether two snapshots hold the same page: both absent, or both present with the same text.
 * Opening, closing or saving a page in an editor changes its version or dirty state, not its
 * content, so it is no conflict (specification 43, repairs of 7 October 2026). */
export function sameContent(
  a: Pick<DocumentSnapshot, "exists" | "text">,
  b: Pick<DocumentSnapshot, "exists" | "text">,
): boolean {
  return a.exists === b.exists && a.text === b.text;
}
function cancelled(signal?: AbortSignal): void {
  if (signal?.aborted) throw new Error("CANCELLED: document snapshot cancelled.");
}
function inside(root: string, target: string): boolean {
  const relative = path.relative(root, target);
  return relative === "" || (!path.isAbsolute(relative) && relative !== ".." && !relative.startsWith(".." + path.sep));
}
function missing(error: unknown): boolean {
  return (error as NodeJS.ErrnoException).code === "ENOENT";
}

/** Extension-host file projects only. Remote SSH hosts have their own local file namespace;
 * a URI from another authority is explicitly unsupported instead of being treated as local. */
export class WorkspaceDocuments {
  constructor(readonly root: vscode.Uri) {}

  async uri(file: string, signal?: AbortSignal): Promise<vscode.Uri> {
    cancelled(signal);
    if (this.root.scheme !== "file" || this.root.authority !== "") {
      throw new Error("UNSUPPORTED_CONTEXT: unsupported workspace URI authority.");
    }
    if (
      !file ||
      file.includes("\\") ||
      file.includes(":") ||
      file.includes("\0") ||
      file.split("/").some((part) => !part || part === "." || part === "..")
    ) {
      throw new Error("INVALID_ARGUMENT: file must be a normalized workspace-relative path.");
    }
    const root = await fs.realpath(this.root.fsPath);
    const uri = vscode.Uri.joinPath(this.root, ...file.split("/"));
    // Resolve every existing ancestor, including symlinks below a not-yet-created file.
    let current = uri.fsPath;
    for (;;) {
      try {
        const canonical = await fs.realpath(current);
        if (!inside(root, canonical)) throw new Error("PROJECT_UNAVAILABLE: path escapes the workspace.");
        break;
      } catch (error) {
        if (!missing(error)) throw error;
        const parent = path.dirname(current);
        if (parent === current) throw error;
        current = parent;
      }
    }
    cancelled(signal);
    return uri;
  }

  async read(file: string, signal?: AbortSignal): Promise<DocumentSnapshot> {
    const uri = await this.uri(file, signal);
    let document = vscode.workspace.textDocuments.find((doc) => !doc.isClosed && doc.uri.toString() === uri.toString());
    let exists = true;
    let text = "";
    if (document?.isDirty) {
      text = document.getText();
    } else {
      try {
        const stat = await fs.stat(uri.fsPath);
        if (!stat.isFile()) throw new Error("INVALID_ARGUMENT: expected a file.");
        if (stat.size > MAX_FILE_BYTES) throw new Error("LIMIT_EXCEEDED: file exceeds 1 MiB.");
        // An editor model holds the text without a UTF-8 byte order mark; read the disk the same
        // way, so opening an unchanged page cannot look like a change of its content.
        text = document ? document.getText() : (await fs.readFile(uri.fsPath, "utf8")).replace(/^﻿/, "");
      } catch (error) {
        if (!missing(error)) throw error;
        exists = false;
        document = undefined;
      }
    }
    if (Buffer.byteLength(text, "utf8") > MAX_FILE_BYTES) throw new Error("LIMIT_EXCEEDED: file exceeds 1 MiB.");
    cancelled(signal);
    const version = document?.version ?? null;
    const hash = createHash("sha256")
      .update(exists ? "present\0" : "absent\0")
      .update(text)
      .digest("hex");
    return {
      file,
      uri: uri.toString(),
      exists,
      text,
      revision: `${version ?? "disk"}:${hash}`,
      version,
      dirty: document?.isDirty ?? false,
    };
  }
}
