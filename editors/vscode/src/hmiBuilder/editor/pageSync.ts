// Keeps one page document and its webview in step (specification 39, §3.1): the webview gets the
// page whenever the text changes, and an edit from the webview becomes the smallest text edit.
// Edits are written one at a time, in the order they came, each on top of the one before; an edit
// that comes while an earlier one is being written waits for it (§21.3 item 4). Only a change made
// outside the canvas refuses an edit.

import * as path from "path";
import * as vscode from "vscode";

import type { HmiPageModel } from "../drawing/model";
import type { HostToWebview } from "./messages";
import { PageFiles, sameModel, smallestEdit } from "./pageDocument";

export interface PreparedPageEdit {
  creates?: Array<{ uri: vscode.Uri; text: string }>;
  changes: Array<{
    document: vscode.TextDocument;
    version: number;
    edits: Array<{ range: vscode.Range; text: string }>;
  }>;
}

/** Why an edit is not written: the text it would replace changed outside the canvas. */
export const CHANGED_OUTSIDE =
  "The page or the project changed outside the canvas before this edit was written, so the edit was not applied and that change is kept.";

export class PageSync {
  /** The page the canvas holds and the document version whose text it is: the page last sent to the
   * canvas, or the edit last written from it. Undefined until the text first loads. */
  private known: { version: number; model: HmiPageModel } | undefined;
  /** The edits being written, one after the other. */
  private queue: Promise<void> = Promise.resolve();
  /** How many edits wait or are being written. */
  private waiting = 0;
  /** A page was read while edits waited; the canvas gets the page once they are written. */
  private owed = false;

  constructor(
    private readonly document: vscode.TextDocument,
    private readonly post: (message: HostToWebview) => void,
    private readonly files: PageFiles,
    /** Each page the text loads as, after the webview got it. */
    private readonly onLoaded: (model: HmiPageModel) => void = () => undefined,
    private readonly prepare?: (
      before: HmiPageModel,
      after: HmiPageModel,
      text: string,
    ) => Promise<PreparedPageEdit | undefined>,
  ) {}

  /** Reads the text and sends the page, or the errors when it does not load. */
  async refresh(): Promise<void> {
    const version = this.document.version;
    const read = await this.files.read(this.document.getText(), this.fileName());
    if (this.document.version !== version) return; // a newer refresh follows
    if (!read.ok) {
      this.post({ type: "invalid", errors: read.errors });
      return;
    }
    // The canvas already shows its own edits that wait to be written; this page would take it back
    // a step, so it gets the page once they are written.
    if (this.waiting > 0) {
      this.owed = true;
      return;
    }
    this.known = { version, model: read.model };
    this.post({ type: "page", model: read.model });
    this.onLoaded(read.model);
  }

  /** Writes an edited page back as the smallest text edit, after every edit that came before it;
   * an unchanged page changes nothing. Settles once this edit is written or refused. */
  apply(model: HmiPageModel): Promise<void> {
    this.waiting += 1;
    const done = this.queue
      .then(() => this.write(model))
      .catch((error: unknown) => this.refuse(error instanceof Error ? error.message : String(error)))
      .finally(() => {
        this.waiting -= 1;
        if (this.waiting === 0 && this.owed) {
          this.owed = false;
          void this.refresh();
        }
      });
    this.queue = done;
    return done;
  }

  /** Writes one edit, on top of the text the canvas's page is. */
  private async write(model: HmiPageModel): Promise<void> {
    const known = this.known;
    if (!known) return; // the text has not loaded yet: the canvas has no page to edit
    const version = this.document.version;
    // The text changed since the canvas got its page, and not by an edit of its own: this edit was
    // made on the page as it was. The refresh under way brings the canvas the changed page.
    if (known.version !== version) return this.refuse(CHANGED_OUTSIDE);
    if (sameModel(model, known.model)) return;
    const written = await this.files.write(model);
    if (!written.ok) {
      this.post({ type: "editRefused", errors: written.errors });
      return;
    }
    let prepared: PreparedPageEdit | undefined;
    try {
      prepared = await this.prepare?.(known.model, model, written.text);
    } catch (error) {
      this.refuse(error instanceof Error ? error.message : String(error));
      return;
    }
    // Nothing of the canvas's own is written meanwhile, as edits wait for each other: a newer text
    // was changed outside the canvas.
    if (
      this.document.version !== version ||
      prepared?.changes.some((change) => change.document.version !== change.version)
    ) {
      this.refuse(CHANGED_OUTSIDE);
      return;
    }
    const replacement = smallestEdit(this.document.getText(), written.text);
    if (!replacement) {
      // The text already says it: the next edit is written on top of this page.
      this.known = { version, model };
      return;
    }
    const edit = new vscode.WorkspaceEdit();
    for (const created of prepared?.creates ?? []) {
      edit.createFile(created.uri, { overwrite: false, ignoreIfExists: false });
      edit.insert(created.uri, new vscode.Position(0, 0), created.text);
    }
    edit.replace(
      this.document.uri,
      new vscode.Range(this.positionOfLine(replacement.startLine), this.positionOfLine(replacement.endLine)),
      replacement.text,
    );
    for (const change of prepared?.changes ?? []) {
      for (const entry of change.edits) edit.replace(change.document.uri, entry.range, entry.text);
    }
    if (!(await vscode.workspace.applyEdit(edit))) {
      this.refuse("VS Code could not apply the project edit.");
      return;
    }
    // The document now holds this page: the next edit is written on top of it.
    this.known = { version: this.document.version, model };
  }

  private refuse(message: string): void {
    this.post({ type: "editRefused", errors: [{ message }] });
  }

  /** The file name errors refer to: the path below the project, `hmi/pages/<page>.toml`. */
  private fileName(): string {
    const parts = this.document.uri.fsPath.split(path.sep);
    const hmi = parts.lastIndexOf("hmi");
    return hmi >= 0 ? parts.slice(hmi).join("/") : path.basename(this.document.uri.fsPath);
  }

  private positionOfLine(line: number): vscode.Position {
    return line < this.document.lineCount
      ? new vscode.Position(line, 0)
      : this.document.lineAt(this.document.lineCount - 1).range.end;
  }
}
