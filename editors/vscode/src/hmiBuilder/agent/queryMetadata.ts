import * as vscode from "vscode";
import type { HmiPageModel, DrawFrame } from "../drawing/model";
import { withValueFormat, formatPath } from "../drawing/valueFormats";
import type { WorkspaceDocuments } from "./documents";
import type { NativeCommand } from "./queries";

interface Selection {
  file: string;
  ids: string[];
  documentVersion: number;
}
const selections = new Map<string, Map<object, { active: boolean; selection: Selection }>>();
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
export function contextSelection(root: vscode.Uri, file?: string): Selection | null {
  const panels = [...(selections.get(root.toString())?.values() ?? [])];
  const matches = panels.filter((entry) => (file ? entry.selection.file === file : entry.active));
  return matches.length === 1 ? { ...matches[0].selection, ids: [...matches[0].selection.ids] } : null;
}

/** Enrichment reuses native definitions and shared display inheritance, without parsing TOML. */
export async function inspectMetadata(
  model: HmiPageModel,
  file: string,
  documents: WorkspaceDocuments,
  native: NativeCommand,
  text?: string,
): Promise<Record<string, unknown>> {
  const root_uri = documents.root.toString();
  const open: Record<string, string> = {};
  for (const doc of vscode.workspace.textDocuments) {
    if (
      doc.isDirty &&
      doc.uri.scheme === documents.root.scheme &&
      doc.uri.authority === documents.root.authority &&
      doc.uri.path.startsWith(documents.root.path.replace(/\/$/, "") + "/hmi/")
    )
      open[doc.uri.path.slice(documents.root.path.replace(/\/$/, "").length + 1)] = doc.getText();
  }
  if (text !== undefined) open[file] = text;
  const catalogue = await native("trust-lsp.hmiCatalogue", { root_uri, open });
  if (!catalogue.ok) throw new Error(`VALIDATION_FAILED: ${String(catalogue.error ?? "catalogue unavailable")}`);
  const formats = (catalogue.formats ?? {}) as DrawFrame["formats"];
  const groups: Record<string, string[]> = Object.create(null) as Record<string, string[]>;
  const provenance: Record<string, unknown> = Object.create(null) as Record<string, unknown>;
  for (const object of model.object) {
    if (object.group) (groups[object.group] ??= []).push(object.id);
    const resolved = withValueFormat(object, { formats });
    provenance[object.id] = {
      bindings: Object.fromEntries(
        Object.entries(object.bind).map(([key, binding]) => [
          key,
          { ...binding, source: "authored", formatKey: formatPath(binding.path) },
        ]),
      ),
      unit: {
        value: resolved.unit ?? null,
        source:
          object.unit !== undefined
            ? "object"
            : object.bind.value?.unit !== undefined
              ? "binding"
              : resolved.unit !== undefined
                ? "project-format"
                : "unspecified",
      },
      range: { min: resolved.min ?? null, max: resolved.max ?? null, source: "authored-or-unspecified" },
      tokens: { source: "shared-theme-and-project-overrides" },
    };
  }
  let placed: unknown = {};
  if (model.object.some((object) => object.type === "faceplate")) {
    const source = text ?? (await documents.read(file)).text;
    const result = await native("trust-lsp.hmiPlacedFaceplates", { root_uri, file, text: source, open });
    if (!result.ok) throw new Error(`VALIDATION_FAILED: ${String(result.error ?? "faceplate resolution failed")}`);
    placed = result.placed ?? {};
  }
  return {
    groups,
    provenance,
    placed,
    navigation: { order: model.order, group: model.group, hidden: model.hidden ?? false, icon: model.icon },
    unresolved: model.object
      .filter((object) => object.type === "faceplate" && !(placed as Record<string, unknown>)[object.id])
      .map((object) => ({ object: object.id, reason: "missing-or-incompatible-faceplate" })),
  };
}
