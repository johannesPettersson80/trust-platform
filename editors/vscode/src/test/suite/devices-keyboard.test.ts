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

// The keyboard and the drag are input and view geometry: this test draws the shell from a small view
// model written here, presses keys and the mouse on it and checks what it posts. Expected values are
// the literals of the specification and the view model (specification 25 §5.11 and §5.13, design
// §4.5 D2), never a value read back from the component.

/** The shell component under test, bundled the way the extension ships it. */
type WebviewComponent = React.ComponentType<Record<string, unknown>>;

/** Bundles the shell entry point and returns `App`, as the extension ships it. */
function loadApp(): WebviewComponent {
  const extensionRoot = path.resolve(__dirname, "..", "..", "..");
  const temp = fs.mkdtempSync(path.join(extensionRoot, "node_modules", ".trust-devices-keyboard-test-"));
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
    url: "https://devices-keyboard.test/",
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

/** An inspector whose title names the item it belongs to, so the test can tell them apart. */
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

/** One computer, one runtime, one connection and one device, each with its own inspector. */
function keyboardView(): Record<string, unknown> {
  return {
    toolbar: toolbar(),
    computers: [
      {
        id: "host-1",
        heading: "This computer",
        reachability: { word: "Reachable", tone: "ok" },
        inspector: inspector("This computer"),
        runtimes: [
          {
            id: "runtime-1",
            name: "Line runtime",
            state: { word: "Running", tone: "ok" },
            inspector: inspector("Line runtime"),
            connections: [
              {
                id: "conn-1",
                name: "Modbus meter",
                dot: "ok",
                live: true,
                role_word: "CLIENT",
                marks: [],
                look: "normal",
                inspector: inspector("Modbus meter"),
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
        name: "MQTT broker",
        presents: "MQTT broker",
        inspector: inspector("MQTT broker"),
      },
    ],
  };
}

suite("Devices & Connections panel keyboard and drag", () => {
  test("tab_reaches_every_item_in_drawing_order", async () => {
    const App = loadApp();
    const rendered = await render(App, { view: keyboardView(), post: () => undefined });
    try {
      // The canvas draws the frame holding its runtimes, a connection pill inside its runtime, and
      // then the devices, so Tab reaches them in that order (specification 25 §5.11). An item that
      // is not focusable, or an order that is not the drawing order, is the bug this catches.
      const focusable = [...rendered.container.querySelectorAll('[data-item-id][tabindex="0"]')].map(
        (item) => item.getAttribute("data-item-id"),
      );
      assert.deepStrictEqual(
        focusable,
        ["host-1", "runtime-1", "conn-1", "device-1"],
        "every item is reachable with Tab, in the order the canvas draws them",
      );
    } finally {
      await rendered.unmount();
    }
  });

  test("enter_opens_the_items_inspector", async () => {
    const App = loadApp();
    const rendered = await render(App, { view: keyboardView(), post: () => undefined });
    try {
      // Enter on a focused item opens the inspector of that item, not of another (specification 25
      // §5.11). Enter that opened nothing, or the wrong item's inspector, is the bug this catches.
      const cases = [
        { id: "host-1", title: "This computer" },
        { id: "runtime-1", title: "Line runtime" },
        { id: "conn-1", title: "Modbus meter" },
        { id: "device-1", title: "MQTT broker" },
      ];
      for (const item of cases) {
        const element = rendered.container.querySelector(`[data-item-id="${item.id}"]`);
        assert.ok(element, `${item.id}: the item is drawn`);
        await act(async () => {
          element.focus();
          element.dispatchEvent(
            new rendered.window.KeyboardEvent("keydown", { key: "Enter", bubbles: true }),
          );
        });
        const title = rendered.container.querySelector('[data-role="inspector-title"]');
        assert.ok(title, `${item.id}: Enter opens its inspector`);
        assert.strictEqual(
          title.textContent,
          item.title,
          `${item.id}: the inspector is the item's own`,
        );
      }
    } finally {
      await rendered.unmount();
    }
  });

  test("escape_closes_the_open_pane", async () => {
    const App = loadApp();
    const rendered = await render(App, { view: keyboardView(), post: () => undefined });
    try {
      const card = rendered.container.querySelector('[data-item-id="runtime-1"]');
      await act(async () => {
        card.click();
      });
      assert.ok(
        rendered.container.querySelector('[data-role="inspector"]'),
        "choosing an item opens its inspector",
      );
      // Escape closes the open right-hand pane (specification 25 §5.11). A pane that stayed open is
      // the bug this catches.
      await act(async () => {
        rendered.window.dispatchEvent(
          new rendered.window.KeyboardEvent("keydown", { key: "Escape", bubbles: true }),
        );
      });
      assert.strictEqual(
        rendered.container.querySelector('[data-role="inspector"]'),
        null,
        "Escape closes the open pane",
      );
    } finally {
      await rendered.unmount();
    }
  });

  test("a_click_on_the_empty_canvas_closes_the_inspector", async () => {
    const App = loadApp();
    const rendered = await render(App, { view: keyboardView(), post: () => undefined });
    try {
      const card = rendered.container.querySelector('[data-item-id="device-1"]');
      await act(async () => {
        card.click();
      });
      assert.ok(
        rendered.container.querySelector('[data-role="inspector"]'),
        "choosing an item opens its inspector",
      );
      // A click on the empty canvas closes the inspector (specification 25 §5.11). A canvas that
      // left the inspector open, or opened something else, is the bug this catches.
      await act(async () => {
        rendered.container.querySelector('[data-role="graph"]').click();
      });
      assert.strictEqual(
        rendered.container.querySelector('[data-role="inspector"]'),
        null,
        "a click on the empty canvas closes the inspector",
      );
    } finally {
      await rendered.unmount();
    }
  });

  test("a_dragged_card_posts_its_position_and_applies_saved_positions", async () => {
    const App = loadApp();
    const posted: unknown[] = [];
    const rendered = await render(App, {
      view: keyboardView(),
      post: (message: unknown) => {
        posted.push(message);
      },
      positions: { "device-1": { x: 100, y: 200 } },
    });
    try {
      const positioned = () => rendered.container.querySelector('[data-item-id="device-1"]');
      // The host's saved positions are applied when the panel opens again (specification 25 §5.11).
      // A device drawn at its laid-out position despite a saved one is the bug this catches.
      assert.strictEqual(positioned().style.left, "100px", "the saved position is applied");
      assert.strictEqual(positioned().style.top, "200px", "the saved position is applied");

      // A card is dragged by the pointer's movement: 100 + 30 and 200 + 40 (specification 25 §5.13).
      // A dragged position that ignored the movement, or the wrong base, is the bug this catches.
      await act(async () => {
        positioned().dispatchEvent(
          new rendered.window.MouseEvent("mousedown", {
            bubbles: true,
            clientX: 100,
            clientY: 200,
          }),
        );
      });
      await act(async () => {
        rendered.window.dispatchEvent(
          new rendered.window.MouseEvent("mousemove", {
            bubbles: true,
            clientX: 130,
            clientY: 240,
          }),
        );
      });
      assert.strictEqual(positioned().style.left, "130px", "the dragged position replaces the layout");
      assert.strictEqual(positioned().style.top, "240px", "the dragged position replaces the layout");

      // When the drag ends, the webview posts the dragged item's position by its identity
      // (specification 25 §5.11). A missing message, or a wrong key or value, is the bug this catches.
      await act(async () => {
        rendered.window.dispatchEvent(
          new rendered.window.MouseEvent("mouseup", { bubbles: true, clientX: 130, clientY: 240 }),
        );
      });
      assert.deepStrictEqual(
        posted,
        [{ type: "positions", positions: { "device-1": { x: 130, y: 240 } } }],
        "a drag end posts the dragged item's position by its identity",
      );
    } finally {
      await rendered.unmount();
    }
  });
});
