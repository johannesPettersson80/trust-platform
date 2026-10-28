import * as assert from "assert";
import * as fs from "fs";
import * as path from "path";
import * as React from "react";
import { act } from "react";
import { createRoot, type Root } from "react-dom/client";

const { JSDOM } = require("jsdom") as {
  JSDOM: new (html: string, options: { url: string }) => any;
};
const { buildSync } = require("esbuild") as {
  buildSync(options: Record<string, unknown>): void;
};

// The toolbar part of a small view model, written with literal values as Rust sends them. The three
// keys are Rust's `Toolbar` fields, in their declared order (crates/trust-devices/src/view/types/
// graph.rs:57-64).
function toolbar(): Record<string, unknown> {
  return {
    add: {
      id: "add",
      label: "+ Add",
      enabled: true,
      emphasis: "primary",
      op: { kind: "ui", ui: { kind: "open", pane: { kind: "add" } } },
    },
    discover: {
      id: "discover",
      label: "Discover",
      enabled: false,
      emphasis: "secondary",
      reason: "Nothing to discover yet.",
      op: { kind: "ui", ui: { kind: "open", pane: { kind: "discover" } } },
    },
    add_host_slot: {
      id: "add_host",
      label: "Add host",
      enabled: true,
      emphasis: "secondary",
      op: { kind: "ui", ui: { kind: "open", pane: { kind: "add" } } },
    },
  };
}

type DevicesApp = React.ComponentType<{
  view: unknown;
  post: (message: unknown) => void;
}>;

function loadDevicesApp(): DevicesApp {
  const extensionRoot = path.resolve(__dirname, "..", "..", "..");
  const temp = fs.mkdtempSync(
    path.join(extensionRoot, "node_modules", ".trust-devices-panel-test-"),
  );
  const outfile = path.join(temp, "devices-panel.cjs");
  try {
    buildSync({
      entryPoints: [path.join(extensionRoot, "src", "devicesPanel", "webview", "App.tsx")],
      outfile,
      bundle: true,
      platform: "node",
      format: "cjs",
      external: ["react", "react-dom", "vscode"],
      logLevel: "silent",
    });
    return require(outfile).App as DevicesApp;
  } finally {
    fs.rmSync(temp, { recursive: true, force: true });
  }
}

// JSDOM's elements stay untyped, as in ads-discovery-results-rendered.test.ts: the extension's
// tsconfig has no DOM lib, so a `HTMLElement` here would not name a type.
interface RenderedDevicesApp {
  container: any;
  unmount(): Promise<void>;
}

async function renderDevicesApp(
  view: unknown,
  post: (message: unknown) => void,
): Promise<RenderedDevicesApp> {
  const dom = new JSDOM("<!doctype html><body><div id=\"root\"></div></body>", {
    url: "https://devices-panel.test/",
  });
  const globals = globalThis as any;
  const previousWindow = globals.window;
  const previousDocument = globals.document;
  const previousActEnvironment = globals.IS_REACT_ACT_ENVIRONMENT;
  globals.window = dom.window;
  globals.document = dom.window.document;
  globals.IS_REACT_ACT_ENVIRONMENT = true;
  const App = loadDevicesApp();
  const container = dom.window.document.getElementById("root");
  const root: Root = createRoot(container);
  await act(async () => {
    root.render(React.createElement(App, { view, post }));
  });
  return {
    container,
    async unmount() {
      await act(async () => {
        root.unmount();
      });
      globals.window = previousWindow;
      globals.document = previousDocument;
      globals.IS_REACT_ACT_ENVIRONMENT = previousActEnvironment;
      dom.window.close();
    },
  };
}

suite("Devices & Connections panel shell", () => {
  test("toolbar_items_render_in_the_given_order_with_their_labels", async () => {
    const rendered = await renderDevicesApp({ toolbar: toolbar(), computers: [] }, () => undefined);
    try {
      const labels = [...rendered.container.querySelectorAll("button")].map(
        (button) => button.textContent,
      );
      assert.deepStrictEqual(labels, ["+ Add", "Discover", "Add host"]);
    } finally {
      await rendered.unmount();
    }
  });

  test("a_disabled_toolbar_item_cannot_be_activated", async () => {
    const posted: unknown[] = [];
    const rendered = await renderDevicesApp({ toolbar: toolbar(), computers: [] }, (message) => {
      posted.push(message);
    });
    try {
      const enabled = rendered.container.querySelector('[data-toolbar-item="add"]');
      const disabled = rendered.container.querySelector('[data-toolbar-item="discover"]');
      assert.strictEqual(disabled.disabled, true);
      await act(async () => {
        enabled.click();
      });
      await act(async () => {
        disabled.click();
      });
      assert.deepStrictEqual(posted, [
        {
          type: "action",
          action: {
            id: "add",
            label: "+ Add",
            enabled: true,
            emphasis: "primary",
            op: { kind: "ui", ui: { kind: "open", pane: { kind: "add" } } },
          },
        },
      ]);
    } finally {
      await rendered.unmount();
    }
  });

  test("a_computer_frame_holds_every_runtime_it_is_given", async () => {
    const rendered = await renderDevicesApp(
      {
        toolbar: toolbar(),
        computers: [
          {
            id: "host-line",
            heading: "This computer",
            platform: "Raspberry Pi 5 · Linux aarch64",
            reachability: { word: "Reachable", tone: "ok" },
            runtimes: [
              { id: "runtime-line", name: "Line controller", state: { word: "Running", tone: "ok" } },
              { id: "runtime-packaging", name: "Packaging", state: { word: "Running", tone: "ok" } },
            ],
          },
          {
            id: "host-chip",
            heading: "STM32U5A9 192.168.10.31",
            platform: "STM32U5A9 chip",
            reachability: { word: "Reachable", tone: "ok" },
            runtimes: [
              { id: "runtime-conveyor", name: "Conveyor node", state: { word: "Running", tone: "ok" } },
            ],
          },
        ],
      },
      () => undefined,
    );
    try {
      const frames = [...rendered.container.querySelectorAll('[data-item-id^="host-"]')];
      assert.strictEqual(frames.length, 2, "one frame per computer");
      const lineFrame = rendered.container.querySelector('[data-item-id="host-line"]');
      assert.ok(lineFrame);
      const lineRuntimeIds = [...lineFrame.querySelectorAll("[data-item-id]")].map((card) =>
        card.getAttribute("data-item-id"),
      );
      assert.deepStrictEqual(lineRuntimeIds, ["runtime-line", "runtime-packaging"]);
    } finally {
      await rendered.unmount();
    }
  });

  test("a_runtime_card_shows_name_state_and_platform_exactly_as_given", async () => {
    const rendered = await renderDevicesApp(
      {
        toolbar: toolbar(),
        computers: [
          {
            id: "host-line",
            heading: "This computer",
            platform: "Raspberry Pi 5 · Linux aarch64",
            reachability: { word: "Reachable", tone: "ok" },
            runtimes: [
              { id: "runtime-line", name: "Line controller", state: { word: "Running", tone: "ok" } },
            ],
          },
        ],
      },
      () => undefined,
    );
    try {
      const card = rendered.container.querySelector('[data-item-id="runtime-line"]');
      assert.ok(card);
      assert.strictEqual(
        card.querySelector('[data-role="runtime-name"]')?.textContent,
        "Line controller",
      );
      assert.strictEqual(
        card.querySelector('[data-role="runtime-platform"]')?.textContent,
        "Raspberry Pi 5 · Linux aarch64",
      );
      assert.strictEqual(card.querySelector('[data-role="pill"]')?.textContent, "Running");
    } finally {
      await rendered.unmount();
    }
  });

  test("a_long_runtime_name_keeps_its_full_text_on_hover", async () => {
    const longName =
      "Bottling line packaging controller for the north conveyor with a long descriptive name";
    const rendered = await renderDevicesApp(
      {
        toolbar: toolbar(),
        computers: [
          {
            id: "host-line",
            heading: "This computer",
            platform: "Raspberry Pi 5 · Linux aarch64",
            reachability: { word: "Reachable", tone: "ok" },
            runtimes: [{ id: "runtime-long", name: longName, state: { word: "Running", tone: "ok" } }],
          },
        ],
      },
      () => undefined,
    );
    try {
      const card = rendered.container.querySelector('[data-item-id="runtime-long"]');
      assert.ok(card);
      const name = card.querySelector('[data-role="runtime-name"]');
      assert.ok(name);
      assert.strictEqual(name.getAttribute("title"), longName);
      assert.strictEqual(name.textContent, longName);
      assert.strictEqual(name.style.textOverflow, "ellipsis");
      assert.strictEqual(name.style.whiteSpace, "nowrap");
    } finally {
      await rendered.unmount();
    }
  });
});
