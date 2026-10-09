// Between the program and the HMI (specification 39, §9 and §10): Create Pages from Program
// proposes a page of the program's equipment; Add Missing Variables to Program computes the
// declarations and calls a page's bindings need and shows them as one change for approval.

import * as vscode from "vscode";
import { CREATE_PLC_COMMAND, createPlcCode } from "./generatePlc";
import { openHmiTexts } from "./textFiles";
import { allowWriting, pageRules } from "./writePolicy";

export const ADD_MISSING_VARIABLES_COMMAND = "trust-lsp.hmiBuilder.addMissingVariables";
export const CREATE_PAGES_COMMAND = "trust-lsp.hmiBuilder.createPagesFromProgram";
export const CREATE_SETTINGS_PAGE_COMMAND = "trust-lsp.hmiBuilder.createSettingsPage";

export interface CreatePagesArgs {
  /** The project folder; by default the active file's workspace folder, else the first. */
  folder?: string;
}

export interface CreatePagesResult {
  page?: string;
  count: number;
}

export interface CreateSettingsPageArgs extends CreatePagesArgs {
  /** The program instance; asked for when several have settings. */
  program?: string;
}

export interface CreateSettingsPageResult extends CreatePagesResult {
  skipped: Array<{ variable: string; message: string }>;
}

/** The project folder: the one asked for, else the active file's workspace folder, else the first. */
function projectFolder(args: CreatePagesArgs): vscode.Uri | undefined {
  if (args.folder) return vscode.Uri.parse(args.folder);
  const active = vscode.window.activeTextEditor?.document.uri;
  return (active && vscode.workspace.getWorkspaceFolder(active)?.uri) ?? vscode.workspace.workspaceFolders?.[0]?.uri;
}

async function exists(uri: vscode.Uri): Promise<boolean> {
  try {
    await vscode.workspace.fs.stat(uri);
    return true;
  } catch {
    return false;
  }
}

async function createPagesFromProgram(args: CreatePagesArgs): Promise<CreatePagesResult> {
  const folder = projectFolder(args);
  if (!folder) {
    void vscode.window.showWarningMessage("Open a truST project to create HMI pages from its program.");
    return { count: 0 };
  }
  const answer = (await vscode.commands.executeCommand("trust-lsp.hmiProposePage", {
    root_uri: folder.toString(),
    open: openHmiTexts(folder),
  })) as
    | {
        ok?: boolean;
        text?: string;
        count?: number;
        unmapped?: Array<{ instance: string; block: string }>;
        errors?: Array<{ message: string }>;
      }
    | undefined;
  if (answer?.ok !== true || typeof answer.text !== "string") {
    void vscode.window.showErrorMessage(
      answer?.errors?.[0]?.message ?? "The truST language server could not propose a page.",
    );
    return { count: 0 };
  }
  const count = answer.count ?? 0;
  const unmapped = answer.unmapped ?? [];
  if (unmapped.length)
    void vscode.window.showWarningMessage(
      `No visual definition for ${unmapped
        .slice(0, 3)
        .map((item) => `${item.instance} (${item.block})`)
        .join(", ")}. Add a component definition to give these FBs a picture.`,
    );
  if (count === 0) {
    if (!unmapped.length)
      void vscode.window.showInformationMessage(
        "No additional equipment pictures are needed. Existing views have been preserved.",
      );
    return { count };
  }
  const page = await writeNewPage(folder, "overview", answer.text);
  return { page: page.toString(), count };
}

/** A new page, never over an existing one, opened in the HMI Builder; a screen layout when the
 * project has none (§4.10). */
async function writeNewPage(folder: vscode.Uri, name: string, text: string): Promise<vscode.Uri> {
  const pages = vscode.Uri.joinPath(folder, "hmi", "pages");
  let page = vscode.Uri.joinPath(pages, `${name}.toml`);
  for (let number = 2; await exists(page); number += 1) page = vscode.Uri.joinPath(pages, `${name}-${number}.toml`);
  const screen = vscode.Uri.joinPath(folder, "hmi", "screen.toml");
  const edit = new vscode.WorkspaceEdit();
  if (!(await exists(screen))) {
    edit.createFile(screen, { overwrite: false, ignoreIfExists: false, contents: Buffer.from("format = 1\n", "utf8") });
  }
  edit.createFile(page, { overwrite: false, ignoreIfExists: false, contents: Buffer.from(text, "utf8") });
  if (!(await vscode.workspace.applyEdit(edit))) throw new Error("VS Code could not create the HMI page.");
  await vscode.commands.executeCommand("vscode.openWith", page, "trust-lsp.hmiBuilder");
  return page;
}

interface SettingsAnswer {
  ok?: boolean;
  text?: string;
  count?: number;
  choose?: string[];
  skipped?: CreateSettingsPageResult["skipped"];
  errors?: Array<{ message: string }>;
}

async function createSettingsPage(args: CreateSettingsPageArgs): Promise<CreateSettingsPageResult> {
  const folder = projectFolder(args);
  const nothing: CreateSettingsPageResult = { count: 0, skipped: [] };
  if (!folder) {
    void vscode.window.showWarningMessage("Open a truST project to create a settings page from its program.");
    return nothing;
  }
  const ask = async (program?: string): Promise<SettingsAnswer | undefined> =>
    (await vscode.commands.executeCommand("trust-lsp.hmiProposeSettingsPage", {
      root_uri: folder.toString(),
      ...(program ? { program } : {}),
    })) as SettingsAnswer | undefined;
  let answer = await ask(args.program);
  if (answer?.ok === true && answer.choose) {
    const program = await vscode.window.showQuickPick(answer.choose, {
      placeHolder: "The program whose settings the page sets",
    });
    if (!program) return nothing;
    answer = await ask(program);
  }
  if (answer?.ok !== true) {
    void vscode.window.showErrorMessage(
      answer?.errors?.[0]?.message ?? "The truST language server could not propose a settings page.",
    );
    return nothing;
  }
  const skipped = answer.skipped ?? [];
  const count = answer.count ?? 0;
  if (count === 0 || typeof answer.text !== "string") {
    void vscode.window.showInformationMessage(
      "The program declares no retained variable a control can set, so the page would be empty.",
    );
    return { count: 0, skipped };
  }
  const page = await writeNewPage(folder, "settings", answer.text);
  // The page exists to change these values: the runtime's write policy lets Engineers write them,
  // as each control asks (§12.2), so the page works without editing runtime.toml by hand.
  try {
    const { root, rules } = await pageRules(page);
    const added = await allowWriting(root, rules);
    if (added.length > 0)
      void vscode.window.showInformationMessage(
        `Engineers may now change ${added.length === 1 ? "this setting" : `these ${added.length} settings`} on the runtime display: the write rules are added to runtime.toml.`,
      );
  } catch (error) {
    void vscode.window.showWarningMessage(
      `The settings page is created, but its write rules are not: ${error instanceof Error ? error.message : String(error)}`,
    );
  }
  if (skipped.length > 0) {
    void vscode.window.showWarningMessage(
      `Not on the page: ${skipped.map((entry) => `${entry.variable} (${entry.message})`).join("; ")}`,
    );
  }
  return { page: page.toString(), count, skipped };
}

export interface AddMissingVariablesArgs {
  /** The page; by default the page open in the HMI Builder. */
  page?: string;
  /** Show the change for approval first (the default); false applies it at once. */
  confirm?: boolean;
}

export interface AddMissingVariablesResult {
  applied: boolean;
  added: Array<{ object: string; variable: string; type: string }>;
  skipped: Array<{ object: string; message: string }>;
  notes: Array<{ object: string; message: string }>;
}

interface Answer {
  ok?: boolean;
  errors?: Array<{ message: string }>;
  edit?: {
    changes?: Record<string, Array<{ range: { start: { line: number; character: number } }; newText: string }>>;
  };
  added?: AddMissingVariablesResult["added"];
  skipped?: AddMissingVariablesResult["skipped"];
  notes?: AddMissingVariablesResult["notes"];
}

/** The page open in the HMI Builder, or a page file open as text. */
function activePage(): vscode.Uri | undefined {
  const input = vscode.window.tabGroups.activeTabGroup.activeTab?.input;
  if (input instanceof vscode.TabInputCustom && input.viewType === "trust-lsp.hmiBuilder") return input.uri;
  const document = vscode.window.activeTextEditor?.document;
  return document && /[\\/]hmi[\\/](pages|popups)[\\/][^\\/]+\.toml$/.test(document.uri.fsPath)
    ? document.uri
    : undefined;
}

/** The project folder of a page: the folder that holds its `hmi/`. */
function projectOf(page: vscode.Uri): { root: vscode.Uri; file: string } | undefined {
  const parts = page.path.split("/");
  const hmi = parts.lastIndexOf("hmi");
  if (hmi < 1) return undefined;
  return { root: page.with({ path: parts.slice(0, hmi).join("/") }), file: parts.slice(hmi).join("/") };
}

async function addMissingVariables(args: AddMissingVariablesArgs): Promise<AddMissingVariablesResult> {
  const result: AddMissingVariablesResult = { applied: false, added: [], skipped: [], notes: [] };
  const page = args.page ? vscode.Uri.parse(args.page) : activePage();
  const project = page && projectOf(page);
  if (!page || !project) {
    void vscode.window.showWarningMessage(
      "Open a page of the HMI Builder to add the variables it needs to the program.",
    );
    return result;
  }
  const document = await vscode.workspace.openTextDocument(page);
  const answer = (await vscode.commands.executeCommand("trust-lsp.hmiAddMissingVariables", {
    text: document.getText(),
    file: project.file,
    root_uri: project.root.toString(),
  })) as Answer | undefined;
  if (answer?.ok !== true) {
    void vscode.window.showErrorMessage(
      answer?.errors?.[0]?.message ?? "The truST language server could not compute the variables.",
    );
    return result;
  }
  result.added = answer.added ?? [];
  result.skipped = answer.skipped ?? [];
  result.notes = answer.notes ?? [];
  // One change, shown for approval unless asked otherwise, applied as normal undoable edits (§10.2).
  const edit = new vscode.WorkspaceEdit();
  const metadata: vscode.WorkspaceEditEntryMetadata = {
    label: "Add missing variables to program",
    needsConfirmation: args.confirm !== false,
  };
  for (const [uri, edits] of Object.entries(answer.edit?.changes ?? {})) {
    for (const change of edits) {
      edit.insert(
        vscode.Uri.parse(uri),
        new vscode.Position(change.range.start.line, change.range.start.character),
        change.newText,
        metadata,
      );
    }
  }
  if (edit.size > 0) result.applied = await vscode.workspace.applyEdit(edit, { isRefactoring: true });
  const left = [...result.skipped, ...result.notes].map((entry) => `${entry.object}: ${entry.message}`);
  if (result.added.length === 0 && left.length === 0) {
    void vscode.window.showInformationMessage("The program already has every variable the page needs.");
  } else if (left.length > 0) {
    void vscode.window.showWarningMessage(
      `${left.length === 1 ? "One binding needs" : `${left.length} bindings need`} your attention: ${left.slice(0, 3).join("; ")}`,
    );
  }
  return result;
}

export function registerHmiProgramEdits(): vscode.Disposable {
  return vscode.Disposable.from(
    vscode.commands.registerCommand(CREATE_PLC_COMMAND, createPlcCode),
    vscode.commands.registerCommand(ADD_MISSING_VARIABLES_COMMAND, (args?: AddMissingVariablesArgs) =>
      addMissingVariables(args ?? {}),
    ),
    vscode.commands.registerCommand(CREATE_PAGES_COMMAND, (args?: CreatePagesArgs) =>
      createPagesFromProgram(args ?? {}),
    ),
    vscode.commands.registerCommand(CREATE_SETTINGS_PAGE_COMMAND, (args?: CreateSettingsPageArgs) =>
      createSettingsPage(args ?? {}),
    ),
  );
}
