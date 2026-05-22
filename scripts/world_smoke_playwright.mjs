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
const screenshotT0 = path.join(artifactDir, "world_smoke_t0.png");
const screenshotTN = path.join(artifactDir, "world_smoke_tN.png");

const trace = JSON.parse(await fs.readFile(tracePath, "utf8"));
assertTraceReady(trace);
await fs.mkdir(artifactDir, { recursive: true });
await fs.writeFile(htmlPath, htmlSource(), "utf8");

const server = await startStaticServer(root);
let browser;
try {
  const url = `http://127.0.0.1:${server.port}/target/gate-artifacts/world_smoke_renderer.html`;
  browser = await playwrightBrowser.launch({ headless: true });
  const page = await browser.newPage({ viewport: { width: 960, height: 640 } });
  const browserErrors = [];
  page.on("console", (message) => {
    if (message.type() === "error") {
      browserErrors.push(message.text());
    }
  });
  page.on("pageerror", (error) => browserErrors.push(error.message));
  await page.goto(url, { waitUntil: "networkidle" });
  const origin = await page.evaluate(async (scene) => window.__worldSmokeInit(scene), scenePayload());
  if (origin !== "scena_webgl" && origin !== "scena_webgpu") {
    throw new Error(`renderer_origin must be scena_webgl or scena_webgpu, got ${origin}`);
  }
  const first = trace.per_tick_trace[0];
  const last = trace.per_tick_trace[trace.per_tick_trace.length - 1];
  await renderAt(page, first);
  await page.locator("#scene").screenshot({ path: screenshotT0 });
  await renderAt(page, last);
  await page.locator("#scene").screenshot({ path: screenshotTN });
  const fatalBrowserErrors = browserErrors.filter((message) =>
    /webgl|webgpu|wgpu|validation|trust-twin renderer failed/i.test(message)
  );
  if (fatalBrowserErrors.length > 0) {
    throw new Error(`browser renderer reported errors:\n${fatalBrowserErrors.join("\n")}`);
  }
  trace.renderer_origin = origin;
  trace.screenshot_t0_png = "target/gate-artifacts/world_smoke_t0.png";
  trace.screenshot_t_n_png = "target/gate-artifacts/world_smoke_tN.png";
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
    async (position) => window.__worldSmokeRender(position),
    [0.0, tick.cube_center_y, 0.0],
  );
  await page.waitForTimeout(100);
}

function assertTraceReady(value) {
  if (value.world_abstraction?.type_name !== "World") {
    throw new Error("world_smoke_trace.json does not contain a World abstraction trace");
  }
  for (const [name, assertion] of Object.entries(value.assertions ?? {})) {
    if (assertion?.ok !== true) {
      throw new Error(`world smoke assertion ${name} is not true`);
    }
  }
  if (!Array.isArray(value.per_tick_trace) || value.per_tick_trace.length < 2) {
    throw new Error("world_smoke_trace.json has no usable per_tick_trace");
  }
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
        id: "floor-solid",
        primitive: "box",
        local_position: [0.0, 0.0, 0.0],
        transform: { scale: [6.0, 0.1, 6.0] },
        material: { base_color: "#2f3b4f", roughness: 0.8 },
      },
      {
        id: "cube",
        primitive: "cube",
        local_position: [0.0, 2.5, 0.0],
        transform: { scale: [1.0, 1.0, 1.0] },
        material: { base_color: "#f97316", emissive: "#000000", opacity: 1.0 },
      },
    ],
    camera: [
      {
        id: "main",
        kind: "perspective",
        lens: "standard",
        position: [4.0, 3.2, 7.0],
        target: [0.0, 1.2, 0.0],
        fov_degrees: 58.0,
      },
    ],
    light: [
      { kind: "directional", position: [2.0, 5.0, 3.0], intensity: 1.2 },
    ],
    bind3d: [
      {
        node: "cube",
        property: "transform.position",
        source: "World.CubePosition",
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
    window.__worldSmokeRender = async (position) => {
      if (!handle) throw new Error("world smoke renderer not initialized");
      apply_values(handle, JSON.stringify({ "World.CubePosition": position }));
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
