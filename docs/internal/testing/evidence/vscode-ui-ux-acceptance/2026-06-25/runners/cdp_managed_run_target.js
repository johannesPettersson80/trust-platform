// RUN-09: managed runtime selected from Devices & Connections appears in the Run card.
const path = require("path");
const fs = require("fs");
const cp = require("child_process");
const repo = "/home/johannes/projects/trust-platform";
const ext = path.join(repo, "editors/vscode");
const pngHygienePath = path.join(__dirname, "png-hygiene.js");
const { runTests } = require(path.join(ext, "node_modules/@vscode/test-electron"));
const PORT = 9375;
const evidenceRoot = process.env.TRUST_UX_EVIDENCE_ROOT || path.resolve(__dirname, "..");
const screenshotsDir =
  process.env.TRUST_UX_SCREENSHOTS_DIR || path.join(evidenceRoot, "screenshots-raw");
const jsonDir = process.env.TRUST_UX_JSON_DIR || path.join(evidenceRoot, "json");
const base = path.join(evidenceRoot, "runner-output", "cdp_managed_run_target");
const outDir = path.join(base, "out");
const testsDir = path.join(base, "tests");
const fleet = path.join(base, "fleet-root");

fs.rmSync(base, { recursive: true, force: true });
fs.mkdirSync(outDir, { recursive: true });
fs.mkdirSync(testsDir, { recursive: true });
fs.cpSync(path.join(repo, "examples/network_canvas_demo"), fleet, { recursive: true });
fs.mkdirSync(screenshotsDir, { recursive: true });
fs.mkdirSync(jsonDir, { recursive: true });

cp.execFileSync(
  path.join(repo, "target/debug/trust-runtime"),
  ["fleet", "runtime", "add", "--fleet-root", fleet, "--name", "cell1", "--template", "simulate", "--json"],
  { encoding: "utf8" }
);

fs.mkdirSync(path.join(outDir, "ud", "User"), { recursive: true });
fs.writeFileSync(
  path.join(outDir, "ud", "User", "settings.json"),
  JSON.stringify({
    "window.titleBarStyle": "native",
    "window.commandCenter": false,
    "chat.commandCenter.enabled": false,
    "workbench.layoutControl.enabled": false,
    "workbench.startupEditor": "none",
    "workbench.tips.enabled": false,
    "telemetry.telemetryLevel": "off",
    "update.mode": "none",
    "git.enabled": false,
    "git.openRepositoryInParentFolders": "never",
    "workbench.colorTheme": "Default Dark Modern",
  })
);

const codeDir = fs
  .readdirSync(path.join(ext, ".vscode-test"))
  .find((entry) => entry.startsWith("vscode-linux-arm64-"));
const codeBin = path.join(ext, ".vscode-test", codeDir, "code");

fs.writeFileSync(
  path.join(testsDir, "index.js"),
  `
const path=require("path"),fs=require("fs"),http=require("http"),cp=require("child_process"),vscode=require("vscode");
const WebSocket=require(${JSON.stringify(path.join(ext, "node_modules/ws"))});
const pngHygiene=require(${JSON.stringify(pngHygienePath)});
const outDir=${JSON.stringify(outDir)};
const screenshotsDir=${JSON.stringify(screenshotsDir)};
const jsonDir=${JSON.stringify(jsonDir)};
const PORT=${PORT};
function sleep(ms){return new Promise(r=>setTimeout(r,ms));}
async function cmd(id,...a){try{return await vscode.commands.executeCommand(id,...a);}catch(e){return undefined;}}
	function shot(name,accepted){const raw=path.join(outDir,name+".raw.png"),dest=path.join(outDir,name+".png");const env=Object.assign({},process.env,{PATH:"/usr/bin:/bin:"+(process.env.PATH||"")});cp.execFileSync("/usr/bin/import",["-window","root",raw],{stdio:"ignore",env});pngHygiene.stripPngFile(raw);try{cp.execFileSync("/usr/bin/convert",[raw,"-strip","-bordercolor","black","-border","1","-trim","+repage",dest],{stdio:"ignore",env});}catch(e){fs.copyFileSync(raw,dest);}pngHygiene.stripPngFile(dest);if(accepted){pngHygiene.copyPngStripped(dest,path.join(screenshotsDir,accepted+".png"));}}
function httpJson(p){return new Promise((res,rej)=>{const rq=http.get("http://localhost:"+PORT+p,r=>{let b="";r.on("data",c=>b+=c);r.on("end",()=>{try{res(JSON.parse(b));}catch(e){rej(e);}});});rq.on("error",rej);rq.setTimeout(5000,()=>rq.destroy(new Error("timeout")));});}
function cdp(wsUrl){return new Promise((resolve,reject)=>{const ws=new WebSocket(wsUrl);let id=0;const pending=new Map();ws.on("message",d=>{const m=JSON.parse(d.toString());if(m.id&&pending.has(m.id)){pending.get(m.id)(m);pending.delete(m.id);}});ws.on("error",reject);ws.on("open",()=>resolve({send:(method,params,sessionId)=>new Promise(r=>{const i=++id;let done=false;const fin=v=>{if(!done){done=true;r(v);}};pending.set(i,fin);setTimeout(()=>fin({__timeout:method}),8000);ws.send(JSON.stringify({id:i,method,params:params||{},sessionId}));}),close:()=>ws.close()}));});}
suite("managed-run-target",function(){this.timeout(140000);test("select managed target",async function(){
  const log={steps:[]};
  const ext=vscode.extensions.getExtension("trust-platform.trust-lsp");if(ext)await ext.activate();await sleep(2500);
  await cmd("workbench.action.closeAuxiliaryBar");await cmd("workbench.action.closePanel");
  await cmd("trust-lsp.networkCanvas.open");await sleep(8000);
  const ver=await httpJson("/json/version");
  let targets=[];let page;let webview;
  for(let i=0;i<20;i+=1){
    targets=await httpJson("/json");
    page=targets.find(t=>t.type==="page");
    webview=targets.find(t=>t.type==="iframe"&&/index\\.html/.test(t.url||""));
    if(page&&webview)break;
    await sleep(500);
  }
  if(!page||!webview){fs.writeFileSync(path.join(outDir,"targets.json"),JSON.stringify(targets,null,2));throw new Error("VS Code webview target not found");}
  const conn=await cdp(ver.webSocketDebuggerUrl);
  const win=await conn.send("Browser.getWindowForTarget",{targetId:page.id});
  if(win.result&&win.result.windowId){await conn.send("Browser.setWindowBounds",{windowId:win.result.windowId,bounds:{left:0,top:0,width:1440,height:900,windowState:"normal"}});}
  const at=await conn.send("Target.attachToTarget",{targetId:webview.id,flatten:true});
  const sid=at.result&&at.result.sessionId;await conn.send("Runtime.enable",{},sid);
  async function evalInner(body){const expr="(function(){try{var f=document.querySelector('iframe');var d=f&&f.contentDocument;if(!d)return 'NO_INNER_DOC';"+body+"}catch(e){return 'ERR:'+e.message;}})()";const ev=await conn.send("Runtime.evaluate",{expression:expr,returnByValue:true},sid);return ev&&ev.result&&ev.result.result&&ev.result.result.value;}
  async function clickNode(sub){return await evalInner("var s="+JSON.stringify(sub.toLowerCase())+";var w=f.contentWindow;var node=[...d.querySelectorAll('.react-flow__node')].find(function(n){return ((n.textContent||'').toLowerCase()).indexOf(s)>=0;});if(!node)return 'NODE_NOT_FOUND:'+s;var r=node.getBoundingClientRect();var cx=r.left+r.width/2,cy=r.top+18;['pointerdown','pointerup'].forEach(function(t){try{node.dispatchEvent(new w.PointerEvent(t,{bubbles:true,cancelable:true,clientX:cx,clientY:cy,pointerId:1,button:0,isPrimary:true}));}catch(e){}});try{node.dispatchEvent(new w.MouseEvent('click',{bubbles:true,cancelable:true,clientX:cx,clientY:cy,button:0}));}catch(e){}return 'NODE_CLICKED';");}
  async function clickText(sub){return await evalInner("var s="+JSON.stringify(sub.toLowerCase())+";var b=[...d.querySelectorAll('button,[role=button]')].find(function(x){return ((x.textContent||x.getAttribute('aria-label')||'').toLowerCase()).indexOf(s)>=0;});if(!b)return 'NOT_FOUND:'+s;b.click();return 'CLICKED:'+(b.textContent||'').trim().slice(0,40);");}
  log.steps.push({s:"node-cell1",v:await clickNode("cell1")});await sleep(1600);
  log.steps.push({s:"set-run-target",v:await clickText("set as run target")});await sleep(1600);
  await cmd("trust.home.focus");await sleep(2000);
  shot("RUN-09-managed-run-target","RUN-09-managed-run-target");
  log.runCardText=await vscode.env.clipboard.readText().catch(()=>undefined);
  conn.close();
  const failed=log.steps.filter(step=>String(step.v||"").includes("NOT_FOUND")||String(step.v||"").includes("NODE_NOT_FOUND"));
  fs.writeFileSync(path.join(jsonDir,"RUN-09-managed-run-target-proof.json"),JSON.stringify(log,null,2));
  if(failed.length){throw new Error("managed target selection failed: "+JSON.stringify(failed));}
});});`
);
fs.writeFileSync(
  path.join(testsDir, "run.js"),
  `const Mocha=require(${JSON.stringify(path.join(ext, "node_modules/mocha"))});const path=require("path");exports.run=function(){const m=new Mocha({ui:"tdd",timeout:140000});m.addFile(path.join(__dirname,"index.js"));return new Promise((res,rej)=>m.run(f=>f?rej(new Error(f+" fail")):res()));};`
);

async function main() {
  await runTests({
    vscodeExecutablePath: codeBin,
    extensionDevelopmentPath: ext,
    extensionTestsPath: path.join(testsDir, "run.js"),
    launchArgs: [
      fleet,
      "--remote-debugging-port=" + PORT,
      "--ozone-platform=x11",
      "--disable-gpu",
      "--use-gl=angle",
      "--use-angle=swiftshader",
      "--in-process-gpu",
      "--no-sandbox",
      "--user-data-dir",
      path.join(outDir, "ud"),
      "--extensions-dir",
      path.join(outDir, "ed"),
      "--disable-workspace-trust",
      "--skip-welcome",
    ],
    extensionTestsEnv: {
      ST_LSP_TEST_SERVER: path.join(repo, "target/debug/trust-lsp"),
      ST_RUNTIME_TEST_BIN: path.join(repo, "target/debug/trust-runtime"),
    },
  });
  console.log("MANAGED_RUN_TARGET_DONE");
}
main().catch((error) => {
  console.error(error);
  process.exit(1);
});
