/// <reference lib="dom" />
import * as comparison from "../../controlStudio/webview/controllerComparisonPresentation";
import * as assert from "assert";
import { engineerWorkflowControls } from "../../controlStudio/engineerWorkflowPresentation";
const { JSDOM } = require("jsdom");

suite("Control Studio shared workflow layout", () => {
  test("preserves supported controller choices and rejects unknown saved choices", () => {
    const { decodeEngineerStudy } = require("../../controlStudio/engineerStudyPersistence");
    for (const controllerChoice of ["automatic", "P", "PI", "PD", "PID"]) {
      assert.strictEqual(decodeEngineerStudy({ campaigns: [], controllerChoice })?.controllerChoice, controllerChoice);
    }
    assert.strictEqual(decodeEngineerStudy({ campaigns: [], controllerChoice: "unknown" }), undefined);
    const legacy = decodeEngineerStudy({ campaigns: [], family: "PI" });
    const { restoredEngineerView } = require("../../controlStudio/restoredEngineerView");
    assert.strictEqual(restoredEngineerView(legacy).controllerChoice, "PI");
  });

  test("restores controller comparisons as historical evidence without candidate authority", () => {
    let api: any = {};
    try { api = require("../../controlStudio/restoredEngineerView"); } catch { /* expected missing behavior */ }
    assert.strictEqual(typeof api.restoredEngineerView, "function");
    const study = { family: "PI", task: { referenceChange: 1 }, selectedModel: { revision: "internal-model" },
      familyEvaluations: [{ family: "P", status: "unavailable", reason: "Gain limit exceeded." },
        { family: "PI", status: "actual-pass", reason: "Target response passed." }] };
    const view = api.restoredEngineerView(study);
    assert.strictEqual(view.status, "restored");
    assert.strictEqual(view.family, "PI");
    assert.deepStrictEqual(view.familyEvaluations, study.familyEvaluations);
    assert.strictEqual(view.stageEvidence.hasCandidate, false);
    assert.strictEqual(view.stageEvidence.validationHasResult, false);
    assert.strictEqual(view.stageEvidence.validationPassed, false);
    assert.match(view.reason, /historical.*Tune.*Verify/i);
    assert.ok(!view.reason.includes("internal-model"));
    const presentation = comparison;
    assert.strictEqual(typeof presentation.renderHistoricalControllerComparison, "function");
    const doc = new JSDOM("<div id='history'></div>").window.document;
    const history = doc.querySelector("#history");
    presentation.renderHistoricalControllerComparison(history, view.familyEvaluations);
    assert.match(history.textContent, /Historical/);
    assert.match(history.textContent, /PI.*Passed/);
    assert.match(history.textContent, /Gain limit exceeded/);
    assert.strictEqual(history.querySelectorAll("button").length, 0);
  });

  test("renders rejected controller reasons as inspectable detail", () => {
    let api: any = {};
    try { api = comparison; } catch { /* expected missing behavior */ }
    assert.strictEqual(typeof api.renderControllerComparison, "function");
    const doc = new JSDOM("<div id='detail'></div>").window.document;
    const detail = doc.querySelector("#detail");
    api.renderControllerComparison(detail, [
      { structure: "P", status: "Rejected", reason: "Required gain exceeds the actuator limit." },
      { structure: "PI", status: "Rejected", reason: "Integral gain cannot be represented by this controller." },
    ]);
    assert.match(detail.textContent, /P.*Required gain exceeds/);
    assert.match(detail.textContent, /PI.*Integral gain cannot/);
    assert.strictEqual(detail.querySelectorAll("button").length, 0, "rejection evidence cannot expose Apply");
  });

  test("describes the selected model without exposing internal identifiers", () => {
    const api = require("../../controlStudio/engineerWorkflowPresentation");
    assert.strictEqual(typeof api.modelReadyReason, "function");
    assert.strictEqual(api.modelReadyReason({ source: "provided", revision: "sha256:internal" }, true),
      "Model ready. Recommend P control first; PI, PD and PID are available if the task needs them.");
    assert.strictEqual(api.modelReadyReason({ source: "fitted", revision: "sha256:internal" }, false),
      "Model found and validated. Tune the P controller for your requirements.");
  });

  test("shows one stopped-state explanation and preserves specific connection faults", () => {
    const api = require("../../controlStudio/engineerWorkflowPresentation");
    assert.strictEqual(typeof api.unavailableLoopPresentation, "function");
    assert.deepStrictEqual(api.unavailableLoopPresentation("Simulation", "Runtime is not producing scans"), {
      status: "Simulator stopped", explanation: "Start the simulator to observe the response.",
    });
    assert.deepStrictEqual(api.unavailableLoopPresentation("Physical", "Connection lost"), {
      status: "Process unavailable", explanation: "Connection lost",
    });
  });

  test("uses shared buttons and keeps secondary workflow operations collapsed", () => {
    const document = new JSDOM(engineerWorkflowControls("m", true, "V", true)).window.document;
    const buttons = Array.from(document.querySelectorAll("button")) as HTMLButtonElement[];
    assert.ok(buttons.every(button => button.classList.contains("trust-button")), "workflow actions must use shared truST buttons");
    assert.strictEqual(document.querySelectorAll(".trust-button--primary").length, 1);
    assert.ok(document.querySelector("[data-engineer-find]").classList.contains("trust-button--primary"));
    const more = document.querySelector("[data-engineer-options]");
    assert.ok(more && !more.open, "secondary operations must be collapsed by default");
    assert.ok(more.contains(document.querySelector("[data-engineer-rollback]")));
    assert.strictEqual(document.querySelector("[data-engineer-status]").textContent, "");
    assert.ok(document.querySelector("[data-engineer-cancel]").hidden);
  });

  test("explains the first workflow action and names scenario controls by their purpose", () => {
    const document = new JSDOM(engineerWorkflowControls("m", true, "V", true)).window.document;
    assert.strictEqual(document.querySelector("[data-engineer-status]").textContent,
      "",
      "the start state uses its action labels without duplicate introductory prose");
    assert.strictEqual(document.querySelector("[data-engineer-scenario-controls] > summary").textContent,
      "Verification scenarios",
      "manual controls must explain that scenarios are mandatory verification evidence");
  });
});
