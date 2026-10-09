// Project edits the language server proposes for equipment and controller records (specification
// 39, §21.3 and §21.4 item 6; specification 35 §17.17): `plant.toml` and the generated
// declarations for an instance's choices, or a record's own declaration. An edit applies only to
// the texts it was made from; files without unsaved changes of the user's own are saved. Shared by
// the HMI Builder's inspector and Control Studio's Keep in project.
import * as vscode from "vscode";
import { openHmiTexts } from "./textFiles";

interface NativeEdit {
  range: { start: { line: number; character: number }; end: { line: number; character: number } };
  newText: string;
}

/** A proposed project edit: the changes, the texts they were made from, and the files they create. */
export interface ProjectEditAnswer {
  ok: boolean;
  creates?: Array<{ uri: string; text: string }>;
  expected?: Record<string, string>;
  edit?: { changes?: Record<string, NativeEdit[]> };
  errors?: Array<{ message: string }>;
  error?: string;
}

/** Changed choices of an equipment instance; `null` or an empty text removes a choice. */
export interface EquipmentChanges {
  settings?: Record<string, boolean | number | string | null>;
  dependencies?: Record<string, string>;
  signals?: Record<string, string>;
}

/** The text of an answer's first refusal. */
export function refusal(answer: ProjectEditAnswer | undefined, fallback: string): string {
  return answer?.errors?.[0]?.message ?? answer?.error ?? fallback;
}

/** Whether a setting's value in the project and a value from the PLC are the same: numbers within
 * a rounding difference, anything else exactly. */
export function sameSetting(project: unknown, running: boolean | number | string): boolean {
  if (typeof project === "number" && typeof running === "number")
    return Math.abs(project - running) <= 1e-9 * Math.max(1, Math.abs(project), Math.abs(running));
  return project === running;
}

/** `plant.toml` as the editor or the disk has it; undefined when the project has none. */
export async function plantManifest(project: vscode.Uri): Promise<string | undefined> {
  const uri = vscode.Uri.joinPath(project, "plant.toml");
  const open = vscode.workspace.textDocuments.find((document) => document.uri.toString() === uri.toString());
  if (open) return open.getText();
  try {
    return Buffer.from(await vscode.workspace.fs.readFile(uri)).toString("utf8");
  } catch (error) {
    if (error instanceof vscode.FileSystemError && error.code === "FileNotFound") return undefined;
    throw error;
  }
}

/** Applies a proposed edit when every file it changes still has the text it was made from and
 * `unchanged` confirms the caller's own documents did not change meanwhile; throws why not. */
export async function applyProjectEdit(
  answer: ProjectEditAnswer,
  unchanged: () => boolean = () => true,
): Promise<void> {
  const targets = [];
  for (const [uri, edits] of Object.entries(answer.edit?.changes ?? {})) {
    const document = await vscode.workspace.openTextDocument(vscode.Uri.parse(uri));
    if (answer.expected?.[uri] !== document.getText())
      throw new Error("The project changed while preparing this edit. Your changes were preserved.");
    targets.push({ document, version: document.version, edits, dirty: document.isDirty });
  }
  if (!unchanged() || targets.some((target) => target.document.version !== target.version))
    throw new Error("The project changed while preparing this edit. Your changes were preserved.");
  const edit = new vscode.WorkspaceEdit();
  for (const created of answer.creates ?? [])
    edit.createFile(vscode.Uri.parse(created.uri), {
      overwrite: false,
      ignoreIfExists: false,
      contents: Buffer.from(created.text, "utf8"),
    });
  for (const target of targets)
    for (const change of target.edits)
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
  // An answer that changes and creates nothing (the project already holds the values) needs no
  // edit; a file creation alone does not count in `edit.size`.
  const changes = (answer.creates?.length ?? 0) + targets.reduce((count, target) => count + target.edits.length, 0);
  if (changes === 0) return;
  if (!(await vscode.workspace.applyEdit(edit))) throw new Error("VS Code could not apply the project edit.");
  // Saved as Create PLC code saves (plant.toml has no editor, so an unsaved change to it was
  // lost); a file with unsaved changes of the user's own stays unsaved, for the user to review.
  for (const target of targets) if (!target.dirty) await target.document.save();
}

/** The language server's change of an instance's choices against `manifest`, applied; `page` is
 * the page being edited, unsaved, whose version must not change meanwhile. */
export async function applyEquipmentChanges(
  project: vscode.Uri,
  instance: string,
  changes: EquipmentChanges,
  manifest: string | undefined,
  page?: vscode.TextDocument,
): Promise<void> {
  const version = page?.version;
  const answer = await vscode.commands.executeCommand<ProjectEditAnswer>("trust-lsp.hmiEquipment", {
    instance,
    root_uri: project.toString(),
    open: openHmiTexts(project),
    manifest,
    changes,
    ...(page ? { text: page.getText(), file: page.uri.path.slice(project.path.length + 1) } : {}),
  });
  if (!answer?.ok) throw new Error(refusal(answer, "The equipment rejected these project settings."));
  await applyProjectEdit(answer, () => page?.version === version);
}

/** Keeps values the PLC has as an instance's settings in the project (§21.4 item 6): those that
 * differ from the project's values, as one project edit. Values its definition has no setting for
 * are answered as `unknown`. */
export async function keepEquipmentSettings(
  project: vscode.Uri,
  instance: string,
  values: Readonly<Record<string, boolean | number | string>>,
): Promise<{ kept: string[]; unknown: string[] }> {
  const manifest = await plantManifest(project);
  const described = await vscode.commands.executeCommand<
    ProjectEditAnswer & { owned?: boolean; settings?: Array<{ member: string; value: unknown }> }
  >("trust-lsp.hmiEquipment", { instance, root_uri: project.toString(), open: openHmiTexts(project), manifest });
  if (!described?.ok) throw new Error(refusal(described, "Equipment configuration is unavailable."));
  if (!described.owned) throw new Error(`${instance} is declared in your PLC code: copy the values there.`);
  const known = new Map((described.settings ?? []).map((setting) => [setting.member, setting.value]));
  const settings: Record<string, boolean | number | string> = {};
  for (const [member, value] of Object.entries(values))
    if (known.has(member) && !sameSetting(known.get(member), value)) settings[member] = value;
  const kept = Object.keys(settings);
  if (kept.length) {
    if ((await plantManifest(project)) !== manifest)
      throw new Error("Project settings changed meanwhile. Keep in project again to review the current values.");
    await applyEquipmentChanges(project, instance, { settings }, manifest);
  }
  return { kept, unknown: Object.keys(values).filter((member) => !known.has(member)) };
}
