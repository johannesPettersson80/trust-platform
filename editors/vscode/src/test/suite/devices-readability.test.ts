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

// The readability rule of specification 25 §5.11 at a 1280 × 800 window: a long name is not cut
// silently — it wraps or ends with an ellipsis and its full text is the element's `title`. Expected
// values are the literals written here, never a value read back from the component.

/** The shell component under test, bundled the way the extension ships it. */
type WebviewComponent = React.ComponentType<Record<string, unknown>>;

/** Bundles the shell entry point and returns `App`, as the extension ships it. */
function loadApp(): WebviewComponent {
  const extensionRoot = path.resolve(__dirname, "..", "..", "..");
  const temp = fs.mkdtempSync(
    path.join(extensionRoot, "node_modules", ".trust-devices-readability-test-"),
  );
  const outfile = path.join(temp, "devices-app.cjs");
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
    return require(outfile).App as WebviewComponent;
  } finally {
    fs.rmSync(temp, { recursive: true, force: true });
  }
}

// JSDOM's elements stay untyped, as in devices-inspector-render.test.ts: the extension's tsconfig has
// no DOM lib, so a `HTMLElement` here would not name a type.
interface Rendered {
  container: any;
  window: any;
  unmount(): Promise<void>;
}

async function render(
  component: WebviewComponent,
  props: Record<string, unknown>,
): Promise<Rendered> {
  const dom = new JSDOM('<!doctype html><body><div id="root"></div></body>', {
    url: "https://devices-readability.test/",
  });
  const globals = globalThis as any;
  const previousWindow = globals.window;
  const previousDocument = globals.document;
  const previousActEnvironment = globals.IS_REACT_ACT_ENVIRONMENT;
  globals.window = dom.window;
  globals.document = dom.window.document;
  globals.IS_REACT_ACT_ENVIRONMENT = true;
  const container = dom.window.document.getElementById("root");
  const root: Root = createRoot(container);
  await act(async () => {
    root.render(React.createElement(component, props));
  });
  return {
    container,
    window: dom.window,
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

// An action with the fields the panel draws (mod.rs `Action`).
function action(overrides: Record<string, unknown>): Record<string, unknown> {
  return { id: "connect", label: "Connect", enabled: true, emphasis: "primary", ...overrides };
}

/** An inspector with only the fields the panel draws when nothing is opened in the test. */
function inspector(title: string): Record<string, unknown> {
  return {
    breadcrumb: "Devices & Connections / Item",
    kind: "Runtime",
    title,
    primary: [],
    secondary: [],
    sections: [],
  };
}

/** The toolbar's three actions and the switch's two words, as Rust declares them. */
function toolbar(): Record<string, unknown> {
  return {
    add: action({ id: "add", label: "+ Add" }),
    discover: action({ id: "discover", label: "Discover", emphasis: "secondary" }),
    add_host_slot: action({ id: "add_host", label: "Add host", emphasis: "secondary" }),
    view_switch: { topology: "Topology", list: "List" },
  };
}

/** A view whose computer, runtime, connection and device names are all too long for one line. */
function longNamesView(names: {
  computer: string;
  runtime: string;
  connection: string;
  device: string;
}): Record<string, unknown> {
  return {
    toolbar: toolbar(),
    computers: [
      {
        id: "host-1",
        heading: names.computer,
        platform: "Raspberry Pi 5 · Linux aarch64",
        reachability: { word: "Reachable", tone: "ok" },
        inspector: inspector(names.computer),
        runtimes: [
          {
            id: "runtime-1",
            name: names.runtime,
            state: { word: "Running", tone: "ok" },
            inspector: inspector(names.runtime),
            connections: [
              {
                id: "conn-1",
                name: names.connection,
                dot: "ok",
                live: false,
                role_word: "CLIENT",
                marks: [],
                look: "normal",
                inspector: inspector(names.connection),
              },
            ],
          },
        ],
      },
    ],
    devices: [
      {
        id: "device-1",
        kind: "external",
        name: names.device,
        presents: "MQTT broker",
        inspector: inspector(names.device),
      },
    ],
  };
}

suite("Devices & Connections panel readability", () => {
  test("long_names_wrap_or_ellipsize_with_the_full_text_on_hover", async () => {
    const App = loadApp();
    const names = {
      computer: "Computer 192.168.100.200 · Raspberry Pi 5 Model B Rev 1.0 · Linux aarch64",
      runtime: "Bottling line packaging controller on the north conveyor with a long name",
      connection: "Modbus TCP to the packaging line PLC at 192.168.100.200 port 502",
      device: "MQTT broker of the packaging line telemetry at mqtt.example.internal",
    };
    const rendered = await render(App, { view: longNamesView(names), post: () => undefined });
    try {
      // The rule is stated for a 1280 × 800 window (specification 25 §5.11). JSDOM runs no layout, so
      // the size changes nothing it draws; the rule the panel relies on at that size is asserted here.
      rendered.window.innerWidth = 1280;
      rendered.window.innerHeight = 800;
      const cases = [
        { role: "computer-heading", name: names.computer },
        { role: "runtime-name", name: names.runtime },
        { role: "connection-name", name: names.connection },
        { role: "device-name", name: names.device },
      ];
      for (const item of cases) {
        const element = rendered.container.querySelector(`[data-role="${item.role}"]`);
        assert.ok(element, `${item.role}: the name is drawn`);
        assert.strictEqual(element.textContent, item.name, `${item.role}: the full text is drawn`);
        // A name element without a title shows no full text on hover: the bug this catches.
        assert.strictEqual(
          element.getAttribute("title"),
          item.name,
          `${item.role}: the full text is the tooltip`,
        );
        const style = element.style;
        // Cut silently — overflow hidden with neither an ellipsis nor wrapping — is the bug this
        // catches.
        const ellipsized = style.textOverflow === "ellipsis" && style.overflow === "hidden";
        const wrapped = style.whiteSpace === "normal" || style.overflowWrap === "anywhere";
        assert.ok(
          ellipsized || wrapped,
          `${item.role}: the name wraps or ends with an ellipsis, never cut silently`,
        );
      }
    } finally {
      await rendered.unmount();
    }
  });
});
