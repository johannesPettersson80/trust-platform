// CDP: managed-runtime lifecycle in the Canvas — inspector -> Start -> Running -> Logs -> Stop.
// Self-contained: creates its own fleet root + managed cell. Covers DC-10 and RUN-09/10/11/12.
const path=require("path"),fs=require("fs"),cp=require("child_process");
const repo="/home/johannes/projects/trust-platform", ext=path.join(repo,"editors/vscode");
const pngHygienePath=path.join(__dirname,"png-hygiene.js");
const { runTests }=require(path.join(ext,"node_modules/@vscode/test-electron"));
const PORT=9374;
const evidenceRoot=process.env.TRUST_UX_EVIDENCE_ROOT||path.resolve(__dirname,"..");
const screenshotsDir=process.env.TRUST_UX_SCREENSHOTS_DIR||path.join(evidenceRoot,"screenshots-raw");
const jsonDir=process.env.TRUST_UX_JSON_DIR||path.join(evidenceRoot,"json");
const base=path.join(evidenceRoot,"runner-output","cdp_dc_managed");
const outDir=path.join(base,"out"), testsDir=path.join(base,"tests"), fleet=path.join(base,"fleet-root");
fs.rmSync(base,{recursive:true,force:true}); fs.mkdirSync(outDir,{recursive:true}); fs.mkdirSync(testsDir,{recursive:true});
fs.cpSync(path.join(repo,"examples/network_canvas_demo"),fleet,{recursive:true});
fs.mkdirSync(screenshotsDir,{recursive:true}); fs.mkdirSync(jsonDir,{recursive:true});
// create a managed runtime cell in the fleet root
const rt=path.join(repo,"target/debug/trust-runtime");
try{ cp.execFileSync(rt,["fleet","runtime","add","--fleet-root",fleet,"--name","cell1","--template","simulate","--json"],{encoding:"utf8"}); }
catch(e){ fs.writeFileSync(path.join(outDir,"fleet-add.err.txt"),String(e&&e.message||e)); }
fs.mkdirSync(path.join(outDir,"ud","User"),{recursive:true});
fs.writeFileSync(path.join(outDir,"ud","User","settings.json"),JSON.stringify({"window.titleBarStyle":"native","window.commandCenter":false,"chat.commandCenter.enabled":false,"workbench.layoutControl.enabled":false,"workbench.startupEditor":"none","workbench.tips.enabled":false,"telemetry.telemetryLevel":"off","update.mode":"none","git.enabled":false,"git.openRepositoryInParentFolders":"never","workbench.colorTheme":"Default Dark Modern"}));
const codeDir=fs.readdirSync(path.join(ext,".vscode-test")).filter(d=>d.startsWith("vscode-linux-arm64-")).sort().pop();
const codeBin=path.join(ext,".vscode-test",codeDir,"code");
fs.writeFileSync(path.join(testsDir,"index.js"),`
const path=require("path"),fs=require("fs"),http=require("http"),cp=require("child_process"),vscode=require("vscode");
const WebSocket=require(${JSON.stringify(path.join(ext,"node_modules/ws"))});
const pngHygiene=require(${JSON.stringify(pngHygienePath)});
const outDir=${JSON.stringify(outDir)}; const PORT=${PORT};
const screenshotsDir=${JSON.stringify(screenshotsDir)};
const jsonDir=${JSON.stringify(jsonDir)};
function sleep(ms){return new Promise(r=>setTimeout(r,ms));}
async function cmd(id,...a){try{return await vscode.commands.executeCommand(id,...a);}catch(e){return undefined;}}
function httpJson(p){return new Promise((res,rej)=>{const rq=http.get("http://localhost:"+PORT+p,r=>{let b="";r.on("data",c=>b+=c);r.on("end",()=>{try{res(JSON.parse(b));}catch(e){rej(e);}});});rq.on("error",rej);rq.setTimeout(5000,()=>rq.destroy(new Error("t")));});}
function shot(name,accepted){const raw=path.join(outDir,name+".raw.png"),dest=path.join(outDir,name+".png");
  const env=Object.assign({},process.env,{PATH:"/usr/bin:/bin:"+(process.env.PATH||"")});
  try{cp.execFileSync("/usr/bin/import",["-window","root",raw],{stdio:"ignore",env});
    pngHygiene.stripPngFile(raw);
    try{cp.execFileSync("/usr/bin/convert",[raw,"-strip","-bordercolor","black","-border","1","-trim","+repage",dest],{stdio:"ignore",env});}catch(e){fs.copyFileSync(raw,dest);}
    pngHygiene.stripPngFile(dest);
    if(accepted){fs.copyFileSync(dest,path.join(screenshotsDir,accepted+".png"));}
  }catch(e){fs.writeFileSync(path.join(outDir,name+".err.txt"),String(e&&e.message||e));}}
function cdp(wsUrl){return new Promise((resolve,reject)=>{const ws=new WebSocket(wsUrl);let id=0;const pending=new Map();
  ws.on("message",d=>{const m=JSON.parse(d.toString());if(m.id&&pending.has(m.id)){pending.get(m.id)(m);pending.delete(m.id);}});
  ws.on("error",reject);
  ws.on("open",()=>resolve({send:(method,params,sessionId)=>new Promise(r=>{const i=++id;let done=false;const fin=v=>{if(!done){done=true;r(v);}};pending.set(i,fin);setTimeout(()=>fin({__timeout:method}),8000);ws.send(JSON.stringify({id:i,method,params:params||{},sessionId}));}),close:()=>ws.close()}));
});}
suite("dc-managed",function(){this.timeout(280000);test("run",async function(){
  const log={steps:[]};
  const x=vscode.extensions.getExtension("trust-platform.trust-lsp"); if(x) await x.activate(); await sleep(2500);
  await cmd("workbench.action.closeAuxiliaryBar"); await cmd("workbench.action.closePanel");
  await cmd("trust-lsp.networkCanvas.open"); await sleep(8000);
  const ver=await httpJson("/json/version"); const targets=await httpJson("/json");
  const page=targets.find(t=>t.type==="page");
  const webview=targets.find(t=>t.type==="iframe"&&/index\\.html/.test(t.url||""));
  const conn=await cdp(ver.webSocketDebuggerUrl);
  const win=await conn.send("Browser.getWindowForTarget",{targetId:page.id});
  if(win.result&&win.result.windowId){await conn.send("Browser.setWindowBounds",{windowId:win.result.windowId,bounds:{left:0,top:0,width:1920,height:1080,windowState:"normal"}});}
  await sleep(1500);
  const pageAt=await conn.send("Target.attachToTarget",{targetId:page.id,flatten:true});
  const pageSid=pageAt.result&&pageAt.result.sessionId; await conn.send("Runtime.enable",{},pageSid);
  const at=await conn.send("Target.attachToTarget",{targetId:webview.id,flatten:true});
  const sid=at.result&&at.result.sessionId; await conn.send("Runtime.enable",{},sid);
  async function evalPage(body){const expr="(function(){try{"+body+"}catch(e){return 'ERR:'+e.message;}})()";const ev=await conn.send("Runtime.evaluate",{expression:expr,returnByValue:true},pageSid);return ev&&ev.result&&ev.result.result&&ev.result.result.value;}
  async function evalInner(body){const expr="(function(){try{var f=document.querySelector('iframe');var d=f&&f.contentDocument;if(!d)return 'NO_INNER_DOC';"+body+"}catch(e){return 'ERR:'+e.message;}})()";const ev=await conn.send("Runtime.evaluate",{expression:expr,returnByValue:true},sid);return ev&&ev.result&&ev.result.result&&ev.result.result.value;}
  async function clickExact(sub){return await evalInner("var s="+JSON.stringify(sub.toLowerCase())+";var b=[...d.querySelectorAll('button,[role=button]')].find(function(x){return ((x.textContent||x.getAttribute('aria-label')||'').trim().toLowerCase())===s;});if(!b)return 'NOT_FOUND_EXACT:'+s;b.click();return 'CLICKED:'+(b.textContent||'').trim().slice(0,26);");}
  async function clickText(sub){return await evalInner("var s="+JSON.stringify(sub.toLowerCase())+";var b=[...d.querySelectorAll('button,[role=button]')].find(function(x){return ((x.textContent||x.getAttribute('aria-label')||'').toLowerCase()).indexOf(s)>=0;});if(!b)return 'NOT_FOUND:'+s;b.click();return 'CLICKED:'+(b.textContent||'').trim().slice(0,26);");}
  async function clickNode(sub){return await evalInner("var s="+JSON.stringify(sub.toLowerCase())+";var w=f.contentWindow;var node=[...d.querySelectorAll('.react-flow__node')].find(function(n){return ((n.textContent||'').toLowerCase()).indexOf(s)>=0;});if(!node)return 'NODE_NOT_FOUND:'+s;var r=node.getBoundingClientRect();var cx=r.left+r.width/2,cy=r.top+18;['pointerdown','pointerup'].forEach(function(t){try{node.dispatchEvent(new w.PointerEvent(t,{bubbles:true,cancelable:true,clientX:cx,clientY:cy,pointerId:1,button:0,isPrimary:true}));}catch(e){}});try{node.dispatchEvent(new w.MouseEvent('click',{bubbles:true,cancelable:true,clientX:cx,clientY:cy,button:0}));}catch(e){}return 'NODE_CLICKED';");}
  log.canvas=await evalInner("return (d.body&&d.body.innerText||'').slice(0,160);");
  log.steps.push({s:"node-cell1",v:await clickNode("cell1")}); await sleep(2000);
  log.inspector=await evalInner("var bs=[...d.querySelectorAll('button')].map(function(b){return (b.textContent||'').trim();}).filter(Boolean);return JSON.stringify([...new Set(bs)].slice(0,24));");
  shot("DC-10-managed-runtime-node","DC-10-managed-runtime-node");
  log.steps.push({s:"set-run-target",v:await clickText("set as run target")}); await sleep(1200);
  shot("DC-08-set-as-run-target","DC-08-set-as-run-target");
  log.steps.push({s:"start",v:await clickExact("start")});
  let running=false; for(let i=0;i<24;i++){ await sleep(1000); const t=(await evalInner("return (d.body&&d.body.innerText||'').toLowerCase();"))||""; if(/running|connected/.test(t)){running=true;break;} }
  log.running=running; await sleep(1500); await cmd("notifications.clearAll"); await sleep(1000);
  await clickText("fit view"); await sleep(1100);            // FIX 1: re-fit so nodes are visible after the attach re-layout
  log.steps.push({s:"node-cell1-running",v:await clickNode("cell1")}); await sleep(1600);
  log.steps.push({s:"focus-running-node",v:await clickExact("focus")}); await sleep(1400);
  log.runningInspector=await evalInner("var bs=[...d.querySelectorAll('button')].map(function(b){return (b.textContent||'').trim();}).filter(Boolean);return JSON.stringify([...new Set(bs)].slice(0,24));");
  shot("RUN-10-managed-running-connected","RUN-10-managed-running-connected");
  // FIX 2: Logs -> focus the OUTPUT view (truST Runtime channel), not the Debug Console, before maximizing.
  log.steps.push({s:"logs",v:await clickText("logs")}); await sleep(2500);
  await cmd("workbench.panel.output.focus"); await sleep(1800);
  await cmd("workbench.action.toggleMaximizedPanel"); await sleep(1500);
  shot("RUN-12-managed-logs","RUN-12-managed-logs");
  await cmd("workbench.action.toggleMaximizedPanel"); await sleep(700); await cmd("workbench.action.closePanel"); await sleep(900);
  // FIX 3: Stop the managed cell1. Try the current inspector first; if its primary isn't Stop (panel ops
  // can drop the selection / surface the online twin's Disconnect), re-select the cell1 node and retry.
  function btns(){return "var bs=[...d.querySelectorAll('button')].map(function(b){return (b.textContent||'').trim();}).filter(Boolean);return JSON.stringify([...new Set(bs)].slice(0,24));";}
  log.afterNodes=await evalInner("return JSON.stringify([...d.querySelectorAll('.react-flow__node')].map(function(n){return (n.textContent||'').trim().slice(0,38);}));");
  log.stopBtns1=await evalInner(btns());
  let sv=await clickExact("stop");
  if(/NOT_FOUND/.test(sv)){ await clickNode("cell1"); await sleep(1600); log.stopBtns2=await evalInner(btns()); sv=await clickExact("stop"); if(/NOT_FOUND/.test(sv)) sv=await clickText("stop"); }
  log.steps.push({s:"stop",v:sv});
  let stopped=false; let statusStopped=false; let statusText="";
  for(let i=0;i<18;i++){
    await sleep(1000);
    const t=(await evalInner("return (d.body&&d.body.innerText||'').toLowerCase();"))||"";
    statusText=String(await evalPage("return (document.body&&document.body.innerText||'').replace(/\\\\s+/g,' ').trim();")||"");
    const lowerStatus=statusText.toLowerCase();
    stopped=/stopped/.test(t)&&!/connected|running/.test(t);
    statusStopped=/trust:\\s*cell1\\s+stopped/i.test(statusText)||(/cell1\\s+stopped/i.test(statusText)&&!/trust:\\s*cell1\\s+running/i.test(statusText));
    if(stopped&&statusStopped){break;}
  }
  log.stopped=stopped; await cmd("notifications.clearAll"); await sleep(1200);
  log.statusBarAfterStop=statusText;
  log.statusBarStopped=statusStopped;
  shot("RUN-11-managed-stopped","RUN-11-managed-stopped");
  const failed=log.steps.filter(function(step){
    const value=String(step.v||"");
    if(step.s==="focus-running-node" && value.indexOf("NOT_FOUND")>=0){
      return false;
    }
    return value.indexOf("NOT_FOUND")>=0 || value.indexOf("NODE_NOT_FOUND")>=0;
  });
  fs.writeFileSync(path.join(outDir,"diag.json"),JSON.stringify(log,null,2));
  fs.writeFileSync(path.join(jsonDir,"managed-runtime-proof.json"),JSON.stringify(log,null,2));
  if(failed.length){throw new Error("managed runtime workflow did not find required UI action: "+JSON.stringify(failed));}
  if(!/⋯|More actions/i.test(String(log.inspector||"")) || !/⋯|More actions/i.test(String(log.runningInspector||""))){
    throw new Error("managed runtime inspector did not expose More actions overflow: "+JSON.stringify({inspector:log.inspector,runningInspector:log.runningInspector}));
  }
  if(/Focus/i.test(String(log.inspector||"")) || /Focus/i.test(String(log.runningInspector||""))){
    throw new Error("managed runtime inspector still exposes Focus as a top-level action: "+JSON.stringify({inspector:log.inspector,runningInspector:log.runningInspector}));
  }
  if(!running){throw new Error("managed runtime did not reach running/connected state");}
  if(!stopped){throw new Error("managed runtime did not reach stopped state");}
  if(!statusStopped){throw new Error("managed Stop left status bar inconsistent: "+statusText.slice(0,500));}
  conn.close();
});});`);
fs.writeFileSync(path.join(testsDir,"run.js"),`const Mocha=require(${JSON.stringify(path.join(ext,"node_modules/mocha"))});const path=require("path");exports.run=function(){const m=new Mocha({ui:"tdd",timeout:280000});m.addFile(path.join(__dirname,"index.js"));return new Promise((res,rej)=>m.run(f=>f?rej(new Error(f+" fail")):res()));};`);
async function main(){
  await runTests({ vscodeExecutablePath:codeBin, extensionDevelopmentPath:ext, extensionTestsPath:path.join(testsDir,"run.js"),
    launchArgs:[fleet,"--remote-debugging-port="+PORT,"--ozone-platform=x11","--disable-gpu","--use-gl=angle","--use-angle=swiftshader","--in-process-gpu","--no-sandbox","--user-data-dir="+path.join(outDir,"ud"),"--extensions-dir="+path.join(outDir,"ed"),"--disable-workspace-trust","--skip-welcome"],
    extensionTestsEnv:{ ST_LSP_TEST_SERVER:path.join(repo,"target/debug/trust-lsp"), ST_RUNTIME_TEST_BIN:path.join(repo,"target/debug/trust-runtime") } });
  console.log("DC_MANAGED_DONE");
}
main().catch(e=>{console.error(e);process.exit(1);});
