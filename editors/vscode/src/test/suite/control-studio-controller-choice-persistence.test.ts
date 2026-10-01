import * as assert from "assert";
import { ThreePageController } from "../../controlStudio/webview/threePageController";

const { JSDOM } = require("jsdom") as { JSDOM: new (html: string) => any };

const loop = {
  controller: { instance: "P1.Controller", type: "Control.PID", law: { kind: "pid" } },
} as any;

function surface() {
  const dom = new JSDOM(`<main><select data-studio-controller><option value="P1.Controller">P1.Controller · Control.PID</option></select><select data-engineer-tuning-method><option value="automatic">Automatic</option><option value="SIMC">SIMC</option></select></main>`);
  const previousDocument = (globalThis as any).document;
  const previousOption = (globalThis as any).Option;
  (globalThis as any).Option = dom.window.Option;
  const previousWindow = (globalThis as any).window;
  (globalThis as any).document = dom.window.document;
  (globalThis as any).window = dom.window;
  const root = dom.window.document.querySelector("main") as HTMLElement;
  return { dom, root, restore() { (globalThis as any).Option = previousOption; (globalThis as any).document = previousDocument; (globalThis as any).window = previousWindow; dom.window.close(); } };
}

suite("Control Studio controller choice persistence", () => {
  test("retains automatic selection mode in the host study after recommending a candidate", async () => {
    const { EngineerWorkflowSession } = require("../../controlStudio/engineerWorkflowSession");
    const { pidLoop, channel } = require("./control-studio-pid-fixtures");
    const target = pidLoop();
    const model = { revision: "model-auto", source: "provided", validation: "validated", evidence: ["bench"], kind: "transfer-function",
      numerator: [1], denominator: [2, 1], delaySeconds: 0, inputs: [channel("u", "V")], outputs: [channel("y", "m")] };
    const engine = new EngineerWorkflowSession(target, () => model, () => {});
    const scope = { projectId: target.projectId, editorSessionId: "automatic-choice", loopId: target.id, loopRevision: target.revision };
    const posts: any[] = [];
    try {
      await engine.handle({ schemaVersion: 1, kind: "control-studio.engineer.tune", messageId: "auto-choice", ...scope, expectedLoopRevision: scope.loopRevision,
        payload: { family: "automatic", applicationMode: "manual", referenceChange: 1, maximumError: .1, settlingBand: .02, settlingTimeSeconds: 12, durationSeconds: 24 } }, scope,
        async (payload: unknown) => { posts.push(payload); });
      assert.strictEqual(posts[posts.length - 1].status, "candidate", String(posts[posts.length - 1].reason));
      assert.strictEqual(engine.snapshotStudy().controllerChoice, "automatic");
    } finally { engine.dispose(); }
  });
  test("reports with the ready model the configuration that automatic tuning chooses", async () => {
    const { EngineerWorkflowSession } = require("../../controlStudio/engineerWorkflowSession");
    const { pidLoop, channel, pidInitialization } = require("./control-studio-pid-fixtures");
    const target = pidLoop();
    target.controller.pidInitialization = pidInitialization();
    const model = { revision: "model-recommended", source: "provided", validation: "validated", evidence: ["bench"], kind: "transfer-function",
      numerator: [1], denominator: [2, 1], delaySeconds: 0, inputs: [channel("u", "V")], outputs: [channel("y", "m")],
      operatingPoint: { inputs: [0], outputs: [0] } };
    const engine = new EngineerWorkflowSession(target, () => model, () => {});
    const scope = { projectId: target.projectId, editorSessionId: "recommended-choice", loopId: target.id, loopRevision: target.revision };
    const posts: any[] = [];
    try {
      const ready = engine.modelChanged();
      assert.strictEqual(ready.status, "model-ready", String(ready.reason));
      assert.ok(["P", "PI", "PD", "PID"].includes(String(ready.recommendedFamily)), `Tune opens with a recommendation: ${ready.recommendedFamily}`);
      await engine.handle({ schemaVersion: 1, kind: "control-studio.engineer.tune", messageId: "recommended-tune", ...scope, expectedLoopRevision: scope.loopRevision,
        payload: { family: "automatic", automaticRequirements: true } }, scope, async (payload: unknown) => { posts.push(payload); });
      const candidate = posts[posts.length - 1];
      assert.strictEqual(candidate.status, "candidate", String(candidate.reason));
      assert.strictEqual(candidate.family, ready.recommendedFamily, "the marked recommendation is the configuration automatic tuning chooses");
      assert.strictEqual(candidate.recommendedFamily, ready.recommendedFamily);
    } finally { engine.dispose(); }
  });
  test("keeps automatic mode while displaying the actual recommended family", () => {
    const view = surface();
    try {
      const controller = new ThreePageController(view.root, loop, () => undefined);
      controller.receive({ availableFamilies: ["P", "PI", "PD", "PID"] });
      controller.receive({ status: "candidate", family: "PI" });
      const select = view.root.querySelector<HTMLSelectElement>("[data-studio-controller]")!;
      assert.strictEqual(controller.choice, "automatic", "a recommendation must not become an explicit family choice");
      assert.strictEqual(select.selectedOptions[0].dataset.family, "PI", "Display the actual recommended configuration");
      assert.match([...select.options].find(option => option.dataset.family === "PI")?.textContent ?? "", /PI.*recommended/);
    } finally { view.restore(); }
  });

  test("keeps editing the automatic candidate's configuration when the host has no recommendation", () => {
    const view = surface();
    try {
      const controller = new ThreePageController(view.root, loop, () => undefined);
      controller.receive({ availableFamilies: ["P", "PI", "PD", "PID"] });
      controller.receive({ status: "candidate", family: "PI" });
      controller.receive({ status: "edited", controllerChoice: "automatic", recommendedFamily: null });
      const select = view.root.querySelector<HTMLSelectElement>("[data-studio-controller]")!;
      assert.strictEqual(controller.selectedFamily, "PI", "manual settings keep the configuration of the candidate they edit");
      assert.strictEqual(select.selectedOptions[0].dataset.family, "PI");
      assert.strictEqual([...select.options].some(option => /recommended/.test(option.textContent ?? "")), false, "no recommendation is claimed without one");
    } finally { view.restore(); }
  });

  test("opens Tune on the host's recommended configuration before any tuning", () => {
    const view = surface();
    try {
      const controller = new ThreePageController(view.root, loop, () => undefined);
      controller.receive({ status: "model-ready", availableFamilies: ["P", "PI", "PD", "PID"], controllerChoice: "automatic", recommendedFamily: "PI" });
      const select = view.root.querySelector<HTMLSelectElement>("[data-studio-controller]")!;
      assert.strictEqual(controller.choice, "automatic", "showing the recommendation is not an explicit choice");
      assert.strictEqual(select.selectedOptions[0].textContent, "P1.Controller · PI — recommended");
      assert.deepStrictEqual([...select.options].filter(option => /recommended/.test(option.textContent ?? "")).map(option => option.dataset.family), ["PI"],
        "only the actual recommendation is marked");
    } finally { view.restore(); }
  });

  test("keeps the recommendation marked beside an explicit choice", () => {
    const view = surface();
    try {
      const controller = new ThreePageController(view.root, loop, () => undefined);
      controller.receive({ status: "model-ready", availableFamilies: ["P", "PI", "PD", "PID"], controllerChoice: "automatic", recommendedFamily: "PI" });
      const select = view.root.querySelector<HTMLSelectElement>("[data-studio-controller]")!;
      const pd = [...select.options].find(option => option.dataset.family === "PD")!;
      select.value = pd.value; select.dispatchEvent(new view.dom.window.Event("change", { bubbles: true }));
      controller.receive({ status: "edited", controllerChoice: "PD", recommendedFamily: "PI" });
      assert.strictEqual(select.selectedOptions[0].textContent, "P1.Controller · PD", "the explicit choice stays selected and unmarked");
      assert.strictEqual([...select.options].find(option => option.dataset.family === "PI")?.textContent, "P1.Controller · PI — recommended");
    } finally { view.restore(); }
  });

  test("preserves explicit PD across recommendation and restored payloads", () => {
    const view = surface();
    try {
      const controller = new ThreePageController(view.root, loop, () => undefined);
      controller.receive({ availableFamilies: ["P", "PI", "PD", "PID"] });
      const select = view.root.querySelector<HTMLSelectElement>("[data-studio-controller]")!;
      const pd = [...select.options].find(option => option.dataset.family === "PD")!;
      select.value = pd.value; select.dispatchEvent(new view.dom.window.Event("change", { bubbles: true }));
      controller.receive({ status: "candidate", family: "PI" });
      assert.strictEqual(controller.choice, "PD");
      assert.strictEqual(select.selectedOptions[0].dataset.family, "PD");
      controller.receive({ controllerChoice: "PD" });
      assert.strictEqual(controller.choice, "PD");
      assert.strictEqual(select.selectedOptions[0].dataset.family, "PD");
      assert.strictEqual([...select.options].some(option => option.dataset.family === "PD" && /recommended/.test(option.textContent ?? "")), false,
        "an explicit family must not be relabelled as recommended without a matching recommendation");
    } finally { view.restore(); }
  });

  test("explains beside Tune why a ready model cannot be tuned", () => {
    const { JSDOM: Dom } = require("jsdom");
    const dom = new Dom(`<main><button data-engineer-find></button><button data-engineer-tune></button><button data-engineer-verify></button><button data-engineer-apply></button>
      <p data-studio-message="model" hidden></p><p data-studio-message="tune" hidden></p><p data-studio-message="verify" hidden></p></main>`);
    try {
      const { ThreePageOperation } = require("../../controlStudio/webview/threePageOperation");
      const root = dom.window.document.querySelector("main");
      const operation = new ThreePageOperation(root, () => undefined, () => undefined, () => undefined);
      const tune = root.querySelector("[data-engineer-tune]"), message = root.querySelector('[data-studio-message="tune"]');
      operation.receive("engineer", { status: "model-ready", compatibleControllerRequired: true,
        reason: "Declare a compatible ST controller adapter before tuning." });
      assert.strictEqual(tune.disabled, true);
      assert.strictEqual(message.hidden, false, "the reason Tune is unavailable is visible beside it");
      assert.strictEqual(message.textContent, "Declare a compatible ST controller adapter before tuning.");
      operation.receive("engineer", { status: "model-ready", compatibleControllerRequired: false, reason: "Model ready." });
      assert.strictEqual(tune.disabled, false);
      assert.strictEqual(message.hidden, true, "an available Tune needs no explanation");
    } finally { dom.window.close(); }
  });

  test("reports a recovered Apply failure without naming PID for other controllers", () => {
    const { recoveryConfirmedReason } = require("../../controlStudio/pidCommissioning");
    const reason = recoveryConfirmedReason();
    assert.match(reason, /Recovery confirmed in Manual/);
    assert.doesNotMatch(reason, /PID/, "the same commissioning drives LQR, LQG, ON/OFF and project controllers");
  });

  test("reports a failed Apply and its recovery as separate sentences", () => {
    const commissioning = require("../../controlStudio/pidCommissioning");
    assert.strictEqual(typeof commissioning.withRecoveryOutcome, "function", "the failure and the recovery must not run into one sentence");
    assert.strictEqual(commissioning.withRecoveryOutcome("first controller output P1.AppliedPowerPct violates reviewed transfer tolerance", commissioning.recoveryConfirmedReason()),
      "first controller output P1.AppliedPowerPct violates reviewed transfer tolerance. Recovery confirmed in Manual; previous controller parameters restored.");
    assert.strictEqual(commissioning.withRecoveryOutcome("The process did not settle.", "Recovery unconfirmed: timeout"), "The process did not settle. Recovery unconfirmed: timeout");
  });
});
