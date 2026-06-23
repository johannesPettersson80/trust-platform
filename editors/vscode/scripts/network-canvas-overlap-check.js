#!/usr/bin/env node
// Geometry gate for the React Flow Network Canvas: renders the built webview bundle in headless
// Chromium across several fixtures (incl. Edit mode and a dense fleet), measures every
// `.react-flow__node` rect, and FAILS if any two nodes PARTIALLY overlap (parent⊃child nesting is
// fine; partial overlap is a bug). Replaces the deleted HTML-view gate. Catches both layout-math
// and render/position-merge overlaps (e.g. the 2026-06-18 edit-mode SCADA overlap).
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
];

// Runs inside the page: flag pairs that overlap where NEITHER contains the other.
const MEASURE = `
function __measure(){
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
  var pre=document.createElement('pre');pre.id='nc-geom-result';
  pre.textContent=JSON.stringify({count:items.length,overlaps:bad});
  document.body.appendChild(pre);
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
setTimeout(function(){ ${fixture.edit ? '__click("Edit");' : ""} setTimeout(__measure,1100); },800);
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
    console.warn("[canvas-geometry] SKIP — no Chromium found (set CHROME=/path). Geometry not gated here.");
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
    if (result.overlaps.length > 0) {
      console.error(`[canvas-geometry] ✗ ${fixture.name}: ${result.overlaps.length} overlap(s) of ${result.count} nodes`);
      for (const o of result.overlaps) {
        console.error(`    ${o}`);
      }
      failures++;
    } else {
      console.log(`[canvas-geometry] ✓ ${fixture.name}: no overlaps (${result.count} nodes)`);
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
