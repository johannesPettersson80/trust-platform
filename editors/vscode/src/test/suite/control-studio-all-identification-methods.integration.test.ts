import * as assert from "assert";
import * as fs from "fs";
import * as path from "path";
import { withStudio } from "./control-studio-three-page.integration.test";
import { pollValue, requiredEnvironment } from "./plant-rendered-harness";

const clippedRelay = process.env.TRUST_IDENTIFY_RELAY_CLIPPED === "1";
const methods = ["automatic", "mv_step", "mv_pulse", "setpoint_step", "relay", "prbs", "multisine"] as const;
suite("Control Studio all identification methods", () => {
  for (const method of methods) test(clippedRelay && method === "relay" ? "reports clipped relay output and retains the actual experiment" : `executes ${method} from the method selector and shows its actual experiment`, async function () {
    if (process.env.TRUST_IDENTIFY_ALL_METHODS !== "1") this.skip();
    const selected = process.env.TRUST_IDENTIFY_METHODS?.split(",") ?? methods;
    if (!selected.includes(method)) this.skip();
    this.timeout(210000);
    let loop: any;
    let observations: unknown;
    const suffix = process.env.TRUST_CONTROL_STUDIO_PID_GROUP_NATIVE === "1" ? "mimo" : "siso";
    try {
      await withStudio(async (inner, reopen, capture) => {
        assert.strictEqual(loop.controller.type, "Control.PID", "Execute the real library controller");
        await inner(`d.querySelector('[data-engineer-stage-tab=model]').click();d.defaultView.__methodEvents=[];d.defaultView.addEventListener('trust-control-studio-engineer',e=>{const p=e.detail;d.defaultView.__methodEvents.push({status:p.status,flowStage:p.flowStage,reason:p.reason,experiment:p.identificationExperiment,model:p.model,presentation:p.identificationPresentation,runs:p.identification?.runs?.map(r=>({id:r.run.id,request:r.request,run:r.run,evaluation:r.evaluation.status}))});});const method=d.querySelector('[data-engineer-acquisition-method]');method.value=${JSON.stringify(method)};method.dispatchEvent(new d.defaultView.Event('change',{bubbles:true}));return 'method selected';`);
        await pollValue("method input stored", () => inner("return String(d.querySelector('[data-studio-three-page]').dataset.busy??'false');"), value => value === "false", 10000);
        assert.strictEqual(await inner(`return d.querySelector('[data-engineer-acquisition-method]').value;`), method);
        assert.strictEqual(await inner("const button=d.querySelector('[data-engineer-find]');if(!button.checkVisibility()||button.disabled)return 'unavailable';button.click();return 'started';"), "started");
        const events = JSON.parse(await pollValue(`${method} terminal`, () => inner("return JSON.stringify(d.defaultView.__methodEvents);"), raw => JSON.parse(raw).some((event: any) => ["completed", "paused", "failed"].includes(event.status)), 170000));
        observations = events;
        if (clippedRelay && method === "relay") {
          assert.ok(events.some((event: any) => event.status === "failed" && /clipped or intermediate output values/.test(event.reason)), "Explain the rate-limited waveform");
          assert.ok(events.some((event: any) => event.experiment?.inputs?.length), "Keep the rejected experiment visible");
          assert.ok(Number(await inner("return d.querySelector('[data-control-studio-chart]').dataset.pointCount;")) > 100);
          await capture("method-siso-relay-clipped");
          await inner("d.querySelector('[data-studio-reset-chart]').click();return 'reset';");
          await pollValue("reset complete", () => inner("return d?.querySelector('[data-control-studio-chart]')?.dataset.pointCount??'loading';").catch(() => "loading"), value => Number(value) >= 4, 10000);
          return;
        }
        assert.ok(events.some((event: any) => event.status === "completed" && event.flowStage === "Identify"),
          JSON.stringify(events.map((event: any) => ({ status: event.status, reason: event.reason }))));
        assert.ok(events.filter((event: any) => event.runs).every((event: any) => event.runs.every((run: any) => run.request.execution === "connected-runtime" && run.request.acquisition.method === "runtime_scan_buffer")), "Use the connected plant's scan buffer");
        const result = [...events].reverse().find((event: any) => event.status === "model-ready");
        assert.ok(result?.model, "The selected method must produce a provisional plant model");
        assert.ok(result.runs?.length === 1 && result.runs.every((run: any) => run.run.evidence === "ST-executed" && run.evaluation === "Pass"), "Use one bounded actual-ST estimation record");
        assert.strictEqual(await inner("return d.querySelector('[data-studio-three-page]').dataset.selectedStage;"), "model");
        assert.strictEqual(await inner("return String(!!d.querySelector('dialog[open]'));"), "false");
        const experiment = result.presentation?.experiment;
        assert.ok(experiment?.inputs?.length && experiment.outputs?.length, "Show applied inputs and measured outputs with the selected model");
        for (const input of experiment.inputs) {
          const values = input.points.map((point: any) => point.value).filter((value: any) => typeof value === "number");
          assert.ok(Math.max(...values) - Math.min(...values) > 0.001, `${input.id} actually changes`);
        }
        const runs = result.runs.filter((run: any) => run.run.samples.length > 4);
        assert.strictEqual(new Set(runs.map((run: any) => run.id)).size, 1, "Identify performs one acquisition");
        assert.strictEqual(result.model.validation, "unvalidated", "Independent checking belongs on Validate");
        if (method === "mv_pulse") {
          const command = runs[0].request.experiment;
          assert.ok(command.trajectory.some((event: any) => event.value === command.restorationValue && event.atMs > 0 && event.atMs < command.durationMs), "Pulse restores during the record");
        }
        if (method === "setpoint_step") assert.ok(runs.every((run: any) => run.request.experiment.affectedSignal === loop.signals.setpoint.symbol), "Step the declared reference while recording the applied actuator output");
        if (method === "relay") assert.ok(runs.every((run: any) => run.request.experiment.relayFeedback && !run.request.experiment.trajectory.length), "Switch from measured feedback, not a timed square wave");
        if (method === "prbs" || method === "multisine") assert.ok(runs.every((run: any) => run.request.experiment.trajectory.length > 15), "Use the selected frequency excitation sequence");
        assert.ok(Number(await inner("return d.querySelector('[data-control-studio-chart]').dataset.seriesCount;")) >= experiment.outputs.length * 2 + experiment.inputs.length, "Plot every input, measurement and prediction");
        await capture(`method-${suffix}-${method}`);
        assert.ok(Number(await inner("return String(d.querySelector('[data-studio-model-equation]').getBoundingClientRect().height);")) <= 120,
          "The full model equation stays scrollable in a compact result so the experiment remains visible");
        if (suffix === "mimo") {
          await inner("d.defaultView.scrollTo(0,d.body.scrollHeight);return 'last channels';");
          await capture(`method-${suffix}-${method}-lower-channels`);
          await inner("d.defaultView.scrollTo(0,0);return 'top';");
        }
        await reopen();
        await inner("d.querySelector('[data-engineer-stage-tab=model]').click();return 'reopened';");
        assert.match(await inner("return d.querySelector('[data-control-studio-chart]').getAttribute('aria-label');"), /Applied input/);
        await inner("d.querySelector('[data-studio-reset-chart]').click();return 'reset';");
        await pollValue("reset complete", () => inner("return d?.querySelector('[data-control-studio-chart]')?.dataset.pointCount??'loading';").catch(() => "loading"), value => Number(value) >= 4, 10000);
      }, { loop: value => { loop = value; loop.experiment.durationSeconds = suffix === "mimo" ? 60 : 30; loop.experiment.relayDirection = "positive"; if (method === "relay") { loop.experiment.kind = "relay"; loop.experiment.amplitude = clippedRelay ? 10 : 1; } if (suffix === "mimo") loop.identificationPolicy = { ...loop.identificationPolicy, maximumOrder: 8 }; } });
    } finally {
      if (observations) fs.writeFileSync(path.join(path.dirname(requiredEnvironment("TRUST_CONTROL_STUDIO_RENDERED_SCREENSHOT")), `method-${suffix}-${method}.json`), JSON.stringify(observations));
    }
  });
});
