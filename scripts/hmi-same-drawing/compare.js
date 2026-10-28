// Same drawing (specification 39, §19.4): each reference page is drawn by the runtime's /hmi and by
// the HMI Builder's shipped webview in Live mode, both at the page's design size. The screenshots
// are compared pixel by pixel in dark and light themes with the same runtime values.
// Build mode deliberately remains static and is covered by the editor acceptance journeys.
//
//   node scripts/hmi-same-drawing/compare.js <out-dir>
//
// Environment: TRUST_RUNTIME (the trust-runtime binary), PLAYWRIGHT_CORE (the playwright-core
// module, default "playwright-core"), CHROMIUM (the browser executable). Build the webview first
// (`npm run build:hmi-builder` in editors/vscode). Exit status 1 when any page differs.

const fs = require("fs");
const http = require("http");
const net = require("net");
const os = require("os");
const path = require("path");
const cp = require("child_process");
const { pathToFileURL } = require("url");

const { chromium } = require(process.env.PLAYWRIGHT_CORE || "playwright-core");

const repo = path.resolve(__dirname, "../..");
const reference = path.join(__dirname, "reference");
const media = path.join(repo, "editors/vscode/media");
const catalogue = JSON.parse(fs.readFileSync(path.join(repo, "crates/trust-runtime/src/hmi/catalogue/components.json"), "utf8"));
const runtimeBin = process.env.TRUST_RUNTIME || path.join(repo, "target/debug/trust-runtime");
const outDir = path.resolve(process.argv[2] || "hmi-same-drawing");
const PAGES = ["displays", "layout", "pid"];
const THEMES = ["dark", "light"];
/** The largest difference in a colour channel that counts as rasterization noise (§19.4):
 * gradient dithering and the edges of blur effects vary between documents (up to 28 of 255
 * measured, on a gauge's glowing arc). */
const NOISE = 32;
// Both frontends render the same instant. Otherwise even an unbound trend advances its
// time-axis labels every frame and the live DOM can never settle for a pixel comparison.
const RENDER_TIME_MS = Date.now();
async function useRenderTime(tab) {
  await tab.addInitScript(now => {
    // DrawFrame.nowMs and the live store use Date.now; timers and PLC time keep running.
    Date.now = () => now;
  }, RENDER_TIME_MS);
}


const sleep = (ms) => new Promise((resolve) => setTimeout(resolve, ms));

function freePort() {
  return new Promise((resolve, reject) => {
    const server = net.createServer();
    server.once("error", reject);
    server.listen(0, "127.0.0.1", () => {
      const { port } = server.address();
      server.close(() => resolve(port));
    });
  });
}

function getJson(url) {
  return new Promise((resolve, reject) => {
    const request = http.get(url, (response) => {
      let body = "";
      response.on("data", (chunk) => (body += chunk));
      response.on("end", () => {
        try {
          resolve({ status: response.statusCode, body: JSON.parse(body) });
        } catch (error) {
          reject(error);
        }
      });
    });
    request.on("error", reject);
    request.setTimeout(3000, () => request.destroy(new Error("timeout")));
  });
}

async function until(what, check, ms = 60000) {
  const deadline = Date.now() + ms;
  for (;;) {
    try {
      const value = await check();
      if (value) return value;
    } catch (error) {
      if (error.fatal) throw error;
    }
    if (Date.now() > deadline) throw new Error(`timed out waiting for ${what}`);
    await sleep(250);
  }
}

/** A copy of the reference project in the theme, with a runtime configuration serving the web
 * pages on `port`. */
function projectFor(theme, port) {
  const project = fs.mkdtempSync(path.join(os.tmpdir(), `hmi-same-drawing-${theme}-`));
  fs.cpSync(reference, project, { recursive: true });
  fs.cpSync(path.join(repo, "libraries/hmi/src"), path.join(project, "src", "hmi-library"), { recursive: true });
  // The pid page runs the real Control.PID from the shipped control library.
  fs.cpSync(path.join(repo, "libraries/control/src"), path.join(project, "src", "control-library"), { recursive: true });
  const screen = path.join(project, "hmi", "screen.toml");
  fs.writeFileSync(screen, fs.readFileSync(screen, "utf8").replace(/theme = "\w+"/, `theme = "${theme}"`));
  fs.writeFileSync(
    path.join(project, "runtime.toml"),
    [
      "[bundle]",
      "version = 1",
      "",
      "[resource]",
      'name = "Cpu"',
      "cycle_interval_ms = 100",
      "",
      "[runtime.control]",
      'endpoint = "tcp://127.0.0.1:0"',
      'mode = "production"',
      "debug_enabled = false",
      'auth_token = "hmi-same-drawing"',
      "",
      "[runtime.log]",
      'level = "warn"',
      "",
      "[runtime.retain]",
      'mode = "none"',
      "save_interval_ms = 1000",
      "",
      "[runtime.watchdog]",
      "enabled = false",
      "timeout_ms = 5000",
      'action = "halt"',
      "",
      "[runtime.fault]",
      'policy = "halt"',
      "",
      "[runtime.web]",
      "enabled = true",
      `listen = "127.0.0.1:${port}"`,
      'auth = "local"',
      "tls = false",
      "",
      "[runtime.tls]",
      'mode = "disabled"',
      "",
    ].join("\n"),
  );
  return project;
}

async function startRuntime(project, port) {
  cp.execFileSync(runtimeBin, ["build", "--project", project], { stdio: "inherit" });
  const child = cp.spawn(runtimeBin, ["run", "--project", project], { stdio: ["ignore", "inherit", "inherit"] });
  let exited;
  child.on("exit", (code) => (exited = code));
  await until("the runtime's HMI pages", async () => {
    if (exited !== undefined) throw Object.assign(new Error(`the runtime exited with ${exited}`), { fatal: true });
    return (await getJson(`http://127.0.0.1:${port}/api/hmi/page`)).status === 200;
  });
  return child;
}

/** The page as the runtime draws it, and the page model and screen it draws from. */
async function runtimeShot(browser, port, name) {
  const { body } = await getJson(`http://127.0.0.1:${port}/api/hmi/page?name=${encodeURIComponent(name)}`);
  if (!body.screen || !body.page) throw new Error(`the page ${name} does not load: ${JSON.stringify(body.errors ?? body.error)}`);
  const [width, height] = body.screen.size;
  const tab = await browser.newPage({ viewport: { width, height } });
  await useRenderTime(tab);
  const problems = watch(tab);
  const update = { cycle: 0, values: {} };
  tab.on("websocket", socket => socket.on("framereceived", ({payload}) => {
    try { const frame = JSON.parse(String(payload));
      if (frame.values) { Object.assign(update.values, frame.values); update.cycle = frame.cycle ?? update.cycle; }
    } catch { /* Non-value protocol frames do not contribute to the comparison. */ }
  }));
  await tab.goto(`http://127.0.0.1:${port}/hmi/${encodeURIComponent(name)}`);
  await tab.waitForSelector(".hmi-surface > svg [data-object]");
  // The crop compares the page drawing, not the floating frame-owned login bar. Login,
  // roles and session presentation are exercised separately by the runtime browser suite.
  // Do not hide or mask any node within the page SVG.
  await tab.addStyleTag({ content: ".hmi-session { visibility: hidden !important; }" });
  // Live values: the stream's values drawn (none still unknown), and the drawing settled. An
  // unbound object has no value and draws the same on both surfaces.
  const bound = body.page.model.object.some((object) => Object.keys(object.bind).length > 0);
  await until(
    "the live values",
    () => tab.evaluate((bound) => !document.querySelector('[data-state="unknown"]') && (!bound || document.querySelector('[data-state="confirmed"]') !== null), bound),
    20000,
  );
  await tab.evaluate(() => document.fonts.ready);
  let before, after;
  try {
    await until("a settled drawing", async () => {
      before = await tab.evaluate(() => document.querySelector(".hmi-surface").innerHTML);
      await sleep(600);
      after = await tab.evaluate(() => document.querySelector(".hmi-surface").innerHTML);
      return before === after;
    });
  } catch (error) {
    fs.writeFileSync(path.join(outDir, `${body.screen.theme}-${name}-before.html`), before ?? "");
    fs.writeFileSync(path.join(outDir, `${body.screen.theme}-${name}-after.html`), after ?? "");
    await tab.screenshot({ path: path.join(outDir, `${body.screen.theme}-${name}-unsettled.png`) });
    throw error;
  }
  const png = await boxShot(tab, ".hmi-surface");
  await tab.close();
  return { png, model: body.page.model, screen: body.screen, formats: body.formats, update, problems };
}

/** A screenshot of the box an element covers: the drawing inside is redrawn while it is shown, so
 * the box is taken, not the element. */
async function boxShot(tab, selector) {
  const box = await tab.evaluate((selector) => {
    const rect = document.querySelector(selector).getBoundingClientRect();
    return { x: rect.left, y: rect.top, width: rect.width, height: rect.height };
  }, selector);
  return tab.screenshot({ clip: box });
}

function watch(tab) {
  const problems = [];
  tab.on("console", (message) => message.type() === "error" && problems.push(message.text()));
  tab.on("pageerror", (error) => problems.push(String(error)));
  tab.on("requestfailed", (request) => problems.push(`failed ${request.url()}`));
  return problems;
}

/** The same page in the editor: the shipped webview bundle, sent the page model, the catalogue and
 * the screen as the HMI Builder's editor sends them, at zoom 100 %. */
async function editorShot(browser, model, screen, project, update, formats) {
  const harness = path.join(fs.mkdtempSync(path.join(os.tmpdir(), "hmi-same-drawing-editor-")), "index.html");
  const url = (file) => pathToFileURL(file).href;
  fs.writeFileSync(
    harness,
    `<!doctype html><html><head><meta charset="utf-8"><link rel="stylesheet" href="${url(path.join(media, "hmiBuilderWebview.css"))}"></head>
<body><div id="root" data-font-url="${url(path.join(media, "fonts", "InterVariable.woff2"))}" data-asset-base="${url(project)}"></div>
<script src="${url(path.join(media, "hmiBuilderWebview.js"))}"></script></body></html>`,
  );
  const tab = await browser.newPage({ viewport: { width: Math.max(1920, screen.size[0] + 700), height: Math.max(1200, screen.size[1] + 400) } });
  await useRenderTime(tab);
  const problems = watch(tab);
  await tab.addInitScript(() => {
    window.__posted = [];
    window.acquireVsCodeApi = () => ({ postMessage: (message) => window.__posted.push(message), getState: () => undefined, setState: () => undefined });
  });
  await tab.goto(url(harness));
  await until("the webview's ready", () => tab.evaluate(() => window.__posted.some((message) => message.type === "ready")));
  await tab.evaluate(
    ([catalogue, screen, model, formats]) => {
      for (const data of [{ type: "catalogue", catalogue, formats }, { type: "screen", screen }, { type: "page", model }]) {
        window.dispatchEvent(new MessageEvent("message", { data }));
      }
    },
    [catalogue, screen, model, formats],
  );
  await tab.waitForSelector('[data-part="canvas-page"] svg [data-object]');
  for (let step = 0; step < 12; step += 1) {
    const zoom = await tab.evaluate(() => Number.parseInt(document.querySelector(".hb-zoom-value").textContent, 10));
    if (zoom === 100) break;
    await tab.click(zoom > 100 ? 'button[title="Zoom out"]' : 'button[title="Zoom in"]');
    await sleep(100);
  }
  await tab.getByRole("button", { name: "Live", exact: true }).click();
  await tab.evaluate(update => {
    window.dispatchEvent(new MessageEvent("message", { data: { type: "live", state: "connected", target: "Reference runtime" } }));
    window.dispatchEvent(new MessageEvent("message", { data: { type: "liveValues", update } }));
  }, update);
  await tab.waitForFunction(() => !document.querySelector('[data-part="canvas-page"] [data-state="unknown"]'));
  // Gradients are dithered by screen position: the sheet goes to the window's corner, where the
  // runtime draws the page, so both are rasterized alike. Nothing inside the sheet changes.
  await tab.addStyleTag({ content: '[data-part="canvas-page"] { position: fixed !important; left: 0 !important; top: 0 !important; margin: 0 !important; z-index: 100; }' });
  await tab.evaluate(() => document.fonts.ready);
  await sleep(300);
  const png = await boxShot(tab, '[data-part="canvas-page"] .hb-drawing');
  await tab.close();
  return { png, problems };
}

/** Compares two screenshots pixel by pixel in the browser; differing pixels are red in the diff. */
async function compare(browser, first, second) {
  const tab = await browser.newPage();
  const result = await tab.evaluate(
    async ([a, b, tolerance]) => {
      const load = (data) =>
        new Promise((resolve, reject) => {
          const image = new Image();
          image.onload = () => resolve(image);
          image.onerror = reject;
          image.src = `data:image/png;base64,${data}`;
        });
      const [left, right] = await Promise.all([load(a), load(b)]);
      if (left.width !== right.width || left.height !== right.height) {
        return { sizes: [[left.width, left.height], [right.width, right.height]], differing: -1 };
      }
      const pixels = (image) => {
        const canvas = document.createElement("canvas");
        canvas.width = image.width;
        canvas.height = image.height;
        const context = canvas.getContext("2d");
        context.drawImage(image, 0, 0);
        return context.getImageData(0, 0, image.width, image.height);
      };
      const one = pixels(left);
      const two = pixels(right);
      const canvas = document.createElement("canvas");
      canvas.width = left.width;
      canvas.height = left.height;
      const context = canvas.getContext("2d");
      const diff = context.createImageData(left.width, left.height);
      let differing = 0;
      let noise = 0;
      let maxDelta = 0;
      for (let index = 0; index < one.data.length; index += 4) {
        let delta = 0;
        for (let channel = 0; channel < 4; channel += 1) delta = Math.max(delta, Math.abs(one.data[index + channel] - two.data[index + channel]));
        maxDelta = Math.max(maxDelta, delta);
        const grey = (one.data[index] + one.data[index + 1] + one.data[index + 2]) / 12;
        // Red: a real difference; dim orange: noise; grey: the same.
        diff.data[index] = delta > tolerance ? 255 : delta > 0 ? 110 : grey;
        diff.data[index + 1] = delta > tolerance ? 0 : delta > 0 ? 60 : grey;
        diff.data[index + 2] = delta > tolerance ? 0 : grey;
        diff.data[index + 3] = 255;
        if (delta > tolerance) differing += 1;
        else if (delta > 0) noise += 1;
      }
      context.putImageData(diff, 0, 0);
      return { sizes: [[left.width, left.height]], differing, noise, maxDelta, diff: canvas.toDataURL("image/png").split(",")[1] };
    },
    [first.toString("base64"), second.toString("base64"), NOISE],
  );
  await tab.close();
  return result;
}

(async () => {
  fs.mkdirSync(outDir, { recursive: true });
  const browser = await chromium.launch({ executablePath: process.env.CHROMIUM, args: ["--no-sandbox", "--force-device-scale-factor=1"] });
  const report = [];
  let failed = false;
  try {
    for (const theme of THEMES) {
      const port = await freePort();
      const project = projectFor(theme, port);
      const runtime = await startRuntime(project, port);
      try {
        for (const name of PAGES) {
          const live = await runtimeShot(browser, port, name);
          const editor = await editorShot(browser, live.model, live.screen, project, live.update, live.formats);
          const result = await compare(browser, live.png, editor.png);
          const stem = path.join(outDir, `${theme}-${name}`);
          fs.writeFileSync(`${stem}-runtime.png`, live.png);
          fs.writeFileSync(`${stem}-editor.png`, editor.png);
          if (result.diff) fs.writeFileSync(`${stem}-diff.png`, Buffer.from(result.diff, "base64"));
          const entry = { theme, page: name, sizes: result.sizes, differing: result.differing, noise: result.noise, maxDelta: result.maxDelta, problems: [...live.problems, ...editor.problems] };
          report.push(entry);
          failed ||= result.differing !== 0 || entry.problems.length > 0;
          console.log(JSON.stringify(entry));
          if (theme === THEMES[0] && name === PAGES[0]) {
            // The control: one value drawn one digit differently must be seen (60.0 against 60.4).
            const changed = structuredClone(live.update);
            const path = live.model.object.find(object => object.id === "temperature").bind.value.path;
            const key = Object.keys(changed.values).find(key => key.toLowerCase() === path.toLowerCase());
            if (!key || typeof changed.values[key]?.value !== "number") throw new Error("Temperature control has no numeric live value");
            changed.values[key].value += 0.4;
            const control = await compare(browser, live.png, (await editorShot(browser, live.model, live.screen, project, changed, live.formats)).png);
            const seen = control.differing > 0;
            report.push({ control: "one digit changed", differing: control.differing, seen });
            failed ||= !seen;
            console.log(JSON.stringify({ control: "one digit changed", differing: control.differing, seen }));
          }
        }
      } finally {
        runtime.kill();
      }
    }
  } finally {
    await browser.close();
  }
  fs.writeFileSync(path.join(outDir, "report.json"), JSON.stringify(report, null, 2));
  console.log(failed ? "HMI_SAME_DRAWING_DIFFERS" : "HMI_SAME_DRAWING_IDENTICAL");
  process.exit(failed ? 1 : 0);
})().catch((error) => {
  console.error(error);
  process.exit(2);
});
