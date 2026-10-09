// New HMI Screen (specification 39, §4.10): a screen layout and a first page from a template.
// The files are written through the language server's format implementation, so they are
// canonical, and an existing screen layout is never overwritten. The program gets what serves the
// screens (§18 Phase 5).

import * as path from "path";
import * as vscode from "vscode";

import type { HmiPageModel } from "../drawing/model";
import { prepareBundledLibraries } from "./libraryEdit";

export const NEW_SCREEN_COMMAND = "trust-lsp.hmiBuilder.newScreen";

/** A screen layout as the language server's format takes it. */
interface ScreenFile {
  size: [number, number];
  theme: "dark" | "light";
  symbols: "flat" | "rendered";
  top_bar?: { title: string; clock: boolean; alarms: boolean; user: boolean; page_title: boolean };
  navigation: { style: "side" | "tabs" | "none"; position: "left" | "right" };
}

interface Template {
  id: string;
  label: string;
  detail: string;
  screen: (title: string) => ScreenFile;
}

const TEMPLATES: Template[] = [
  {
    id: "operator-panel",
    label: "Operator panel",
    detail: "For a 1920 × 1080 panel: top bar with title, clock, alarms and user, side bar on the left",
    screen: (title) => ({
      size: [1688, 1024],
      theme: "dark",
      symbols: "flat",
      top_bar: { title, clock: true, alarms: true, user: true, page_title: true },
      navigation: { style: "side", position: "left" },
    }),
  },
  {
    id: "overview",
    label: "Overview",
    detail: "For a 1280 × 800 panel: top bar with title, clock and user, tabs",
    screen: (title) => ({
      size: [1280, 696],
      theme: "dark",
      symbols: "flat",
      top_bar: { title, clock: true, alarms: false, user: true, page_title: false },
      navigation: { style: "tabs", position: "left" },
    }),
  },
  {
    id: "single-page",
    label: "Single page",
    detail: "1024 × 768, without top bar or navigation",
    screen: () => ({
      size: [1024, 768],
      theme: "dark",
      symbols: "flat",
      navigation: { style: "none", position: "left" },
    }),
  },
];

const FIRST_PAGE: HmiPageModel = {
  title: "Overview",
  order: 0,
  icon: "overview",
  object: [
    {
      id: "heading",
      type: "text",
      variant: "heading",
      at: [40, 32],
      size: [600, 40],
      rotation: 0,
      text: "Overview",
      bind: {},
    },
  ],
  pipe: [],
};

interface NewScreenArgs {
  /** The template's id; asked for when missing. */
  template?: string;
  /** The project folder; the active file's workspace folder, or the first one, when missing. */
  folder?: string;
}

export function registerHmiTemplates(extensionPath: string): vscode.Disposable {
  return vscode.commands.registerCommand(NEW_SCREEN_COMMAND, (args?: NewScreenArgs) =>
    newScreen(args ?? {}, extensionPath),
  );
}

async function newScreen(args: NewScreenArgs, extensionPath: string): Promise<void> {
  const folder = args.folder ? vscode.Uri.parse(args.folder) : projectFolder();
  if (!folder) {
    void vscode.window.showErrorMessage("Open a truST project folder first.");
    return;
  }
  const template = args.template ? TEMPLATES.find((candidate) => candidate.id === args.template) : await pickTemplate();
  if (!template) return;
  const screenFile = vscode.Uri.joinPath(folder, "hmi", "screen.toml");
  if (await exists(screenFile)) {
    void vscode.window.showInformationMessage(
      "This project already has a screen layout (hmi/screen.toml); it is left as it is.",
    );
    return;
  }
  // The title is the folder's name in words: `room-heating` is "Room heating" (§4 item 8).
  const title = path
    .basename(folder.fsPath)
    .replace(/[-_]+/g, " ")
    .trim()
    .replace(/^./, (letter) => letter.toUpperCase());
  const screen = await formatted("screen", template.screen(title));
  const page = await formatted("page", FIRST_PAGE);
  if (screen === undefined || page === undefined) {
    void vscode.window.showErrorMessage(
      "The truST language server is not running yet, so the screen could not be created.",
    );
    return;
  }
  const pageFile = vscode.Uri.joinPath(folder, "hmi", "pages", "overview.toml");
  // Written to disk at once, as the libraries are: a page left in an unsaved buffer showed the
  // empty file's problem until the user saved it.
  const edit = new vscode.WorkspaceEdit();
  edit.createFile(screenFile, { overwrite: false, ignoreIfExists: false, contents: Buffer.from(screen, "utf8") });
  if (!(await exists(pageFile))) {
    edit.createFile(pageFile, { overwrite: false, ignoreIfExists: false, contents: Buffer.from(page, "utf8") });
  }
  const verifyLibraries = await prepareBundledLibraries(
    folder,
    extensionPath,
    [
      { name: "Hmi", folder: "hmi" },
      { name: "Plant", folder: "plant" },
    ],
    edit,
  );
  await verifyLibraries();
  if (!(await vscode.workspace.applyEdit(edit)))
    throw new Error("The HMI files could not be created. Existing project files were preserved.");
  await vscode.commands.executeCommand("vscode.openWith", pageFile, "trust-lsp.hmiBuilder");
}

async function pickTemplate(): Promise<Template | undefined> {
  const picked = await vscode.window.showQuickPick(
    TEMPLATES.map((template) => ({ label: template.label, detail: template.detail, template })),
    { title: "New HMI Screen", placeHolder: "Choose a screen layout to start from" },
  );
  return picked?.template;
}

/** The project folder: the active file's workspace folder, else the first one. */
export function projectFolder(): vscode.Uri | undefined {
  const active = vscode.window.activeTextEditor?.document.uri;
  const folder = active ? vscode.workspace.getWorkspaceFolder(active) : undefined;
  return (folder ?? vscode.workspace.workspaceFolders?.[0])?.uri;
}

export async function exists(file: vscode.Uri): Promise<boolean> {
  try {
    await vscode.workspace.fs.stat(file);
    return true;
  } catch {
    return false;
  }
}

/** Canonical text from the language server, or undefined when it cannot give it. */
async function formatted(kind: "screen" | "page", model: unknown): Promise<string | undefined> {
  try {
    const answer = (await vscode.commands.executeCommand("trust-lsp.hmiFileFormat", { kind, model })) as
      { ok?: boolean; text?: string } | undefined;
    return answer?.ok === true ? answer.text : undefined;
  } catch {
    return undefined;
  }
}
