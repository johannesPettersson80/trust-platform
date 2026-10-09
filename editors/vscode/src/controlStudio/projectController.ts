import { decentralizedGroupDesign, projectControllerSeed, type ProjectControllerSeed } from "./projectControllerSeed";
import type { DecentralizedPiDesign } from "./decentralizedPidDesign";
import { feedforwardDesign, type FeedforwardDesign } from "./feedforwardPidDesign";
import { decoupledGroupDesign, type DecouplerDesign } from "./decoupledGroupDesign";
import { projectModelParameters, type ProjectModelParameter } from "./projectModelParameters";
import { componentChoices, componentSymbols, validProjectComponents, type ProjectTuningComponent } from "./projectTuningComponents";
import type { ControlLoopDefinition } from "./loopDefinition";
import type { ParameterBinding } from "./parameterBinding";
import type { GeneralModel } from "./generalModel";
import type { EngineerRequirements } from "./engineerVerificationPlan";
import { engineerVerificationCampaign, campaignStatus, type EngineerScenarioResult } from "./engineerVerificationCampaign";
import { decodeControllerInitialization, type ControllerInitialization } from "./controllerInitialization";
import type { RuntimeVerificationClient, RuntimeVerificationResult, RuntimeVerificationAttempt } from "./verificationClient";
import { materializeBoundParameters } from "./parameterBinding";
import { PROJECT_CONTROLLER_FAMILIES, type ControllerFamily } from "./controllerFamilies";
import { selectedModelAnalysis } from "./selectedModelAnalysis";
import { calculateDeclaredRouth } from "./routhHurwitz";
import { ISOLATED_SIMULATION_SCAN_LIMIT, SEARCH_WALL_TIME_LIMIT_MS } from "./controlStudioLimits";


export interface ProjectControllerBinding {
  family: typeof PROJECT_CONTROLLER_FAMILIES[number];
  convention: "project-owned-native-controller";
  /** Further controllers' modes the transfer switches with the loop's mode, for example a cascade's outer controller. */
  additionalModes?: readonly { symbol: string; manualValue: boolean }[];
  initialization: ControllerInitialization;
  seed?: ProjectControllerSeed;
  components?: readonly ProjectTuningComponent[];
  modelParameters?: Readonly<Record<string, ProjectModelParameter>>;
  requiredScenarioIds?: readonly string[];
  axes: readonly { symbol: string; values: readonly number[] }[];
  objective: { requirementId: string; direction: "minimize" | "maximize" };
  maximumCandidates: number;
  maximumCycles: number;
  wallBudgetMs: number;
}
export interface ProjectControllerDesign {
  status: "provisional";
  candidate: { applied: false; structure: ControllerFamily; parameters: Readonly<Record<string, number>>; proportionalGain?: never };
  /** Per-member path, relative gain and effective gain of a group designed from its model. */
  decentralized?: DecentralizedPiDesign;
  /** Both paths, the feedforward values and the predicted deviations of a feedforward loop (Specification 35 §17.12). */
  feedforward?: FeedforwardDesign;
  /** The decoupler D of a decoupled group, by plant input and member (Specification 35 §17.15). */
  decoupler?: Omit<DecouplerDesign, "members">;
  selectedModel: { revision: string; provenance: string };
  actualSTVerify: true;
  scope: string;
}
export function decodeProjectController(value: unknown, binding?: ParameterBinding): ProjectControllerBinding | undefined {
  if (!record(value) || !binding || value.convention !== "project-owned-native-controller"
    || !(PROJECT_CONTROLLER_FAMILIES as readonly unknown[]).includes(value.family) || !decodeControllerInitialization(value.initialization)
    || !Array.isArray(value.axes) || value.axes.length > 8
    || !Number.isInteger(value.maximumCandidates) || (value.maximumCandidates as number) < 1 || (value.maximumCandidates as number) > 64
    || !Number.isInteger(value.maximumCycles) || (value.maximumCycles as number) < 1 || (value.maximumCycles as number) > ISOLATED_SIMULATION_SCAN_LIMIT
    || !Number.isInteger(value.wallBudgetMs) || (value.wallBudgetMs as number) < 1 || (value.wallBudgetMs as number) > SEARCH_WALL_TIME_LIMIT_MS
    || !record(value.objective) || typeof value.objective.requirementId !== "string" || !value.objective.requirementId
    || !["minimize", "maximize"].includes(String(value.objective.direction))) return undefined;
  if (value.components !== undefined && !validProjectComponents(value.components, binding)) return undefined;
  if (value.modelParameters !== undefined && (!record(value.modelParameters) || Object.keys(value.modelParameters).length > 64
    || Object.keys(value.modelParameters).some(symbol => !binding.parameters.some(parameter => parameter.symbol === symbol)))) return undefined;
  if (value.requiredScenarioIds !== undefined && (!Array.isArray(value.requiredScenarioIds) || value.requiredScenarioIds.length > 64
    || !value.requiredScenarioIds.every(id => typeof id === "string" && !!id))) return undefined;
  if (value.seed !== undefined && !(record(value.seed) && (value.seed.kind === "decentralized-pid" ? value.seed.family === "PI" && Object.keys(value.seed).length === 2
    : value.seed.kind === "cascade-pid" ? validCascadeSeed(value.seed, binding)
    : value.seed.kind === "feedforward-pid" ? validFeedforwardSeed(value.seed, binding)
    : value.seed.kind === "smith-pid" ? validSmithSeed(value.seed, binding)
    : value.seed.kind === "decoupled-pid" ? validDecoupledSeed(value.seed, binding)
    : value.seed.kind === "trajectory-feedforward-pid" ? validTrajectorySeed(value.seed, binding)
    : value.seed.kind === "parallel-pid" && validParallelSeed(value.seed, binding)))) return undefined;
  if (value.additionalModes !== undefined && !validAdditionalModes(value.additionalModes, binding)) return undefined;
  const used = new Set<string>();
  for (const axis of value.axes) {
    if (!record(axis) || typeof axis.symbol !== "string" || used.has(axis.symbol) || !Array.isArray(axis.values) || !axis.values.length || axis.values.length > 64) return undefined;
    const parameter = binding.parameters.find(parameter => parameter.symbol === axis.symbol);
    if (!parameter || axis.values.some(value => typeof value !== "number" || !Number.isFinite(value) || value < parameter.minimum || value > parameter.maximum
      || parameter.iec_type === "REAL" && Math.fround(value) !== value)) return undefined;
    used.add(axis.symbol);
  }
  if ((value.components as ProjectTuningComponent[] | undefined)?.some(component => componentSymbols(component).some(symbol => used.has(symbol)))) return undefined;
  return structuredClone(value) as unknown as ProjectControllerBinding;
}

/** Coordinates existing isolated runs; the controller algorithm remains in the loaded ST. */
export async function tuneProjectController(loop: ControlLoopDefinition, model: GeneralModel, task: EngineerRequirements,
  observed: Readonly<Record<string, number>>, pins: Readonly<Record<string, number>>, expectedProjectRevision: string, runtime: RuntimeVerificationClient,
  cancelled: () => boolean, retain: (run: RuntimeVerificationResult) => void, retainAttempt: (attempt: RuntimeVerificationAttempt | undefined) => void,
  progress: (text: string) => Promise<void>, selectedFamily?: ControllerFamily): Promise<ProjectControllerDesign> {
  const definition = decodeProjectController(loop.controller.project, loop.parameterBinding);
  if (!definition || !loop.parameterBinding) throw new Error("Declare the project controller's bounded tuning axes and initialization contract.");
  const symbols = loop.parameterBinding.parameters.map(parameter => parameter.symbol);
  const derived = projectModelParameters(loop, model, definition.modelParameters);
  const allowedFamilies = projectControllerFamilies(definition);
  if (selectedFamily && !allowedFamilies.includes(selectedFamily)) throw new Error("The accepted controller family cannot be realized by this project adapter.");
  const families = selectedFamily ? [selectedFamily] : allowedFamilies;
  const candidates: Array<{ values: Record<string, number>; complexity: number; family: ControllerFamily }> = [];
  const seedFailures: string[] = [];
  for (const family of families) {
    try {
      const seedFamily = ["P", "PI", "PD", "PID"].includes(family) ? family as ProjectControllerSeed["family"] : definition.seed?.family;
      const seed = projectControllerSeed(loop, model, task, definition.seed, seedFamily);
      const base = materializeBoundParameters(loop.parameterBinding, symbols, { ...observed, ...derived, ...seed, ...pins }, {});
      let grid = [{ values: base, complexity: family === "P" ? 0 : family === "PI" || family === "PD" ? 1 : 2, family }];
      for (const axis of definition.axes) {
        const values = axis.symbol in pins ? [pins[axis.symbol]] : [...new Set(axis.values)];
        if (grid.length * values.length > definition.maximumCandidates) throw new Error("The declared tuning grid exceeds its candidate budget; pin settings or narrow the axes.");
        grid = grid.flatMap(candidate => values.map(value => ({ ...candidate, values: { ...candidate.values, [axis.symbol]: value } })));
      }
      for (const component of definition.components ?? []) {
        const choices = componentChoices(loop, component, pins);
        if (grid.length * choices.length > definition.maximumCandidates) throw new Error("The joint controller/component grid exceeds its candidate budget; pin settings or narrow the choices.");
        grid = grid.flatMap(candidate => choices.map(choice => ({ ...candidate, values: { ...candidate.values, ...choice.values }, complexity: candidate.complexity + choice.complexity })));
      }
      const parallel = definition.seed?.kind === "parallel-pid" ? definition.seed : undefined;
      if (parallel) grid = grid.filter(candidate => !((family === "P" || family === "PD") && parallel.integralSymbol && candidate.values[parallel.integralSymbol]
        || (family === "P" || family === "PI") && parallel.derivativeSymbol && candidate.values[parallel.derivativeSymbol]));
      if (!grid.length) throw new Error("Pinned integral/derivative settings conflict with the accepted controller family.");
      candidates.push(...grid);
    } catch (error) { seedFailures.push(`${family}: ${error instanceof Error ? error.message : String(error)}`); }
  }
  if (!candidates.length) throw new Error(`No compatible controller seed: ${seedFailures.join("; ")}`);
  if (candidates.length > definition.maximumCandidates) throw new Error("The automatic family/component comparison exceeds its candidate budget; choose a family or pin settings.");
  candidates.sort((a, b) => Number(!symbols.every(symbol => a.values[symbol] === observed[symbol])) - Number(!symbols.every(symbol => b.values[symbol] === observed[symbol])));
  const planFailures = new Set<string>();
  const prepared = candidates.map(candidate => { try {
    const { values } = candidate;
    materializeBoundParameters(loop.parameterBinding!, symbols, values, {});
    if (loop.analysis && loop.analysisParameterSymbols && model.kind === "transfer-function" && model.delaySeconds === 0) {
      const connected = selectedModelAnalysis(loop.analysis, model);
      const analysis = { ...connected, controller: { ...connected.controller, parameters: Object.fromEntries(
        Object.entries(connected.controller.parameters).map(([name, parameter]) => {
          const symbol = loop.analysisParameterSymbols![name];
          return [name, { ...parameter, value: symbol in values ? values[symbol] : parameter.value }];
        })) } };
      const gain = analysis.controller.parameters[analysis.rootLocus.parameter]?.value;
      try { if (gain === undefined || calculateDeclaredRouth(analysis, gain).polynomial.stable !== true) return undefined; }
      catch { return undefined; }
    }
    return { ...candidate, campaign: engineerVerificationCampaign(loop, model, task, values) };
  } catch (error) { planFailures.add(error instanceof Error ? error.message : String(error)); return undefined; } }).filter((candidate): candidate is NonNullable<typeof candidate> => candidate !== undefined);
  const cycles = prepared.reduce((sum, candidate) => sum + candidate.campaign.reduce((total, scenario) => total + scenario.request.duration_ms / scenario.request.step_ms + 1, 0), 0);
  if (!prepared.length) throw new Error(`No candidate has a valid initialization/scenario plan inside the stability constraints. ${[...planFailures].join(" ")}`);
  if (cycles > definition.maximumCycles) throw new Error("The mandatory tuning campaign exceeds its total-cycle budget.");
  if (prepared.some(candidate => !candidate.campaign.some(scenario => scenario.requirements.some(requirement => requirement.id === definition.objective.requirementId)))) throw new Error("The tuning objective must name a mandatory observed requirement.");
  if (prepared.some(candidate => definition.requiredScenarioIds?.some(id => !candidate.campaign.some(scenario => scenario.id === id)))) throw new Error("The declared controller/component policy requires additional mandatory scenarios before tuning.");
  const deadline = Date.now() + definition.wallBudgetMs;
  let best: { values: Record<string, number>; score: number; complexity: number; family: ControllerFamily } | undefined;
  let revision = expectedProjectRevision;
  for (const [index, candidate] of prepared.entries()) {
    if (cancelled()) throw new Error("Controller tuning cancelled.");
    if (Date.now() >= deadline) break;
    await progress(`Tuning ${definition.family}: candidate ${index + 1}/${prepared.length}, using the selected model and actual ST.`);
    const results: EngineerScenarioResult[] = [];
    let failed = false;
    let programChanged = false;
    for (const scenario of candidate.campaign) {
      if (cancelled()) throw new Error("Controller tuning cancelled.");
      const remaining = deadline - Date.now();
      if (remaining <= 0) { failed = true; break; }
      try {
        const result = await runtime.verify({ ...scenario.request, wall_budget_ms: Math.min(scenario.request.wall_budget_ms, remaining) }, scenario.requirements);
        retain(result);
        if (revision && revision !== result.projectRevision) {
          programChanged = true;
          throw new Error("The loaded program changed during controller tuning.");
        }
        revision = result.projectRevision;
        results.push({ id: scenario.id, result });
      } catch (error) {
        const attempt = runtime.snapshot(); retainAttempt(attempt);
        if (programChanged || cancelled() || attempt?.status === "cancellation-unconfirmed" || revision && attempt?.projectRevision && attempt.projectRevision !== revision) throw error;
        failed = true; break;
      }
    }
    if (failed || campaignStatus(candidate.campaign.map(scenario => scenario.id), results) !== "Pass") continue;
    const measurements = results.flatMap(result => result.result.evaluation.requirements).filter(result => result.id === definition.objective.requirementId);
    if (!measurements.length || measurements.some(result => result.value === undefined || !Number.isFinite(result.value))) continue;
    const score = definition.objective.direction === "minimize" ? Math.max(...measurements.map(result => result.value!)) : -Math.min(...measurements.map(result => result.value!));
    if (!best || candidate.complexity < best.complexity || candidate.complexity === best.complexity && score < best.score) best = { values: candidate.values, score, complexity: candidate.complexity, family: candidate.family };
  }
  if (!best) throw new Error("No project-controller candidate met every mandatory requirement within the declared tuning budget; retained runs show the failing or missing conditions.");
  const decoupled = definition.seed?.kind === "decoupled-pid" ? decoupledGroupDesign(loop, model, task, definition.seed) : undefined;
  const decentralized = definition.seed?.kind === "decentralized-pid" ? decentralizedGroupDesign(loop, model, task)
    : decoupled ? { gainMatrix: [], relativeGains: [], members: decoupled.members } : undefined;
  const decoupler = decoupled ? { matrix: decoupled.matrix, inputIds: decoupled.inputIds, memberIds: decoupled.memberIds } : undefined;
  const feedforward = definition.seed?.kind === "feedforward-pid" ? (({ values: _values, ...design }) => design)(feedforwardDesign(loop, model, task, definition.seed)) : undefined;
  return { status: "provisional", candidate: { applied: false, structure: best.family, parameters: best.values },
    ...(decentralized ? { decentralized } : {}), ...(feedforward ? { feedforward } : {}), ...(decoupler ? { decoupler } : {}),
    selectedModel: { revision: model.revision, provenance: `declared-${model.source}-${model.validation}` }, actualSTVerify: true,
    scope: decentralized
      ? "Each member's PI designed by SIMC on its own path of the group model, for the gain it has with the other loops closed; checked on the actual ST. Reviewed Verify/Apply remain separate operations."
      : "Best observed qualifying record in the declared finite tuning grid. This is not a global optimum or a stability-region proof; reviewed Verify/Apply remain separate operations." };
}
/** A seeded P/PI/PD/PID law offers the families whose gains its seed names (Specification 35 §17.1). */
export function projectControllerFamilies(definition: ProjectControllerBinding): readonly ControllerFamily[] {
  const seed = definition.seed;
  if (!seed || !["P", "PI", "PD", "PID"].includes(definition.family)) return [definition.family];
  if (seed.kind !== "parallel-pid") return ["P", "PI", "PD", "PID"];
  return (["P", "PI", "PD", "PID"] as const).filter(family => (!family.includes("I") || seed.integralSymbol) && (!family.includes("D") || seed.derivativeSymbol));
}
/** A cascade's two PIDs each name their three gains, all distinct bound parameters (Specification 35 §17.11). */
function validCascadeSeed(seed: Record<string, unknown>, binding: ParameterBinding): boolean {
  if (seed.family !== "PI" || !record(seed.inner) || !record(seed.outer) || typeof seed.inner.instance !== "string" || !seed.inner.instance
    || typeof seed.outer.instance !== "string" || !seed.outer.instance || seed.inner.instance === seed.outer.instance) return false;
  const symbols = [seed.inner, seed.outer].flatMap(gains => [gains.proportionalSymbol, gains.integralSymbol, gains.derivativeSymbol]);
  return symbols.every(symbol => typeof symbol === "string" && binding.parameters.some(parameter => parameter.symbol === symbol))
    && new Set(symbols).size === 6;
}
/** A PI's three gains and the feedforward gain with, both or neither, its lead and lag times: distinct bound parameters (§17.12). */
function validFeedforwardSeed(seed: Record<string, unknown>, binding: ParameterBinding): boolean {
  const optional = [seed.leadSymbol, seed.lagSymbol];
  if (seed.family !== "PI" || optional.some(symbol => symbol !== undefined) && !optional.every(symbol => symbol !== undefined)) return false;
  const symbols = [seed.proportionalSymbol, seed.integralSymbol, seed.derivativeSymbol, seed.gainSymbol, ...optional.filter(symbol => symbol !== undefined)];
  return symbols.every(symbol => typeof symbol === "string" && binding.parameters.some(parameter => parameter.symbol === symbol))
    && new Set(symbols).size === symbols.length;
}
/** A square decoupler of distinct bound parameters, at most eight by eight (§17.15). */
function validDecoupledSeed(seed: Record<string, unknown>, binding: ParameterBinding): boolean {
  const rows = seed.matrixSymbols;
  if (seed.family !== "PI" || !Array.isArray(rows) || !rows.length || rows.length > 8 || !rows.every(row => Array.isArray(row) && row.length === rows.length)) return false;
  const symbols = (rows as unknown[][]).flat();
  return symbols.every(symbol => typeof symbol === "string" && binding.parameters.some(parameter => parameter.symbol === symbol)) && new Set(symbols).size === symbols.length;
}
/** A PI's three gains and the program's model gain, time constant and delay: distinct bound parameters (§17.13). */
function validSmithSeed(seed: Record<string, unknown>, binding: ParameterBinding): boolean {
  const symbols = [seed.proportionalSymbol, seed.integralSymbol, seed.derivativeSymbol, seed.gainSymbol, seed.timeConstantSymbol, seed.delaySymbol];
  return seed.family === "PI" && symbols.every(symbol => typeof symbol === "string" && binding.parameters.some(parameter => parameter.symbol === symbol))
    && new Set(symbols).size === symbols.length;
}
/** At most eight further BOOL modes, each distinct from the loop's own mode. */
function validAdditionalModes(value: unknown, binding: ParameterBinding): boolean {
  if (!Array.isArray(value) || value.length > 8) return false;
  const symbols = value.map(mode => record(mode) && typeof mode.symbol === "string" && mode.symbol.trim() && typeof mode.manualValue === "boolean"
    && Object.keys(mode).length === 2 ? mode.symbol.toUpperCase() : "");
  return symbols.every(Boolean) && new Set([...symbols, binding.mode.symbol.toUpperCase()]).size === symbols.length + 1;
}
/** The proportional gain and each gain the family uses are distinct bound parameters; a gain the program lacks is omitted. */
/** A trajectory feedforward's PID gains and its rate and acceleration gains: five distinct bound parameters (§17.16). */
function validTrajectorySeed(seed: Record<string, unknown>, binding: ParameterBinding): boolean {
  const symbols = [seed.proportionalSymbol, seed.integralSymbol, seed.derivativeSymbol, seed.rateSymbol, seed.accelerationSymbol];
  return ["P", "PI", "PD", "PID"].includes(String(seed.family)) && Object.keys(seed).length === 7
    && symbols.every(symbol => typeof symbol === "string" && binding.parameters.some(parameter => parameter.symbol === symbol))
    && new Set(symbols).size === 5;
}
function validParallelSeed(seed: Record<string, unknown>, binding: ParameterBinding): boolean {
  const family = String(seed.family);
  if (!["P", "PI", "PD", "PID"].includes(family) || family.includes("I") && seed.integralSymbol === undefined
    || family.includes("D") && seed.derivativeSymbol === undefined) return false;
  const symbols = [seed.proportionalSymbol, seed.integralSymbol, seed.derivativeSymbol].filter(symbol => symbol !== undefined);
  return symbols.every(symbol => typeof symbol === "string" && binding.parameters.some(parameter => parameter.symbol === symbol))
    && new Set(symbols).size === symbols.length;
}
function record(value: unknown): value is Record<string, unknown> { return !!value && typeof value === "object" && !Array.isArray(value); }
