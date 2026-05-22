#!/usr/bin/env node
import { createRequire } from "node:module";
import fs from "node:fs/promises";
import http from "node:http";
import path from "node:path";

const require = createRequire(import.meta.url);
const { chromium: playwrightBrowser } = require("./captures/node_modules/playwright");

const root = process.cwd();
const artifactDir = path.join(root, "target/gate-artifacts");
const tracePath = path.join(artifactDir, "world_smoke_trace.json");
const htmlPath = path.join(artifactDir, "world_smoke_renderer.html");
const screenshotInitial = path.join(artifactDir, "world_smoke_initial.png");
const screenshotGrip = path.join(artifactDir, "world_smoke_grip.png");
const screenshotCarry = path.join(artifactDir, "world_smoke_carry.png");
const screenshotFinal = path.join(artifactDir, "world_smoke_final.png");

const trace = JSON.parse(await fs.readFile(tracePath, "utf8"));
assertTraceReady(trace);
await fs.mkdir(artifactDir, { recursive: true });
await fs.writeFile(htmlPath, htmlSource(), "utf8");

const server = await startStaticServer(root);
let browser;
try {
  const url = `http://127.0.0.1:${server.port}/target/gate-artifacts/world_smoke_renderer.html`;
  browser = await playwrightBrowser.launch({
    headless: true,
    args: [
      "--enable-webgl",
      "--ignore-gpu-blocklist",
      "--use-gl=angle",
      "--use-angle=swiftshader",
    ],
  });
  const page = await browser.newPage({ viewport: { width: 960, height: 640 } });
  const browserErrors = [];
  page.on("console", (message) => {
    if (message.type() === "error") {
      browserErrors.push(message.text());
    }
  });
  page.on("pageerror", (error) => browserErrors.push(error.message));
  await page.goto(url, { waitUntil: "networkidle" });
  await page.waitForFunction(() => typeof window.__worldSmokeInit === "function", null, {
    timeout: 15_000,
  });
  const origin = await page.evaluate(async (scene) => window.__worldSmokeInit(scene), scenePayload());
  if (origin !== "scena_webgl" && origin !== "scena_webgpu") {
    throw new Error(`renderer_origin must be scena_webgl or scena_webgpu, got ${origin}`);
  }
  const frames = selectFrames(trace);
  await renderAt(page, frames.initial);
  await page.locator("#scene").screenshot({ path: screenshotInitial });
  await renderAt(page, frames.grip);
  await page.locator("#scene").screenshot({ path: screenshotGrip });
  await renderAt(page, frames.carry);
  await page.locator("#scene").screenshot({ path: screenshotCarry });
  await renderAt(page, frames.release);
  await page.locator("#scene").screenshot({ path: screenshotFinal });
  const fatalBrowserErrors = browserErrors.filter((message) =>
    /webgl|webgpu|wgpu|validation|trust-twin renderer failed/i.test(message)
  );
  if (fatalBrowserErrors.length > 0) {
    throw new Error(`browser renderer reported errors:\n${fatalBrowserErrors.join("\n")}`);
  }
  trace.renderer_origin = origin;
  trace.screenshot_initial_png = "target/gate-artifacts/world_smoke_initial.png";
  trace.screenshot_grip_png = "target/gate-artifacts/world_smoke_grip.png";
  trace.screenshot_carry_png = "target/gate-artifacts/world_smoke_carry.png";
  trace.screenshot_final_png = "target/gate-artifacts/world_smoke_final.png";
  await fs.writeFile(tracePath, `${JSON.stringify(trace, null, 2)}\n`, "utf8");
  console.log(`world smoke rendered with renderer_origin=${origin}`);
} finally {
  if (browser) {
    await browser.close();
  }
  await new Promise((resolve) => server.instance.close(resolve));
}

async function renderAt(page, tick) {
  await page.evaluate(
    async (positions) => window.__worldSmokeRender(positions),
    {
      carrier: tick.carrier.center,
      workpiece: tick.workpiece.center,
    },
  );
  await page.waitForTimeout(100);
}

function assertTraceReady(value) {
  if (value.world_abstraction?.type_name !== "World") {
    throw new Error("world_smoke_trace.json does not contain a World abstraction trace");
  }
  for (const [name, assertion] of Object.entries(value.assertions ?? {})) {
    if (assertion && typeof assertion === "object" && "ok" in assertion && assertion.ok !== true) {
      throw new Error(`world smoke assertion ${name} is not true`);
    }
  }
  const required = [
    "workpiece_above_floor",
    "carrier_above_floor",
    "no_fixture_interpenetration",
    "grip_event_has_contact",
    "carry_constraint_driven",
    "release_destroyed_joint",
    "workpiece_settled_on_fixture",
  ];
  for (const name of required) {
    if (value.assertions?.[name]?.ok !== true) {
      throw new Error(`P1 assertion ${name} is not true`);
    }
  }
  if (!Array.isArray(value.per_tick_trace) || value.per_tick_trace.length < 2) {
    throw new Error("world_smoke_trace.json has no usable per_tick_trace");
  }
  if (!value.per_tick_trace.every((tick) => tick.carrier?.center && tick.workpiece?.center)) {
    throw new Error("world_smoke_trace.json does not contain P1 carrier/workpiece positions");
  }
}

function selectFrames(value) {
  const initial = value.per_tick_trace[0];
  const grip = value.per_tick_trace.find((tick) => tick.actuator_state === "Carrying" && hasContact(tick, "carrier", "workpiece"));
  const releaseTransition = value.per_tick_trace.find((tick) => tick.actuator_state === "Releasing");
  const active = value.per_tick_trace.filter((tick) => tick.active_joints?.length > 0);
  const carry = active[Math.floor(active.length / 2)];
  const release = value.per_tick_trace[value.per_tick_trace.length - 1];
  if (!initial || !grip || !carry || !releaseTransition || !release) {
    throw new Error("trace does not contain initial/grip/carry/release frames");
  }
  return { initial, grip, carry, release };
}

function hasContact(tick, a, b) {
  return (tick.contacts ?? []).some((contact) =>
    (contact.a === a && contact.b === b) || (contact.a === b && contact.b === a)
  );
}

function scenePayload() {
  return {
    render: {
      background: "#101827",
      auto_exposure: "off",
    },
    floor: {
      enabled: true,
      floor_y: 0.0,
      bounds_min: [-3.0, 0.0, -3.0],
      bounds_max: [3.0, 2.8, 3.0],
      padding: 0.5,
      line_spacing: 0.5,
      color: "#273449",
      line_color: "#5e718f",
      roughness: 0.85,
    },
    node: [
      {
        id: "floor",
        primitive: "box",
        local_position: [0.0, 0.0, 0.0],
        transform: { scale: [6.0, 0.1, 6.0] },
        material: { base_color: "#2f3b4f" },
      },
      {
        id: "fixture",
        primitive: "box",
        local_position: [2.0, 0.3, 0.0],
        transform: { scale: [1.5, 0.5, 1.5] },
        material: { base_color: "#64748b", emissive: "#000000", opacity: 1.0 },
      },
      {
        id: "workpiece",
        primitive: "cube",
        local_position: [0.0, 0.3, 0.0],
        transform: { scale: [0.5, 0.5, 0.5] },
        material: { base_color: "#f97316", emissive: "#000000", opacity: 1.0 },
      },
      {
        id: "carrier",
        primitive: "box",
        local_position: [0.0, 1.4, 0.0],
        transform: { scale: [0.9, 0.3, 0.9] },
        material: { base_color: "#38bdf8", emissive: "#000000", opacity: 1.0 },
      },
      {
        id: "carrier-tool",
        parent: "carrier",
        primitive: "cube",
        local_position: [0.0, -0.22, 0.0],
        transform: { scale: [0.16, 0.12, 0.16] },
        material: { base_color: "#facc15", emissive: "#000000", opacity: 1.0 },
      },
    ],
    camera: [
      {
        id: "main",
        kind: "perspective",
        lens: "standard",
        position: [4.5, 3.0, 6.5],
        target: [1.0, 0.8, 0.0],
        fov_degrees: 58.0,
      },
    ],
    light: [
      { kind: "directional", position: [2.0, 5.0, 3.0], intensity: 1.2 },
    ],
    bind3d: [
      {
        node: "workpiece",
        property: "transform.position",
        source: "World.WorkpiecePosition",
      },
      {
        node: "carrier",
        property: "transform.position",
        source: "World.CarrierPosition",
      },
    ],
  };
}

function htmlSource() {
  return `<!doctype html>
<html>
<head>
  <meta charset="utf-8">
  <title>world smoke proof</title>
  <style>
    html, body { margin: 0; width: 100%; height: 100%; background: #101827; }
    #scene { display: block; width: 960px; height: 640px; }
  </style>
</head>
<body>
  <canvas id="scene" width="960" height="640"></canvas>
  <script type="module">
    import initWasm, { init, apply_scene, apply_values, render_frame, renderer_origin } from "/editors/vscode/media/trust-twin/trust-twin-renderer.js";
    let handle = null;
    window.__worldSmokeInit = async (scene) => {
      await initWasm();
      const canvas = document.getElementById("scene");
      handle = await init(canvas);
      await apply_scene(handle, JSON.stringify(scene));
      const origin = renderer_origin(handle);
      window.__trustTwinRendererOrigin = origin;
      return origin;
    };
    window.__worldSmokeRender = async (positions) => {
      if (!handle) throw new Error("world smoke renderer not initialized");
      apply_values(handle, JSON.stringify({
        "World.CarrierPosition": positions.carrier,
        "World.WorkpiecePosition": positions.workpiece,
      }));
      render_frame(handle);
      await new Promise((resolve) => requestAnimationFrame(() => requestAnimationFrame(resolve)));
    };
  </script>
</body>
</html>
`;
}

async function startStaticServer(baseDir) {
  const instance = http.createServer(async (request, response) => {
    try {
      const url = new URL(request.url ?? "/", "http://127.0.0.1");
      const filePath = path.normalize(path.join(baseDir, decodeURIComponent(url.pathname)));
      if (!filePath.startsWith(baseDir)) {
        response.writeHead(403);
        response.end("forbidden");
        return;
      }
      const body = await fs.readFile(filePath);
      response.writeHead(200, { "content-type": contentType(filePath) });
      response.end(body);
    } catch (error) {
      response.writeHead(404);
      response.end(String(error));
    }
  });
  await new Promise((resolve) => instance.listen(0, "127.0.0.1", resolve));
  return { instance, port: instance.address().port };
}

function contentType(filePath) {
  if (filePath.endsWith(".html")) return "text/html; charset=utf-8";
  if (filePath.endsWith(".js")) return "application/javascript; charset=utf-8";
  if (filePath.endsWith(".wasm")) return "application/wasm";
  if (filePath.endsWith(".png")) return "image/png";
  if (filePath.endsWith(".json")) return "application/json; charset=utf-8";
  return "application/octet-stream";
}
