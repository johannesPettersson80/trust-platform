import * as assert from "assert";

import { createPlantDraft } from "../../networkCanvas/plant/addFlow";
import {
  numberText,
  receivePlantEditResult,
  requestPlantEdit,
  typedNumber,
} from "../../networkCanvas/plant/editorSession";
import { unavailablePlantSignals } from "../../networkCanvas/plant/signalCatalog";

// Specification 47 §20.1: the webview sends its draft to Rust through the extension host and
// renders the answer; these tests prove the message wiring only.
suite("Plant editor session wiring", () => {
  test("a request posts the draft and resolves with the answer of the same request", async () => {
    const posted: Array<Record<string, unknown>> = [];
    const draft = createPlantDraft({ id: "runtime:local", name: "Simulator" });
    const signals = unavailablePlantSignals("Compiled PLC signals are not available yet.");
    const pending = requestPlantEdit((message) => posted.push(message as Record<string, unknown>), {
      draft,
      action: { kind: "open" },
      signals,
    });
    assert.strictEqual(posted.length, 1);
    const request = posted[0];
    assert.strictEqual(request.type, "plantEdit");
    assert.strictEqual(request.draft, draft);
    assert.deepStrictEqual(request.action, { kind: "open" });
    assert.strictEqual(request.signals, signals);
    assert.strictEqual(typeof request.requestId, "number");

    assert.strictEqual(receivePlantEditResult({ type: "meta" }), false, "other messages pass through");
    assert.strictEqual(
      receivePlantEditResult({ type: "plantEditResult", requestId: -5, ok: true, answer: {} }),
      true,
      "an answer for no pending request is consumed"
    );
    const answer = { draft: { plantId: "new_plant" }, issues: [], can_save: false, view: {}, candidate: {} };
    assert.strictEqual(
      receivePlantEditResult({ type: "plantEditResult", requestId: request.requestId, ok: true, answer }),
      true
    );
    assert.deepStrictEqual(await pending, { ok: true, answer });
  });

  test("a refused request resolves with the host's reason", async () => {
    const posted: Array<Record<string, unknown>> = [];
    const pending = requestPlantEdit((message) => posted.push(message as Record<string, unknown>), {
      draft: createPlantDraft({ id: "runtime:local", name: "Simulator" }),
      signals: unavailablePlantSignals("No project."),
    });
    assert.strictEqual("action" in posted[0], false, "a plain check sends no action");
    receivePlantEditResult({
      type: "plantEditResult",
      requestId: posted[0].requestId,
      ok: false,
      message: "Open the project folder to edit its simulated plants.",
    });
    assert.deepStrictEqual(await pending, {
      ok: false,
      message: "Open the project folder to edit its simulated plants.",
    });
  });

  test("number fields show finite numbers and send what the text spells", () => {
    assert.strictEqual(numberText(2.5), "2.5");
    assert.strictEqual(numberText(Number.NaN), "");
    assert.strictEqual(numberText(null), "");
    assert.strictEqual(numberText(undefined), "");
    assert.strictEqual(typedNumber(" 3 "), 3);
    assert.ok(Number.isNaN(typedNumber("")), "empty text is no value");
    assert.ok(Number.isNaN(typedNumber("abc")));
  });
});
