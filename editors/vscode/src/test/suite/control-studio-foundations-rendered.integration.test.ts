import * as assert from "assert";
import * as fs from "fs";
import * as path from "path";
import * as vscode from "vscode";

import { runtimeLifecycleService } from "../../runtimeLifecycle";
import { attachControlStudio } from "./control-studio-rendered-harness";
import {
  captureRenderedWorkbench,
  connectCdp,
  devToolsPort,
  evaluateInWebview,
  httpJson,
  pollValue,
  requiredEnvironment,
  sleep,
} from "./plant-rendered-harness";
import type { ControlStudioWorkflowResult, WorkflowResultSample } from "../../controlStudio/workflowResult";

type LessonExpectation = {
  readonly id: "control-lesson-01" | "control-lesson-02" | "control-lesson-03" | "control-capstone-cold-storage";
  readonly projectFolder: string;
};

const lessons: readonly LessonExpectation[] = [
  { id: "control-lesson-01", projectFolder: "01-cold-storage-loop-anatomy" },
  { id: "control-lesson-02", projectFolder: "02-cold-storage-on-off" },
  { id: "control-lesson-03", projectFolder: "03-cold-storage-hysteresis-dwell" },
  { id: "control-capstone-cold-storage", projectFolder: "80-cold-storage" },
];

suite("Control Studio foundations actual rendered workflow", () => {
  for (const lesson of lessons) {
    test(`${lesson.id} starts the real simulator, runs its project-owned scenario, and retains its recording`, async function () {
      if (process.env.TRUST_CONTROL_STUDIO_FOUNDATIONS !== "1") this.skip();
      this.timeout(190_000);
      await vscode.extensions.getExtension("trust-platform.trust-lsp")?.activate();
      const project = vscode.workspace.workspaceFolders?.[0]?.uri.fsPath;
      assert.ok(project, "the rendered runner must open a copied lesson project");
      const loop = JSON.parse(fs.readFileSync(path.join(project, "control-loop.json"), "utf8"));
      assert.strictEqual(loop.id, lesson.id, "the runner must exercise the authoritative lesson binding");
      if (lesson.id === "control-capstone-cold-storage" && process.env.TRUST_FOUNDATION_CHANGED_DOOR === "1") {
        assert.match(fs.readFileSync(path.join(project, "src", "Main.st"), "utf8"), /DoorCloseScan\s*:\s*DINT\s*:=\s*DINT#1050/,
          "the copied changed-load fixture must extend only the door-close scan from 90 to 105 seconds");
      }
      const authorityChannelId = loop.signals.additional.find((signal: { id: string; address: string }) =>
        signal.address === loop.experiment.activation.address
      )?.id;
      assert.ok(authorityChannelId, "the rendered lesson must record its experiment-authority channel");

      const port = await devToolsPort(requiredEnvironment("TRUST_PLANT_RENDERED_USER_DATA_DIR"), 15_000);
      const connection = await connectCdp((await httpJson(port, "/json/version")).webSocketDebuggerUrl);
      try {
        await runtimeLifecycleService.stopRuntime();
        const start = await runtimeLifecycleService.startLocalSimulator();
        assert.strictEqual(start.ok, true, JSON.stringify(start));
        await vscode.commands.executeCommand("trust-lsp.controlStudio.open", {
          schemaVersion: 1,
          source: "ConfiguredLoop",
          scope: { projectId: loop.projectId, runtimeId: loop.runtimeId },
          loopDefinition: loop,
        });
        const session = await attachControlStudio(connection, port);
        const inner = (body: string) => evaluateInWebview(connection, session, body);
        const stage = async (name: "Monitor" | "Test" | "Validate") => inner(
          `var b=Array.from(d.querySelectorAll('[data-control-studio-mode]')).find(x=>x.textContent.trim()===${JSON.stringify(name)});if(!b)return 'missing';b.click();return b.textContent.trim();`
        );

        await pollValue("fresh lesson sample", async () => {
          await runtimeLifecycleService.requestIoState();
          return inner("return d.querySelector('[data-control-studio-quality]')?.textContent||'';");
        }, text => !/Unavailable|Waiting|stale/i.test(text), 15_000);

        assert.strictEqual(await stage("Monitor"), "Monitor");
        await inner("d.querySelector('[data-control-studio-record-start]').click();return 'recording';");
        await pollValue("recording started", () => inner("return String(d.querySelector('[data-control-studio-record-stop]').disabled);"), value => value === "false", 10_000);

        assert.strictEqual(await stage("Test"), "Test");
        await pollValue("fresh scenario review sample", async () => {
          await runtimeLifecycleService.requestIoState();
          return inner("return d.querySelector('[data-control-studio-quality]')?.textContent||'';");
        }, text => !/Unavailable|Waiting|stale/i.test(text), 15_000);
        await sleep(300);
        await runtimeLifecycleService.requestIoState();
        await inner("d.querySelector('[data-test-review]').click();return 'review';");
        const review = await pollValue("project-owned scenario review", () =>
          inner("return d.querySelector('[data-test-state]')?.textContent||'';"),
        value => value.includes("Simulation test ready"), 15_000);
        assert.match(review, /Simulation test ready/);
        assert.match(await inner("return d.querySelector('[data-test-signal]')?.textContent||'';"), /^P1\.ExperimentEnabled · Disabled → Enabled/);
        assert.match(await inner("return d.querySelector('[data-test-timeout]')?.textContent||'';"), /120\.1 s maximum/);
        await inner("d.querySelector('[data-test-approve]').click();return 'started';");
        await pollValue("lesson scenario running", () => inner("return d.querySelector('[data-test-state]')?.textContent||'';"), value => value.includes("Test running"), 10_000);
        const terminal = await pollValue("lesson scenario restoration", () => inner("return d.querySelector('[data-test-state]')?.textContent||'';"), value => /Completed|restor/i.test(value), 145_000);
        assert.match(terminal, /Completed|restor/i);

        assert.strictEqual(await stage("Monitor"), "Monitor");
        await inner("d.querySelector('[data-control-studio-record-stop]').click();return 'stopped';");
        await pollValue("complete runtime recording", () => inner("return d.querySelector('[data-control-studio-recording-evidence]')?.textContent||'';"), value => /Complete recording|Recording retained/.test(value), 15_000);

        const captureDir = process.env.TRUST_CONTROL_STUDIO_FOUNDATIONS_SCREENSHOT_DIR;
        if (captureDir) {
          fs.mkdirSync(captureDir, { recursive: true });
          await vscode.commands.executeCommand("notifications.clearAll");
          captureRenderedWorkbench(path.join(captureDir, `${lesson.id}-monitor.png`));
        }

        assert.strictEqual(await stage("Validate"), "Validate");
        assert.strictEqual(await inner("return String(d.querySelector('[data-result-save]').disabled);"), "false", "a complete actual lesson recording must be saveable");
        await inner("d.defaultView.addEventListener('message',e=>{if(e.data?.kind==='control-studio.workflow-result'&&e.data?.payload?.result)d.defaultView.__foundationResult=e.data.payload.result;});d.querySelector('[data-result-save]').click();return 'save';");
        await pollValue("saved lesson result", () => inner("return d.querySelector('[data-result-state]')?.textContent||'';"), value => value.includes("Saved revision"), 15_000);
        await inner("var h=d.querySelector('[data-result-history]');h.value=Array.from(h.options).filter(o=>o.value).sort((a,b)=>Number(b.value)-Number(a.value))[0].value;h.dispatchEvent(new Event('change'));d.querySelector('[data-result-open]').click();return 'open';");
        const encoded = await pollValue("retained raw recording", () => inner("return d.defaultView.__foundationResult?JSON.stringify(d.defaultView.__foundationResult):'';"), value => value.length > 0, 15_000);
        const result = JSON.parse(encoded) as ControlStudioWorkflowResult;
        assert.strictEqual(result.loop.id, lesson.id);
        assert.strictEqual(result.recording.acquisition?.method, "runtime_scan_buffer");
        assert.strictEqual(result.recording.acquisition?.completeness, "complete");
        assert.ok(result.recording.rawSamples.length >= 1_100, `expected the real 120.1 s / 0.1 s recording, got ${result.recording.rawSamples.length}`);
        assert.ok(result.recording.rawSamples.every(sample => sample.appliedMv !== null && sample.appliedMv >= 0 && sample.appliedMv <= 100));
        assert.ok(result.recording.rawSamples.some(sample => auxiliary(sample, authorityChannelId) === 1), "the scenario must really assert ExperimentEnabled");
        assert.strictEqual(auxiliary(result.recording.rawSamples[result.recording.rawSamples.length - 1], authorityChannelId), 0, "normal completion must restore ExperimentEnabled");
        if (lesson.id === "control-lesson-01") assertManualObservation(result.recording.rawSamples, authorityChannelId);
        else if (lesson.id === "control-lesson-02") assertSingleThresholdFeedback(result.recording.rawSamples, authorityChannelId);
        else if (lesson.id === "control-lesson-03") assertHysteresisAndDwell(result.recording.rawSamples, authorityChannelId);
        else assertColdRoomCommissioning(result.recording.rawSamples, authorityChannelId);
      } finally {
        connection.close();
        await runtimeLifecycleService.stopRuntime();
      }
    });
  }
});

function auxiliary(sample: WorkflowResultSample | undefined, id: string): number | null | undefined {
  return sample?.additional?.find(channel => channel.id === id)?.value;
}

function assertManualObservation(samples: readonly WorkflowResultSample[], authorityChannelId: string): void {
  const active = samples.filter(sample => auxiliary(sample, authorityChannelId) === 1);
  assert.ok(active.some(sample => sample.appliedMv === 100), "manual lesson must command full cooling");
  assert.ok(active.some(sample => sample.appliedMv === 0), "manual lesson must restore cooling OFF during the same run");
  const firstCooling = active.find(sample => sample.appliedMv === 100)?.pv;
  const minimum = Math.min(...active.map(sample => sample.pv ?? Infinity));
  const afterCooling = active.find(sample => sample.appliedMv === 0 && sample.pv !== null)?.pv;
  assert.ok(firstCooling !== null && firstCooling !== undefined && minimum < firstCooling - 1.5, "the real plant must cool while the fixed command is ON");
  assert.ok(afterCooling !== null && afterCooling !== undefined && afterCooling >= minimum, "the fixed sequence must release the room to warm after cooling ends");
}

function assertSingleThresholdFeedback(samples: readonly WorkflowResultSample[], authorityChannelId: string): void {
  const active = samples.filter(sample => auxiliary(sample, authorityChannelId) === 1);
  assert.ok(active.some(sample => sample.appliedMv === 100), "ON/OFF lesson must turn refrigeration ON above the threshold");
  assert.ok(active.some(sample => sample.appliedMv === 0), "ON/OFF lesson must turn refrigeration OFF below the threshold");
  const changes = active.slice(1).filter((sample, index) => sample.appliedMv !== active[index].appliedMv).length;
  assert.ok(changes > 100, `single-threshold control must visibly chatter, got ${changes} observed changes`);
  assert.ok(active.every(sample => auxiliary(sample, "switch-count") !== null), "the rendered recording must retain the project switch-count channel");
}

function assertHysteresisAndDwell(samples: readonly WorkflowResultSample[], authorityChannelId: string): void {
  const active = samples.filter(sample => auxiliary(sample, authorityChannelId) === 1);
  assert.ok(active.some(sample => auxiliary(sample, "dwell-inhibit") === 1), "dwell must visibly postpone a requested state change");
  assert.ok(active.some(sample => auxiliary(sample, "requested-cooling-state") !== auxiliary(sample, "applied-cooling-state")), "requested and applied cooling must differ while dwell inhibits a normal transition");
  assert.ok(active.every(sample => [0, 100].includes(sample.appliedMv ?? Number.NaN)), "the applied compressor command must remain binary");
  const changes = active.slice(1).filter((sample, index) => sample.appliedMv !== active[index].appliedMv).length;
  assert.ok(changes > 0 && changes <= 20, `hysteresis and dwell must replace chatter with bounded transitions, got ${changes}`);
  assert.ok(active.some(sample => (auxiliary(sample, "dwell-elapsed") ?? 0) >= 8), "recorded state timing must reach the declared eight-second dwell");
}

function assertColdRoomCommissioning(samples: readonly WorkflowResultSample[], authorityChannelId: string): void {
  const active = samples.filter(sample => auxiliary(sample, authorityChannelId) === 1);
  assert.ok(active.every(sample => [0, 100].includes(sample.appliedMv ?? Number.NaN)), "commissioning command must remain exactly OFF or ON");
  const postCooldown = active.slice(400);
  assert.ok(postCooldown.length >= 700, "the baseline must retain the declared post-cooldown window");
  assert.ok(postCooldown.every(sample => sample.pv !== null && sample.pv >= 1 && sample.pv <= 3), "the baseline commissioning trace must keep PV in the declared 1–3 degC product band after 40 seconds");
  assert.ok(active.every(sample => auxiliary(sample, "temperature-quality-good") === 1), "commissioning evidence must retain good temperature quality");
  assert.ok(active.some(sample => auxiliary(sample, "applied-state-transition-count") !== null), "commissioning evidence must retain applied transition counts");
  const transitions = active.slice(1).map((sample, index) => sample.appliedMv !== active[index].appliedMv ? index + 1 : -1)
    .filter(index => index >= 0);
  const normalIntervals = transitions.slice(0, -1).map((transition, index) => transition - (transitions[index - 1] ?? 0));
  assert.ok(normalIntervals.every(interval => interval >= 80), `every completed normal applied state must meet the 8-second dwell, got scan intervals ${normalIntervals.join(", ")}`);
}
