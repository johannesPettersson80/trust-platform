// Explicit design-to-source transaction. Drawing edits never call this module.
import * as vscode from "vscode";
import { openHmiTexts } from "./textFiles";
import { allowWriting, type WriteRule } from "./writePolicy";

export const CREATE_PLC_COMMAND = "trust-lsp.hmiBuilder.createPlcCode";
interface Change {
  range: { start: { line: number; character: number }; end: { line: number; character: number } };
  newText: string;
}
interface Answer {
  ok: boolean;
  needsControlCycle?: boolean;
  needsScreenTask?: Array<{ id: string; label: string; description: string }>;
  errors?: Array<{ message: string }>;
  expected?: Record<string, string>;
  notes?: Array<{ instance?: string; member?: string; message: string }>;
  creates?: Array<{ uri: string; text: string }>;
  edit?: { changes?: Record<string, Change[]> };
  /** The write rules the runtime display needs to operate the code's PID loops (§10.5). */
  writes?: WriteRule[];
}
const active = new Set<string>();

/** Whether `uri` names an existing file with nothing in it. */
async function isEmptyFile(uri: vscode.Uri): Promise<boolean> {
  try {
    return (await vscode.workspace.fs.stat(uri)).size === 0;
  } catch {
    return false;
  }
}
let openConnections: vscode.OutputChannel | undefined;

/** The control cycle typed in milliseconds, or why it cannot be one: the language server checks it
 * (`trust-lsp.hmiChecks`, specification 39 §10.3 item 3). */
async function controlCycle(text: string): Promise<{ milliseconds?: number; problem?: string }> {
  const answer = await vscode.commands.executeCommand<{
    ok?: boolean;
    error?: string;
    milliseconds?: number;
    problem?: string;
  }>("trust-lsp.hmiChecks", { action: "cycle", text });
  return answer?.ok === true ? answer : { problem: answer?.error ?? "The truST language server is not running." };
}

/** What is still to connect, each on its own line: a notification cuts a long list short. */
async function showOpenConnections(notes: Array<{ instance?: string; message: string }>): Promise<void> {
  const count = notes.length === 1 ? "1 connection is" : `${notes.length} connections are`;
  const show = await vscode.window.showWarningMessage(`PLC code created. ${count} still open.`, "Show");
  if (show !== "Show") return;
  openConnections ??= vscode.window.createOutputChannel("truST HMI: open connections");
  openConnections.clear();
  for (const note of notes) openConnections.appendLine(`${note.instance ?? ""}: ${note.message}`);
  openConnections.show(true);
}

export async function createPlcCode(args: { folder?: string } = {}): Promise<boolean> {
  const root = args.folder ? vscode.Uri.parse(args.folder) : vscode.workspace.workspaceFolders?.[0]?.uri;
  if (!root) {
    void vscode.window.showWarningMessage("Open a truST project first.");
    return false;
  }
  const key = root.toString();
  if (active.has(key)) return false;
  active.add(key);
  try {
    let versions = new Map<vscode.TextDocument, number>();
    let cycle_ms: number | undefined;
    const prepare = (screen_task?: string): Thenable<Answer | undefined> => {
      versions = new Map(
        vscode.workspace.textDocuments
          .filter((document) => document.uri.path.startsWith(`${root.path}/`))
          .map((document) => [document, document.version]),
      );
      return vscode.commands.executeCommand<Answer>("trust-lsp.hmiAddMissingVariables", {
        root_uri: key,
        open: openHmiTexts(root),
        generate_project: true,
        screen_task,
        cycle_ms,
      });
    };
    let answer = await prepare();
    if (answer?.needsControlCycle) {
      const period = await vscode.window.showInputBox({
        title: "PLC control cycle",
        prompt: "This project has no task yet. Choose its cycle in milliseconds.",
        validateInput: async (text) => (await controlCycle(text)).problem,
      });
      if (period === undefined) return false;
      cycle_ms = (await controlCycle(period)).milliseconds;
      answer = await prepare();
    }
    if (answer?.needsScreenTask) {
      const selected = await vscode.window.showQuickPick(answer.needsScreenTask, {
        title: "Which PLC task should update the HMI?",
        placeHolder: "Choose an existing task; its period remains unchanged",
      });
      if (!selected) return false;
      answer = await prepare(selected.id);
    }
    if (!answer?.ok)
      throw new Error(answer?.errors?.map((error) => error.message).join("\n") || "PLC code could not be created.");
    const targets = [];
    for (const [uri, changes] of Object.entries(answer.edit?.changes ?? {})) {
      const document = await vscode.workspace.openTextDocument(vscode.Uri.parse(uri));
      if (answer.expected?.[uri] !== document.getText())
        throw new Error("Source changed while preparing PLC code. Your work was preserved.");
      targets.push({ document, version: document.version, changes, dirty: document.isDirty });
    }
    // New files are written to disk with their text, so closing VS Code cannot leave an empty file.
    const edit = new vscode.WorkspaceEdit();
    for (const created of answer.creates ?? []) {
      const uri = vscode.Uri.parse(created.uri);
      // An empty file of that name, left by an earlier run whose text was never saved, is taken over.
      const overwrite = await isEmptyFile(uri);
      edit.createFile(uri, { overwrite, ignoreIfExists: false, contents: Buffer.from(created.text, "utf8") });
    }
    for (const target of targets) {
      if (target.document.version !== target.version)
        throw new Error("Source changed while preparing PLC code. Your work was preserved.");
      for (const change of target.changes)
        edit.replace(
          target.document.uri,
          new vscode.Range(
            change.range.start.line,
            change.range.start.character,
            change.range.end.line,
            change.range.end.character,
          ),
          change.newText,
        );
    }
    if ([...versions].some(([document, version]) => document.version !== version))
      throw new Error(
        "The design changed while preparing PLC code. Your changes were preserved; create the code again when ready.",
      );
    if (edit.size === 0) {
      void vscode.window.showInformationMessage("PLC code is already up to date.");
      return true;
    }
    if (!(await vscode.workspace.applyEdit(edit))) throw new Error("VS Code could not apply the generated PLC code.");
    // The generated code is saved like the files it creates; a file with unsaved changes of the
    // user's own stays unsaved, for the user to review.
    for (const target of targets) if (!target.dirty) await target.document.save();
    // Operators run a PID loop from its panel: its Manual switch, setpoint and manual output need
    // their write rules, which runtime.toml gets unless it has them already (§12.2).
    if (answer.writes?.length) {
      try {
        await allowWriting(root, answer.writes);
      } catch (error) {
        void vscode.window.showWarningMessage(
          `PLC code created, but the PID loops' write rules are not: ${error instanceof Error ? error.message : String(error)}`,
        );
      }
    }
    if (answer.notes?.length) void showOpenConnections(answer.notes);
    else
      void vscode.window.showInformationMessage("PLC code created. Build the project when you are ready to check it.");
    return true;
  } catch (error) {
    void vscode.window.showErrorMessage(error instanceof Error ? error.message : String(error));
    return false;
  } finally {
    active.delete(key);
  }
}
