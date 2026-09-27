import * as assert from "assert";
import * as fs from "fs";
import * as path from "path";
import type { HotHeaterInner } from "./control-studio-hot-heater-helpers";
import { clickHotHeaterControl } from "./control-studio-hot-heater-helpers";
import { pollValue } from "./plant-rendered-harness";

/** Continues the real imported-recording journey with the lesson's own native Test. */
export async function validateImportedStepWithNativeTest(inner: HotHeaterInner,
  capture: (name: string) => Promise<void>, captures: string): Promise<void> {
  const click = (selector: string) => clickHotHeaterControl(inner, selector);
  const preview = JSON.parse(await inner("return d.querySelector('[data-control-studio]').dataset.stepReview;"));
  assert.strictEqual(preview.phase, "validation_required");
  await inner("var root=d.querySelector('[data-control-studio]');d.defaultView.addEventListener('message',e=>{var m=e.data;if(m?.kind==='control-studio.experiment.result'&&m.phase==='terminal')root.dataset.validationTerminal=JSON.stringify(m.payload);});return 'observe native validation';");
  await click("[data-test-review]");
  await pollValue("validation Test reviewed", () => inner("return d.querySelector('[data-test-state]').textContent;"), value => /Simulation test ready/.test(value), 10000);
  await capture("08-validation-reviewed");
  assert.match(await inner("return d.querySelector('[data-test-timeout]').textContent;"), /5 s baseline/,
    "the native validation recipe must acquire the complete measured five-second baseline before activation");
  await click("[data-test-approve]");
  await capture("09-validation-baseline");
  const terminal = JSON.parse(await pollValue("native independent validation completed", () => inner("return d.querySelector('[data-control-studio]').dataset.validationTerminal??'';"), value => Boolean(value), 60000));
  fs.writeFileSync(path.join(captures, "validation-terminal.json"), JSON.stringify(terminal, null, 2));
  assert.strictEqual(terminal.status, "completed", JSON.stringify(terminal));
  await capture("10-validation-completed");
  await inner("[...d.querySelectorAll('[data-control-studio-mode]')].find(b=>b.getAttribute('aria-label')?.startsWith('Identify,')).click();return 'Identify';");
  await click("[data-identify-run]");
  const qualification = await pollValue("native validation qualification", () => inner("return d.querySelector('[data-identify-state]').textContent;"), value => /qualified|failed|Incomplete|Review/i.test(value), 10000);
  assert.match(qualification, /Validation recording qualified/, "the fresh native recording must qualify as independent validation after an imported estimation");
  await capture("11-validation-qualified");
  assert.strictEqual(await inner("return d.querySelector('[data-plot-tab=recorded-response]')?.getAttribute('aria-selected');"), "true",
    "checking a new native recording must show that recording rather than the previous estimation fit");
  await inner("delete d.querySelector('[data-control-studio]').dataset.stepReview;return 'await frozen validation';");
  await click("[data-identify-fit]");
  const validation = JSON.parse(await pollValue("frozen-model validation result", () => inner("return d.querySelector('[data-control-studio]').dataset.stepReview??'';"), value => Boolean(value), 10000));
  fs.writeFileSync(path.join(captures, "independent-step-validation.json"), JSON.stringify({ preview, validation }, null, 2));
  assert.strictEqual(validation.phase, "acceptance_required", JSON.stringify(validation));
  assert.deepStrictEqual(validation.fit.stepResponse.best, preview.fit.stepResponse.best,
    "native validation must preserve the imported estimation's fitted parameters");
  assert.strictEqual(validation.validationQualification.stepResponse.inputChange, 10);
  assert.strictEqual(preview.fit.stepResponse.inputChange, 12);
  assert.strictEqual(validation.validationQualification.stepResponse.baseline.count, 50);
  assert.strictEqual(validation.presentation.stepResponse.inputChange, 10);
  assert.strictEqual(await inner("return [...d.querySelectorAll('[data-step-response-report] tr')].find(r=>r.querySelector('th').textContent==='Applied change').querySelector('td').textContent;"), "10 %",
    "the validation measurement table must match the plotted native run");
  assert.ok(validation.fit.points.every((point: { residual: number }) => Math.abs(point.residual) <= .05 + 1e-9));
  await capture("12-validation-fit");
  await click("[data-identify-accept]");
  await pollValue("explicit model acceptance", () => inner("return d.querySelector('[data-identify-state]').textContent;"), value => /accepted/i.test(value), 10000);
  await capture("13-model-accepted");
}
