import * as assert from "assert";
import { EngineerWorkflowSession } from "../../controlStudio/engineerWorkflowSession";
import { channel, pidLoop } from "./control-studio-pid-fixtures";

type Payload = Record<string, any>;

const gains = (proportional: number) => ({ "Main.Kp": proportional, "Main.Ki": 0, "Main.Kd": 0 });
const tuning = { family: "PI", applicationMode: "manual", referenceChange: 1, maximumError: 0.1, settlingBand: 0.02,
  settlingTimeSeconds: 2, durationSeconds: 4 };

function status(overrides: Payload = {}) {
  return { program_revision: "program-revision", binding_revision: "binding-revision", active_revision: "active-revision",
    values: gains(2), status: "applied", previous_revision: "previous-revision", previous_values: gains(1), reason: null,
    transaction_id: "transaction", supports_request_id: true, ...overrides };
}

/** One session driven through its public message boundary, with every post retained. */
function drive(session: EngineerWorkflowSession, loop: any) {
  const scope = { projectId: loop.projectId, editorSessionId: "session-operations", loopId: loop.id, loopRevision: loop.revision };
  const posts: Payload[] = [];
  let sequence = 0;
  const send = (action: string, payload: Payload = {}) => session.handle({ schemaVersion: 1, kind: `control-studio.engineer.${action}`,
    messageId: `${action}-${++sequence}`, projectId: scope.projectId, editorSessionId: scope.editorSessionId, loopId: scope.loopId,
    expectedLoopRevision: scope.loopRevision, payload }, scope, async value => { posts.push(value); });
  return { posts, send, last: () => posts[posts.length - 1] };
}

async function until(label: string, condition: () => boolean): Promise<void> {
  const deadline = Date.now() + 5000;
  while (!condition()) {
    if (Date.now() > deadline) throw new Error(`Timed out waiting for ${label}.`);
    await new Promise(resolve => setTimeout(resolve, 5));
  }
}

function withParameterClient<T>(client: Payload, run: () => Promise<T>): Promise<T> {
  const parameters = require("../../controlStudio/runtimeParameterConnection");
  const previous = parameters.connectRuntimeParameters;
  parameters.connectRuntimeParameters = async () => ({ source: "simulation", client });
  return run().finally(() => { parameters.connectRuntimeParameters = previous; });
}

suite("Control Studio Manual rollback ticket", () => {
  test("retains the rollback ticket before dispatching the reviewed Manual rollback and clears it once observed", async () => {
    const loop = pidLoop();
    const order: string[] = [];
    const tickets: Payload[] = [];
    const current = status();
    const review = { transaction_id: "rollback-1", observed: current, values: gains(1) };
    await withParameterClient({
      status: async () => current,
      reviewRollback: async () => review,
      rollback: async (reviewed: unknown, physicalApproved: boolean) => {
        order.push("rollback");
        assert.strictEqual(reviewed, review, "the confirmed rollback dispatches exactly the reviewed values");
        assert.strictEqual(physicalApproved, false);
        return status({ active_revision: "previous-revision", values: gains(1), previous_revision: null, previous_values: null });
      },
    }, async () => {
      const session = new EngineerWorkflowSession(loop, () => undefined, () => undefined, undefined,
        async owner => { order.push("ticket"); tickets.push(structuredClone(owner)); });
      const { posts, send, last } = drive(session, loop);
      try {
        await send("rollback", { applicationMode: "manual" });
        assert.strictEqual(last().status, "review", JSON.stringify(last()));
        assert.strictEqual(last().rollback, true);
        assert.deepStrictEqual([last().previous, last().next], [gains(2), gains(1)]);
        assert.deepStrictEqual(order, [], "a review writes nothing and retains no ticket");

        await send("confirm");
        assert.deepStrictEqual(order, ["ticket", "rollback"], "the ticket is retained before the rollback is dispatched");
        assert.deepStrictEqual(tickets, [{ operation: "rollback", origin: { program_revision: "program-revision", binding_revision: "binding-revision",
          review: { transaction_id: "rollback-1", active_revision: "active-revision", previous_revision: "previous-revision",
            values: gains(2), requested: gains(1) } } }]);
        const applied = posts.find(post => post.status === "applied");
        assert.ok(applied, JSON.stringify(posts.map(post => post.status)));
        assert.match(applied.reason, /Previous values restored and observed after ST execution in Manual/);
        assert.strictEqual(session.hasPendingManualApplication(), false, "an observed rollback leaves no pending ticket");
      } finally { session.dispose(); }
    });
  });

  test("keeps the retained ticket as recovery when the dispatched rollback is not confirmed", async () => {
    const loop = pidLoop();
    const order: string[] = [];
    const current = status();
    await withParameterClient({
      status: async () => current,
      reviewRollback: async () => ({ transaction_id: "rollback-2", observed: current, values: gains(1) }),
      rollback: async () => { order.push("rollback"); throw new Error("connection lost"); },
    }, async () => {
      const session = new EngineerWorkflowSession(loop, () => undefined, () => undefined, undefined,
        async () => { order.push("ticket"); });
      const { send, last } = drive(session, loop);
      try {
        await send("rollback", { applicationMode: "manual" });
        await send("confirm");
        assert.deepStrictEqual(order, ["ticket", "rollback"]);
        assert.strictEqual(last().status, "recovery-required", JSON.stringify(last()));
        assert.strictEqual(last().retryManualRecovery, true);
        assert.match(last().reason, /The Manual rollback could not be confirmed: connection lost\. Retry recovery/);
        const reason = last().reason;

        await send("tune", tuning);
        assert.strictEqual(last().status, "recovery-required", "new work waits for the retained rollback to be recovered");
        assert.strictEqual(last().retryManualRecovery, true);
        assert.strictEqual(last().reason, reason);
        assert.deepStrictEqual(order, ["ticket", "rollback"], "the unconfirmed rollback is never dispatched again");
      } finally { session.dispose(); }
    });
  });
});

suite("Control Studio Validate cancellation", () => {
  test("blocks new work until the runtime confirms the cancelled validation, and a second Cancel retries the confirmation", async () => {
    const loop = pidLoop();
    const model: any = { kind: "transfer-function", numerator: [1], denominator: [1, 1], delaySeconds: 0,
      revision: "validate-cancel", source: "provided", validation: "validated", evidence: ["record"],
      inputs: [channel("u", "V")], outputs: [channel("y", "m")] };
    const search = require("../../controlStudio/runtimeSearchConnection");
    const verification = require("../../controlStudio/verificationClient");
    const prototype = verification.RuntimeVerificationClient.prototype;
    const previous = { search: search.connectRuntimeSearch, verify: prototype.verify, cancel: prototype.cancel,
      snapshot: prototype.snapshot, dispose: prototype.dispose };
    let running: ((error: Error) => void) | undefined;
    let confirmed = false;
    let cancellations = 0;
    search.connectRuntimeSearch = async () => ({ source: "simulation", request: async (operation: string) => {
      throw new Error(`unexpected runtime operation ${operation}`);
    } });
    // The runtime job runs until it is cancelled; the runtime confirms the cancellation only on the second request.
    prototype.verify = function () { return new Promise((_, reject) => { running = reject; }); };
    prototype.cancel = async function () { cancellations++; running?.(new Error("Verification cancelled.")); running = undefined; };
    prototype.dispose = function (this: { cancel(): Promise<void> }) { void this.cancel(); };
    prototype.snapshot = function () {
      return { status: confirmed ? "cancelled" : "cancellation-unconfirmed", jobId: "validate-job", projectRevision: "project",
        request: { execution: "isolated-simulation", expected_project_revision: "project" },
        reason: confirmed ? "The runtime confirmed the cancellation." : "The runtime has not confirmed the cancellation." };
    };
    try {
      await withParameterClient({ status: async () => status({ values: gains(1) }) }, async () => {
        const session = new EngineerWorkflowSession(loop, () => model, () => undefined);
        const { posts, send, last } = drive(session, loop);
        try {
          await send("tune", tuning);
          assert.strictEqual(last().status, "candidate", JSON.stringify(last()));

          const validating = send("verify", { applicationMode: "manual" });
          await until("the validation's runtime job", () => running !== undefined);
          await send("cancel");
          await validating;
          const unconfirmed = last();
          assert.strictEqual(unconfirmed.status, "failed", JSON.stringify(unconfirmed));
          assert.strictEqual(unconfirmed.cancellationUnconfirmed, true);
          assert.match(unconfirmed.reason, /The runtime has not confirmed the cancellation\. New work is blocked; press Cancel to retry confirmation\./);
          assert.ok(unconfirmed.verificationAttempts.some((attempt: Payload) => attempt.jobId === "validate-job"
            && attempt.status === "cancellation-unconfirmed"), "the unresolved runtime job is retained");
          assert.ok(!posts.some(post => post.status === "verified" || post.status === "verification-failed"), "a cancelled validation reports no result");

          await send("tune", tuning);
          assert.strictEqual(last().status, "busy", "no new candidate while the cancellation is unconfirmed");

          const before = cancellations;
          confirmed = true;
          await send("cancel");
          assert.ok(cancellations > before, "the second Cancel asks the runtime again");
          assert.strictEqual(last().status, "cancelled", JSON.stringify(last()));
          assert.strictEqual(last().cancellationUnconfirmed, false);

          await send("tune", tuning);
          assert.strictEqual(last().status, "candidate", "new work starts once the runtime confirmed the cancellation");
        } finally { session.dispose(); }
      });
    } finally {
      search.connectRuntimeSearch = previous.search;
      Object.assign(prototype, { verify: previous.verify, cancel: previous.cancel, snapshot: previous.snapshot, dispose: previous.dispose });
    }
  });
});
