import { runSelectedShaftExperiment } from "./control-studio-root-experiment-harness";
import { runtimeLifecycleService } from "../../runtimeLifecycle";
import * as assert from "assert";
import * as vscode from "vscode";
import * as fs from "fs";
import * as path from "path";
import { ParameterApplicationClient } from "../../controlStudio/parameterApplication";
import { attachControlStudio } from "./control-studio-rendered-harness";
import { captureRenderedWorkbench, connectCdp, devToolsPort, evaluateInWebview, httpJson, pollValue, requiredEnvironment, sleep } from "./plant-rendered-harness";

suite("Rendered generic parameter workflow", () => {
  test("reviews a selected gain, confirms the observed value, and rolls back", async function () {
    if (process.env.TRUST_PLANT_RENDERED_TEST !== "1") this.skip();
    this.timeout(60000);
    await vscode.extensions.getExtension("trust-platform.trust-lsp")?.activate();
    const theme = process.env.TRUST_CONTROL_STUDIO_RENDERED_THEME;
    if (theme) await vscode.workspace.getConfiguration("workbench").update("colorTheme", theme, vscode.ConfigurationTarget.Global);
    const actualRuntime = process.env.TRUST_CONTROL_STUDIO_ACTUAL_PARAMETERS === "1";
    const repository = process.env.TRUST_REPOSITORY_ROOT ?? path.resolve(__dirname, "../../../../..");
    const loop = JSON.parse(fs.readFileSync(path.join(repository, "examples/control/07-classical-stability-and-frequency-design/45-root-locus/control-loop.json"), "utf8"));
    assert.ok(loop.parameterBinding && loop.analysisParameterSymbols, "root project must supply its real parameter mapping");
    let transport: any;
    try { transport = require("../../controlStudio/runtimeParameterConnection"); } catch { transport = undefined; }
    const previousFactory = transport?.connectRuntimeParameters;
    let values = { "P1.Kp": 16, "P1.Kd": 7 };
    let previous: typeof values | null = null;
    let state = "idle";
    const calls: string[] = [];
    if (transport && !actualRuntime) transport.connectRuntimeParameters = async () => ({ source: "simulation", client: new ParameterApplicationClient(loop.parameterBinding, "simulation", async (operation, params) => {
      calls.push(operation);
      if (operation === "control.parameters.apply") { previous = values; values = params.values as typeof values; state = "applied"; return { status: "pending" }; }
      if (operation === "control.parameters.rollback") { values = previous!; previous = null; state = "applied"; return { status: "pending" }; }
      return { program_revision: "program", binding_revision: "binding", active_revision: JSON.stringify(values), values, status: state,
        previous_revision: previous ? JSON.stringify(previous) : null, previous_values: previous, reason: null };
    }) });
    if (actualRuntime) {
      const started = await runtimeLifecycleService.startLocalSimulator();
      assert.strictEqual(started.ok, true, JSON.stringify(started));
    }
    const observedValues = async () => actualRuntime ? (await (await previousFactory(loop)).client.status()).values : values;
    const connection = await connectCdp((await httpJson(await devToolsPort(requiredEnvironment("TRUST_PLANT_RENDERED_USER_DATA_DIR"), 15000), "/json/version")).webSocketDebuggerUrl);
    try {
      await vscode.commands.executeCommand("trust-lsp.controlStudio.open", { schemaVersion: 1, source: "ConfiguredLoop",
        scope: { projectId: loop.projectId, runtimeId: loop.runtimeId }, loopDefinition: loop });
      const session = await attachControlStudio(connection, await devToolsPort(requiredEnvironment("TRUST_PLANT_RENDERED_USER_DATA_DIR"), 15000));
      const inner = (body: string) => evaluateInWebview(connection, session, body);
      await inner("Array.from(d.querySelectorAll('[data-control-studio-mode]')).find(b=>b.textContent.trim()==='Tune').click();return 'tune';");
      assert.strictEqual(await inner("return String(Boolean(d.querySelector('[data-parameters-inspect]')));"), "true", "custom ST needs the generic parameter review controls");
      await pollValue("parameter handlers ready", () => inner("return d.querySelector('[data-parameters]').dataset.ready;"), value => value === "true", 10000);
      await inner("const tools=d.querySelector('[data-engineer-tools]');if(tools){const parents=[];for(let p=tools.parentElement;p;p=p.parentElement)if(p.tagName==='DETAILS')parents.push(p);for(const p of parents.reverse())if(!p.open)p.querySelector(':scope > summary').click();tools.click();}return 'advanced parameter controls';");
      await inner("Array.from(d.querySelectorAll('[data-control-studio-mode]')).find(b=>b.textContent.trim()==='Tune').click();return 'tune';");
      await pollValue("visible Tune pane", () => inner("return String(!d.querySelector('[data-workflow-pane=Tune]').hidden);"), value => value === "true", 10000);
      await inner("d.querySelector('[data-parameters-inspect]').click();return 'inspect';");
      await pollValue("inspected parameters", () => inner("return d.querySelector('[data-parameters-values]').value+' | '+d.querySelector('[data-parameters-status]').textContent;"), value => value.includes("P1.Kp"), 10000);
      await inner("d.querySelector('[data-declared-root-run]').click();return 'calculate';");
      await pollValue("calculated root candidate", () => inner("return d.querySelector('[data-declared-root-result]').textContent;"), value => value.includes("Kp = 20"), 10000);
      await inner("d.querySelector('[data-declared-root-select=\"20\"]').click();return 'select';");
      await pollValue("selected root gain", () => inner("return d.querySelector('[data-declared-root-result] tr[data-selected=true]').textContent;"), value => value.includes("Kp = 20"), 10000);
      await inner("d.querySelector('[data-parameters-selected]').click();return 'copy selected';");
      await pollValue("selected values populated", () => inner("return d.querySelector('[data-parameters-values]').value;"), value => value.includes("20"), 10000);
      await inner("d.querySelector('[data-parameters-review]').click();return 'review';");
      await pollValue("exact review", () => inner("return d.querySelector('[data-parameters-review-table]').textContent;"), value => value.includes("20") && value.includes("N·m/rad"), 10000);
      assert.strictEqual(await inner("var input=d.querySelector('[data-parameters-values]');var expected=d.defaultView.getComputedStyle(d.body).getPropertyValue('--vscode-input-background').trim();var probe=d.createElement('div');probe.style.backgroundColor=expected;d.body.appendChild(probe);var same=d.defaultView.getComputedStyle(input).backgroundColor===d.defaultView.getComputedStyle(probe).backgroundColor;probe.remove();return String(same);"), "true");
      assert.ok(!calls.includes("control.parameters.apply"), "reviewing must not write");
      await inner("d.querySelector('[data-parameters-review-area]').scrollIntoView({block:'center'});return 'review visible';");
      assert.strictEqual(await inner("var r=d.querySelector('[data-parameters-review-area]').getBoundingClientRect();return String(r.height>0&&r.top>=0&&r.bottom<=d.defaultView.innerHeight);"), "true", "the complete review must be visible before approval");
      await vscode.commands.executeCommand("notifications.clearAll");
      await sleep(350);
      if (process.env.TRUST_CONTROL_STUDIO_RENDERED_SCREENSHOT) captureRenderedWorkbench(process.env.TRUST_CONTROL_STUDIO_RENDERED_SCREENSHOT);
      await inner("d.querySelector('[data-parameters-apply]').click();return 'apply';");
      await pollValue("observed confirmation", () => inner("return d.querySelector('[data-parameters-status]').textContent;"), value => value.includes("observed after ST"), 10000);
      assert.strictEqual((await observedValues())["P1.Kp"], 20);
      if (process.env.TRUST_CONTROL_STUDIO_RENDERED_SCREENSHOT) captureRenderedWorkbench(process.env.TRUST_CONTROL_STUDIO_RENDERED_SCREENSHOT.replace(/\.png$/, "-applied.png"));
      if (actualRuntime) await runSelectedShaftExperiment(inner);
      await inner("d.querySelector('[data-parameters-rollback]').click();return 'review rollback';");
      await pollValue("rollback review", () => inner("return d.querySelector('[data-parameters-apply]').textContent;"), value => value.includes("rollback"), 10000);
      await inner("d.querySelector('[data-parameters-apply]').click();return 'rollback';");
      await pollValue("rollback confirmation", () => inner("return d.querySelector('[data-parameters-status]').textContent;"), value => value.includes("Previous values restored"), 10000);
      assert.strictEqual((await observedValues())["P1.Kp"], 16);
      await inner("d.querySelector('[data-parameters-status]').scrollIntoView({block:'center'});return 'visible';");
      if (process.env.TRUST_CONTROL_STUDIO_RENDERED_SCREENSHOT) captureRenderedWorkbench(process.env.TRUST_CONTROL_STUDIO_RENDERED_SCREENSHOT.replace(/\.png$/, "-rollback.png"));
    } finally {
      connection.close();
      if (actualRuntime) await runtimeLifecycleService.stopRuntime();
      if (transport && !actualRuntime) transport.connectRuntimeParameters = previousFactory;
    }
  });
});
