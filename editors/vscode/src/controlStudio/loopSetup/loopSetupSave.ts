import * as fs from "fs";
import * as path from "path";
import { validateControlLoopDefinition, type ControlLoopDefinition } from "../loopDefinition";
import { assertControlLoopIdAvailable } from "../projectLoop";
import type { CompiledCatalog } from "./compiledCatalog";
import { identifyLoopDefinition, type IdentifyLoopDraft } from "./identifyLoopGenerator";
import { pidLoopDefinition, type PidLoopDraft } from "./pidLoopGenerator";
import { ownLawLoopDefinition, type OwnLawLoopDraft } from "./ownLawLoopGenerator";
import { feedforwardLoopDefinition, type FeedforwardDraft } from "./feedforwardGenerator";

type Scope = { projectId: string; runtimeId: string };
type Unscoped<T> = Omit<T, "projectId" | "runtimeId">;
/** The drafts the TypeScript route still generates. The engine saves every other structure itself (`setupSave`). */
export type LoopSetupDraft =
  | ({ structure: "pid" } & Unscoped<PidLoopDraft>)
  | ({ structure: "identify" } & Unscoped<IdentifyLoopDraft>)
  | ({ structure: "own-law" } & Unscoped<OwnLawLoopDraft>)
  | ({ structure: "feedforward" } & Unscoped<FeedforwardDraft>);

/** Writes the loops of one confirmed setup draft to `control-loops/`, all or none, and returns their Studio requests. */
export async function saveLoopSetup(projectRoot: string, scope: Scope, catalog: CompiledCatalog, draft: LoopSetupDraft) {
  const loops: ControlLoopDefinition[] = draft.structure === "identify" ? [identifyLoopDefinition(catalog, { ...draft, ...scope })]
    : draft.structure === "own-law" ? [ownLawLoopDefinition(catalog, { ...draft, ...scope })]
    : draft.structure === "feedforward" ? [feedforwardLoopDefinition(catalog, { ...draft, ...scope })]
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
