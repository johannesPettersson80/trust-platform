import { spawnSync } from "node:child_process";
import { createHash } from "node:crypto";
import fs from "node:fs/promises";
import { existsSync } from "node:fs";
import { createRequire } from "node:module";
import Module from "node:module";
import path from "node:path";
import { fileURLToPath, pathToFileURL } from "node:url";

import { chromium } from "./captures/node_modules/playwright/index.mjs";

const repoRoot = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "..");
const artifactDir = path.join(repoRoot, "target", "gate-artifacts");
const artifactPath = path.join(artifactDir, "trust-twin-robot-cell-motion.json");
const htmlPath = path.join(artifactDir, "trust-twin-robot-cell-production-webview.html");
const beforePng = path.join(artifactDir, "trust-twin-robot-cell-before.png");
const closedPng = path.join(artifactDir, "trust-twin-robot-cell-closed-grip.png");
const afterPng = path.join(artifactDir, "trust-twin-robot-cell-after.png");
const stalePng = path.join(artifactDir, "trust-twin-robot-cell-stale.png");
const viewPath = path.join(
  repoRoot,
  "examples",
  "trust-twin",
  "robot-cell",
  "hmi",
  "views",
  "robot-cell.view.toml",
);
const panelOutPath = path.join(repoRoot, "editors", "vscode", "out", "trustTwinPanel.js");
const extensionRoot = path.join(repoRoot, "editors", "vscode");

const command = "node scripts/trust_twin_robot_cell_playwright.mjs";
const artifact = JSON.parse(await fs.readFile(artifactPath, "utf8"));
const sceneView = parseToml(viewPath);
const samples = artifact.trace_samples || [];
if (!Array.isArray(samples) || samples.length < 5) {
  throw new Error(`${relative(artifactPath)} must contain at least five trace_samples`);
}

const beforeSample = sampleByStep(samples, 1);
const closedSample = sampleByStep(samples, 2);
const afterSample = sampleByStep(samples, 5);
const staleSample = sampleByStep(samples, 4);

await fs.mkdir(artifactDir, { recursive: true });
const html = loadProductionPanelHtml();
await fs.writeFile(htmlPath, html, "utf8");

const browser = await chromium.launch({ headless: true });
const page = await browser.newPage({ viewport: { width: 1200, height: 760 }, deviceScaleFactor: 1 });
const started = performance.now();
const browserMessages = [];
page.on("console", (message) => browserMessages.push(`console:${message.type()}: ${message.text()}`));
page.on("pageerror", (error) => browserMessages.push(`pageerror: ${error.message}`));

try {
  await page.addInitScript(() => {
    window.__trustTwinMessages = [];
    window.acquireVsCodeApi = () => ({
      postMessage(message) {
        window.__trustTwinMessages.push(message);
      },
      getState() {
        return undefined;
      },
      setState() {},
    });
  });
  await page.goto(pathToFileURL(htmlPath).href, { waitUntil: "domcontentloaded" });
  await page.evaluate(() => {
    const theme = {
      "--vscode-font-family": "Arial, sans-serif",
      "--vscode-editor-foreground": "#111827",
      "--vscode-editor-background": "#f8fafc",
      "--vscode-panel-border": "#94a3b8",
      "--vscode-button-background": "#1d4ed8",
      "--vscode-button-foreground": "#ffffff",
      "--vscode-button-secondaryBackground": "#e2e8f0",
      "--vscode-button-secondaryForeground": "#111827",
      "--vscode-focusBorder": "#2563eb",
      "--vscode-inputValidation-errorBorder": "#dc2626",
      "--vscode-inputValidation-errorBackground": "#fee2e2",
      "--vscode-inputValidation-errorForeground": "#991b1b",
    };
    for (const [name, value] of Object.entries(theme)) {
      document.documentElement.style.setProperty(name, value);
    }
  });
  await page.waitForSelector("main#surface", { timeout: 10_000 });

  const before = await renderAndCapture(page, beforeSample, true, beforePng);
  const closed = await renderAndCapture(page, closedSample, true, closedPng);
  const after = await renderAndCapture(page, afterSample, true, afterPng);
  const liveBeforeStale = await renderScene(page, staleSample, true);
  const stale = await renderAndCapture(page, staleSample, false, stalePng);
  const fps = await measureRenderFps(page, samples);
  const elapsedMs = performance.now() - started;
  const diff = imageDiff(beforePng, afterPng);

  const checks = {
    box_on_pickup_surface: assertBoxRestsOnSurface(before.rects, "BOX-1", "PICKUP-1"),
    closed_grip_around_box: assertClosedGripAroundBox(closed.rects),
    wrist_above_floor_after: assertToolingAboveFloor(after.rects),
    stale_freezes_pose: assertStaleFreezesPose(liveBeforeStale, stale),
  };

  if (diff.pixel_difference_count < 2500) {
    throw new Error(`robot-cell production-webview frame diff too small: ${diff.pixel_difference_count}`);
  }

  const beforeBytes = await fs.readFile(beforePng);
  const closedBytes = await fs.readFile(closedPng);
  const afterBytes = await fs.readFile(afterPng);
  const staleBytes = await fs.readFile(stalePng);

  artifact.renderer_origin = "production_webview";
  artifact.frame_hashes_before_after = {
    before: sha256(beforeBytes),
    closed_grip: sha256(closedBytes),
    after: sha256(afterBytes),
    stale: sha256(staleBytes),
  };
  artifact.pixel_difference_count = diff.pixel_difference_count;
  artifact.screenshot_video_path = {
    html: relative(htmlPath),
    before_png: relative(beforePng),
    closed_grip_png: relative(closedPng),
    after_png: relative(afterPng),
    stale_png: relative(stalePng),
  };
  artifact.playwright = {
    command,
    result: "ok",
    browser: await browser.version(),
    renderer_origin: "production_webview",
    html_source: "editors/vscode/src/trustTwinPanel.ts",
  };
  artifact.fps_latency = {
    fps: Math.round(fps * 10) / 10,
    latency_ms: Math.round(elapsedMs * 10) / 10,
  };
  artifact.disconnected_state_result = "ok";
  artifact.visual_motion_checks = checks;
  // assistant_visual_verdict MUST come from a real visual-review call against
  // the captured PNGs. It is intentionally NOT set here. The previous
  // implementation wrote a hardcoded approval string into this field on every
  // run regardless of what the screenshots showed; that field is now absent
  // until a real review hook is wired in.
  //
  // evidence_blockers is appended to, never stripped. Blockers must be cleared
  // by passing their actual check, not by filtering them out of the artifact.
  delete artifact.assistant_visual_verdict;
  const existingBlockers = Array.isArray(artifact.evidence_blockers)
    ? artifact.evidence_blockers
    : [];
  const resolvedBlockers = new Set([
    "playwright_motion_capture_pending",
    "runtime_disconnect_stale_visual_pending",
    "renderer_is_placeholder_no_scena",
  ]);
  artifact.evidence_blockers = Array.from(
    new Set(existingBlockers.filter((blocker) => !resolvedBlockers.has(blocker))),
  );
  await fs.writeFile(artifactPath, `${JSON.stringify(artifact, null, 2)}\n`, "utf8");

  console.log(
    JSON.stringify(
      {
        artifact: relative(artifactPath),
        renderer_origin: artifact.renderer_origin,
        before_png: relative(beforePng),
        closed_grip_png: relative(closedPng),
        after_png: relative(afterPng),
        stale_png: relative(stalePng),
        pixel_difference_count: diff.pixel_difference_count,
        fps: artifact.fps_latency.fps,
        checks,
      },
      null,
      2,
    ),
  );
} finally {
  await browser.close();
}

function loadProductionPanelHtml() {
  if (!existsSync(panelOutPath)) {
    throw new Error(
      `${relative(panelOutPath)} is missing; run 'cd editors/vscode && npm run compile' first`,
    );
  }
  const require = createRequire(import.meta.url);
  const originalLoad = Module._load;
  Module._load = function patchedLoad(request, parent, isMain) {
    if (request === "vscode") {
      return vscodeStub();
    }
    return originalLoad.call(this, request, parent, isMain);
  };
  try {
    const panelModule = require(panelOutPath);
    if (typeof panelModule.__testGetTrustTwinPanelHtmlForPlaywright !== "function") {
      throw new Error(
        "compiled trustTwinPanel.js does not export __testGetTrustTwinPanelHtmlForPlaywright",
      );
    }
    return panelModule.__testGetTrustTwinPanelHtmlForPlaywright(extensionRoot);
  } finally {
    Module._load = originalLoad;
  }
}

function vscodeStub() {
  return {
    Uri: {
      file(filePath) {
        return uri(filePath);
      },
      joinPath(base, ...segments) {
        return uri(path.join(base.fsPath, ...segments));
      },
    },
    workspace: {
      workspaceFolders: [],
      getConfiguration() {
        return {
          get() {
            return undefined;
          },
        };
      },
      createFileSystemWatcher() {
        return disposable();
      },
      onDidChangeConfiguration() {
        return disposable();
      },
      getWorkspaceFolder() {
        return undefined;
      },
    },
    window: {
      activeTextEditor: undefined,
      createWebviewPanel() {
        throw new Error("createWebviewPanel is not available in the Playwright harness");
      },
    },
    commands: {
      registerCommand() {
        return disposable();
      },
    },
    ViewColumn: { Beside: 2 },
  };
}

function uri(filePath) {
  const fsPath = path.resolve(filePath);
  return {
    fsPath,
    toString() {
      return pathToFileURL(fsPath).href;
    },
  };
}

function disposable() {
  return { dispose() {} };
}

function parseToml(filePath) {
  const code = [
    "import json, sys, tomllib",
    "with open(sys.argv[1], 'rb') as fh:",
    "    print(json.dumps(tomllib.load(fh)))",
  ].join("\n");
  const result = spawnSync("python3", ["-c", code, filePath], {
    encoding: "utf8",
    maxBuffer: 8 * 1024 * 1024,
  });
  if (result.status !== 0) {
    throw new Error(`failed to parse TOML ${relative(filePath)}: ${result.stderr || result.stdout}`);
  }
  return JSON.parse(result.stdout);
}

function imageDiff(leftPath, rightPath) {
  const code = [
    "import json, sys",
    "from PIL import Image, ImageChops",
    "left = Image.open(sys.argv[1]).convert('RGBA')",
    "right = Image.open(sys.argv[2]).convert('RGBA')",
    "if left.size != right.size:",
    "    raise SystemExit(f'image sizes differ: {left.size} != {right.size}')",
    "diff = ImageChops.difference(left, right)",
    "pixels = diff.getdata()",
    "count = sum(1 for pixel in pixels if pixel != (0, 0, 0, 0))",
    "print(json.dumps({'pixel_difference_count': count, 'width': left.size[0], 'height': left.size[1]}))",
  ].join("\n");
  const result = spawnSync("python3", ["-c", code, leftPath, rightPath], {
    encoding: "utf8",
    maxBuffer: 8 * 1024 * 1024,
  });
  if (result.status !== 0) {
    throw new Error(`failed to diff screenshots: ${result.stderr || result.stdout}`);
  }
  return JSON.parse(result.stdout);
}

async function renderAndCapture(page, sample, connected, screenshotPath) {
  const state = await renderScene(page, sample, connected);
  await page.screenshot({ path: screenshotPath, fullPage: true });
  return state;
}

async function renderScene(page, sample, connected) {
  await page.evaluate((payload) => {
    window.postMessage({ type: "scene", payload }, "*");
  }, scenePayload(sample, connected));
  try {
    await page.waitForSelector('[data-trust-twin-node="BOX-1"]', { timeout: 10_000 });
  } catch (error) {
    const bodyText = await page.locator("body").innerText().catch(() => "<no body text>");
    throw new Error(
      `${error.message}\nBrowser messages:\n${browserMessages.join("\n")}\nBody:\n${bodyText}`,
    );
  }
  await page.waitForTimeout(230);
  return page.evaluate(() => {
    const rect = (id) => {
      const element = document.querySelector(`[data-trust-twin-node="${id}"]`);
      if (!element) {
        throw new Error(`missing rendered node ${id}`);
      }
      const bounds = element.getBoundingClientRect();
      const style = window.getComputedStyle(element);
      return {
        left: bounds.left,
        right: bounds.right,
        top: bounds.top,
        bottom: bounds.bottom,
        width: bounds.width,
        height: bounds.height,
        center_x: bounds.left + bounds.width / 2,
        center_y: bounds.top + bounds.height / 2,
        opacity: Number(style.opacity),
        transform: style.transform,
      };
    };
    const surface = document.querySelector("main#surface");
    return {
      offline: surface ? surface.classList.contains("offline") : false,
      rects: {
        "PICKUP-1": rect("PICKUP-1"),
        "DROP-1": rect("DROP-1"),
        "BOX-1": rect("BOX-1"),
        "ROBOT-1.wrist": rect("ROBOT-1.wrist"),
        "GRIPPER-1": rect("GRIPPER-1"),
        "GRIPPER-1.left_jaw": rect("GRIPPER-1.left_jaw"),
        "GRIPPER-1.right_jaw": rect("GRIPPER-1.right_jaw"),
      },
    };
  });
}

async function measureRenderFps(page, samplesToRender) {
  const start = performance.now();
  for (const sample of samplesToRender) {
    await page.evaluate((payload) => {
      window.postMessage({ type: "scene", payload }, "*");
    }, scenePayload(sample, true));
    await page.evaluate(() => new Promise((resolve) => requestAnimationFrame(() => resolve())));
  }
  const elapsed = Math.max(1, performance.now() - start);
  return (samplesToRender.length / elapsed) * 1000;
}

function scenePayload(sample, connected) {
  const scenePage = {
    id: "robot-cell",
    title: "Robot Cell",
    order: 0,
    kind: "scene3d",
    view: "views/robot-cell.view.toml",
    scene_view: sceneView,
  };
  return {
    page: scenePage,
    scenePage,
    pages: [scenePage],
    breadcrumbs: ["Robot Cell"],
    connected,
    valuesBySource: valuesBySource(sample),
    workspaceView: {
      path: "examples/trust-twin/robot-cell/hmi/views/robot-cell.view.toml",
      loaded: true,
      bytes: 0,
    },
  };
}

function valuesBySource(sample) {
  return {
    "Main.RobotShoulderAngle": sample.shoulder_angle,
    "Main.RobotElbowAngle": sample.elbow_angle,
    "Main.RobotWristAngle": sample.wrist_angle,
    "Main.RobotGripperOpen": sample.gripper_open,
    "Main.RobotGripperX": sample.gripper_position[0],
    "Main.RobotGripperY": sample.gripper_position[1],
    "Main.RobotGripperZ": sample.gripper_position[2],
    "Main.RobotBoxX": sample.box_position[0],
    "Main.RobotBoxY": sample.box_position[1],
    "Main.RobotBoxZ": sample.box_position[2],
    "Main.RobotStatusLight": sample.status_emissive === "#22c55e",
  };
}

function assertBoxRestsOnSurface(rects, boxId, zoneId) {
  const box = rects[boxId];
  const zone = rects[zoneId];
  const contactGapPx = Math.abs(box.bottom - zone.top);
  const centerInsideZone = box.center_x >= zone.left && box.center_x <= zone.right;
  if (contactGapPx > 5 || !centerInsideZone) {
    throw new Error(
      `box is not visually resting on pickup surface: contactGapPx=${contactGapPx.toFixed(
        2,
      )}, centerInsideZone=${centerInsideZone}`,
    );
  }
  return { ok: true, contact_gap_px: round(contactGapPx), center_inside_zone: centerInsideZone };
}

function assertClosedGripAroundBox(rects) {
  const box = rects["BOX-1"];
  const leftJaw = rects["GRIPPER-1.left_jaw"];
  const rightJaw = rects["GRIPPER-1.right_jaw"];
  const minJawCenter = Math.min(leftJaw.center_x, rightJaw.center_x);
  const maxJawCenter = Math.max(leftJaw.center_x, rightJaw.center_x);
  const bracketsBox = minJawCenter < box.center_x && maxJawCenter > box.center_x;
  const jawCenterGap = maxJawCenter - minJawCenter;
  if (!bracketsBox || jawCenterGap < box.width * 0.4 || jawCenterGap > box.width * 3.0) {
    throw new Error(
      `closed gripper does not bracket box: brackets=${bracketsBox}, jawCenterGap=${jawCenterGap.toFixed(
        2,
      )}, boxWidth=${box.width.toFixed(2)}, boxCenter=${box.center_x.toFixed(
        2,
      )}, minJawCenter=${minJawCenter.toFixed(2)}, maxJawCenter=${maxJawCenter.toFixed(2)}`,
    );
  }
  return { ok: true, jaw_center_gap_px: round(jawCenterGap), box_width_px: round(box.width) };
}

function assertToolingAboveFloor(rects) {
  const floorCenterY = rects["DROP-1"].center_y;
  const checked = ["ROBOT-1.wrist", "GRIPPER-1", "GRIPPER-1.left_jaw", "GRIPPER-1.right_jaw"];
  const failures = checked.filter((id) => rects[id].bottom > floorCenterY + 5);
  if (failures.length > 0) {
    throw new Error(`robot wrist/tooling below floor/table plane: ${failures.join(", ")}`);
  }
  return { ok: true, checked };
}

function assertStaleFreezesPose(live, stale) {
  if (!stale.offline) {
    throw new Error("stale render did not set the production webview offline state");
  }
  for (const id of Object.keys(live.rects)) {
    const dx = Math.abs(live.rects[id].center_x - stale.rects[id].center_x);
    const dy = Math.abs(live.rects[id].center_y - stale.rects[id].center_y);
    if (dx > 1 || dy > 1) {
      throw new Error(`stale render moved ${id}: dx=${dx.toFixed(2)}, dy=${dy.toFixed(2)}`);
    }
  }
  if (stale.rects["BOX-1"].opacity > 0.75) {
    throw new Error(`stale render did not visually grey out nodes; opacity=${stale.rects["BOX-1"].opacity}`);
  }
  return { ok: true, box_opacity: stale.rects["BOX-1"].opacity };
}

function sampleByStep(allSamples, step) {
  const sample = allSamples.find((entry) => entry.step === step);
  if (!sample) {
    throw new Error(`missing trace sample for step ${step}`);
  }
  return sample;
}

function relative(filePath) {
  return path.relative(repoRoot, filePath).replaceAll(path.sep, "/");
}

function sha256(bytes) {
  return createHash("sha256").update(bytes).digest("hex");
}

function round(value) {
  return Math.round(value * 100) / 100;
}
