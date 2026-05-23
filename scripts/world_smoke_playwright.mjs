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
const screenshotTransfer = path.join(artifactDir, "world_smoke_transfer.png");
const screenshotHandoff = path.join(artifactDir, "world_smoke_handoff.png");
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
  if (frames.carry) {
    await renderAt(page, frames.carry);
    await page.locator("#scene").screenshot({ path: screenshotCarry });
  }
  if (frames.transfer) {
    await renderAt(page, frames.transfer);
    await page.locator("#scene").screenshot({ path: screenshotTransfer });
  }
  if (frames.handoff) {
    await renderAt(page, frames.handoff);
    await page.locator("#scene").screenshot({ path: screenshotHandoff });
  }
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
  if (frames.carry) {
    trace.screenshot_carry_png = "target/gate-artifacts/world_smoke_carry.png";
  }
  if (frames.transfer) {
    trace.screenshot_transfer_png = "target/gate-artifacts/world_smoke_transfer.png";
  }
  if (frames.handoff) {
    trace.screenshot_handoff_png = "target/gate-artifacts/world_smoke_handoff.png";
  }
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
  const positions = tick.arm_a_links?.length || tick.arm_b_links?.length
    ? {
        arm_a_link_1: armInstanceLink(tick, "arm_a", "link_1")?.rapier_position,
        arm_a_link_2: armInstanceLink(tick, "arm_a", "link_2")?.rapier_position,
        arm_a_tool: armInstanceLink(tick, "arm_a", "tool")?.rapier_position,
        arm_b_link_1: armInstanceLink(tick, "arm_b", "link_1")?.rapier_position,
        arm_b_link_2: armInstanceLink(tick, "arm_b", "link_2")?.rapier_position,
        arm_b_tool: armInstanceLink(tick, "arm_b", "tool")?.rapier_position,
        workpiece: tick.workpiece.center,
      }
    : tick.arm_links?.length
    ? {
        arm_link_1: armLink(tick, "link_1")?.rapier_position,
        arm_link_2: armLink(tick, "link_2")?.rapier_position,
        arm_tool: armLink(tick, "tool")?.rapier_position,
        workpiece: tick.workpiece.center,
      }
    : tick.carrier_a?.center
    ? {
        carrier_a: tick.carrier_a.center,
        carrier_b: tick.carrier_b.center,
        workpiece: tick.workpiece.center,
      }
    : {
        carrier: tick.carrier.center,
        workpiece: tick.workpiece.center,
      };
  await page.evaluate(
    async (positions) => window.__worldSmokeRender(positions),
    positions,
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
  const p1Required = [
    "workpiece_above_floor",
    "carrier_above_floor",
    "no_fixture_interpenetration",
    "grip_event_has_contact",
    "carry_constraint_driven",
    "release_destroyed_joint",
    "workpiece_settled_on_fixture",
  ];
  const p2Required = [
    "exclusive_ownership",
    "ownership_transfer_atomic",
    "handoff_order_deterministic",
    "no_phantom_carry",
    "determinism_hash_stable",
  ];
  const p3Required = [
    "urdf_parsed_once",
    "arm_rendered_through_handoff",
    "fk_matches_rapier",
    "joint_limits_enforced",
    "arm_links_above_floor",
    "determinism_hash_stable",
  ];
  const p4Required = ["multi_urdf_arms_loaded", "per_arm_fk_consistency"];
  const isP4 = (value.urdf?.instances ?? []).length >= 2;
  const required = isP4
    ? [...p1Required, ...p2Required, ...p3Required, ...p4Required]
    : value.urdf
    ? [...p1Required, ...p3Required]
    : value.actuators
    ? [...p1Required, ...p2Required]
    : p1Required;
  for (const name of required) {
    if (value.assertions?.[name]?.ok !== true) {
      throw new Error(`world smoke assertion ${name} is not true`);
    }
  }
  if (!Array.isArray(value.per_tick_trace) || value.per_tick_trace.length < 2) {
    throw new Error("world_smoke_trace.json has no usable per_tick_trace");
  }
  const hasWorkpiecePositions = value.per_tick_trace.every((tick) => tick.workpiece?.center);
  const hasP2Positions = value.per_tick_trace.every((tick) =>
    tick.carrier_a?.center && tick.carrier_b?.center
  );
  const hasP4Positions = value.per_tick_trace.every((tick) =>
    armInstanceLink(tick, "arm_a", "link_1")
      && armInstanceLink(tick, "arm_a", "link_2")
      && armInstanceLink(tick, "arm_a", "tool")
      && armInstanceLink(tick, "arm_b", "link_1")
      && armInstanceLink(tick, "arm_b", "link_2")
      && armInstanceLink(tick, "arm_b", "tool")
  );
  const hasP3Positions = value.per_tick_trace.every((tick) =>
    armLink(tick, "link_1") && armLink(tick, "link_2") && armLink(tick, "tool")
  );
  const hasP1Positions = value.per_tick_trace.every((tick) => tick.carrier?.center);
  if (!hasWorkpiecePositions) {
    throw new Error("world_smoke_trace.json does not contain workpiece positions for every tick");
  }
  if (!hasP1Positions && !hasP2Positions && !hasP3Positions && !hasP4Positions) {
    throw new Error("world_smoke_trace.json does not contain supported world-smoke positions");
  }
}

function selectFrames(value) {
  const isP4 = (value.urdf?.instances ?? []).length >= 2;
  if (isP4) {
    const initial = value.per_tick_trace[0];
    const grip = value.per_tick_trace.find((tick) =>
      tick.tick_events?.includes("joint_create(arm_a.tool, workpiece)") && hasContact(tick, "arm_a.tool", "workpiece")
    );
    const transfer = value.per_tick_trace.find((tick) =>
      stateIs(tick, "arm_a", "Held") && stateIs(tick, "arm_b", "AcceptingHandoff")
    );
    const handoffTick = value.handoff_plan?.atomic_tick;
    const handoff = value.per_tick_trace.find((tick) => tick.tick === handoffTick + 1);
    const release = value.per_tick_trace[value.per_tick_trace.length - 1];
    if (!initial || !grip || !transfer || !handoff || !release) {
      throw new Error("trace does not contain P4 initial/grip/transfer/handoff/final frames");
    }
    return { initial, grip, transfer, handoff, release };
  }
  if (value.urdf) {
    const initial = value.per_tick_trace[0];
    const grip = value.per_tick_trace.find((tick) =>
      tick.tick_events?.includes("joint_create(arm.tool, workpiece)") && hasContact(tick, "arm.tool", "workpiece")
    );
    const active = value.per_tick_trace.filter((tick) => tick.active_joints?.includes("fixed(arm.tool, workpiece_grip)"));
    const carry = active[Math.floor(active.length / 2)];
    const release = value.per_tick_trace[value.per_tick_trace.length - 1];
    if (!initial || !grip || !carry || !release) {
      throw new Error("trace does not contain P3 initial/grip/carry/final frames");
    }
    return { initial, grip, carry, release };
  }
  if (value.actuators) {
    const initial = value.per_tick_trace[0];
    const grip = value.per_tick_trace.find((tick) =>
      tick.tick_events?.includes("joint_create(carrier_a, workpiece)")
    );
    const transfer = value.per_tick_trace.find((tick) =>
      stateIs(tick, "carrier_a", "Held") && stateIs(tick, "carrier_b", "AcceptingHandoff")
    );
    const handoffTick = value.handoff_plan?.atomic_tick;
    const handoff = value.per_tick_trace.find((tick) => tick.tick === handoffTick + 1);
    const release = value.per_tick_trace[value.per_tick_trace.length - 1];
    if (!initial || !grip || !transfer || !handoff || !release) {
      throw new Error("trace does not contain P2 initial/grip/transfer/handoff/final frames");
    }
    return { initial, grip, transfer, handoff, release };
  }
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

function stateIs(tick, name, state) {
  return (tick.actuator_states ?? []).some((sample) => sample.name === name && sample.state === state);
}

function hasContact(tick, a, b) {
  return (tick.contacts ?? []).some((contact) =>
    (contact.a === a && contact.b === b) || (contact.a === b && contact.b === a)
  );
}

function armLink(tick, name) {
  return (tick.arm_links ?? []).find((link) => link.name === name);
}

function armInstanceLink(tick, arm, name) {
  const links = arm === "arm_a" ? tick.arm_a_links : tick.arm_b_links;
  return (links ?? []).find((link) => link.name === name);
}

function scenePayload() {
  const isP4 = (trace.urdf?.instances ?? []).length >= 2;
  const isP3 = Boolean(trace.urdf) && !isP4;
  const isP2 = Boolean(trace.actuators) && !isP4;
  const nodes = [
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
    ...(isP4
      ? [
          {
            id: "transfer_zone",
            primitive: "box",
            local_position: [1.8, 0.07, 0.0],
            transform: { scale: [0.36, 0.04, 0.36] },
            material: { base_color: "#facc15", emissive: "#000000", opacity: 1.0 },
          },
          {
            id: "arm_a_base",
            primitive: "box",
            local_position: [0.3, 0.85, 0.0],
            transform: { scale: [0.3, 0.3, 0.3] },
            material: { base_color: "#94a3b8", emissive: "#000000", opacity: 1.0 },
          },
          {
            id: "arm_a_link_1",
            primitive: "box",
            local_position: [0.0, 0.0, 0.0],
            transform: { scale: [0.9, 0.12, 0.16] },
            material: { base_color: "#38bdf8", emissive: "#000000", opacity: 1.0 },
          },
          {
            id: "arm_a_link_2",
            primitive: "box",
            local_position: [0.0, 0.0, 0.0],
            transform: { scale: [0.9, 0.10, 0.14] },
            material: { base_color: "#22c55e", emissive: "#000000", opacity: 1.0 },
          },
          {
            id: "arm_a_tool",
            primitive: "box",
            local_position: [0.0, 0.0, 0.0],
            transform: { scale: [0.4, 0.2, 0.2] },
            material: { base_color: "#facc15", emissive: "#000000", opacity: 1.0 },
          },
          {
            id: "arm_b_base",
            primitive: "box",
            local_position: [1.4, 0.85, 0.0],
            transform: { scale: [0.3, 0.3, 0.3] },
            material: { base_color: "#c4b5fd", emissive: "#000000", opacity: 1.0 },
          },
          {
            id: "arm_b_link_1",
            primitive: "box",
            local_position: [0.0, 0.0, 0.0],
            transform: { scale: [0.9, 0.12, 0.16] },
            material: { base_color: "#a78bfa", emissive: "#000000", opacity: 1.0 },
          },
          {
            id: "arm_b_link_2",
            primitive: "box",
            local_position: [0.0, 0.0, 0.0],
            transform: { scale: [0.9, 0.10, 0.14] },
            material: { base_color: "#f472b6", emissive: "#000000", opacity: 1.0 },
          },
          {
            id: "arm_b_tool",
            primitive: "box",
            local_position: [0.0, 0.0, 0.0],
            transform: { scale: [0.4, 0.2, 0.2] },
            material: { base_color: "#fbbf24", emissive: "#000000", opacity: 1.0 },
          },
        ]
      : isP3
      ? [
          {
            id: "arm_base",
            primitive: "box",
            local_position: [0.3, 0.85, 0.0],
            transform: { scale: [0.3, 0.3, 0.3] },
            material: { base_color: "#94a3b8", emissive: "#000000", opacity: 1.0 },
          },
          {
            id: "arm_link_1",
            primitive: "box",
            local_position: [0.0, 0.0, 0.0],
            transform: { scale: [0.9, 0.12, 0.16] },
            material: { base_color: "#38bdf8", emissive: "#000000", opacity: 1.0 },
          },
          {
            id: "arm_link_2",
            primitive: "box",
            local_position: [0.0, 0.0, 0.0],
            transform: { scale: [0.9, 0.10, 0.14] },
            material: { base_color: "#22c55e", emissive: "#000000", opacity: 1.0 },
          },
          {
            id: "arm_tool",
            primitive: "box",
            local_position: [0.0, 0.0, 0.0],
            transform: { scale: [0.4, 0.2, 0.2] },
            material: { base_color: "#facc15", emissive: "#000000", opacity: 1.0 },
          },
        ]
      : isP2
      ? [
          {
            id: "transfer_zone",
            primitive: "box",
            local_position: [1.0, 0.08, 0.0],
            transform: { scale: [0.5, 0.04, 0.5] },
            material: { base_color: "#facc15", emissive: "#000000", opacity: 1.0 },
          },
          {
            id: "carrier_a",
            primitive: "box",
            local_position: [0.0, 1.4, 0.0],
            transform: { scale: [0.9, 0.3, 0.9] },
            material: { base_color: "#38bdf8", emissive: "#000000", opacity: 1.0 },
          },
          {
            id: "carrier_a_tool",
            parent: "carrier_a",
            primitive: "cube",
            local_position: [0.0, -0.22, 0.0],
            transform: { scale: [0.16, 0.12, 0.16] },
            material: { base_color: "#facc15", emissive: "#000000", opacity: 1.0 },
          },
          {
            id: "carrier_b",
            primitive: "box",
            local_position: [1.0, 1.8, 0.7],
            transform: { scale: [0.9, 0.3, 0.9] },
            material: { base_color: "#a78bfa", emissive: "#000000", opacity: 1.0 },
          },
          {
            id: "carrier_b_tool",
            parent: "carrier_b",
            primitive: "cube",
            local_position: [0.0, 0.0, -0.52],
            transform: { scale: [0.16, 0.16, 0.12] },
            material: { base_color: "#fde047", emissive: "#000000", opacity: 1.0 },
          },
        ]
      : [
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
        ]),
  ];
  const bindings = [
    {
      node: "workpiece",
      property: "transform.position",
      source: "World.WorkpiecePosition",
    },
    ...(isP4
      ? [
          {
            node: "arm_a_link_1",
            property: "transform.position",
            source: "World.ArmALink1Position",
          },
          {
            node: "arm_a_link_2",
            property: "transform.position",
            source: "World.ArmALink2Position",
          },
          {
            node: "arm_a_tool",
            property: "transform.position",
            source: "World.ArmAToolPosition",
          },
          {
            node: "arm_b_link_1",
            property: "transform.position",
            source: "World.ArmBLink1Position",
          },
          {
            node: "arm_b_link_2",
            property: "transform.position",
            source: "World.ArmBLink2Position",
          },
          {
            node: "arm_b_tool",
            property: "transform.position",
            source: "World.ArmBToolPosition",
          },
        ]
      : isP3
      ? [
          {
            node: "arm_link_1",
            property: "transform.position",
            source: "World.ArmLink1Position",
          },
          {
            node: "arm_link_2",
            property: "transform.position",
            source: "World.ArmLink2Position",
          },
          {
            node: "arm_tool",
            property: "transform.position",
            source: "World.ArmToolPosition",
          },
        ]
      : isP2
      ? [
          {
            node: "carrier_a",
            property: "transform.position",
            source: "World.CarrierAPosition",
          },
          {
            node: "carrier_b",
            property: "transform.position",
            source: "World.CarrierBPosition",
          },
        ]
      : [
          {
            node: "carrier",
            property: "transform.position",
            source: "World.CarrierPosition",
          },
        ]),
  ];
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
    node: nodes,
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
    bind3d: bindings,
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
        "World.ArmLink1Position": positions.arm_link_1,
        "World.ArmLink2Position": positions.arm_link_2,
        "World.ArmToolPosition": positions.arm_tool,
        "World.ArmALink1Position": positions.arm_a_link_1,
        "World.ArmALink2Position": positions.arm_a_link_2,
        "World.ArmAToolPosition": positions.arm_a_tool,
        "World.ArmBLink1Position": positions.arm_b_link_1,
        "World.ArmBLink2Position": positions.arm_b_link_2,
        "World.ArmBToolPosition": positions.arm_b_tool,
        "World.CarrierAPosition": positions.carrier_a,
        "World.CarrierBPosition": positions.carrier_b,
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
