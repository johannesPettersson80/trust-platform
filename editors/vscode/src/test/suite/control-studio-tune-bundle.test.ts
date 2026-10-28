import * as assert from "assert";
import { ThreePageTuning } from "../../controlStudio/webview/threePageTuning";
import { setOpenedOffers } from "../../controlStudio/webview/openedOffers";
import { pidLoop } from "./control-studio-pid-fixtures";

const { JSDOM } = require("jsdom");

function withTuning(check: (root: HTMLElement, tuning: ThreePageTuning, posts: any[], window: any) => void): void {
  const dom = new JSDOM(`<main>
    <select data-studio-controller><option value="PI">PI</option></select>
    <select data-engineer-tuning-method><option value="automatic">Automatic</option><option value="manual">Manual</option></select>
    <div data-studio-tuning-result></div>
  </main>`);
  const previous = (globalThis as any).document;
  (globalThis as any).document = dom.window.document;
  // The engine's rows for the loop's settings, each with its label, unit and bounds (C2).
  setOpenedOffers({ controllerOffer: { families: [], familySettings: {}, settingRows: [
    { symbol: "Main.Kp", label: "Proportional gain (Kp)", unit: "V/m", minimum: 0, maximum: 100 },
    { symbol: "Main.Ki", label: "Integral gain (Ki)", unit: "V/(m*s)", minimum: 0, maximum: 100 },
    { symbol: "Main.Kd", label: "Derivative gain (Kd)", unit: "V*s/m", minimum: 0, maximum: 100 }] } } as any);
  try {
    const root = dom.window.document.querySelector("main") as HTMLElement;
    const posts: any[] = [];
    // The PI family takes the proportional and integral gains; the derivative gain is not among them (C2).
    const tuning = new ThreePageTuning(root, pidLoop(), (kind, payload) => posts.push({ kind, payload }), () => "PI", undefined,
      family => family === "PI" ? ["Main.Kp", "Main.Ki"] : undefined);
    tuning.receive({ status: "candidate", family: "PI", values: { "Main.Kp": 1.23456789, "Main.Ki": 0.25, "Main.Kd": 0 } });
    // The settings the study kept pinned (A8.6): the engine's recorded inputs, which the opened page shows again.
    tuning.restoreInputs({ method: "automatic", pins: { "Main.Ki": 0.25 } });
    const method = root.querySelector<HTMLSelectElement>("[data-engineer-tuning-method]")!;
    method.value = "manual";
    method.dispatchEvent(new dom.window.Event("change"));
    check(root, tuning, posts, dom.window);
  } finally { setOpenedOffers({}); (globalThis as any).document = previous; dom.window.close(); }
}

suite("Control Studio shipped Tune interaction", () => {
  test("manual PI settings retain precise values and units, keep automatic pins for the next run, and submit no derivative settings", () => {
    withTuning((root, tuning) => {
      assert.strictEqual(root.querySelectorAll("[data-studio-manual-value]").length, 2);
      assert.strictEqual(root.querySelector<HTMLInputElement>("[data-studio-manual-value='Main.Kp']")!.value, "1.23456789");
      assert.strictEqual(root.querySelector("[data-studio-manual-pin]"), null, "manual settings have no Pin checkboxes");
      assert.deepStrictEqual(tuning.preferences(true), { method: "manual", values: { "Main.Kp": "1.23456789", "Main.Ki": "0.25" } }, "the entries are sent as typed");
      assert.deepStrictEqual(tuning.pinnedValues(), { "Main.Ki": 0.25 });
      assert.match(root.textContent ?? "", /V\/m/);
    });
  });

  test("blank Manual input stays blank instead of becoming zero, and the engine's refusal marks its row", () => {
    withTuning((root, tuning, posts, window) => {
      const input = root.querySelector<HTMLInputElement>("[data-studio-manual-value='Main.Kp']")!;
      input.value = "";
      input.dispatchEvent(new window.Event("input", { bubbles: true }));
      assert.strictEqual(tuning.preferences(true).values?.["Main.Kp"], "", "a blank entry is sent as typed: the engine refuses it under its row (B4.5)");
      const sent = posts[posts.length - 1];
      assert.strictEqual(sent.kind, "control-studio.engineer.invalidate");
      assert.strictEqual(sent.payload.tuningPreferences.values["Main.Kp"], "", "the blank entry is kept, neither dropped nor zero");
      assert.ok(!("family" in sent.payload),
        "the family the selector shows (PI) is not a choice and is not sent: the engine takes the study's structure, or the candidate's family for the manual method (B4.2)");
      assert.strictEqual(root.querySelector("[data-studio-manual-value='Main.Kp']"), input);
      assert.strictEqual(input.value, "");
      assert.strictEqual(input.hasAttribute("aria-invalid"), false, "the page marks no row before the engine refuses the entry");
      tuning.receive({ status: "failed", flowStage: "Tune",
        failure: { field: "Main.Kp", message: "Enter a proportional gain between 0 and 100." } });
      assert.strictEqual(input.getAttribute("aria-invalid"), "true", "the engine's refusal marks the row it names");
    });
  });

  test("the settings go with the manual method alone: another method sends none, and the engine would refuse them (B4.2)", () => {
    withTuning((root, tuning) => {
      assert.deepStrictEqual(Object.keys(tuning.preferences(true)).sort(), ["method", "values"], "the manual method sends its entries");
      const method = root.querySelector<HTMLSelectElement>("[data-engineer-tuning-method]")!;
      method.value = "automatic";
      assert.deepStrictEqual(tuning.preferences(), { method: "automatic" }, "Automatic sends the method and no settings");
    });
  });
});
