import * as assert from "assert";
import * as navigation from "../../controlStudio/webview/engineerNavigation";
import { engineerWorkflowControls } from "../../controlStudio/engineerWorkflowPresentation";
const { JSDOM } = require("jsdom");

suite("Control Studio Tune setting applicability", () => {
  test("hides PID-only Tune controls for P and restores them for PI", () => {
    const dom = new JSDOM(`<div data-control-studio data-project-id="p" data-editor-session-id="e" data-loop-id="l" data-loop-revision="r">
      ${engineerWorkflowControls("°C", true, "%", true)}
    </div>`);
    const previous = new Map<string, PropertyDescriptor | undefined>();
    for (const key of ["window", "HTMLElement", "CustomEvent"]) previous.set(key, Object.getOwnPropertyDescriptor(globalThis, key));
    Object.assign(globalThis, { window: dom.window, HTMLElement: dom.window.HTMLElement, CustomEvent: dom.window.CustomEvent });
    try {
      const section = dom.window.document.querySelector("[data-engineer-workflow]") as HTMLElement;
      section.dataset.pidAvailable = "true";
      section.querySelector<HTMLSelectElement>("[data-engineer-tuning-method]")!.value = "SIMC";
      const update = navigation.initializeEngineerNavigation(section, () => {});
      const family = section.querySelector<HTMLSelectElement>("[data-engineer-family]")!;
      family.value = "P";
      update({ status: "model-ready", model: { revision: "model-1" }, stageEvidence: { hasModel: true } });
      section.querySelector<HTMLButtonElement>("[data-engineer-stage-tab=tune]")!.click();
      const objective = section.querySelector("[data-engineer-tuning-objective]")!;
      const method = section.querySelector("[data-engineer-tuning-method]")!;
      assert.strictEqual(hasHiddenAncestor(objective, section), true);
      assert.strictEqual(hasHiddenAncestor(method, section), true);
      assert.strictEqual(hasHiddenAncestor(section.querySelector("[data-engineer-gain-mode]")!, section), false);
      assert.strictEqual(hasHiddenAncestor(section.querySelector("[data-engineer-gain-value]")!, section), true);
      const gainMode = section.querySelector<HTMLSelectElement>("[data-engineer-gain-mode]")!;
      gainMode.value = "manual";
      gainMode.dispatchEvent(new dom.window.Event("change", { bubbles: true }));
      assert.strictEqual(hasHiddenAncestor(section.querySelector("[data-engineer-gain-value]")!, section), false,
        "a specific P gain remains available when its input is relevant");
      assert.strictEqual(hasHiddenAncestor(section.querySelector("[data-engineer-setting-pins]")!, section), false);

      family.value = "PI";
      section.querySelector<HTMLButtonElement>("[data-engineer-stage-tab=tune]")!.click();
      assert.strictEqual(hasHiddenAncestor(objective, section), false);
      assert.strictEqual(hasHiddenAncestor(method, section), false);
    } finally {
      for (const [key, descriptor] of previous) {
        if (descriptor) Object.defineProperty(globalThis, key, descriptor);
        else delete (globalThis as any)[key];
      }
    }
  });
});

function hasHiddenAncestor(node: Element, boundary: Element): boolean {
  let current: Element | null = node;
  while (current && current !== boundary) {
    if (current instanceof HTMLElement && current.hidden) return true;
    current = current.parentElement;
  }
  return false;
}
