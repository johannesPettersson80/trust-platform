import * as assert from "assert";
import { ThreePageTuning } from "../../controlStudio/webview/threePageTuning";
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
  try {
    const root = dom.window.document.querySelector("main") as HTMLElement;
    const posts: any[] = [];
    const tuning = new ThreePageTuning(root, pidLoop(), (kind, payload) => posts.push({ kind, payload }), () => "PI");
    tuning.receive({ status: "candidate", family: "PI", inputRevision: 2,
      values: { "Main.Kp": 1.23456789, "Main.Ki": 0.25, "Main.Kd": 0 },
      settingsPins: { values: { "Main.Ki": 0.25 } } });
    const method = root.querySelector<HTMLSelectElement>("[data-engineer-tuning-method]")!;
    method.value = "manual";
    method.dispatchEvent(new dom.window.Event("change"));
    check(root, tuning, posts, dom.window);
  } finally { (globalThis as any).document = previous; dom.window.close(); }
}

suite("Control Studio shipped Tune interaction", () => {
  test("manual PI settings retain precise values and units, keep automatic pins for the next run, and submit no derivative settings", () => {
    withTuning((root, tuning) => {
      assert.strictEqual(root.querySelectorAll("[data-studio-manual-value]").length, 2);
      assert.strictEqual(root.querySelector<HTMLInputElement>("[data-studio-manual-value='Main.Kp']")!.value, "1.23456789");
      assert.strictEqual(root.querySelector("[data-studio-manual-pin]"), null, "manual settings have no Pin checkboxes");
      assert.deepStrictEqual(tuning.preferences(true), { method: "manual", values: { "Main.Kp": 1.23456789, "Main.Ki": 0.25 } });
      assert.deepStrictEqual(tuning.pinnedValues(), { "Main.Ki": 0.25 });
      assert.match(root.textContent ?? "", /V\/m/);
    });
  });

  test("blank Manual input stays invalid across a late edit acknowledgement instead of becoming zero", () => {
    withTuning((root, tuning, posts, window) => {
      const input = root.querySelector<HTMLInputElement>("[data-studio-manual-value='Main.Kp']")!;
      input.value = "";
      input.dispatchEvent(new window.Event("input", { bubbles: true }));
      assert.throws(() => tuning.preferences(true), /proportional gain.*between/i);
      assert.strictEqual(input.getAttribute("aria-invalid"), "true");
      const sent = posts[posts.length - 1];
      assert.strictEqual(sent.kind, "control-studio.engineer.invalidate");
      assert.ok(!("Main.Kp" in sent.payload.tuningPreferences.values));
      tuning.receive({ status: "edited", inputRevision: 3, tuningPreferences: sent.payload.tuningPreferences });
      assert.strictEqual(root.querySelector("[data-studio-manual-value='Main.Kp']"), input);
      assert.strictEqual(input.value, "");
      assert.strictEqual(input.getAttribute("aria-invalid"), "true");
    });
  });

  test("an older restored preference cannot replace the engineer's newer method and gain edit", () => {
    withTuning((root, tuning, _posts, window) => {
      const input = root.querySelector<HTMLInputElement>("[data-studio-manual-value='Main.Kp']")!;
      input.value = "3.5";
      input.dispatchEvent(new window.Event("input", { bubbles: true }));
      tuning.receive({ status: "restored", inputRevision: 1,
        tuningPreferences: { method: "automatic", values: { "Main.Kp": 1 } } });
      assert.strictEqual(root.querySelector<HTMLSelectElement>("[data-engineer-tuning-method]")!.value, "manual");
      assert.strictEqual(input.value, "3.5");
      assert.strictEqual(tuning.preferences(true).values?.["Main.Kp"], 3.5);
    });
  });
});
