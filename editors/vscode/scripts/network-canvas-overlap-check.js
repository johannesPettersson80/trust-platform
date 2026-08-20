#!/usr/bin/env node
// Render gate for the React Flow Network Canvas: renders the built webview bundle in headless
// Chromium across several fixtures (incl. Edit mode and a dense fleet), measures every
// `.react-flow__node` rect, and FAILS if any two nodes PARTIALLY overlap (parent⊃child nesting is
// fine; partial overlap is a bug). It also locks visible status semantics that cannot be proved by
// the pure graph model alone. Replaces the deleted HTML-view gate. Catches both layout-math and
// render/position-merge overlaps (e.g. the 2026-06-18 edit-mode SCADA overlap).
//
// Skips gracefully (exit 0) when no Chromium is available, so CI without a browser doesn't fail;
// it gates wherever Chromium is present (dev machines, browser-equipped CI).
const { execFileSync } = require("child_process");
const fs = require("fs");
const os = require("os");
const path = require("path");

const ROOT = path.resolve(__dirname, "..");
const BUNDLE = path.join(ROOT, "media", "networkCanvasWebview.js");
const CSS = path.join(ROOT, "media", "networkCanvasWebview.css");

function findChrome() {
  const cands = [
    process.env.CHROME,
    process.env.CHROMIUM,
    "chromium",
    "chromium-browser",
    "google-chrome",
    "google-chrome-stable",
  ].filter(Boolean);
  for (const c of cands) {
    try {
      execFileSync(c, ["--version"], { stdio: "ignore" });
      return c;
    } catch {
      /* try next */
    }
  }
  return null;
}

// A runtime with `n` endpoints (protocol-cycled), used to build fixtures.
function rt(id, name, protocols) {
  return {
    id,
    name,
    mode: "online",
    health: "connected",
    detail: "ok",
    endpoints: protocols.map((p, i) => ({
      id: `${id}-ep${i}`,
      kind: p === "opcua" || p === "mqtt" ? "service" : p === "mesh" ? "peer" : "field",
      protocol: p,
      name: `${p} ${i}`,
      role: p === "opcua" ? "server" : p === "ethercat" ? "master" : "client",
      health: "connected",
      detail: "ok",
      // §10.2: EtherCAT segments carry slave children → taller node; gate must stay overlap-free.
      children:
        p === "ethercat"
          ? [
              { id: `${id}-ep${i}-s0`, kind: "field_slave", slot: 0, name: "EK1100 (slot 0)", model: "EK1100", channels: 1, source: "config", health: "configured_policy" },
              { id: `${id}-ep${i}-s1`, kind: "field_slave", slot: 1, name: "EL1008 (slot 1)", model: "EL1008", channels: 8, source: "config", health: "configured_policy" },
              { id: `${id}-ep${i}-s2`, kind: "field_slave", slot: 2, name: "EL2008 (slot 2)", model: "EL2008", channels: 8, source: "config", health: "configured_policy" },
            ]
          : undefined,
    })),
  };
}

const FIXTURES = [
  {
    name: "single (view)",
    edit: false,
    nc: {
      kind: "graph",
      title: "NC",
      summary: "",
      hosts: [{ id: "h1", hostname: "pi-1", label: "pi", health: "connected", containers: [], runtimes: [rt("rt1", "Line A", ["modbus_tcp", "opcua"])] }],
      links: [{ id: "l1", from: "rt1-ep1", to: "ext-scada", protocol: "opcua", role: "server", status: "connected", secure: true }],
      external: [{ id: "ext-scada", name: "SCADA", kind: "client" }],
      faults: [],
    },
  },
  {
    name: "single (edit)",
    edit: true,
    nc: {
      kind: "graph",
      title: "NC",
      summary: "",
      hosts: [{ id: "h1", hostname: "pi-1", label: "pi", health: "connected", containers: [], runtimes: [rt("rt1", "Line A", ["modbus_tcp", "opcua"])] }],
      links: [{ id: "l1", from: "rt1-ep1", to: "ext-scada", protocol: "opcua", role: "server", status: "connected", secure: true }],
      external: [{ id: "ext-scada", name: "SCADA", kind: "client" }],
      faults: [],
    },
  },
  {
    name: "dense fleet (edit)",
    edit: true,
    nc: {
      kind: "graph",
      title: "NC",
      summary: "",
      hosts: [
        {
          id: "h1",
          hostname: "pi-1",
          label: "pi",
          health: "connected",
          containers: [],
          runtimes: [rt("rtA", "Line A", ["modbus_tcp", "opcua", "mqtt", "ethercat", "mesh"])],
        },
        {
          id: "h2",
          hostname: "ipc-2",
          label: "ipc",
          health: "degraded",
          containers: [{ id: "c1", name: "pkr", image: "trust:0.24", status: "running", runtimes: [rt("rtB", "Packer", ["modbus_tcp", "mesh"]), rt("rtC", "Filler", ["gpio"])] }],
          runtimes: [rt("rtD", "Edge", ["opcua", "mesh"])],
        },
      ],
      links: [
        { id: "l1", from: "rtA-ep1", to: "ext-scada", protocol: "opcua", role: "server", status: "connected", secure: true },
        { id: "l2", from: "rtA-ep2", to: "ext-broker", protocol: "mqtt", role: "client", status: "connected", secure: false },
        { id: "l3", from: "rtD-ep0", to: "ext-hist", protocol: "opcua", role: "server", status: "degraded", secure: false },
      ],
      external: [
        { id: "ext-scada", name: "SCADA", kind: "client" },
        { id: "ext-broker", name: "Broker", kind: "broker" },
        { id: "ext-hist", name: "Historian", kind: "client" },
      ],
      faults: [],
    },
  },
  {
    name: "status truth (view)",
    edit: false,
    renderContractRuntimeId: "runtime:managed-unavailable",
    nc: {
      kind: "graph",
      title: "NC",
      summary: "",
      hosts: [
        {
          id: "host:status-truth",
          hostname: "this-computer",
          label: "This computer",
          health: "connected",
          containers: [],
          runtimes: [
            {
              id: "runtime:managed-unavailable",
              name: "Managed runtime",
              mode: "managed",
              health: "error",
              lifecycleState: "unavailable",
              detail: "Status unavailable — refresh before starting.",
              controlEndpoint: "tcp://127.0.0.1:9902",
              managed: true,
              managedName: "cell1",
              endpoints: [
                { id: "endpoint:connected", kind: "service", protocol: "opcua", name: "Connected", role: "client", health: "connected", detail: "connected" },
                { id: "endpoint:degraded", kind: "service", protocol: "opcua", name: "Degraded", role: "client", health: "degraded", detail: "degraded" },
                { id: "endpoint:error", kind: "service", protocol: "opcua", name: "Error", role: "client", health: "error", detail: "error" },
                { id: "endpoint:future", kind: "service", protocol: "opcua", name: "Future", role: "client", health: "future_status", detail: "future" },
                { id: "endpoint:configured", kind: "service", protocol: "opcua", name: "Configured", role: "client", health: "configured_policy", detail: "Waiting for runtime restart." },
                { id: "endpoint:disabled", kind: "service", protocol: "opcua", name: "Disabled", role: "client", health: "disabled", detail: "This endpoint is disabled." },
                { id: "endpoint:mesh", kind: "peer", protocol: "mesh", name: "Mesh / Zenoh", role: "peer", health: "degraded", detail: "Two configured peers." },
              ],
            },
            {
              id: "runtime:managed-starting",
              name: "Managed alpha",
              mode: "managed",
              health: "pending",
              lifecycleState: "starting",
              detail: "Starting managed local runtime…",
              managed: true,
              managedName: "cell-starting",
              endpoints: [],
            },
            {
              id: "runtime:managed-stopping",
              name: "Managed beta",
              mode: "managed",
              health: "pending",
              lifecycleState: "stopping",
              detail: "Stopping managed local runtime…",
              managed: true,
              managedName: "cell-stopping",
              endpoints: [],
            },
            {
              id: "runtime:configured",
              name: "Configured runtime",
              mode: "connect",
              health: "configured_policy",
              detail: "Configured in project files; runtime is not running.",
              endpoints: [],
            },
          ],
        },
      ],
      links: [
        { id: "link:connected", from: "endpoint:connected", to: "external:connected", protocol: "opcua", role: "client", status: "connected", secure: false, detail: "Session established." },
        { id: "link:degraded", from: "endpoint:degraded", to: "external:degraded", protocol: "opcua", role: "client", status: "degraded", secure: false, detail: "Peer handshake is retrying." },
        { id: "link:error", from: "endpoint:error", to: "external:error", protocol: "opcua", role: "client", status: "error", secure: false, detail: "Peer certificate was rejected." },
        { id: "link:future", from: "endpoint:future", to: "external:future", protocol: "opcua", role: "client", status: "future_status", secure: false },
        { id: "link:configured", from: "endpoint:configured", to: "external:configured", protocol: "opcua", role: "client", status: "configured_policy", secure: false, detail: "Waiting for runtime restart." },
        { id: "link:mesh-degraded", from: "endpoint:mesh", to: "external:mesh-degraded", protocol: "mesh", role: "peer", status: "degraded", secure: true, detail: "Mesh peer latency is elevated." },
        { id: "link:mesh-error", from: "endpoint:mesh", to: "external:mesh-error", protocol: "mesh", role: "peer", status: "error", secure: true, detail: "Mesh peer authentication failed." },
      ],
      external: [
        { id: "external:connected", name: "Connected peer", kind: "server" },
        { id: "external:degraded", name: "Degraded peer", kind: "server" },
        { id: "external:error", name: "Error peer", kind: "server" },
        { id: "external:future", name: "Future peer", kind: "server" },
        { id: "external:configured", name: "Configured peer", kind: "server" },
        { id: "external:mesh-degraded", name: "tcp/192.168.77.11:7447", kind: "peer" },
        { id: "external:mesh-error", name: "tcp/192.168.77.12:7447", kind: "peer" },
      ],
      faults: [],
    },
  },
];

// Runs inside the page: flag pairs that overlap where NEITHER contains the other, then exercise
// visible status truth against the actual React/SVG DOM for the dedicated contract fixture.
const MEASURE = `
function __finishMeasure(result){
  var pre=document.createElement('pre');pre.id='nc-geom-result';
  pre.textContent=JSON.stringify(result);
  document.body.appendChild(pre);
}
function __check(result,condition,message){if(!condition)result.renderFailures.push(message);}
function __edgeIsDashed(result,id){
  var edge=document.querySelector('.react-flow__edge[data-id="'+id+'"]');
  var path=edge&&edge.querySelector('.react-flow__edge-path');
  __check(result,Boolean(path),'missing rendered edge '+id);
  if(!path)return undefined;
  var dash=(path.style.strokeDasharray||getComputedStyle(path).strokeDasharray||'').trim();
  return dash!==''&&dash!=='none';
}
function __resolvedColor(value){
  var probe=document.createElement('span');
  probe.style.color=value;document.body.appendChild(probe);
  var color=getComputedStyle(probe).color;probe.remove();return color;
}
function __checkHealthEdge(result,id,status,detail,expectedColor){
  var edge=document.querySelector('.react-flow__edge[data-id="'+id+'"]');
  var path=edge&&edge.querySelector('.react-flow__edge-path');
  __check(result,Boolean(path),'missing rendered edge '+id);
  if(path&&expectedColor){
    __check(result,getComputedStyle(path).stroke===__resolvedColor(expectedColor),id+' must use '+status+' semantic tone');
  }
  var target=edge&&edge.querySelector('[data-link-health]');
  __check(result,Boolean(target),id+' must expose an interactive health-detail target');
  if(!target)return;
  target.dispatchEvent(new MouseEvent('mouseover',{bubbles:true,cancelable:true,view:window}));
  target.focus();
  var label=(target.getAttribute('aria-label')||'').replace(/\\s+/g,' ').trim();
  var title=target.querySelector('title');
  __check(result,label===status+' — '+detail,id+' must expose exact accessible status detail (got "'+label+'")');
  __check(result,Boolean(title)&&(title.textContent||'').trim()===status+' — '+detail,id+' must preserve exact native hover text');
}
function __checkHealthTooltip(result,id,status,detail){
  var tooltip=document.querySelector('[data-link-health-detail="'+id+'"]');
  __check(result,Boolean(tooltip),id+' must show readable status detail on hover or focus');
  if(tooltip){
    __check(result,(tooltip.textContent||'').replace(/\\s+/g,' ').trim()===status+' — '+detail,id+' tooltip text must preserve status detail');
  }
}
function __measure(renderContractRuntimeId){
  function rc(el){var r=el.getBoundingClientRect();return {x:r.left,y:r.top,r:r.right,b:r.bottom};}
  function ov(a,b){return a.x<b.r&&b.x<a.r&&a.y<b.b&&b.y<a.b;}
  function contains(a,b){var t=3;return a.x<=b.x+t&&a.y<=b.y+t&&a.r>=b.r-t&&a.b>=b.b-t;}
  var nodes=[].slice.call(document.querySelectorAll('.react-flow__node'));
  var items=nodes.map(function(n){return {id:n.getAttribute('data-id'),rc:rc(n)};});
  var bad=[];
  for(var i=0;i<items.length;i++)for(var j=i+1;j<items.length;j++){
    var A=items[i].rc,B=items[j].rc;
    if(ov(A,B)&&!contains(A,B)&&!contains(B,A)){
      var ox=Math.min(A.r,B.r)-Math.max(A.x,B.x),oy=Math.min(A.b,B.b)-Math.max(A.y,B.y);
      if(ox>4&&oy>4)bad.push(items[i].id+' x '+items[j].id+' ('+Math.round(ox)+'x'+Math.round(oy)+')');
    }
  }
  var result={count:items.length,overlaps:bad,renderFailures:[]};
  if(!renderContractRuntimeId){__finishMeasure(result);return;}

  __check(result,__edgeIsDashed(result,'link:degraded')===false,'degraded proven link must render solid');
  __check(result,__edgeIsDashed(result,'link:error')===false,'error proven link must render solid');
  __check(result,__edgeIsDashed(result,'link:future')===true,'unrecognized future link must render dashed');
  __checkHealthEdge(result,'link:degraded','Degraded','Peer handshake is retrying.','var(--vscode-charts-yellow, var(--vscode-editorWarning-foreground, #e0b341))');
  __checkHealthEdge(result,'link:error','Error','Peer certificate was rejected.','var(--vscode-charts-red, var(--vscode-errorForeground, #f0584f))');
  __checkHealthEdge(result,'link:configured','Configured','Waiting for runtime restart.',null);
  __check(result,__edgeIsDashed(result,'link:mesh-degraded')===false,'degraded proven mesh peer must render solid');
  __check(result,__edgeIsDashed(result,'link:mesh-error')===false,'error proven mesh peer must render solid');
  __checkHealthEdge(result,'link:mesh-degraded','Degraded','Mesh peer latency is elevated.','var(--vscode-charts-yellow, var(--vscode-editorWarning-foreground, #e0b341))');
  __checkHealthEdge(result,'link:mesh-error','Error','Mesh peer authentication failed.','var(--vscode-charts-red, var(--vscode-errorForeground, #f0584f))');
  var structuralMeshEdge=document.querySelector('.react-flow__edge[data-id="mesh-endpoint:mesh"]');
  __check(result,Boolean(structuralMeshEdge),'mesh endpoint must remain connected to the shared bus');
  __check(result,!structuralMeshEdge||!structuralMeshEdge.querySelector('[data-link-health]'),'structural mesh drop must not create a meaningless interactive Link target');
  var configuredRuntime=document.querySelector('.react-flow__node[data-id="runtime:configured"]');
  __check(result,Boolean(configuredRuntime),'missing configured runtime node');
  if(configuredRuntime){
    var configuredText=(configuredRuntime.textContent||'').replace(/\\s+/g,' ').trim();
    __check(result,/\\bConfigured\\b/.test(configuredText),'configured runtime must expose the Configured product label');
    __check(result,configuredText.indexOf('Configured only')<0&&configuredText.indexOf('configured_policy')<0,'configured runtime must not expose private or raw backend labels');
  }
  var disabledEndpoint=document.querySelector('.react-flow__node[data-id="endpoint:disabled"]');
  __check(result,Boolean(disabledEndpoint),'missing disabled endpoint node');
  if(disabledEndpoint){
    var disabledLabels=[].slice.call(disabledEndpoint.querySelectorAll('[title="disabled"]')).map(function(node){
      return (node.textContent||'').replace(/\\s+/g,' ').trim();
    });
    __check(result,disabledLabels.indexOf('Disabled')>=0,'disabled endpoint must expose the Disabled product label (got '+JSON.stringify(disabledLabels)+')');
  }
  for(var managedState of [
    ['runtime:managed-starting','Starting'],
    ['runtime:managed-stopping','Stopping'],
    ['runtime:managed-unavailable','Status unavailable'],
  ]){
    var managedNode=document.querySelector('.react-flow__node[data-id="'+managedState[0]+'"]');
    __check(result,Boolean(managedNode),'missing '+managedState[0]+' node');
    if(managedNode){
      var managedText=(managedNode.textContent||'').replace(/\\s+/g,' ').trim();
      var managedNormalized=managedText.toLowerCase().replace(/\\s+/g,'');
      var expectedNormalized=managedState[1].toLowerCase().replace(/\\s+/g,'');
      __check(result,managedNormalized.indexOf(expectedNormalized)>=0,managedState[0]+' must expose '+managedState[1]+' on the node (got "'+managedText+'")');
    }
  }
  var errorTarget=document.querySelector('.react-flow__edge[data-id="link:error"] [data-link-health]');
  if(errorTarget){
    errorTarget.dispatchEvent(new MouseEvent('mouseover',{bubbles:true,cancelable:true,view:window}));
    errorTarget.focus();
  }

  setTimeout(function(){
    __checkHealthTooltip(result,'link:error','Error','Peer certificate was rejected.');
    var degradedTarget=document.querySelector('.react-flow__edge[data-id="link:degraded"] [data-link-health]');
    if(degradedTarget){
      degradedTarget.dispatchEvent(new MouseEvent('mouseover',{bubbles:true,cancelable:true,view:window}));
      degradedTarget.focus();
    }
    setTimeout(function(){
      __checkHealthTooltip(result,'link:degraded','Degraded','Peer handshake is retrying.');
      var runtime=document.querySelector('.react-flow__node[data-id="'+renderContractRuntimeId+'"]');
      __check(result,Boolean(runtime),'missing managed-unavailable runtime node');
      if(!runtime){__finishMeasure(result);return;}
      runtime.dispatchEvent(new MouseEvent('click',{bubbles:true,cancelable:true,view:window}));
      setTimeout(function(){
        var inspector=document.querySelector('aside[aria-label="Node summary"]');
        __check(result,Boolean(inspector),'managed-unavailable node must open its inspector');
        if(inspector){
          var text=(inspector.textContent||'').replace(/\\s+/g,' ').trim();
          var primary=inspector.querySelector('button.trust-button--primary');
          __check(result,text.indexOf('Status unavailable')>=0,'inspector must show Status unavailable');
          __check(result,!/\\bStopped\\b/.test(text),'managed-unavailable runtime must not render as Stopped');
          __check(result,Boolean(primary),'managed-unavailable inspector must render a primary action');
          if(primary){
            __check(result,(primary.textContent||'').trim()==='Start','managed-unavailable primary action must stay Start');
            __check(result,primary.disabled===true,'managed-unavailable Start must be disabled');
          }
        }
        __finishMeasure(result);
      },200);
    },50);
  },50);
}
`;

function harnessHtml(fixture) {
  return `<!DOCTYPE html><html><head><meta charset="UTF-8"/>
<link rel="stylesheet" href="file://${CSS}"/>
<style>*{box-sizing:border-box;margin:0;padding:0}html,body,#root{width:100%;height:100%;overflow:hidden;background:#0f1116;color:#eef1f5;font-family:system-ui,sans-serif}</style>
</head><body><div id="root"></div>
<script>window.acquireVsCodeApi=function(){return{postMessage(){},getState(){},setState(){}}};</script>
<script>window.__NC__=${JSON.stringify(fixture.nc)};</script>
<script>${MEASURE}
function __click(t){var b=[].slice.call(document.querySelectorAll("button")).find(function(x){return x.textContent.replace(/\\s+/g," ").indexOf(t)>=0;});if(b)b.click();}
setTimeout(function(){ ${fixture.edit ? '__click("Edit");' : ""} setTimeout(function(){__measure(${JSON.stringify(fixture.renderContractRuntimeId ?? null)});},1100); },800);
</script>
<script src="file://${BUNDLE}"></script>
</body></html>`;
}

function unescapeHtml(s) {
  return s.replace(/&amp;/g, "&").replace(/&lt;/g, "<").replace(/&gt;/g, ">").replace(/&quot;/g, '"').replace(/&#39;/g, "'");
}

function main() {
  if (!fs.existsSync(BUNDLE)) {
    console.error(`[canvas-geometry] bundle not found at ${BUNDLE} — run \`npm run build:network-canvas\` first.`);
    process.exit(1);
  }
  const chrome = findChrome();
  if (!chrome) {
    const message = "no Chromium found (set CHROME=/path). Render contract not gated here.";
    if (process.argv.includes("--require-browser")) {
      console.error(`[canvas-geometry] ${message} --require-browser forbids skipping.`);
      process.exit(1);
    }
    console.warn(`[canvas-geometry] SKIP — ${message}`);
    process.exit(0);
  }

  const tmp = fs.mkdtempSync(path.join(os.tmpdir(), "nc-geom-"));
  let failures = 0;
  for (const fixture of FIXTURES) {
    const file = path.join(tmp, `${fixture.name.replace(/\W+/g, "_")}.html`);
    fs.writeFileSync(file, harnessHtml(fixture));
    let dom = "";
    try {
      dom = execFileSync(
        chrome,
        ["--headless=new", "--disable-gpu", "--no-sandbox", "--hide-scrollbars", "--window-size=1600,900", "--virtual-time-budget=5000", "--dump-dom", `file://${file}`],
        { encoding: "utf8", stdio: ["ignore", "pipe", "ignore"], maxBuffer: 64 * 1024 * 1024 }
      );
    } catch (err) {
      console.error(`[canvas-geometry] ${fixture.name}: chromium failed — ${err.message}`);
      failures++;
      continue;
    }
    const m = dom.match(/<pre id="nc-geom-result">([\s\S]*?)<\/pre>/);
    if (!m) {
      console.error(`[canvas-geometry] ${fixture.name}: no measurement produced (render failed?)`);
      failures++;
      continue;
    }
    let result;
    try {
      result = JSON.parse(unescapeHtml(m[1]));
    } catch (e) {
      console.error(`[canvas-geometry] ${fixture.name}: bad measurement JSON`);
      failures++;
      continue;
    }
    let fixtureFailed = false;
    if (result.overlaps.length > 0) {
      console.error(`[canvas-geometry] ✗ ${fixture.name}: ${result.overlaps.length} overlap(s) of ${result.count} nodes`);
      for (const o of result.overlaps) {
        console.error(`    ${o}`);
      }
      fixtureFailed = true;
    }
    const renderFailures = Array.isArray(result.renderFailures) ? result.renderFailures : [];
    if (renderFailures.length > 0) {
      console.error(`[canvas-geometry] ✗ ${fixture.name}: ${renderFailures.length} rendered contract failure(s)`);
      for (const failure of renderFailures) {
        console.error(`    ${failure}`);
      }
      fixtureFailed = true;
    }
    if (fixtureFailed) {
      failures++;
    } else {
      console.log(`[canvas-geometry] ✓ ${fixture.name}: no overlaps (${result.count} nodes)`);
      if (fixture.renderContractRuntimeId) {
        console.log(`[canvas-geometry] ✓ ${fixture.name}: rendered status contract`);
      }
    }
  }
  fs.rmSync(tmp, { recursive: true, force: true });
  if (failures > 0) {
    console.error(`[canvas-geometry] FAILED (${failures} fixture(s))`);
    process.exit(1);
  }
  console.log("[canvas-geometry] OK");
}

main();
