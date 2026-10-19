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

// The list and the shell are drawn from the Rust view model; the expected values below are the
// literals of that contract (crates/trust-devices/src/view/types/graph.rs `ListView`, `Column` and
// `ListCells`, design §2.1), never a value read back from a component.

/** A webview component under test, bundled the way the extension ships it. */
type WebviewComponent = React.ComponentType<Record<string, unknown>>;

/** Bundles one webview entry point and returns its named export, as the extension ships it. */
function loadComponent(component: "App" | "ListView"): WebviewComponent {
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
  unmount(): Promise<void>;
}

async function render(
  component: WebviewComponent,
  props: Record<string, unknown>,
): Promise<Rendered> {
  const dom = new JSDOM('<!doctype html><body><div id="root"></div></body>', {
    url: "https://devices-list.test/",
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

/** The toolbar's three actions, as Rust declares them. */
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
  };
}

/** An inspector holding only its identity, for the row-selection test. */
function inspector(title: string): Record<string, unknown> {
  return {
    breadcrumb: "Devices & Connections / Runtime",
    kind: "Runtime",
    title,
    primary: [],
    secondary: [],
    sections: [],
  };
}

suite("Devices & Connections panel list view", () => {
  test("the_list_has_the_given_columns_and_cells", async () => {
    const ListView = loadComponent("ListView");
    const rendered = await render(ListView, {
      columns: [
        { id: "kind", label: "Kind" },
        { id: "name", label: "Name" },
        { id: "state", label: "State" },
      ],
      computers: [
        {
          id: "host-1",
          list: { kind: "computer", cells: { kind: "Computer", name: "This computer", state: "Reachable" } },
          runtimes: [
            {
              id: "runtime-1",
              list: { kind: "runtime", cells: { kind: "Runtime", name: "Simulator", state: "Running" } },
            },
          ],
        },
      ],
      devices: [
        {
          id: "device-1",
          list: { kind: "device", cells: { kind: "Device", name: "MQTT broker", state: "Connected" } },
        },
      ],
      onSelectRow: () => undefined,
    });
    try {
      const headings = [...rendered.container.querySelectorAll('[data-role="list-heading"]')].map(
        (heading) => heading.textContent,
      );
      assert.deepStrictEqual(
        headings,
        ["Kind", "Name", "State"],
        "the columns are drawn under their own headings, in the view's order",
      );
      const rows = [...rendered.container.querySelectorAll('[data-role="list-row"]')];
      assert.deepStrictEqual(
        rows.map((row) => row.getAttribute("data-item-id")),
        ["host-1", "runtime-1", "device-1"],
        "one row per computer, runtime and device of the canvas",
      );
      const cellsOf = (id: string): string[] =>
        [...rendered.container.querySelectorAll(`[data-item-id="${id}"] td`)].map(
          (cell) => cell.textContent,
        );
      assert.deepStrictEqual(
        cellsOf("host-1"),
        ["Computer", "This computer", "Reachable"],
        "each row's cells come from the item's own cells, by column id",
      );
      assert.deepStrictEqual(
        cellsOf("runtime-1"),
        ["Runtime", "Simulator", "Running"],
        "the runtime's row is filled from its own cells",
      );
      assert.deepStrictEqual(
        cellsOf("device-1"),
        ["Device", "MQTT broker", "Connected"],
        "the device's row is filled from its own cells",
      );
    } finally {
      await rendered.unmount();
    }
  });

  test("hidden_items_have_no_row_and_dimmed_rows_are_dimmed", async () => {
    const ListView = loadComponent("ListView");
    const rendered = await render(ListView, {
      columns: [{ id: "name", label: "Name" }],
      computers: [
        {
          id: "host-visible",
          list: { kind: "computer", cells: { name: "This computer" } },
          runtimes: [
            {
              id: "runtime-visible",
              list: { kind: "runtime", cells: { name: "Simulator" } },
              connections: [
                { id: "conn-visible", list: { kind: "connection", cells: { name: "Modbus meter" } } },
                {
                  id: "conn-hidden",
                  hidden: true,
                  list: { kind: "connection", cells: { name: "Hidden connection" } },
                },
              ],
            },
          ],
        },
      ],
      devices: [
        { id: "device-dimmed", dimmed: true, list: { kind: "device", cells: { name: "MQTT broker" } } },
        { id: "device-hidden", hidden: true, list: { kind: "device", cells: { name: "Hidden device" } } },
      ],
      onSelectRow: () => undefined,
    });
    try {
      const ids = [...rendered.container.querySelectorAll('[data-role="list-row"]')].map((row) =>
        row.getAttribute("data-item-id"),
      );
      assert.deepStrictEqual(
        ids,
        ["host-visible", "runtime-visible", "conn-visible", "device-dimmed"],
        "a hidden connection or device has no row",
      );
      assert.strictEqual(
        rendered.container.querySelector('[data-item-id="device-dimmed"]').getAttribute("data-dimmed"),
        "true",
        "a dimmed item's row is dimmed",
      );
      assert.strictEqual(
        rendered.container
          .querySelector('[data-item-id="conn-visible"]')
          .getAttribute("data-dimmed"),
        "false",
        "a lit item's row is not dimmed",
      );
    } finally {
      await rendered.unmount();
    }
  });

  test("a_heading_sorts_and_a_second_click_reverses", async () => {
    const ListView = loadComponent("ListView");
    const rendered = await render(ListView, {
      columns: [{ id: "name", label: "Name" }],
      computers: [
        {
          id: "host-1",
          list: { kind: "computer", cells: { name: "cherry" } },
          runtimes: [
            { id: "r-apple", list: { kind: "runtime", cells: { name: "apple" } } },
            { id: "r-banana", list: { kind: "runtime", cells: { name: "banana" } } },
          ],
        },
      ],
      devices: [],
      onSelectRow: () => undefined,
    });
    try {
      const order = (): (string | null)[] =>
        [...rendered.container.querySelectorAll('[data-role="list-row"]')].map((row) =>
          row.getAttribute("data-item-id"),
        );
      assert.deepStrictEqual(
        order(),
        ["host-1", "r-apple", "r-banana"],
        "the rows start in the canvas's own order",
      );
      const heading = rendered.container.querySelector('[data-column="name"]');
      assert.ok(heading, "the column has a heading button");
      await act(async () => {
        heading.click();
      });
      assert.deepStrictEqual(
        order(),
        ["r-apple", "r-banana", "host-1"],
        "the first click sorts the rows by the column's displayed text",
      );
      await act(async () => {
        heading.click();
      });
      assert.deepStrictEqual(
        order(),
        ["host-1", "r-banana", "r-apple"],
        "a second click on the same heading reverses the order",
      );
    } finally {
      await rendered.unmount();
    }
  });

  test("selecting_a_row_opens_its_inspector", async () => {
    const App = loadComponent("App");
    const rendered = await render(App, {
      view: {
        toolbar: toolbar(),
        computers: [
          {
            id: "host-1",
            heading: "This computer",
            reachability: { word: "Reachable", tone: "ok" },
            list: { kind: "computer", cells: { name: "This computer" } },
            runtimes: [
              {
                id: "runtime-1",
                name: "Line runtime",
                state: { word: "Running", tone: "ok" },
                inspector: inspector("Line runtime"),
                list: { kind: "runtime", cells: { name: "Line runtime" } },
              },
            ],
          },
        ],
        list: { columns: [{ id: "name", label: "Name" }] },
      },
      post: () => undefined,
    });
    try {
      await act(async () => {
        rendered.container.querySelector('[data-view="list"]').click();
      });
      const row = rendered.container.querySelector('[data-item-id="runtime-1"]');
      assert.ok(row, "the runtime has a row in the list");
      await act(async () => {
        row.click();
      });
      const pane = rendered.container.querySelector('[data-role="inspector"]');
      assert.ok(pane, "selecting a row opens the item's inspector");
      assert.strictEqual(
        pane.querySelector('[data-role="inspector-title"]').textContent,
        "Line runtime",
        "the row opens the same inspector the canvas opens",
      );
    } finally {
      await rendered.unmount();
    }
  });
});
