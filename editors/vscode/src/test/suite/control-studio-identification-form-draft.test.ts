import * as assert from "assert";
import { IdentificationWorkflow, type IdentificationSample } from "../../controlStudio/identification";
import { decodeIdentificationState } from "../../controlStudio/identificationState";
import { StudioAnalysisSessions } from "../../controlStudio/studioAnalysisSessions";
import { decodeIdentificationFormDraft, IDENTIFICATION_FORM_KEYS, type IdentificationFormDraft } from "../../controlStudio/identificationFormDraft";

const form: IdentificationFormDraft = {
  schemaVersion: 1, model: "fopdt", heldStep: true,
  values: Object.fromEntries(IDENTIFICATION_FORM_KEYS.map(key => [key, key === "delay.minimum" ? "" : "5"])) as any,
  excludedStart: "", excludedEnd: "",
};
const samples = (change: number): IdentificationSample[] => Array.from({ length: 120 }, (_, index) => ({
  timeSeconds: index / 10, input: index < 20 ? 50 : 50 + change,
  output: index < 20 ? 20 : 20 + change * .5 * (1 - Math.exp(-(index / 10 - 2) / 3)),
  quality: "good", inputUnit: "%", outputUnit: "degC",
}));

function acceptedWorkflow(): IdentificationWorkflow {
  const workflow = new IdentificationWorkflow();
  workflow.qualify(samples(10), [], "Simulation", "first_order", "estimation"); workflow.fit();
  workflow.qualify(samples(12), [], "Simulation", "first_order", "validation"); workflow.fit(); workflow.accept();
  return workflow;
}

suite("Control Studio identification form drafts", () => {
  test("round-trips raw blank fields and rejects malformed form schema", () => {
    assert.deepStrictEqual(decodeIdentificationFormDraft(JSON.parse(JSON.stringify(form))), form);
    assert.strictEqual(decodeIdentificationFormDraft({ ...form, schemaVersion: 2 }), undefined);
    assert.strictEqual(decodeIdentificationFormDraft({ ...form, values: { ...form.values, "delay.step": 0.1 } }), undefined);
  });

  test("retains an initial form-only editing state without claiming an acquisition", () => {
    const workflow = new IdentificationWorkflow();
    workflow.editForm(form);
    const saved: any = workflow.snapshot();
    assert.strictEqual(saved.phase, "editing");
    assert.deepStrictEqual(saved.form, form);
    assert.strictEqual(saved.current, undefined);
    assert.strictEqual(saved.estimation, undefined);
    const reopened = new IdentificationWorkflow(); reopened.restore(saved);
    assert.deepStrictEqual(reopened.snapshot(), saved);
  });

  test("editing an accepted workflow clears authority but preserves original records", () => {
    const workflow = acceptedWorkflow();
    const before: any = workflow.snapshot();
    workflow.editForm(form);
    const after: any = workflow.snapshot();
    assert.strictEqual(workflow.acceptedModel, undefined);
    assert.strictEqual(after.phase, "editing");
    assert.deepStrictEqual(after.form, form);
    assert.strictEqual(after.current.estimationRecordingId, before.current.estimationRecordingId);
    assert.strictEqual(after.estimation, undefined);
    const reopened = new IdentificationWorkflow(); reopened.restore(after);
    assert.deepStrictEqual(reopened.snapshot(), after);
  });

  test("selecting another recording preserves the pending form", () => {
    const workflow = new IdentificationWorkflow();
    workflow.editForm(form);
    workflow.selectRecording(samples(10), [], "Simulation", "fopdt", "recording-1");
    const saved: any = workflow.snapshot();
    assert.strictEqual(saved.phase, "selected");
    assert.deepStrictEqual(saved.form, form);
    assert.strictEqual(saved.current.estimationRecordingId, "recording-1");
    const reopened = new IdentificationWorkflow(); reopened.restore(saved);
    assert.deepStrictEqual(reopened.snapshot(), saved);
  });

  test("editing a preview retains its estimation record and restores it", () => {
    const workflow = new IdentificationWorkflow();
    workflow.qualify(samples(10), [], "Simulation", "first_order", "preview-estimation"); workflow.fit();
    workflow.editForm(form);
    const saved: any = workflow.snapshot();
    assert.strictEqual(saved.phase, "editing");
    assert.strictEqual(saved.estimation.estimationRecordingId, "preview-estimation");
    const reopened = new IdentificationWorkflow(); reopened.restore(saved);
    assert.deepStrictEqual(reopened.snapshot(), saved);
  });

  test("editing a rejected current record preserves that record through restore", () => {
    const workflow = new IdentificationWorkflow();
    workflow.qualify(samples(0), [], "Simulation", "first_order", "rejected-record");
    workflow.editForm(form);
    const saved: any = workflow.snapshot();
    assert.strictEqual(saved.phase, "editing");
    assert.strictEqual(saved.current.estimationRecordingId, "rejected-record");
    const reopened = new IdentificationWorkflow(); reopened.restore(saved);
    assert.deepStrictEqual(reopened.snapshot(), saved);
  });

  test("session edit checks admission before invalidating the selected model", () => {
    const sessions = new StudioAnalysisSessions() as any;
    sessions.identification.restore(undefined);
    sessions.model.selectModel({ revision: "sha256:provided", source: "provided", validation: "unvalidated", evidence: [], delaySeconds: 0,
      kind: "transfer-function", numerator: [1], denominator: [1, 1],
      inputs: [{ id: "u", unit: "%", operatingRange: { minimum: 0, maximum: 100 } }],
      outputs: [{ id: "y", unit: "degC", operatingRange: { minimum: 0, maximum: 100 } }] });
    sessions.editIdentification(form);
    assert.strictEqual(sessions.model.selectedModel(), undefined);
    assert.strictEqual(sessions.identification.snapshot().phase, "editing");
  });

  test("busy edit admission leaves identification and selected model unchanged", () => {
    const sessions = new StudioAnalysisSessions() as any;
    sessions.identification.editForm(form);
    const before = JSON.stringify(sessions.identification.snapshot());
    sessions.engineer = { assertModelReplacementAllowed: () => { throw new Error("operation active"); } };
    assert.throws(() => sessions.editIdentification(form), /operation active/);
    assert.strictEqual(JSON.stringify(sessions.identification.snapshot()), before);
  });
});
