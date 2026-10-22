// The renders of a draft (AT-07, AT-08): each page is drawn by the shared renderer, with its project images and the
// synthetic scenario of its state. The draft and the project context come from the authoring commands.

import * as path from "path";
import { alarmScenario } from "./alarmScenario";
import { sha256 } from "./artifacts";
import { assetRevisions, loadPreviewAssets } from "./assets";
import { context, drafts, EngineProject, envelopeFor, nativeCall, openDocuments, unwrap } from "./engineCommands";
import { renderAgentPage, PreviewCrop } from "./render";
import { runSimulator, SimulatorStep } from "./simulator";
import { scenarioFrame } from "./scenarios";
import { withProjectSymbols } from "../drawing/symbols";
import type { CustomSymbolModel } from "../drawing/symbols/custom";
import type { Catalogue } from "../editor/catalogue";
import type { AlarmMessage, ProjectTexts, ScreenModel } from "../editor/messages";
import type { DrawFrame, HmiPageModel, PlacedFaceplate } from "../drawing/model";
import { designValues } from "../editor/designSamples";
import { designHistory, DESIGN_NOW_MS } from "../editor/designHistory";

/** The context sections a render reads, requested from `hmi_get_context`. */
export const PREVIEW_SECTIONS = ["screen", "texts", "catalogue", "symbols", "formats", "alarms"];

/** A draft's pages at its revision (`drafts read`). */
export interface DraftPages {
  draftId: string;
  revision: string;
  programRevision: string | null;
  files: Record<string, string>;
}

/** What a render reads: the project, the draft, the project context and the font the renderer draws with. */
export interface PreviewSession {
  project: EngineProject;
  fontFile: string;
  draft: DraftPages;
  /** The context answer's data, with the sections of `PREVIEW_SECTIONS`. */
  projectContext: Record<string, unknown>;
  signal: AbortSignal;
}

/** One rendered state: its answer data and the PNG images it produced, in order. */
export interface RenderedState {
  data: Record<string, unknown>;
  images: string[];
}

/** The session of a render: the draft at the revision the request names, and the project context. */
export async function previewSession(
  project: EngineProject,
  fontFile: string,
  input: Record<string, unknown>,
  signal: AbortSignal,
): Promise<PreviewSession> {
  const read = unwrap(await drafts(project, "read", String(input.draftId)));
  if (typeof input.revision === "string" && read.data.revision !== input.revision)
    throw new Error("CONFLICT: draft revision changed.");
  const answer = unwrap(await context(project, envelopeFor(project.workspaceId, { sections: PREVIEW_SECTIONS })));
  return {
    project,
    fontFile,
    draft: {
      draftId: String(read.data.draftId),
      revision: String(read.data.revision),
      programRevision: (read.data.programRevision as string | null) ?? null,
      files: read.data.files as Record<string, string>,
    },
    projectContext: answer.data,
    signal,
  };
}

/** The page a render names: the request's file, or the draft's only page. */
export function selectedFile(draft: DraftPages, file: unknown): string {
  const chosen =
    typeof file === "string" ? file : Object.keys(draft.files).length === 1 ? Object.keys(draft.files)[0] : undefined;
  if (!chosen || !Object.prototype.hasOwnProperty.call(draft.files, chosen))
    throw new Error("INVALID_ARGUMENT: select a file in the draft.");
  return chosen;
}

/** Appends a rendered state's images after the images already collected, renumbering its artifacts to match. */
export function appendImages(images: string[], rendered: RenderedState): number {
  const offset = images.length;
  for (const artifact of rendered.data.artifacts as Array<Record<string, unknown>>)
    artifact.imageIndex = Number(artifact.imageIndex) + offset;
  images.push(...rendered.images);
  return offset;
}

async function parsePage(session: PreviewSession, text: string, file: string): Promise<HmiPageModel> {
  const parsed = await nativeCall(session.project.token, "trust-lsp.hmiFileParse", { kind: "page", text, file });
  return parsed.model as HmiPageModel;
}

/** One page of the draft drawn in one state (`hmi_render_draft`, and each state of `hmi_preview_states`). */
export async function renderState(
  session: PreviewSession,
  input: Record<string, unknown>,
  state: string,
): Promise<RenderedState> {
  const { project, draft, projectContext, signal } = session;
  const file = selectedFile(draft, input.file);
  const model = await parsePage(session, draft.files[file], file);
  const symbols = projectContext.symbols as CustomSymbolModel[];
  for (const object of model.object)
    if (object.type.startsWith("my:") && !symbols.some((symbol) => `my:${symbol.name}` === object.type))
      throw new Error(`VALIDATION_FAILED: missing project symbol ${object.type}.`);
  const screen = projectContext.screen as ScreenModel;
  const catalogue = projectContext.catalogue as Catalogue;
  let placed: Record<string, PlacedFaceplate | null> = {};
  if (model.object.some((object) => object.type === "faceplate")) {
    const open = Object.fromEntries(
      Object.entries(openDocuments(project.root))
        .filter(([, document]) => document.dirty)
        .map(([name, document]) => [name, document.text]),
    );
    placed = (
      await nativeCall(project.token, "trust-lsp.hmiPlacedFaceplates", {
        root_uri: project.root.toString(),
        file,
        text: draft.files[file],
        open,
      })
    ).placed as typeof placed;
    for (const object of model.object)
      if (object.type === "faceplate" && !placed[object.id])
        throw new Error(`VALIDATION_FAILED: faceplate ${object.id} is missing or does not fit its instance.`);
  }
  const models = [model, ...Object.values(placed).flatMap((placement) => (placement ? [placement.model] : []))];
  const assets = await loadPreviewAssets(
    project,
    models.flatMap((page) => page.object.flatMap((object) => (object.image ? [object.image] : []))),
    signal,
  );
  let values = Object.assign({}, ...models.map((page) => designValues(page, catalogue))) as DrawFrame["values"];
  const scenario = withProjectSymbols(symbols, () => scenarioFrame(models, values, state, catalogue));
  values = scenario.values;
  const unsupported = scenario.unsupported;
  const theme = input.theme ?? screen.theme;
  if (theme !== "dark" && theme !== "light") throw new Error("INVALID_ARGUMENT: theme must be dark or light.");
  const projectTexts = projectContext.texts as ProjectTexts;
  const language = input.language ?? projectTexts.languages[0]?.tag ?? "en";
  if (typeof language !== "string" || !projectTexts.languages.some((entry) => entry.tag === language))
    throw new Error("INVALID_ARGUMENT: select a language declared by this project.");
  const texts = { ...projectTexts.builtin[language], ...projectTexts.project[language] };
  const frame: DrawFrame = {
    ...scenario.frame,
    alarms: alarmScenario(projectContext.alarms as AlarmMessage[], models, state === "alarm" || state === "fault"),
    asset: (name) => {
      if (!assets[name]) throw new Error(`RENDER_FAILED: image was not loaded: ${name}.`);
      return assets[name].url;
    },
    language,
    texts,
    size: screen.size,
    values,
    placed,
    history: designHistory(models, values),
    nowMs: DESIGN_NOW_MS,
    readOnly: true,
    formats: projectContext.formats as DrawFrame["formats"],
  };
  const rendered = await renderAgentPage(model, frame, theme, session.fontFile, screen.colors, signal, {
    symbols,
    crop: input.crop as PreviewCrop | undefined,
    viewport: input.viewport as { width: number; height: number } | undefined,
  });
  const images = [rendered.png];
  const artifacts: Array<Record<string, unknown>> = [
    { kind: "page", imageIndex: 0, width: rendered.width, height: rendered.height, scale: 1 },
  ];
  if (rendered.crop) {
    const { png, ...bounds } = rendered.crop;
    images.push(png);
    artifacts.push({ kind: "crop", imageIndex: 1, ...bounds, scale: 1 });
  }
  if (rendered.viewport) {
    const { png, objects, ...size } = rendered.viewport;
    artifacts.push({ kind: "viewport", imageIndex: images.length, ...size, objects });
    images.push(png);
  }
  artifacts.forEach((artifact, index) => {
    artifact.sha256 = sha256(Buffer.from(images[index], "base64"));
  });
  const manifest = {
    contractVersion: 1,
    draftId: draft.draftId,
    draftRevision: draft.revision,
    file,
    fileSha256: sha256(draft.files[file]),
    programRevision: draft.programRevision,
    theme,
    language,
    scenario: state,
    origin: "synthetic",
    designSize: screen.size,
    viewport: input.viewport ?? { width: rendered.width, height: rendered.height },
    scale: rendered.viewport?.scale ?? 1,
    renderer: rendered.renderer,
    fontHashes: rendered.fontHashes,
    assetRevisions: assetRevisions(assets),
    frameSha256: sha256(JSON.stringify({ model, placed, frame, symbols, colors: screen.colors })),
    capturedAt: rendered.capturedAt,
    images: artifacts,
  };
  return {
    data: {
      draftId: draft.draftId,
      revision: draft.revision,
      manifest,
      file,
      theme,
      language,
      scenario: state,
      origin: "synthetic",
      assetRevisions: assetRevisions(assets),
      width: rendered.width,
      height: rendered.height,
      objects: rendered.objects,
      placed,
      fonts: rendered.fonts,
      texts: rendered.texts,
      artifacts,
      unsupportedStateObjects: unsupported,
      visualReview: "required",
    },
    images,
  };
}

/** Every state of `hmi_preview_states`, one after another, with their images numbered in order. */
export async function renderStates(
  session: PreviewSession,
  input: Record<string, unknown>,
  states: string[],
): Promise<RenderedState> {
  const frames: Record<string, unknown>[] = [];
  const images: string[] = [];
  for (const state of states) {
    const rendered = await renderState(session, input, state);
    appendImages(images, rendered);
    frames.push(rendered.data);
  }
  return { data: { draftId: session.draft.draftId, revision: session.draft.revision, frames }, images };
}

/** The bounded simulator journey of `hmi_preview_states` (mode `simulator`), on the named session's deployed pages. */
export async function simulatorJourney(
  session: PreviewSession,
  input: Record<string, unknown>,
): Promise<Record<string, unknown>> {
  const { project, draft, projectContext } = session;
  if (draft.programRevision !== projectContext.programRevision)
    throw new Error("CONFLICT: the program changed since this simulation proposal was created.");
  const models: Record<string, HmiPageModel> = {};
  for (const [file, text] of Object.entries(draft.files))
    models[path.basename(file, ".toml")] = await parsePage(session, text, file);
  return runSimulator(
    project.root.toString(),
    String(input.sessionId),
    input.steps as SimulatorStep[],
    models,
    session.signal,
  );
}
