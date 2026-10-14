import { record, type EngineLoop, type EngineRouteContext, type EngineStageRoute } from "../engineRouteContext";
import type { AcquisitionSettings, InputsSection, Structure, TuneMethod, TuningRequirements } from "../../generated/InputsRequest";
import type { LoopMessageScope } from "../../messages";

/**
 * The study inputs (specification 35, §0.1a `inputs`, A8.1, A8.4). The page's editors change the inputs of an engine-served loop;
 * each change goes to `controlStudio/inputs` as the whole section the engine recorded, with the change applied, and the engine's
 * answer (what the change invalidated, and the stages) goes to the page. The route only names the page's values for the engine;
 * what an input means and what a change invalidates is the engine's.
 *
 * A controller family that a binding selection chose before the loop opened (B1.3) is recorded as the loop's structure when the
 * engine opens it. A change the engine does not hold stays on the TypeScript route: a channel mapping (a loop with channels is
 * refused, and the page shows its Channels button only for one), a binding change (the setup stage's, V6 S9), and a tuning method
 * the engine does not run.
 */

/** The page's tuning methods as the engine names them (B4.2); Automatic is none. */
const METHOD_OF_PAGE = new Map<string, TuneMethod | undefined>([
  ["automatic", undefined],
  ["SIMC", "SIMC"],
  ["AMIGO", "AMIGO"],
  ["relay", "relay"],
  ["manual", "manual"],
]);

/** A controller family as the engine names it (B4.4), or undefined for Automatic or any other value. */
function familyOf(value: unknown): Structure | undefined {
  return value === "P" || value === "PI" || value === "PD" || value === "PID" ? value : undefined;
}

/** Whether the engine holds a page change (see the module's header). */
export function engineHoldsChange(payload: Record<string, unknown>): boolean {
  if (payload.channelMapping !== undefined || payload.bindingInvalid !== undefined) return false;
  if (payload.tuningPreferences === undefined) return true;
  const method = record(payload.tuningPreferences)?.method;
  return typeof method === "string" && METHOD_OF_PAGE.has(method);
}

/**
 * The study's inputs after one page change (A8.1): the inputs the change names take its values, and the others stay as the engine
 * recorded them. Call it only for a change that `engineHoldsChange` accepts.
 */
export function inputsAfterChange(recorded: InputsSection, payload: Record<string, unknown>, loopRevision: string): InputsSection {
  const next: InputsSection = { ...recorded };
  if (payload.acquisitionSettings !== undefined) next.acquisition = record(payload.acquisitionSettings) as AcquisitionSettings | undefined;
  if (payload.family !== undefined) next.structure = familyOf(payload.family);
  if (payload.tuningPreferences !== undefined) {
    const preferences = record(payload.tuningPreferences);
    const method = String(preferences?.method);
    next.method = METHOD_OF_PAGE.get(method);
    // The manual settings belong to the manual method alone (B4.2).
    next.settings = method === "manual" ? (record(preferences?.values) as Record<string, number> | undefined) : undefined;
  }
  if (payload.pinnedValues === null) {
    next.pins = undefined;
  } else if (payload.pinnedValues !== undefined) {
    const values = record(payload.pinnedValues);
    const family = familyOf(payload.family) ?? next.structure;
    if (values && family) next.pins = { family, loopRevision, values: values as Record<string, number> };
  }
  if (payload.referenceChange !== undefined) {
    // The requirements editor sends its requirements with the scenarios it declares (B4.3).
    const { scenarioDefinitions, ...requirements } = payload;
    next.requirements = requirements as unknown as TuningRequirements;
    if (scenarioDefinitions !== undefined) next.scenarios = scenarioDefinitions as InputsSection["scenarios"];
  }
  return next;
}

/** The stage route of the study inputs, which an opened loop's recorded inputs reach through `opened`. */
export interface InputsStageRoute extends EngineStageRoute {
  /** Takes the inputs that `controlStudio/open` answered for the loop; the next change merges into them (A8.6). */
  opened(inputs: InputsSection | undefined, scope: LoopMessageScope): Promise<void>;
  /** A copy of the inputs the engine recorded for the served loop: the settings of Identify, Tune and Validate come from here (B3.2, B4.2). */
  recorded(): InputsSection;
  /** Settles when every change the page made so far has its answer, so that a request the page made after them follows them. */
  settled(): Promise<void>;
  /**
   * The controller family a binding selection chose for the loop, before the panel opens it (B1.3). The engine records it as the
   * loop's structure when the loop opens; a study that already records that family is left as it is. A family the engine's
   * structures do not name (P, PI, PD, PID) is no structure, and is not recorded.
   */
  bindingFamily(family: string | undefined): void;
}

export function createInputsRoute(context: EngineRouteContext): InputsStageRoute {
  /** The inputs the engine recorded for the served loop. */
  let recorded: InputsSection = {};
  /** Advances on each reset, so that a change still on the wire for an earlier loop does not write the inputs of the loop now open. */
  let generation = 0;
  /** Changes reach the engine one at a time, so that each merges into the inputs the change before it recorded. */
  let queue: Promise<unknown> = Promise.resolve();
  /** The family a binding selection chose; it waits for the loop to open, so that a reset of the stages does not drop it. */
  let pendingFamily: Structure | undefined;

  async function change(payload: Record<string, unknown>, scope: LoopMessageScope, loop: EngineLoop): Promise<boolean> {
    const started = generation;
    const inputs = inputsAfterChange(recorded, payload, scope.loopRevision);
    const reply = await context.ask(scope, "inputs", () =>
      context.client.inputs({ inputs, loopId: loop.id, projectRoot: context.projectRoot, runtimeId: loop.runtimeId })
    );
    // The engine serves the loop even when it cannot be reached: its start failure is shown, and the change is not repeated.
    if (!reply) return true;
    if ("failure" in reply) {
      await context.postFailure(scope, "inputs", reply.failure);
      return true;
    }
    if (started === generation) recorded = inputs;
    await context.post(scope, { kind: "control-studio.engine.result", method: "inputs", result: reply.result });
    return true;
  }

  /** Runs one change after the earlier ones have been answered. */
  function serialized(run: () => Promise<boolean>): Promise<boolean> {
    const answer = queue.then(run);
    queue = answer.catch(() => undefined);
    return answer;
  }

  return {
    async handle(message, scope) {
      const loop = context.served();
      if (!loop || message.kind !== "control-studio.engineer.invalidate") return false;
      const payload = record(message.payload) ?? {};
      if (!engineHoldsChange(payload)) return false;
      return serialized(() => change(payload, scope, loop));
    },
    async opened(inputs, scope) {
      recorded = inputs ? structuredClone(inputs) : {};
      const family = pendingFamily;
      pendingFamily = undefined;
      const loop = context.served();
      if (family !== undefined && loop && family !== recorded.structure) {
        await serialized(() => change({ family }, scope, loop));
      }
      await context.post(scope, { kind: "control-studio.engine.result", method: "inputs", result: { inputs: recorded } });
    },
    reset() {
      generation += 1;
      recorded = {};
      queue = Promise.resolve();
    },
    bindingFamily(family) {
      pendingFamily = familyOf(family);
    },
    recorded: () => structuredClone(recorded),
    settled: () => queue.then(() => undefined),
  };
}
