// The page file requests of the truST language server (`trust-lsp.hmiFileParse` and
// `trust-lsp.hmiFileFormat`), reached through the commands the language client registers.

import type { CustomSymbolModel } from "../drawing/symbols/custom";
import * as vscode from "vscode";

import type { DrawFrame, HmiPageModel, PlacedFaceplate } from "../drawing/model";
import { openHmiTexts } from "./textFiles";
import type { Catalogue } from "./catalogue";
import { AlarmMessage, DEFAULT_SCREEN, FaceplateEntry, ScreenModel, VariableNode } from "./messages";
import type { FileError, PageFiles, PageRead, PageWrite } from "./pageDocument";

const NOT_RUNNING: FileError = { message: "The truST language server is not running yet." };

function errorsOf(answer: unknown): FileError[] {
  const errors = (answer as { errors?: unknown } | undefined)?.errors;
  return Array.isArray(errors) && errors.length > 0
    ? (errors as FileError[])
    : [{ message: "The language server gave no answer." }];
}

async function request(command: string, argument: unknown): Promise<unknown> {
  try {
    return argument === undefined
      ? await vscode.commands.executeCommand<unknown>(command)
      : await vscode.commands.executeCommand<unknown>(command, argument);
  } catch {
    return { ok: false, errors: [NOT_RUNNING] };
  }
}

/** A file under `hmi/popups/` is a popup, one under `hmi/panels/` a faceplate, any other a
 * page (§4.9, §8.7, §15.1). */
function kindOfFile(file: string): "page" | "popup" | "faceplate" | "panel" {
  if (/(^|[\\/])hmi[\\/]panels[\\/]/.test(file)) return "panel";
  if (/(^|[\\/])hmi[\\/]faceplates[\\/]/.test(file)) return "faceplate";
  return /(^|[\\/])hmi[\\/]popups[\\/]/.test(file) ? "popup" : "page";
}

export const languageServerPageFiles: PageFiles = {
  async read(text: string, file: string): Promise<PageRead> {
    const answer = (await request("trust-lsp.hmiFileParse", { kind: kindOfFile(file), text, file })) as {
      ok?: boolean;
      model?: HmiPageModel;
    };
    return answer?.ok === true && answer.model
      ? { ok: true, model: answer.model }
      : { ok: false, errors: errorsOf(answer) };
  },
  async write(model: HmiPageModel): Promise<PageWrite> {
    // A faceplate's model has its fields; a popup's its own size; a page's takes the screen's.
    const kind =
      model.panel !== undefined ? "panel" : model.fields !== undefined ? "faceplate" : model.size ? "popup" : "page";
    const answer = (await request("trust-lsp.hmiFileFormat", { kind, model })) as { ok?: boolean; text?: string };
    return answer?.ok === true && typeof answer.text === "string"
      ? { ok: true, text: answer.text }
      : { ok: false, errors: errorsOf(answer) };
  },
};

/** The names of the faceplates a click can open: the project's and its libraries' (§8.7). */
export async function languageServerFaceplates(project: vscode.Uri): Promise<string[]> {
  const answer = (await request("trust-lsp.hmiFaceplates", {
    root_uri: project.toString(),
    open: openHmiTexts(project),
  })) as { ok?: boolean; faceplates?: Array<{ name: string }> };
  return answer?.ok === true ? (answer.faceplates ?? []).map((faceplate) => faceplate.name) : [];
}

/** The faceplates of the project and its libraries for the Faceplates group (§8.9): their names,
 * titles and sizes; one that does not load is left out. */
export async function languageServerFaceplateEntries(project: vscode.Uri): Promise<FaceplateEntry[]> {
  const answer = (await request("trust-lsp.hmiFaceplates", {
    root_uri: project.toString(),
    open: openHmiTexts(project),
  })) as {
    ok?: boolean;
    faceplates?: Array<{
      name: string;
      title?: string;
      size?: [number, number];
      fields?: Record<string, unknown>;
      requires_instance?: boolean;
      error?: string;
    }>;
  };
  if (answer?.ok !== true) return [];
  return (answer.faceplates ?? [])
    .filter((faceplate) => faceplate.error === undefined)
    .map(({ name, title, size, fields, requires_instance }) => ({
      name,
      requiresInstance: requires_instance ?? (fields === undefined || Object.keys(fields).length > 0),
      ...(title !== undefined ? { title } : {}),
      ...(size ? { size } : {}),
    }));
}

/** The instances of the program a faceplate fits (§8.9 item 1), or why there are none. */
export async function languageServerFaceplateInstances(
  project: vscode.Uri,
  faceplate: string,
): Promise<{ instances: string[]; problem?: string }> {
  const answer = (await request("trust-lsp.hmiFaceplateInstances", {
    root_uri: project.toString(),
    faceplate,
    open: openHmiTexts(project),
  })) as { ok?: boolean; instances?: string[]; errors?: Array<{ message: string }> } | undefined;
  if (answer?.ok === true) return { instances: answer.instances ?? [] };
  return { instances: [], problem: answer?.errors?.[0]?.message ?? "The truST language server is not running." };
}

/** A page's placed faceplates bound to their instances as the display binds them (§8.9), by
 * object id; `null` where the project lacks the faceplate. */
export async function languageServerPlacedFaceplates(
  project: vscode.Uri,
  text: string,
  file: string,
): Promise<Record<string, PlacedFaceplate | null>> {
  const answer = (await request("trust-lsp.hmiPlacedFaceplates", {
    root_uri: project.toString(),
    text,
    file,
    open: openHmiTexts(project),
  })) as { ok?: boolean; placed?: Record<string, PlacedFaceplate | null> } | undefined;
  return answer?.ok === true ? (answer.placed ?? {}) : {};
}

/** The file of a faceplate of the project or its libraries, by name, or undefined (§3.3 item 8). */
export async function languageServerFaceplateUri(project: vscode.Uri, name: string): Promise<vscode.Uri | undefined> {
  const answer = (await request("trust-lsp.hmiFaceplates", {
    root_uri: project.toString(),
    open: openHmiTexts(project),
  })) as { ok?: boolean; faceplates?: Array<{ name: string; uri?: string }> };
  const uri = answer?.ok === true ? answer.faceplates?.find((faceplate) => faceplate.name === name)?.uri : undefined;
  return uri ? vscode.Uri.parse(uri) : undefined;
}

/** The component catalogue, or undefined while the language server cannot give it. */
export async function languageServerCatalogue(): Promise<Catalogue | undefined> {
  const answer = (await request("trust-lsp.hmiCatalogue", undefined)) as { ok?: boolean; catalogue?: Catalogue };
  return answer?.ok === true ? answer.catalogue : undefined;
}

/** The catalogue with the project's own symbols (§14.4), and the symbols, or nothing when the
 * language server gives none. */
export async function languageServerProjectCatalogue(
  project: vscode.Uri,
): Promise<{ catalogue: Catalogue; symbols: CustomSymbolModel[]; formats?: DrawFrame["formats"] } | undefined> {
  const answer = (await request("trust-lsp.hmiCatalogue", {
    root_uri: project.toString(),
    open: openHmiTexts(project),
  })) as {
    ok?: boolean;
    catalogue?: Catalogue;
    equipment?: import("./catalogue").EquipmentDefinition[];
    symbols?: CustomSymbolModel[];
    formats?: DrawFrame["formats"];
  };
  if (answer?.ok !== true || !answer.catalogue) return undefined;
  // My equipment (§21.9): the project's own blocks, proposed or defined by the project itself.
  const own = (await request("trust-lsp.hmiOwnEquipment", {
    root_uri: project.toString(),
    open: openHmiTexts(project),
  })) as { ok?: boolean; equipment?: import("./catalogue").EquipmentDefinition[]; mine?: string[] };
  const mine = new Set((own?.ok === true ? (own.mine ?? []) : []).map((block) => block.toLowerCase()));
  const equipment = [
    ...(answer.equipment ?? []).map((definition) =>
      mine.has(definition.block.toLowerCase()) ? { ...definition, mine: true } : definition,
    ),
    ...(own?.ok === true ? (own.equipment ?? []).map((definition) => ({ ...definition, mine: true })) : []),
  ];
  return { catalogue: { ...answer.catalogue, equipment }, symbols: answer.symbols ?? [], formats: answer.formats };
}

/** The screen layout of `hmi/screen.toml`, or the defaults when it is missing or does not load. */
export async function languageServerScreen(screenFile: vscode.Uri): Promise<ScreenModel> {
  let text: string;
  try {
    const open = vscode.workspace.textDocuments.find((document) => document.uri.toString() === screenFile.toString());
    text = open ? open.getText() : new TextDecoder().decode(await vscode.workspace.fs.readFile(screenFile));
  } catch {
    return DEFAULT_SCREEN;
  }
  const answer = (await request("trust-lsp.hmiFileParse", { kind: "screen", text, file: "hmi/screen.toml" })) as {
    ok?: boolean;
    model?: {
      size?: [number, number];
      theme?: "light" | "dark";
      symbols?: ScreenModel["symbols"];
      colors?: ScreenModel["colors"];
      top_bar?: unknown;
      navigation?: { style?: ScreenModel["navigation"] };
    };
  };
  if (answer?.ok !== true || !answer.model?.size) return DEFAULT_SCREEN;
  const { size, theme, symbols, colors, top_bar: topBar, navigation } = answer.model;
  return {
    size,
    theme: theme ?? "dark",
    symbols,
    colors,
    topBar: topBar !== undefined && topBar !== null,
    navigation: navigation?.style,
  };
}

/** The program's variable tree for the picker (§7.1), or why the program gives none, and the
 * declared alarm metadata (§14.5). The authoring request reads edited declarations and pending
 * equipment without compiling or executing PLC code. */
export async function languageServerVariables(
  project: vscode.Uri,
): Promise<{ variables: VariableNode[]; problem?: string; alarms: AlarmMessage[] }> {
  const answer = (await request("trust-lsp.hmiBindings", {
    authoring: true,
    root_uri: project.toString(),
    open: openHmiTexts(project),
  })) as {
    ok?: boolean;
    variables?: VariableNode[];
    variablesError?: string | null;
    alarms?: AlarmMessage[];
    error?: string;
  };
  const alarms = answer?.ok === true ? (answer.alarms ?? []) : [];
  if (answer?.ok === true && !answer.variablesError) return { variables: answer.variables ?? [], alarms };
  return { variables: [], problem: answer?.variablesError ?? answer?.error ?? errorsOf(answer)[0].message, alarms };
}
