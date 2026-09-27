import * as assert from "assert";
import * as fs from "fs";
import * as path from "path";
import * as vscode from "vscode";
import { runtimeLifecycleService } from "../../runtimeLifecycle";
import { attachControlStudio } from "./control-studio-rendered-harness";
import { captureRenderedWorkbench, connectCdp, devToolsPort, evaluateInWebview, httpJson, pollValue, requiredEnvironment, sleep } from "./plant-rendered-harness";
import type { ControlStudioWorkflowResult, WorkflowResultSample } from "../../controlStudio/workflowResult";

const expected = new Map([['control-lesson-13', 'process'], ['control-water-heater-energy-balance', 'energy']]);

suite("Control Studio heater model actual rendered workflow", () => {
  for (const [id, kind] of expected) test(`${id} executes its held heater case and retains independent evidence`, async function () {
    if (process.env.TRUST_CONTROL_STUDIO_HEATER_MODEL !== "1") this.skip();
    this.timeout(75_000);
    await vscode.extensions.getExtension("trust-platform.trust-lsp")?.activate();
    const project = vscode.workspace.workspaceFolders![0].uri.fsPath;
    const loop = JSON.parse(fs.readFileSync(path.join(project, "control-loop.json"), "utf8"));
    assert.strictEqual(loop.id, id);
    const authority = loop.signals.additional.find((x: any) => x.address === loop.experiment.activation.address).id;
    const port = await devToolsPort(requiredEnvironment("TRUST_PLANT_RENDERED_USER_DATA_DIR"), 15_000);
    const connection = await connectCdp((await httpJson(port, "/json/version")).webSocketDebuggerUrl);
    try {
      await runtimeLifecycleService.stopRuntime(); assert.strictEqual((await runtimeLifecycleService.startLocalSimulator()).ok, true);
      await vscode.commands.executeCommand("trust-lsp.controlStudio.open", {schemaVersion:1,source:"ConfiguredLoop",scope:{projectId:loop.projectId,runtimeId:loop.runtimeId},loopDefinition:loop});
      const session=await attachControlStudio(connection,port), inner=(body:string)=>evaluateInWebview(connection,session,body);
      const stage=async(name:string)=>inner(`var b=[...d.querySelectorAll('[data-control-studio-mode]')].find(x=>x.textContent.trim()===${JSON.stringify(name)});b.click();return b.textContent.trim();`);
      await pollValue("fresh hot state",async()=>{await runtimeLifecycleService.requestIoState();return inner("return d.querySelector('[data-control-studio-quality]').textContent;");},x=>!/Unavailable|Waiting|stale/i.test(x),15_000);
      if(kind==='process') { assert.match(await inner("return d.querySelector('[data-control-studio-pv]').textContent;"),/35/); assert.match(await inner("return d.querySelector('[data-control-studio-mv]').textContent;"),/75/); }
      await stage('Monitor'); await inner("d.querySelector('[data-control-studio-record-start]').click();return 'ok';");
      await stage('Test'); await sleep(300); await runtimeLifecycleService.requestIoState(); await inner("d.querySelector('[data-test-review]').click();return 'ok';");
      await pollValue("review",()=>inner("return d.querySelector('[data-test-state]').textContent;"),x=>x.includes('Simulation test ready'),15_000);
      assert.match(await inner("return d.querySelector('[data-test-timeout]').textContent;"),/30\.1 s maximum/);
      await inner("d.querySelector('[data-test-approve]').click();return 'ok';");
      await pollValue("complete",()=>inner("return d.querySelector('[data-test-state]').textContent;"),x=>/Completed|restor/i.test(x),45_000);
      await stage('Monitor'); await inner("d.querySelector('[data-control-studio-record-stop]').click();return 'ok';");
      await pollValue("complete recording",()=>inner("return d.querySelector('[data-control-studio-recording-evidence]').textContent;"),x=>/Complete recording|Recording retained/.test(x),15_000);
      const shots=process.env.TRUST_CONTROL_STUDIO_HEATER_MODEL_SCREENSHOT_DIR; if(shots){fs.mkdirSync(shots,{recursive:true});captureRenderedWorkbench(path.join(shots,`${id}.png`));}
      await stage('Validate'); await inner("d.defaultView.addEventListener('message',e=>{if(e.data?.kind==='control-studio.workflow-result')d.defaultView.__heater=e.data.payload.result});d.querySelector('[data-result-save]').click();return 'ok';");
      await pollValue("save",()=>inner("return d.querySelector('[data-result-state]').textContent;"),x=>x.includes('Saved revision'),15_000);
      await inner("var h=d.querySelector('[data-result-history]');h.value=[...h.options].filter(o=>o.value).sort((a,b)=>Number(b.value)-Number(a.value))[0].value;h.dispatchEvent(new Event('change'));d.querySelector('[data-result-open]').click();return 'open';");
      const result=JSON.parse(await pollValue("result",()=>inner("return d.defaultView.__heater?JSON.stringify(d.defaultView.__heater):'';"),x=>x.length>0,15_000)) as ControlStudioWorkflowResult;
      const active=result.recording.rawSamples.filter(s=>aux(s,authority)===1); assert.ok(active.length>=280);
      const held=active.filter(s=>s.appliedMv===85); assert.ok(held.length>=298&&held.length<=302,`expected 30 s held 85% segment, got ${held.length} scans`); assert.ok(held.every(s=>s.appliedMv===85));
      const firstZero=result.recording.rawSamples.findIndex(s=>s.appliedMv===0&&s.scan>active[active.length-1].scan); assert.ok(firstZero>=0,"restoration must command zero after the held window"); assert.ok(result.recording.rawSamples.slice(firstZero).every(s=>s.appliedMv===0),"terminal restoration must remain at zero without reheating");
      assert.ok(active[0].pv!>=34.9&&active[0].pv!<=35.1); assert.ok(active[active.length-1].pv!>36.8&&active[active.length-1].pv!<37.1);
      if(kind==='energy'){const model=active.map(s=>aux(s,'model')); assert.ok(model.every(x=>x!==null)); assert.ok(active.some((s,i)=>Math.abs((model[i]??0)-(s.pv??0))>1e-4),"Euler model must be recorded separately from measured PV"); assert.ok(active.every((s,i)=>Math.abs((model[i]??0)-(s.pv??0))<0.08),"Euler and measured PV must agree within declared teaching tolerance");}
    } finally {connection.close();await runtimeLifecycleService.stopRuntime();}
  });
});
function aux(s:WorkflowResultSample,id:string){return s.additional?.find(x=>x.id===id)?.value;}
