import * as assert from "assert";

import {
  createControlStudioRuntimeStartHandler,
  type ControlStudioRuntimeLifecycle,
  type ControlStudioRuntimeStartEngine,
  type ControlStudioRuntimeStartScope,
} from "../../controlStudio/runtimeStartAction";
import type { SimulatorStartAnswer } from "../../controlStudio/generated/SimulatorStartAnswer";
import type { SimulatorStartRequest } from "../../controlStudio/generated/SimulatorStartRequest";
import type { SimulatorStartReply } from "../../controlStudio/generated/SimulatorStartReply";
import type { EngineAnswer } from "../../controlStudio/engine/engineClient";

// The decisions are the engine's (`controlStudio/simulatorStart`, specification 35 A2.8, pinned by trust-control's `simulator_start`
// tests). These tests use an engine double that answers the way the engine's rules do, and cover only the host's wiring: what it
// forwards, when it starts the simulator, and what it returns.

/** The lifecycle service's facts the host reads: the runtime's mode and whether it runs, and its start. */
interface LifecycleDouble {
  runtimeMode: "simulate" | "online";
  running?: boolean;
  startLocalSimulator(): Promise<unknown>;
}

suite("Control Studio runtime start action", () => {
  test("starts the simulator once and returns the same outcome to a request that joins the start in flight", async () => {
    let starts = 0;
    let finish: ((value: { ok: true; message: string }) => void) | undefined;
    const engine = fakeEngine();
    const handler = startHandler({
      runtimeMode: "simulate",
      startLocalSimulator: async () => {
        starts++;
        return new Promise(resolve => { finish = resolve; });
      },
    }, engine);
    const request = scope();
    const first = handler({ runtimeId: "simulator", runtimeMode: "simulate" }, request);
    const second = handler({ runtimeId: "simulator", runtimeMode: "simulate" }, request);
    await new Promise(resolve => setImmediate(resolve));
    assert.strictEqual(starts, 1);
    assert.deepStrictEqual(engine.decisions.map(entry => entry.pending), ["idle", "sameScope"]);
    finish!({ ok: true, message: "Simulator started." });
    assert.deepStrictEqual(await Promise.all([first, second]), [
      { ok: true, message: "Simulator started." },
      { ok: true, message: "Simulator started." },
    ]);
    assert.deepStrictEqual(engine.outcomes, [{ ok: true, message: "Simulator started." }]);
  });

  test("forwards the request and the runtime facts, and returns the engine's refusal without starting", async () => {
    let starts = 0;
    const engine = fakeEngine();
    const handler = startHandler({
      runtimeMode: "online",
      startLocalSimulator: async () => { starts++; return { ok: true, message: "started" }; },
    }, engine);
    const request = scope();
    const stale = await handler({ runtimeId: "other", runtimeMode: "simulate" }, request);
    assert.strictEqual(stale.ok, false);
    assert.strictEqual(stale.failure?.kind, "invalid_scope");
    const online = await handler({ runtimeId: "simulator", runtimeMode: "simulate" }, request);
    assert.strictEqual(online.ok, false);
    assert.strictEqual(online.failure?.kind, "not_simulator");
    assert.deepStrictEqual(engine.decisions[0], {
      loopRuntimeId: "simulator",
      requestedRuntimeId: "other",
      requestedRuntimeMode: "simulate",
      runtimeMode: "online",
      running: false,
      starting: false,
      pending: "idle",
    });
    assert.strictEqual(starts, 0);
  });

  test("returns the lifecycle failure the engine words, and does not stay locked", async () => {
    let starts = 0;
    const engine = fakeEngine();
    const handler = startHandler({
      runtimeMode: "simulate",
      startLocalSimulator: async () => (++starts === 1
        ? { ok: false, failure: { message: "Compile failed; inspect Problems." } }
        : { ok: true, message: "Simulator started." }),
    }, engine);
    const request = scope();
    const failed = await handler({ runtimeId: "simulator", runtimeMode: "simulate" }, request);
    assert.strictEqual(failed.ok, false);
    assert.strictEqual(failed.failure?.message, "Compile failed; inspect Problems.");
    assert.strictEqual((await handler({ runtimeId: "simulator", runtimeMode: "simulate" }, request)).ok, true);
    assert.strictEqual(starts, 2);
  });

  test("a start the lifecycle service threw is reported to the engine as a failed start", async () => {
    const engine = fakeEngine();
    const handler = startHandler({
      runtimeMode: "simulate",
      startLocalSimulator: async () => { throw new Error("The debug session did not start."); },
    }, engine);
    const reply = await handler({ runtimeId: "simulator", runtimeMode: "simulate" }, scope());
    assert.strictEqual(reply.ok, false);
    assert.strictEqual(reply.failure?.message, "The debug session did not start.");
    assert.deepStrictEqual(engine.outcomes, [{ ok: false, failureMessage: "The debug session did not start." }]);
  });
});

/** The handler over the lifecycle double: the host reads the same facts the lifecycle service reports. */
function startHandler(lifecycle: LifecycleDouble, engine: ControlStudioRuntimeStartEngine) {
  const snapshot = async () => ({ status: { running: lifecycle.running ?? false, runtimeMode: lifecycle.runtimeMode }, starting: false });
  return createControlStudioRuntimeStartHandler(
    { snapshot, startLocalSimulator: lifecycle.startLocalSimulator } as unknown as ControlStudioRuntimeLifecycle,
    engine,
  );
}

/**
 * An engine double for the host's wiring: it records what the host forwards, and answers the decision the engine's rules give for
 * a request to start the simulator of `simulator` while idle, or join it while it is in flight.
 */
function fakeEngine(): ControlStudioRuntimeStartEngine & { decisions: SimulatorStartRequest[]; outcomes: unknown[] } {
  const decisions: SimulatorStartRequest[] = [];
  const outcomes: unknown[] = [];
  return {
    decisions,
    outcomes,
    async simulatorStart(request: SimulatorStartRequest): Promise<EngineAnswer<SimulatorStartAnswer>> {
      decisions.push(request);
      if (request.requestedRuntimeId !== request.loopRuntimeId) {
        return { result: { decision: "reply", reply: refusal("invalid_scope") } };
      }
      if (request.pending === "sameScope") return { result: { decision: "join" } };
      if (request.runtimeMode !== "simulate") return { result: { decision: "reply", reply: refusal("not_simulator") } };
      return { result: { decision: "start" } };
    },
    async simulatorStartResult(outcome): Promise<EngineAnswer<SimulatorStartReply>> {
      outcomes.push(outcome);
      if (outcome.ok) return { result: { ok: true, message: outcome.message || "Simulator started." } };
      return { result: { ok: false, failure: { kind: "start_failed", message: outcome.failureMessage ?? "" } } };
    },
  };
}

function refusal(kind: "invalid_scope" | "not_simulator"): SimulatorStartReply {
  const messages = {
    invalid_scope: "The Control Studio scope or loop revision changed. Reopen the current loop before starting the simulator.",
    not_simulator: "Start is available only for the local simulator. Connect to a physical runtime explicitly.",
  };
  return { ok: false, failure: { kind, message: messages[kind] } };
}

function scope(): ControlStudioRuntimeStartScope {
  return { schemaVersion: 1, projectId: "project", editorSessionId: "editor", loopId: "loop", loopRevision: "revision", runtimeId: "simulator", runtimeMode: "simulate" };
}
