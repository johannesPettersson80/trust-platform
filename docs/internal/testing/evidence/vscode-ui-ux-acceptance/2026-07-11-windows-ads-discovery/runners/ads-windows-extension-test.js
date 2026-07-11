const crypto = require("crypto");
const cp = require("child_process");
const fs = require("fs");
const http = require("http");
const path = require("path");
const vscode = require("vscode");

const evidenceRoot = path.resolve(process.env.TRUST_ADS_VISUAL_EVIDENCE_ROOT);
const screenshotsDir = path.join(evidenceRoot, "screenshots");
const diagnosticsPath = path.join(
  evidenceRoot,
  "json",
  "ads-windows-visual-diagnostics.json"
);
const runMetadataPath = path.join(evidenceRoot, "json", "run-metadata.json");
const stateFile = path.resolve(process.env.TRUST_ADS_UI_FIXTURE_STATE);
const cdpPort = Number(process.env.TRUST_ADS_VISUAL_CDP_PORT || 19971);
const strict = process.env.TRUST_ADS_VISUAL_STRICT !== "0";
const pngHygiene = require(path.resolve(process.env.TRUST_PNG_HYGIENE));

const ext = path.resolve(
  process.env.TRUST_REPO || "/home/johannes/projects/trust-platform-ads-windows-fix",
  "editors/vscode"
);
const WebSocket = require(path.join(ext, "node_modules", "ws"));

const THEMES = [
  { name: "Default Dark Modern", slug: "dark", bodyClass: "vscode-dark" },
  { name: "Default Light+", slug: "light", bodyClass: "vscode-light" },
  {
    name: "Dark High Contrast",
    configName: "Default High Contrast",
    slug: "high-contrast",
    bodyClass: "vscode-high-contrast",
  },
];

const STATES = [
  { id: "sole-runtime", fixture: "sole_runtime" },
  { id: "invalid-host-port", fixture: "sole_runtime" },
  { id: "identity-not-found", fixture: "identity_not_found" },
  { id: "manual-declared", fixture: "manual_declared" },
  { id: "route-required", fixture: "route_required" },
  { id: "multiple-ports", fixture: "multiple_ports" },
];

function sleep(ms) {
  return new Promise((resolve) => setTimeout(resolve, ms));
}

function sha256(file) {
  return crypto.createHash("sha256").update(fs.readFileSync(file)).digest("hex");
}

function edgeDensity(file, geometry) {
  const result = cp.spawnSync(
    "convert",
    [
      file,
      "-crop",
      geometry,
      "+repage",
      "-colorspace",
      "gray",
      "-edge",
      "1",
      "-threshold",
      "5%",
      "-format",
      "%[fx:mean]",
      "info:-",
    ],
    { encoding: "utf8", timeout: 15000, maxBuffer: 1024 * 1024 }
  );
  if (result.status !== 0) {
    throw new Error(
      `ImageMagick paint-integrity check failed for ${file} ${geometry}: ${String(
        result.stderr || result.stdout
      ).trim()}`
    );
  }
  const density = Number(String(result.stdout || "").trim());
  if (!Number.isFinite(density)) {
    throw new Error(
      `ImageMagick returned a non-numeric edge density for ${file} ${geometry}: ${JSON.stringify(
        result.stdout
      )}`
    );
  }
  return density;
}

function imagePaintIntegrity(file, width, height) {
  const crop = (x, y, w, h) => {
    const left = Math.max(0, Math.floor(width * x));
    const top = Math.max(0, Math.floor(height * y));
    const cropWidth = Math.max(1, Math.min(width - left, Math.floor(width * w)));
    const cropHeight = Math.max(
      1,
      Math.min(height - top, Math.floor(height * h))
    );
    return `${cropWidth}x${cropHeight}+${left}+${top}`;
  };
  // These regions are intentionally stable in every captured state and every
  // supported theme. A missing compositor tile can still produce a valid PNG,
  // sensible dimensions, and a varied full-frame histogram, but it removes the
  // text/borders in one or more of these regions and collapses edge density.
  const regions = [
    { id: "explorer-chrome", geometry: crop(0, 0, 0.236, 0.275), minimum: 0.025 },
    { id: "top-toolbar", geometry: crop(0.242, 0, 0.52, 0.096), minimum: 0.045 },
    { id: "canvas-host-card", geometry: crop(0.299, 0.218, 0.403, 0.585), minimum: 0.035 },
    { id: "active-drawer", geometry: crop(0.764, 0.095, 0.236, 0.854), minimum: 0.045 },
    { id: "status-bar", geometry: crop(0, 0.974, 1, 0.026), minimum: 0.065 },
  ];
  const checks = regions.map((region) => {
    const density = edgeDensity(file, region.geometry);
    return {
      region: region.id,
      geometry: region.geometry,
      edge_density: density,
      minimum: region.minimum,
      pass: density >= region.minimum,
    };
  });
  return { passed: checks.every((check) => check.pass), checks };
}

function httpJson(requestPath) {
  return new Promise((resolve, reject) => {
    const request = http.get(
      `http://127.0.0.1:${cdpPort}${requestPath}`,
      (response) => {
        let body = "";
        response.on("data", (chunk) => (body += chunk));
        response.on("end", () => {
          try {
            resolve(JSON.parse(body));
          } catch (error) {
            reject(error);
          }
        });
      }
    );
    request.on("error", reject);
    request.setTimeout(5000, () => request.destroy(new Error("HTTP timeout")));
  });
}

async function waitHttp(requestPath, timeoutMs = 30000) {
  const started = Date.now();
  let lastError;
  while (Date.now() - started < timeoutMs) {
    try {
      return await httpJson(requestPath);
    } catch (error) {
      lastError = error;
      await sleep(250);
    }
  }
  throw lastError || new Error(`Timed out waiting for ${requestPath}`);
}

function connectCdp(webSocketUrl) {
  return new Promise((resolve, reject) => {
    const socket = new WebSocket(webSocketUrl);
    let nextId = 0;
    const pending = new Map();
    socket.on("message", (raw) => {
      const message = JSON.parse(raw.toString());
      if (!message.id || !pending.has(message.id)) return;
      const finish = pending.get(message.id);
      pending.delete(message.id);
      finish(message);
    });
    socket.on("error", reject);
    socket.on("open", () =>
      resolve({
        send(method, params = {}, sessionId) {
          return new Promise((done, fail) => {
            const id = ++nextId;
            const timer = setTimeout(() => {
              pending.delete(id);
              fail(new Error(`CDP timeout: ${method}`));
            }, 15000);
            pending.set(id, (message) => {
              clearTimeout(timer);
              if (message.error) {
                fail(new Error(`${method}: ${JSON.stringify(message.error)}`));
              } else {
                done(message);
              }
            });
            socket.send(JSON.stringify({ id, method, params, sessionId }));
          });
        },
        close() {
          socket.close();
        },
      })
    );
  });
}

async function waitForTargets() {
  const started = Date.now();
  let last = [];
  while (Date.now() - started < 40000) {
    last = await waitHttp("/json");
    const page = last.find((target) => target.type === "page");
    const webview = last.find(
      (target) => target.type === "iframe" && /index\.html/.test(target.url || "")
    );
    if (page && webview) return { page, webview };
    await sleep(350);
  }
  throw new Error(
    `Could not find VS Code page and webview targets: ${JSON.stringify(
      last.map(({ id, type, url }) => ({ id, type, url }))
    )}`
  );
}

function assertion(id, pass, detail) {
  return { id, pass: Boolean(pass), detail };
}

function stateName(theme, state, suffix = "") {
  return `ADS-WIN-01-${theme.slug}-${state.id}${suffix}`;
}

exports.run = async function run() {
  const diagnostics = {
    generated_at: new Date().toISOString(),
    evidence_kind:
      "Real Extension Development Host and real webview; ADS command outcomes are deterministic UI fixtures, not hardware proof.",
    strict,
    themes: THEMES,
    expected_states: [
      "default-ready",
      "service-check-confirmation",
      ...STATES.map((state) => state.id),
      "service-recheck-confirmation",
      "manual-declared-inputs",
      "sole-runtime-browse-variables",
    ],
    run_metadata: JSON.parse(fs.readFileSync(runMetadataPath, "utf8")),
    captures: [],
    failures: [],
  };

  const extension = vscode.extensions.getExtension("trust-platform.trust-lsp");
  if (!extension) throw new Error("truST extension is not installed in the dev host");
  await extension.activate();
  await vscode.commands.executeCommand("workbench.action.closeAuxiliaryBar");
  await vscode.commands.executeCommand("workbench.action.closePanel");
  await vscode.commands.executeCommand("trust-lsp.networkCanvas.open");
  await sleep(6500);
  await vscode.commands.executeCommand("workbench.action.closeAuxiliaryBar");
  await vscode.commands.executeCommand("workbench.action.closePanel");

  const version = await waitHttp("/json/version");
  const { page, webview } = await waitForTargets();
  const cdp = await connectCdp(version.webSocketDebuggerUrl);
  // Current arm64 Electron builds do not all expose Browser.getWindowForTarget.
  // The outer launcher pins --window-size; use the Browser domain only when it
  // is available so capture remains portable across cached VS Code builds.
  try {
    const windowInfo = await cdp.send("Browser.getWindowForTarget", {
      targetId: page.id,
    });
    if (windowInfo.result && windowInfo.result.windowId) {
      await cdp.send("Browser.setWindowBounds", {
        windowId: windowInfo.result.windowId,
        bounds: {
          left: 0,
          top: 0,
          width: 1920,
          height: 1080,
          windowState: "normal",
        },
      });
    }
  } catch (_) {
    // --window-size=1920,1080 is the authoritative fallback.
  }
  await sleep(1000);
  const pageAttached = await cdp.send("Target.attachToTarget", {
    targetId: page.id,
    flatten: true,
  });
  const webviewAttached = await cdp.send("Target.attachToTarget", {
    targetId: webview.id,
    flatten: true,
  });
  const pageSession = pageAttached.result.sessionId;
  const webviewSession = webviewAttached.result.sessionId;
  await cdp.send("Page.enable", {}, pageSession);
  await cdp.send("Runtime.enable", {}, webviewSession);

  async function evalInner(body) {
    const expression = `(function(){try{var f=document.querySelector('iframe');var d=(f&&f.contentDocument)||document;var w=(f&&f.contentWindow)||window;${body}}catch(error){return {__error:error.message,__stack:error.stack};}})()`;
    const response = await cdp.send(
      "Runtime.evaluate",
      { expression, returnByValue: true, awaitPromise: true },
      webviewSession
    );
    const value = response.result && response.result.result
      ? response.result.result.value
      : undefined;
    if (value && value.__error) {
      throw new Error(`inner webview evaluation failed: ${value.__error}`);
    }
    return value;
  }

  async function waitInner(body, label, timeoutMs = 35000) {
    const started = Date.now();
    let value;
    while (Date.now() - started < timeoutMs) {
      value = await evalInner(body);
      if (value) return value;
      await sleep(250);
    }
    throw new Error(`Timed out waiting for ${label}; last=${JSON.stringify(value)}`);
  }

  async function clickSelector(selector) {
    return evalInner(
      `var e=d.querySelector(${JSON.stringify(selector)});if(!e)return {clicked:false};e.click();return {clicked:true,text:(e.textContent||e.getAttribute('aria-label')||'').trim()};`
    );
  }

  async function clickText(pattern) {
    return evalInner(
      `var re=new RegExp(${JSON.stringify(pattern)},'i');var e=[...d.querySelectorAll('button,[role=button]')].find(function(x){return re.test((x.textContent||x.getAttribute('aria-label')||'').trim());});if(!e)return {clicked:false};e.click();return {clicked:true,text:(e.textContent||e.getAttribute('aria-label')||'').trim()};`
    );
  }

  async function setInput(selector, value) {
    return evalInner(
      `var e=d.querySelector(${JSON.stringify(selector)});if(!e)return {set:false};var p=e.tagName==='SELECT'?w.HTMLSelectElement.prototype:(e.tagName==='TEXTAREA'?w.HTMLTextAreaElement.prototype:w.HTMLInputElement.prototype);var setter=Object.getOwnPropertyDescriptor(p,'value').set;setter.call(e,${JSON.stringify(value)});e.dispatchEvent(new w.Event('input',{bubbles:true}));e.dispatchEvent(new w.Event('change',{bubbles:true}));return {set:true,value:e.value};`
    );
  }

  async function selectLocation(value) {
    return evalInner(
      `var e=d.querySelector('input[data-role="ads-location-option"][value=${JSON.stringify(value)}]');if(!e)return {selected:false};e.click();return {selected:true,value:e.value};`
    );
  }

  async function resetDrawers() {
    for (let attempt = 0; attempt < 4; attempt += 1) {
      const result = await evalInner(
        `var as=[...d.querySelectorAll('aside')];if(!as.length)return {closed:false};var e=as[as.length-1].querySelector('button[aria-label="Close"]');if(!e)return {closed:false};e.click();return {closed:true};`
      );
      if (!result || !result.closed) break;
      await sleep(400);
    }
  }

  async function openDiscoverDrawer() {
    await resetDrawers();
    const clicked = await clickText("^Discover$");
    if (!clicked.clicked) {
      throw new Error(`Discover toolbar button not found: ${JSON.stringify(clicked)}`);
    }
    await waitInner(
      `return Boolean(d.querySelector('aside[aria-label="Discover devices"]'));`,
      "Discover devices drawer"
    );
    await sleep(350);
  }

  async function openDiscover() {
    await openDiscoverDrawer();
    await evalInner(
      `var boxes=[...d.querySelectorAll('input[type="checkbox"][aria-label^="Include "]')];for(var box of boxes){if(box.getAttribute('data-role')!=='ads-discovery-flow'&&box.checked)box.click();}return boxes.map(function(box){return {label:box.getAttribute('aria-label'),checked:box.checked,role:box.getAttribute('data-role')};});`
    );
    await waitInner(
      `return d.querySelectorAll('[data-role="ads-find-twincat"]').length===1;`,
      "one Find TwinCAT action"
    );
  }

  async function configure(state) {
    if (state.id === "service-check-confirmation" || state.id === "sole-runtime") {
      await selectLocation("same_computer");
    } else if (state.id === "invalid-host-port") {
      await selectLocation("known_address");
      await setInput('[data-role="ads-host"]', "127.0.0.1:851");
    } else if (state.id === "identity-not-found") {
      await selectLocation("known_address");
      await setInput('[data-role="ads-host"]', "192.168.77.99");
    } else if (state.id === "manual-declared") {
      await selectLocation("known_address");
      await setInput('[data-role="ads-host"]', "192.168.77.11");
      await clickSelector('[data-role="ads-advanced-toggle"]');
      await setInput('[data-role="ads-ams-net-id"]', "100.67.6.217.1.1");
    } else if (state.id === "multiple-ports") {
      await selectLocation("known_address");
      await setInput('[data-role="ads-host"]', "192.168.77.31");
      await clickSelector('[data-role="ads-advanced-toggle"]');
      await setInput('[data-role="ads-custom-ports"]', "9000, 9000");
    }
    await sleep(300);
  }

  async function findTwinCatIdentity(state) {
    if (state.id === "invalid-host-port") return;
    const click = await clickSelector('[data-role="ads-find-twincat"]');
    if (!click.clicked) throw new Error(`Find TwinCAT was not clickable in ${state.id}`);
    if (state.id === "identity-not-found") {
      await waitInner(
        `var text=(d.querySelector('aside[aria-label="Discover devices"]')?.innerText||'');return !text.includes('Finding TwinCAT')&&/TwinCAT/i.test(text)&&/AMS Net ID/i.test(text);`,
        "identity-not-found manual fallback guidance",
        45000
      );
      await sleep(300);
      return;
    }
    await waitInner(
      `var computer=d.querySelector('[data-role="ads-computer"]');var safety=d.querySelector('[data-role="ads-probe-safety"][data-state="confirmation-required"]');var rows=d.querySelectorAll('[data-role="ads-plc-runtime"]');return Boolean(computer&&safety&&rows.length===0);`,
      `${state.id} identity and service-check confirmation`,
      45000
    );
    await sleep(300);
  }

  async function confirmAndCheckAdsServices(state) {
    if (state.id === "invalid-host-port" || state.id === "identity-not-found") {
      return;
    }
    const confirmation = await clickSelector(
      '[data-role="ads-probe-safety-confirmation"]'
    );
    if (!confirmation.clicked) {
      throw new Error(
        `ADS service safety confirmation was not clickable in ${state.id}`
      );
    }
    await waitInner(
      `var e=d.querySelector('[data-role="ads-check-services"]');return Boolean(e&&!e.disabled);`,
      `${state.id} enabled ADS service check action`
    );
    const check = await clickSelector('[data-role="ads-check-services"]');
    if (!check.clicked) {
      throw new Error(`Check ADS services was not clickable in ${state.id}`);
    }
    const expectedRows = state.id === "route-required" ? 1 : state.id === "multiple-ports" ? 7 : 6;
    await waitInner(
      `var rows=[...d.querySelectorAll('[data-role="ads-plc-runtime"]')];var probing=(d.body.innerText||'').includes('Checking PLC runtimes');return rows.length===${expectedRows}&&!probing;`,
      `${state.id} PLC runtime results`,
      45000
    );
    await sleep(300);
  }

  async function executeState(state) {
    await findTwinCatIdentity(state);
    await confirmAndCheckAdsServices(state);
  }

  async function focusForCapture(state) {
    return evalInner(
      `var selectors=${JSON.stringify(
        state.id === "invalid-host-port"
          ? ['[data-role="ads-advanced-toggle"]']
          : state.id === "service-check-confirmation" ||
              state.id === "service-recheck-confirmation"
            ? ['[data-role="ads-probe-safety-confirmation"]']
          : state.id === "identity-not-found"
            ? ['[data-role="ads-find-twincat"]']
            : state.id === "route-required"
            ? ['[data-role="ads-route-setup"]']
            : state.id === "multiple-ports"
              ? ['input[aria-label^="Select PLC runtime 1"][aria-label*="ADS 851"]']
              : ['[data-role="ads-browse-variables"]']
      )};var e;for(var s of selectors){var candidate=d.querySelector(s);if(candidate&&!candidate.disabled){e=candidate;break;}}if(!e)return {focused:false};e.focus();var c=w.getComputedStyle(e);return {focused:d.activeElement===e,text:(e.textContent||e.getAttribute('aria-label')||'').trim(),outlineStyle:c.outlineStyle,outlineWidth:c.outlineWidth,outlineColor:c.outlineColor,borderWidth:c.borderWidth,borderColor:c.borderColor};`
    );
  }

  async function domSnapshot(theme, state) {
    return evalInner(
      `var pane=d.querySelector('aside[aria-label="Discover devices"]')||d.querySelector('aside[aria-label="Browse variables"]')||d.querySelector('aside[aria-label="Browse tags"]');var rect=pane&&pane.getBoundingClientRect();var footer=pane&&pane.lastElementChild;var footerRect=footer&&footer.getBoundingClientRect();var flow=d.querySelectorAll('[data-role="ads-discovery-flow"]');var find=d.querySelectorAll('[data-role="ads-find-twincat"]');var computers=d.querySelectorAll('[data-role="ads-computer"]');var includeRows=[...d.querySelectorAll('input[type="checkbox"][aria-label^="Include "]')].map(function(e){return {label:e.getAttribute('aria-label'),checked:e.checked,disabled:e.disabled,role:e.getAttribute('data-role')};});var rows=[...d.querySelectorAll('[data-role="ads-plc-runtime"]')].map(function(e){var input=e.querySelector('input[type="radio"]');return {port:Number(e.getAttribute('data-ads-port')),status:e.getAttribute('data-status'),text:(e.innerText||'').replace(/\\s+/g,' ').trim(),selectable:Boolean(input),checked:Boolean(input&&input.checked),disabled:Boolean(input&&input.disabled)};});var fields=[...d.querySelectorAll('input[data-role],select[data-role]')].map(function(e){return {role:e.getAttribute('data-role'),value:e.value,placeholder:e.getAttribute('placeholder'),checked:e.type==='radio'||e.type==='checkbox'?e.checked:undefined,disabled:e.disabled,invalid:e.getAttribute('aria-invalid')};});var buttons=[...d.querySelectorAll('button')].map(function(e){return {role:e.getAttribute('data-role'),text:(e.innerText||e.getAttribute('aria-label')||'').replace(/\\s+/g,' ').trim(),disabled:e.disabled,title:e.title||'',state:e.getAttribute('data-state')};}).filter(function(e){return e.text;});var offenders=[];if(pane){for(var e of pane.querySelectorAll('*')){if(e.clientWidth>0&&e.scrollWidth>e.clientWidth+2&&!['INPUT','SELECT','TEXTAREA','PRE','CODE'].includes(e.tagName)){var c=w.getComputedStyle(e);if(!['auto','scroll','hidden','clip'].includes(c.overflowX)){offenders.push({tag:e.tagName,text:(e.textContent||'').replace(/\\s+/g,' ').trim().slice(0,100),clientWidth:e.clientWidth,scrollWidth:e.scrollWidth,overflowX:c.overflowX});}}}}var wraps=[...d.querySelectorAll('.trust-field__message--error,.trust-help,[data-role="ads-identity-status"]')].filter(function(e){return (e.textContent||'').trim().length>20;}).map(function(e){var c=w.getComputedStyle(e);return {text:(e.textContent||'').replace(/\\s+/g,' ').trim(),clientWidth:e.clientWidth,scrollWidth:e.scrollWidth,height:e.getBoundingClientRect().height,lineHeight:c.lineHeight,whiteSpace:c.whiteSpace,overflowWrap:c.overflowWrap};});var identityRecovery=d.querySelector('[data-role="ads-empty-result"]');var probeSafety=d.querySelector('[data-role="ads-probe-safety"]');var staleResults=d.querySelector('[data-role="ads-results-stale"]');var focus=d.activeElement;var focusStyle=focus?w.getComputedStyle(focus):undefined;var paneStyle=pane?w.getComputedStyle(pane):undefined;return {theme:${JSON.stringify(theme.name)},expectedThemeClass:${JSON.stringify(theme.bodyClass)},state:${JSON.stringify(state.id)},bodyClass:d.body.className,themeKind:d.body.getAttribute('data-vscode-theme-kind'),drawerText:pane?(pane.innerText||'').replace(/\\s+/g,' ').trim():'',flowCount:flow.length,findTwinCatCount:find.length,computerCount:computers.length,identityRecovery:identityRecovery?{state:identityRecovery.getAttribute('data-state'),text:(identityRecovery.innerText||'').replace(/\\s+/g,' ').trim()}:null,probeSafety:probeSafety?{state:probeSafety.getAttribute('data-state'),text:(probeSafety.innerText||'').replace(/\\s+/g,' ').trim()}:null,staleResults:staleResults?{state:staleResults.getAttribute('data-state'),text:(staleResults.innerText||'').replace(/\\s+/g,' ').trim()}:null,includeRows:includeRows,whereLabel:(d.body.innerText||'').includes('Where is TwinCAT?'),originLabel:(d.body.innerText||'').includes('Discovery runs from'),locationLabels:[...d.querySelectorAll('[data-role="ads-location"] label')].map(function(e){return (e.innerText||'').replace(/\\s+/g,' ').trim();}),rows:rows,fields:fields,buttons:buttons,geometry:rect?{left:rect.left,right:rect.right,top:rect.top,bottom:rect.bottom,width:rect.width,height:rect.height,clientWidth:pane.clientWidth,scrollWidth:pane.scrollWidth,clientHeight:pane.clientHeight,scrollHeight:pane.scrollHeight,viewportWidth:w.innerWidth,viewportHeight:w.innerHeight,footer:footerRect?{top:footerRect.top,bottom:footerRect.bottom,height:footerRect.height}:null,borderLeftWidth:paneStyle.borderLeftWidth,borderLeftColor:paneStyle.borderLeftColor,backgroundColor:paneStyle.backgroundColor}:null,horizontalOverflowOffenders:offenders,wrapping:wraps,focus:focus?{tag:focus.tagName,text:(focus.textContent||focus.getAttribute('aria-label')||'').trim(),outlineStyle:focusStyle.outlineStyle,outlineWidth:focusStyle.outlineWidth,outlineColor:focusStyle.outlineColor,borderWidth:focusStyle.borderWidth,borderColor:focusStyle.borderColor}:null};`
    );
  }

  function evaluateAssertions(theme, state, dom, focused) {
    const checks = [];
    const hasText = (needle) => dom.drawerText.includes(needle);
    checks.push(assertion("single-twincat-flow", dom.flowCount === 1, dom.flowCount));
    checks.push(assertion("one-find-twincat-action", dom.findTwinCatCount === 1, dom.findTwinCatCount));
    if (state.id === "default-ready") {
      const twinCatRows = dom.includeRows.filter((row) =>
        /^Include TwinCAT$/i.test(row.label)
      );
      const find = dom.buttons.find(
        (button) => button.role === "ads-find-twincat"
      );
      const scan = dom.buttons.find((button) => button.role === "scan-selected");
      checks.push(
        assertion(
          "default-one-twincat-row",
          twinCatRows.length === 1 && twinCatRows[0].checked,
          twinCatRows
        )
      );
      checks.push(
        assertion(
          "default-distinct-find-and-multi-scan",
          find && !find.disabled && scan && !scan.disabled &&
            find.text === "Find TwinCAT" &&
            /^Scan \d+ selected types?$/.test(scan.text),
          { find, scan }
        )
      );
      checks.push(
        assertion(
          "default-other-recommended-selected",
          dom.includeRows.filter(
            (row) => row.checked && !/^Include TwinCAT$/i.test(row.label)
          ).length >= 1,
          dom.includeRows
        )
      );
      checks.push(
        assertion(
          "default-confirmed-ads-service-copy",
          /851/.test(dom.drawerText) &&
            /854/.test(dom.drawerText) &&
            /301/.test(dom.drawerText) &&
            /501/.test(dom.drawerText) &&
            /confirm that other software/i.test(dom.drawerText) &&
            /then check/i.test(dom.drawerText),
          dom.drawerText
        )
      );
    } else {
      checks.push(
        assertion(
          "no-redundant-generic-scan-action",
          !dom.buttons.some((button) => button.role === "scan-selected"),
          dom.buttons.filter((button) => button.role === "scan-selected")
        )
      );
    }
    checks.push(assertion("origin-label", dom.originLabel, dom.drawerText.slice(0, 300)));
    checks.push(assertion("target-label", dom.whereLabel, dom.drawerText.slice(0, 300)));
    checks.push(
      assertion(
        "location-language",
        dom.locationLabels.some((label) => /^On the discovery computer$/i.test(label)) &&
          dom.locationLabels.some((label) => /^On the discovery computer's network\b/i.test(label)) &&
          dom.locationLabels.some((label) => /^At (a )?known address$/i.test(label)),
        dom.locationLabels
      )
    );
    checks.push(
      assertion(
        "drawer-width-340",
        dom.geometry && dom.geometry.width >= 338 && dom.geometry.width <= 342,
        dom.geometry
      )
    );
    checks.push(
      assertion(
        "no-horizontal-overflow",
        dom.geometry &&
          dom.geometry.scrollWidth <= dom.geometry.clientWidth + 2 &&
          dom.horizontalOverflowOffenders.length === 0,
        { geometry: dom.geometry, offenders: dom.horizontalOverflowOffenders }
      )
    );
    checks.push(
      assertion(
        "footer-visible",
        dom.geometry &&
          dom.geometry.footer &&
          dom.geometry.footer.top >= dom.geometry.top &&
          dom.geometry.footer.bottom <= dom.geometry.viewportHeight + 1,
        dom.geometry && dom.geometry.footer
      )
    );
    checks.push(
      assertion(
        "theme-applied",
        String(dom.bodyClass).includes(theme.bodyClass),
        dom.bodyClass
      )
    );
    checks.push(
      assertion(
        "long-copy-wraps",
        dom.wrapping.every((item) => item.scrollWidth <= item.clientWidth + 2),
        dom.wrapping
      )
    );
    checks.push(
      assertion(
        "status-text-not-color-only",
        dom.rows.every((row) =>
          /(variable|Available|Not running|unavailable|Route setup required|Check failed|no variables)/i.test(
            row.text
          )
        ),
        dom.rows
      )
    );

    if (theme.slug === "high-contrast") {
      checks.push(
        assertion(
          "high-contrast-border",
          dom.geometry && parseFloat(dom.geometry.borderLeftWidth) >= 1,
          dom.geometry
        )
      );
      checks.push(
        assertion(
          "high-contrast-focus",
          focused && focused.focused &&
            (parseFloat(focused.outlineWidth) >= 1 || parseFloat(focused.borderWidth) >= 1),
          focused
        )
      );
    }

    if (["sole-runtime", "manual-declared", "multiple-ports"].includes(state.id)) {
      const recheck = dom.buttons.find(
        (button) => button.role === "ads-recheck-services"
      );
      checks.push(
        assertion(
          "completed-results-offer-safe-recheck",
          recheck && recheck.text === "Check services again" && !recheck.disabled,
          recheck
        )
      );
    }

    if (["service-check-confirmation", "sole-runtime"].includes(state.id)) {
      checks.push(
        assertion(
          "same-computer-uses-friendly-local-router-identity",
          /TwinCAT computer · On the discovery computer/i.test(dom.drawerText) &&
            !/TwinCAT computer · 127\.0\.0\.1\b/i.test(dom.drawerText),
          dom.drawerText
        )
      );
    }

    if (state.id === "service-check-confirmation") {
      const confirmation = dom.fields.find(
        (field) => field.role === "ads-probe-safety-confirmation"
      );
      const checkServices = dom.buttons.find(
        (button) => button.role === "ads-check-services"
      );
      checks.push(
        assertion(
          "service-check-retains-found-computer",
          dom.computerCount === 1 && dom.rows.length === 0,
          { computerCount: dom.computerCount, rows: dom.rows }
        )
      );
      checks.push(
        assertion(
          "service-check-explains-temporary-client",
          dom.probeSafety &&
            dom.probeSafety.state === "confirmation-required" &&
            dom.probeSafety.text.includes(
              "Checking opens a temporary ADS connection from This computer. Before checking, stop any truST runtime or other software there that is currently reading TwinCAT. Leave TwinCAT and the PLC running."
            ),
          dom.probeSafety
        )
      );
      checks.push(
        assertion(
          "service-check-requires-explicit-confirmation",
          confirmation &&
            confirmation.checked === false &&
            confirmation.disabled === false &&
            hasText(
              "I stopped other software on This computer that is reading TwinCAT"
            ),
          { confirmation, drawerText: dom.drawerText }
        )
      );
      checks.push(
        assertion(
          "service-check-action-disabled-before-confirmation",
          checkServices &&
            checkServices.text === "Check 6 ADS services" &&
            checkServices.disabled,
          checkServices
        )
      );
    } else if (state.id === "service-recheck-confirmation") {
      const confirmation = dom.fields.find(
        (field) => field.role === "ads-probe-safety-confirmation"
      );
      const customPorts = dom.fields.find(
        (field) => field.role === "ads-custom-ports"
      );
      const checkServices = dom.buttons.find(
        (button) => button.role === "ads-check-services"
      );
      const browse = dom.buttons.find(
        (button) => button.role === "ads-browse-variables"
      );
      checks.push(
        assertion(
          "service-recheck-keeps-prior-results",
          dom.rows.length === 7 &&
            dom.rows.some((row) => row.port === 851 && row.status === "available") &&
            dom.rows.some((row) => row.port === 9000) &&
            !dom.rows.some((row) => row.port === 9001),
          dom.rows
        )
      );
      checks.push(
        assertion(
          "service-recheck-shows-changed-advanced-ports",
          customPorts && customPorts.value === "9000, 9001",
          customPorts
        )
      );
      checks.push(
        assertion(
          "service-recheck-marks-prior-results-stale",
          dom.staleResults &&
            dom.staleResults.state === "ports-changed" &&
            dom.staleResults.text ===
              "These results use the previous ADS service list. Check the updated services before selecting or browsing one.",
          dom.staleResults
        )
      );
      checks.push(
        assertion(
          "service-recheck-disables-stale-selection-and-browse",
          dom.rows.filter((row) => row.selectable).length === 2 &&
            dom.rows.filter((row) => row.selectable).every((row) => row.disabled) &&
            browse &&
            browse.disabled &&
            /settings changed.*updated services/i.test(browse.title),
          { rows: dom.rows, browse }
        )
      );
      checks.push(
        assertion(
          "service-recheck-requires-fresh-confirmation",
          dom.probeSafety &&
            dom.probeSafety.state === "confirmation-required" &&
            confirmation &&
            confirmation.checked === false &&
            confirmation.disabled === false &&
            hasText(
              "I stopped other software on This computer that is reading TwinCAT"
            ),
          {
            probeSafety: dom.probeSafety,
            confirmation,
            drawerText: dom.drawerText,
          }
        )
      );
      checks.push(
        assertion(
          "service-recheck-action-disabled",
          checkServices &&
            checkServices.state === "ports-changed" &&
            checkServices.text === "Check updated ADS services" &&
            checkServices.disabled,
          checkServices
        )
      );
    } else if (state.id === "sole-runtime") {
      const row851 = dom.rows.find((row) => row.port === 851);
      const browse = dom.buttons.find((button) => button.role === "ads-browse-variables");
      checks.push(assertion("sole-runtime-available-copy", row851 && /Available/i.test(row851.text) && /12 variables/i.test(row851.text), row851));
      checks.push(assertion("sole-runtime-auto-selected", row851 && row851.checked, row851));
      checks.push(assertion("browse-variables-enabled", browse && !browse.disabled, browse));
      checks.push(
        assertion(
          "sole-runtime-default-service-plan",
          JSON.stringify(dom.rows.map((row) => row.port)) ===
            JSON.stringify([851, 852, 853, 854, 301, 501]),
          dom.rows
        )
      );
    } else if (state.id === "invalid-host-port") {
      const find = dom.buttons.find((button) => button.role === "ads-find-twincat");
      checks.push(assertion("host-port-inline-error", /without a port/i.test(dom.drawerText), dom.drawerText));
      checks.push(assertion("invalid-no-zero-found", !/0 found|Nothing found/i.test(dom.drawerText), dom.drawerText));
      checks.push(assertion("invalid-find-disabled", find && find.disabled, find));
      checks.push(assertion("invalid-has-no-results", dom.rows.length === 0, dom.rows));
    } else if (state.id === "identity-not-found") {
      checks.push(
        assertion(
          "identity-miss-no-naked-zero-found",
          !/\b0 found\b/i.test(dom.drawerText),
          dom.drawerText
        )
      );
      checks.push(
        assertion(
          "identity-miss-contextual-guidance",
          /TwinCAT/i.test(dom.drawerText) &&
            /Advanced/i.test(dom.drawerText) &&
            /AMS Net ID/i.test(dom.drawerText) &&
            /manual/i.test(dom.drawerText),
          dom.drawerText
        )
      );
      checks.push(
        assertion(
          "identity-miss-is-visible-error-recovery",
          dom.identityRecovery &&
            dom.identityRecovery.state === "error" &&
            /TwinCAT identity did not answer UDP discovery/i.test(dom.drawerText) &&
            !/UdpIdentifyBlocked/i.test(dom.drawerText) &&
            /Enter AMS Net ID/i.test(dom.identityRecovery.text),
          {
            identityRecovery: dom.identityRecovery,
            drawerText: dom.drawerText,
          }
        )
      );
      checks.push(
        assertion(
          "identity-miss-no-result-card",
          dom.computerCount === 0 && dom.rows.length === 0,
          { computerCount: dom.computerCount, rows: dom.rows }
        )
      );
    } else if (state.id === "manual-declared") {
      const host = dom.fields.find((field) => field.role === "ads-host");
      const netId = dom.fields.find((field) => field.role === "ads-ams-net-id");
      checks.push(assertion("manual-host-visible", host && host.value === "192.168.77.11", host));
      checks.push(assertion("manual-net-id-visible", netId && netId.value === "100.67.6.217.1.1", netId));
      checks.push(assertion("manual-unverified-copy", /Entered manually.*identity not verified yet/i.test(dom.drawerText), dom.drawerText));
    } else if (state.id === "route-required") {
      const route = dom.buttons.find((button) => button.role === "ads-route-setup");
      const confirmation = dom.fields.find(
        (field) => field.role === "ads-probe-safety-confirmation"
      );
      const recheck = dom.buttons.find(
        (button) => button.role === "ads-check-services"
      );
      checks.push(assertion("route-retains-computer", /ROUTE-REQUIRED-TWINCAT/.test(dom.drawerText), dom.drawerText));
      checks.push(assertion("route-status", /Route setup required/.test(dom.drawerText), dom.drawerText));
      checks.push(assertion("route-action", route && !route.disabled && /Set up route/.test(route.text), route));
      checks.push(
        assertion(
          "route-recheck-needs-fresh-confirmation",
          confirmation &&
            !confirmation.checked &&
            recheck &&
            recheck.text === "Check services again" &&
            recheck.disabled,
          { confirmation, recheck }
        )
      );
    } else if (state.id === "multiple-ports") {
      const usable = dom.rows.filter((row) => row.selectable);
      const browse = dom.buttons.find((button) => button.role === "ads-browse-variables");
      checks.push(assertion("multiple-two-usable", usable.length === 2 && usable.some((row) => row.port === 851) && usable.some((row) => row.port === 853), usable));
      checks.push(assertion("multiple-needs-selection", usable.every((row) => !row.checked), usable));
      checks.push(assertion("multiple-visible-reason", /Choose a TwinCAT service before browsing variables/i.test(dom.drawerText), dom.drawerText));
      checks.push(assertion("multiple-browse-disabled", browse && browse.disabled, browse));
      checks.push(
        assertion(
          "default-and-custom-ports-reported",
          dom.rows.some(
            (row) =>
              row.port === 301 &&
              /Additional task 1/i.test(row.text) &&
              /Available.*not supported/i.test(row.text)
          ) &&
            dom.rows.some(
              (row) =>
                row.port === 501 &&
                /NC SAF service/i.test(row.text) &&
                /Not running|unavailable/i.test(row.text)
            ) &&
            dom.rows.some(
              (row) =>
                row.port === 9000 &&
                /Custom service/i.test(row.text) &&
                /Available.*no variables/i.test(row.text)
            ) &&
            JSON.stringify(dom.rows.map((row) => row.port)) ===
              JSON.stringify([851, 852, 853, 854, 301, 501, 9000]),
          dom.rows
        )
      );
    }
    return checks;
  }

  async function screenshot(name) {
    try {
      await vscode.commands.executeCommand("notifications.clearAll");
    } catch (_) {
      // Notifications are auxiliary to this evidence surface.
    }
    const destination = path.join(screenshotsDir, `${name}.png`);
    const attemptsDir = path.join(evidenceRoot, "runner-output", "capture-attempts");
    fs.mkdirSync(attemptsDir, { recursive: true });
    const attempts = [];
    for (let attempt = 1; attempt <= 3; attempt += 1) {
      try {
        await cdp.send("Page.bringToFront", {}, pageSession);
      } catch (_) {
        // Some cached Electron builds do not expose bringToFront on this target.
      }
      await evalInner(
        `d.documentElement.getBoundingClientRect();return new Promise(function(resolve){w.requestAnimationFrame(function(){w.requestAnimationFrame(function(){resolve(true);});});});`
      );
      await sleep(250 + attempt * 150);
      const captured = await cdp.send(
        "Page.captureScreenshot",
        { format: "png", fromSurface: true },
        pageSession
      );
      const data = captured.result && captured.result.data;
      if (!data) throw new Error(`No PNG returned for ${name} attempt ${attempt}`);
      const attemptFile = path.join(attemptsDir, `${name}-${attempt}.png`);
      pngHygiene.writePngBase64(attemptFile, data);
      const valid = pngHygiene.assertValidCapture(attemptFile, {
        minBytes: 20000,
        minWidth: 1200,
        minHeight: 700,
      });
      const paintIntegrity = imagePaintIntegrity(
        attemptFile,
        valid.width,
        valid.height
      );
      attempts.push({
        attempt,
        sha256: sha256(attemptFile),
        paint_integrity: paintIntegrity,
      });
      if (!paintIntegrity.passed) {
        continue;
      }
      pngHygiene.copyPngStripped(attemptFile, destination);
      return {
        path: path.relative(evidenceRoot, destination),
        bytes: fs.statSync(destination).size,
        width: valid.width,
        height: valid.height,
        sha256: sha256(destination),
        pixelSha256: pngHygiene.pixelSha256File(destination),
        paint_integrity: paintIntegrity,
        capture_attempt: attempt,
        rejected_attempts: attempts.filter(
          (candidate) => !candidate.paint_integrity.passed
        ),
      };
    }
    throw new Error(
      `${name}: all screenshot attempts failed image paint-integrity checks: ${JSON.stringify(
        attempts
      )}`
    );
  }

  try {
    for (const theme of THEMES) {
      await vscode.workspace
        .getConfiguration("workbench")
        .update(
          "colorTheme",
          theme.configName || theme.name,
          vscode.ConfigurationTarget.Global
        );
      await sleep(1800);
      await waitInner(
        `return d.body.className.includes(${JSON.stringify(theme.bodyClass)});`,
        `${theme.name} webview theme`
      );

      await openDiscoverDrawer();
      const defaultState = { id: "default-ready" };
      const defaultFocused = await evalInner(
        `var e=d.querySelector('[data-role="ads-find-twincat"]');if(!e)return {focused:false};e.focus();var c=w.getComputedStyle(e);return {focused:d.activeElement===e,text:(e.textContent||'').trim(),outlineStyle:c.outlineStyle,outlineWidth:c.outlineWidth,outlineColor:c.outlineColor,borderWidth:c.borderWidth,borderColor:c.borderColor};`
      );
      const defaultDom = await domSnapshot(theme, defaultState);
      const defaultAssertions = evaluateAssertions(
        theme,
        defaultState,
        defaultDom,
        defaultFocused
      );
      const defaultImage = await screenshot(stateName(theme, defaultState));
      defaultAssertions.push(
        assertion(
          "image-paint-integrity",
          defaultImage.paint_integrity.passed,
          defaultImage.paint_integrity
        )
      );
      diagnostics.captures.push({
        theme: theme.name,
        theme_slug: theme.slug,
        state: defaultState.id,
        fixture_state: "not-invoked",
        deterministic_fixture: false,
        image: defaultImage,
        dom: defaultDom,
        focused: defaultFocused,
        assertions: defaultAssertions,
        passed: defaultAssertions.every((check) => check.pass),
      });
      for (const check of defaultAssertions.filter((item) => !item.pass)) {
        diagnostics.failures.push({
          theme: theme.name,
          state: defaultState.id,
          assertion: check.id,
          detail: check.detail,
        });
      }
      await resetDrawers();

      const confirmationState = {
        id: "service-check-confirmation",
        fixture: "sole_runtime",
      };
      fs.writeFileSync(
        stateFile,
        JSON.stringify(
          { state: confirmationState.fixture, theme: theme.name },
          null,
          2
        ) + "\n"
      );
      await openDiscover();
      await configure(confirmationState);
      await findTwinCatIdentity(confirmationState);
      const confirmationFocused = await focusForCapture(confirmationState);
      const confirmationDom = await domSnapshot(theme, confirmationState);
      const confirmationAssertions = evaluateAssertions(
        theme,
        confirmationState,
        confirmationDom,
        confirmationFocused
      );
      const confirmationImage = await screenshot(
        stateName(theme, confirmationState)
      );
      confirmationAssertions.push(
        assertion(
          "image-paint-integrity",
          confirmationImage.paint_integrity.passed,
          confirmationImage.paint_integrity
        )
      );
      diagnostics.captures.push({
        theme: theme.name,
        theme_slug: theme.slug,
        state: confirmationState.id,
        fixture_state: confirmationState.fixture,
        deterministic_fixture: true,
        image: confirmationImage,
        dom: confirmationDom,
        focused: confirmationFocused,
        assertions: confirmationAssertions,
        passed: confirmationAssertions.every((check) => check.pass),
      });
      for (const check of confirmationAssertions.filter((item) => !item.pass)) {
        diagnostics.failures.push({
          theme: theme.name,
          state: confirmationState.id,
          assertion: check.id,
          detail: check.detail,
        });
      }
      await resetDrawers();

      for (const state of STATES) {
        fs.writeFileSync(
          stateFile,
          JSON.stringify({ state: state.fixture, theme: theme.name }, null, 2) + "\n"
        );
        await openDiscover();
        await configure(state);
        if (state.id === "manual-declared") {
          const inputFocused = await evalInner(
            `var e=d.querySelector('[data-role="ads-ams-net-id"]');if(!e)return {focused:false};e.focus();var c=w.getComputedStyle(e);return {focused:d.activeElement===e,text:'AMS Net ID',outlineStyle:c.outlineStyle,outlineWidth:c.outlineWidth,outlineColor:c.outlineColor,borderWidth:c.borderWidth,borderColor:c.borderColor};`
          );
          const inputState = { id: "manual-declared-inputs" };
          const inputDom = await domSnapshot(theme, inputState);
          const host = inputDom.fields.find((field) => field.role === "ads-host");
          const netId = inputDom.fields.find(
            (field) => field.role === "ads-ams-net-id"
          );
          const inputAssertions = evaluateAssertions(
            theme,
            inputState,
            inputDom,
            inputFocused
          ).concat([
            assertion(
              "manual-input-host-visible",
              host && host.value === "192.168.77.11",
              host
            ),
            assertion(
              "manual-input-net-id-visible",
              netId && netId.value === "100.67.6.217.1.1",
              netId
            ),
            assertion(
              "manual-input-confirmed-301-501-copy",
              /301/.test(inputDom.drawerText) &&
                /501/.test(inputDom.drawerText) &&
                /confirmed service check/i.test(inputDom.drawerText) &&
                /at most ten/i.test(inputDom.drawerText),
              inputDom.drawerText
            ),
            assertion(
              "manual-input-no-premature-result",
              inputDom.rows.length === 0,
              inputDom.rows
            ),
          ]);
          const inputImage = await screenshot(
            stateName(theme, inputState)
          );
          inputAssertions.push(
            assertion(
              "image-paint-integrity",
              inputImage.paint_integrity.passed,
              inputImage.paint_integrity
            )
          );
          diagnostics.captures.push({
            theme: theme.name,
            theme_slug: theme.slug,
            state: inputState.id,
            fixture_state: state.fixture,
            deterministic_fixture: true,
            image: inputImage,
            dom: inputDom,
            focused: inputFocused,
            assertions: inputAssertions,
            passed: inputAssertions.every((check) => check.pass),
          });
          for (const check of inputAssertions.filter((item) => !item.pass)) {
            diagnostics.failures.push({
              theme: theme.name,
              state: inputState.id,
              assertion: check.id,
              detail: check.detail,
            });
          }
        }
        await executeState(state);
        const focused = await focusForCapture(state);
        const dom = await domSnapshot(theme, state);
        const assertions = evaluateAssertions(theme, state, dom, focused);
        const image = await screenshot(stateName(theme, state));
        assertions.push(
          assertion(
            "image-paint-integrity",
            image.paint_integrity.passed,
            image.paint_integrity
          )
        );
        const capture = {
          theme: theme.name,
          theme_slug: theme.slug,
          state: state.id,
          fixture_state: state.fixture,
          deterministic_fixture: true,
          image,
          dom,
          focused,
          assertions,
          passed: assertions.every((check) => check.pass),
        };
        diagnostics.captures.push(capture);
        for (const check of assertions.filter((item) => !item.pass)) {
          diagnostics.failures.push({
            theme: theme.name,
            state: state.id,
            assertion: check.id,
            detail: check.detail,
          });
        }

        if (state.id === "sole-runtime") {
          const clicked = await clickSelector('[data-role="ads-browse-variables"]');
          if (!clicked.clicked) throw new Error("Could not click Browse variables");
          await waitInner(
            `var pane=d.querySelector('aside[aria-label="Browse variables"]')||d.querySelector('aside[aria-label="Browse tags"]');var port=d.querySelector('[data-role="ads-browse-port"]');var variables=pane?[...pane.querySelectorAll('input[aria-label^="Select MAIN."]')].filter(function(e){var r=e.getBoundingClientRect();return r.width>0&&r.height>0;}):[];return Boolean(pane&&port&&port.value==='851'&&!((pane.innerText||'').includes('Loading symbols'))&&variables.length>0);`,
            `${theme.name} ADS 851 Browse Variables drawer`,
            45000
          );
          const browseDom = await evalInner(
            `var pane=d.querySelector('aside[aria-label="Browse variables"]')||d.querySelector('aside[aria-label="Browse tags"]');var input=d.querySelector('[data-role="ads-browse-port"]');var search=pane.querySelector('input[placeholder]');var variables=[...pane.querySelectorAll('input[aria-label^="Select MAIN."]')].filter(function(e){var r=e.getBoundingClientRect();return r.width>0&&r.height>0;}).map(function(e){return e.getAttribute('aria-label');});var buttons=[...pane.querySelectorAll('button')].map(function(e){return (e.innerText||e.getAttribute('aria-label')||'').replace(/\\s+/g,' ').trim();}).filter(Boolean);var rect=pane.getBoundingClientRect();return {theme:${JSON.stringify(theme.name)},state:'sole-runtime-browse-variables',ariaLabel:pane.getAttribute('aria-label'),drawerText:(pane.innerText||'').replace(/\\s+/g,' ').trim(),adsPort:input&&input.value,searchPlaceholder:search&&search.getAttribute('placeholder'),variableRows:variables,buttons:buttons,geometry:{left:rect.left,right:rect.right,top:rect.top,bottom:rect.bottom,width:rect.width,height:rect.height,clientWidth:pane.clientWidth,scrollWidth:pane.scrollWidth},bodyClass:d.body.className};`
          );
          const browseAssertions = [
            assertion("browse-drawer-present", Boolean(browseDom.drawerText), browseDom.drawerText),
            assertion(
              "browse-drawer-accessible-name",
              browseDom.ariaLabel === "Browse variables",
              browseDom.ariaLabel
            ),
            assertion(
              "browse-drawer-uses-variables-language",
              /Browse variables/i.test(browseDom.drawerText) &&
                !/Browse tags/i.test(browseDom.drawerText),
              browseDom.drawerText
            ),
            assertion("browse-drawer-ads-851", browseDom.adsPort === "851", browseDom.adsPort),
            assertion("browse-drawer-loaded", !/Loading symbols/i.test(browseDom.drawerText), browseDom.drawerText),
            assertion(
              "browse-drawer-variable-row-visible",
              browseDom.variableRows.length > 0,
              browseDom.variableRows
            ),
            assertion(
              "browse-drawer-search-variables",
              browseDom.searchPlaceholder === "Search variables",
              browseDom.searchPlaceholder
            ),
            assertion(
              "browse-drawer-variable-actions",
              browseDom.buttons.includes("Browse variables") &&
                browseDom.buttons.includes("Add variables"),
              browseDom.buttons
            ),
            assertion(
              "browse-drawer-no-old-action-vocabulary",
              !browseDom.buttons.some((label) =>
                /^(Browse tags|Browse symbols|Add tags)$/i.test(label)
              ) && browseDom.searchPlaceholder !== "Search symbols",
              { buttons: browseDom.buttons, searchPlaceholder: browseDom.searchPlaceholder }
            ),
            assertion("browse-drawer-no-overflow", browseDom.geometry.scrollWidth <= browseDom.geometry.clientWidth + 2, browseDom.geometry),
          ];
          const browseImage = await screenshot(
            stateName(theme, state, "-browse-variables")
          );
          browseAssertions.push(
            assertion(
              "image-paint-integrity",
              browseImage.paint_integrity.passed,
              browseImage.paint_integrity
            )
          );
          diagnostics.captures.push({
            theme: theme.name,
            theme_slug: theme.slug,
            state: "sole-runtime-browse-variables",
            fixture_state: state.fixture,
            deterministic_fixture: true,
            image: browseImage,
            dom: browseDom,
            assertions: browseAssertions,
            passed: browseAssertions.every((check) => check.pass),
          });
          for (const check of browseAssertions.filter((item) => !item.pass)) {
            diagnostics.failures.push({
              theme: theme.name,
              state: "sole-runtime-browse-variables",
              assertion: check.id,
              detail: check.detail,
            });
          }
        }

        if (state.id === "multiple-ports") {
          await setInput('[data-role="ads-custom-ports"]', "9000, 9001");
          await waitInner(
            `var rows=d.querySelectorAll('[data-role="ads-plc-runtime"]');var safety=d.querySelector('[data-role="ads-probe-safety"][data-state="confirmation-required"]');var confirmation=d.querySelector('[data-role="ads-probe-safety-confirmation"]');var button=d.querySelector('[data-role="ads-check-services"][data-state="ports-changed"]');return Boolean(rows.length===7&&safety&&confirmation&&!confirmation.checked&&button&&button.disabled&&(button.innerText||'').trim()==='Check updated ADS services');`,
            `${theme.name} changed ADS ports require fresh confirmation`,
            45000
          );
          const armedRecheck = await clickSelector(
            '[data-role="ads-probe-safety-confirmation"]'
          );
          if (!armedRecheck.clicked) {
            throw new Error(
              `${theme.name} recheck confirmation was not clickable`
            );
          }
          await waitInner(
            `var confirmation=d.querySelector('[data-role="ads-probe-safety-confirmation"]');var button=d.querySelector('[data-role="ads-check-services"]');return Boolean(confirmation&&confirmation.checked&&button&&!button.disabled);`,
            `${theme.name} armed ADS service recheck`
          );
          await setInput('[data-role="ads-custom-ports"]', "9000, 9002");
          await waitInner(
            `var confirmation=d.querySelector('[data-role="ads-probe-safety-confirmation"]');var button=d.querySelector('[data-role="ads-check-services"][data-state="ports-changed"]');return Boolean(confirmation&&!confirmation.checked&&button&&button.disabled);`,
            `${theme.name} port-plan change invalidates prior confirmation`,
            45000
          );
          await setInput('[data-role="ads-custom-ports"]', "9000, 9001");
          await waitInner(
            `var rows=d.querySelectorAll('[data-role="ads-plc-runtime"]');var confirmation=d.querySelector('[data-role="ads-probe-safety-confirmation"]');var button=d.querySelector('[data-role="ads-check-services"][data-state="ports-changed"]');return Boolean(rows.length===7&&confirmation&&!confirmation.checked&&button&&button.disabled);`,
            `${theme.name} final ADS service recheck confirmation state`,
            45000
          );
          const recheckState = {
            id: "service-recheck-confirmation",
            fixture: state.fixture,
          };
          const recheckFocused = await focusForCapture(recheckState);
          const recheckDom = await domSnapshot(theme, recheckState);
          const recheckAssertions = evaluateAssertions(
            theme,
            recheckState,
            recheckDom,
            recheckFocused
          );
          const recheckImage = await screenshot(
            stateName(theme, recheckState)
          );
          recheckAssertions.push(
            assertion(
              "image-paint-integrity",
              recheckImage.paint_integrity.passed,
              recheckImage.paint_integrity
            )
          );
          diagnostics.captures.push({
            theme: theme.name,
            theme_slug: theme.slug,
            state: recheckState.id,
            fixture_state: recheckState.fixture,
            deterministic_fixture: true,
            image: recheckImage,
            dom: recheckDom,
            focused: recheckFocused,
            assertions: recheckAssertions,
            passed: recheckAssertions.every((check) => check.pass),
          });
          for (const check of recheckAssertions.filter((item) => !item.pass)) {
            diagnostics.failures.push({
              theme: theme.name,
              state: recheckState.id,
              assertion: check.id,
              detail: check.detail,
            });
          }
        }
      }
    }
  } finally {
    diagnostics.capture_count = diagnostics.captures.length;
    diagnostics.expected_minimum_capture_count = 33;
    diagnostics.all_images = diagnostics.captures.map((capture) => capture.image.path);
    diagnostics.png_hygiene = pngHygiene.validateCaptureTree(evidenceRoot, {
      roots: ["screenshots"],
      minBytes: 20000,
      minWidth: 1200,
      minHeight: 700,
      rejectDuplicates: true,
    }).valid.map((item) => ({
      path: path.relative(evidenceRoot, item.file),
      width: item.width,
      height: item.height,
      size: item.size,
      singleColorRatio: item.singleColorRatio,
    }));
    diagnostics.passed =
      diagnostics.capture_count >= diagnostics.expected_minimum_capture_count &&
      diagnostics.failures.length === 0;
    fs.writeFileSync(diagnosticsPath, JSON.stringify(diagnostics, null, 2) + "\n");
    cdp.close();
  }

  if (strict && !diagnostics.passed) {
    throw new Error(
      `Visual acceptance failed (${diagnostics.failures.length} assertion failure(s), ${diagnostics.capture_count}/${diagnostics.expected_minimum_capture_count} captures); see ${diagnosticsPath}`
    );
  }
};
