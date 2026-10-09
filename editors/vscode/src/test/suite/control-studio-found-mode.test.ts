import * as assert from "assert";

import { channel, pidInitialization, pidLoop } from "./control-studio-pid-fixtures";
import { decodeCommissioningCheckpoint } from "../../controlStudio/commissioningCheckpoint";
import { controllerAdapter } from "../../controlStudio/controllerAdapters";
import { commissioningTarget } from "../../controlStudio/commissioningObservation";
import { EngineerWorkflowSession } from "../../controlStudio/engineerWorkflowSession";
import { InterruptedCommissioningRecovery } from "../../controlStudio/interruptedCommissioningRecovery";
import { PidCommissioning } from "../../controlStudio/pidCommissioning";
import { ProportionalCommissioning } from "../../controlStudio/proportionalCommissioning";
import { parallelPidTracking, pidTransferBinding } from "../../controlStudio/pidTransfer";
import { proportionalTracking, proportionalTransferBinding } from "../../controlStudio/proportionalTransfer";
import { RuntimeRecordingSession } from "../../controlStudio/runtimeRecording";
import { RuntimeTransferClient } from "../../controlStudio/runtimeTransfer";

/** The requirements of the study: 4 s of test duration and a 0.02 settling band (B6.2, B6.3). */
const TASK: any = { referenceChange: 1, maximumError: 0.1, settlingBand: 0.02, settlingTimeSeconds: 2, durationSeconds: 4 };

type Kind = "pid" | "proportional" | "library";

function singleOutputModel(): any {
  return { kind: "transfer-function", numerator: [1], denominator: [1, 1], delaySeconds: 0, revision: "first-order", source: "provided",
    validation: "validated", evidence: ["bench"], normalizedError: 0.01, inputs: [channel("u", "V")], outputs: [channel("y", "m")],
    operatingPoint: { inputs: [0], outputs: [2] } };
}

function pick(values: Record<string, number | boolean>, symbols: readonly string[]): Record<string, number> {
  const picked: Record<string, number> = {};
  for (const symbol of symbols) picked[symbol] = Number(values[symbol]);
  return picked;
}

/**
 * A fake runtime for one reviewed controller, built as control-studio-apply-process-time.test.ts builds its fake: the transfers and the
 * parameter record share one store, so the mode and the gains can be read back. Only the runtime's answers are replaced.
 */
/** The study's loop. The fake runtime and the reopened session use the same one, as a checkpoint belongs to its loop (A8.6). */
function studyLoop(kind: Kind): any {
  const loop: any = pidLoop();
  loop.controller.pidInitialization = kind === "library" ? controlPidStates() : pidInitialization();
  if (kind === "library") loop.controller.instance = "Main.Controller";
  loop.signals.processVariable.range = { minimum: 0, maximum: 10 };
  // Gains are bounded by magnitude, ±1e6, and keep their sign for a reverse-acting loop (numerical annex N6.4).
  for (const parameter of loop.parameterBinding.parameters) { parameter.minimum = -1e6; parameter.maximum = 1e6; }
  if (kind === "proportional") loop.controller.law = { kind: "proportional", gainSymbol: "Main.Kp" };
  return loop;
}

/** The seven retained states of the shipped Control.PID, as a loop whose controller is Main.Controller declares them. */
function controlPidStates(): any {
  const members: [string, string, unknown][] = [["Setpoint", "m", { source: "model-output", index: 0 }], ["SetpointRate", "m/s", 0],
    ["ProcessValue", "m", { source: "model-output", index: 0 }], ["ProcessValueRate", "m/s", 0], ["FeedForward", "V", 0],
    ["NominalOutput", "V", { source: "model-input", index: 0 }], ["SaturatedOutput", "V", { source: "model-input", index: 0 }]];
  return { kind: "control-pid-v1", maximumSnapshotAgeMs: 100, states: members.map(([name, unit, initial]) => ({
    symbol: `Main.Controller.StateValue.${name}`, iec_type: "LREAL", unit, minimum: -10000, maximum: 10000, initial, postScan: "bounded" })) };
}

function fakeRuntime(kind: Kind, foundInAuto: boolean): any {
  const loop = studyLoop(kind);
  const binding: any = kind === "pid" ? pidTransferBinding(loop)
    : kind === "library" ? controllerAdapter(loop).transferBinding() : proportionalTransferBinding(loop);
  const paramSymbols: string[] = loop.parameterBinding.parameters.map((parameter: any) => parameter.symbol);
  const defaults: Record<string, number> = { "Main.SP": 2, "Main.PV": 2.004, "Main.MV": 0, "Main.ManualCommand": 0,
    "Main.Integral": 0, "Main.PreviousError": 0, "Main.Kp": 1, "Main.Ki": 0, "Main.Kd": 0 };
  const world: any = { values: {} as Record<string, number | boolean>, active: "original", previous: null, previousValues: null,
    applies: [] as boolean[] };
  for (const member of [...binding.states, ...binding.guards, binding.output, ...(binding.additional_outputs ?? [])]) {
    world.values[member.symbol] = defaults[member.symbol] ?? 0;
  }
  // A declared controller's binding holds its states, not its gains: the parameter record is seeded beside it.
  if (kind === "library") for (const symbol of paramSymbols) world.values[symbol] = defaults[symbol] ?? 0;
  world.values[binding.mode.symbol] = foundInAuto ? !binding.mode.manual_value : binding.mode.manual_value;

  const status = () => ({ program_revision: "program", binding_revision: "transfer", values: { ...world.values }, runtime_time_ns: 1_000_000_000,
    status: "idle", reason: null, transaction: null, supported_tracking: ["parallel-pid-v1", "proportional", "declared-state-v1"] });
  const paramStatus = () => ({ program_revision: "program", binding_revision: "parameters", active_revision: world.active,
    previous_revision: world.previous, status: "applied", transaction_id: null, reason: null, supports_request_id: true,
    values: pick(world.values, paramSymbols), previous_values: world.previousValues });
  const parameters: any = {
    status: async () => paramStatus(),
    apply: async (review: any) => {
      world.previous = world.active; world.previousValues = pick(world.values, paramSymbols); world.active = "applied-1";
      Object.assign(world.values, review.values);
      return paramStatus();
    },
    reviewRollback: async () => ({ transaction_id: "rollback-1", values: world.previousValues, observed: paramStatus() }),
    rollback: async (review: any) => {
      Object.assign(world.values, review.values); world.active = world.previous; world.previous = null; world.previousValues = null;
      return paramStatus();
    },
    cancelOwner: async () => "terminal",
  };

  const proto: any = RuntimeTransferClient.prototype;
  const savedTransfer = { status: proto.status, apply: proto.apply, cancel: proto.cancel };
  proto.status = async () => status();
  proto.apply = async (values: Record<string, number>, manual: boolean) => {
    Object.assign(world.values, values);
    world.values[binding.mode.symbol] = manual ? binding.mode.manual_value : !binding.mode.manual_value;
    world.applies.push(manual);
    return status();
  };
  proto.cancel = async () => undefined;

  // The recording answers with a departure from the permitted range, or with a running response that stops when the test lets it.
  const recording: { mode: "out-of-range" | "running"; finish: boolean; polls: number } = { mode: "out-of-range", finish: false, polls: 0 };
  const sessions: any = RuntimeRecordingSession;
  const savedStart = sessions.start;
  const savedRecover = sessions.recover;
  sessions.start = async () => {
    const follow = async () => {
      if (recording.mode === "running") {
        if (recording.finish) return { state: "stopped", elapsedMs: 4000, sampleCount: 0, complete: false, reason: "stopped", rawSamples: [], nextSample: 0 };
        recording.polls++;
        return { state: "recording", elapsedMs: recording.polls * 100, sampleCount: 0, complete: false, reason: null, rawSamples: [], nextSample: 0 };
      }
      return { state: "recording", elapsedMs: 0, sampleCount: 1, complete: false, reason: null, rawSamples: [{ pv: 25 }], nextSample: 1 };
    };
    return { loop, follow, stop: async () => ({ rawSamples: [], evidence: { completeness: "incomplete", reason: "fake recording" } }),
      release: async () => undefined };
  };

  const connection: any = require("../../controlStudio/runtimeConnection");
  const savedBind = connection.bindRuntimeConnection;
  connection.bindRuntimeConnection = async () => ({ source: "simulation", endpoint: "fake", request: async () => ({}) });
  const parameterConnection: any = require("../../controlStudio/runtimeParameterConnection");
  const savedConnect = parameterConnection.connectRuntimeParameters;
  parameterConnection.connectRuntimeParameters = async () => ({ source: "simulation", client: parameters });

  // A P law writes the complete record: its integral and derivative gains are explicit zero terms (Specification 35, C0 and C13).
  const candidate = kind === "proportional" ? { "Main.Kp": 2, "Main.Ki": 0, "Main.Kd": 0 } : { "Main.Kp": 2, "Main.Ki": 0.5, "Main.Kd": 0 };
  const fake: any = { kind, loop, binding, model: singleOutputModel(), task: TASK, parameters, world, recording, recovered: undefined,
    review: { observed: paramStatus(), values: candidate, transaction_id: "apply-1" },
    restore: () => {
      proto.status = savedTransfer.status; proto.apply = savedTransfer.apply; proto.cancel = savedTransfer.cancel;
      sessions.start = savedStart; sessions.recover = savedRecover;
      connection.bindRuntimeConnection = savedBind; parameterConnection.connectRuntimeParameters = savedConnect;
    } };
  sessions.recover = async () => fake.recovered;
  return fake;
}

async function withFake(kind: Kind, foundInAuto: boolean, body: (fake: any) => Promise<void>): Promise<void> {
  const fake = fakeRuntime(kind, foundInAuto);
  try { await body(fake); } finally { fake.restore(); }
}

async function reviewedCommissioning(fake: any, wait?: unknown): Promise<any> {
  return fake.kind === "pid"
    ? PidCommissioning.review(fake.loop, fake.model, fake.task, fake.parameters, fake.review, undefined, wait as never)
    : ProportionalCommissioning.review(fake.loop, fake.model, fake.task, fake.parameters, fake.review, wait as never);
}

const inAuto = (fake: any): boolean => fake.world.values[fake.binding.mode.symbol] !== fake.binding.mode.manual_value;
const lastWrite = (fake: any): boolean => fake.world.applies[fake.world.applies.length - 1];

/** Waits, in small steps, until a condition the fake runtime reaches later holds. */
async function until(condition: () => boolean): Promise<void> {
  for (let waited = 0; !condition(); waited += 5) {
    if (waited > 2000) throw new Error("the condition did not hold in time");
    await new Promise(resolve => setTimeout(resolve, 5));
  }
}

/** The checkpoint of an Apply interrupted after its settings were applied (A6.2, A8.6). The origin is the record before the Apply. */
function interruptedCheckpoint(fake: any, foundMode: "auto" | "manual" | undefined): any {
  const recoveryTracking = fake.kind === "pid" ? parallelPidTracking(fake.loop, fake.binding, false)
    : fake.kind === "library" ? controllerAdapter(fake.loop).recoveryTracking(fake.model) : proportionalTracking(fake.loop, fake.binding, false);
  return { id: "checkpoint-1", loopRevision: fake.loop.revision, stage: "parameters", model: fake.model, task: TASK,
    origin: fake.review.observed, candidate: { ...fake.review.observed.values, ...fake.review.values },
    parameterOperation: { id: "apply-1", program: "program", binding: "parameters" },
    references: { [fake.binding.states[0].symbol]: 2 }, binding: fake.binding, recoveryTracking, tolerance: 0.1,
    ...(foundMode === undefined ? {} : { foundMode }) };
}

/** The checkpoint of an Apply whose response the runtime is still recording when the panel reopens (A8.5). */
function reattachedCheckpoint(fake: any, foundMode: "auto" | "manual" | undefined): any {
  return { ...interruptedCheckpoint(fake, foundMode), stage: "recording",
    recording: { schemaVersion: 1, recordingId: "r1", projectId: fake.loop.projectId, runtimeId: fake.loop.runtimeId, loopId: fake.loop.id,
      loopRevision: fake.loop.revision, source: "Simulation", bindings: [] },
    observation: { startedAtProcessNs: 1, durationMs: 4000 } };
}

/** An engineer session that holds the journalled checkpoint; its journal writes and retained recordings are recorded. */
function sessionWith(checkpoint: unknown, writes: unknown[], retained: unknown[]): EngineerWorkflowSession {
  const loop = studyLoop("pid");
  const session = new EngineerWorkflowSession(loop, () => singleOutputModel(), () => undefined,
    async recording => { retained.push(recording); }, undefined, undefined, undefined, undefined,
    async written => { writes.push(written); });
  (session as any).state.task = TASK;
  session.restoreCommissioningCheckpoint(checkpoint as never);
  return session;
}

/** A recording whose sample is already outside the permitted range of 0 degC..10 degC, so the runtime holds it up to the departure (B6.3). */
function departingRecording(): any {
  const samples = [{ scan: 0, elapsedSeconds: 0, controllerMode: "Auto", sp: 5, pv: 12.5, appliedMv: 2, quality: "good" }];
  const stopped = { rawSamples: samples, samples: [], evidence: { completeness: "incomplete", reason: "fake recording" } };
  return {
    follow: async () => ({ state: "recording", elapsedMs: 0, sampleCount: samples.length, complete: false, reason: null, rawSamples: samples,
      nextSample: samples.length }),
    stop: async () => stopped,
    release: async () => undefined,
  };
}

/** A response that holds the reviewed target for the reviewed duration, in Auto and inside every bound, recorded at each execution (B6.3). */
function holdingRecording(loop: any, target: number): any {
  const period = loop.controller.taskPeriodMs / 1000;
  const units = { sp: loop.signals.setpoint.unit, pv: loop.signals.processVariable.unit, mv: loop.signals.manipulatedVariable.unit };
  const samples = Array.from({ length: Math.round(TASK.durationSeconds / period) + 1 }, (_, scan) => ({ scan, elapsedSeconds: scan * period,
    controllerMode: "Auto", sp: target, pv: target, appliedMv: 0, quality: "good", units }));
  const stopped = { rawSamples: samples, samples: [], evidence: { completeness: "complete" } };
  return {
    follow: async () => ({ state: "stopped", elapsedMs: TASK.durationSeconds * 1000, sampleCount: samples.length, complete: true, reason: null,
      rawSamples: samples, nextSample: samples.length }),
    stop: async () => stopped,
    release: async () => undefined,
  };
}

/** A transfer the runtime confirmed at the output image under its owner's identity, as the transfer client keeps its outcomes. */
function confirmedTransfer(id: string): any {
  return { program_revision: "program", binding_revision: "transfer", values: {}, status: "applied", reason: null,
    transaction: { id, program_revision: "program", binding_revision: "transfer", status: "applied", reason: null, observed_values: null,
      confirmation: "output-image" } };
}

suite("the mode a commissioning found (A6.2)", () => {
  suite("a response that passes while reattached (B6.3, A8.6)", () => {
    test("the receipt carries the Apply's confirmed transfers: the journalled controller transfer and the target change the runtime reports", async () => {
      await withFake("pid", true, async fake => {
        await fake.parameters.apply(fake.review);
        (RuntimeTransferClient.prototype as any).status = async () => confirmedTransfer("target-1");
        fake.recovered = holdingRecording(fake.loop, commissioningTarget(fake.loop, fake.model, TASK, fake.binding));
        const checkpoint = { ...reattachedCheckpoint(fake, "auto"), transfer: { id: "target-1", program: "program", binding: "transfer" },
          observation: { startedAtProcessNs: 1, durationMs: 4000, transfers: [confirmedTransfer("auto-1")] } };
        const writes: unknown[] = [], retained: unknown[] = [], posts: any[] = [];
        const session = sessionWith(checkpoint, writes, retained);
        assert.strictEqual(await session.recoverApplyObservation(async payload => { posts.push(payload); }), true);
        const last = posts[posts.length - 1];
        assert.strictEqual(last.status, "commissioned", last.reason);
        assert.deepStrictEqual(last.commissioning.transfers.map((transfer: any) => transfer.transaction.id), ["auto-1", "target-1"],
          "the saved result needs both confirmed transfers of the Apply");
        assert.ok(last.commissioning.transfers.every((transfer: any) => transfer.status === "applied"
          && transfer.transaction.confirmation === "output-image"));
      });
    });

    test("a target change the runtime no longer reports under its journalled identity is not added to the receipt", async () => {
      await withFake("pid", true, async fake => {
        await fake.parameters.apply(fake.review);
        (RuntimeTransferClient.prototype as any).status = async () => confirmedTransfer("someone-else");
        fake.recovered = holdingRecording(fake.loop, commissioningTarget(fake.loop, fake.model, TASK, fake.binding));
        const checkpoint = { ...reattachedCheckpoint(fake, "auto"), transfer: { id: "target-1", program: "program", binding: "transfer" },
          observation: { startedAtProcessNs: 1, durationMs: 4000, transfers: [confirmedTransfer("auto-1")] } };
        const posts: any[] = [];
        const session = sessionWith(checkpoint, [], []);
        assert.strictEqual(await session.recoverApplyObservation(async payload => { posts.push(payload); }), true);
        assert.deepStrictEqual(posts[posts.length - 1].commissioning.transfers.map((transfer: any) => transfer.transaction.id), ["auto-1"]);
      });
    });
  });

  suite("the commissioning journal", () => {
    for (const kind of ["pid", "proportional"] as Kind[]) {
      for (const foundInAuto of [true, false]) {
        const mode = foundInAuto ? "auto" : "manual";
        test(`a ${kind} Apply found in ${mode} journals that mode with every checkpoint it writes`, async () => {
          await withFake(kind, foundInAuto, async fake => {
            const writes: any[] = [];
            const commission = await reviewedCommissioning(fake);
            commission.setCheckpointWriter(async (checkpoint: unknown) => { writes.push(structuredClone(checkpoint)); });
            await commission.run();
            assert.strictEqual(writes[0].stage, "prepared", "the first checkpoint is written before anything is applied");
            assert.ok(writes.length > 1, "the campaign writes further checkpoints");
            assert.ok(writes.every(checkpoint => checkpoint.foundMode === mode), "every checkpoint keeps the mode the loop was found in");
          });
        });
      }
    }
  });

  test("an Apply journals the noise its Auto entry allowed for, so that a restore allows the same (A6.4)", async () => {
    await withFake("pid", true, async fake => {
      const writes: any[] = [];
      const commission = await PidCommissioning.review(fake.loop, fake.model, fake.task, fake.parameters, fake.review, undefined, undefined, 0.05);
      commission.setCheckpointWriter(async (checkpoint: unknown) => { writes.push(structuredClone(checkpoint)); });
      await commission.run();
      assert.ok(writes.length > 1 && writes.every(checkpoint => checkpoint.processNoise === 0.05), "every checkpoint keeps the measured noise");
    });
  });

  suite("an interrupted restore (A6.2, A8.6)", () => {
    test("a PID loop found in Auto returns to Auto once its previous settings are back", async () => {
      await withFake("pid", true, async fake => {
        await fake.parameters.apply(fake.review);
        const writes: any[] = [];
        const recovery = await InterruptedCommissioningRecovery.review(fake.loop, interruptedCheckpoint(fake, "auto"), fake.parameters,
          async checkpoint => { writes.push(checkpoint); });
        await recovery.rollback();
        assert.strictEqual(fake.world.values["Main.Kp"], 1, "the previous gain is back");
        assert.strictEqual(inAuto(fake), true, "the loop returns to the mode it was found in");
        assert.strictEqual(lastWrite(fake), false, "the last write enters Auto");
        assert.strictEqual(recovery.modeStatement(), "the loop returned to Auto");
        assert.strictEqual(writes[writes.length - 1].stage, "recovered");
      });
    });

    test("a proportional loop found in Auto returns to Auto once its previous gain is back", async () => {
      await withFake("proportional", true, async fake => {
        await fake.parameters.apply(fake.review);
        const recovery = await InterruptedCommissioningRecovery.review(fake.loop, interruptedCheckpoint(fake, "auto"), fake.parameters,
          async () => undefined);
        await recovery.rollback();
        assert.strictEqual(fake.world.values["Main.Kp"], 1, "the previous gain is back");
        assert.strictEqual(inAuto(fake), true, "the loop returns to the mode it was found in");
        assert.strictEqual(lastWrite(fake), false, "the last write enters Auto");
        assert.strictEqual(recovery.modeStatement(), "the loop returned to Auto");
      });
    });

    test("a Control.PID loop found in Auto returns to Auto through its declared initialization, allowing Apply's measured noise", async () => {
      await withFake("library", true, async fake => {
        await fake.parameters.apply(fake.review);
        const proto: any = RuntimeTransferClient.prototype;
        const fakeApply = proto.apply;
        const autoEntries: any[] = [];
        proto.apply = async function (values: unknown, manual: boolean, tolerance: number, tracking: any, ...rest: unknown[]) {
          if (!manual) autoEntries.push({ tolerance, tracking });
          return fakeApply.call(this, values, manual, tolerance, tracking, ...rest);
        };
        const noise = 0.05;
        const recovery = await InterruptedCommissioningRecovery.review(fake.loop, { ...interruptedCheckpoint(fake, "auto"), processNoise: noise },
          fake.parameters, async () => undefined);
        assert.match(recovery.rollbackDescription(), /Then the loop returns to Auto, the mode it was found in\./);
        await recovery.rollback();
        assert.strictEqual(fake.world.values["Main.Kp"], 1, "the previous gain is back");
        assert.strictEqual(inAuto(fake), true, "the loop returns to the mode it was found in (A6.2, B6.4)");
        assert.strictEqual(recovery.modeStatement(), "the loop returned to Auto");
        assert.strictEqual(autoEntries.length, 1, "one Auto entry, once the previous settings are back");
        assert.strictEqual(autoEntries[0].tracking.kind, "declared-state-v1", "the entry goes through the declared initialization, as Apply's does");
        assert.strictEqual(autoEntries[0].tracking.align_output, true, "the entry starts from the current output");
        // A6.4 for the restored gains (Kp 1, Ki 0, Kd 0): Kp·2n.
        assert.ok(Math.abs(autoEntries[0].tolerance - 2 * 1 * noise) < 1e-12, `the entry allows ${autoEntries[0].tolerance}, A6.4 gives ${2 * noise}`);
      });
    });

    test("a loop found in Manual stays in Manual, and no Auto entry is written", async () => {
      await withFake("pid", false, async fake => {
        await fake.parameters.apply(fake.review);
        const recovery = await InterruptedCommissioningRecovery.review(fake.loop, interruptedCheckpoint(fake, "manual"), fake.parameters,
          async () => undefined);
        await recovery.rollback();
        assert.strictEqual(fake.world.values["Main.Kp"], 1, "the previous gain is back");
        assert.strictEqual(inAuto(fake), false, "the loop stays in Manual");
        assert.ok(fake.world.applies.every((manual: boolean) => manual), "no Auto entry is written");
        assert.strictEqual(recovery.modeStatement(), "the loop stays in Manual");
      });
    });

    test("a checkpoint written before the mode was journalled stays in Manual and says that the mode is unknown", async () => {
      await withFake("pid", true, async fake => {
        await fake.parameters.apply(fake.review);
        const recovery = await InterruptedCommissioningRecovery.review(fake.loop, interruptedCheckpoint(fake, undefined), fake.parameters,
          async () => undefined);
        assert.match(recovery.rollbackDescription(), /The mode the loop was found in is unknown, so it stays in Manual\./);
        await recovery.rollback();
        assert.strictEqual(fake.world.values["Main.Kp"], 1, "the previous gain is back");
        assert.strictEqual(inAuto(fake), false, "as before the mode was journalled, the restore leaves the loop in Manual");
        assert.match(recovery.modeStatement(), /the mode it was found in is unknown/);
      });
    });
  });

  suite("a departure found while reattached (B6.3, A8.5)", () => {
    test("restores the previous settings and returns a loop found in Auto to Auto", async () => {
      await withFake("pid", true, async fake => {
        await fake.parameters.apply(fake.review);
        fake.recovered = departingRecording();
        const writes: unknown[] = [], retained: unknown[] = [], posts: any[] = [];
        const session = sessionWith(reattachedCheckpoint(fake, "auto"), writes, retained);
        assert.strictEqual(await session.recoverApplyObservation(async payload => { posts.push(payload); }), true);
        const last = posts[posts.length - 1];
        assert.strictEqual(last.status, "failed");
        assert.match(last.reason, /left its permitted range.*; the previous settings were restored, and the loop returned to Auto\./);
        assert.strictEqual(fake.world.values["Main.Kp"], 1, "the previous gain is back");
        assert.strictEqual(inAuto(fake), true, "the loop returns to the mode it was found in");
        assert.strictEqual(lastWrite(fake), false, "the last write enters Auto");
      });
    });

    test("restores the previous settings and leaves a loop found in Manual in Manual", async () => {
      await withFake("pid", false, async fake => {
        await fake.parameters.apply(fake.review);
        fake.recovered = departingRecording();
        const writes: unknown[] = [], retained: unknown[] = [], posts: any[] = [];
        const session = sessionWith(reattachedCheckpoint(fake, "manual"), writes, retained);
        assert.strictEqual(await session.recoverApplyObservation(async payload => { posts.push(payload); }), true);
        const last = posts[posts.length - 1];
        assert.strictEqual(last.status, "failed");
        assert.match(last.reason, /; the previous settings were restored, and the loop stays in Manual\./);
        assert.strictEqual(fake.world.values["Main.Kp"], 1, "the previous gain is back");
        assert.strictEqual(inAuto(fake), false, "the loop keeps the mode it was found in");
      });
    });

    test("restores the previous settings and says that the mode is unknown for a checkpoint written before the mode was journalled", async () => {
      await withFake("pid", true, async fake => {
        await fake.parameters.apply(fake.review);
        fake.recovered = departingRecording();
        const writes: unknown[] = [], retained: unknown[] = [], posts: any[] = [];
        const session = sessionWith(reattachedCheckpoint(fake, undefined), writes, retained);
        assert.strictEqual(await session.recoverApplyObservation(async payload => { posts.push(payload); }), true);
        const last = posts[posts.length - 1];
        assert.strictEqual(last.status, "failed");
        assert.match(last.reason, /the mode it was found in is unknown/);
        assert.strictEqual(fake.world.values["Main.Kp"], 1, "the previous gain is back");
        assert.strictEqual(inAuto(fake), false, "as before the mode was journalled, the loop is left in Manual");
      });
    });
  });

  suite("the live observation's progress (B9.3, A3.6)", () => {
    for (const kind of ["pid", "proportional"] as Kind[]) {
      test(`the ${kind} Apply shows the process time elapsed and remaining while its response is recorded`, async () => {
        await withFake(kind, true, async fake => {
          fake.recording.mode = "running";
          const shown: string[] = [];
          const wait = { progress: (text: string) => { shown.push(text); }, cancelled: () => false, pollMs: 0, wallClockMs: () => 0 };
          const commission = await reviewedCommissioning(fake, wait);
          const running = commission.run();
          await until(() => shown.some(text => text.startsWith("Recording the response")));
          fake.recording.finish = true;
          await running;
          const progress = shown.filter(text => text.startsWith("Recording the response"));
          assert.match(progress[0], /^Recording the response: [\d.]+ s of 4 s elapsed, [\d.]+ s of process time remaining\.$/);
        });
      });
    }
  });

  suite("the journalled mode decodes (A6.2)", () => {
    test("a checkpoint decodes with the mode it was found in, and one written before the mode was journalled decodes without it", () => {
      const loop: any = { id: "loop:1", revision: "loop-revision", projectId: "project:1", runtimeId: "runtime:1",
        parameterBinding: { loop_id: "loop:1", task_name: "MainTask", parameters: [{ symbol: "Main.Kp", iec_type: "LREAL", unit: "1",
          convention: "gain", minimum: 0, maximum: 10 }], mode: { symbol: "Main.Manual", manual_value: true } } };
      const model = { revision: "model:1", source: "provided", validation: "unvalidated", evidence: [], delaySeconds: 0,
        kind: "transfer-function", numerator: [1], denominator: [1, 1],
        inputs: [{ id: "u", unit: "1", operatingRange: { minimum: -10, maximum: 10 } }],
        outputs: [{ id: "y", unit: "1", operatingRange: { minimum: -10, maximum: 10 } }] };
      const checkpoint: any = {
        id: "checkpoint:1", loopRevision: "loop-revision", stage: "completed", model,
        task: { referenceChange: 1, maximumError: 0.2, settlingBand: 0.1, settlingTimeSeconds: 1, durationSeconds: 2 },
        origin: { supports_request_id: true, transaction_id: "parameter:1", program_revision: "program:1", binding_revision: "binding:1", active_revision: "active:1", values: { "Main.Kp": 1 }, status: "applied", previous_revision: null, previous_values: null, reason: null },
        candidate: { "Main.Kp": 2 }, parameterOperation: { id: "parameter:1", program: "program:1", binding: "binding:1" },
        references: { "Main.SP": 0 }, binding: { loop_id: "loop:1" }, recoveryTracking: {}, tolerance: 0.1,
        recordingResult: { samples: [], rawSamples: [{}], evidence: { method: "runtime_scan_buffer", completeness: "incomplete", observedSampleCount: 1, missingScanRanges: [], droppedObservedSamples: 0, actualCycleTimingAvailable: false, freshnessAvailable: false, reason: "retained" } },
      };
      const legacy = decodeCommissioningCheckpoint(checkpoint, loop);
      assert.ok(legacy, "a checkpoint written before the mode was journalled still decodes");
      assert.strictEqual(legacy?.foundMode, undefined);
      assert.strictEqual(decodeCommissioningCheckpoint({ ...checkpoint, foundMode: "auto" }, loop)?.foundMode, "auto");
      assert.strictEqual(decodeCommissioningCheckpoint({ ...checkpoint, foundMode: "sideways" }, loop), undefined, "an unknown mode is malformed");
      assert.strictEqual(decodeCommissioningCheckpoint({ ...checkpoint, processNoise: 0.05 }, loop)?.processNoise, 0.05);
      assert.strictEqual(decodeCommissioningCheckpoint({ ...checkpoint, processNoise: -1 }, loop), undefined, "a negative noise is malformed");
    });
  });
});
