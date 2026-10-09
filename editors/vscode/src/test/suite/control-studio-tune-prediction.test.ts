import * as assert from "assert";
import { EngineerWorkflowSession } from "../../controlStudio/engineerWorkflowSession";

/** A cold room whose program switches a compressor; its law is ST that Studio does not calculate itself. */
function coldRoomLoop() {
  const { onOffLoopDefinition } = require("../../controlStudio/loopSetup/pidLoopSetup");
  return onOffLoopDefinition({
    symbols: [
      { symbol: "P1.RoomC", type: "LREAL", address: "%IL0" }, { symbol: "P1.CompressorPct", type: "LREAL", address: "%QL0" },
      { symbol: "P1.SetpointC", type: "LREAL", address: "%ML0" }, { symbol: "P1.ManualPct", type: "LREAL", address: "%ML8" },
      { symbol: "P1.AutoMode", type: "BOOL" }, { symbol: "P1.CoolingOn", type: "BOOL" },
      ...["HysteresisC", "MinimumOnS", "MinimumOffS"].map(name => ({ symbol: `P1.${name}`, type: "LREAL" })),
    ],
    controllers: [{ instance: "P1", type: "Main", tasks: [{ name: "MainTask", intervalMs: 100 }] }], links: [],
  }, { projectId: "project:tune", runtimeId: "runtime-1", name: "Cold room", action: "reverse", instance: "P1",
    processVariable: { symbol: "P1.RoomC", unit: "degC", minimum: -10, maximum: 20 }, setpoint: { symbol: "P1.SetpointC" },
    manipulatedVariable: { symbol: "P1.CompressorPct", unit: "%", minimum: 0, maximum: 100 }, mode: { symbol: "P1.AutoMode", whenTrue: "Auto" },
    manualOutput: "P1.ManualPct", hysteresis: "P1.HysteresisC", bandConvention: "half-band", minimumOn: "P1.MinimumOnS",
    minimumOff: "P1.MinimumOffS", latch: "P1.CoolingOn", equipmentMinimumOnSeconds: 20, equipmentMinimumOffSeconds: 20,
    experimentStep: 100, experimentDurationSeconds: 300 });
}

suite("Control Studio Tune prediction for laws in the program", () => {
  test("predicts the current and the tuned switching settings by running the actual ST on the selected model", async () => {
    const loop = coldRoomLoop();
    const model: any = { kind: "transfer-function", numerator: [-0.1], denominator: [60, 1], delaySeconds: 0, revision: "room", source: "fitted",
      validation: "validated", evidence: ["recording"], inputs: [{ id: "P1.CompressorPct", unit: "%", operatingRange: { minimum: 0, maximum: 100 } }],
      outputs: [{ id: "P1.RoomC", unit: "degC", operatingRange: { minimum: 2, maximum: 12 } }], operatingPoint: { inputs: [0], outputs: [12] } };
    const search = require("../../controlStudio/runtimeSearchConnection");
    const parameters = require("../../controlStudio/runtimeParameterConnection");
    const verification = require("../../controlStudio/verificationClient");
    const oldSearch = search.connectRuntimeSearch, oldParameters = parameters.connectRuntimeParameters, oldVerify = verification.RuntimeVerificationClient.prototype.verify;
    const current = { "P1.HysteresisC": 1, "P1.MinimumOnS": 20, "P1.MinimumOffS": 20 };
    search.connectRuntimeSearch = async () => ({ source: "simulation", request: async (operation: string) => {
      if (operation === "control.simulation.run.status") return { ready: true, project_revision: "project", execution: "isolated-simulation", source_basis: "loaded-runtime-sources" };
      throw new Error(`unexpected search operation ${operation}`);
    } });
    parameters.connectRuntimeParameters = async () => ({ source: "simulation", client: {
      status: async () => ({ values: current, program_revision: "program", binding_revision: "binding", active_revision: "active", status: "applied",
        previous_revision: null, previous_values: null, reason: null }) } });
    const requests: any[] = [];
    verification.RuntimeVerificationClient.prototype.verify = async function (request: any) {
      requests.push(request);
      const samples = [0, 1].map(time => ({ timeSeconds: time, sequence: time, values: { "P1.RoomC": { value: 12 - time, quality: "good" } } }));
      return { projectRevision: "project", request, run: { samples }, evaluation: { status: "Pass", run: { samples }, requirements: [], specification: [] } };
    };
    const session = new EngineerWorkflowSession(loop, () => model, () => undefined);
    const scope = { projectId: loop.projectId, editorSessionId: "tune-prediction", loopId: loop.id, loopRevision: loop.revision };
    const posts: any[] = [];
    try {
      await session.handle({ schemaVersion: 1, kind: "control-studio.engineer.tune", messageId: "tune", projectId: scope.projectId,
        editorSessionId: scope.editorSessionId, loopId: scope.loopId, expectedLoopRevision: scope.loopRevision,
        payload: { family: "ON/OFF", referenceChange: -7, maximumError: 2, settlingBand: 0.5, settlingTimeSeconds: 90, durationSeconds: 300, applicationMode: "auto" } },
      scope, async value => { posts.push(value); });
      const candidate = posts.find(post => post.status === "candidate");
      assert.ok(candidate, `Tune produces a candidate: ${JSON.stringify(posts.map(post => post.reason ?? post.status))}`);
      assert.ok(candidate.modelPrediction?.candidate?.run?.samples?.length && candidate.modelPrediction?.baseline?.run?.samples?.length,
        "the candidate carries the predicted responses of the tuned and the current settings");
      const hysteresis = (request: any) => request.parameter_overrides.find((parameter: any) => parameter.symbol === "P1.HysteresisC")?.value;
      assert.deepStrictEqual(requests.map(hysteresis), [1, 0.5], "the current settings and the tuned ones run on the same model");
    } finally {
      session.dispose(); search.connectRuntimeSearch = oldSearch; parameters.connectRuntimeParameters = oldParameters;
      verification.RuntimeVerificationClient.prototype.verify = oldVerify;
    }
  });
});
