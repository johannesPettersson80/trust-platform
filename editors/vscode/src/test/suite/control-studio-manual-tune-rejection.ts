import * as assert from "assert";
import type { EvaluateWebview } from "./control-studio-navigation-assertions";
import { pollValue } from "./plant-rendered-harness";

export async function exerciseRejectedManualTune(inner: EvaluateWebview,
  click: (selector: string) => Promise<void>, capture: (name: string) => Promise<void>): Promise<void> {
  await click("[data-engineer-options] > summary");
  await inner("const mode=d.querySelector('[data-engineer-gain-mode]');mode.value='manual';mode.dispatchEvent(new Event('change'));d.querySelector('[data-engineer-gain-value]').value='101';return 'manual gain above the declared maximum of 100';");
  await click("[data-engineer-inspector-close=settings]");
  await click("[data-engineer-tune]");
  const result=JSON.parse(await pollValue("rejected manual tuning", () => inner("return JSON.stringify(d.defaultView.__defaultLatest??{});"), value => ["failed","candidate"].includes(JSON.parse(value).status), 15000));
  assert.strictEqual(result.status,"failed","a manual gain above the declared parameter bound must be rejected");
  assert.strictEqual(await inner("return d.querySelector('[data-engineer-stage-tab][aria-selected=true]').dataset.engineerStageTab;"),"tune");
  assert.strictEqual(await inner("return String(d.querySelector('[data-engineer-tune]').disabled);"),"false");
  await capture("manual-tune-rejected");
  await click("[data-engineer-options] > summary");
  await inner("const mode=d.querySelector('[data-engineer-gain-mode]');mode.value='automatic';mode.dispatchEvent(new Event('change'));d.querySelector('[data-engineer-gain-value]').value='1';return 'automatic restored';");
  await click("[data-engineer-inspector-close=settings]");
}
