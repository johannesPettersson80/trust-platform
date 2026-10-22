import { record, type EngineLoop, type EngineRouteContext, type EngineScope, type EngineStageRoute } from "../engineRouteContext";
import type { ChannelSelection, InputsSection } from "../../generated/InputsRequest";

/**
 * The study inputs (specification 35, §0.1a `inputs`, A8.1, A8.4). The page's editors change the inputs of an engine-served loop;
 * each change goes to `controlStudio/inputs` as the page sent it, and the engine merges it into the study's inputs and answers the
 * inputs the study now holds, the stages and what the change invalidated. The route forwards each change and posts the engine's
 * answer as it is. What an input means, how a change merges, what it invalidates, and whether the loop takes a change at all (it refuses
 * one while an Apply's checkpoint waits for its Restore, B6.4) are the engine's (trust-control's `study::inputs_change`). A channel mapping
 * goes as the loop's channel selection, which the engine checks and keeps; a binding change names no input, so the engine answers it
 * unchanged.
 *
 * A controller family that a binding selection chose before the loop opened (B1.3) is sent as a change when the engine opens the loop.
 */

/** The stage route of the study inputs, which an opened loop's recorded inputs reach through `opened`. */
export interface InputsStageRoute extends EngineStageRoute {
  /** Takes the inputs that `controlStudio/open` answered for the loop and posts them to the page, which fills its editors from them (A8.6). */
  opened(inputs: InputsSection | undefined, scope: EngineScope): Promise<void>;
  /** Settles when every change the page made so far has its answer, so that a request the page made after them follows them. */
  settled(): Promise<void>;
  /**
   * The controller family a binding selection chose for the loop, before the panel opens it (B1.3). The engine merges it as a change of
   * the loop's family when the loop opens, and decides what the family means; a family the engine does not name clears the structure.
   */
  bindingFamily(family: string | undefined): void;
}

export function createInputsRoute(context: EngineRouteContext): InputsStageRoute {
  /** Changes reach the engine one at a time, so that each merges into the inputs the change before it recorded. */
  let queue: Promise<unknown> = Promise.resolve();
  /** The family a binding selection chose; it waits for the loop to open, so that a reset of the stages does not drop it. */
  let pendingFamily: string | undefined;

  /** Sends one change to the engine and posts its answer. Returns the inputs the engine answered, none when it answered a failure. */
  async function change(payload: Record<string, unknown>, scope: EngineScope, loop: EngineLoop): Promise<InputsSection | undefined> {
    // The channel mapping is the loop's channel selection, which the engine checks and keeps with its answer (A8.1); the rest of the
    // change is merged into the study's inputs as the page sent it.
    const { channelMapping, ...inputChange } = payload;
    const reply = await context.ask(scope, "inputs", () =>
      context.client.inputs({
        change: inputChange,
        ...(channelMapping !== undefined ? { channelSelection: channelMapping as ChannelSelection } : {}),
        loopId: loop.id,
        projectRoot: context.projectRoot,
        runtimeId: loop.runtimeId,
        // The journal folder, where the engine reads whether an Apply's checkpoint waits for its Restore, which refuses the change (B6.4).
        storageFolder: context.storageFolder,
      })
    );
    // The engine serves the loop even when it cannot be reached: its start failure is shown, and the change is not repeated.
    if (!reply) return undefined;
    if ("failure" in reply) {
      await context.postFailure(scope, "inputs", reply.failure);
      return undefined;
    }
    await context.post(scope, { kind: "control-studio.engine.result", method: "inputs", result: reply.result });
    return reply.result.inputs;
  }

  /** Runs one change after the earlier ones have been answered. */
  function serialized<T>(run: () => Promise<T>): Promise<T> {
    const answer = queue.then(run);
    queue = answer.catch(() => undefined);
    return answer;
  }

  return {
    async handle(message, scope) {
      const loop = context.served();
      if (!loop || message.kind !== "control-studio.engineer.invalidate") return false;
      await serialized(() => change(record(message.payload) ?? {}, scope, loop));
      return true;
    },
    async opened(inputs, scope) {
      // The inputs the page fills its editors from: the open answer's, or the engine's answer to the binding family that follows it.
      let shown: InputsSection = inputs ? structuredClone(inputs) : {};
      const family = pendingFamily;
      pendingFamily = undefined;
      const loop = context.served();
      if (family !== undefined && loop) {
        const answered = await serialized(() => change({ family }, scope, loop));
        if (answered) shown = structuredClone(answered);
      }
      await context.post(scope, { kind: "control-studio.engine.result", method: "inputs", result: { inputs: shown } });
    },
    reset() {
      queue = Promise.resolve();
    },
    bindingFamily(family) {
      pendingFamily = family;
    },
    settled: () => queue.then(() => undefined),
  };
}
