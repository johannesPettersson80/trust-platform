import * as fs from "fs";
import * as path from "path";
import { validateControlLoopDefinition, type ControlLoopDefinition } from "../loopDefinition";
import { assertControlLoopIdAvailable } from "../projectLoop";
import type { CompiledCatalog } from "./compiledCatalog";
import { identifyLoopDefinition, type IdentifyLoopDraft } from "./identifyLoopGenerator";
import { pidLoopDefinition, type PidLoopDraft } from "./pidLoopGenerator";
import { cascadeLoopDefinition, type CascadeTogetherDraft } from "./cascadeGenerator";
import { onOffLoopDefinition, type OnOffLoopDraft } from "./onOffLoopGenerator";
import { pidGroupLoopDefinition, type PidGroupDraft } from "./pidGroupGenerator";
import { stateFeedbackLoopDefinition, type StateFeedbackLoopDraft } from "./stateFeedbackLoopGenerator";
import type { LqgLoopDraft } from "./lqgLoopGenerator";
import { decodeGeneralModel } from "../generalModel";
import { ownLawLoopDefinition, type OwnLawLoopDraft } from "./ownLawLoopGenerator";
import { searchLawLoopDefinition, type SearchLawLoopDraft } from "./searchLawLoopGenerator";
import { feedforwardLoopDefinition, type FeedforwardDraft } from "./feedforwardGenerator";
import { smithPredictorLoopDefinition, type SmithPredictorDraft } from "./smithPredictorGenerator";
import { gainScheduleLoopDefinitions, type GainScheduleDraft } from "./gainScheduleGenerator";

type Scope = { projectId: string; runtimeId: string };
type Unscoped<T> = Omit<T, "projectId" | "runtimeId">;
export type LoopSetupDraft =
  | ({ structure: "pid" } & Unscoped<PidLoopDraft>)
  | ({ structure: "cascade" } & Unscoped<CascadeTogetherDraft>)
  | ({ structure: "identify" } & Unscoped<IdentifyLoopDraft>)
  | ({ structure: "group" } & Unscoped<PidGroupDraft>)
  | ({ structure: "onoff" } & Unscoped<OnOffLoopDraft>)
  | ({ structure: "state-feedback" } & (Unscoped<StateFeedbackLoopDraft> | Unscoped<LqgLoopDraft> & { family: "LQG" }))
  | ({ structure: "own-law" } & Unscoped<OwnLawLoopDraft>)
  | ({ structure: "search" } & Unscoped<SearchLawLoopDraft>)
  | ({ structure: "feedforward" } & Unscoped<FeedforwardDraft>)
  | ({ structure: "smith" } & Unscoped<SmithPredictorDraft>)
  | ({ structure: "schedule" } & Unscoped<GainScheduleDraft>);

/** The model file an LQG estimator implements, inside the project (Specification 35 §17.1). */
async function estimatorModel(projectRoot: string, file: string) {
  const resolved = path.resolve(projectRoot, file);
  if (path.relative(projectRoot, resolved).startsWith("..") || path.isAbsolute(path.relative(projectRoot, resolved))) throw new Error("Choose a model file inside the project.");
  const model = decodeGeneralModel(JSON.parse(await fs.promises.readFile(resolved, "utf8")));
  if (!model) throw new Error(`${file} is not a valid model.`);
  return model;
}

/** Writes the loops of one confirmed setup draft to `control-loops/`, all or none, and returns their Studio requests. */
export async function saveLoopSetup(projectRoot: string, scope: Scope, catalog: CompiledCatalog, draft: LoopSetupDraft) {
  const loops: ControlLoopDefinition[] = draft.structure === "cascade" ? [cascadeLoopDefinition(catalog, { ...draft, ...scope })]
    : draft.structure === "identify" ? [identifyLoopDefinition(catalog, { ...draft, ...scope })]
    : draft.structure === "group" ? [pidGroupLoopDefinition(catalog, { ...draft, ...scope })]
    : draft.structure === "onoff" ? [onOffLoopDefinition(catalog, { ...draft, ...scope })]
    : draft.structure === "state-feedback" ? [stateFeedbackLoopDefinition(catalog, draft.family === "LQG"
      ? { ...draft, ...scope, estimator: { ...draft.estimator, model: await estimatorModel(projectRoot, draft.estimator.modelFile) } } : { ...draft, ...scope })]
    : draft.structure === "own-law" ? [ownLawLoopDefinition(catalog, { ...draft, ...scope })]
    : draft.structure === "search" ? [searchLawLoopDefinition(catalog, { ...draft, ...scope })]
    : draft.structure === "feedforward" ? [feedforwardLoopDefinition(catalog, { ...draft, ...scope })]
    : draft.structure === "smith" ? [smithPredictorLoopDefinition(catalog, { ...draft, ...scope })]
    : draft.structure === "schedule" ? gainScheduleLoopDefinitions(catalog, { ...draft, ...scope })
    : [pidLoopDefinition(catalog, { ...draft, ...scope })];
  for (const loop of loops) {
    const validated = validateControlLoopDefinition(loop, scope);
    if (validated.kind !== "accepted") throw new Error(`The selected bindings are incompatible: ${validated.reason.replace(/_/g, " ")}.`);
  }
  const directory = path.join(projectRoot, "control-loops");
  const files = loops.map(loop => path.join(directory, `${loop.id.replace(/^loop:/, "")}.json`));
  for (const [index, file] of files.entries()) {
    if (fs.existsSync(file)) throw new Error(`A loop named ${loops[index].name} already exists in this project.`);
    await assertControlLoopIdAvailable(projectRoot, loops[index].id);
  }
  await fs.promises.mkdir(directory, { recursive: true });
  for (const [index, file] of files.entries()) await fs.promises.writeFile(file, `${JSON.stringify(loops[index], null, 2)}\n`, { flag: "wx" });
  return loops.map(loop => ({ schemaVersion: 1 as const, source: "ConfiguredLoop" as const, scope: { ...scope }, loopDefinition: loop }));
}
