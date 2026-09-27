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
});
