// CROSS-02: the endpoint inspector shows per-point `var ← external-ref · type · access` mappings,
// not a bare "N nodes" count. Opens the ads_line1 canvas (ads.toml has 4 [[connections.points]]),
// clicks the ADS endpoint, and reads the inspector connection summary.
const path = require("path"), fs = require("fs");
const repo = process.env.TRUST_REPO || "/home/johannes/projects/trust-platform";
const ext = path.join(repo, "editors/vscode");
const pngHygienePath = path.join(__dirname, "png-hygiene.js");
const { runTests } = require(path.join(ext, "node_modules/@vscode/test-electron"));
const PORT = 9381;
const evidenceRoot = process.env.TRUST_UX_EVIDENCE_ROOT
  ? path.resolve(process.env.TRUST_UX_EVIDENCE_ROOT)
  : path.resolve(__dirname, "..");
const screenshotsDir = path.join(evidenceRoot, "screenshots-raw");
const jsonDir = path.join(evidenceRoot, "json");
const base = path.join(evidenceRoot, "runner-output", "cross02-ads-mapping");
const outDir = path.join(base, "out"), workspace = path.join(base, "project"), testsDir = path.join(base, "tests");
fs.rmSync(base, { recursive: true, force: true });
fs.mkdirSync(outDir, { recursive: true });
fs.mkdirSync(testsDir, { recursive: true });
fs.mkdirSync(screenshotsDir, { recursive: true });
fs.mkdirSync(jsonDir, { recursive: true });
fs.cpSync(path.join(repo, "examples/communication/ads_line1"), workspace, { recursive: true });
fs.mkdirSync(path.join(outDir, "ud", "User"), { recursive: true });
fs.writeFileSync(path.join(outDir, "ud", "User", "settings.json"), JSON.stringify({
  "window.titleBarStyle": "native", "window.commandCenter": false, "chat.commandCenter.enabled": false,
  "workbench.layoutControl.enabled": false, "workbench.startupEditor": "none", "workbench.tips.enabled": false,
  "telemetry.telemetryLevel": "off", "update.mode": "none", "workbench.colorTheme": "Default Dark Modern"
}));
const codeDir = fs.readdirSync(path.join(ext, ".vscode-test")).filter(d => d.startsWith("vscode-linux-")).sort().pop();
const codeBin = path.join(ext, ".vscode-test", codeDir, "code");
fs.writeFileSync(path.join(testsDir, "index.js"), `
const path=require("path"),fs=require("fs"),http=require("http"),cp=require("child_process"),vscode=require("vscode");
const WebSocket=require(${JSON.stringify(path.join(ext, "node_modules/ws"))});
const pngHygiene=require(${JSON.stringify(pngHygienePath)});
const outDir=${JSON.stringify(outDir)}; const screenshotsDir=${JSON.stringify(screenshotsDir)}; const jsonDir=${JSON.stringify(jsonDir)}; const PORT=${PORT};
function sleep(ms){return new Promise(r=>setTimeout(r,ms));}
async function cmd(id,...a){try{return await vscode.commands.executeCommand(id,...a);}catch(e){return undefined;}}
function httpJson(p){return new Promise((res,rej)=>{const rq=http.get("http://localhost:"+PORT+p,r=>{let b="";r.on("data",c=>b+=c);r.on("end",()=>{try{res(JSON.parse(b));}catch(e){rej(e);}});});rq.on("error",rej);rq.setTimeout(5000,()=>rq.destroy(new Error("t")));});}
function shot(name){const raw=path.join(outDir,name+".raw.png"),dest=path.join(outDir,name+".png");
  const env=Object.assign({},process.env,{PATH:"/usr/bin:/bin:"+(process.env.PATH||"")});
  cp.execFileSync("/usr/bin/import",["-window","root",raw],{stdio:"ignore",env});
  pngHygiene.stripPngFile(raw);
  try{cp.execFileSync("/usr/bin/convert",[raw,"-strip","-bordercolor","black","-border","1","-trim","+repage",dest],{stdio:"ignore",env});}catch(e){fs.copyFileSync(raw,dest);}
  pngHygiene.stripPngFile(dest);
  fs.copyFileSync(dest,path.join(screenshotsDir,name+".png"));
  return "screenshots-raw/"+name+".png";}
function cdp(wsUrl){return new Promise((resolve,reject)=>{const ws=new WebSocket(wsUrl);let id=0;const pending=new Map();
  ws.on("message",d=>{const m=JSON.parse(d.toString());if(m.id&&pending.has(m.id)){pending.get(m.id)(m);pending.delete(m.id);}});
  ws.on("error",reject);
  ws.on("open",()=>resolve({send:(method,params,sessionId)=>new Promise(r=>{const i=++id;let done=false;const fin=v=>{if(!done){done=true;r(v);}};pending.set(i,fin);setTimeout(()=>fin({__timeout:method}),8000);ws.send(JSON.stringify({id:i,method,params:params||{},sessionId}));}),close:()=>ws.close()}));
});}
suite("cross02-ads-mapping",function(){this.timeout(180000);test("endpoint inspector shows var-to-symbol point mappings",async function(){
  const x=vscode.extensions.getExtension("trust-platform.trust-lsp"); if(x) await x.activate(); await sleep(2500);
  await cmd("workbench.action.closeAuxiliaryBar"); await cmd("workbench.action.closePanel");
  await cmd("trust-lsp.networkCanvas.open"); await sleep(9000);
  await cmd("workbench.action.closePanel"); await sleep(500);
  const ver=await httpJson("/json/version"); const targets=await httpJson("/json");
  const page=targets.find(t=>t.type==="page");
  const webview=targets.find(t=>t.type==="iframe"&&/index\\.html/.test(t.url||""));
  const conn=await cdp(ver.webSocketDebuggerUrl);
  const win=await conn.send("Browser.getWindowForTarget",{targetId:page.id});
  if(win.result&&win.result.windowId){await conn.send("Browser.setWindowBounds",{windowId:win.result.windowId,bounds:{left:0,top:0,width:1920,height:1080,windowState:"normal"}});}
  await sleep(1500);
  const at=await conn.send("Target.attachToTarget",{targetId:webview.id,flatten:true});
  const sid=at.result&&at.result.sessionId; await conn.send("Runtime.enable",{},sid);
  async function evalInner(body){const expr="(function(){try{var f=document.querySelector('iframe');var d=f&&f.contentDocument;var w=f&&f.contentWindow;if(!d)return 'NO_INNER_DOC';"+body+"}catch(e){return 'ERR:'+e.message;}})()";const ev=await conn.send("Runtime.evaluate",{expression:expr,returnByValue:true},sid);return ev&&ev.result&&ev.result.result&&ev.result.result.value;}
  await sleep(1200);
  const clickRes=await evalInner([
    "var nodes=[...d.querySelectorAll('.react-flow__node')];",
    "var allTexts=nodes.map(function(n){return {t:(n.textContent||'').replace(/\\\\s+/g,' ').trim().slice(0,40),cls:(n.getAttribute('class')||'')};});",
    "var node=nodes.find(function(n){var t=(n.textContent||'').toLowerCase();var isRt=t.indexOf('commadsres')>=0||t.indexOf('computer')>=0||t.indexOf('reachable')>=0;return !isRt&&(t.indexOf('ads')>=0||t.indexOf('twincat')>=0||t.indexOf('client')>=0);});",
    "if(!node){return JSON.stringify({error:'NO_ADS_ENDPOINT',nodes:allTexts});}",
    "var r=node.getBoundingClientRect();var cx=r.left+r.width/2,cy=r.top+18;",
    "['pointerdown','pointerup'].forEach(function(t){try{node.dispatchEvent(new w.PointerEvent(t,{bubbles:true,cancelable:true,clientX:cx,clientY:cy,pointerId:1,button:0,isPrimary:true}));}catch(e){}});",
    "try{node.dispatchEvent(new w.MouseEvent('click',{bubbles:true,cancelable:true,clientX:cx,clientY:cy,button:0}));}catch(e){}",
    "return JSON.stringify({clicked:(node.textContent||'').replace(/\\\\s+/g,' ').trim().slice(0,40)});"
  ].join(""));
  await sleep(2000);
  const inspText=await evalInner([
    "var insp=d.querySelector('.trust-inspector')||[...d.querySelectorAll('[class*=inspector]')].pop()||d.querySelector('aside');",
    "return insp?(insp.textContent||'').replace(/\\\\s+/g,' ').trim():'NO_INSPECTOR';"
  ].join(""));
  const screenshot=shot("CROSS-02-ads-point-mapping");
  const hasArrow=/←/.test(String(inspText));
  const hasSymbol=/MAIN\\.Temperature|GVL\\.LineReady|GVL\\.Setpoint/.test(String(inspText));
  const hasVar=/line1_temp|line1_ready|line1_setpoint|line1_status/.test(String(inspText));
  fs.writeFileSync(path.join(jsonDir,"CROSS-02-ads-point-mapping-proof.json"),JSON.stringify({
    row:"CROSS-02",
    workflow:"Endpoint inspector renders var-to-symbol point mappings",
    action:"Opened the ads_line1 canvas and clicked the ADS endpoint; read the inspector connection summary.",
    click:clickRes,
    screenshot:screenshot,
    hasArrowMapping:hasArrow,
    hasExternalSymbol:hasSymbol,
    hasStVar:hasVar,
    inspectorText:String(inspText).slice(0,1500)
  },null,2));
  conn.close();
});});`);
fs.writeFileSync(path.join(testsDir, "run.js"), `const Mocha=require(${JSON.stringify(path.join(ext, "node_modules/mocha"))});const path=require("path");exports.run=function(){const m=new Mocha({ui:"tdd",timeout:180000});m.addFile(path.join(__dirname,"index.js"));return new Promise((res,rej)=>m.run(f=>f?rej(new Error(f+" fail")):res()));};`);
async function main() {
  await runTests({
    vscodeExecutablePath: codeBin, extensionDevelopmentPath: ext, extensionTestsPath: path.join(testsDir, "run.js"),
    launchArgs: [workspace, "--remote-debugging-port=" + PORT, "--ozone-platform=x11", "--disable-gpu", "--use-gl=angle", "--use-angle=swiftshader", "--in-process-gpu", "--no-sandbox", "--user-data-dir=" + path.join(outDir, "ud"), "--extensions-dir=" + path.join(outDir, "ed"), "--disable-workspace-trust", "--skip-welcome"],
    extensionTestsEnv: { ST_LSP_TEST_SERVER: path.join(repo, "target/debug/trust-lsp"), ST_RUNTIME_TEST_BIN: path.join(repo, "target/debug/trust-runtime") }
  });
  console.log("CROSS02_ADS_MAPPING_DONE");
}
main().catch(e => { console.error(e); process.exit(1); });
