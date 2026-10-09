import * as assert from "assert";
import * as fs from "fs";
import * as path from "path";
import * as vscode from "vscode";
import { configuredPlantRequest, type VerificationPlant } from "../../controlStudio/configuredPlantVerification";
import { controllerAdapter } from "../../controlStudio/controllerAdapters";
import { bindRuntimeConnection } from "../../controlStudio/runtimeConnection";
import { ParameterApplicationClient } from "../../controlStudio/parameterApplication";
import { RuntimeTransferClient } from "../../controlStudio/runtimeTransfer";
import { RuntimeVerificationClient } from "../../controlStudio/verificationClient";
import { engineerVerificationCampaign, campaignStatus } from "../../controlStudio/engineerVerificationCampaign";
import { tuneProjectController } from "../../controlStudio/projectController";
import type { GeneralModel } from "../../controlStudio/generalModel";
import type { EngineerRequirements } from "../../controlStudio/engineerVerificationPlan";
import { runtimeLifecycleService } from "../../runtimeLifecycle";

suite("Control Studio native PID group", () => {
  test("tunes validates applies and restores two actual library PIDs without changing an unselected PID", async function () {
    if (process.env.TRUST_CONTROL_STUDIO_PID_GROUP_NATIVE !== "1") this.skip();
    this.timeout(180000);
    const project = vscode.workspace.workspaceFolders![0].uri.fsPath;
    const loop = JSON.parse(fs.readFileSync(path.join(project, "control-loop.json"), "utf8"));
    const task: EngineerRequirements = { referenceChange: 1, maximumError: 0.15, settlingBand: 0.03, settlingTimeSeconds: 18, durationSeconds: 24 };
    for (const channel of loop.channels.outputs) channel.requirements = { ...task };
    const adapter = controllerAdapter(loop);
    assert.deepStrictEqual(adapter.families, ["paired-PI"]);
    // Declared fixture equations; this test proves actual-ST group ownership, not identification accuracy.
    const coordinate = (id: string, unit: string) => ({ id, unit, operatingRange: { minimum: 0, maximum: 100 } });
    const model: GeneralModel = {
      kind: "continuous-state-space", revision: "declared-group-fixture", source: "provided", validation: "validated", evidence: ["fixture plant equations"],
      inputs: [coordinate("u1", "%"), coordinate("u2", "%")], outputs: [coordinate("y1", "°C"), coordinate("y2", "°C")],
      states: [coordinate("x1", "°C"), coordinate("x2", "°C")],
      a: [[-0.25, 0], [0, -0.25]], b: [[0.375, 0], [0, 0.375]], c: [[1, 0], [0, 1]], d: [[0, 0], [0, 0]],
      delaySeconds: 0.5, operatingPoint: { inputs: [0, 0], outputs: [20, 20] },
    };
    const started = await runtimeLifecycleService.startLocalSimulator();
    assert.strictEqual(started.ok, true, JSON.stringify(started));
    let verification: RuntimeVerificationClient | undefined;
    try {
      const connection = await bindRuntimeConnection(loop.runtimeId);
      if (connection.source !== "simulation") throw new Error("The test must own its simulator.");
      const groupParameters = new ParameterApplicationClient(loop.parameterBinding, connection.source, connection.request);
      const before = await groupParameters.status();
      const single = JSON.parse(fs.readFileSync(path.resolve(__dirname, "../../../../../examples/control-pid-commissioning/control-loop.json"), "utf8").replace(/P1\./g, "P3."));
      single.controller.modeSignal = "P3.AutoMode"; single.parameterBinding.mode.symbol = "P3.AutoMode";
      const unselectedParameters = new ParameterApplicationClient(single.parameterBinding, connection.source, connection.request);
      const unselectedBefore = await unselectedParameters.status();
      const unselectedTransfer = new RuntimeTransferClient(controllerAdapter(single).transferBinding(), before.program_revision, connection.request);
      const unselectedStates = (await unselectedTransfer.status()).values;
      verification = new RuntimeVerificationClient({ source: "simulation", request: connection.request });
      const readiness = await connection.request("control.simulation.run.status", {}) as { project_revision: string };
      const design = await tuneProjectController(loop, model, task, before.values, {}, readiness.project_revision, verification,
        () => false, () => undefined, () => undefined, async () => undefined, "paired-PI");
      const values = adapter.materialize({ parameters: design.candidate.parameters }, before.values);
      assert.strictEqual(values["P1.Parameters.ProportionalGain"], 1.5);
      assert.strictEqual(values["P1.SecondaryParameters.ProportionalGain"], 1.5);
      const campaign = engineerVerificationCampaign(loop, model, task, values);
      assert.strictEqual(campaign.length, 3, "Validate the joint response and each reference independently");
      // This native check runs the project's configured plant directly; the binding rule of B5.3 is covered by control-studio-apply-process-time.test.ts.
      const configured: VerificationPlant = { kind: "configured", plantId: "project-plant" };
      const results = [];
      for (const scenario of campaign) {
        const request = configuredPlantRequest(loop, scenario.request, configured);
        results.push({ id: scenario.id, result: await verification.verify(request, scenario.requirements) });
      }
      assert.strictEqual(campaignStatus(campaign.map(scenario => scenario.id), results), "Pass", JSON.stringify(results.map(item => ({ id: item.id, evaluation: item.result.evaluation }))));
      const initialization = adapter.initialization(model, task, values);
      assert.strictEqual((await verification.verify(configuredPlantRequest(loop, initialization.request, configured), initialization.requirements)).evaluation.status, "Pass");
      assert.deepStrictEqual((await groupParameters.status()).values, before.values, "Tune and Validate must leave the running controllers unchanged");
      const review = await groupParameters.review(values);
      const commissioning = await adapter.commission(model, task, groupParameters, review);
      const result = await commissioning.run();
      assert.strictEqual(result.status, "passed", result.reason);
      const transfer = result.transfers.find(item => item.transaction?.applied_values?.["P1.Secondary.StateValue.NominalOutput"] !== undefined)!;
      assert.ok(transfer, "One native transfer adopts both members' retained states");
      for (const state of loop.controller.project.initialization.states) {
        assert.ok(transfer.transaction?.applied_values?.[state.symbol] !== undefined, state.symbol);
        assert.ok(transfer.transaction?.observed_values?.[state.symbol] !== undefined, state.symbol);
      }
      assert.strictEqual(transfer.transaction?.confirmation, "output-image");
      assert.deepStrictEqual((await unselectedParameters.status()).values, unselectedBefore.values);
      assert.deepStrictEqual((await unselectedTransfer.status()).values, unselectedStates, "The unselected controller's states and mode remain untouched");
      const restored = await commissioning.rollback();
      assert.deepStrictEqual(restored.values, before.values, "Restore the complete selected parameter union");
      assert.deepStrictEqual((await unselectedParameters.status()).values, unselectedBefore.values);
      assert.deepStrictEqual((await unselectedTransfer.status()).values, unselectedStates);
    } finally { verification?.dispose(); await runtimeLifecycleService.stopRuntime(); }
  });
});
