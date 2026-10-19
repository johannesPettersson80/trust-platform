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

// The browse pane of OPC UA nodes, EtherCAT channels and the globals a server exposes is drawn from the
// Rust view model (design §4.5 "D5: browse panes" and D5b, specification 25 §5.4 and §11.2). The expected
// values below are the literal texts of the golden browse views that
// `crates/trust-devices/src/panes/tests/golden_browse.rs` writes; they are never read back from the
// component.

/** A webview component under test, bundled the way the extension ships it. */
type WebviewComponent = React.ComponentType<Record<string, unknown>>;

/** Bundles the browse pane entry point and returns `BrowsePane`, as the extension ships it. */
function loadBrowsePane(): WebviewComponent {
  const extensionRoot = path.resolve(__dirname, "..", "..", "..");
  const temp = fs.mkdtempSync(
    path.join(extensionRoot, "node_modules", ".trust-devices-browse-pane-"),
  );
  const outfile = path.join(temp, "devices-browse-pane.cjs");
  try {
    buildSync({
      entryPoints: [
        path.join(extensionRoot, "src", "devicesPanel", "webview", "BrowsePane.tsx"),
      ],
      outfile,
      bundle: true,
      platform: "node",
      format: "cjs",
      external: ["react", "react-dom", "vscode"],
      logLevel: "silent",
    });
    return require(outfile).BrowsePane as WebviewComponent;
  } finally {
    fs.rmSync(temp, { recursive: true, force: true });
  }
}

// The golden browse views, written by the Rust test and never by hand.
const fixtures = path.resolve(
  path.resolve(__dirname, "..", "..", ".."),
  "..",
  "..",
  "crates",
  "trust-devices",
  "tests",
  "fixtures",
  "devices_browse",
);

/** The golden browse view `name`, as Rust serialized it. */
function goldenView(name: string): any {
  return JSON.parse(fs.readFileSync(path.join(fixtures, `${name}.json`), "utf8"));
}

// JSDOM's elements stay untyped, as in devices-forms-render.test.ts: the extension's tsconfig has no
// DOM lib, so an `HTMLElement` here would not name a type.
interface Rendered {
  container: any;
  window: any;
  update(props: Record<string, unknown>): Promise<void>;
  unmount(): Promise<void>;
}

async function render(
  component: WebviewComponent,
  props: Record<string, unknown>,
): Promise<Rendered> {
  const dom = new JSDOM('<!doctype html><body><div id="root"></div></body>', {
    url: "https://devices-browse-pane.test/",
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
    async update(next: Record<string, unknown>) {
      await act(async () => {
        root.render(React.createElement(component, next));
      });
    },
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

/** Clicks an element the way a user does. */
async function click(rendered: Rendered, element: any): Promise<void> {
  assert.ok(element, "the element to click is drawn");
  await act(async () => {
    element.click();
  });
}

/** Opens every group that is closed, until none is left (the groups open on a click). */
async function openAllGroups(rendered: Rendered): Promise<void> {
  for (;;) {
    const closed = rendered.container.querySelector(
      '[data-role="browse-group-toggle"][aria-expanded="false"]',
    );
    if (closed === null) {
      return;
    }
    await click(rendered, closed);
  }
}

/** The row of a tree that draws `name`. */
function rowNamed(container: any, name: string): any {
  const row = [...container.querySelectorAll('[data-role="browse-node"]')].find(
    (candidate: any) =>
      candidate.querySelector('[data-role="browse-name"]').textContent === name,
  );
  assert.ok(row, `a row named ${name} is drawn`);
  return row;
}

/** The group that draws `name`: a `browse-group` whose toggle holds the name. Groups are not rows. */
function groupNamed(container: any, name: string): any {
  const group = [...container.querySelectorAll('[data-role="browse-group"]')].find(
    (candidate: any) =>
      candidate.querySelector('[data-role="browse-group-toggle"] [data-role="browse-name"]')
        .textContent === name,
  );
  assert.ok(group, `a group named ${name} is drawn`);
  return group;
}

/** The text of an optional element inside a row, or `undefined` when the row draws none. */
function textIn(row: any, role: string): string | undefined {
  const element = row.querySelector(`[data-role="${role}"]`);
  return element === null ? undefined : element.textContent;
}

/** The names the pane draws, in the order they are drawn. */
function drawnNames(container: any): string[] {
  return [...container.querySelectorAll('[data-role="browse-name"]')].map(
    (name: any) => name.textContent,
  );
}

/** The `Add` button and its reason, as the pane draws them. */
function addOf(container: any): { label: string; disabled: boolean; reason: string | undefined } {
  const button = container.querySelector('[data-role="browse-add"]');
  const reason = container.querySelector('[data-role="browse-add-reason"]');
  return {
    label: button.textContent,
    disabled: button.disabled,
    reason: reason === null ? undefined : reason.textContent,
  };
}

/** The action button that draws `label`. */
function actionButton(container: any, label: string): any {
  const button = [...container.querySelectorAll('[data-role="action-button"]')].find(
    (candidate: any) => candidate.textContent === label,
  );
  assert.ok(button, `an action button ${label} is drawn`);
  return button;
}

suite("Devices & Connections browse pane", () => {
  test("a_tree_draws_each_name_path_type_access_and_added_mark_in_order", async () => {
    const BrowsePane = loadBrowsePane();
    // opcua_nodes: a folder of two points and a folder of two alarms (specification 25 §11.2). Groups
    // start closed, as the old panel's did. A wrong order, a group drawn with an access word, a point
    // without its Added mark or a mark word that is not the view's own breaks it.
    const view = goldenView("opcua_nodes");
    const rendered = await render(BrowsePane, {
      browse: view,
      onAction: () => undefined,
      onClose: () => undefined,
    });
    try {
      const container = rendered.container;
      assert.strictEqual(
        container.querySelector('[data-role="browse-title"]').textContent,
        "Browse OPC UA nodes",
        "the title is drawn as Rust gives it",
      );
      assert.strictEqual(
        container.querySelector('[data-role="browse-breadcrumb"]').textContent,
        "Devices & Connections",
        "the breadcrumb is drawn as Rust gives it",
      );
      assert.deepStrictEqual(drawnNames(container), ["Line"], "groups start closed");

      await openAllGroups(rendered);
      assert.deepStrictEqual(
        drawnNames(container),
        ["Line", "Speed", "Count", "Alarms", "Fault", "Anonymous"],
        "every node is drawn, depth first, in the order Rust gives",
      );
      const speed = rowNamed(container, "Speed");
      assert.strictEqual(textIn(speed, "browse-path"), "Line.Speed", "a leaf draws its path");
      assert.strictEqual(textIn(speed, "browse-type"), "Double", "a leaf draws its type");
      assert.strictEqual(textIn(speed, "browse-access"), "read/write", "a writable leaf says so");
      assert.strictEqual(
        textIn(speed, "browse-added"),
        "Added",
        "a point the connection has carries the view's own Added word",
      );
      const count = rowNamed(container, "Count");
      assert.strictEqual(textIn(count, "browse-access"), "read-only", "a read-only leaf says so");
      assert.strictEqual(
        count.querySelector('[data-role="browse-added"]'),
        null,
        "a point the connection does not have carries no Added mark",
      );
      const line = groupNamed(container, "Line");
      assert.strictEqual(
        textIn(line.querySelector('[data-role="browse-group-toggle"]'), "browse-access"),
        undefined,
        "a group carries no access word",
      );
    } finally {
      await rendered.unmount();
    }
  });

  test("a_leaf_without_a_raw_node_id_cannot_be_ticked", async () => {
    const BrowsePane = loadBrowsePane();
    // Specification 25 §11.2: a leaf without a raw NodeId is not selectable. A wrong implementation
    // draws a tick box for it, so Add could post a node that comm.apply cannot identify. The tick boxes
    // are named by the view's own words (`Select Line.Speed`).
    const rendered = await render(BrowsePane, {
      browse: goldenView("opcua_nodes"),
      onAction: () => undefined,
      onClose: () => undefined,
    });
    try {
      await openAllGroups(rendered);
      const container = rendered.container;
      assert.strictEqual(
        container.querySelectorAll('[data-role="browse-tick"]').length,
        3,
        "only the three leaves with a raw NodeId can be ticked",
      );
      assert.strictEqual(
        rowNamed(container, "Anonymous").querySelector('[data-role="browse-tick"]'),
        null,
        "the leaf without a raw NodeId draws no tick box",
      );
      assert.strictEqual(
        rowNamed(container, "Speed").querySelector('[data-role="browse-tick"]').getAttribute("aria-label"),
        "Select Line.Speed",
        "the tick box is named by the view's own text",
      );
    } finally {
      await rendered.unmount();
    }
  });

  test("add_says_nothing_is_ticked_until_a_leaf_is_ticked", async () => {
    const BrowsePane = loadBrowsePane();
    // Design §3.2 J53: Rust's Add is enabled, and the pane says `no_selection_reason` while nothing is
    // ticked. One tick enables it and counts the tick in the view's own counted label; unticking it
    // disables it again. A wrong implementation enables Add with nothing ticked, or keeps the count.
    const rendered = await render(BrowsePane, {
      browse: goldenView("opcua_nodes"),
      onAction: () => undefined,
      onClose: () => undefined,
    });
    try {
      await openAllGroups(rendered);
      const container = rendered.container;
      assert.deepStrictEqual(
        addOf(container),
        { label: "Add nodes", disabled: true, reason: "Select at least one symbol to add." },
        "with nothing ticked, Add is disabled with the view's own reason",
      );
      const tick = () => rowNamed(container, "Speed").querySelector('[data-role="browse-tick"]');
      await click(rendered, tick());
      assert.deepStrictEqual(
        addOf(container),
        { label: "Add nodes (1)", disabled: false, reason: undefined },
        "one tick enables Add and counts it in the view's counted label",
      );
      await click(rendered, tick());
      assert.deepStrictEqual(
        addOf(container),
        { label: "Add nodes", disabled: true, reason: "Select at least one symbol to add." },
        "unticking the only leaf disables Add again",
      );
    } finally {
      await rendered.unmount();
    }
  });

  test("add_posts_browse_add_with_the_ticked_keys_and_the_toggle_once", async () => {
    const BrowsePane = loadBrowsePane();
    // Design D5b R1: Add posts Rust's own `browse.add` action with the ticked raw keys and the write
    // toggle as its values, once per click. A wrong implementation sends the paths in place of the raw
    // NodeIds, drops the toggle, sends a key that was unticked, or posts twice.
    const view = goldenView("opcua_nodes");
    const posted: unknown[][] = [];
    const rendered = await render(BrowsePane, {
      browse: view,
      onAction: (action: unknown, values: unknown) => posted.push([action, values]),
      onClose: () => undefined,
    });
    try {
      await openAllGroups(rendered);
      const container = rendered.container;
      const tick = (name: string) => rowNamed(container, name).querySelector('[data-role="browse-tick"]');
      await click(rendered, tick("Speed"));
      await click(rendered, tick("Count"));
      await click(rendered, tick("Count"));
      await click(rendered, container.querySelector('[data-role="browse-write-toggle"]'));
      await click(rendered, container.querySelector('[data-role="browse-add"]'));
      assert.deepStrictEqual(
        posted,
        [[view.add.action, { keys: ["ns=2;s=Line.Speed"], writable: true }]],
        "Add posts its own action once, with the one ticked raw NodeId and the toggle",
      );
    } finally {
      await rendered.unmount();
    }
  });

  test("a_disabled_add_and_write_toggle_show_the_reason_of_the_failed_browse", async () => {
    const BrowsePane = loadBrowsePane();
    // opcua_untrusted_certificate: the browse failed, so Rust disables Add and the write toggle with the
    // same reason (specification 25 §5.4). A wrong implementation enables Add or drops the reason.
    const rendered = await render(BrowsePane, {
      browse: goldenView("opcua_untrusted_certificate"),
      onAction: () => undefined,
      onClose: () => undefined,
    });
    try {
      const container = rendered.container;
      assert.deepStrictEqual(
        addOf(container),
        { label: "Add nodes", disabled: true, reason: "Resolve the browse error before adding tags." },
        "a failed browse disables Add with Rust's reason",
      );
      const toggle = container.querySelector('[data-role="browse-write-toggle"]');
      assert.strictEqual(toggle.disabled, true, "the write toggle is disabled with Add");
      assert.strictEqual(
        container.querySelector('[data-role="browse-write-label"]').textContent,
        "Allow writes (default: read-only)",
        "the write toggle's label is Rust's",
      );
      assert.strictEqual(
        container.querySelector('[data-role="browse-write-reason"]').textContent,
        "Resolve the browse error before adding tags.",
        "the write toggle says why it cannot be changed",
      );
      assert.strictEqual(
        container.querySelector('[data-role="browse-error-title"]').textContent,
        "Server certificate not trusted",
        "the error box has Rust's title",
      );
      assert.strictEqual(
        container.querySelector('[data-role="browse-error-detail"]').textContent,
        "Confirm this is the expected server, then choose Trust certificate to browse it.",
        "the error box has Rust's detail",
      );
    } finally {
      await rendered.unmount();
    }
  });

  test("the_recovery_of_an_error_box_posts_its_action_unchanged", async () => {
    const BrowsePane = loadBrowsePane();
    // The recovery reopens the pane with the trust flag set (specification 25 §5.4). A ui action carries no
    // values, so the recovery is posted alone. A wrong implementation posts a rebuilt action, or values.
    const view = goldenView("opcua_untrusted_certificate");
    const posted: unknown[][] = [];
    const rendered = await render(BrowsePane, {
      browse: view,
      onAction: (...args: unknown[]) => posted.push(args),
      onClose: () => undefined,
    });
    try {
      await click(rendered, actionButton(rendered.container, "Trust certificate"));
      assert.deepStrictEqual(
        posted,
        [[view.error.recovery]],
        "the recovery is posted exactly as Rust gave it, with no values",
      );
    } finally {
      await rendered.unmount();
    }
  });

  test("a_leaf_draws_its_access_tooltip_and_its_added_label_from_the_view", async () => {
    const BrowsePane = loadBrowsePane();
    // opcua_nodes: Speed is read/write and already in the project; Count is read-only and is not. The tooltip
    // of the access word and the accessible name of the Added mark are the view's templates, filled with the
    // leaf's own access word and path (design D5c R11). A wrong implementation draws neither, or another word.
    const view = goldenView("opcua_nodes");
    const rendered = await render(BrowsePane, {
      browse: view,
      onAction: () => undefined,
      onClose: () => undefined,
    });
    try {
      const container = rendered.container;
      await openAllGroups(rendered);
      const speed = rowNamed(container, "Speed");
      assert.strictEqual(
        speed.querySelector('[data-role="browse-access"]').getAttribute("title"),
        "read/write on the device",
        "the access word's tooltip is the view's template, filled with the access word",
      );
      const added = speed.querySelector('[data-role="browse-added"]');
      assert.strictEqual(added.textContent, "Added", "the Added mark draws the view's word");
      assert.strictEqual(
        added.getAttribute("aria-label"),
        "Already added: Line.Speed",
        "the Added mark is named from the view's template and the leaf's path",
      );
      assert.strictEqual(
        rowNamed(container, "Count").querySelector('[data-role="browse-added"]'),
        null,
        "a leaf the project does not have carries no Added mark",
      );
    } finally {
      await rendered.unmount();
    }
  });

  test("an_ethercat_channel_draws_its_access_and_its_added_mark", async () => {
    const BrowsePane = loadBrowsePane();
    // ethercat_channels: a terminal with two PDO channels, the first already selected. A wrong
    // implementation marks both, or takes the access word from the group.
    const rendered = await render(BrowsePane, {
      browse: goldenView("ethercat_channels"),
      onAction: () => undefined,
      onClose: () => undefined,
    });
    try {
      const container = rendered.container;
      assert.strictEqual(
        container.querySelector('[data-role="browse-title"]').textContent,
        "Browse PDO channels",
        "the channel pane's title is Rust's",
      );
      assert.strictEqual(addOf(container).label, "Add channels", "the channel pane's Add is Rust's");
      await openAllGroups(rendered);
      assert.deepStrictEqual(drawnNames(container), ["EL2008", "Output 1", "Output 2"]);
      const output1 = rowNamed(container, "Output 1");
      assert.strictEqual(textIn(output1, "browse-access"), "read/write");
      assert.strictEqual(textIn(output1, "browse-type"), "BOOL");
      assert.strictEqual(textIn(output1, "browse-added"), "Added", "the selected channel is marked");
      const output2 = rowNamed(container, "Output 2");
      assert.strictEqual(textIn(output2, "browse-access"), "read-only");
      assert.strictEqual(
        output2.querySelector('[data-role="browse-added"]'),
        null,
        "an unselected channel carries no Added mark",
      );
    } finally {
      await rendered.unmount();
    }
  });

  test("the_exposed_globals_mark_the_global_that_is_already_exposed", async () => {
    const BrowsePane = loadBrowsePane();
    // expose_globals: a server exposes `Speed`; `Count` is offered. A wrong implementation marks the globals
    // by their full `global.` path, so none of them reads as exposed.
    const rendered = await render(BrowsePane, {
      browse: goldenView("expose_globals"),
      onAction: () => undefined,
      onClose: () => undefined,
    });
    try {
      const container = rendered.container;
      assert.strictEqual(
        container.querySelector('[data-role="browse-title"]').textContent,
        "Choose globals to expose",
        "the expose pane's title is Rust's",
      );
      assert.strictEqual(addOf(container).label, "Expose selected", "the expose pane's Add is Rust's");
      assert.strictEqual(
        textIn(rowNamed(container, "Speed"), "browse-path"),
        "global.Speed",
        "the path is drawn as given",
      );
      assert.strictEqual(
        textIn(rowNamed(container, "Speed"), "browse-added"),
        "Added",
        "Speed is already exposed",
      );
      assert.strictEqual(
        rowNamed(container, "Count").querySelector('[data-role="browse-added"]'),
        null,
        "Count is not exposed yet",
      );
    } finally {
      await rendered.unmount();
    }
  });

  test("search_filters_leaves_by_name_and_by_path_and_says_when_nothing_matches", async () => {
    const BrowsePane = loadBrowsePane();
    // The old panel's search (`BrowseTagsPanel.tsx:243`, `:105`): a case-insensitive match of a leaf's
    // name or its path, flat, with the view's own no-match text. A wrong implementation matches only the
    // name, keeps the groups, or says nothing when nothing matches.
    const rendered = await render(BrowsePane, {
      browse: goldenView("opcua_nodes"),
      onAction: () => undefined,
      onClose: () => undefined,
    });
    try {
      const container = rendered.container;
      const search = container.querySelector('[data-role="browse-search"]');
      assert.strictEqual(search.placeholder, "Search symbols", "the search box is Rust's");
      const typeSearch = async (text: string) => {
        await act(async () => {
          const setter = Object.getOwnPropertyDescriptor(
            rendered.window.HTMLInputElement.prototype,
            "value",
          )?.set;
          setter?.call(search, text);
          search.dispatchEvent(new rendered.window.Event("input", { bubbles: true }));
        });
      };
      await typeSearch("COUNT");
      assert.deepStrictEqual(drawnNames(container), ["Count"], "a name matches case-insensitively");
      await typeSearch("alarms.");
      assert.deepStrictEqual(
        drawnNames(container),
        ["Fault", "Anonymous"],
        "a path matches, and the matches are flat, without their groups",
      );
      await typeSearch("zzz");
      assert.strictEqual(
        container.querySelector('[data-role="browse-no-match"]').textContent,
        "No matching symbols.",
        "a search with no match says so in the view's words",
      );
    } finally {
      await rendered.unmount();
    }
  });

  test("the_added_word_and_the_pane_texts_are_the_views_own", async () => {
    const BrowsePane = loadBrowsePane();
    // The mark's word, the subtitle and the Add label come from the view, not from the component: a view
    // whose words differ shows its own words. A wrong implementation keeps the word `Added` as a literal.
    const view = goldenView("opcua_nodes");
    view.added_label = "In the project";
    view.add.label = "Add points";
    view.subtitle = "Plant server";
    const rendered = await render(BrowsePane, {
      browse: view,
      onAction: () => undefined,
      onClose: () => undefined,
    });
    try {
      await openAllGroups(rendered);
      const container = rendered.container;
      assert.strictEqual(textIn(rowNamed(container, "Speed"), "browse-added"), "In the project");
      assert.strictEqual(addOf(container).label, "Add points");
      assert.strictEqual(
        container.querySelector('[data-role="browse-subtitle"]').textContent,
        "Plant server",
      );
    } finally {
      await rendered.unmount();
    }
  });

  test("a_result_of_this_pane_is_drawn_in_its_footer", async () => {
    const BrowsePane = loadBrowsePane();
    // A notice names the pane that started its action (specification 25 §5.13). The text is the Rust text
    // of an added OPC UA point (`texts/plans.rs`). A wrong implementation draws it nowhere, or draws it
    // in a pane it does not name.
    const view = goldenView("opcua_nodes");
    const notice = (pane: string) => ({
      pane,
      tone: "ok",
      text: "Added OPC UA client connection with 1 node(s). Restart the runtime to read it.",
      field_errors: [],
      actions: [],
    });
    const rendered = await render(BrowsePane, {
      browse: view,
      notice: notice(view.id),
      onAction: () => undefined,
      onClose: () => undefined,
    });
    try {
      assert.strictEqual(
        rendered.container.querySelector('[data-role="browse-notice-text"]').textContent,
        "Added OPC UA client connection with 1 node(s). Restart the runtime to read it.",
      );
      await rendered.update({ browse: view, notice: notice("browse:ads"), onAction: () => undefined, onClose: () => undefined });
      assert.strictEqual(
        rendered.container.querySelector('[data-role="browse-notice"]'),
        null,
        "a notice of another pane is not drawn here",
      );
    } finally {
      await rendered.unmount();
    }
  });
});
