import * as assert from "assert";
import { PARAMETER_CONTROLS } from "../../controlStudio/parameterPresentation";
import { initializeParameterWorkflow } from "../../controlStudio/webview/parameterWorkflow";

const { JSDOM } = require("jsdom") as { JSDOM: new (html: string) => any };

const projectId = "project:control-lesson-28";
const editorSessionId = "editor-parameter-fields";
const loopId = "control-lesson-28";
const loopRevision = "sha256:lesson-11";
const proportional = "P1.Parameters.ProportionalGain";
const integral = "P1.Parameters.IntegralGain";

const binding = {
  loop_id: loopId,
  task_name: "MainTask",
  parameters: [
    { symbol: proportional, iec_type: "LREAL", unit: "percentage-point/degC", convention: "parallel proportional gain", minimum: 1, maximum: 4 },
    { symbol: integral, iec_type: "LREAL", unit: "percentage-point/(degC*s)", convention: "parallel integral gain", minimum: 0.2, maximum: 1 },
  ],
  mode: { symbol: "P1.ExperimentEnabled", manual_value: false },
};

function scopedMessage(window: any, payload: Record<string, unknown>): any {
  return new window.MessageEvent("message", { data: {
    schemaVersion: 1,
    kind: "control-studio.engineering.result",
    stage: "parameters",
    projectId,
    editorSessionId,
    loopId,
    expectedLoopRevision: loopRevision,
    payload,
  } });
}

function observed(values: Record<string, number>, activeRevision = "rev-current"): Record<string, unknown> {
  return {
    program_revision: "program-current",
    binding_revision: "binding-current",
    active_revision: activeRevision,
    values,
    status: "idle",
    previous_revision: null,
    previous_values: null,
    reason: null,
  };
}

suite("Control Studio parameter field presentation", () => {
  test("edits one labelled gain, preserves the other, invalidates review, and reflects observed rollback", () => {
    const dom = new JSDOM(`<main data-control-studio data-project-id="${projectId}" data-editor-session-id="${editorSessionId}" data-loop-id="${loopId}" data-loop-revision="${loopRevision}">${PARAMETER_CONTROLS}</main>`);
    const previousDocument = (globalThis as any).document;
    const previousWindow = (globalThis as any).window;
    (globalThis as any).document = dom.window.document;
    (globalThis as any).window = dom.window;
    const posts: Array<{ kind: string; payload?: Record<string, unknown> }> = [];
    try {
      initializeParameterWorkflow((kind, payload) => posts.push({ kind, payload }));
      const root = (dom.window.document as Document).querySelector<HTMLElement>("[data-control-studio]")!;
      const dispatch = (payload: Record<string, unknown>) => dom.window.dispatchEvent(scopedMessage(dom.window, payload));
      const initialValues = { [proportional]: 2, [integral]: 0.5 };

      dispatch({ status: "inspected", source: "simulation", values: initialValues, observed: observed(initialValues), binding });

      const fields = [...root.querySelectorAll<HTMLInputElement>("input[data-parameter-symbol]")];
      assert.strictEqual(fields.length, 2, "the two actual LREAL record leaves must be editable as numeric fields");
      assert.deepStrictEqual(fields.map(field => field.dataset.parameterSymbol), [proportional, integral]);
      assert.deepStrictEqual(fields.map(field => [field.type, field.min, field.max]), [
        ["number", "1", "4"], ["number", "0.2", "1"],
      ]);
      for (const field of fields) {
        assert.ok(field.id, "each parameter field needs a label target");
        assert.ok(root.querySelector(`label[for="${field.id}"]`), `missing label for ${field.dataset.parameterSymbol}`);
      }

      const proportionalField = root.querySelector<HTMLInputElement>(`input[data-parameter-symbol="${proportional}"]`)!;
      proportionalField.value = "2.4";
      proportionalField.dispatchEvent(new dom.window.Event("input", { bubbles: true }));
      const values = JSON.parse(root.querySelector<HTMLTextAreaElement>("[data-parameters-values]")!.value);
      assert.strictEqual(values[proportional], 2.4);
      assert.strictEqual(values[integral], 0.5, "editing Kp must preserve Ki");
      assert.strictEqual(root.querySelector<HTMLButtonElement>("[data-parameters-apply]")!.disabled, true);
      assert.strictEqual(root.querySelector<HTMLElement>("[data-parameters-review-area]")!.hidden, true);

      dispatch({
        status: "reviewed", source: "simulation", reviewId: "review-24",
        values: { [proportional]: 2.4, [integral]: 0.5 }, observed: observed(initialValues), binding,
      });
      assert.strictEqual(root.querySelector<HTMLElement>("[data-parameters-review-area]")!.hidden, false);
      assert.strictEqual(root.querySelector<HTMLButtonElement>("[data-parameters-apply]")!.disabled, false);

      assert.strictEqual(root.querySelector<HTMLElement>("[data-parameters-editor]")?.hidden, true,
        "the review must not duplicate the editor above its confirmation action");
      const change = root.querySelector<HTMLButtonElement>("[data-parameters-change]");
      assert.ok(change, "review must offer an explicit return to editing");
      change.click();
      assert.strictEqual(root.querySelector<HTMLElement>("[data-parameters-review-area]")!.hidden, true);
      assert.strictEqual(root.querySelector<HTMLElement>("[data-parameters-editor]")!.hidden, false);

      proportionalField.value = "2";
      proportionalField.dispatchEvent(new dom.window.Event("input", { bubbles: true }));
      const resetValues = JSON.parse(root.querySelector<HTMLTextAreaElement>("[data-parameters-values]")!.value);
      assert.deepStrictEqual(resetValues, initialValues, "editing after review must remove stale reviewed 2.4 while retaining Ki 0.5");
      assert.strictEqual(root.querySelector<HTMLButtonElement>("[data-parameters-apply]")!.disabled, true);
      assert.strictEqual(root.querySelector<HTMLElement>("[data-parameters-review-area]")!.hidden, true);

      dispatch({
        status: "reviewed", source: "simulation", reviewId: "review-rollback",
        rollback: true, values: initialValues, observed: {
          ...observed({ [proportional]: 2.4, [integral]: 0.5 }, "rev-applied"),
          previous_revision: "rev-baseline", previous_values: initialValues,
        }, binding,
      });
      assert.strictEqual(root.querySelector<HTMLButtonElement>("[data-parameters-apply]")!.disabled, false);
      root.querySelector<HTMLButtonElement>("[data-parameters-apply]")!.click();
      assert.ok(posts.some(post => post.kind === "control-studio.parameters.apply"), "applied review must use the existing parameter owner");

      dispatch({ status: "applied", source: "simulation", values: initialValues, observed: observed(initialValues, "rev-baseline"), binding });
      const appliedValues = JSON.parse(root.querySelector<HTMLTextAreaElement>("[data-parameters-values]")!.value);
      assert.deepStrictEqual(appliedValues, initialValues, "observed rollback must replace stale candidate JSON");
      assert.strictEqual(proportionalField.value, "2");
      assert.strictEqual(root.querySelector<HTMLInputElement>(`input[data-parameter-symbol="${integral}"]`)!.value, "0.5");
      assert.ok(!root.textContent?.includes("2.4"), "no stale candidate value may remain after observed rollback");

      dispatch({ status: "failed", reason: `${proportional} is outside its declared bounds after IEC rounding.` });
      assert.strictEqual(root.querySelector<HTMLElement>("[data-parameters-status]")!.textContent,
        "Proportional gain must be between 1 and 4.");

      const raw = root.querySelector<HTMLElement>("[data-parameters-values]")!;
      const identity = root.querySelector<HTMLElement>("[data-parameters-identity]")!;
      assert.ok(raw.closest("details") && !raw.closest("details")!.open, "raw JSON belongs in a closed technical-details disclosure");
      assert.ok(identity.closest("details") && !identity.closest("details")!.open, "control identity belongs in a closed technical-details disclosure");
    } finally {
      (globalThis as any).document = previousDocument;
      (globalThis as any).window = previousWindow;
      dom.window.close();
    }
  });
});
