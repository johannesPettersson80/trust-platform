import * as assert from "assert";
import { MainWorkflowError, classifyMainWorkflowFailure, nextActionKind, nextActionLabel, presentMainWorkflowFailure, verificationFailure } from "../../controlStudio/mainWorkflowFailure";
import { PhysicalApprovalRequired } from "../../controlStudio/runtimeTarget";
import { AcquisitionLimitsRequired } from "../../controlStudio/acquisitionSettings";
import { ChannelRequirementsRequired } from "../../controlStudio/controllerChannelPreferences";
import { AutomaticControllerSelectionError } from "../../controlStudio/automaticControllerRecommendation";
import { RuntimeVerificationExecutionError } from "../../controlStudio/verificationClient";
import { RuntimeControlError } from "../../runtimeControlClient";
import { failureOf, recoverIdentification } from "../../controlStudio/engineerWorkflowSession";
import type { IdentificationRecovery } from "../../controlStudio/connectedIdentificationRecovery";
import type { AutomaticIdentificationResult } from "../../controlStudio/automaticIdentification";
import { decodeControlStudioHostCommand, decodeManualPreviewRequest } from "../../controlStudio/messages";
import { latestRequestGate, previewManualSettings } from "../../controlStudio/engineerActions/manualPreview";
import { assertGainSignMatchesAction } from "../../controlStudio/engineerActions/tune";
import { explicitExperiment } from "../../controlStudio/loopSetup/loopBindings";
import { gainScheduleLoopDefinitions } from "../../controlStudio/loopSetup/gainScheduleGenerator";
import { ExperimentRecordingOwner, experimentRecordingWindowMs, startExperimentWithRecording } from "../../controlStudio/experimentRecording";
import { pidSymbols, scope, TASK } from "./control-studio-cascade-fixtures";

suite("Control Studio next actions and failures", () => {
  test("classifies each failure by its error type and runtime code, never by its message", () => {
    const kind = (error: unknown) => classifyMainWorkflowFailure(error).nextAction;
    assert.strictEqual(kind(new MainWorkflowError("The model is missing.", { cause: "model-or-data", nextAction: "identify" })), "identify");
    assert.strictEqual(kind(new PhysicalApprovalRequired("parameter application")), "approve");
    assert.strictEqual(kind(new AcquisitionLimitsRequired()), "open-limits");
    assert.strictEqual(kind(new RuntimeVerificationExecutionError("The isolated run stopped.")), "none");
    for (const code of ["experiment_active", "transaction_pending", "controller_write_pending"]) {
      const failure = classifyMainWorkflowFailure(new RuntimeControlError("A run is in progress.", code));
      assert.deepStrictEqual([failure.nextAction, failure.summary], ["wait", "Wait for the running operation to finish."], code);
    }
    assert.strictEqual(kind(new RuntimeControlError("Approve the change on the page.", "physical_approval_required")), "approve");
    for (const code of ["invalid_target", "invalid_experiment", "controller_unavailable"]) {
      assert.strictEqual(kind(new RuntimeControlError("The target is not set up.", code)), "open-loop-setup", code);
    }
    for (const code of ["outside_limits"]) {
      assert.strictEqual(kind(new RuntimeControlError("The signal is outside its limits.", code)), "open-limits", code);
    }
    assert.strictEqual(kind(new RuntimeControlError("Something else happened.", "unexpected_code")), "none");
    assert.strictEqual(kind(new RuntimeControlError("No code was given.")), "none");
    // A plain error is "none" whatever its words say: connection, identification and model text are not types.
    for (const text of ["Connection refused: connect the runtime", "Identification failed: no adequate model", "The model is missing"]) {
      assert.strictEqual(kind(new Error(text)), "none", text);
    }
    assert.strictEqual(failureOf(new AutomaticControllerSelectionError([])).nextAction, "open-requirements");
    assert.strictEqual(failureOf(new ChannelRequirementsRequired("A channel requirement is missing.")).nextAction, "open-requirements");
  });

  test("maps every legacy next action to its typed kind", () => {
    const legacy = ["configure-acquisition", "configure-scenarios", "find-model", "retune", "choose-controller", "revise-requirements",
      "review-implementation", "choose-application-mode", "reapply"] as const;
    assert.deepStrictEqual(legacy.map(value => nextActionKind(value)),
      ["open-limits", "open-requirements", "identify", "tune", "choose-method", "open-requirements", "open-loop-setup", "none", "validate"]);
    assert.strictEqual(new MainWorkflowError("A retune is needed.", { cause: "settings", nextAction: "retune" }).failure.nextAction, "tune",
      "a module that still throws a legacy value gets the typed kind");
    assert.strictEqual(nextActionLabel("choose-method"), "Choose another method");
    assert.strictEqual(nextActionLabel("wait"), "", "a wait offers no button");
  });

  test("never offers identify for a failure that is not typed as needing a new model", () => {
    const plain = new Error("Identification failed: the fit did not converge.");
    assert.strictEqual(presentMainWorkflowFailure(plain.message, classifyMainWorkflowFailure(plain)).nextAction.kind, "none");
    assert.strictEqual(verificationFailure([{ id: "settling", status: "Inconclusive" }]).nextAction, "none");
    assert.strictEqual(verificationFailure([]).nextAction, "none");
    assert.strictEqual(verificationFailure([{ id: "settling", status: "Fail" }]).nextAction, "tune");
  });

  test("splits a failure into its first sentence and its full text, without the Next suffix", () => {
    const text = "Gain 0.21 °C per % is too small for this step. Choose a larger step or a longer hold. Next: Identify plant.";
    const presented = presentMainWorkflowFailure(text, { cause: "model-or-data", nextAction: "identify" });
    assert.strictEqual(presented.message, "Gain 0.21 °C per % is too small for this step.", "the decimal point does not end the sentence");
    assert.strictEqual(presented.detail, "Gain 0.21 °C per % is too small for this step. Choose a larger step or a longer hold.");
    assert.deepStrictEqual(presented.nextAction, { kind: "identify", label: "Identify plant" });
    const single = presentMainWorkflowFailure("Runtime stopped.", { cause: "runtime", nextAction: "connect-runtime" });
    assert.strictEqual(single.message, "Runtime stopped.");
    assert.strictEqual("detail" in single, false, "a one-sentence failure has no separate detail");
  });

  test("routes each host command and manual preview only within the loop's scope", () => {
    const loopScope = { projectId: "project:p", editorSessionId: "editor-1", loopId: "loop:heater", loopRevision: "sha256:r1" };
    const envelope = (kind: string, payload: unknown, overrides: Record<string, unknown> = {}) => ({ schemaVersion: 1, kind,
      messageId: "message-1", projectId: loopScope.projectId, editorSessionId: loopScope.editorSessionId, loopId: loopScope.loopId,
      expectedLoopRevision: loopScope.loopRevision, payload, ...overrides });
    assert.strictEqual(decodeControlStudioHostCommand(envelope("control-studio.host-command", { command: "connect-runtime" }), loopScope), "connect-runtime");
    assert.strictEqual(decodeControlStudioHostCommand(envelope("control-studio.host-command", { command: "open-loop-setup" }), loopScope), "open-loop-setup");
    assert.strictEqual(decodeControlStudioHostCommand(envelope("control-studio.host-command", { command: "delete-project" }), loopScope), undefined);
    assert.strictEqual(decodeControlStudioHostCommand(envelope("control-studio.host-command", { command: "connect-runtime", extra: true }), loopScope), undefined);
    assert.strictEqual(decodeControlStudioHostCommand(envelope("control-studio.host-command", { command: "connect-runtime" }, { loopId: "loop:other" }), loopScope), undefined);
    assert.strictEqual(decodeControlStudioHostCommand(envelope("control-studio.engineer.manual-preview", { command: "connect-runtime" }), loopScope), undefined);
    const preview = decodeManualPreviewRequest(envelope("control-studio.engineer.manual-preview", { settings: { "P1.Kp": 2, "P1.Ki": "x" } }), loopScope);
    assert.deepStrictEqual(preview?.settings, { "P1.Kp": 2, "P1.Ki": Number.NaN }, "a value that is not a number is kept so that the preview refuses it");
    assert.strictEqual(decodeManualPreviewRequest(envelope("control-studio.engineer.manual-preview", { settings: { "P1.Kp": 2 } },
      { expectedLoopRevision: "sha256:old" }), loopScope), undefined);
  });

  test("answers only the latest manual preview and refuses a setting outside its bounds", async () => {
    const gate = latestRequestGate();
    const first = gate.begin(), second = gate.begin();
    assert.deepStrictEqual([gate.isLatest(first), gate.isLatest(second)], [false, true], "a superseded preview is dropped");
    const loop = { parameterBinding: { parameters: [{ symbol: "P1.Kp", iec_type: "REAL", unit: "%/degC", convention: "gain", minimum: 0, maximum: 10 }] } } as any;
    const host = { state: { controllerChoice: "P", family: "P", settingsPins: undefined, task: undefined },
      model: () => ({ revision: "sha256:m" }), loop: () => loop, runningControllerGains: async () => undefined } as any;
    const refused = await previewManualSettings(host, { "P1.Kp": 50 });
    assert.strictEqual(refused.status, "failed");
    assert.deepStrictEqual((refused.failure as any).nextAction, { kind: "none", label: "" });
    assert.match(String(refused.reason), /P1\.Kp must be between 0 and 10 %\/degC; 50 is outside that range\./);
    const unknown = await previewManualSettings(host, { "P1.Ki": 1 });
    assert.strictEqual((unknown.failure as any).nextAction.kind, "none");
  });

  test("sets up a loop without an identification step or duration, writing only the values the engineer set", () => {
    const kind = { execution: "control-studio" as const, kind: "mv_step" as const };
    assert.deepStrictEqual(explicitExperiment("Heater", kind, undefined, undefined), {}, "neither value set: no experiment is written");
    assert.deepStrictEqual(explicitExperiment("Heater", kind, 10, 60).experiment,
      { execution: "control-studio", kind: "mv_step", amplitude: 10, durationSeconds: 60, seed: 1 });
    assert.throws(() => explicitExperiment("Heater", kind, undefined, 60), /needs both the output step and the identification duration/);
    assert.throws(() => explicitExperiment("Heater", kind, 10, 0.5), /identification duration between 1 s and 24 h/);
  });

  test("gives a Test recording its planned duration plus a restoration margin, within the recording limit", async () => {
    assert.strictEqual(experimentRecordingWindowMs(60_000), 70_000, "the margin is at least 10 s");
    assert.strictEqual(experimentRecordingWindowMs(3_600_000), 3_960_000, "the margin is 10 % of a longer hold");
    assert.strictEqual(experimentRecordingWindowMs(86_400_000), 86_400_000, "the window is capped at the recording limit");
    let planned: number | undefined;
    const owner = new ExperimentRecordingOwner(async (loop: any, _persistence?: unknown, plannedDurationMs?: number) => {
      planned = plannedDurationMs;
      return { loop, stop: async () => { throw new Error("not stopped in this test"); } } as any;
    });
    // The recording identity is journaled before Start is sent (A8.5, E11.2), so the test supplies a journal.
    const journal = { persist: async () => undefined, clear: async () => undefined };
    const result = await startExperimentWithRecording(owner, {} as any, async () => "started", journal, undefined, experimentRecordingWindowMs(60_000));
    assert.strictEqual(result, "started");
    assert.strictEqual(planned, 70_000, "the runtime window is the planned duration with its margin");
  });

  test("refuses a model whose gain contradicts the controller action before any design runs", () => {
    const loop = (action: "direct" | "reverse" | undefined) => ({ controller: { action },
      signals: { processVariable: { unit: "degC" }, manipulatedVariable: { unit: "%" } } }) as any;
    const transfer = (numerator: number[], denominator: number[], inputs = 1, outputs = 1) => ({ kind: "transfer-function",
      inputs: Array(inputs).fill({}), outputs: Array(outputs).fill({}), numerator, denominator, revision: "sha256:m" }) as any;
    assert.throws(() => assertGainSignMatchesAction(loop("reverse"), transfer([2], [5, 1])), (error: Error) => error instanceof MainWorkflowError
      && (error as MainWorkflowError).failure.nextAction === "open-loop-setup" && /the measurement rises when the output rises/.test(error.message),
    "a positive static gain under a reverse action is refused, with the next action open-loop-setup");
    assert.doesNotThrow(() => assertGainSignMatchesAction(loop("direct"), transfer([2], [5, 1])), "a positive gain agrees with direct");
    assert.doesNotThrow(() => assertGainSignMatchesAction(loop(undefined), transfer([2], [5, 1])), "a missing action counts as direct");
    assert.throws(() => assertGainSignMatchesAction(loop(undefined), transfer([-2], [5, 1])), MainWorkflowError, "a negative gain contradicts direct");
    assert.doesNotThrow(() => assertGainSignMatchesAction(loop("reverse"), transfer([-2], [5, 1])), "a negative gain agrees with reverse");
    assert.doesNotThrow(() => assertGainSignMatchesAction(loop("reverse"), transfer([2], [5, 1], 2, 1)), "several inputs have no one scalar gain");
    assert.doesNotThrow(() => assertGainSignMatchesAction(loop("reverse"),
      { kind: "discrete-state-space", inputs: [{}], outputs: [{}], revision: "sha256:s" } as any), "a state model is not checked here");
    assert.throws(() => assertGainSignMatchesAction(loop("reverse"), transfer([1], [2, 0])), (error: Error) => error instanceof MainWorkflowError
      && /the measurement rises when the output rises/.test(error.message), "an integrator's positive rate gain is refused under reverse");
  });

  test("bounds each scheduled gain's magnitude, so a loop whose gains are negative can be set up", () => {
    const catalog = {
      symbols: [
        { symbol: "P1.LevelM", type: "LREAL", address: "%IL0" }, { symbol: "P1.SetpointM", type: "LREAL", address: "%ML0" },
        { symbol: "P1.ValvePct", type: "LREAL", address: "%QL0" }, { symbol: "P1.ManualPct", type: "LREAL", address: "%ML8" },
        { symbol: "P1.Auto", type: "BOOL" }, { symbol: "P1.KpLow", type: "LREAL" }, { symbol: "P1.KiLow", type: "LREAL" },
        { symbol: "P1.KpHigh", type: "LREAL" }, { symbol: "P1.KiHigh", type: "LREAL" },
        ...pidSymbols("P1.Loop", "P1.Parameters"),
      ],
      controllers: [{ instance: "P1", type: "Main", tasks: TASK }, { instance: "P1.Loop", type: "Control.PID", tasks: TASK }],
      links: [
        { instance: "P1.Loop", method: "Configure", argument: "Parameters", record: "P1.Parameters", output: null, fields: {} },
        { instance: "P1.Loop", method: "Update", argument: "Input", record: "P1.Input", output: "P1.ValvePct",
          fields: { ManualOutput: ["P1.ManualPct"], ProcessValue: ["P1.LevelM"], Setpoint: ["P1.SetpointM"] } },
      ],
      writes: ["P1.ValvePct", "P1.Parameters.ProportionalGain", "P1.Parameters.IntegralGain"],
    };
    const points = [{ name: "Low", proportionalGain: "P1.KpLow", integralGain: "P1.KiLow" }, { name: "High", proportionalGain: "P1.KpHigh", integralGain: "P1.KiHigh" }];
    const draft = { ...scope, name: "Tank level", instance: "P1.Loop", action: "direct", parameterRecord: "P1.Parameters",
      processVariable: { symbol: "P1.LevelM", unit: "m", minimum: 0, maximum: 2 }, manipulatedVariable: { symbol: "P1.ValvePct", unit: "%", minimum: 0, maximum: 100 },
      setpoint: { symbol: "P1.SetpointM" }, mode: { symbol: "P1.Auto", whenTrue: "Auto" }, manualOutput: "P1.ManualPct",
      experimentStep: 10, experimentDurationSeconds: 120, points };
    const loops = gainScheduleLoopDefinitions(catalog as any, draft as any);
    assert.strictEqual(loops.length, 2, "one loop per operating point");
    for (const loop of loops) {
      const gains = loop.parameterBinding!.parameters;
      assert.strictEqual(gains.length, 2, "each operating point declares its proportional and integral gain");
      for (const gain of gains) {
        assert.strictEqual(gain.minimum, -gain.maximum, `${gain.symbol} has a range symmetric about zero`);
        assert.ok(gain.maximum > 0, `${gain.symbol} keeps a positive magnitude bound`);
      }
    }
  });

  test("a journal holding a running campaign resumes and fits, and does not cancel", async () => {
    const found = { model: { revision: "sha256:fit" }, runs: [] } as unknown as AutomaticIdentificationResult;
    const calls: string[] = [];
    const recovery: IdentificationRecovery = {
      cancel: async () => { calls.push("cancel"); },
      resume: async progress => { calls.push("resume"); await progress("Following the running experiment"); return found; },
      snapshot: () => undefined,
    };
    const payload = await recoverIdentification(recovery, result => {
      calls.push("store");
      assert.strictEqual(result, found, "the resumed campaign's fit is the result that is stored");
      return { status: "model-ready" };
    });
    assert.deepStrictEqual(calls, ["resume", "store"], "a running campaign is resumed, never cancelled");
    assert.deepStrictEqual(payload, { status: "model-ready" });
  });

  test("a journal without a resumable campaign is cancelled to restore the original mode", async () => {
    const calls: string[] = [];
    const recovery: IdentificationRecovery = { cancel: async () => { calls.push("cancel"); }, snapshot: () => undefined };
    const payload = await recoverIdentification(recovery, () => { calls.push("store"); return { status: "model-ready" }; });
    assert.deepStrictEqual([calls, payload], [["cancel"], undefined]);
  });
});
