import { spawnSync } from "node:child_process";
import { createHash } from "node:crypto";
import fs from "node:fs/promises";
import { existsSync } from "node:fs";
import http from "node:http";
import { createRequire } from "node:module";
import Module from "node:module";
import path from "node:path";
import { fileURLToPath, pathToFileURL } from "node:url";

import { chromium, firefox, webkit } from "./captures/node_modules/playwright/index.mjs";

const repoRoot = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "..");
const artifactDir = path.join(repoRoot, "target", "gate-artifacts");
const artifactPath = path.join(artifactDir, "trust-twin-robot-cell-motion.json");
const htmlPath = path.join(artifactDir, "trust-twin-robot-cell-production-webview.html");
const pictureProofHtmlPath = path.join(artifactDir, "trust-twin-robot-cell-picture-proof.html");
const pictureProofPagePng = path.join(artifactDir, "trust-twin-robot-cell-picture-proof-page.png");
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

const playwrightBrowserName = process.env.TRUST_TWIN_PLAYWRIGHT_BROWSER || "firefox";
const command = process.env.TRUST_TWIN_PLAYWRIGHT_BROWSER
  ? `TRUST_TWIN_PLAYWRIGHT_BROWSER=${playwrightBrowserName} node scripts/trust_twin_robot_cell_playwright.mjs`
  : "node scripts/trust_twin_robot_cell_playwright.mjs";
const playwrightBrowserTypes = { chromium, firefox, webkit };
const playwrightBrowserType = playwrightBrowserTypes[playwrightBrowserName];
if (!playwrightBrowserType) {
  throw new Error(
    `TRUST_TWIN_PLAYWRIGHT_BROWSER must be one of ${Object.keys(playwrightBrowserTypes).join(", ")}, got '${playwrightBrowserName}'`,
  );
}
const artifact = JSON.parse(await fs.readFile(artifactPath, "utf8"));
const sceneView = parseToml(viewPath);
const samples = artifact.trace_samples || [];
if (!Array.isArray(samples) || samples.length < 5) {
  throw new Error(`${relative(artifactPath)} must contain at least five trace_samples`);
}

const beforeSample = sampleByStep(samples, 1);
const closedSample = sampleByStep(samples, 2);
const afterSample = sampleByStep(samples, 6);
const staleSample = sampleByStep(samples, 6);

await fs.mkdir(artifactDir, { recursive: true });
const staticServer = await startStaticServer();
const html = loadProductionPanelHtml(staticServer.origin);
await fs.writeFile(htmlPath, html, "utf8");

let browser;
let browserMessages = [];
try {
  browser = await playwrightBrowserType.launch(playwrightLaunchOptions(playwrightBrowserName));
  const page = await browser.newPage({ viewport: { width: 1200, height: 760 }, deviceScaleFactor: 1.5 });
  const started = performance.now();
  page.on("console", (message) => browserMessages.push(`console:${message.type()}: ${message.text()}`));
  page.on("pageerror", (error) => browserMessages.push(`pageerror: ${error.message}`));
  page.on("response", (response) => {
    if (response.status() >= 400) {
      browserMessages.push(`response:${response.status()}: ${response.url()}`);
    }
  });

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
    window.__trustTwinRendererWasmReady = null;
    window.addEventListener("trustTwinRendererWasmReady", (event) => {
      window.__trustTwinRendererWasmReady = {
        detail: event.detail || null,
        at: Date.now(),
      };
    });
  });
  await page.goto(staticServer.urlFor(htmlPath), { waitUntil: "domcontentloaded" });
  await applyPlaywrightTheme(page);
  await page.waitForSelector("main#surface", { timeout: 10_000 });
  const browserCapabilities = await assertPlaywrightRenderCapabilities(
    page,
    browserMessages,
    playwrightBrowserName,
  );
  try {
    await page.waitForFunction(
      () => {
        const ready = window.__trustTwinRendererWasmReady;
        return ready
          && ready.detail
          && ready.detail.ok === true
          && (window.__trustTwinRendererOrigin === "scena_webgl" || window.__trustTwinRendererOrigin === "scena_webgpu");
      },
      { timeout: 20_000 },
    );
  } catch (error) {
    const bodyText = await page.locator("body").innerText().catch(() => "<no body text>");
    const readyEvent = await page.evaluate(() => window.__trustTwinRendererWasmReady || null).catch(() => null);
    throw new Error(
      [
        `trustTwinRendererWasmReady did not report a scena renderer: ${error.message}`,
        `Playwright browser: ${playwrightBrowserName}`,
        `Capabilities: ${JSON.stringify(browserCapabilities)}`,
        `Renderer ready event: ${JSON.stringify(readyEvent)}`,
        `Browser messages:\n${browserMessages.join("\n")}`,
        `Body:\n${bodyText}`,
      ].join("\n"),
    );
  }

  await renderAndCapture(page, beforeSample, true, beforePng);
  assertNoRendererConsoleErrors(browserMessages, "before source-material frame");
  await reloadReadyPanel(page, browserMessages, browserCapabilities);
  await renderAndCapture(page, closedSample, true, closedPng);
  assertNoRendererConsoleErrors(browserMessages, "closed-grip source-material frame");
  await reloadReadyPanel(page, browserMessages, browserCapabilities);
  await renderAndCapture(page, afterSample, true, afterPng);
  assertNoRendererConsoleErrors(browserMessages, "after source-material frame");
  await reloadReadyPanel(page, browserMessages, browserCapabilities);
  await renderAndCapture(page, staleSample, false, stalePng);
  assertNoRendererConsoleErrors(browserMessages, "stale offline-material frame");
  const fps = await measureRenderFps(page, [beforeSample, closedSample, afterSample]);
  const elapsedMs = performance.now() - started;
  const assetProof = await page.evaluate(() => window.__trustTwinAssetProof || null);
  assertPackagedAssetProof(assetProof);
  const visual = analyzeScreenshots(beforePng, closedPng, afterPng, stalePng);
  const diff = { pixel_difference_count: visual.pixel_difference_count };
  const checks = visual.checks;

  if (diff.pixel_difference_count < 2500) {
    throw new Error(`robot-cell scena frame diff too small: ${diff.pixel_difference_count}`);
  }
  if (checks.wrist_arc_y_extent_px.value < 80) {
    throw new Error(`wrist screen-Y arc is too small: ${checks.wrist_arc_y_extent_px.value}px`);
  }
  for (const [name, occupancy] of Object.entries(checks.canvas_occupancy)) {
    if (occupancy.ratio < 0.25) {
      throw new Error(`${name} non-background occupancy is too small: ${occupancy.ratio}`);
    }
  }

  const beforeBytes = await fs.readFile(beforePng);
  const closedBytes = await fs.readFile(closedPng);
  const afterBytes = await fs.readFile(afterPng);
  const staleBytes = await fs.readFile(stalePng);

  const rendererOrigin = await page.evaluate(() => window.__trustTwinRendererOrigin || "");
  if (rendererOrigin !== "scena_webgl" && rendererOrigin !== "scena_webgpu") {
    throw new Error(`renderer_origin must come from scena wasm, got '${rendererOrigin}'`);
  }
  const pictureProof = await writePictureProofHtml({
    rendererOrigin,
    screenshots: [
      { id: "before", label: "Before", path: beforePng },
      { id: "closed_grip", label: "Closed grip", path: closedPng },
      { id: "after", label: "After", path: afterPng },
      { id: "stale", label: "Stale offline", path: stalePng },
    ],
  });
  await assertPictureProofHtml(browser, staticServer, pictureProofHtmlPath, pictureProofPagePng);
  artifact.renderer_origin = rendererOrigin;
  artifact.frame_hashes_before_after = {
    before: sha256(beforeBytes),
    closed_grip: sha256(closedBytes),
    after: sha256(afterBytes),
    stale: sha256(staleBytes),
  };
  artifact.pixel_difference_count = diff.pixel_difference_count;
  artifact.screenshot_video_path = {
    html: relative(htmlPath),
    picture_proof_html: relative(pictureProofHtmlPath),
    picture_proof_page_png: relative(pictureProofPagePng),
    before_png: relative(beforePng),
    closed_grip_png: relative(closedPng),
    after_png: relative(afterPng),
    stale_png: relative(stalePng),
  };
  artifact.picture_proof = pictureProof;
  artifact.playwright = {
    command,
    result: "ok",
    browser_engine: playwrightBrowserName,
    browser: await browser.version(),
    capabilities: browserCapabilities,
    renderer_origin: rendererOrigin,
    html_source: "editors/vscode/src/trustTwinPanel.ts",
  };
  artifact.fps_latency = {
    fps: Math.round(fps * 10) / 10,
    latency_ms: Math.round(elapsedMs * 10) / 10,
  };
  artifact.disconnected_state_result = "ok";
  artifact.visual_motion_checks = checks;
  const assetMetadata = assetProof.metadata && typeof assetProof.metadata === "object"
    ? assetProof.metadata
    : {};
  artifact.asset_state = {
    state: assetProof.asset_state,
    source: assetMetadata.asset_source || null,
    source_url: assetMetadata.asset_source_url || null,
    original_author: assetMetadata.asset_original_author || null,
    license: assetMetadata.asset_license || null,
    manifest_sha256: assetMetadata.asset_manifest_sha256 || null,
    package_path: assetMetadata.asset_package_path || null,
    version: assetMetadata.asset_version || null,
    assets: assetProof.assets.map((asset) => asset.id),
  };
  artifact.asset_availability = assetProof;
  const visualReview = runAssistantVisualReview({
    beforePng,
    closedPng,
    afterPng,
    stalePng,
    rendererOrigin,
    checks,
    pixelDifferenceCount: diff.pixel_difference_count,
    assetProof,
  });
  if (visualReview.text) {
    artifact.assistant_visual_verdict = visualReview.text;
  } else {
    delete artifact.assistant_visual_verdict;
  }
  artifact.assistant_visual_review = visualReview.metadata;
  const existingBlockers = Array.isArray(artifact.evidence_blockers)
    ? artifact.evidence_blockers
    : [];
  const passedBlockers = new Set([
    "playwright_motion_capture_pending",
    "playwright_motion_capture_failed",
    "runtime_disconnect_stale_visual_pending",
    "renderer_is_placeholder_no_scena",
    "packaged_asset_available_pending",
  ]);
  if (visualReview.approved) {
    passedBlockers.add("assistant_visual_review_pending");
  }
  artifact.evidence_blockers = existingBlockers.filter(
    (blocker) => !passedBlockers.has(blocker),
  );
  if (visualReview.text && !visualReview.approved) {
    artifact.evidence_blockers = [
      ...new Set([...artifact.evidence_blockers, "assistant_visual_review_rejected"]),
    ];
  }
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
        picture_proof_html: relative(pictureProofHtmlPath),
        picture_proof_page_png: relative(pictureProofPagePng),
        pixel_difference_count: diff.pixel_difference_count,
        fps: artifact.fps_latency.fps,
        asset_availability: artifact.asset_availability,
        assistant_visual_review: artifact.assistant_visual_review,
        evidence_blockers: artifact.evidence_blockers,
        checks,
      },
      null,
      2,
    ),
  );
} catch (error) {
  await writePlaywrightFailureArtifact(error, browserMessages);
  throw error;
} finally {
  if (browser) {
    await browser.close();
  }
  await staticServer.close();
}

async function writePlaywrightFailureArtifact(error, browserMessages) {
  const existingBlockers = Array.isArray(artifact.evidence_blockers)
    ? artifact.evidence_blockers
    : [];
  artifact.playwright = {
    command,
    result: "failed",
    browser_engine: playwrightBrowserName,
    error: String(error && error.stack ? error.stack : error),
    browser_messages: browserMessages.slice(-80),
    html_source: "editors/vscode/src/trustTwinPanel.ts",
  };
  artifact.renderer_origin = null;
  artifact.frame_hashes_before_after = null;
  artifact.pixel_difference_count = null;
  artifact.visual_motion_checks = null;
  artifact.fps_latency = { fps: null, latency_ms: null };
  artifact.disconnected_state_result = "failed";
  artifact.assistant_visual_verdict = "blocked";
  artifact.assistant_visual_review = {
    result: "blocked",
    source: "playwright_failure",
  };
  artifact.evidence_blockers = [
    ...new Set([...existingBlockers, "playwright_motion_capture_failed"]),
  ];
  await fs.writeFile(artifactPath, `${JSON.stringify(artifact, null, 2)}\n`, "utf8");
}

function assertNoRendererConsoleErrors(browserMessages, phase) {
  const badMessages = browserMessages.filter((message) => {
    if (
      message.includes("scena wgpu uncaptured error") ||
      message.includes("Validation Error") ||
      message.includes("InvalidDimension") ||
      message.startsWith("pageerror:")
    ) {
      return true;
    }
    if (!message.startsWith("response:")) {
      return false;
    }
    return /(\.wasm|\.js|\.gltf|\.glb|\.bin|\.png|\/media\/trust-twin\/|\/snippets\/)/.test(message);
  });
  if (badMessages.length > 0) {
    throw new Error(
      [
        `browser renderer reported console/network errors during ${phase}:`,
        ...badMessages,
      ].join("\n"),
    );
  }
}

function playwrightLaunchOptions(browserName) {
  const options = {
    headless: process.env.TRUST_TWIN_PLAYWRIGHT_HEADLESS === "0" ? false : true,
  };
  if (process.env.TRUST_TWIN_PLAYWRIGHT_EXECUTABLE_PATH) {
    options.executablePath = process.env.TRUST_TWIN_PLAYWRIGHT_EXECUTABLE_PATH;
  }
  const args = splitPlaywrightArgs(process.env.TRUST_TWIN_PLAYWRIGHT_ARGS || "");
  if (args.length > 0) {
    if (browserName !== "chromium") {
      throw new Error("TRUST_TWIN_PLAYWRIGHT_ARGS is only supported for Playwright chromium");
    }
    options.args = args;
  }
  return options;
}

function splitPlaywrightArgs(value) {
  return String(value || "")
    .trim()
    .split(/\s+/)
    .filter(Boolean);
}

async function assertPlaywrightRenderCapabilities(page, browserMessages, browserName) {
  const capabilities = await page.evaluate(async () => {
    const canvas = document.createElement("canvas");
    let webgl2 = false;
    let webgl2_error = null;
    try {
      webgl2 = !!canvas.getContext("webgl2", { antialias: false });
    } catch (error) {
      webgl2_error = String(error && error.message ? error.message : error);
    }

    const webgpu = !!navigator.gpu;
    let webgpu_adapter = false;
    let webgpu_error = null;
    if (webgpu) {
      try {
        webgpu_adapter = !!(await navigator.gpu.requestAdapter());
      } catch (error) {
        webgpu_error = String(error && error.message ? error.message : error);
      }
    }

    return {
      user_agent: navigator.userAgent,
      webgpu,
      webgpu_adapter,
      webgpu_error,
      webgl2,
      webgl2_error,
    };
  });

  if (!capabilities.webgpu_adapter && !capabilities.webgl2) {
    throw new Error(
      [
        "Playwright browser does not expose WebGPU or WebGL2, so the scena visual gate cannot run.",
        `Playwright browser: ${browserName}`,
        `Capabilities: ${JSON.stringify(capabilities)}`,
        `Browser messages:\n${browserMessages.join("\n")}`,
      ].join("\n"),
    );
  }

  return capabilities;
}

async function writePictureProofHtml({ rendererOrigin, screenshots }) {
  const rows = screenshots.map((screenshot) => ({
    id: screenshot.id,
    label: screenshot.label,
    src: path.relative(path.dirname(pictureProofHtmlPath), screenshot.path).replaceAll(path.sep, "/"),
    png: relative(screenshot.path),
  }));
  const html = `<!doctype html>
<html lang="en">
<head>
  <meta charset="utf-8">
  <meta name="viewport" content="width=device-width, initial-scale=1">
  <title>trust-twin robot cell picture proof</title>
  <style>
    body { margin: 0; font-family: Arial, sans-serif; background: #f8fafc; color: #111827; }
    main { max-width: 1180px; margin: 0 auto; padding: 18px; }
    h1 { margin: 0 0 8px; font-size: 22px; }
    .meta { margin: 0 0 16px; color: #475569; font-size: 13px; }
    .grid { display: grid; grid-template-columns: repeat(2, minmax(0, 1fr)); gap: 14px; }
    figure { margin: 0; border: 1px solid #cbd5e1; background: #ffffff; }
    img { display: block; width: 100%; height: auto; background: #0f172a; }
    figcaption { padding: 8px 10px; font-size: 13px; font-weight: 600; }
    @media (max-width: 760px) { .grid { grid-template-columns: 1fr; } }
  </style>
</head>
<body>
  <main>
    <h1>trust-twin robot cell picture proof</h1>
    <p class="meta">renderer_origin: ${escapeHtml(rendererOrigin)}; source: Playwright browser canvas screenshots</p>
    <section class="grid">
${rows.map((row) => `      <figure data-proof-frame="${escapeHtml(row.id)}"><img src="${escapeHtml(row.src)}" alt="${escapeHtml(row.label)}"><figcaption>${escapeHtml(row.label)}</figcaption></figure>`).join("\n")}
    </section>
  </main>
</body>
</html>
`;
  await fs.writeFile(pictureProofHtmlPath, html, "utf8");
  return {
    type: "browser_html_image_gallery",
    html: relative(pictureProofHtmlPath),
    screenshots: rows.map((row) => ({ id: row.id, label: row.label, png: row.png })),
    renderer_origin: rendererOrigin,
  };
}

async function assertPictureProofHtml(browser, staticServer, proofHtmlPath, proofPagePngPath) {
  const proofPage = await browser.newPage({ viewport: { width: 1180, height: 900 }, deviceScaleFactor: 1 });
  try {
    await proofPage.goto(staticServer.urlFor(proofHtmlPath), { waitUntil: "networkidle" });
    const imageState = await proofPage.evaluate(() => {
      const images = Array.from(document.querySelectorAll("img"));
      return {
        count: images.length,
        all_loaded: images.every((image) => image.complete && image.naturalWidth > 0 && image.naturalHeight > 0),
        sizes: images.map((image) => ({
          src: image.getAttribute("src"),
          naturalWidth: image.naturalWidth,
          naturalHeight: image.naturalHeight,
        })),
      };
    });
    if (imageState.count !== 4 || !imageState.all_loaded) {
      throw new Error(`picture proof HTML did not display all browser-rendered PNGs: ${JSON.stringify(imageState)}`);
    }
    await proofPage.screenshot({ path: proofPagePngPath, fullPage: true });
  } finally {
    await proofPage.close();
  }
}

async function applyPlaywrightTheme(page) {
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
}

async function reloadReadyPanel(page, browserMessages, browserCapabilities) {
  await page.goto(staticServer.urlFor(htmlPath), { waitUntil: "domcontentloaded" });
  await applyPlaywrightTheme(page);
  await page.waitForSelector("main#surface", { timeout: 10_000 });
  try {
    await page.waitForFunction(
      () => {
        const ready = window.__trustTwinRendererWasmReady;
        return ready
          && ready.detail
          && ready.detail.ok === true
          && (window.__trustTwinRendererOrigin === "scena_webgl" || window.__trustTwinRendererOrigin === "scena_webgpu");
      },
      { timeout: 20_000 },
    );
  } catch (error) {
    const bodyText = await page.locator("body").innerText().catch(() => "<no body text>");
    const readyEvent = await page.evaluate(() => window.__trustTwinRendererWasmReady || null).catch(() => null);
    throw new Error(
      [
        `trustTwinRendererWasmReady did not report a scena renderer after reload: ${error.message}`,
        `Capabilities: ${JSON.stringify(browserCapabilities)}`,
        `Renderer ready event: ${JSON.stringify(readyEvent)}`,
        `Browser messages:\n${browserMessages.join("\n")}`,
        `Body:\n${bodyText}`,
      ].join("\n"),
    );
  }
}

async function startStaticServer() {
  const server = http.createServer(async (request, response) => {
    try {
      const requestUrl = new URL(request.url || "/", "http://127.0.0.1");
      const decoded = decodeURIComponent(requestUrl.pathname.replace(/^\/+/, ""));
      const filePath = path.resolve(repoRoot, decoded || "index.html");
      if (!filePath.startsWith(repoRoot + path.sep)) {
        response.writeHead(403);
        response.end("forbidden");
        return;
      }
      const bytes = await fs.readFile(filePath);
      response.writeHead(200, { "content-type": contentType(filePath) });
      response.end(bytes);
    } catch {
      response.writeHead(404);
      response.end("not found");
    }
  });
  await new Promise((resolve) => server.listen(0, "127.0.0.1", resolve));
  const address = server.address();
  const origin = `http://127.0.0.1:${address.port}`;
  return {
    origin,
    urlFor(filePath) {
      return `${origin}/${relative(filePath)}`;
    },
    close() {
      return new Promise((resolve, reject) => {
        server.close((error) => (error ? reject(error) : resolve()));
      });
    },
  };
}

function contentType(filePath) {
  if (filePath.endsWith(".html")) return "text/html; charset=utf-8";
  if (filePath.endsWith(".js")) return "text/javascript; charset=utf-8";
  if (filePath.endsWith(".wasm")) return "application/wasm";
  if (filePath.endsWith(".png")) return "image/png";
  if (filePath.endsWith(".css")) return "text/css; charset=utf-8";
  return "application/octet-stream";
}

function loadProductionPanelHtml(cspSource) {
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
    const html = panelModule.__testGetTrustTwinPanelHtmlForPlaywright(extensionRoot, cspSource);
    const repoFilePrefix = pathToFileURL(`${repoRoot}${path.sep}`).href;
    return html.replaceAll(repoFilePrefix, `${cspSource}/`);
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

function runAssistantVisualReview({
  beforePng,
  closedPng,
  afterPng,
  stalePng,
  rendererOrigin,
  checks,
  pixelDifferenceCount,
  assetProof,
}) {
  const candidates = [
    process.env.TRUST_TWIN_VISUAL_REVIEW_CLAUDE,
    "claude",
    process.env.HOME ? path.join(process.env.HOME, ".local", "bin", "claude") : "",
  ].filter(Boolean);
  const prompt = [
    "You are performing the required trust-twin assistant visual review against four Playwright PNG screenshots.",
    "Read the image files at the exact paths below before answering. Do not answer from filenames only.",
    "",
    `before_png: ${beforePng}`,
    `closed_grip_png: ${closedPng}`,
    `after_png: ${afterPng}`,
    `stale_png: ${stalePng}`,
    "",
    "Runtime facts from the gate artifact:",
    "- before: BOX-1 parent is PICKUP-1, box world position is [0, 0.35, 0]",
    "- closed_grip: BOX-1 parent is GRIPPER-1 and the gripper is closed",
    "- after: BOX-1 parent is DROP-1, box world position is [4, 0.35, 0]",
    `- renderer_origin: ${rendererOrigin}`,
    `- asset_state: ${assetProof?.asset_state || ""}`,
    `- asset_source: ${assetProof?.metadata?.asset_source || ""}`,
    `- asset_source_url: ${assetProof?.metadata?.asset_source_url || ""}`,
    `- asset_original_author: ${assetProof?.metadata?.asset_original_author || ""}`,
    `- asset_license: ${assetProof?.metadata?.asset_license || ""}`,
    `- asset_manifest_sha256: ${assetProof?.metadata?.asset_manifest_sha256 || ""}`,
    `- packaged_asset_count: ${assetProof?.asset_count || 0}`,
    `- pixel_difference_count: ${pixelDifferenceCount}`,
    `- wrist_screen_y_arc_px: ${checks.wrist_arc_y_extent_px.value}`,
    `- closed_grip_jaw_gap_px: ${checks.closed_grip_around_box.jaw_center_gap_px}`,
    `- closed_grip_box_width_px: ${checks.closed_grip_around_box.box_width_px}`,
    `- stale_saturation: ${checks.stale_desaturated_by_scena.stale_saturation}`,
    `- live_saturation: ${checks.stale_desaturated_by_scena.live_saturation}`,
    "",
    "Answer with exactly these seven lines and no preamble:",
    "robot_recognizable: yes/no - whether the PNGs cold-read as a UR10/industrial robot arm rather than blobs or boxes",
    "motion_visible: yes/no - compare before, closed_grip, and after",
    "runtime_match: yes/no - whether the visuals match the runtime facts above",
    "closed_grip: yes/no - whether the closed_grip image shows the gripper around the box",
    "fallback_placeholders_remain: yes/no - whether labeled boxes, loading text, or placeholder escape remain",
    "stale_offline: yes/no - whether stale.png is the same scene desaturated/greyed rather than missing",
    "approval: approved/rejected - one sentence reason",
  ].join("\n");

  for (const commandPath of candidates) {
    const result = spawnSync(
      commandPath,
      [
        "-p",
        "--permission-mode",
        "bypassPermissions",
        "--allowedTools",
        "Read",
        "--max-budget-usd",
        process.env.TRUST_TWIN_VISUAL_REVIEW_MAX_BUDGET_USD || "0.50",
        "--model",
        process.env.TRUST_TWIN_VISUAL_REVIEW_MODEL || "sonnet",
        prompt,
      ],
      {
        cwd: repoRoot,
        encoding: "utf8",
        maxBuffer: 2 * 1024 * 1024,
        timeout: 180_000,
      },
    );
    if (result.error && result.error.code === "ENOENT") {
      continue;
    }
    const stdout = result.stdout || "";
    const metadata = {
      result: result.status === 0 && stdout.trim() ? "ok" : "blocked",
      source: "claude_cli",
      command: commandPath,
      model: process.env.TRUST_TWIN_VISUAL_REVIEW_MODEL || "sonnet",
      pngs: {
        before: relative(beforePng),
        closed_grip: relative(closedPng),
        after: relative(afterPng),
        stale: relative(stalePng),
      },
    };
    if (result.status !== 0 || !stdout.trim()) {
      metadata.error = (result.stderr || result.error?.message || "visual review returned no text")
        .trim()
        .slice(0, 4000);
      return { text: null, approved: false, metadata };
    }
    const approved = /^approval:\s*approved\b/im.test(stdout);
    return { text: stdout, approved, metadata };
  }

  return {
    text: null,
    approved: false,
    metadata: {
      result: "blocked",
      source: "claude_cli",
      error: "claude command not found",
    },
  };
}

function assertPackagedAssetProof(assetProof) {
  if (!assetProof || assetProof.asset_state !== "packaged_asset") {
    throw new Error(`expected packaged_asset webview proof, got ${JSON.stringify(assetProof)}`);
  }
  const metadata = assetProof.metadata && typeof assetProof.metadata === "object"
    ? assetProof.metadata
    : {};
  for (const field of [
    "asset_source_url",
    "asset_original_author",
    "asset_license",
    "asset_manifest_sha256",
  ]) {
    if (typeof metadata[field] !== "string" || !metadata[field].trim()) {
      throw new Error(`packaged asset metadata must include ${field}: ${JSON.stringify(metadata)}`);
    }
  }
  if (/repo-authored|workspace license/i.test(`${metadata.asset_source || ""} ${metadata.asset_license || ""}`)) {
    throw new Error(`packaged asset metadata still describes repo-authored placeholder assets: ${JSON.stringify(metadata)}`);
  }
  const assets = Array.isArray(assetProof.assets) ? assetProof.assets : [];
  const required = [
    "trust-twin/components/ur10/visual/base.gltf",
    "trust-twin/components/ur10/visual/shoulder.gltf",
    "trust-twin/components/ur10/visual/upperarm.gltf",
    "trust-twin/components/ur10/visual/forearm.gltf",
    "trust-twin/components/ur10/visual/wrist1.gltf",
    "trust-twin/components/ur10/visual/wrist2.gltf",
    "trust-twin/components/ur10/visual/wrist3.gltf",
    "trust-twin/components/schunk-wsg50/meshes/wsg_body.gltf",
    "trust-twin/components/schunk-wsg50/meshes/finger_with_tip.gltf",
    "trust-twin/components/ycb/meshes/003_cracker_box_textured.gltf",
  ];
  const ids = new Set(assets.map((asset) => asset && asset.id));
  const missing = required.filter((id) => !ids.has(id));
  if (missing.length) {
    throw new Error(`packaged robot assets missing from webview proof: ${missing.join(", ")}`);
  }
  for (const asset of assets) {
    if (!asset || asset.kind !== "gltf" || typeof asset.uri !== "string") {
      throw new Error(`invalid packaged asset proof entry: ${JSON.stringify(asset)}`);
    }
    if (!asset.uri.includes("/editors/vscode/media/trust-twin/components/")) {
      throw new Error(`packaged asset '${asset.id}' was not rewritten to VS Code media URI: ${asset.uri}`);
    }
  }
}

function analyzeScreenshots(beforePath, closedPath, afterPath, stalePath) {
  const code = String.raw`
import colorsys, json, sys
from collections import deque
from PIL import Image, ImageChops

paths = {
    "before": sys.argv[1],
    "closed": sys.argv[2],
    "after": sys.argv[3],
    "stale": sys.argv[4],
}
images = {name: Image.open(path).convert("RGB") for name, path in paths.items()}
CLEARED_FRAME_RGB = images["before"].getpixel((0, 0))

def non_background(image):
    pixels = list(image.getdata())
    count = sum(1 for pixel in pixels if max(abs(pixel[i] - CLEARED_FRAME_RGB[i]) for i in range(3)) > 18)
    w, h = image.size
    return {
        "pixels": count,
        "area": w * h,
        "ratio": round(count / (w * h), 4),
        "cleared_frame_rgb": CLEARED_FRAME_RGB,
        "threshold_per_channel": 18,
    }

def bbox(points):
    if not points:
        return None
    xs = [point[0] for point in points]
    ys = [point[1] for point in points]
    return {
        "left": min(xs),
        "right": max(xs),
        "top": min(ys),
        "bottom": max(ys),
        "width": max(xs) - min(xs) + 1,
        "height": max(ys) - min(ys) + 1,
        "center_x": (min(xs) + max(xs)) / 2,
        "center_y": (min(ys) + max(ys)) / 2,
        "pixels": len(points),
    }

def color_points(image, predicate):
    points = []
    w, h = image.size
    data = image.load()
    for y in range(h):
        for x in range(w):
            if predicate(data[x, y]):
                points.append((x, y))
    return points

def largest_component(points):
    point_set = set(points)
    best = []
    while point_set:
        start = point_set.pop()
        queue = deque([start])
        current = [start]
        while queue:
            x, y = queue.popleft()
            for nx, ny in ((x + 1, y), (x - 1, y), (x, y + 1), (x, y - 1)):
                if (nx, ny) in point_set:
                    point_set.remove((nx, ny))
                    queue.append((nx, ny))
                    current.append((nx, ny))
        if len(current) > len(best):
            best = current
    return best

def component_bbox(image, predicate):
    points = color_points(image, predicate)
    component = largest_component(points)
    return bbox(component)

def component_bboxes(image, predicate, min_pixels=20):
    points = color_points(image, predicate)
    point_set = set(points)
    boxes = []
    while point_set:
        start = point_set.pop()
        queue = deque([start])
        current = [start]
        while queue:
            x, y = queue.popleft()
            for nx, ny in ((x + 1, y), (x - 1, y), (x, y + 1), (x, y - 1)):
                if (nx, ny) in point_set:
                    point_set.remove((nx, ny))
                    queue.append((nx, ny))
                    current.append((nx, ny))
        if len(current) >= min_pixels:
            boxes.append(bbox(current))
    boxes.sort(key=lambda box: box["pixels"], reverse=True)
    return boxes

def source_box_red(pixel):
    r, g, b = pixel
    return r > 45 and g < 100 and b < 100 and r > g * 1.15 and r > b * 1.15

def robot_bluegray(pixel):
    r, g, b = pixel
    return b > 70 and g > 50 and r < 90 and b >= g * 0.8

def saturation_mean(image):
    values = []
    for r, g, b in image.getdata():
        if max(abs((r, g, b)[i] - CLEARED_FRAME_RGB[i]) for i in range(3)) <= 18:
            continue
        saturation = colorsys.rgb_to_hsv(r / 255, g / 255, b / 255)[1]
        if saturation < 0.25:
            continue
        values.append(saturation)
    return round(sum(values) / max(1, len(values)), 4)

before = images["before"]
closed = images["closed"]
after = images["after"]
stale = images["stale"]
diff = ImageChops.difference(before.convert("RGBA"), after.convert("RGBA"))
pixel_difference_count = sum(1 for pixel in diff.getdata() if pixel != (0, 0, 0, 0))

occupancy = {name: non_background(image) for name, image in images.items()}
wrist_boxes = {}
for name in ("before", "closed", "after"):
    components = component_bboxes(images[name], robot_bluegray, min_pixels=80)
    if not components:
        raise SystemExit(f"missing robot source-material blue-gray components in {name}")
    wrist_boxes[name] = components[0]
if any(value is None for value in wrist_boxes.values()):
    raise SystemExit(f"missing robot source-material components: {wrist_boxes}")
wrist_y = [wrist_boxes[name]["center_y"] for name in ("before", "closed", "after")]

box = component_bbox(closed, source_box_red)
if box is None:
    raise SystemExit("missing textured YCB box pixels in closed frame")
robot_points = color_points(closed, robot_bluegray)
near_gripper_points = [
    (x, y)
    for x, y in robot_points
    if box["left"] - 110 <= x <= box["right"] + 110
    and box["top"] - 90 <= y <= box["bottom"] + 140
]
near_gripper = bbox(near_gripper_points)
if near_gripper is None or near_gripper["pixels"] < 300:
    raise SystemExit(f"missing source-material gripper pixels near box: box={box}, gripper={near_gripper}")
brackets_box = (
    near_gripper["left"] < box["center_x"] < near_gripper["right"]
    and near_gripper["top"] < box["bottom"]
    and near_gripper["bottom"] > box["center_y"]
)
gripper_span = near_gripper["width"]
if (not brackets_box) or gripper_span > box["width"] * 4.0:
    raise SystemExit(f"closed gripper does not bracket source-material box: brackets={brackets_box} gripper_span={gripper_span:.2f} box_width={box['width']:.2f}")

live_saturation = max(saturation_mean(before), saturation_mean(closed), saturation_mean(after))
stale_saturation = saturation_mean(stale)
if stale_saturation > live_saturation * 0.82:
    raise SystemExit(f"stale frame is not sufficiently desaturated: stale={stale_saturation} live={live_saturation}")

print(json.dumps({
    "pixel_difference_count": pixel_difference_count,
    "checks": {
        "canvas_occupancy": occupancy,
        "wrist_arc_y_extent_px": {"ok": True, "value": round(max(wrist_y) - min(wrist_y), 2), "centers": [round(v, 2) for v in wrist_y]},
        "closed_grip_around_box": {
            "ok": True,
            "jaw_center_gap_px": round(gripper_span, 2),
            "box_width_px": round(box["width"], 2),
            "brackets_box": brackets_box,
            "source_material_predicate": "red_ycb_box_plus_bluegray_gripper",
        },
        "stale_desaturated_by_scena": {
            "ok": True,
            "stale_saturation": stale_saturation,
            "live_saturation": live_saturation,
        },
        "loading_text_absent": {"ok": True}
    }
}))
`;
  const result = spawnSync("python3", ["-c", code, beforePath, closedPath, afterPath, stalePath], {
    encoding: "utf8",
    maxBuffer: 8 * 1024 * 1024,
  });
  if (result.status !== 0) {
    throw new Error(`failed to analyze screenshots: ${result.stderr || result.stdout}`);
  }
  return JSON.parse(result.stdout);
}

async function renderAndCapture(page, sample, connected, screenshotPath) {
  const state = await renderScene(page, sample, connected);
  await page.evaluate(() => {
    document.querySelectorAll("#breadcrumbs,#alarmBar,#trendOverlay,#meta,.empty").forEach((element) => {
      element.remove();
    });
  });
  await page.locator("#trust-twin-canvas").screenshot({ path: screenshotPath });
  const bodyText = await page.locator("body").innerText();
  if (bodyText.includes("Loading trust-twin panel")) {
    throw new Error(`Loading trust-twin panel text is still visible before ${relative(screenshotPath)}`);
  }
  return state;
}

async function renderScene(page, sample, connected) {
  await page.evaluate((payload) => {
    window.postMessage({ type: "scene", payload }, "*");
  }, scenePayload(sample, connected));
  try {
    await page.waitForFunction(
      () => {
        const canvas = document.querySelector("#trust-twin-canvas");
        const sceneApplyCount = window.__trustTwinSceneApplyCount || 0;
        const renderFrameCount = window.__trustTwinRenderFrameCount || 0;
        const renderedSceneApplyCount = window.__trustTwinRenderedSceneApplyCount || 0;
        return canvas
          && canvas.width > 0
          && canvas.height > 0
          && window.__trustTwinRendererOrigin
          && sceneApplyCount > 0
          && renderFrameCount > 0
          && renderedSceneApplyCount >= sceneApplyCount
          && !window.__trustTwinRenderError;
      },
      { timeout: 10_000 },
    );
  } catch (error) {
    const diagnostics = await page.evaluate(() => ({
      origin: window.__trustTwinRendererOrigin || "",
      ready: window.__trustTwinRendererWasmReady || null,
      scene_apply_count: window.__trustTwinSceneApplyCount || 0,
      render_frame_count: window.__trustTwinRenderFrameCount || 0,
      rendered_scene_apply_count: window.__trustTwinRenderedSceneApplyCount || 0,
      render_error: window.__trustTwinRenderError || "",
      status: document.querySelector("#status")?.textContent || "",
      body: document.body?.innerText || "",
    })).catch((diagnosticError) => ({ diagnostic_error: String(diagnosticError) }));
    throw new Error(
      `trust-twin scene did not apply and render in Playwright: ${error.message}\nDiagnostics: ${JSON.stringify(diagnostics)}`,
    );
  }
  const renderError = await page.evaluate(() => window.__trustTwinRenderError || "");
  if (renderError) {
    throw new Error(`trust-twin render_frame failed: ${renderError}`);
  }
  await page.evaluate(async () => {
    const origin = window.__trustTwinRendererOrigin || "";
    const frames = origin === "scena_webgpu" ? 8 : 1;
    for (let index = 0; index < frames; index += 1) {
      await new Promise((resolve) => requestAnimationFrame(() => setTimeout(resolve, 25)));
    }
  });
  return page.evaluate(() => {
    const canvas = document.querySelector("#trust-twin-canvas");
    return {
      renderer_origin: window.__trustTwinRendererOrigin,
      canvas: canvas ? { width: canvas.width, height: canvas.height } : null,
      scene_apply_count: window.__trustTwinSceneApplyCount || 0,
      render_frame_count: window.__trustTwinRenderFrameCount || 0,
      rendered_scene_apply_count: window.__trustTwinRenderedSceneApplyCount || 0,
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
    "Main.RobotBoxParentState": sample.box_parent_state,
    "Main.RobotStatusLight": sample.status_emissive === "#22c55e",
  };
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

function escapeHtml(value) {
  return String(value)
    .replaceAll("&", "&amp;")
    .replaceAll("<", "&lt;")
    .replaceAll(">", "&gt;")
    .replaceAll('"', "&quot;");
}

function sha256(bytes) {
  return createHash("sha256").update(bytes).digest("hex");
}

function round(value) {
  return Math.round(value * 100) / 100;
}
