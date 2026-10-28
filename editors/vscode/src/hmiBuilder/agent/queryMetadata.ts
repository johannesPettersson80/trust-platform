import * as vscode from "vscode";

/** A builder panel's selection, as the authoring commands read it (`PanelSelection` in trust-lsp). */
export interface Selection {
  file: string;
  ids: string[];
  documentVersion: number;
}

interface PanelEntry {
  active: boolean;
  selection: Selection;
}

const selections = new Map<string, Map<object, PanelEntry>>();

/** Selection authority is the actual project editor panel, never a caller-provided root. */
export function recordSelection(root: vscode.Uri, owner: object, selection: Selection, active: boolean): void {
  let panels = selections.get(root.toString());
  if (!panels) {
    panels = new Map();
    selections.set(root.toString(), panels);
  }
  panels.set(owner, { active, selection: { ...selection, ids: [...selection.ids] } });
}

export function forgetSelection(root: vscode.Uri, owner: object): void {
  const panels = selections.get(root.toString());
  panels?.delete(owner);
  if (!panels?.size) selections.delete(root.toString());
}

/** Every builder panel's selection in the project; the command chooses among them. */
export function selectionsOf(root: vscode.Uri): Array<Selection & { active: boolean }> {
  return [...(selections.get(root.toString())?.values() ?? [])].map((entry) => ({
    ...entry.selection,
    ids: [...entry.selection.ids],
    active: entry.active,
  }));
}
