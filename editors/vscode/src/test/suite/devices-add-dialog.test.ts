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

// The Add dialog is drawn from the Rust view model (design §4.5 "D3: forms and the Add dialog",
// specification 25 §5.9 and §5.14). The expected values below are the literal texts of the golden
// views that `crates/trust-devices/src/view/tests/golden.rs` writes; they are never read back from
// the component.

/** A webview component under test, bundled the way the extension ships it. */
type WebviewComponent = React.ComponentType<Record<string, unknown>>;

/** Bundles one webview entry point and returns its named export, as the extension ships it. */
function loadComponent(component: "AddDialog" | "App"): WebviewComponent {
  const extensionRoot = path.resolve(__dirname, "..", "..", "..");
  const temp = fs.mkdtempSync(
    path.join(extensionRoot, "node_modules", `.trust-devices-add-dialog-${component}-`),
  );
  const outfile = path.join(temp, `devices-add-dialog-${component}.cjs`);
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

// The golden views, written by the Rust test and never by hand.
const fixtures = path.resolve(
  path.resolve(__dirname, "..", "..", ".."),
  "..",
  "..",
  "crates",
  "trust-devices",
  "tests",
  "fixtures",
  "devices_view",
);

/** The golden view `name`, as Rust serialized it. */
function goldenView(name: string): any {
  return JSON.parse(fs.readFileSync(path.join(fixtures, `${name}.json`), "utf8"));
}

/** The `add` member of the golden view `name`: the Add dialog Rust gives. */
function addOf(name: string): any {
  return goldenView(name).add;
}

// JSDOM's elements stay untyped, as in devices-form-actions.test.ts: the extension's tsconfig has no
// DOM lib, so a `HTMLElement` here would not name a type.
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
    url: "https://devices-add-dialog.test/",
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

/** Renders the dialog alone with a golden view's `add`, deciding nothing itself. */
async function renderDialog(add: any, onAction: (action: unknown) => void = () => undefined) {
  return render(loadComponent("AddDialog"), { dialog: add, onAction, onClose: () => undefined });
}

/** Clicks an element the way a user does. */
async function click(rendered: Rendered, element: any): Promise<void> {
  assert.ok(element, "the element to click is drawn");
  await act(async () => {
    element.click();
  });
}

/** The labels of the groups the dialog draws, in the order they are drawn. */
function groupLabels(container: any): string[] {
  return [...container.querySelectorAll('[data-role="add-group"]')].map((group: any) =>
    group.getAttribute("data-group"),
  );
}

/** The protocols of the entries a section draws, in the order they are drawn. */
function entryProtocols(container: any): string[] {
  return [...container.querySelectorAll("[data-entry]")].map((entry: any) =>
    entry.getAttribute("data-entry"),
  );
}

suite("Devices & Connections Add dialog", () => {
  test("runtimes_come_first_then_the_groups_in_order", async () => {
    // The `add` of the golden view `search_and_filter`, whose catalog gives the canonical groups.
    const add = addOf("search_and_filter");
    const rendered = await renderDialog(add);
    try {
      const container = rendered.container;
      // specification 25 §5.14: the groups are §5.9's, "preceded by `Runtimes`", and §5.9 fixes their
      // canonical order (Devices and I/O, Read tags from another PLC or server, Share truST values,
      // Send and receive messages; the advanced Advanced integrations stays hidden until the toggle).
      assert.deepStrictEqual(
        groupLabels(container),
        [add.runtimes.label, ...add.groups.filter((group: any) => !group.advanced).map((group: any) => group.label)],
        "the Runtimes group is drawn first, then the catalog's groups in Rust's order",
      );
      assert.strictEqual(
        groupLabels(container)[0],
        "Runtimes",
        "the first group is `Runtimes` (specification 25 §5.14)",
      );
      // N01: the Runtimes group holds its entries, in Rust's own order.
      assert.deepStrictEqual(
        [...container.querySelectorAll('[data-role="add-group"][data-group="Runtimes"] [data-entry]')].map(
          (entry: any) => entry.getAttribute("data-entry"),
        ),
        add.runtimes.entries.map((entry: any) => entry.protocol),
        "the Runtimes entries are drawn in the order Rust gives them",
      );
    } finally {
      await rendered.unmount();
    }
  });

  test("found_entries_show_their_rows_and_action", async () => {
    // No golden view carries a found entry: Rust fills `AddDialog.found` from the session's last
    // search, which no golden scenario runs (`view/mod.rs` leaves it empty). The entry below is
    // built from specification 25 §5.14 ("each with its protocol, source, and confidence labels
    // (§2), its details … and `Add`, or `Adopt` for a runtime") and §2 ("human-readable protocol,
    // discovery-source and confidence labels").
    const add = addOf("empty_project");
    const found = {
      card: "found-runtime-1",
      title: "truST runtime",
      rows: [
        { label: "Protocol", value: "truST runtime", wide: false },
        { label: "Source", value: "mDNS", wide: false },
        { label: "Confidence", value: "Confirmed", wide: false },
        { label: "Address", value: "192.168.10.37:5680", wide: false },
      ],
      action: {
        id: "runtime.adopt",
        label: "Adopt",
        enabled: true,
        emphasis: "secondary",
        op: { kind: "rust", action: "runtime.adopt", payload: {} },
      },
    };
    const rendered = await renderDialog({ ...add, found: [found] });
    try {
      const container = rendered.container;
      assert.strictEqual(
        container.querySelector('[data-role="add-found-heading"]').textContent,
        "Found on the network",
        "the finds are drawn under `Found on the network` (specification 25 §5.14)",
      );
      const entry = container.querySelector('[data-role="add-found-entry"]');
      assert.strictEqual(
        entry.getAttribute("data-card"),
        "found-runtime-1",
        "the find is drawn with its own card identity",
      );
      assert.strictEqual(
        entry.querySelector('[data-role="add-found-title"]').textContent,
        "truST runtime",
        "the find's title is drawn as given",
      );
      assert.deepStrictEqual(
        [...entry.querySelectorAll('[data-role="add-found-row"]')].map((row: any) => [
          row.querySelector('[data-role="add-found-row-label"]').textContent,
          row.querySelector('[data-role="add-found-row-value"]').textContent,
        ]),
        [
          ["Protocol", "truST runtime"],
          ["Source", "mDNS"],
          ["Confidence", "Confirmed"],
          ["Address", "192.168.10.37:5680"],
        ],
        "the find's rows are drawn, label and value, in the order Rust gives them",
      );
      assert.strictEqual(
        entry.querySelector('[data-role="add-found-action"]').textContent,
        "Adopt",
        "the find's action is drawn with Rust's own label",
      );
    } finally {
      await rendered.unmount();
    }
  });

  test("advanced_groups_hide_until_the_toggle_shows_them", async () => {
    const add = addOf("search_and_filter");
    const advanced = add.groups.filter((group: any) => group.advanced);
    assert.strictEqual(advanced.length, 1, "the golden view carries one advanced group");
    const rendered = await renderDialog(add);
    try {
      const container = rendered.container;
      // specification 25 §5.9 and the preservation table's toggle (design §2 N03): the advanced
      // entries are hidden until the toggle shows them, its text is its own `show_label` or
      // `hide_label`, and its `count` is the number of hidden entries.
      assert.deepStrictEqual(
        groupLabels(container).includes("Advanced integrations"),
        false,
        "an advanced group is not drawn before the toggle shows it",
      );
      assert.strictEqual(
        container.querySelector('[data-role="add-advanced-label"]').textContent,
        "Show advanced integrations",
        "the toggle's text is its own show_label",
      );
      assert.strictEqual(
        container.querySelector('[data-role="add-advanced-count"]').textContent,
        "4",
        "the toggle's count is drawn as Rust gives it (four hidden entries)",
      );
      assert.strictEqual(
        container.querySelector('[data-role="add-advanced-toggle"]').getAttribute("data-shown"),
        "false",
        "the advanced groups start hidden",
      );

      await click(rendered, container.querySelector('[data-role="add-advanced-toggle"]'));
      assert.strictEqual(
        container.querySelector('[data-role="add-advanced-label"]').textContent,
        "Hide advanced integrations",
        "the toggle's text becomes its own hide_label once shown",
      );
      assert.strictEqual(
        container.querySelector('[data-role="add-advanced-toggle"]').getAttribute("data-shown"),
        "true",
        "the toggle's state is shown after it is chosen",
      );
      assert.deepStrictEqual(
        groupLabels(container),
        [add.runtimes.label, ...add.groups.map((group: any) => group.label)],
        "the advanced group is drawn in its place once the toggle shows it",
      );
      assert.deepStrictEqual(
        [...container.querySelectorAll('[data-role="add-group"][data-group="Advanced integrations"] [data-entry]')].map(
          (entry: any) => entry.getAttribute("data-entry"),
        ),
        advanced[0].entries.map((entry: any) => entry.protocol),
        "the advanced entries are drawn in Rust's own order",
      );
    } finally {
      await rendered.unmount();
    }
  });

  test("quick_starts_appear_while_the_runtime_has_no_connections", async () => {
    // specification 25 §5.14: "When a runtime has no connections yet, the dialog also offers the
    // quick starts … `Local simulated counter`, `Loopback sanity check`, `Modbus drive or meter`,
    // and `MQTT broker`." The golden view `search_and_filter` targets the simulator, which has no
    // connections, so Rust gives all four; `empty_project` targets the same simulator but carries no
    // catalog, so Rust gives none, and the dialog draws none.
    const add = addOf("search_and_filter");
    assert.strictEqual(add.quick_starts.length, 4, "the golden view carries the four quick starts");
    const rendered = await renderDialog(add);
    try {
      const container = rendered.container;
      const starts = [...container.querySelectorAll('[data-role="add-quick-start"]')];
      assert.deepStrictEqual(
        starts.map((start: any) => start.querySelector('[data-role="add-quick-start-title"]').textContent),
        ["Local simulated counter", "Loopback sanity check", "Modbus drive or meter", "MQTT broker"],
        "the quick starts are drawn with Rust's own titles, in Rust's order",
      );
      assert.deepStrictEqual(
        starts.map((start: any) =>
          start.querySelector('[data-role="add-quick-start-description"]').textContent,
        ),
        add.quick_starts.map((start: any) => start.description),
        "each quick start is drawn with Rust's own description",
      );
      assert.deepStrictEqual(
        starts.map((start: any) => start.getAttribute("data-action")),
        add.quick_starts.map((start: any) => start.action.id),
        "each quick start posts the action Rust gives it",
      );
    } finally {
      await rendered.unmount();
    }
    const empty = await renderDialog(addOf("empty_project"));
    try {
      assert.deepStrictEqual(
        [...empty.container.querySelectorAll('[data-role="add-quick-start"]')].map(
          (start: any) => start.getAttribute("data-action"),
        ),
        [],
        "a view whose runtime gives no quick starts draws none",
      );
    } finally {
      await empty.unmount();
    }
  });

  test("catalog_unavailable_replaces_the_groups", async () => {
    // specification 25 §5.14: `Catalog unavailable` is reserved for a missing `trust-runtime`; R1:
    // the dialog draws the catalog state as its notice in place of the groups. No golden view
    // carries both a catalog state and groups (a missing catalog gives none), so this builds one
    // from two golden views: the notice of `empty_project` and the groups of `search_and_filter`.
    const unavailable = addOf("empty_project");
    const dialog = { ...unavailable, groups: addOf("search_and_filter").groups };
    const rendered = await renderDialog(dialog);
    try {
      const container = rendered.container;
      assert.strictEqual(
        container.querySelector('[data-role="add-catalog-notice"]').textContent,
        "No device or connection types are available.",
        "the catalog state is drawn as the dialog's notice",
      );
      assert.deepStrictEqual(
        groupLabels(container),
        ["Runtimes"],
        "the notice is drawn in place of the groups",
      );
      // Rust's own test keeps it: `catalog_unavailable_replaces_the_groups` — the `Runtimes` entries
      // stay visible without a catalog (panes/tests/add.rs, "Runtimes stays").
      assert.deepStrictEqual(
        entryProtocols(container),
        unavailable.runtimes.entries.map((entry: any) => entry.protocol),
        "the Runtimes group is still drawn without a catalog",
      );
    } finally {
      await rendered.unmount();
    }
  });

  test("choosing_an_entry_posts_its_action", async () => {
    const view = goldenView("search_and_filter");
    const modbus = view.add.groups
      .find((group: any) => group.label === "Devices and I/O")
      .entries.find((entry: any) => entry.protocol === "modbus_tcp");
    const posted: any[] = [];
    const rendered = await render(loadComponent("App"), {
      view,
      post: (message: unknown) => {
        posted.push(message);
      },
    });
    try {
      const container = rendered.container;
      // specification 25 §5.14: `+ Add` opens one dialog, drawn from the view's own `add`.
      await click(rendered, container.querySelector('[data-toolbar-item="add"]'));
      const entry = () => container.querySelector('[data-entry="modbus_tcp"]');
      assert.ok(entry(), "the + Add toolbar action opens the Add dialog");
      // R3 and specification 25 §5.9: "Entry titles, badges, and purposes stay as listed here".
      assert.strictEqual(
        entry().querySelector('[data-role="add-entry-title"]').textContent,
        "Modbus",
        "the entry is drawn with Rust's own title",
      );
      assert.strictEqual(
        entry().querySelector('[data-role="add-entry-badge"]').textContent,
        "MB",
        "the entry is drawn with Rust's own badge",
      );
      assert.strictEqual(
        entry().querySelector('[data-role="add-entry-purpose"]').textContent,
        "Connect PLCs, drives, meters, and remote I/O.",
        "the entry is drawn with Rust's own purpose",
      );

      await click(rendered, entry());
      assert.deepStrictEqual(
        posted[posted.length - 1],
        { type: "action", action: modbus.action },
        "choosing an entry posts its action unchanged, in the panel's own envelope",
      );
    } finally {
      await rendered.unmount();
    }
  });

  test("a_runtime_add_connection_slot_opens_the_dialog", async () => {
    // specification 25 §5.14: "opened from a runtime's menu or its `Add connection` slot, it adds to
    // that runtime"; the slot's `ui` op opens the dialog, drawn from the view's own `add`.
    const view = goldenView("search_and_filter");
    const rendered = await render(loadComponent("App"), { view, post: () => undefined });
    try {
      const container = rendered.container;
      assert.strictEqual(
        container.querySelector('[data-role="add-pane"]'),
        null,
        "no dialog is open before the slot is chosen",
      );
      await click(rendered, container.querySelector('[data-slot="add_connection"]'));
      assert.ok(
        container.querySelector('[data-role="add-pane"]'),
        "a runtime's Add connection slot opens the Add dialog",
      );
      // The slot's click is that action alone: the runtime card and the canvas read the same click
      // after the slot does, and the selection they would make must not replace the pane the slot
      // opened (specification 25 §5.11, one pane at a time, with §5.14).
      assert.strictEqual(
        container.querySelector('[data-role="inspector"]'),
        null,
        "the slot's own click does not also select the runtime card it sits in",
      );
      assert.strictEqual(
        container.querySelector('[data-role="add-title"]').textContent,
        view.add.title,
        "the opened dialog is Rust's own add",
      );
    } finally {
      await rendered.unmount();
    }
  });

  test("the_found_heading_and_the_close_label_are_the_views_own", async () => {
    // Specification 25 §5.14 and design D5b R5: the `Found on the network` heading (`AddDialog.tsx:124`) and
    // the close button's name come from the view. The copy below changes both and adds one find, so a
    // component that keeps its own literal shows the wrong words.
    const copy = JSON.parse(JSON.stringify(addOf("search_and_filter")));
    copy.found_heading = "Seen on the line";
    copy.close_label = "Dismiss";
    copy.found = [
      {
        card: "card:plc-1",
        title: "Line PLC",
        rows: [],
        action: {
          id: "add",
          label: "Add",
          enabled: true,
          emphasis: "primary",
          op: { kind: "rust", action: "runtime.adopt", payload: {} },
        },
      },
    ];
    const rendered = await renderDialog(copy);
    try {
      assert.strictEqual(
        rendered.container.querySelector('[data-role="add-found-heading"]').textContent,
        "Seen on the line",
        "the heading is the view's own text",
      );
      assert.strictEqual(
        rendered.container.querySelector('[data-role="add-close"]').getAttribute("aria-label"),
        "Dismiss",
        "the close button's name is the view's own text",
      );
    } finally {
      await rendered.unmount();
    }
  });
});
