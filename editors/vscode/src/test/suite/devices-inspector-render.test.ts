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

// The inspector and the shell are drawn from the Rust view model; the expected values below are the
// literals of that contract (crates/trust-devices/src/view/types/, design §2.1), never a value read
// back from the component.

/** A webview component under test, bundled the way the extension ships it. */
type WebviewComponent = React.ComponentType<Record<string, unknown>>;

/** Bundles one webview entry point and returns its named export, as the extension ships it. */
function loadComponent(component: "Inspector" | "App" | "Toolbar"): WebviewComponent {
  const extensionRoot = path.resolve(__dirname, "..", "..", "..");
  const temp = fs.mkdtempSync(
    path.join(extensionRoot, "node_modules", `.trust-devices-${component}-test-`),
  );
  const outfile = path.join(temp, `devices-${component}.cjs`);
  try {
    buildSync({
      entryPoints: [
        path.join(extensionRoot, "src", "devicesPanel", "webview", `${component}.tsx`),
      ],
      outfile,
      bundle: true,
      platform: "node",
      format: "cjs",
      external: ["react", "react-dom", "vscode"],
      logLevel: "silent",
    });
    return require(outfile)[component] as WebviewComponent;
  } finally {
    fs.rmSync(temp, { recursive: true, force: true });
  }
}

// JSDOM's elements stay untyped, as in ads-discovery-results-rendered.test.ts: the extension's
// tsconfig has no DOM lib, so a `HTMLElement` here would not name a type.
interface Rendered {
  container: any;
  window: any;
  copied: string[];
  unmount(): Promise<void>;
}

async function render(
  component: WebviewComponent,
  props: Record<string, unknown>,
): Promise<Rendered> {
  const dom = new JSDOM('<!doctype html><body><div id="root"></div></body>', {
    url: "https://devices-inspector.test/",
  });
  const copied: string[] = [];
  Object.defineProperty(dom.window.navigator, "clipboard", {
    value: {
      writeText: (text: string) => {
        copied.push(text);
        return Promise.resolve();
      },
    },
    configurable: true,
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
    copied,
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

// The golden view of the project whose runtime is stopped and has browsable protocols, as Rust serialized it.
function stoppedBrowsesView(): any {
  const extensionRoot = path.resolve(__dirname, "..", "..", "..");
  const file = path.resolve(
    extensionRoot,
    "..",
    "..",
    "crates",
    "trust-devices",
    "tests",
    "fixtures",
    "devices_view",
    "stopped_browses.json",
  );
  return JSON.parse(fs.readFileSync(file, "utf8"));
}

/** The connection named `name` of a golden view. */
function connectionNamed(view: any, name: string): any {
  for (const computer of view.computers) {
    for (const runtime of computer.runtimes) {
      for (const connection of runtime.connections) {
        if (connection.name === name) {
          return connection;
        }
      }
    }
  }
  throw new Error(`a connection named ${name}`);
}

// An action with the fields the inspector draws (mod.rs `Action`). Each test overrides only what it
// is about, so its expected values stay visible.
function action(overrides: Record<string, unknown>): Record<string, unknown> {
  return { id: "connect", label: "Connect", enabled: true, emphasis: "primary", ...overrides };
}

/** An inspector with the fields the panel draws (graph.rs `Inspector`). */
function inspector(overrides: Record<string, unknown>): Record<string, unknown> {
  return {
    breadcrumb: "Devices & Connections / Runtime",
    kind: "Runtime",
    title: "Line runtime",
    health: { word: "Running", tone: "ok" },
    primary: [],
    secondary: [],
    sections: [],
    ...overrides,
  };
}

/** The toolbar's three actions and the switch's two words, as Rust declares them. */
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
      enabled: true,
      emphasis: "secondary",
      op: { kind: "ui", ui: { kind: "open", pane: { kind: "discover" } } },
    },
    add_host_slot: {
      id: "add_host",
      label: "Add host",
      enabled: true,
      emphasis: "secondary",
      op: { kind: "ui", ui: { kind: "open", pane: { kind: "add" } } },
    },
    view_switch: { topology: "Topology", list: "List" },
  };
}

/** A shell view holding one runtime with the given inspector, for the App-level tests. */
function panelView(runtimeInspector: Record<string, unknown>): Record<string, unknown> {
  return {
    toolbar: toolbar(),
    computers: [
      {
        id: "host-1",
        heading: "This computer",
        reachability: { word: "Reachable", tone: "ok" },
        runtimes: [
          {
            id: "runtime-1",
            name: "Line runtime",
            state: { word: "Running", tone: "ok" },
            inspector: runtimeInspector,
            context_menu: [action({ id: "focus", label: "Focus", emphasis: "secondary" })],
          },
        ],
      },
    ],
  };
}

/** A shell view holding one runtime with the given right-click menu, for the menu tests. */
function menuView(contextMenu: Record<string, unknown>[]): Record<string, unknown> {
  return {
    toolbar: toolbar(),
    computers: [
      {
        id: "host-1",
        heading: "This computer",
        reachability: { word: "Reachable", tone: "ok" },
        runtimes: [
          {
            id: "runtime-1",
            name: "Line runtime",
            state: { word: "Running", tone: "ok" },
            inspector: inspector({ title: "Line runtime" }),
            context_menu: contextMenu,
          },
        ],
      },
    ],
  };
}

/** A shell view whose computer and runtime each offer an Edit slot, for the hiding test. */
function slotView(): Record<string, unknown> {
  return {
    toolbar: toolbar(),
    computers: [
      {
        id: "host-1",
        heading: "This computer",
        reachability: { word: "Reachable", tone: "ok" },
        inspector: computerInspector(),
        slots: [action({ id: "set_up_runtime", label: "Set up runtime", emphasis: "secondary" })],
        runtimes: [
          {
            id: "runtime-1",
            name: "Line runtime",
            state: { word: "Running", tone: "ok" },
            inspector: runtimeFacts(),
            context_menu: [action({ id: "focus", label: "Focus", emphasis: "secondary" })],
            slots: [action({ id: "add_connection", label: "Add connection", emphasis: "secondary" })],
          },
        ],
      },
    ],
  };
}

/** The inspector of a computer, with its own literal facts. */
function computerInspector(): Record<string, unknown> {
  return inspector({
    breadcrumb: "Devices & Connections / Computer",
    kind: "Computer",
    title: "This computer",
    health: { word: "Reachable", tone: "ok" },
    primary: [],
    secondary: [
      action({ id: "focus", label: "Focus", emphasis: "secondary" }),
      action({ id: "settings", label: "Settings", emphasis: "secondary" }),
    ],
    sections: [
      {
        rows: [
          { label: "Board", value: "Raspberry Pi 5", wide: false },
          { label: "Address", value: "192.168.10.20", wide: false, copy: "192.168.10.20" },
        ],
      },
    ],
  });
}

/** The inspector of a runtime, with its own literal facts. */
function runtimeFacts(): Record<string, unknown> {
  return inspector({
    breadcrumb: "Devices & Connections / Runtime",
    kind: "Runtime",
    title: "Line runtime",
    health: { word: "Degraded", tone: "warning" },
    primary: [
      action({ id: "connect", label: "Connect", emphasis: "primary" }),
      action({
        id: "live_values",
        label: "Live Values",
        enabled: false,
        reason: "Connect to this runtime to see its live values.",
        emphasis: "quiet",
      }),
    ],
    secondary: [action({ id: "set_run_target", label: "Set as run target", emphasis: "secondary" })],
    sections: [
      {
        rows: [
          {
            label: "State",
            value: "Degraded · Runtime answered fleet.topology.",
            tone: "warning",
            wide: false,
          },
          { label: "Address", value: "192.168.77.10:5680", wide: false, copy: "192.168.77.10:5680" },
        ],
      },
      { heading: "Connector", rows: [{ label: "Health", value: "OK", wide: true }] },
    ],
  });
}

/** The inspector of a connection, with its own literal facts. */
function connectionInspector(): Record<string, unknown> {
  return inspector({
    breadcrumb: "Devices & Connections / Client · device",
    kind: "Client · device",
    title: "Modbus meter",
    health: { word: "Connected", tone: "ok" },
    primary: [action({ id: "edit", label: "Edit settings", emphasis: "primary" })],
    secondary: [action({ id: "test", label: "Test", emphasis: "secondary" })],
    sections: [
      {
        rows: [
          { label: "Name", value: "Modbus meter", wide: false },
          { label: "Device address", value: "127.0.0.1:502", wide: false },
        ],
      },
    ],
  });
}

/** The inspector of a device, with its own literal facts. */
function deviceInspector(): Record<string, unknown> {
  return inspector({
    breadcrumb: "Devices & Connections / Shared system",
    kind: "Shared system",
    title: "MQTT broker",
    health: { word: "Connected", tone: "neutral" },
    primary: [],
    secondary: [action({ id: "focus", label: "Focus", emphasis: "secondary" })],
    sections: [
      {
        rows: [
          { label: "Presents", value: "MQTT broker", wide: false },
          { label: "Address", value: "127.0.0.1:1883", wide: false },
        ],
      },
    ],
  });
}

/** A view holding one computer, one runtime, one connection and one device, each with its inspector. */
function fullView(): Record<string, unknown> {
  return {
    toolbar: toolbar(),
    computers: [
      {
        id: "host-1",
        heading: "This computer",
        reachability: { word: "Reachable", tone: "ok" },
        inspector: computerInspector(),
        runtimes: [
          {
            id: "runtime-1",
            name: "Line runtime",
            state: { word: "Running", tone: "ok" },
            inspector: runtimeFacts(),
            connections: [
              {
                id: "conn-1",
                name: "Modbus meter",
                dot: "ok",
                live: true,
                role_word: "CLIENT",
                marks: [],
                look: "normal",
                inspector: connectionInspector(),
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
        inspector: deviceInspector(),
      },
    ],
  };
}

suite("Devices & Connections panel inspector", () => {
  test("the_inspector_shows_every_fact_of_each_item_kind", async () => {
    const App = loadComponent("App");
    const cases = [
      { kind: "computer", target: '[data-role="computer-heading"]', inspector: computerInspector() },
      { kind: "runtime", target: '[data-item-id="runtime-1"]', inspector: runtimeFacts() },
      { kind: "connection", target: '[data-item-id="conn-1"]', inspector: connectionInspector() },
      { kind: "device", target: '[data-item-id="device-1"]', inspector: deviceInspector() },
    ];
    const rendered = await render(App, { view: fullView(), post: () => undefined });
    try {
      for (const item of cases) {
        const target = rendered.container.querySelector(item.target);
        assert.ok(target, `${item.kind}: the item is drawn`);
        await act(async () => {
          target.click();
        });
        const root = rendered.container.querySelector('[data-role="inspector"]');
        assert.ok(root, `${item.kind}: choosing the item opens its inspector`);
        assert.strictEqual(
          root.querySelector('[data-role="inspector-breadcrumb"]').textContent,
          item.inspector.breadcrumb,
          `${item.kind}: the breadcrumb is drawn as the view gives it`,
        );
        assert.strictEqual(
          root.querySelector('[data-role="inspector-title"]').textContent,
          item.inspector.title,
          `${item.kind}: the title is drawn as the view gives it`,
        );
        const health = item.inspector.health as { word: string; tone: string };
        assert.strictEqual(
          root.querySelector('[data-role="inspector-health"]').getAttribute("data-tone"),
          health.tone,
          `${item.kind}: the health dot is drawn in the health's own tone`,
        );
        assert.strictEqual(
          root.querySelector('[data-role="inspector-health-word"]').textContent,
          health.word,
          `${item.kind}: the health word is drawn as the view gives it`,
        );

        const labels: string[] = [...root.querySelectorAll('[data-role="row-label"]')].map(
          (label) => label.textContent,
        );
        const values: string[] = [...root.querySelectorAll('[data-role="row-value"]')].map(
          (value) => value.textContent,
        );
        const expectedRows = (
          item.inspector.sections as Array<{ rows: Array<{ label: string; value: string }> }>
        ).flatMap((section) => section.rows);
        assert.deepStrictEqual(labels, expectedRows.map((row) => row.label), `${item.kind}: rows`);
        assert.deepStrictEqual(values, expectedRows.map((row) => row.value), `${item.kind}: values`);

        const actionLabels: string[] = [...root.querySelectorAll("[data-action]")].map(
          (cell) => cell.querySelector('[data-role="action-button"]').textContent,
        );
        const expectedActions = [
          ...(item.inspector.primary as Array<{ label: string }>),
          ...(item.inspector.secondary as Array<{ label: string }>),
        ];
        assert.deepStrictEqual(
          actionLabels,
          expectedActions.map((entry) => entry.label),
          `${item.kind}: primary then secondary actions`,
        );

        const wideCount = (
          item.inspector.sections as Array<{ rows: Array<{ wide: boolean }> }>
        )
          .flatMap((section) => section.rows)
          .filter((row) => row.wide).length;
        assert.strictEqual(
          root.querySelectorAll('[data-role="inspector-row"][data-wide="true"]').length,
          wideCount,
          `${item.kind}: a wide row takes the whole width`,
        );

        const copyRow = expectedRows.find(
          (row) => (row as { copy?: string }).copy !== undefined,
        ) as { copy: string } | undefined;
        const copyButton = root.querySelector('[data-role="row-copy"]');
        if (copyRow === undefined) {
          assert.strictEqual(copyButton, null, `${item.kind}: no copy button without copy text`);
        } else {
          rendered.copied.length = 0;
          await act(async () => {
            copyButton.click();
          });
          assert.deepStrictEqual(
            rendered.copied,
            [copyRow.copy],
            `${item.kind}: the copy button copies the row's own text`,
          );
        }
      }
    } finally {
      await rendered.unmount();
    }
  });

  test("the_view_switch_draws_its_words_from_the_toolbar", async () => {
    const Toolbar = loadComponent("Toolbar");
    const selected: string[] = [];
    const rendered = await render(Toolbar, {
      toolbar: toolbar(),
      view: "list",
      onSelectView: (view: string) => {
        selected.push(view);
      },
      onAction: () => undefined,
    });
    try {
      const buttons = [...rendered.container.querySelectorAll('[data-role="view-switch"] button')];
      assert.deepStrictEqual(
        buttons.map((button) => button.textContent),
        ["Topology", "List"],
        "the switch draws the two words the toolbar gives, never a label of its own",
      );
      assert.deepStrictEqual(
        buttons.map((button) => button.getAttribute("data-view")),
        ["topology", "list"],
        "each choice keeps its own view",
      );
      assert.strictEqual(
        buttons[1].getAttribute("data-selected"),
        "true",
        "the shown view is the selected one",
      );
      const actions = [...rendered.container.querySelectorAll("[data-toolbar-item]")].map(
        (item) => item.textContent,
      );
      assert.deepStrictEqual(
        actions,
        ["+ Add", "Discover", "Add host"],
        "the switch adds no button of its own to the toolbar's actions",
      );
      await act(async () => {
        buttons[0].click();
      });
      assert.deepStrictEqual(selected, ["topology"], "choosing a view hands it back to the shell");
    } finally {
      await rendered.unmount();
    }
  });

  test("primary_and_secondary_actions_are_drawn_by_their_emphasis", async () => {
    const Inspector = loadComponent("Inspector");
    const rendered = await render(Inspector, {
      inspector: inspector({
        primary: [
          action({ id: "connect", label: "Connect", emphasis: "primary" }),
          action({ id: "focus", label: "Focus", emphasis: "quiet" }),
        ],
        secondary: [
          action({ id: "test", label: "Test", emphasis: "secondary" }),
          action({ id: "remove", label: "Remove", emphasis: "danger" }),
        ],
      }),
      onAction: () => undefined,
      onClose: () => undefined,
    });
    try {
      const cells = [...rendered.container.querySelectorAll("[data-action]")];
      assert.deepStrictEqual(
        cells.map((cell) => cell.getAttribute("data-action")),
        ["connect", "focus", "test", "remove"],
        "the primary actions come before the secondary ones, each in its given order",
      );
      assert.deepStrictEqual(
        cells.map((cell) => cell.getAttribute("data-emphasis")),
        ["primary", "quiet", "secondary", "danger"],
        "each action is drawn by its own emphasis",
      );
      const fontWeight = (id: string) =>
        cells
          .find((cell) => cell.getAttribute("data-action") === id)
          .querySelector('[data-role="action-button"]').style.fontWeight;
      assert.strictEqual(fontWeight("connect"), "650", "a primary action is drawn filled");
      assert.strictEqual(fontWeight("focus"), "500", "a quiet action is drawn lightly");
      assert.strictEqual(fontWeight("test"), "500", "a secondary action is drawn lightly");
      assert.strictEqual(fontWeight("remove"), "500", "a danger action is drawn lightly");
    } finally {
      await rendered.unmount();
    }
  });

  test("the_browse_entry_posts_its_open_action_unchanged", async () => {
    const Inspector = loadComponent("Inspector");
    // stopped_browses: the OPC UA server serves its globals from this project, so its `Choose globals` entry is
    // open without a runtime and posts the action Rust gave, unchanged (design D5c R10). A wrong implementation
    // posts a copy without its request, or posts nothing.
    const view = stoppedBrowsesView();
    const server = connectionNamed(view, "OPC UA server");
    const entry = server.inspector.primary.find((action: any) => action.id === "browse");
    assert.ok(entry, "the server offers its browse");
    const posted: unknown[] = [];
    const rendered = await render(Inspector, {
      inspector: server.inspector,
      onAction: (action: unknown) => {
        posted.push(action);
      },
      onClose: () => undefined,
    });
    try {
      const button = rendered.container.querySelector(
        '[data-action="browse"] [data-role="action-button"]',
      );
      assert.strictEqual(button.textContent, "Choose globals", "the entry says what it opens");
      assert.strictEqual(button.disabled, false, "the project's own globals open without a runtime");
      await act(async () => {
        button.click();
      });
      assert.deepStrictEqual(posted, [entry], "the entry posts its open action unchanged");
    } finally {
      await rendered.unmount();
    }
  });

  test("a_disabled_remote_browse_says_why_it_waits", async () => {
    const Inspector = loadComponent("Inspector");
    // stopped_browses: the ADS PLC needs its runtime, which is stopped. Its entry is disabled and shows the reason
    // in the words of Rust (design D5c R10). A wrong implementation enables the entry, or drops the reason.
    const ads = connectionNamed(stoppedBrowsesView(), "ADS PLC");
    const rendered = await render(Inspector, {
      inspector: ads.inspector,
      onAction: () => undefined,
      onClose: () => undefined,
    });
    try {
      const cell = rendered.container.querySelector('[data-action="browse"]');
      const button = cell.querySelector('[data-role="action-button"]');
      const reason = "Start or connect the runtime, then Browse again to load symbols.";
      assert.strictEqual(button.textContent, "Manage tags", "the entry says what it browses");
      assert.strictEqual(button.disabled, true, "a remote browse waits for its runtime");
      assert.strictEqual(
        cell.querySelector('[data-role="action-reason"]').textContent,
        reason,
        "the reason is shown as text",
      );
      assert.strictEqual(button.getAttribute("title"), reason, "the reason is the tooltip");
    } finally {
      await rendered.unmount();
    }
  });

  test("a_disabled_action_shows_its_reason_as_text_and_tooltip", async () => {
    const Inspector = loadComponent("Inspector");
    const reason = "Deploy is not available for this target yet.";
    const posted: unknown[] = [];
    const rendered = await render(Inspector, {
      inspector: inspector({
        primary: [
          action({
            id: "deploy",
            label: "Deploy",
            enabled: false,
            reason,
            emphasis: "quiet",
          }),
        ],
      }),
      onAction: (entry: unknown) => {
        posted.push(entry);
      },
      onClose: () => undefined,
    });
    try {
      const cell = rendered.container.querySelector('[data-action="deploy"]');
      const button = cell.querySelector('[data-role="action-button"]');
      assert.strictEqual(button.disabled, true, "a disabled action is not clickable");
      assert.strictEqual(
        cell.querySelector('[data-role="action-reason"]').textContent,
        reason,
        "the reason is shown as text",
      );
      assert.strictEqual(button.getAttribute("title"), reason, "the reason is the tooltip");
      await act(async () => {
        button.click();
      });
      assert.deepStrictEqual(posted, [], "a disabled action posts nothing");
    } finally {
      await rendered.unmount();
    }
  });

  test("a_confirmed_action_posts_only_after_the_confirm_button", async () => {
    const App = loadComponent("App");
    const remove = action({
      id: "remove",
      label: "Remove",
      enabled: true,
      emphasis: "danger",
      confirm: {
        text: "Remove this connection? The runtime stops using it.",
        confirm: "Remove",
        cancel: "Cancel",
      },
      op: { kind: "rust", action: "connection.remove", payload: { connection: "conn-1" } },
    });
    const posted: unknown[] = [];
    const rendered = await render(App, {
      view: panelView(inspector({ secondary: [remove] })),
      post: (message: unknown) => {
        posted.push(message);
      },
    });
    try {
      const card = rendered.container.querySelector('[data-item-id="runtime-1"]');
      await act(async () => {
        card.click();
      });
      const cell = rendered.container.querySelector('[data-action="remove"]');
      await act(async () => {
        cell.querySelector('[data-role="action-button"]').click();
      });
      assert.deepStrictEqual(posted, [], "choosing a confirmed action posts nothing yet");
      assert.strictEqual(
        rendered.container.querySelector('[data-role="confirm-text"]').textContent,
        "Remove this connection? The runtime stops using it.",
        "the confirmation text is drawn as given",
      );
      const confirmButton = rendered.container.querySelector('[data-role="confirm-yes"]');
      const cancelButton = rendered.container.querySelector('[data-role="confirm-cancel"]');
      assert.strictEqual(confirmButton.textContent, "Remove");
      assert.strictEqual(cancelButton.textContent, "Cancel");

      await act(async () => {
        cancelButton.click();
      });
      assert.deepStrictEqual(posted, [], "cancelling posts nothing");
      assert.strictEqual(
        rendered.container.querySelector('[data-role="confirm-text"]'),
        null,
        "cancelling closes the confirmation",
      );

      await act(async () => {
        cell.querySelector('[data-role="action-button"]').click();
      });
      await act(async () => {
        rendered.container.querySelector('[data-role="confirm-yes"]').click();
      });
      assert.deepStrictEqual(
        posted,
        [{ type: "action", action: remove }],
        "the confirming button posts the action, with its op, in the shell's envelope",
      );
    } finally {
      await rendered.unmount();
    }
  });

  test("a_runtimes_context_menu_shows_its_four_entries_from_the_view_model", async () => {
    const App = loadComponent("App");
    const addEntry = action({
      id: "add",
      label: "Add device or connection…",
      emphasis: "secondary",
      op: { kind: "ui", ui: { kind: "open", pane: { kind: "add" } } },
    });
    const setRunTarget = action({
      id: "set_run_target",
      label: "Set as run target",
      enabled: false,
      reason: "This runtime is already the run target.",
      emphasis: "secondary",
      op: { kind: "rust", action: "runtime.set_run_target", payload: { runtime: "runtime-1" } },
    });
    const settings = action({
      id: "settings",
      label: "Settings",
      emphasis: "secondary",
      op: { kind: "ui", ui: { kind: "open", pane: { kind: "settings" } } },
    });
    const focus = action({
      id: "focus",
      label: "Focus",
      emphasis: "secondary",
      op: { kind: "ui", ui: { kind: "focus", item: "runtime-1" } },
    });
    const posted: unknown[] = [];
    const rendered = await render(App, {
      view: menuView([addEntry, setRunTarget, settings, focus]),
      post: (message: unknown) => {
        posted.push(message);
      },
    });
    try {
      const card = rendered.container.querySelector('[data-item-id="runtime-1"]');
      await act(async () => {
        card.dispatchEvent(
          new rendered.window.MouseEvent("contextmenu", { bubbles: true, cancelable: true }),
        );
      });
      const menu = rendered.container.querySelector('[data-role="context-menu"]');
      assert.ok(menu, "a right-click on a runtime opens its menu");
      const cells = [...menu.querySelectorAll("[data-action]")];
      assert.deepStrictEqual(
        cells.map((cell) => cell.getAttribute("data-action")),
        ["add", "set_run_target", "settings", "focus"],
        "the four entries are drawn in the view's order",
      );
      assert.deepStrictEqual(
        cells.map((cell) => cell.querySelector('[data-role="action-button"]').textContent),
        ["Add device or connection…", "Set as run target", "Settings", "Focus"],
        "each entry is drawn with its own label",
      );
      const disabled = cells.find((cell) => cell.getAttribute("data-action") === "set_run_target");
      const disabledButton = disabled.querySelector('[data-role="action-button"]');
      assert.strictEqual(disabledButton.disabled, true, "a disabled entry is not clickable");
      assert.strictEqual(
        disabled.querySelector('[data-role="action-reason"]').textContent,
        "This runtime is already the run target.",
        "a disabled entry shows its reason as text",
      );
      assert.strictEqual(
        disabledButton.getAttribute("title"),
        "This runtime is already the run target.",
        "the reason is the disabled entry's tooltip",
      );
      await act(async () => {
        disabledButton.click();
      });
      assert.deepStrictEqual(posted, [], "choosing the disabled entry posts nothing");

      await act(async () => {
        cells
          .find((cell) => cell.getAttribute("data-action") === "add")
          .querySelector('[data-role="action-button"]')
          .click();
      });
      assert.deepStrictEqual(
        posted,
        [{ type: "action", action: addEntry }],
        "the chosen op is posted unchanged in the shell's envelope",
      );
    } finally {
      await rendered.unmount();
    }
  });

  test("right_click_on_a_device_or_connection_opens_no_menu", async () => {
    const App = loadComponent("App");
    const rendered = await render(App, { view: fullView(), post: () => undefined });
    try {
      for (const target of ['[data-item-id="conn-1"]', '[data-item-id="device-1"]']) {
        const item = rendered.container.querySelector(target);
        assert.ok(item, `${target}: the item is drawn`);
        await act(async () => {
          item.dispatchEvent(
            new rendered.window.MouseEvent("contextmenu", { bubbles: true, cancelable: true }),
          );
        });
        assert.strictEqual(
          rendered.container.querySelector('[data-role="context-menu"]'),
          null,
          `${target}: a right-click opens no menu`,
        );
      }
    } finally {
      await rendered.unmount();
    }
  });

  test("edit_slots_hide_while_a_pane_is_open", async () => {
    const App = loadComponent("App");
    const rendered = await render(App, { view: slotView(), post: () => undefined });
    try {
      const slotIds = () =>
        [...rendered.container.querySelectorAll("[data-slot]")].map((slot) =>
          slot.getAttribute("data-slot"),
        );
      const card = rendered.container.querySelector('[data-item-id="runtime-1"]');
      assert.deepStrictEqual(
        slotIds(),
        ["set_up_runtime", "add_connection"],
        "the Edit slots are drawn while no pane is open",
      );

      await act(async () => {
        card.click();
      });
      assert.ok(
        rendered.container.querySelector('[data-role="inspector"]'),
        "choosing the runtime opens its inspector",
      );
      assert.deepStrictEqual(slotIds(), [], "the inspector hides the Edit slots");

      await act(async () => {
        rendered.window.dispatchEvent(
          new rendered.window.KeyboardEvent("keydown", { key: "Escape", bubbles: true }),
        );
      });
      assert.strictEqual(
        rendered.container.querySelector('[data-role="inspector"]'),
        null,
        "Escape closes the inspector",
      );
      assert.deepStrictEqual(
        slotIds(),
        ["set_up_runtime", "add_connection"],
        "closing the pane brings the Edit slots back",
      );

      await act(async () => {
        card.dispatchEvent(
          new rendered.window.MouseEvent("contextmenu", { bubbles: true, cancelable: true }),
        );
      });
      assert.ok(
        rendered.container.querySelector('[data-role="context-menu"]'),
        "the context menu is open",
      );
      assert.deepStrictEqual(slotIds(), [], "the context menu hides the Edit slots");
    } finally {
      await rendered.unmount();
    }
  });

  test("opening_the_inspector_closes_the_other_pane_and_escape_closes_it", async () => {
    const App = loadComponent("App");
    const rendered = await render(App, {
      view: panelView(inspector({ title: "Line runtime" })),
      post: () => undefined,
    });
    try {
      const card = rendered.container.querySelector('[data-item-id="runtime-1"]');
      await act(async () => {
        card.dispatchEvent(
          new rendered.window.MouseEvent("contextmenu", { bubbles: true, cancelable: true }),
        );
      });
      assert.ok(
        rendered.container.querySelector('[data-role="context-menu"]'),
        "the menu is open first",
      );
      assert.strictEqual(rendered.container.querySelector('[data-role="inspector"]'), null);

      await act(async () => {
        card.click();
      });
      assert.ok(
        rendered.container.querySelector('[data-role="inspector"]'),
        "choosing an item opens its inspector",
      );
      assert.strictEqual(
        rendered.container.querySelector('[data-role="context-menu"]'),
        null,
        "opening the inspector closes the other pane",
      );

      await act(async () => {
        rendered.window.dispatchEvent(
          new rendered.window.KeyboardEvent("keydown", { key: "Escape", bubbles: true }),
        );
      });
      assert.strictEqual(
        rendered.container.querySelector('[data-role="inspector"]'),
        null,
        "Escape closes the inspector",
      );

      await act(async () => {
        card.click();
      });
      assert.ok(rendered.container.querySelector('[data-role="inspector"]'));
      await act(async () => {
        rendered.container.querySelector('[data-role="inspector-close"]').click();
      });
      assert.strictEqual(
        rendered.container.querySelector('[data-role="inspector"]'),
        null,
        "the close button closes the inspector",
      );
    } finally {
      await rendered.unmount();
    }
  });
});
