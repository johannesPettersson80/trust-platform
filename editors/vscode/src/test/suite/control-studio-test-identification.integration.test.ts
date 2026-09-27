import * as assert from "assert";
import * as fs from "fs";
import * as path from "path";
import * as vscode from "vscode";
import { runtimeLifecycleService } from "../../runtimeLifecycle";
import { attachControlStudio } from "./control-studio-rendered-harness";
import { captureRenderedWorkbench, connectCdp, devToolsPort, evaluateInWebview, httpJson, pollValue, requiredEnvironment } from "./plant-rendered-harness";

suite("Control Studio actual Test identification workflow", () => {
  test("fits distinct actual Test records and requires explicit model acceptance", async function () {
    if (process.env.TRUST_CONTROL_STUDIO_ACTUAL_TEST_IDENTIFY_ACCEPT !== "1") this.skip();
    this.timeout(120_000);
    await vscode.extensions.getExtension("trust-platform.trust-lsp")?.activate();
    const project = vscode.workspace.workspaceFolders![0].uri.fsPath;
    const loop = JSON.parse(fs.readFileSync(path.join(project, "control-loop.json"), "utf8"));
    const port = await devToolsPort(requiredEnvironment("TRUST_PLANT_RENDERED_USER_DATA_DIR"), 15_000);
    const connection = await connectCdp((await httpJson(port, "/json/version")).webSocketDebuggerUrl);
    try {
      const start = await runtimeLifecycleService.startLocalSimulator();
      assert.strictEqual(start.ok, true, JSON.stringify(start));
      await vscode.commands.executeCommand("trust-lsp.controlStudio.open", { schemaVersion: 1, source: "ConfiguredLoop",
        scope: { projectId: loop.projectId, runtimeId: loop.runtimeId }, loopDefinition: loop });
      const session = await attachControlStudio(connection, port);
      const inner = (body: string) => evaluateInWebview(connection, session, body);
      const captures = process.env.TRUST_CONTROL_STUDIO_RUNTIME_SCREENSHOT_DIR;
      await inner("d.defaultView.__terminalTests=[];d.defaultView.addEventListener('message',e=>{if(e.data?.kind==='control-studio.experiment.result'&&e.data?.phase==='terminal'&&e.data?.payload?.identification)d.defaultView.__terminalTests.push(e.data.payload.identification);});return 'listening';");
      await pollValue("advanced handlers ready", () => inner("return d.querySelector('[data-finite-search]')?.dataset.ready??'';"), value => value === "true", 10000);
      await inner("const tools=d.querySelector('[data-engineer-tools]');const parents=[];for(let p=tools.parentElement;p;p=p.parentElement)if(p.tagName==='DETAILS')parents.push(p);for(const p of parents.reverse())if(!p.open)p.querySelector(':scope > summary').click();tools.click();return 'advanced controls';");
      const stage = async (name: string) => inner(`Array.from(d.querySelectorAll('[data-control-studio-mode]')).find(b=>b.textContent.trim()===${JSON.stringify(name)}).click();return 'stage';`);
      await stage("Test");
      await pollValue("fresh Test controls", async () => {
        await runtimeLifecycleService.requestIoState();
        return inner("return d.querySelector('[data-control-studio-quality]').textContent;");
      }, value => !/Unavailable|Waiting/i.test(value), 15_000);
      const originalDuration = await inner("return d.querySelector('#test-duration').value;");
      await inner("const duration=d.querySelector('#test-duration');duration.value='0';duration.dispatchEvent(new Event('input',{bubbles:true}));d.querySelector('[data-test-review]').click();return 'invalid duration';");
      await pollValue("invalid Test duration", () => inner("return d.querySelector('[data-test-state]').textContent;"), value => /duration|positive|short|bounded/i.test(value), 10_000);
      assert.strictEqual(await inner("return String(d.querySelector('[data-test-approve]').hidden);"), "true", "invalid duration cannot produce an approval plan");
      await inner(`const duration=d.querySelector('#test-duration');duration.value=${JSON.stringify(originalDuration)};duration.dispatchEvent(new Event('input',{bubbles:true}));return 'restore duration';`);
      const completeAndFit = async (role: "estimation" | "validation") => {
        await stage("Test");
        await pollValue(`fresh ${role} Test signals`, async () => {
          await runtimeLifecycleService.requestIoState();
          return inner("return d.querySelector('[data-control-studio-quality]').textContent;");
        }, value => !/Unavailable|Waiting/i.test(value), 15_000);
        await inner("d.querySelector('[data-test-review]').click();return 'review';");
        const reviewed = JSON.parse(await pollValue(`${role} reviewed plan`, () => inner(
          "var root=d.querySelector('[data-control-studio]');var state=d.querySelector('[data-test-state]');return JSON.stringify({id:root?.dataset.experimentPlanId||'',state:state?.textContent||''});"
        ), value => {
          try { return JSON.parse(value).id.length > 0; } catch { return false; }
        }, 10_000));
        const planId = reviewed.id as string;
        if (captures && role === "estimation") {
          fs.mkdirSync(captures, { recursive: true });
          await stage("Test");
          assert.strictEqual(await inner("return String(d.querySelector('[data-workflow-pane=Test]').hidden);"), "false",
            "the reviewed automatic plan stays visible in its Test workspace");
          await inner("d.querySelector('[data-test-state]').scrollIntoView({block:'center'});return 'visible';");
          await new Promise(resolve => setTimeout(resolve, 250));
          captureRenderedWorkbench(path.join(captures, "automatic-test-plan.png"));
        }
        await inner("d.querySelector('[data-test-approve]').click();return 'start';");
        await pollValue(`${role} Test running`, () => inner("return d.querySelector('[data-test-state]').textContent;"), value => value.includes("Test running"), 10_000);
        await pollValue(`${role} Test completion`, () => inner("return d.querySelector('[data-test-state]').textContent;"), value => /Completed/i.test(value), 20_000);
        await stage("Identify");
        await pollValue(`${role} Identify ready`, () => inner("return String(d.querySelector('[data-identify-run]').disabled);"), value => value === "false", 10_000);
        await inner("d.querySelector('[data-identify-run]').click();return 'qualify';");
        const qualified = await pollValue(`${role} qualification`, () => inner("return d.querySelector('[data-identify-state]').textContent;"),
          value => value.includes(role === "estimation" ? "Estimation recording qualified" : "Validation recording qualified"), 10_000);
        assert.match(qualified, role === "estimation" ? /Fit a preview/ : /retained estimation run/);
        await inner("const details=d.querySelector('[data-identify-details]');if(details.open)details.querySelector(':scope > summary').click();details.querySelector(':scope > summary').click();return 'qualification details';");
        assert.strictEqual(await inner("return String(d.querySelector('[data-identify-details]').open);"), "true", "qualification details summary opens its evidence");
        assert.ok((await inner("return d.querySelector('[data-identify-detail-rows]').textContent;" )).length > 0, "qualification details contain evidence rows");
        if (role === "estimation") assert.strictEqual(await inner("return String(d.querySelector('[data-identification-fit]').hidden);"), "true", "fit section remains unavailable before Fit");
        if (role === "validation") {
          const terminalRecords = JSON.parse(await inner("return JSON.stringify(d.defaultView.__terminalTests);"));
          assert.strictEqual(terminalRecords.length, 2, "both completed Test terminal records reach Identify");
          assert.notStrictEqual(terminalRecords[0].experimentId, terminalRecords[1].experimentId,
            "separate Test executions must retain distinct immutable recording identities");
        }
        await inner("d.querySelector('[data-identify-fit]').click();return 'fit';");
        const fit = await pollValue(`${role} fit`, () => inner("return d.querySelector('[data-identify-state]').textContent;"),
          value => role === "estimation" ? value.includes("Preview only") : value.includes("Review the evidence before acceptance"), 15_000);
        await inner("const details=d.querySelector('[data-identify-model-details]');if(details.open)details.querySelector(':scope > summary').click();details.querySelector(':scope > summary').click();return 'model details';");
        assert.strictEqual(await inner("return String(d.querySelector('[data-identify-model-details]').open);"), "true", "model details summary opens its evidence");
        assert.ok((await inner("return d.querySelector('[data-identify-model-rows]').textContent;" )).length > 0, "model details contain evidence rows");
        return { planId, fit };
      };

      const estimation = await completeAndFit("estimation");
      assert.match(estimation.fit, /Preview only/);
      const validation = await completeAndFit("validation");
      assert.notStrictEqual(validation.planId, estimation.planId, "independent validation requires a distinct actual Test plan");
      assert.match(validation.fit, /Review the evidence before acceptance/);
      assert.strictEqual(await inner("return String(d.querySelector('[data-identify-accept]').hidden);"), "false",
        "a qualified model remains unapplied until the engineer explicitly accepts it");
      await inner("d.querySelector('[data-identify-accept]').click();return 'accept';");
      const accepted = await pollValue("accepted identified model", () => inner("return d.querySelector('[data-identify-state]').textContent;"),
        value => value.includes("Accepted for design"), 10_000);
      assert.match(accepted, /normalized validation error/);
      await pollValue("guided model handoff", () => inner("return String(!d.querySelector('[data-engineer-use-model]').hidden&&!d.querySelector('[data-engineer-tune]').disabled);"), value => value === "true", 10000);
      assert.strictEqual(await inner("return [...d.querySelectorAll('[data-control-studio-mode]')].find(b=>b.textContent.trim()==='Identify')?.dataset.status||'';"), "complete",
        "explicit acceptance is the only transition that completes Identify");
      if (captures) {
        fs.mkdirSync(captures, { recursive: true });
        await inner("d.querySelector('[data-identify-state]').scrollIntoView({block:'center'});return 'visible';");
        await new Promise(resolve => setTimeout(resolve, 250));
        captureRenderedWorkbench(path.join(captures, "identified-model-accepted.png"));
      }
    } finally {
      connection.close();
      await runtimeLifecycleService.stopRuntime();
    }
  });
});
