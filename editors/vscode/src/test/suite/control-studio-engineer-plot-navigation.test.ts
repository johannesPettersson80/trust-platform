import * as assert from "assert";
import { engineerPlotForStage, type EngineerStage } from "../../controlStudio/webview/engineerPlotNavigation";

suite("Control Studio engineer plot navigation", () => {
  test("keeps each permanent stage bound to its evidence source", () => {
    assert.deepStrictEqual(
      (["model", "tune", "verify", "apply"] as EngineerStage[]).map(stage => engineerPlotForStage(stage, {
        model: true, candidate: true, predictedResponse: true, recordedResponses: true, samples: true,
      }).id),
      ["fit", "engineering-response", "recorded-responses", "response"],
    );
  });

  test("uses the rendered supplied-model response when one is available", () => {
    assert.strictEqual(engineerPlotForStage("model", { suppliedModel: true }).id, "general-model");
    assert.strictEqual(engineerPlotForStage("model", { suppliedModel: true }).available, true);
  });

  test("prioritizes measured model fit over its optional frequency analysis", () => {
    assert.strictEqual(engineerPlotForStage("model", { model: true, suppliedModel: true }).id, "fit");
  });

  test("does not expose predictions as Verify or Apply evidence", () => {
    const predictionOnly = { model: true, candidate: true };
    assert.strictEqual(engineerPlotForStage("verify", predictionOnly).available, false);
    assert.match(engineerPlotForStage("verify", predictionOnly).reason, /actual ST/i);
    assert.strictEqual(engineerPlotForStage("apply", predictionOnly).available, false);
    assert.match(engineerPlotForStage("apply", predictionOnly).reason, /live response/i);
  });

  test("reports missing model and tuning evidence without fabricating a plot", () => {
    assert.deepStrictEqual(engineerPlotForStage("model", {}), {
      id: "fit", label: "Model fit", available: false,
      reason: "Find or supply a model to view its response.",
    });
    assert.deepStrictEqual(engineerPlotForStage("tune", {}), {
      id: "engineering-response", label: "Predicted response", available: false,
      reason: "Generate a qualified tuning result to view its predicted response.",
    });
  });
});
