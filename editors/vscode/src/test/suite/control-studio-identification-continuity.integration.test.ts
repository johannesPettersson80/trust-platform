import * as assert from "assert";
import * as fs from "fs";
import * as path from "path";
import { withStudio } from "./control-studio-three-page.integration.test";
import { pollValue, requiredEnvironment } from "./plant-rendered-harness";

type View = (body: string) => Promise<string>;
// A 20 s experiment covers the 4 s process several times over, so eight identifications fit the budget.
const setup = (loop: any) => { loop.experiment = { ...loop.experiment, kind: "relay", amplitude: 1, durationSeconds: 20, relayDirection: "positive" }; };
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
    // The held part runs from the step until the input is restored at the end of the experiment.
    const stepStart = points.find((p: any) => p.value !== points[0].value)?.timeSeconds;
    const end = points[points.length - 1].timeSeconds;
    const plateau = points.filter((p: any) => p.timeSeconds > stepStart + 1 && p.timeSeconds < end - 1).map((p: any) => p.value);
    assert.ok(stepStart !== undefined && plateau.length > 10, "the displayed step input leaves its baseline and holds");
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
    this.timeout(900000);
    await withStudio(async (inner, reopen, capture) => {
      const ids = new Set<string>(); await listen(inner);
      for (const method of ["multisine", "mv_step", "mv_pulse", "prbs", "relay", "setpoint_step", "automatic"]) {
        await identify(inner, method, capture, ids);
      }
      await reopen(); await listen(inner);
      await identify(inner, "mv_step", capture, ids);
    }, { loop: setup });
  });
});
