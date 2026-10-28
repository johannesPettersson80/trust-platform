// The editor's quality warnings (specification 39, §14.1 item 4, §14.5 item 6, §18 Phase 5): touch targets, units, unbound
// objects, cut texts, contrast and look-alike colours, and a program that does not serve its screens. They never stop a deployment.
// The rules, thresholds and sentences are the language server's (`trust-lsp.hmiQuality`); this module reports what only the drawing
// knows, the texts it cut to fit and what each text of the component specimen is drawn on, and passes the server's warnings on.

import { drawPage } from "../drawing";
import { collectCuts } from "../drawing/cuts";
import { DrawnText, drawnTexts } from "../drawing/contrast";
import type { HmiObjectModel, HmiPageModel } from "../drawing/model";
import { HmiColorsModel, TOKEN_THEMES } from "../drawing/tokens";
import type { Catalogue } from "./catalogue";
import { designValues } from "./designSamples";
import { specimenDrawings } from "./specimen";

export interface QualityWarning {
  kind: "touch-target" | "unit" | "unbound" | "cut-text" | "contrast" | "look-alike";
  /** The object it is about. */
  object?: string;
  /** The project colour it is about. */
  colour?: { theme: "dark" | "light"; token: string };
  message: string;
}

/** A project language with its text table: the project's texts over truST's words (§14.5). */
export interface LanguageTable {
  tag: string;
  name: string;
  texts: Record<string, string>;
}

/** The texts the drawing cut to fit while it drew a page in one language, or as written when the project has none. */
export interface LanguageCuts {
  language?: LanguageTable;
  cuts: Array<{ object: string; text: string }>;
}

/** What the server answers (`trust-lsp.hmiQuality`). */
export interface QualityAnswer {
  objects: QualityWarning[];
  colours: QualityWarning[];
  display?: string;
}

/** Sends one request to the language server. */
export type QualityRequest = (command: string, argument: unknown) => Promise<unknown>;

/**
 * The texts the drawing cuts to fit when it draws `page` in each of the project's languages, or as written without them (§14.1 item
 * 4, §14.5 item 6). The page is drawn at the size its objects reach, with the design values of its catalogue.
 */
export function drawnCuts(
  page: HmiPageModel,
  catalogue: Catalogue,
  languages: LanguageTable[] | undefined,
): LanguageCuts[] {
  const right = Math.max(1, ...page.object.map((object) => object.at[0] + object.size[0]));
  const bottom = Math.max(1, ...page.object.map((object) => object.at[1] + object.size[1]));
  const values = designValues(page, catalogue);
  const tables = languages && languages.length > 0 ? languages : [undefined];
  return tables.map((table) => {
    const frame = {
      size: [right, bottom] as [number, number],
      values,
      ...(table ? { texts: table.texts, language: table.tag } : {}),
    };
    const { cuts } = collectCuts(() => drawPage(page, frame));
    return { ...(table ? { language: table } : {}), cuts };
  });
}

/** Each text of the specimen with what lies behind it, measured once per catalogue. */
const specimenTexts = new WeakMap<Catalogue, DrawnText[]>();

/** The specimen's texts; the controls of a read-only page are inactive and need no contrast (WCAG 2.1, 1.4.3). */
function textsOfSpecimen(catalogue: Catalogue): DrawnText[] {
  let texts = specimenTexts.get(catalogue);
  if (!texts) {
    texts = specimenDrawings(catalogue)
      .filter((drawing) => drawing.state !== "read-only")
      .flatMap((drawing) => drawnTexts(drawing.svg));
    specimenTexts.set(catalogue, texts);
  }
  return texts;
}

/** The fields of an object the server's checks read, as the file's model has them. */
function objectReport(object: HmiObjectModel): Record<string, unknown> {
  return object as unknown as Record<string, unknown>;
}

/** The warnings about a page's objects (§14.1 item 4); a P&ID is a drawing that never runs, so only its cut texts are warned of. */
export async function pageQualityWarnings(
  request: QualityRequest,
  rootUri: string,
  page: HmiPageModel,
  catalogue: Catalogue,
  languages: LanguageTable[] | undefined,
  view: "page" | "pid" = "page",
): Promise<QualityWarning[]> {
  const answer = await ask(request, rootUri, {
    page: { objects: page.object.map(objectReport), pid: view === "pid", cuts: drawnCuts(page, catalogue, languages) },
  });
  return answer?.objects ?? [];
}

/** The warnings about a project's own colours, each at its theme and token, and whether a program serves the screens (§18 Phase 5). */
export async function colourQualityWarnings(
  request: QualityRequest,
  rootUri: string,
  colors: HmiColorsModel,
  catalogue: Catalogue,
): Promise<{ warnings: QualityWarning[]; display?: string }> {
  const own = Object.fromEntries(
    (["dark", "light"] as const).map((theme) => [theme, Object.entries(colors[theme] ?? {})]),
  );
  const answer = await ask(request, rootUri, {
    colours: { defaults: TOKEN_THEMES, own, texts: textsOfSpecimen(catalogue) },
  });
  return { warnings: answer?.colours ?? [], ...(answer?.display ? { display: answer.display } : {}) };
}

/** Asks the server for the warnings of `part` in the project at `rootUri` (its `file:` URI); none when the server cannot answer. */
async function ask(
  request: QualityRequest,
  rootUri: string,
  part: Record<string, unknown>,
): Promise<QualityAnswer | undefined> {
  const answer = (await request("trust-lsp.hmiQuality", { root_uri: rootUri, ...part })) as
    ({ ok?: boolean } & Partial<QualityAnswer>) | undefined;
  if (answer?.ok !== true) return undefined;
  return {
    objects: answer.objects ?? [],
    colours: answer.colours ?? [],
    ...(answer.display ? { display: answer.display } : {}),
  };
}
