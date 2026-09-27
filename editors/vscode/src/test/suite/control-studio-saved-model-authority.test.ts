import * as assert from "assert";
import { IdentificationWorkflow, type IdentificationSample } from "../../controlStudio/identification";
import { acceptedIdentificationModel } from "../../controlStudio/selectedModelAnalysis";
import { StudioAnalysisSessions } from "../../controlStudio/studioAnalysisSessions";

const loop: any = {
  id: "control-lesson-23", projectId: "project:lesson-15", runtimeId: "runtime:lesson-15",
  name: "Lesson 15", revision: "sha256:loop", schemaVersion: 2,
  controller: { instance: "P1", type: "Control.PID", revision: "sha256:controller", action: "direct", taskPeriodMs: 100 },
  signals: {
    processVariable: { symbol: "P1.PV", unit: "degC", range: { minimum: 0, maximum: 100 } },
    manipulatedVariable: { symbol: "P1.MV", unit: "%", range: { minimum: 0, maximum: 100 } },
  },
  limits: { outputMinimum: 0, outputMaximum: 100 }, observedSampleTiming: { nominalCycleSeconds: .1 },
};
const samples = (change: number): IdentificationSample[] => Array.from({ length: 100 }, (_, index) => ({
  timeSeconds: index / 10, input: index < 20 ? 50 : 50 + change,
  output: index < 20 ? 20 : 20 + change * .5 * (1 - Math.exp(-(index / 10 - 2) / 3)),
  quality: "good", inputUnit: "%", outputUnit: "degC",
}));

function accepted() {
  const workflow = new IdentificationWorkflow() as any;
  workflow.qualify(samples(10), [], "Simulation", "first_order", "estimation");
  workflow.fit();
  workflow.qualify(samples(12), [], "Simulation", "first_order", "validation");
  workflow.fit();
  return { workflow, accepted: workflow.accept() };
}

function resultFixture() {
  const { workflow, accepted: modelReview } = accepted();
  const channels = {
    inputs: [{ id: "P1.MV", unit: "%", operatingRange: { minimum: 0, maximum: 100 } }],
    outputs: [{ id: "P1.PV", unit: "degC", operatingRange: { minimum: 0, maximum: 100 } }],
  };
  const selectedModel = acceptedIdentificationModel(modelReview, channels);
  return {
    workflow,
    modelReview,
    selectedModel,
    result: {
      identificationStudy: JSON.parse(JSON.stringify(workflow.snapshot())),
      model: JSON.parse(JSON.stringify(modelReview.model)),
      modelFit: JSON.parse(JSON.stringify(modelReview.fit)),
      engineerStudy: { selectedModel: JSON.parse(JSON.stringify(selectedModel)), campaigns: [] },
      generalModelAnalysis: { model: JSON.parse(JSON.stringify(selectedModel)) },
    } as any,
  };
}

suite("Control Studio saved model authority", () => {
  test("rejects conflicting supplied analysis and selected model before restoring any owner", () => {
    const fixture = resultFixture(), sessions = new StudioAnalysisSessions();
    sessions.identification.restore(fixture.result.identificationStudy);
    const before = sessions.identification.snapshot();
    const result = structuredClone(fixture.result);
    delete result.identificationStudy;
    result.engineerStudy.selectedModel.numerator[0] += 1;
    assert.throws(() => sessions.restoreIdentification(result, loop), /contradict|match/i);
    assert.deepStrictEqual(sessions.identification.snapshot(), before);
  });
  test("rejects each contradictory dependent model and fit before changing active sessions", () => {
    const fixture = resultFixture();
    const sessions = new StudioAnalysisSessions() as any;
    sessions.identification.restore(fixture.result.identificationStudy);
    const supplied = { ...fixture.selectedModel, source: "provided", validation: "unvalidated", evidence: [] };
    sessions.model.selectModel(supplied);
    const beforeIdentification = JSON.stringify(sessions.identification.snapshot());
    const beforeModel = JSON.stringify(sessions.model.selectedModel());
    const mutations = [
      (result: any) => { result.model.gain += 1; },
      (result: any) => { result.modelFit.rootMeanSquareError += 1; },
      (result: any) => { result.engineerStudy.selectedModel.numerator[0] += 1; },
      (result: any) => { result.generalModelAnalysis.model.numerator[0] += 1; },
    ];
    for (const mutate of mutations) {
      const contradictory = JSON.parse(JSON.stringify(fixture.result));
      mutate(contradictory);
      assert.throws(() => sessions.restoreIdentification(contradictory, loop), /contradict|authority|match/i);
      assert.strictEqual(JSON.stringify(sessions.identification.snapshot()), beforeIdentification);
      assert.strictEqual(JSON.stringify(sessions.model.selectedModel()), beforeModel);
    }
  });

  test("compares model content even when a forged dependent revision is retained", () => {
    const fixture = resultFixture();
    const contradictory = JSON.parse(JSON.stringify(fixture.result));
    contradictory.engineerStudy.selectedModel.numerator[0] += 1;
    contradictory.engineerStudy.selectedModel.revision = fixture.selectedModel.revision;
    assert.throws(() => (new StudioAnalysisSessions() as any).restoreIdentification(contradictory, loop), /contradict|authority|match/i);
  });

  test("accepts equivalent dependent models with reordered object keys", () => {
    const fixture = resultFixture();
    const reordered = (value: Record<string, unknown>) => Object.fromEntries(Object.entries(value).reverse());
    const equivalent = JSON.parse(JSON.stringify(fixture.result));
    equivalent.engineerStudy.selectedModel = reordered(equivalent.engineerStudy.selectedModel);
    equivalent.generalModelAnalysis.model = reordered(equivalent.generalModelAnalysis.model);
    assert.doesNotThrow(() => (new StudioAnalysisSessions(loop) as any).restoreIdentification(equivalent));
  });

  test("keeps supplied-model and historical results without identification study valid", () => {
    const fixture = resultFixture();
    const sessions = new StudioAnalysisSessions() as any;
    sessions.model.selectModel(fixture.selectedModel);
    assert.doesNotThrow(() => sessions.restoreIdentification({
      identificationStudy: undefined, engineerStudy: { selectedModel: fixture.selectedModel, campaigns: [] },
    }, loop));
    assert.deepStrictEqual(sessions.model.selectedModel(), fixture.selectedModel);
  });

  test("clears obsolete identification authority when a supplied model replaces it", async () => {
    const fixture = resultFixture();
    const sessions = new StudioAnalysisSessions() as any;
    sessions.identification.restore(fixture.result.identificationStudy);
    const supplied = { ...fixture.selectedModel, source: "provided", validation: "unvalidated", evidence: [] };
    const handled = await sessions.handle({
      schemaVersion: 1, kind: "control-studio.general-model.import", projectId: "project", editorSessionId: "session",
      loopId: "loop", expectedLoopRevision: "sha256:loop", messageId: "message", payload: { json: JSON.stringify(supplied) },
    }, { projectId: "project", editorSessionId: "session", loopId: "loop", loopRevision: "sha256:loop" }, async () => undefined);
    assert.strictEqual(handled, true);
    assert.strictEqual(sessions.identification.snapshot(), undefined, "supplied replacement must retire the obsolete identification study");
  });
});
