import * as assert from "assert";
import * as fs from "fs";
import * as path from "path";
import { withStudio } from "./control-studio-three-page.integration.test";
import { pollValue, requiredEnvironment } from "./plant-rendered-harness";

type View = (body: string) => Promise<string>;
const setup = (loop: any) => { loop.experiment = { ...loop.experiment, kind: "relay", amplitude: 1, durationSeconds: 60, relayDirection: "positive" }; };
const output = (name: string, value: unknown) => fs.writeFileSync(path.join(path.dirname(requiredEnvironment("TRUST_CONTROL_STUDIO_RENDERED_SCREENSHOT")), `${name}.json`), JSON.stringify(value));
async function listen(inner: View) {
  await inner("d.defaultView.__continuity=[];d.defaultView.addEventListener('trust-control-studio-engineer',e=>{const p=e.detail;d.defaultView.__continuity.push({status:p.status,reason:p.reason,stage:p.flowStage,runs:p.identification?.runs,presentation:p.identificationPresentation});});return 'listening';");
}
async function identify(inner: View, method: string, capture: (name: string) => Promise<void>, previousIds: Set<string>, prefix = "continuity") {
  await inner(`d.defaultView.__continuity=[];const m=d.querySelector('[data-engineer-acquisition-method]');m.value=${JSON.stringify(method)};m.dispatchEvent(new d.defaultView.Event('change',{bubbles:true}));return 'method';`);
  await pollValue("saved selection", () => inner("return JSON.stringify(d.defaultView.__continuity);"), raw => JSON.parse(raw).some((e: any) => e.status === "edited"), 10000);
  assert.strictEqual(await inner("return String(d.defaultView.__continuity.some(e=>e.status==='running'));"), "false", "Method selection alone does not excite the plant");
  await inner("d.querySelector('[data-engineer-find]').click();return 'identify';");
  const events = JSON.parse(await pollValue(`${method} complete`, () => inner("return JSON.stringify(d.defaultView.__continuity);"), raw => JSON.parse(raw).some((e: any) => ["completed", "failed", "paused"].includes(e.status)), 90000));
  output(`${prefix}-${method}-${previousIds.size}`, events);
  await capture(`${prefix}-${method}-${previousIds.size}`);
  assert.ok(events.some((e: any) => e.status === "completed"), JSON.stringify(events.map((e: any) => ({status:e.status,reason:e.reason}))));
  const result = [...events].reverse().find((e: any) => e.status === "model-ready");
  assert.strictEqual(result?.runs?.length, 1);
  for (const run of result.runs) {
    assert.ok(!previousIds.has(run.run.id), `${method} must execute new ST recordings instead of reusing the previous method's experiment`);
    assert.strictEqual(run.run.evidence, "ST-executed");
    assert.strictEqual(run.evaluation.status, "Pass");
    previousIds.add(run.run.id);
  }
  const first = result.runs[0].request;
  if (["mv_step", "automatic"].includes(method)) {
    assert.strictEqual(first.experiment.kind, "mv_step");
    assert.strictEqual(first.experiment.trajectory.length, 1, "One held step, with native end restoration");
    const points = result.presentation.experiment.inputs[0].points;
    const plateau = points.filter((p: any) => p.timeSeconds > 15 && p.timeSeconds < 50).map((p: any) => p.value);
    assert.ok(Math.max(...plateau) - Math.min(...plateau) < 1e-6, "Actual displayed step input has a held plateau");
  }
  if (method === "relay") assert.ok(first.experiment.relayFeedback && first.experiment.trajectory.length === 0);
  if (method === "prbs" || method === "multisine") assert.ok(first.experiment.trajectory.length > 15);
  assert.strictEqual(await inner("return d.querySelector('[data-control-studio-chart]').dataset.seriesCount;"), "3");
  return result;
}

suite("Control Studio identification continuity", () => {
  test("runs each selected method in the same study and reacquires after reopening", async function () {
    if (process.env.TRUST_IDENTIFY_CONTINUOUS !== "1") this.skip();
    this.timeout(600000);
    await withStudio(async (inner, reopen, capture) => {
      const ids = new Set<string>(); await listen(inner);
      for (const method of ["multisine", "mv_step", "mv_pulse", "prbs", "relay", "setpoint_step", "automatic"]) {
        await identify(inner, method, capture, ids);
      }
      await reopen(); await listen(inner);
      await identify(inner, "mv_step", capture, ids);
    }, { loop: setup });
  });

  test("Reset clears the model and dependent result state permanently before a new identification", async function () {
    if (process.env.TRUST_IDENTIFY_CONTINUOUS !== "1") this.skip();
    this.timeout(180000);
    await withStudio(async (inner, reopen, capture) => {
      await listen(inner); const ids = new Set<string>(); await identify(inner, "multisine", capture, ids, "reset-sequence");
      const bindings = await inner("return JSON.stringify([...d.querySelectorAll('[data-studio-inputs],[data-studio-outputs]')].map(s=>s.value));");
      await inner("d.querySelector('[data-studio-reset-chart]').click();return 'reset';");
      await pollValue("fresh reset document", () => inner("return String(d?.querySelector('[data-studio-three-page]')?.dataset.hostReady === 'true' && d?.querySelector('[data-studio-model-result]')?.hidden === true);").catch(() => "loading"), value => value === "true", 15000);
      const state = JSON.parse(await inner("return JSON.stringify({modelVisible:d.querySelector('[data-studio-model-result]').checkVisibility(),equation:d.querySelector('[data-studio-model-equation]').textContent,points:d.querySelector('[data-control-studio-chart]').dataset.pointCount,tuneDisabled:d.querySelector('[data-engineer-tune]').disabled,validateDisabled:d.querySelector('[data-engineer-verify]').disabled,applyDisabled:d.querySelector('[data-engineer-apply]').disabled});"));
      output("reset-result",state); await capture("reset-result");
      assert.strictEqual(state.modelVisible, false, "Reset must remove the identified model, not only its chart");
      assert.strictEqual(state.equation, ""); assert.strictEqual(state.points, "0");
      assert.ok(state.tuneDisabled && state.validateDisabled && state.applyDisabled, "No model, candidate or validation authority remains");
      assert.strictEqual(await inner("return JSON.stringify([...d.querySelectorAll('[data-studio-inputs],[data-studio-outputs]')].map(s=>s.value));"), bindings);
      await reopen(); await listen(inner);
      assert.strictEqual(await inner("return String(d.querySelector('[data-studio-model-result]').checkVisibility());"), "false", "Reopening must not restore the reset model");
      assert.strictEqual(await inner("return String(d.querySelector('[data-engineer-tune]').disabled);"), "true");
      await capture("reset-reopened");
      await identify(inner, "mv_step", capture, ids, "reset-sequence");
    }, { loop: setup });
  });
});
