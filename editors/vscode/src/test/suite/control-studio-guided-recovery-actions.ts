import * as assert from "assert";
import type { EvaluateWebview } from "./control-studio-navigation-assertions";
import { pollValue } from "./plant-rendered-harness";

type Click = (selector: string) => Promise<void>;
type Capture = (name: string) => Promise<void>;

export async function exerciseAcquisitionReview(inner: EvaluateWebview, click: Click, capture: Capture): Promise<void> {
  await click("[data-engineer-options] > summary");
  await inner("const m=d.querySelector('[data-engineer-acquisition-method]');m.value='relay';m.dispatchEvent(new Event('change'));d.querySelector('[data-engineer-relay-direction]').value='';return 'missing relay direction';");
  await click("[data-engineer-inspector-close=settings]");
  await click("[data-engineer-find]");
  await pollValue("identification control recovery", () => inner("return d.defaultView.__defaultLatest?.failure?.nextAction?.kind??'';"), value => value === "open-limits", 10000);
  await capture("acquisition-review-needed");
  await click("[data-engineer-configure-acquisition]");
  assert.strictEqual(await inner("return String(d.querySelector('[data-engineer-options]').open);"), "true");
  await capture("acquisition-review-open");
  await inner("const m=d.querySelector('[data-engineer-acquisition-method]');m.value='configured';m.dispatchEvent(new Event('change'));return 'project setup';");
  await click("[data-engineer-inspector-close=settings]");
}

export async function exerciseRequiredScenarioReview(inner: EvaluateWebview, click: Click, capture: Capture): Promise<void> {
  await click("[data-engineer-configure-scenarios]");
  assert.strictEqual(await inner("return String(d.querySelector('[data-engineer-options]').open&&d.querySelector('[data-engineer-scenario-controls]').open);"), "true");
  await click("[data-engineer-scenario-add]");
  await inner("const n=d.querySelector('[data-engineer-scenario-id]');n.value='extra-check';n.dispatchEvent(new Event('input',{bubbles:true}));return 'required scenario';");
  await capture("required-scenario-added");
  await click("[data-engineer-inspector-close=settings]");
  await click("[data-engineer-stage-tab=tune]");
}
