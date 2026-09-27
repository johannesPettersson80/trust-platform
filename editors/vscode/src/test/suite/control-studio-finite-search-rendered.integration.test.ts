import * as assert from "assert";
import * as vscode from "vscode";
import * as fs from "fs";
import * as path from "path";
import { runtimeLifecycleService } from "../../runtimeLifecycle";
import { buildFiniteSearchDraft } from "../../controlStudio/finiteSearchDraft";
import { attachControlStudio } from "./control-studio-rendered-harness";
import { connectCdp, devToolsPort, evaluateInWebview, httpJson, pollValue, requiredEnvironment, captureRenderedWorkbench } from "./plant-rendered-harness";

suite("Rendered finite ST search", () => {
  test("reviews a finite search and retains failed trials before ranking", async function () {
    if (process.env.TRUST_PLANT_RENDERED_TEST !== "1") this.skip();
    this.timeout(120000);
    await vscode.extensions.getExtension("trust-platform.trust-lsp")?.activate();
    const actual = process.env.TRUST_CONTROL_STUDIO_ACTUAL_SEARCH === "1";
    const repository = process.env.TRUST_REPOSITORY_ROOT ?? path.resolve(__dirname, "../../../../..");
    const loop = JSON.parse(fs.readFileSync(path.join(repository, "examples/control/07-classical-stability-and-frequency-design/45-root-locus/control-loop.json"), "utf8"));
    if (!actual) delete loop.plantId;
    const config: any = buildFiniteSearchDraft(loop, { "P1.Kp": 16, "P1.Kd": 7 });
    config.grid[0].values = [0, 12, 20];
    config.requirements.push({ id: "late-position", symbol: "P1.Angle", minimum: 0.98, maximum: 1.02, start_ms: 4000, end_ms: 8000 });
    let transport: any;
    try { transport = require("../../controlStudio/runtimeSearchConnection"); } catch { transport = undefined; }
    const original = transport?.connectRuntimeSearch;
    let starts = 0;
    if (transport && !actual) transport.connectRuntimeSearch = async () => ({ source: "simulation", request: async (operation: string, params: any) => {
      if (operation === "control.search.start") { starts++; return { job_id: "job", project_revision: "source", trial_count: 3 }; }
      if (!params.job_id) return { ready: true, project_revision: "source", source_basis: "loaded-runtime-sources" };
      const trials = [0, 12, 20].map((kp, index) => {
        const parameters = { "P1.Kp": kp, "P1.Kd": 7 };
        const times = Array.from({ length: 406 }, (_, i) => i * 20 / 1000);
        return { index, parameters, status: index === 0 ? "rejected" : "qualified", score: 3 - index, score_unit: "rad^2 s",
          violations: index === 0 ? ["late-position"] : [],
          ...(params.include_results ? { run: { id: `trial-${index}`, evidence: "ST-executed", parameters,
            conditions: { initialState: config.initial_values, events: [], durationSeconds: 8.1, sampleTimesSeconds: times, seed: config.seed,
              modelRevision: "model", programRevision: "program", parameterSchemaRevision: "parameters" },
            channels: [{ id: "P1.Angle", unit: "rad" }], samples: times.map((timeSeconds, sequence) => ({ timeSeconds, sequence,
              values: { "P1.Angle": { value: index === 0 ? 0 : 1, quality: "good" } } })) } } : {}) };
      });
      return { job_id: "job", project_revision: "source", status: "completed", trial_count: 3, trials,
        ranking: [2, 1], unattempted_trials: [], source_basis: "loaded-runtime-sources", io_provenance: "isolated-simulation-no-physical-drivers" };
    } });
    if (actual) assert.strictEqual((await runtimeLifecycleService.startLocalSimulator()).ok, true);
    const port = await devToolsPort(requiredEnvironment("TRUST_PLANT_RENDERED_USER_DATA_DIR"), 15000);
    const connection = await connectCdp((await httpJson(port, "/json/version")).webSocketDebuggerUrl);
    try {
      const request = { schemaVersion: 1, source: "ConfiguredLoop", scope: { projectId: loop.projectId, runtimeId: loop.runtimeId }, loopDefinition: loop };
      await vscode.commands.executeCommand("trust-lsp.controlStudio.open", request);
      const session = await attachControlStudio(connection, port);
      const inner = (body: string) => evaluateInWebview(connection, session, body);
      const capture = async (stage: string) => {
        const shot = process.env.TRUST_CONTROL_STUDIO_RENDERED_SCREENSHOT;
        if (shot) { await new Promise(resolve => setTimeout(resolve, 250)); captureRenderedWorkbench(shot.replace(/\.png$/, `-${stage}.png`)); }
      };
      assert.strictEqual(await inner("return String(Boolean(d.querySelector('[data-search-review]')));"), "true", "custom ST needs a finite search review workflow");
      await pollValue("search handlers", () => inner("return d.querySelector('[data-finite-search]').dataset.ready;"), value => value === "true", 10000);
      await inner("const tools=d.querySelector('[data-engineer-tools]');if(tools){const parents=[];for(let p=tools.parentElement;p;p=p.parentElement)if(p.tagName==='DETAILS')parents.push(p);for(const p of parents.reverse())if(!p.open)p.querySelector(':scope > summary').click();tools.click();}Array.from(d.querySelectorAll('[data-control-studio-mode]')).find(b=>b.textContent.trim()==='Tune').click();return 'tune';");
      await inner(`var e=d.querySelector('[data-search-request]');e.value=${JSON.stringify(JSON.stringify(config))};e.dispatchEvent(new Event('input'));d.querySelector('[data-search-review]').click();return 'review';`);
      await pollValue("retained search review", () => inner("return d.querySelector('[data-search-status]').textContent;"), value => value.includes("Review ready"), 15000);
      assert.strictEqual(starts, 0);
      await inner("d.querySelector('[data-search-review]').scrollIntoView({block:'center'});return 'review visible';");
      await capture("search-review");
      await inner("d.querySelector('[data-search-start]').click();return 'start';");
      await pollValue("completed finite search", () => inner("return d.querySelector('[data-search-status]').textContent;"), value => value.includes("Search completed"), 65000);
      const table = await inner("return d.querySelector('[data-search-trials]').textContent;");
      assert.match(table, /rejected/);
      assert.match(table, /late-position/);
      assert.match(table, /qualified/);
      assert.doesNotMatch(table, /\d\.\d{8,}/, "displayed search scores use shared engineering precision");
      assert.strictEqual(await inner("return String(d.querySelectorAll('[data-search-select]').length);"), "2");
      await inner("d.querySelector('[data-search-trials]').scrollIntoView({block:'center'});return 'trials visible';");
      await capture("search-trials");
      await inner("const b=d.querySelector('[data-search-select=\"2\"]');if(!b)throw Error('best qualifying candidate is missing');b.click();return 'selected';");
      const selected = await pollValue("selected best candidate", () => inner("const b=d.querySelector('[data-search-select=\"2\"]');return b?JSON.stringify({text:b.textContent,disabled:b.disabled,row:b.closest('tr')?.textContent}):'';"), value => value.includes("Selected"), 15000);
      const selectedState = JSON.parse(selected) as { text: string; disabled: boolean; row: string };
      assert.strictEqual(selectedState.text, "Selected");
      assert.strictEqual(selectedState.disabled, true);
      assert.match(selectedState.row, /"P1\.Kp":20/);
      await capture("search-selected");
      await inner("d.querySelector('[data-search-save]').click();return 'save';");
      await pollValue("saved search", () => inner("return d.querySelector('[data-result-state]').textContent;"), value => value.includes("Saved revision"), 15000);
      await capture("search-saved");
      const old = await inner("return d.querySelector('[data-control-studio]').dataset.editorSessionId;");
      await vscode.commands.executeCommand("workbench.action.closeActiveEditor");
      await vscode.commands.executeCommand("trust-lsp.controlStudio.open", request);
      const restored = await attachControlStudio(connection, port, old);
      await pollValue("restored rejected trial", () => evaluateInWebview(connection, restored, "return d.querySelector('[data-search-trials]').textContent;"), value => value.includes("late-position"), 10000);
    } finally {
      connection.close();
      if (actual) await runtimeLifecycleService.stopRuntime();
      if (transport) transport.connectRuntimeSearch = original;
    }
  });
});
