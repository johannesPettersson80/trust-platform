import * as assert from "assert";
import * as vscode from "vscode";
import * as fs from "fs";
import * as os from "os";
import * as path from "path";
import {connectCdp,devToolsPort,evaluateInWebview,httpJson,pollValue,requiredEnvironment,type CdpConnection} from "./plant-rendered-harness";
suite("Rendered state-space structure",()=>{
 test("shows computed two-tank controllability and changes observability with the sensor row",async function(){
  if(process.env.TRUST_PLANT_RENDERED_TEST!=="1")this.skip();this.timeout(60000);
  await vscode.extensions.getExtension("trust-platform.trust-lsp")?.activate();
  const request=loopRequest();request.loopDefinition.controller.type="EngineerLaw";(request.loopDefinition.controller as any).design="project";
  await vscode.commands.executeCommand("trust-lsp.controlStudio.open",request);
  const port=await devToolsPort(requiredEnvironment("TRUST_PLANT_RENDERED_USER_DATA_DIR"),15000);const connection=await connectCdp((await httpJson(port,"/json/version")).webSocketDebuggerUrl);
  const folder=fs.mkdtempSync(path.join(os.tmpdir(),"state-structure-"));const file=path.join(folder,"model.json");
  const channel=(id:string,unit:string)=>({id,unit,operatingRange:{minimum:-1,maximum:1}});
  const model={kind:"continuous-state-space",revision:"two-tank-linearization",source:"provided",validation:"unvalidated",evidence:[],delaySeconds:0,inputs:[channel("flow","m3/s")],outputs:[channel("level","m")],states:[channel("upstream","m"),channel("downstream","m")],a:[[-0.04,0],[0.04,-0.04]],b:[[2],[0]],c:[[0,1]],d:[[0]]};
  try{
   const session=await attachControlStudio(connection,port);const inner=(body:string)=>evaluateInWebview(connection,session,body);
   await pollValue("state model handlers ready",()=>inner("return d.querySelector('[data-finite-search]')?.dataset.ready??'';"),v=>v==="true",10000);
   await inner("Array.from(d.querySelectorAll('[data-control-studio-mode]')).find(b=>b.textContent.trim()==='Tune').click();return 'selected';");
   assert.strictEqual(await inner("return String(Boolean(d.querySelector('[data-general-model-structure]')));"),"true","Studio must expose structural analysis rather than compute ranks in lesson ST");
   async function upload(){await inner("d.querySelector('#general-model-file').value='';return 'cleared';");fs.writeFileSync(file,JSON.stringify(model));await connection.send("DOM.enable",{},session);await connection.send("DOM.getDocument",{depth:-1,pierce:true},session);const tree=await connection.send("DOM.getFlattenedDocument",{depth:-1,pierce:true},session);const node=tree.nodes.find((n:any)=>n.nodeName==="INPUT"&&n.attributes?.includes("general-model-file"));await connection.send("DOM.setFileInputFiles",{files:[file],backendNodeId:node.backendNodeId},session);await pollValue("imported model",()=>inner("return d.querySelector('[data-general-model-status]').textContent+' '+d.querySelector('[data-general-model-provenance]').textContent;"),s=>s.includes("two-tank-linearization"),10000);await inner("d.querySelector('[data-general-model-analyze]').click();return 'running';");}
   await upload();const full=await pollValue("computed structural matrices",()=>inner("return d.querySelector('[data-general-model-structure]').textContent;"),s=>s.includes("Observability rank 2 of 2"),10000);
   assert.match(full,/Controllability rank 2 of 2/);assert.match(full,/-0.08/);assert.match(full,/0.04/);assert.match(full,/upstream.*downstream/);
   model.c=[[1,0]];await upload();const hidden=await pollValue("changed sensor observability",()=>inner("return d.querySelector('[data-general-model-structure]').textContent;"),s=>s.includes("Observability rank 1 of 2"),10000);
   assert.match(hidden,/Controllability rank 2 of 2/);
  }finally{connection.close();fs.rmSync(folder,{recursive:true,force:true});}
 });
});
async function attachControlStudio(connection: CdpConnection, port: number, previousEditorSession = ""): Promise<string> {
  return pollValue("Control Studio webview", async () => {
    const targets = await httpJson(port, "/json");
    for (const target of targets.filter((entry: any) => /index\.html/.test(entry.url ?? ""))) {
      let sessionId: string | undefined;
      try {
        const attached = await connection.send("Target.attachToTarget", { targetId: target.id, flatten: true });
        sessionId = typeof attached.sessionId === "string" ? attached.sessionId : undefined;
        if (!sessionId)
          continue;
        await connection.send("Runtime.enable", {}, sessionId);
        const marker = await evaluateInWebview(connection, sessionId, `var root=d.querySelector('[data-control-studio]');return String(Boolean(root&&root.dataset.editorSessionId&&root.dataset.editorSessionId!==${JSON.stringify(previousEditorSession)}));`);
        if (marker === "true")
          return sessionId;
        await connection.send("Target.detachFromTarget", { sessionId });
      }
      catch {
        if (sessionId)
          await connection.send("Target.detachFromTarget", { sessionId }).catch(() => undefined);
      }
    }
    return "";
  }, (value) => value.length > 0, 20000);
}
function loopRequest() {
  const loopDefinition = {
    schemaVersion: 1,
    id: "loop:recording-gap",
    name: "20 ms recording gap",
    revision: "sha256:recording-gap-loop",
    projectId: "project:recording-gap",
    runtimeId: "simulator",
    controller: {
      instance: "P1.Controller",
      type: "Control.PID",
      revision: "sha256:recording-gap-controller",
      action: "direct",
      taskPeriodMs: 20,
      modeSignal: "%MX8.0",
    },
    signals: {
      setpoint: { symbol: "Setpoint", address: "%ML0", iecType: "LREAL", direction: "read", unit: "°C", range: { minimum: 0, maximum: 100 } },
      processVariable: { symbol: "ProcessValue", address: "%IL0", iecType: "LREAL", direction: "read", unit: "°C", range: { minimum: 0, maximum: 100 } },
      manipulatedVariable: { symbol: "ManipulatedValue", address: "%QL0", iecType: "LREAL", direction: "write", unit: "%", range: { minimum: 0, maximum: 100 } },
    },
    limits: { outputMinimum: 0, outputMaximum: 100 },
    observedSampleTiming: { nominalPeriodMs: 20 },
  };
  return {
    schemaVersion: 1,
    source: "ConfiguredLoop",
    scope: { projectId: loopDefinition.projectId, runtimeId: loopDefinition.runtimeId },
    loopDefinition,
  };
}
