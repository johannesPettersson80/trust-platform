import * as assert from "assert";
import { execFileSync } from "child_process";
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
  close(): void;
};

function extensionRoot(): string {
  return path.resolve(__dirname, "..", "..", "..");
}

function createHarness(): LiveValuesHarness {
  const dom = new JSDOM(
    `<!doctype html><html><body>
      <input id="filter" />
      <button id="forcedFilter" type="button"></button>
      <button id="releaseAllForces" type="button"></button>
      <div id="status"></div>
      <div id="sections"></div>
    </body></html>`,
    { runScripts: "outside-only", url: "https://live-values.test/" },
  );
  const posted: PostedMessage[] = [];
  dom.window.acquireVsCodeApi = () => ({
    postMessage: (message: PostedMessage) => posted.push(message),
  });
  const source = fs.readFileSync(
    path.join(extensionRoot(), "src", "ioPanel.webview.js"),
    "utf8",
  );
  dom.window.eval(source);

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
                  value: input.value ?? "BOOL(FALSE)",
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
            inputs: [{ name: "I/O Boolean", address: "%IX0.0", value: "BOOL(FALSE)", writable: true }],
            outputs: [], memory: [],
            ads: [{ name: "ADS Long Value", address: "MAIN.RemoteValue", value: "LREAL(123456.789)", writable: true }]
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
    const output = execFileSync(
      browser,
      [
        "--headless=new",
        "--no-sandbox",
        "--disable-gpu",
        "--run-all-compositor-stages-before-draw",
        "--dump-dom",
        htmlPath,
      ],
      { encoding: "utf8", stdio: ["ignore", "pipe", "pipe"] },
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
      harness.sendIoState(2, { value: "BOOL(FALSE)" });
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

      harness.sendIoState(3, { value: "BOOL(FALSE)", forced: true });
      assert.strictEqual(
        harness.document.querySelector(".row .value")?.textContent,
        "TRUE",
        "a forced flag with the previous value is still an unconfirmed scan",
      );

      harness.sendIoState(4, { value: "BOOL(TRUE)", forced: true });
      assert.strictEqual(
        harness.document.querySelector(".row .value")?.textContent,
        "TRUE",
      );
      assert.strictEqual(
        harness.document.querySelector(".state-badge")?.textContent,
        "FORCED",
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
