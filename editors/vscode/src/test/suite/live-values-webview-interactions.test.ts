import * as assert from "assert";
import * as fs from "fs";
import * as os from "os";
import * as path from "path";

const { JSDOM } = require("jsdom") as {
  JSDOM: new (
    html: string,
    options: { runScripts: "outside-only"; url: string },
  ) => any;
};

type PostedMessage = {
  readonly type?: string;
  readonly address?: string;
  readonly value?: string;
};

type LiveValuesHarness = {
  readonly window: any;
  readonly document: any;
  readonly posted: PostedMessage[];
  sendIoState(
    scan: number,
    input?: { readonly value?: string; readonly forced?: boolean },
  ): void;
  /** Sends the panel a message as the host does. */
  send(message: Record<string, unknown>): void;
  /** The status messages' expiries that are pending: the panel's 5000 ms timers, which the harness holds. */
  pendingExpiries(): number;
  /** Lets every pending expiry run, as when 5000 ms passed. */
  expire(): void;
  close(): void;
};

/** The delay after which success feedback expires (specification 25, section 4.2). */
const EXPIRY_MS = 5000;

function extensionRoot(): string {
  return path.resolve(__dirname, "..", "..", "..");
}

/** One row state's operations as the language server answers them (run_targets/live_values.rs, `rows`). */
function operations(
  write: boolean,
  force: boolean,
  release: boolean,
  reasons: { write?: string; force?: string; release?: string } = {},
): Record<string, unknown> {
  return {
    write,
    force,
    release,
    writeReason: write ? null : (reasons.write ?? null),
    forceReason: force ? null : (reasons.force ?? null),
    releaseReason: release ? null : (reasons.release ?? null),
  };
}

/** The Simulator's status as the language server answers it: its session may do everything, an input is written,
 * forced and released, an output and a memory location are forced but never written (specification 25 §4.1, §4.2). */
function simulatorSection(writable: boolean): Record<string, unknown> {
  const sectionReason = writable ? undefined : "Program-driven — use Force to override";
  const readOnly = "Read-only ADS tag. Enable writes when adding the tag, then restart the runtime.";
  return {
    live: operations(writable, true, false, { write: sectionReason }),
    forced: operations(false, false, true, {
      write: "Release force before writing this value.",
      force: "Force is not available for this value.",
    }),
    readOnly: operations(false, false, false, { write: sectionReason ?? readOnly, force: readOnly }),
  };
}

const SIMULATOR_STATUS = {
  running: true,
  runtimeMode: "simulate",
  runtimeState: "running",
  access: { role: "admin", allowWrite: true, allowForce: true, allowRelease: true, reason: null },
  rows: {
    inputs: simulatorSection(true),
    outputs: simulatorSection(false),
    memory: simulatorSection(false),
    ads: simulatorSection(true),
  },
  forceRequiresArm: false,
};

function createHarness(): LiveValuesHarness {
  const dom = new JSDOM(
    `<!doctype html><html><body>
      <input id="filter" />
      <button id="forcedFilter" type="button"></button>
      <button id="releaseAllForces" type="button"></button>
      <div id="status"></div>
      <span id="runtimeStatusText"></span>
      <span id="targetLabel"></span>
      <div id="sections"></div>
    </body></html>`,
    { runScripts: "outside-only", url: "https://live-values.test/" },
  );
  const posted: PostedMessage[] = [];
  dom.window.acquireVsCodeApi = () => ({
    postMessage: (message: PostedMessage) => posted.push(message),
  });
  // The harness holds the status line's expiry timers, so a test lets 5000 ms pass at once.
  const expiries = new Map<number, () => void>();
  let nextExpiry = 1_000_000;
  const setTimer = dom.window.setTimeout.bind(dom.window);
  const clearTimer = dom.window.clearTimeout.bind(dom.window);
  dom.window.setTimeout = (callback: () => void, delay?: number, ...rest: unknown[]) => {
    if (delay !== EXPIRY_MS) {
      return setTimer(callback, delay, ...rest);
    }
    nextExpiry += 1;
    expiries.set(nextExpiry, callback);
    return nextExpiry;
  };
  dom.window.clearTimeout = (id: number) => {
    if (!expiries.delete(id)) {
      clearTimer(id);
    }
  };
  const send = (message: Record<string, unknown>): void => {
    dom.window.dispatchEvent(new dom.window.MessageEvent("message", { data: message }));
  };
  const source = fs.readFileSync(
    path.join(extensionRoot(), "src", "ioPanel.webview.js"),
    "utf8",
  );
  dom.window.eval(source);
  // The panel's first status, as the extension sends it when the panel opens.
  dom.window.dispatchEvent(
    new dom.window.MessageEvent("message", { data: { type: "runtimeStatus", payload: SIMULATOR_STATUS } }),
  );

  return {
    window: dom.window,
    document: dom.window.document,
    posted,
    sendIoState(
      scan: number,
      input: { readonly value?: string; readonly forced?: boolean } = {},
    ): void {
      dom.window.dispatchEvent(
        new dom.window.MessageEvent("message", {
          data: {
            type: "ioState",
            payload: {
              scan,
              inputs: [
                {
                  name: "Boolean input",
                  address: "%IX0.0",
                  // The entry as the debug adapter writes it (specification 25, section 4.3).
                  valueType: "BOOL",
                  value: input.value ?? "FALSE",
                  writable: true,
                  forced: input.forced ?? false,
                },
              ],
              outputs: [],
              memory: [],
              ads: [],
            },
          },
        }),
      );
    },
    send,
    pendingExpiries(): number {
      return expiries.size;
    },
    expire(): void {
      const due = [...expiries.values()];
      expiries.clear();
      for (const callback of due) {
        callback();
      }
    },
    close(): void {
      dom.window.close();
    },
  };
}

function pointerEvent(window: any, type: "pointerdown" | "pointerup"): any {
  return new window.MouseEvent(type, {
    bubbles: true,
    button: 0,
  });
}

function actionButton(document: any, label: string): any {
  return [...document.querySelectorAll("button")].find(
    (button: any) => button.textContent === label,
  );
}

function nextTask(window: any): Promise<void> {
  return new Promise((resolve) => window.setTimeout(resolve, 0));
}

function renderedBrowserExecutable(
  env: NodeJS.ProcessEnv = process.env,
  exists: (candidate: string) => boolean = fs.existsSync,
): string {
  if (env.TRUST_UI_TEST_BROWSER) {
    return env.TRUST_UI_TEST_BROWSER;
  }
  return [
    "/usr/bin/chromium",
    "/usr/bin/chromium-browser",
    "/usr/bin/google-chrome",
    "/usr/bin/google-chrome-stable",
  ].find(exists) ?? "/usr/bin/chromium";
}

function renderedColumnPositions(): Record<string, number[]> {
  const browser = renderedBrowserExecutable();
  if (!fs.existsSync(browser)) {
    throw new Error(
      `Rendered layout test requires Chromium at ${browser}; set TRUST_UI_TEST_BROWSER to override.`,
    );
  }
  const hostSource = fs.readFileSync(
    path.join(extensionRoot(), "src", "ioPanel.ts"),
    "utf8",
  );
  const style = hostSource.match(/<style>([\s\S]*?)<\/style>/)?.[1];
  assert.ok(style, "expected the production Live Values stylesheet");
  const webviewSource = fs
    .readFileSync(path.join(extensionRoot(), "src", "ioPanel.webview.js"), "utf8")
    .replace(/<\/script/gi, "<\\/script");
  const temp = fs.mkdtempSync(path.join(os.tmpdir(), "trust-live-values-layout-"));
  const htmlPath = path.join(temp, "layout.html");
  const html = `<!doctype html><html><head><meta charset="utf-8"><style>${style}</style></head>
    <body>
      <input id="filter" />
      <button id="forcedFilter" type="button"></button>
      <button id="releaseAllForces" type="button"></button>
      <div id="status"></div>
      <div id="sections" class="tree"></div>
      <pre id="layout-result"></pre>
      <script>window.acquireVsCodeApi = () => ({ postMessage() {} });</script>
      <script>${webviewSource}</script>
      <script>
        window.dispatchEvent(new MessageEvent("message", { data: {
          type: "ioState",
          payload: {
            scan: 1,
            inputs: [{ name: "I/O Boolean", address: "%IX0.0", valueType: "BOOL", value: "FALSE", writable: true }],
            outputs: [], memory: [],
            ads: [{ name: "ADS Long Value", address: "MAIN.RemoteValue", valueType: "LREAL", value: "123456.789", writable: true }]
          }
        }}));
        const positions = {};
        for (const row of document.querySelectorAll(".row")) {
          const name = row.querySelector(".name > div")?.textContent || "";
          positions[name] = [...row.children].map((cell) => cell.getBoundingClientRect().left);
        }
        document.getElementById("layout-result").textContent = JSON.stringify(positions);
      </script>
    </body></html>`;
  fs.writeFileSync(htmlPath, html, "utf8");
  try {
    const { runBrowser } = require(path.join(extensionRoot(), "scripts/browser-command.js"));
    const output = runBrowser(
      "render", browser,
      [
        "--headless=new",
        "--no-sandbox",
        "--disable-gpu",
        `--user-data-dir=${path.join(temp, "chrome-profile")}`,
        "--run-all-compositor-stages-before-draw",
        "--dump-dom",
        htmlPath,
      ],
    );
    const rendered = new JSDOM(output, {
      runScripts: "outside-only",
      url: "https://layout-result.test/",
    });
    try {
      const result = rendered.window.document.getElementById("layout-result")?.textContent;
      assert.ok(result, "expected rendered Live Values geometry output");
      return JSON.parse(result) as Record<string, number[]>;
    } finally {
      rendered.window.close();
    }
  } finally {
    fs.rmSync(temp, { recursive: true, force: true });
  }
}

suite("Live Values rendered interactions", function () {
  this.timeout(10_000);

  test("rendered checks discover an installed Chrome-family browser", () => {
    const available = new Set(["/usr/bin/google-chrome"]);
    assert.strictEqual(
      renderedBrowserExecutable({}, (candidate) => available.has(candidate)),
      "/usr/bin/google-chrome",
    );
  });

  test("a live scan cannot interrupt a Boolean toggle gesture", async () => {
    const harness = createHarness();
    try {
      harness.sendIoState(1);
      const toggle = harness.document.querySelector("button.bool-toggle");
      assert.ok(toggle, "expected a rendered Boolean value control");

      toggle.dispatchEvent(pointerEvent(harness.window, "pointerdown"));
      harness.sendIoState(2);

      assert.strictEqual(
        toggle.isConnected,
        true,
        "the active Boolean control must remain mounted until the pointer gesture completes",
      );

      toggle.dispatchEvent(pointerEvent(harness.window, "pointerup"));
      toggle.click();
      await nextTask(harness.window);

      const rendered = harness.document.querySelector("button.bool-toggle");
      assert.strictEqual(rendered?.textContent, "TRUE");
      assert.strictEqual(rendered?.getAttribute("aria-pressed"), "true");
    } finally {
      harness.close();
    }
  });

  test("a numeric row's write box starts empty, with no cut-off hint", () => {
    // Acceptance finding F6: a REAL filled the 46-pixel write box with "1.5116941978474925", cut off, and in the re-run its
    // dim hint was cut off the same way. As the prepared value of a CODESYS or TwinCAT watch list, the box starts empty;
    // the current value stays in the Value column (specification 25, section 4.3).
    const harness = createHarness();
    try {
      harness.window.dispatchEvent(
        new harness.window.MessageEvent("message", {
          data: {
            type: "ioState",
            payload: {
              scan: 1,
              inputs: [{ name: "Tank.Level", address: "%IL0", valueType: "LREAL", value: "1.5116941978474925", writable: true }],
              outputs: [],
              memory: [],
              ads: [],
            },
          },
        }),
      );
      const box = harness.document.querySelector("input.value-input");
      assert.ok(box, "the row has a write box");
      assert.strictEqual(box.value, "");
      assert.strictEqual(box.placeholder, "", "the value is not repeated, cut off, inside the box");
      assert.strictEqual(harness.document.querySelector(".row .value")?.textContent, "1.5116941978474925");
      actionButton(harness.document, "Write").click();
      assert.ok(!harness.posted.some((message) => message.type === "writeInput"), "an empty box writes nothing");
      box.value = "2.5";
      box.dispatchEvent(new harness.window.Event("input", { bubbles: true }));
      actionButton(harness.document, "Write").click();
      // The webview's objects come from its own window, so they are compared by value.
      assert.deepStrictEqual(
        JSON.parse(JSON.stringify(harness.posted.filter((message) => message.type === "writeInput"))),
        [{ type: "writeInput", address: "%IL0", value: "2.5" }],
      );
    } finally {
      harness.close();
    }
  });

  test("a live scan preserves the highlight of the row under the pointer", () => {
    const harness = createHarness();
    try {
      harness.sendIoState(1);
      const row = harness.document.querySelector(".row");
      assert.ok(row, "expected a rendered Live Values row");

      row.dispatchEvent(
        new harness.window.MouseEvent("pointerover", {
          bubbles: true,
        }),
      );
      harness.sendIoState(2);

      const refreshedRow = harness.document.querySelector(".row");
      assert.strictEqual(
        refreshedRow?.classList.contains("pointer-hover"),
        true,
        "the refreshed row must stay highlighted while the pointer remains over it",
      );
    } finally {
      harness.close();
    }
  });

  test("a stale scan cannot visually roll back a pending Boolean force", async () => {
    const harness = createHarness();
    try {
      harness.sendIoState(1);
      const toggle = harness.document.querySelector("button.bool-toggle");
      assert.ok(toggle, "expected a rendered Boolean value control");
      toggle.click();
      assert.strictEqual(toggle.textContent, "TRUE");

      const force = actionButton(harness.document, "Force");
      assert.ok(force, "expected the Force action");
      force.dispatchEvent(pointerEvent(harness.window, "pointerdown"));
      harness.sendIoState(2, { value: "FALSE" });
      force.dispatchEvent(pointerEvent(harness.window, "pointerup"));
      force.click();
      assert.strictEqual(
        harness.document.querySelector(".row .value")?.textContent,
        "TRUE",
        "the chosen force value must appear immediately, without waiting for another scan",
      );
      await nextTask(harness.window);

      assert.strictEqual(
        harness.document.querySelector("button.bool-toggle")?.textContent,
        "TRUE",
        "the chosen force value must not revert while the runtime confirmation is pending",
      );
      assert.strictEqual(
        harness.document.querySelector(".row .value")?.textContent,
        "TRUE",
        "the displayed live value must not flash back to the stale scan value",
      );

      harness.sendIoState(3, { value: "FALSE", forced: true });
      assert.strictEqual(
        harness.document.querySelector(".row .value")?.textContent,
        "TRUE",
        "a forced flag with the previous value is still an unconfirmed scan",
      );

      harness.sendIoState(4, { value: "TRUE", forced: true });
      assert.strictEqual(
        harness.document.querySelector(".row .value")?.textContent,
        "TRUE",
      );
      assert.strictEqual(
        harness.document.querySelector(".state-badge")?.textContent,
        "FORCED",
      );

      // The runtime confirmed the force, so the pending value ends: a later scan shows the runtime's value again.
      harness.sendIoState(5, { value: "FALSE" });
      assert.strictEqual(
        harness.document.querySelector(".row .value")?.textContent,
        "FALSE",
        "a confirmed force must not hold its pending value over later scans",
      );
    } finally {
      harness.close();
    }
  });

  for (const [label, messageType] of [
    ["Write", "writeInput"],
    ["Force", "forceInput"],
  ] as const) {
    test(`a live scan cannot swallow a Boolean ${label} action`, async () => {
      const harness = createHarness();
      try {
        harness.sendIoState(1);
        const toggle = harness.document.querySelector("button.bool-toggle");
        assert.ok(toggle, "expected a rendered Boolean value control");
        toggle.click();
        assert.strictEqual(toggle.textContent, "TRUE");

        const action = actionButton(harness.document, label);
        assert.ok(action, `expected the ${label} action`);
        action.dispatchEvent(pointerEvent(harness.window, "pointerdown"));
        harness.sendIoState(2);

        assert.strictEqual(
          action.isConnected,
          true,
          `the active ${label} button must remain mounted until its click completes`,
        );

        action.dispatchEvent(pointerEvent(harness.window, "pointerup"));
        action.click();
        await nextTask(harness.window);

        const posted = harness.posted.find((message) => message.type === messageType);
        assert.strictEqual(posted?.type, messageType);
        assert.strictEqual(posted?.address, "%IX0.0");
        assert.strictEqual(posted?.value, "TRUE");
      } finally {
        harness.close();
      }
    });
  }

  test("I/O and ADS rows use the same rendered column positions", () => {
    const positions = renderedColumnPositions();
    const io = positions["I/O Boolean"];
    const ads = positions["ADS Long Value"];
    assert.ok(io, "expected the rendered I/O row");
    assert.ok(ads, "expected the rendered ADS row");
    assert.strictEqual(io.length, 5);
    assert.strictEqual(ads.length, 5);
    for (let column = 0; column < io.length; column += 1) {
      assert.ok(
        Math.abs(io[column] - ads[column]) < 0.5,
        `column ${column + 1} is misaligned: I/O=${io[column]}, ADS=${ads[column]}`,
      );
    }
  });
});

/** The status line's text. */
function statusText(harness: LiveValuesHarness): string {
  return harness.document.getElementById("status")?.textContent ?? "";
}

/** Makes a pending Boolean force: the toggle reads TRUE and Force is clicked, while the runtime still says FALSE. */
async function pendingForce(harness: LiveValuesHarness): Promise<void> {
  harness.sendIoState(1);
  const toggle = harness.document.querySelector("button.bool-toggle");
  assert.ok(toggle, "expected a rendered Boolean value control");
  toggle.click();
  const force = actionButton(harness.document, "Force");
  assert.ok(force, "expected the Force action");
  force.click();
  await nextTask(harness.window);
  assert.strictEqual(harness.document.querySelector(".row .value")?.textContent, "TRUE", "the force is pending");
}

// The host says how long a status message stays and what it reports; the panel acts on that, never on the message's
// words (specification 25, sections 4.2 and 4.4). Each test sends a message whose words the panel once matched, without
// the host's word for it, beside one with it.
suite("Live Values status messages", function () {
  test("a message goes when live values arrive only when the host says so", () => {
    // Catches U3c: the panel cleared the language server's guidance by its words, and kept a lost connection's message
    // after values came back.
    const harness = createHarness();
    try {
      harness.send({
        type: "status",
        payload: "Live Values lost connection to the runtime. Restart or reconnect the runtime, then retry.",
        lifetime: "untilValues",
      });
      harness.sendIoState(1);
      assert.strictEqual(statusText(harness), "");

      harness.send({ type: "status", payload: "Start the runtime to see live values." });
      harness.sendIoState(2);
      assert.strictEqual(statusText(harness), "Start the runtime to see live values.");
    } finally {
      harness.close();
    }
  });

  test("success feedback expires after 5000 ms only when the host says so, and never clears a newer message", () => {
    // Catches U3c: the panel expired a message by its words, and an expiry that clears the message after it.
    const harness = createHarness();
    try {
      harness.expire();
      harness.send({ type: "status", payload: "I/O write queued for %IX0.0.", lifetime: "expires" });
      assert.strictEqual(harness.pendingExpiries(), 1);
      harness.expire();
      assert.strictEqual(statusText(harness), "");

      harness.send({ type: "status", payload: "I/O write queued for %IX0.0.", lifetime: "expires" });
      harness.send({ type: "status", payload: "I/O write failed: the runtime refused the value." });
      harness.expire();
      assert.strictEqual(statusText(harness), "I/O write failed: the runtime refused the value.");

      harness.send({ type: "status", payload: "I/O write queued for %IX0.0." });
      assert.strictEqual(harness.pendingExpiries(), 0, "a message without the host's lifetime stays");
    } finally {
      harness.close();
    }
  });

  test("a lost session keeps the target the header showed, without a connection", () => {
    // Catches U3c: the panel made up the header from the guidance's words, an online runtime with no endpoint.
    const harness = createHarness();
    try {
      harness.send({
        type: "runtimeStatus",
        payload: {
          running: true,
          runtimeMode: "online",
          runtimeState: "connected",
          endpoint: "tcp://10.0.0.7:9100",
          targetLabel: "Line PLC",
          access: { role: "admin", allowWrite: true, allowForce: true, allowRelease: true, reason: null },
        },
      });
      assert.strictEqual(harness.document.getElementById("runtimeStatusText")?.textContent, "Connected");
      harness.send({
        type: "status",
        payload: "Connect to the selected runtime to see live values.",
        lifetime: "untilValues",
        sessionLost: true,
      });
      const pill = harness.document.getElementById("runtimeStatusText");
      const target = harness.document.getElementById("targetLabel");
      assert.strictEqual(pill?.textContent, "Not connected");
      assert.strictEqual(target?.textContent, "Line PLC");
      assert.strictEqual(target?.title, "tcp://10.0.0.7:9100");
    } finally {
      harness.close();
    }
  });

  test("guidance sent with a runtime status leaves the header to that status", () => {
    // Catches U3c's finding: after a terminated session's status, the guidance's words overwrote it and lost its
    // endpoint.
    const harness = createHarness();
    try {
      harness.send({
        type: "runtimeStatus",
        payload: {
          running: false,
          runtimeMode: "online",
          runtimeState: "stopped",
          endpoint: "tcp://10.0.0.7:9100",
          targetLabel: "Line PLC",
        },
      });
      harness.send({
        type: "status",
        payload: "Connect to the selected runtime to see live values.",
        lifetime: "untilValues",
        sessionLost: false,
      });
      const target = harness.document.getElementById("targetLabel");
      assert.strictEqual(target?.textContent, "Line PLC");
      assert.strictEqual(target?.title, "tcp://10.0.0.7:9100");
    } finally {
      harness.close();
    }
  });

  test("a failed Force clears the pending Boolean force only when the host says it failed", async () => {
    // Catches U3c: the panel read a failed force from the words "I/O force failed:".
    const failed = createHarness();
    try {
      await pendingForce(failed);
      failed.send({ type: "status", payload: "I/O force failed: the runtime refused the force.", forceFailed: true });
      assert.strictEqual(failed.document.querySelector(".row .value")?.textContent, "FALSE");
    } finally {
      failed.close();
    }
    const unflagged = createHarness();
    try {
      await pendingForce(unflagged);
      unflagged.send({ type: "status", payload: "I/O force failed: the runtime refused the force." });
      assert.strictEqual(unflagged.document.querySelector(".row .value")?.textContent, "TRUE");
    } finally {
      unflagged.close();
    }
  });

  test("a message's treatment is the host's word for it, never its words", () => {
    // Catches U3d: the panel coloured a message by matching words such as "failed", "error" or "queued".
    const harness = createHarness();
    try {
      const status = (): any => harness.document.getElementById("status");
      harness.send({ type: "status", payload: "Settings could not be saved.", treatment: "error" });
      assert.ok(status().classList.contains("status-error"), "an error the host names is an error");
      harness.send({ type: "status", payload: "I/O write failed: the runtime refused the value." });
      assert.ok(!status().classList.contains("status-error"), "the words of a failure make no error");
      harness.send({ type: "status", payload: "Released 2 forces.", treatment: "ok" });
      assert.ok(status().classList.contains("status-ok"), "a success the host names is a success");
      harness.send({ type: "status", payload: "I/O write queued for %IX0.0." });
      assert.ok(!status().classList.contains("status-ok"), "the words of a success make no success");
      harness.send({ type: "status", payload: "Force armed state unknown.", treatment: "warning" });
      assert.ok(status().classList.contains("status-warn"), "a warning the host names is a warning");
    } finally {
      harness.close();
    }
  });

  test("the active-force summary replaces any message but an error or an access reason, and goes with the last force", () => {
    // Catches U3d: the panel kept a message over the summary by its words, and cleared a message by the words
    // "force active".
    const harness = createHarness();
    try {
      harness.send({ type: "status", payload: "Settings could not be saved.", treatment: "error" });
      harness.sendIoState(1, { forced: true });
      assert.strictEqual(statusText(harness), "Settings could not be saved.", "an error stays over the summary");

      harness.send({ type: "status", payload: "Settings saved." });
      harness.sendIoState(2, { forced: true });
      assert.strictEqual(statusText(harness), "I/O force active at %IX0.0.");
      assert.ok(harness.document.getElementById("status")?.classList.contains("status-warn"));
      harness.sendIoState(3);
      assert.strictEqual(statusText(harness), "", "the summary goes when no force remains");

      harness.send({ type: "status", payload: "I/O force active at %IX0.0.", treatment: "warning", forceActive: true });
      harness.sendIoState(4);
      assert.strictEqual(statusText(harness), "", "the host's force feedback goes with the last force too");

      harness.send({ type: "status", payload: "The force active lamp is a plain message." });
      harness.sendIoState(5);
      assert.strictEqual(statusText(harness), "The force active lamp is a plain message.");

      harness.send({
        type: "runtimeStatus",
        payload: { ...SIMULATOR_STATUS, access: { ...SIMULATOR_STATUS.access, reason: "Write access needs another role." } },
      });
      assert.strictEqual(statusText(harness), "Write access needs another role.");
      harness.sendIoState(6, { forced: true });
      assert.strictEqual(statusText(harness), "Write access needs another role.", "an access reason stays over the summary");
      assert.ok(harness.document.getElementById("status")?.classList.contains("status-warn"));
    } finally {
      harness.close();
    }
  });

  test("a release on an armed target says the force remains armed, and that message stays", () => {
    // Catches U3c: the panel read a release from the words "released" or "cleared", and let the arming feedback expire.
    const harness = createHarness();
    try {
      harness.send({ type: "runtimeStatus", payload: { ...SIMULATOR_STATUS, forceRequiresArm: true } });
      harness.sendIoState(1);
      const arm = actionButton(harness.document, "Arm force");
      assert.ok(arm, "expected the Arm force action on a target that requires arming");
      arm.click();
      harness.expire();

      harness.send({ type: "status", payload: "I/O force released at %IX0.0.", lifetime: "expires", released: true });
      assert.strictEqual(statusText(harness), "I/O force released at %IX0.0. Force remains armed for this target.");
      assert.strictEqual(harness.pendingExpiries(), 0, "the arming feedback stays explicit after a release");

      harness.send({ type: "status", payload: "Watch list cleared and released.", lifetime: "expires" });
      assert.strictEqual(statusText(harness), "Watch list cleared and released.");
    } finally {
      harness.close();
    }
  });
});
