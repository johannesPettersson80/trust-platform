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

/** The controller the engine offers the loop on open (`controllerOffer`, C2, C4): four families, each labelled by its name. */
function offer(recommended?: string) {
  return { controllerOffer: { families: ["P", "PI", "PD", "PID"].map(family => ({ family, label: family })),
    recommended: recommended ?? null, familySettings: {}, settingRows: [] } };
}

suite("Control Studio controller choice persistence", () => {
  test("keeps automatic mode while displaying the actual recommended family", () => {
    const view = surface();
    try {
      const controller = new ThreePageController(view.root, loop, () => undefined);
      controller.receive(offer("PI"));
      controller.receive({ status: "candidate", family: "PI" });
      const select = view.root.querySelector<HTMLSelectElement>("[data-studio-controller]")!;
      assert.strictEqual(controller.choice, "automatic", "a recommendation must not become an explicit family choice");
      assert.strictEqual(controller.chosenFamily, undefined,
        "the family the selector shows is not a choice: no request sends it, and the engine takes the study's structure (B4.2)");
      assert.strictEqual(select.selectedOptions[0].dataset.family, "PI", "Display the actual recommended configuration");
      assert.match([...select.options].find(option => option.dataset.family === "PI")?.textContent ?? "", /PI.*recommended/);
    } finally { view.restore(); }
  });

  test("keeps editing the automatic candidate's configuration when the host has no recommendation", () => {
    const view = surface();
    try {
      const controller = new ThreePageController(view.root, loop, () => undefined);
      controller.receive(offer());
      controller.receive({ status: "candidate", family: "PI" });
      const select = view.root.querySelector<HTMLSelectElement>("[data-studio-controller]")!;
      assert.strictEqual(controller.selectedFamily, "PI", "manual settings keep the configuration of the candidate they edit");
      assert.strictEqual(select.selectedOptions[0].dataset.family, "PI");
      assert.strictEqual([...select.options].some(option => /recommended/.test(option.textContent ?? "")), false, "no recommendation is claimed without one");
    } finally { view.restore(); }
  });

  test("opens Tune on Automatic before any tuning, and marks only the recommended configuration (B4.2)", () => {
    const view = surface();
    try {
      const controller = new ThreePageController(view.root, loop, () => undefined);
      controller.receive(offer("PI"));
      const select = view.root.querySelector<HTMLSelectElement>("[data-studio-controller]")!;
      assert.strictEqual(controller.choice, "automatic", "showing the recommendation is not an explicit choice");
      assert.strictEqual(select.selectedOptions[0].textContent, "P1.Controller · Choose automatically",
        "a study with no candidate and no pins shows Automatic; the recommendation is only marked (B4.2)");
      assert.deepStrictEqual([...select.options].filter(option => /recommended/.test(option.textContent ?? "")).map(option => option.dataset.family), ["PI"],
        "only the actual recommendation is marked");
    } finally { view.restore(); }
  });

  test("keeps the recommendation marked beside an explicit choice", () => {
    const view = surface();
    try {
      const controller = new ThreePageController(view.root, loop, () => undefined);
      controller.receive(offer("PI"));
      const select = view.root.querySelector<HTMLSelectElement>("[data-studio-controller]")!;
      const pd = [...select.options].find(option => option.dataset.family === "PD")!;
      select.value = pd.value; select.dispatchEvent(new view.dom.window.Event("change", { bubbles: true }));
      controller.receive({ status: "candidate", family: "PI" });
      assert.strictEqual(select.selectedOptions[0].textContent, "P1.Controller · PD", "the explicit choice stays selected and unmarked");
      assert.strictEqual([...select.options].find(option => option.dataset.family === "PI")?.textContent, "P1.Controller · PI — recommended");
    } finally { view.restore(); }
  });

  test("preserves an explicit PD across the engine's recommendation and candidate", () => {
    const view = surface();
    try {
      const controller = new ThreePageController(view.root, loop, () => undefined);
      controller.receive(offer());
      const select = view.root.querySelector<HTMLSelectElement>("[data-studio-controller]")!;
      const pd = [...select.options].find(option => option.dataset.family === "PD")!;
      select.value = pd.value; select.dispatchEvent(new view.dom.window.Event("change", { bubbles: true }));
      controller.receive({ status: "candidate", family: "PI" });
      assert.strictEqual(controller.choice, "PD");
      assert.strictEqual(controller.chosenFamily, "PD", "an explicit choice is the family a request sends");
      assert.strictEqual(select.selectedOptions[0].dataset.family, "PD");
      assert.strictEqual([...select.options].some(option => option.dataset.family === "PD" && /recommended/.test(option.textContent ?? "")), false,
        "an explicit family must not be relabelled as recommended without a matching recommendation");
    } finally { view.restore(); }
  });
});
