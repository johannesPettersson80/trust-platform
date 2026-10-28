import { panelOf, symbolInstance } from "../editor/placed";
import { alarmScenario } from "./alarmScenario";
import type { ExplorationModel } from "../editor/explorationModel";
import type { HmiPageModel, PlacedFaceplate, PlacedFaceplateModel } from "../drawing/model";
import type { ScreenModel, ProjectTexts } from "../editor/messages";
import type { Catalogue } from "../editor/catalogue";
import type { CustomSymbolModel } from "../drawing/symbols/custom";
import { designValues } from "../editor/designSamples";
import { DESIGN_NOW_MS } from "../editor/designHistory";
import { loadPreviewAssets } from "./assets";
import { EngineProject, NativeCall, nativeRead } from "./engineCommands";

/** A page or popup of the project as the editor holds it: its text, and whether the buffer is unsaved. */
export interface ProjectSnapshot {
  file: string;
  text: string;
  dirty: boolean;
}

/** Snapshot ordinary display data for a network-free, read-only exploration in the builder. The native HMI
 * commands it reads through are `native`, by default the language server's own. */
export async function prepareExploration(
  project: EngineProject,
  draft: { id: string; revision: string; files: Record<string, string> },
  context: Record<string, unknown>,
  snapshots: ProjectSnapshot[],
  signal: AbortSignal,
  native: NativeCall = (command, input) => nativeRead(project.token, command, input),
): Promise<ExplorationModel> {
  const sources = Object.fromEntries(
    snapshots.filter((file) => /^hmi\/(pages|popups)\//.test(file.file)).map((file) => [file.file, file.text]),
  );
  Object.assign(sources, draft.files);
  if (Object.keys(sources).length > 32)
    throw new Error(
      "LIMIT_EXCEEDED: interactive exploration supports 32 pages/popups per snapshot; use image review for larger projects.",
    );
  const open = Object.fromEntries(snapshots.filter((file) => file.dirty).map((file) => [file.file, file.text]));
  Object.assign(open, draft.files);
  const screen = context.screen as ScreenModel,
    texts = context.texts as ProjectTexts,
    language = texts.languages[0]?.tag ?? "en";
  const result: ExplorationModel = {
    pages: Object.create(null),
    popups: Object.create(null),
    values: {},
    assets: {},
    nowMs: DESIGN_NOW_MS,
  };
  const models: HmiPageModel[] = [];
  for (const [file, text] of Object.entries(sources)) {
    if (signal.aborted) throw new Error("CANCELLED: exploration cancelled.");
    const popup = file.startsWith("hmi/popups/");
    const parsed = await native("trust-lsp.hmiFileParse", {
      kind: popup ? "popup" : "page",
      text,
      file,
    });
    if (!parsed.ok) throw new Error(`VALIDATION_FAILED: ${file} cannot be explored until its syntax is fixed.`);
    const model = parsed.model as HmiPageModel,
      name = file.split("/").pop()!.slice(0, -5);
    models.push(model);
    if (popup) result.popups[name] = { name, model: model as HmiPageModel & { size: [number, number] } };
    // Every object that opens a panel for an instance, as the runtime reads it: its click's panel,
    // else an equipment object's own (spec 39 §8.7 item 3).
    const popupObjects = model.object
      .filter((object) => panelOf(object) && symbolInstance(object))
      .map((object) => ({
        id: object.id,
        type: "faceplate",
        at: [0, 0],
        size: [640, 480],
        rotation: 0,
        bind: {},
        faceplate: panelOf(object),
        instance: symbolInstance(object),
      }));
    const placedObjects = model.object.filter((object) => object.type === "faceplate");
    let placements: Record<string, unknown> = { ok: true, placed: {} };
    if (placedObjects.length || popupObjects.length) {
      const prepared = await native("trust-lsp.hmiFileFormat", {
        kind: "page",
        model: {
          title: model.title,
          object: [
            ...placedObjects,
            ...popupObjects.filter((object) => !placedObjects.some((placed) => placed.id === object.id)),
          ],
          pipe: [],
        },
      });
      if (!prepared.ok || typeof prepared.text !== "string")
        throw new Error("VALIDATION_FAILED: preview faceplate references cannot be prepared.");
      placements = await native("trust-lsp.hmiPlacedFaceplates", {
        root_uri: project.root.toString(),
        file,
        text: prepared.text,
        open,
      });
    }
    if (!placements.ok) throw new Error(`VALIDATION_FAILED: ${file} faceplates cannot be explored.`);
    const placed = placements.placed as Record<string, PlacedFaceplate | null>;
    for (const [id, value] of Object.entries(placed))
      if (value) {
        models.push(value.model);
        result.popups[`faceplate:${popup ? "popup:" : ""}${name}/${id}`] = {
          name: `faceplate:${popup ? "popup:" : ""}${name}/${id}`,
          model: value.model,
        };
      }
    if (popup) continue;
    result.pages[name] = {
      screen: {
        size: screen.size,
        theme: screen.theme,
        colors: screen.colors,
        navigation: { style: "tabs", position: "left" },
      },
      page: { name, model },
      pages: [],
      errors: [],
      bundle: `draft:${draft.id}:${draft.revision}`,
      language,
      languages: texts.languages,
      texts: { ...texts.builtin[language], ...texts.project[language] },
      formats: context.formats as import("../drawing/model").DrawFrame["formats"],
      symbols: context.symbols as CustomSymbolModel[],
      faceplates: Object.fromEntries(
        Object.entries(placed).map(([id, value]) => [
          id,
          value ? { name: `${name}/${id}`, model: value.model as PlacedFaceplateModel } : null,
        ]),
      ),
    };
  }
  const names = Object.keys(result.pages);
  for (const data of Object.values(result.pages)) {
    data.pages = names;
    data.navigation = names
      .filter((name) => !result.pages[name].page!.model.hidden)
      .map((name) => ({ name, title: result.pages[name].page!.model.title }));
  }
  result.alarms = alarmScenario(context.alarms as import("../editor/messages").AlarmMessage[], models, true);
  result.values = Object.assign({}, ...models.map((model) => designValues(model, context.catalogue as Catalogue)));
  const assets = await loadPreviewAssets(
    project,
    models.flatMap((model) => model.object.flatMap((object) => (object.image ? [object.image] : []))),
    signal,
  );
  result.assets = Object.fromEntries(Object.entries(assets).map(([name, asset]) => [name, asset.url]));
  return result;
}
