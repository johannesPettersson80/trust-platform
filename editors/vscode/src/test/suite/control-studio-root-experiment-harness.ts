import * as assert from "assert";
import { runtimeLifecycleService } from "../../runtimeLifecycle";
import { pollValue } from "./plant-rendered-harness";
import type { ControlStudioWorkflowResult } from "../../controlStudio/workflowResult";

/** Drives only visible Studio actions; all retained samples come from the actual runtime. */
export async function runSelectedShaftExperiment(inner: (body: string) => Promise<string>): Promise<void> {
  const stage = async (name: string) => inner(`Array.from(d.querySelectorAll('[data-control-studio-mode]')).find(b=>b.textContent.trim()===${JSON.stringify(name)}).click();return 'selected';`);
  await stage("Test");
  await pollValue("fresh shaft signals", async () => {
    await runtimeLifecycleService.requestIoState();
    return inner("return d.querySelector('[data-control-studio-quality]').textContent;");
  }, value => !/Unavailable|Waiting/i.test(value), 15000);
  await inner("d.querySelector('[data-test-review]').click();return 'review';");
  await pollValue("shaft experiment review", () => inner("return d.querySelector('[data-test-state]').textContent;"), value => value.includes("Simulation test ready"), 10000);
  await inner("d.querySelector('[data-test-approve]').click();return 'start';");
  await pollValue("shaft experiment running", () => inner("return d.querySelector('[data-test-state]').textContent;"), value => value.includes("Test running"), 10000);
  await pollValue("shaft restoration", () => inner("return d.querySelector('[data-test-state]').textContent;"), value => /Completed/i.test(value), 15000);
  await stage("Monitor");
  const evidence = await pollValue("shaft recording evidence", () => inner("return d.querySelector('[data-control-studio-recording-evidence]').textContent;"), value => !value.includes("Checking"), 10000);
  assert.match(evidence, /Last test: Completed\. Recording retained/);
  await stage("Tune");
  await inner("d.querySelector('[data-declared-root-save]').click();return 'save';");
  await pollValue("saved shaft response", () => inner("return d.querySelector('[data-declared-root-save-status]').textContent;"), value => value.includes("Saved revision"), 10000);
  await inner("d.defaultView.addEventListener('message',e=>{if(e.data?.kind==='control-studio.workflow-result'&&e.data?.payload?.result)d.defaultView.__shaftResult=e.data.payload.result;});var h=d.querySelector('[data-result-history]');h.value='1';h.dispatchEvent(new Event('change'));d.querySelector('[data-result-open]').click();return 'open saved';");
  const json = await pollValue("retained actual shaft response", () => inner("return d.defaultView.__shaftResult?JSON.stringify(d.defaultView.__shaftResult):'';"), value => value.length > 0, 10000);
  const result = JSON.parse(json) as ControlStudioWorkflowResult;
  const samples = result.recording.rawSamples;
  assert.strictEqual(result.evidence.observation, "Observed");
  assert.strictEqual(result.recording.acquisition?.completeness, "complete");
  assert.match(result.recording.acquisition?.reason ?? "", /Stop: stopped/, "Test acquisition must continue through restoration before its owner stops it");
  assert.ok(samples.length >= 390, "retain the complete eight-second shaft response");
  assert.ok(samples.some(sample => sample.controllerMode === "Auto"));
  assert.strictEqual(samples[samples.length - 1].controllerMode, "Manual", "record post-experiment restoration before stopping acquisition");
  const response = samples.filter(sample => sample.controllerMode === "Auto");
  assert.ok(Math.max(...response.map(sample => sample.pv!)) <= 1.1);
  assert.ok(response.every(sample => sample.appliedMv !== null && Math.abs(sample.appliedMv) <= 20));
  assert.ok(Math.abs(response[response.length - 1].pv! - 1) <= 0.02);
  await stage("Tune");
}
