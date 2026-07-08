// Capture advanced protocol add/section forms (Mesh, OpenOT, Realtime-T0, cloud/federation).
// Clean native profile, current evidence root.
const path=require("path"),fs=require("fs");
const repo=process.env.TRUST_PLATFORM_REPO_ROOT||path.resolve(__dirname,"../../../../../../..");
const ext=path.join(repo,"editors/vscode");
const pngHygienePath=path.join(__dirname,"png-hygiene.js");
const { runTests }=require(path.join(ext,"node_modules/@vscode/test-electron"));
const PORT=Number(process.env.PROTOCOL_FORMS2_CDP_PORT||9361);
const base=process.env.TRUST_UX_EVIDENCE_ROOT?path.resolve(process.env.TRUST_UX_EVIDENCE_ROOT):"/home/johannes/proto-test-evidence/cdp-protocol-forms2";
const runDir=path.join(base,"runner-output","cdp-protocol-forms2");
const outDir=process.env.TRUST_UX_SCREENSHOTS_DIR?path.resolve(process.env.TRUST_UX_SCREENSHOTS_DIR):path.join(base,"screenshots-raw");
const jsonDir=process.env.TRUST_UX_JSON_DIR?path.resolve(process.env.TRUST_UX_JSON_DIR):path.join(base,"json");
const workspace=path.join(runDir,"project"), testsDir=path.join(runDir,"tests"), userDataDir=path.join(runDir,"ud"), extensionsDir=path.join(runDir,"ed");
fs.rmSync(runDir,{recursive:true,force:true}); fs.mkdirSync(outDir,{recursive:true}); fs.mkdirSync(jsonDir,{recursive:true}); fs.mkdirSync(testsDir,{recursive:true});
fs.cpSync(path.join(repo,"examples/network_canvas_demo"), workspace, {recursive:true});
fs.mkdirSync(path.join(userDataDir,"User"),{recursive:true});fs.writeFileSync(path.join(userDataDir,"User","settings.json"),JSON.stringify({"window.titleBarStyle":"native","window.commandCenter":false,"chat.commandCenter.enabled":false,"workbench.layoutControl.enabled":false,"workbench.startupEditor":"none","workbench.tips.enabled":false,"telemetry.telemetryLevel":"off","update.mode":"none","workbench.colorTheme":"Default Dark Modern"}));
const codeDir=fs.readdirSync(path.join(ext,".vscode-test")).find(d=>d.startsWith("vscode-linux-arm64-"))||fs.readdirSync(path.join(ext,".vscode-test")).find(d=>d.startsWith("vscode-linux-x64-"));
const codeBin=path.join(ext,".vscode-test",codeDir,"code");
const PROTOS=[
  ["mesh","ADV-01-mesh-zenoh","Mesh"],
  ["openot","ADV-02-openot","OpenOT"],
  ["realtime t0","ADV-03-realtime-t0","Realtime"],
  ["runtime cloud","ADV-04-runtime-cloud","Federation"]
];
fs.writeFileSync(path.join(testsDir,"index.js"),`
	const path=require("path"),fs=require("fs"),http=require("http"),cp=require("child_process"),vscode=require("vscode");
	const WebSocket=require(${JSON.stringify(path.join(ext,"node_modules/ws"))});
	const pngHygiene=require(${JSON.stringify(pngHygienePath)});
const outDir=${JSON.stringify(outDir)}; const jsonDir=${JSON.stringify(jsonDir)}; const PORT=${PORT}; const PROTOS=${JSON.stringify(PROTOS)};
function sleep(ms){return new Promise(r=>setTimeout(r,ms));}
async function cmd(id,...a){try{return await vscode.commands.executeCommand(id,...a);}catch(e){return "ERR:"+(e&&e.message||e);}}
function httpJson(p){return new Promise((res,rej)=>{const rq=http.get("http://localhost:"+PORT+p,r=>{let b="";r.on("data",c=>b+=c);r.on("end",()=>{try{res(JSON.parse(b));}catch(e){rej(e);}});});rq.on("error",rej);rq.setTimeout(4000,()=>rq.destroy(new Error("t")));});}
function shot(name){const raw=path.join(outDir,name+".raw.png"),dest=path.join(outDir,name+".png");
  const env=Object.assign({},process.env,{PATH:"/usr/bin:/bin:"+(process.env.PATH||"")});
  try{cp.execFileSync("/usr/bin/import",["-window","root",raw],{stdio:"ignore",env});
    pngHygiene.stripPngFile(raw);
    try{cp.execFileSync("/usr/bin/convert",[raw,"-strip","-bordercolor","black","-border","1","-trim","+repage",dest],{stdio:"ignore",env});}catch(e){fs.copyFileSync(raw,dest);}
    pngHygiene.stripPngFile(dest);
  }catch(e){fs.writeFileSync(path.join(outDir,name+".err.txt"),String(e&&e.message||e));}}
function cdp(wsUrl){return new Promise((resolve,reject)=>{const ws=new WebSocket(wsUrl);let id=0;const pending=new Map();
  ws.on("message",d=>{const m=JSON.parse(d.toString());if(m.id&&pending.has(m.id)){pending.get(m.id)(m);pending.delete(m.id);}});
  ws.on("error",reject);
  ws.on("open",()=>resolve({send:(method,params,sessionId)=>new Promise(r=>{const i=++id;let done=false;const fin=v=>{if(!done){done=true;r(v);}};pending.set(i,fin);setTimeout(()=>fin({__timeout:method}),6000);ws.send(JSON.stringify({id:i,method,params:params||{},sessionId}));}),close:()=>ws.close()}));
});}
suite("protocol-forms2",function(){this.timeout(220000);test("run",async function(){
  const log={steps:[]};
  const x=vscode.extensions.getExtension("trust-platform.trust-lsp"); if(x) await x.activate(); await sleep(1500);
  await cmd("workbench.action.closeAuxiliaryBar"); await cmd("workbench.action.closePanel");
  await cmd("trust-lsp.networkCanvas.open"); await sleep(6000);
  const ver=await httpJson("/json/version");
  const targets=await httpJson("/json");
  const page=targets.find(t=>t.type==="page");
  const webview=targets.find(t=>t.type==="iframe"&&/index\\.html/.test(t.url||""));
  const conn=await cdp(ver.webSocketDebuggerUrl);
  const win=await conn.send("Browser.getWindowForTarget",{targetId:page.id});
  if(win.result&&win.result.windowId){await conn.send("Browser.setWindowBounds",{windowId:win.result.windowId,bounds:{left:0,top:0,width:1920,height:1080,windowState:"normal"}});}
  await sleep(1500);
  const at=await conn.send("Target.attachToTarget",{targetId:webview.id,flatten:true});
  const sid=at.result&&at.result.sessionId;
  await conn.send("Runtime.enable",{},sid);
  async function evalInner(body){const expr="(function(){try{var f=document.querySelector('iframe');var d=f&&f.contentDocument;if(!d)return 'NO_INNER_DOC';"+body+"}catch(e){return 'ERR:'+e.message;}})()";const ev=await conn.send("Runtime.evaluate",{expression:expr,returnByValue:true},sid);return ev&&ev.result&&ev.result.result&&ev.result.result.value;}
  async function clickText(sub){return await evalInner("var s="+JSON.stringify(sub.toLowerCase())+";var compact=s.replace(/\\\\s+/g,'');var b=[...d.querySelectorAll('button,[role=button]')].find(function(x){var text=((x.textContent||x.getAttribute('aria-label')||'').toLowerCase()).replace(/\\\\s+/g,' ').trim();var tight=text.replace(/\\\\s+/g,'');return text.indexOf(s)>=0||tight.indexOf(compact)>=0;});if(!b)return 'NOT_FOUND:'+s;b.click();return 'CLICKED:'+(b.textContent||b.getAttribute('aria-label')||'').trim().slice(0,48);");}
  async function waitForText(sub,timeoutMs){const start=Date.now();let text='';while(Date.now()-start<timeoutMs){text=String(await evalInner("return (d.body&&d.body.innerText||'').replace(/\\\\s+/g,' ').trim();")||'');if(text.toLowerCase().indexOf(String(sub).toLowerCase())>=0)return text;await sleep(350);}throw new Error('Timed out waiting for '+sub+': '+text.slice(0,1600));}
  async function openAddPicker(){const clicked=await clickText("+ Add"); await sleep(1000); await waitForText("Add device or connection",10000); return clicked;}
  async function setProfile(value){return await evalInner("var value="+JSON.stringify(value)+";var selects=[...d.querySelectorAll('select')];var profile=selects.find(function(s){return (s.closest('label,div,section,fieldset')||s.parentElement||s).textContent.toLowerCase().indexOf('profile')>=0;})||selects[0];if(!profile)return 'NO_PROFILE_SELECT';var setter=Object.getOwnPropertyDescriptor(HTMLSelectElement.prototype,'value').set;setter.call(profile,value);profile.dispatchEvent(new Event('input',{bubbles:true}));profile.dispatchEvent(new Event('change',{bubbles:true}));return 'PROFILE_SET:'+profile.value;");}
  async function scrollPickerBottom(){return await evalInner("var el=d.querySelector('[data-testid=add-picker-list]');if(!el)return 'NO_PICKER_LIST';el.scrollTop=el.scrollHeight;return 'SCROLLED:'+el.scrollTop;");}
  log.steps.push({s:"edit",v:await clickText("edit")}); await sleep(1000);
  let capturedAdvancedPicker=false;
  for(const [pick,name,configuredLabel] of PROTOS){
    const a=await openAddPicker(); await sleep(500);
    const advanced=await clickText("show advanced integrations"); await sleep(900);
    if(!capturedAdvancedPicker){log.steps.push({s:"scroll-advanced",v:await scrollPickerBottom()}); await sleep(700); shot("ADV-00-advanced-picker-expanded"); capturedAdvancedPicker=true;}
    const p=await clickText(pick); await sleep(1800);
    const configuredInput=pick==="runtime cloud"?await setProfile("plant"):"";
    if(configuredInput) await sleep(600);
    shot(name);
    const save=await clickText("save"); await sleep(2500);
    const configuredText=await waitForText(configuredLabel,10000);
    shot(name+"-configured-only");
    log.steps.push({proto:name,add:a,advanced,pick:p,configuredInput,save,configuredLabel,configuredText:configuredText.slice(0,1200)});
  }
  conn.close();
  fs.writeFileSync(path.join(jsonDir,"ADV-01-04-forms-proof.json"),JSON.stringify(log,null,2));
});});`);
fs.writeFileSync(path.join(testsDir,"run.js"),`const Mocha=require(${JSON.stringify(path.join(ext,"node_modules/mocha"))});const path=require("path");exports.run=function(){const m=new Mocha({ui:"tdd",timeout:220000});m.addFile(path.join(__dirname,"index.js"));return new Promise((res,rej)=>m.run(f=>f?rej(new Error(f+" fail")):res()));};`);
async function main(){
  await runTests({ vscodeExecutablePath:codeBin, extensionDevelopmentPath:ext, extensionTestsPath:path.join(testsDir,"run.js"),
    launchArgs:[workspace,"--remote-debugging-port="+PORT,"--ozone-platform=x11","--disable-gpu","--use-gl=angle","--use-angle=swiftshader","--in-process-gpu","--no-sandbox","--user-data-dir",userDataDir,"--extensions-dir",extensionsDir,"--disable-workspace-trust","--skip-welcome"],
    extensionTestsEnv:{ ST_LSP_TEST_SERVER:path.join(repo,"target/debug/trust-lsp"), ST_RUNTIME_TEST_BIN:path.join(repo,"target/debug/trust-runtime") } });
  console.log("CDP_PROTOCOL_FORMS2_DONE");
}
main().catch(e=>{console.error(e);process.exit(1);});
