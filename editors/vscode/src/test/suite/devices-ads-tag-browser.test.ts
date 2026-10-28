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

// The ADS tag browser and the login form of `Create route` are drawn from the Rust view model (design
// §4.5 "D5: browse panes" and D5b; specification 25 §5.4 and its lines 387-394 and 878-883). The expected
// values below are the literal texts of the golden views that
// `crates/trust-devices/src/panes/tests/golden_browse.rs` writes, and of the devices view fixtures; they
// are never read back from the component.

/** A webview component under test, bundled the way the extension ships it. */
type WebviewComponent = React.ComponentType<Record<string, unknown>>;

/** Bundles one webview entry point and returns its named export, as the extension ships it. */
function loadComponent(component: "AdsTagBrowser" | "FormView" | "App"): WebviewComponent {
  const extensionRoot = path.resolve(__dirname, "..", "..", "..");
  const temp = fs.mkdtempSync(
    path.join(extensionRoot, "node_modules", `.trust-devices-ads-${component}-`),
  );
  const outfile = path.join(temp, `devices-ads-${component}.cjs`);
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

// The golden browse views and the login form, written by the Rust tests and never by hand.
const browseFixtures = path.resolve(
  path.resolve(__dirname, "..", "..", ".."),
  "..",
  "..",
  "crates",
  "trust-devices",
  "tests",
  "fixtures",
  "devices_browse",
);

// The devices view the App is drawn from, written by the Rust view test.
const viewFixtures = path.resolve(
  path.resolve(__dirname, "..", "..", ".."),
  "..",
  "..",
  "crates",
  "trust-devices",
  "tests",
  "fixtures",
  "devices_view",
);

/** The golden browse fixture `name`, as Rust serialized it. */
function golden(name: string): any {
  return JSON.parse(fs.readFileSync(path.join(browseFixtures, `${name}.json`), "utf8"));
}

/** The golden devices view `name`, as Rust serialized it. */
function devicesView(name: string): any {
  return JSON.parse(fs.readFileSync(path.join(viewFixtures, `${name}.json`), "utf8"));
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
    url: "https://devices-ads-tag-browser.test/",
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

/** Types into a text input the way a user does, past React's value tracker. */
async function type(rendered: Rendered, input: any, value: string): Promise<void> {
  const setter = Object.getOwnPropertyDescriptor(
    rendered.window.HTMLInputElement.prototype,
    "value",
  )?.set;
  assert.ok(setter, "HTMLInputElement.value has a setter");
  await act(async () => {
    setter.call(input, value);
    input.dispatchEvent(new rendered.window.Event("input", { bubbles: true }));
  });
}

/** The port card that carries `port`. */
function portCard(container: any, port: number): any {
  const card = container.querySelector(`[data-role="ads-port"][data-port="${port}"]`);
  assert.ok(card, `the card of port ${port} is drawn`);
  return card;
}

/** The action button of `container` that draws `label`. */
function actionButton(container: any, label: string): any {
  const button = [...container.querySelectorAll('[data-role="action-button"]')].find(
    (candidate: any) => candidate.textContent === label,
  );
  assert.ok(button, `an action button ${label} is drawn`);
  return button;
}

/** The tag row of `scope` that draws `name`. */
function tagRow(scope: any, name: string): any {
  const row = [...scope.querySelectorAll('[data-role="browse-node"]')].find(
    (candidate: any) => candidate.querySelector('[data-role="browse-name"]').textContent === name,
  );
  assert.ok(row, `a tag named ${name} is drawn`);
  return row;
}

/** The names of the tags a scope draws, in the order they are drawn. */
function tagNames(scope: any): string[] {
  return [...scope.querySelectorAll('[data-role="browse-name"]')].map(
    (name: any) => name.textContent,
  );
}

suite("Devices & Connections ADS tag browser", () => {
  test("three_port_cards_draw_their_title_state_and_tags", async () => {
    const AdsTagBrowser = loadComponent("AdsTagBrowser");
    // ads_three_ports: port 301 has no route, port 851 is browsed, port 852 answered with an error. A wrong
    // implementation draws the cards out of port order, the wrong state count or the tags out of name order.
    const rendered = await render(AdsTagBrowser, {
      browse: golden("ads_three_ports"),
      onAction: () => undefined,
      onClose: () => undefined,
    });
    try {
      const container = rendered.container;
      assert.deepStrictEqual(
        [...container.querySelectorAll('[data-role="ads-port-title"]')].map(
          (title: any) => title.textContent,
        ),
        ["ADS port 301", "ADS port 851", "ADS port 852"],
        "one card per port, in port order",
      );
      assert.deepStrictEqual(
        [...container.querySelectorAll('[data-role="ads-port-state"]')].map(
          (state: any) => state.textContent,
        ),
        ["0 tags", "2 tags", "0 tags"],
        "each card says how many tags it holds",
      );
      const browsed = portCard(container, 851);
      assert.deepStrictEqual(tagNames(browsed), ["Counter", "Speed"], "the tags in name order");
      assert.strictEqual(
        tagRow(browsed, "Counter").querySelector('[data-role="browse-access"]').textContent,
        "read/write",
        "a writable tag says so",
      );
      assert.strictEqual(
        tagRow(browsed, "Speed").querySelector('[data-role="browse-access"]').textContent,
        "read-only",
        "a read-only tag says so",
      );
    } finally {
      await rendered.unmount();
    }
  });

  test("a_tag_the_project_has_is_marked_and_ticked_from_the_start", async () => {
    const AdsTagBrowser = loadComponent("AdsTagBrowser");
    // MAIN.Counter is already imported on port 851: its box shows the project's state, checked, with the
    // view's Added word. A wrong implementation draws it unchecked, so ticking it would import it again.
    const rendered = await render(AdsTagBrowser, {
      browse: golden("ads_three_ports"),
      onAction: () => undefined,
      onClose: () => undefined,
    });
    try {
      const browsed = portCard(rendered.container, 851);
      const counter = tagRow(browsed, "Counter");
      assert.strictEqual(
        counter.querySelector('[data-role="browse-added"]').textContent,
        "Added",
        "Counter carries the view's Added word",
      );
      assert.strictEqual(
        counter.querySelector('[data-role="browse-tick"]').checked,
        true,
        "an imported tag is drawn ticked",
      );
      const speed = tagRow(browsed, "Speed");
      assert.strictEqual(speed.querySelector('[data-role="browse-added"]'), null);
      assert.strictEqual(speed.querySelector('[data-role="browse-tick"]').checked, false);
    } finally {
      await rendered.unmount();
    }
  });

  test("a_missing_route_draws_its_help_its_copy_files_and_create_route", async () => {
    const AdsTagBrowser = loadComponent("AdsTagBrowser");
    // Specification 25 §5.4 and design N25: the route help is drawn with its generated files and `Create
    // route`. A wrong implementation drops a line, a file, or leaves `Create route` disabled.
    const view = golden("ads_three_ports");
    const rendered = await render(AdsTagBrowser, {
      browse: view,
      onAction: () => undefined,
      onClose: () => undefined,
    });
    try {
      const card = portCard(rendered.container, 301);
      assert.strictEqual(
        card.querySelector('[data-role="ads-route-text"]').textContent,
        "An ADS route is required before tags can be read from port 301.",
        "the route help's text is Rust's",
      );
      assert.deepStrictEqual(
        [...card.querySelectorAll('[data-role="ads-route-line"]')].map((line: any) => line.textContent),
        [
          "Route needs TwinCAT administrator access. Run the generated route script on the TwinCAT computer, then reopen Browse.",
        ],
        "the help line is Rust's",
      );
      assert.strictEqual(actionButton(card, "Copy Download PowerShell").disabled, false);
      assert.strictEqual(actionButton(card, "Create route").disabled, false);
    } finally {
      await rendered.unmount();
    }
  });

  test("create_route_posts_its_form_open_action_unchanged_once", async () => {
    const AdsTagBrowser = loadComponent("AdsTagBrowser");
    // `Create route` is `form.open` with the route's pane, runtime, name and plan (N26), and no values. A
    // wrong implementation posts `ads.route_add` from the pane, asking for no login, or adds values.
    const view = golden("ads_three_ports");
    const posted: unknown[][] = [];
    const rendered = await render(AdsTagBrowser, {
      browse: view,
      onAction: (...args: unknown[]) => posted.push(args),
      onClose: () => undefined,
    });
    try {
      await click(rendered, actionButton(portCard(rendered.container, 301), "Create route"));
      assert.deepStrictEqual(
        posted,
        [[view.ports[0].route.create_route]],
        "Create route is posted exactly as Rust gave it, once, alone",
      );
    } finally {
      await rendered.unmount();
    }
  });

  test("a_copy_of_a_route_file_posts_its_host_effect_unchanged", async () => {
    const AdsTagBrowser = loadComponent("AdsTagBrowser");
    // The copy button carries a host effect that VS Code runs with the file's text (design §2.1).
    const view = golden("ads_three_ports");
    const posted: unknown[][] = [];
    const rendered = await render(AdsTagBrowser, {
      browse: view,
      onAction: (...args: unknown[]) => posted.push(args),
      onClose: () => undefined,
    });
    try {
      await click(rendered, actionButton(portCard(rendered.container, 301), "Copy Download PowerShell"));
      assert.deepStrictEqual(
        posted,
        [[view.ports[0].route.artifacts[0]]],
        "the copy action is posted exactly as Rust gave it",
      );
    } finally {
      await rendered.unmount();
    }
  });

  test("a_failed_port_draws_its_title_and_detail_and_offers_no_recovery", async () => {
    const AdsTagBrowser = loadComponent("AdsTagBrowser");
    // Design J55: a port's own error is drawn on its card only, with no recovery Rust did not give. A wrong
    // implementation shows it for the whole pane, or invents a button.
    const rendered = await render(AdsTagBrowser, {
      browse: golden("ads_three_ports"),
      onAction: () => undefined,
      onClose: () => undefined,
    });
    try {
      const card = portCard(rendered.container, 852);
      assert.strictEqual(
        card.querySelector('[data-role="browse-error-title"]').textContent,
        "ADS port unavailable",
        "the card's error has Rust's title",
      );
      assert.strictEqual(
        card.querySelector('[data-role="browse-error-detail"]').textContent,
        "The selected ADS server did not answer. Check the port and that this server is running, then browse again.",
        "the card's error has Rust's detail",
      );
      assert.strictEqual(
        card.querySelector('[data-role="browse-error"] [data-role="action-button"]'),
        null,
        "an error with no recovery draws no button",
      );
      assert.strictEqual(
        rendered.container.querySelectorAll('[data-role="browse-error"]').length,
        1,
        "the port's error is drawn once, on its own card",
      );
    } finally {
      await rendered.unmount();
    }
  });

  test("port_suggestions_and_their_help_draw_once_the_advanced_toggle_opens", async () => {
    const AdsTagBrowser = loadComponent("AdsTagBrowser");
    // Specification 25 §5.4 and its lines 880-883: the suggested ports, each with its name, and their
    // help, shown under the toggle, whose two texts come from the view. A wrong implementation shows them
    // closed, or with the words of the component.
    const rendered = await render(AdsTagBrowser, {
      browse: golden("ads_three_ports"),
      onAction: () => undefined,
      onClose: () => undefined,
    });
    try {
      const container = rendered.container;
      const toggle = () => container.querySelector('[data-role="ads-advanced-toggle"]');
      assert.strictEqual(toggle().textContent, "Advanced: browse another ADS port", "closed, it offers to open");
      assert.strictEqual(container.querySelector('[data-role="ads-port-suggestions"]'), null, "closed, it draws no suggestion");
      await click(rendered, toggle());
      assert.strictEqual(toggle().textContent, "Hide advanced", "open, it offers to hide");
      assert.deepStrictEqual(
        [...container.querySelectorAll('[data-role="ads-port-suggestion"]')].map((s: any) => s.textContent),
        ["301 I/O", "501 Motion", "851 PLC runtime 1", "852 PLC runtime 2"],
        "each suggestion is drawn with its port and its name",
      );
      assert.strictEqual(
        container.querySelector('[data-role="ads-port-help"]').textContent,
        "Each ADS port is a separate server and symbol namespace. The server must support Symbol Upload.",
      );
    } finally {
      await rendered.unmount();
    }
  });

  test("a_port_not_browsed_yet_posts_its_browse_action_unchanged", async () => {
    const AdsTagBrowser = loadComponent("AdsTagBrowser");
    // ads_port_to_browse: port 852 was reported but not browsed, so its card offers `Browse port 852`, whose
    // action reopens the pane at that port (specification 25 §5.4). A wrong implementation posts it with
    // values or rebuilds it.
    const view = golden("ads_port_to_browse");
    const posted: unknown[][] = [];
    const rendered = await render(AdsTagBrowser, {
      browse: view,
      onAction: (...args: unknown[]) => posted.push(args),
      onClose: () => undefined,
    });
    try {
      await click(rendered, actionButton(portCard(rendered.container, 852), "Browse port 852"));
      assert.deepStrictEqual(posted, [[view.ports[1].browse]], "the browse action is posted as Rust gave it");
    } finally {
      await rendered.unmount();
    }
  });

  test("a_loading_browse_says_it_loads_and_offers_no_add", async () => {
    const AdsTagBrowser = loadComponent("AdsTagBrowser");
    // ads_loading: the browse of port 851 still runs (design §1.5). A wrong implementation draws an empty
    // pane that looks finished, or offers an Add over a browse that has no answer.
    const rendered = await render(AdsTagBrowser, {
      browse: golden("ads_loading"),
      onAction: () => undefined,
      onClose: () => undefined,
    });
    try {
      const container = rendered.container;
      assert.strictEqual(
        container.querySelector('[data-role="browse-view"]').getAttribute("data-loading"),
        "true",
        "the pane says the browse still runs",
      );
      assert.ok(container.querySelector('[data-role="browse-loading"]'), "a loading mark is drawn");
      const card = portCard(container, 851);
      assert.strictEqual(card.querySelector('[data-role="ads-port-state"]').textContent, "Loading…");
      assert.strictEqual(
        card.querySelector('[data-role="ads-port-empty"]').textContent,
        "Loading tags…",
        "the card's body says its tags are loading, in Rust's words",
      );
      assert.strictEqual(
        container.querySelector('[data-role="browse-add"]'),
        null,
        "an ADS pane has no Add button: its tags are ticked one by one",
      );
    } finally {
      await rendered.unmount();
    }
  });

  test("a_tick_posts_the_tags_own_action_with_the_write_toggle", async () => {
    const AdsTagBrowser = loadComponent("AdsTagBrowser");
    // Design D5b R2: ticking an unadded tag posts its `ticked` action, and unticking an added tag posts its
    // `unticked` action; both are posted unchanged, with `writable` from the toggle as the values. A wrong
    // implementation posts one action for both, or leaves the toggle out of the values.
    const view = golden("ads_three_ports");
    const posted: unknown[][] = [];
    const rendered = await render(AdsTagBrowser, {
      browse: view,
      onAction: (...args: unknown[]) => posted.push(args),
      onClose: () => undefined,
    });
    try {
      const container = rendered.container;
      const tick = (name: string) => tagRow(portCard(container, 851), name).querySelector('[data-role="browse-tick"]');
      await click(rendered, tick("Speed"));
      await click(rendered, container.querySelector('[data-role="browse-write-toggle"]'));
      await click(rendered, tick("Counter"));
      assert.deepStrictEqual(
        posted,
        [
          [view.ports[1].tags[1].action, { writable: false }],
          [view.ports[1].tags[0].action, { writable: true }],
        ],
        "ticking posts the tag's own action, with the toggle as the values",
      );
    } finally {
      await rendered.unmount();
    }
  });

  test("the_typed_port_posts_its_value_with_its_browse_action", async () => {
    const AdsTagBrowser = loadComponent("AdsTagBrowser");
    // Design D5b R3: `Browse this port` posts Rust's `browse.port` action with the typed text as its `port`
    // value, and nothing is checked here: Rust reads the text. A wrong implementation parses the text in
    // the webview, or posts the action without the value.
    const view = golden("ads_three_ports");
    const posted: unknown[][] = [];
    const rendered = await render(AdsTagBrowser, {
      browse: view,
      onAction: (...args: unknown[]) => posted.push(args),
      onClose: () => undefined,
    });
    try {
      const container = rendered.container;
      await click(rendered, container.querySelector('[data-role="ads-advanced-toggle"]'));
      await type(rendered, container.querySelector('[data-role="ads-port-input"]'), "3x1");
      await click(rendered, container.querySelector('[data-role="ads-browse-port"]'));
      assert.deepStrictEqual(
        posted,
        [[view.port_input.browse, { port: "3x1" }]],
        "the typed text is the value, as typed, and the action is Rust's own",
      );
      assert.strictEqual(
        container.querySelector('[data-role="ads-port-input"]').placeholder,
        "851",
        "the field's placeholder is Rust's",
      );
    } finally {
      await rendered.unmount();
    }
  });

  test("a_typed_port_refused_by_rust_shows_its_message_in_the_pane", async () => {
    const AdsTagBrowser = loadComponent("AdsTagBrowser");
    // The message is Rust's (`texts/browse.rs`, ENTER_WHOLE_PORT), drawn in the pane that started the action.
    // A wrong implementation draws it nowhere, or draws it in another pane.
    const view = golden("ads_three_ports");
    const notice = (pane: string) => ({
      pane,
      tone: "error",
      text: "Enter a whole-number ADS port from 1 to 65535.",
      field_errors: [],
      actions: [],
    });
    const rendered = await render(AdsTagBrowser, {
      browse: view,
      notice: notice("browse:ads"),
      onAction: () => undefined,
      onClose: () => undefined,
    });
    try {
      assert.strictEqual(
        rendered.container.querySelector('[data-role="browse-notice-text"]').textContent,
        "Enter a whole-number ADS port from 1 to 65535.",
      );
      await rendered.update({ browse: view, notice: notice("browse:opcua_client"), onAction: () => undefined, onClose: () => undefined });
      assert.strictEqual(
        rendered.container.querySelector('[data-role="browse-notice"]'),
        null,
        "a notice of another pane is not drawn here",
      );
    } finally {
      await rendered.unmount();
    }
  });

  test("search_narrows_every_port_by_name_and_by_path_and_says_when_a_port_has_no_match", async () => {
    const AdsTagBrowser = loadComponent("AdsTagBrowser");
    // Presentation over Rust's lists (D5b R4): a case-insensitive match of a tag's name or path on every
    // port, with the view's own no-match text on each port without a match. A wrong implementation matches
    // only names, or lists the ports' other tags.
    const rendered = await render(AdsTagBrowser, {
      browse: golden("ads_three_ports"),
      onAction: () => undefined,
      onClose: () => undefined,
    });
    try {
      const container = rendered.container;
      const search = container.querySelector('[data-role="browse-search"]');
      assert.strictEqual(search.placeholder, "Search tags on all ADS ports", "the search box is Rust's");
      await type(rendered, search, "MAIN.sp");
      assert.deepStrictEqual(tagNames(portCard(container, 851)), ["Speed"], "a path matches");
      assert.deepStrictEqual(
        [...container.querySelectorAll('[data-role="ads-port-no-match"]')].map((text: any) => text.textContent),
        ["No matching tags on this port.", "No matching tags on this port."],
        "the ports without a match say so, in Rust's words",
      );
      await type(rendered, search, "COUNTER");
      assert.deepStrictEqual(tagNames(portCard(container, 851)), ["Counter"], "a name matches case-insensitively");
    } finally {
      await rendered.unmount();
    }
  });

  test("the_connection_details_and_the_port_summary_come_from_the_view", async () => {
    const AdsTagBrowser = loadComponent("AdsTagBrowser");
    // AdsMultiPortTagBrowser.tsx:247-254: the summary of the ports and the connection's details, under a
    // disclosure, in Rust's words. A wrong implementation draws its own labels or drops a row.
    const rendered = await render(AdsTagBrowser, {
      browse: golden("ads_three_ports"),
      onAction: () => undefined,
      onClose: () => undefined,
    });
    try {
      const container = rendered.container;
      assert.strictEqual(
        container.querySelector('[data-role="ads-port-summary"]').textContent,
        "3 ADS ports",
      );
      assert.strictEqual(
        container.querySelector('[data-role="browse-details-summary"]').textContent,
        "Connection details",
      );
      assert.deepStrictEqual(
        [...container.querySelectorAll('[data-role="browse-detail-label"]')].map((label: any) => label.textContent),
        ["AMS Net ID", "Host"],
      );
      assert.deepStrictEqual(
        [...container.querySelectorAll('[data-role="browse-detail-value"]')].map((value: any) => value.textContent),
        ["5.23.91.12.1.1", "192.168.10.5"],
      );
    } finally {
      await rendered.unmount();
    }
  });

  test("done_closes_the_pane_with_the_views_own_label", async () => {
    const AdsTagBrowser = loadComponent("AdsTagBrowser");
    // The old pane's footer button closed it (AdsMultiPortTagBrowser.tsx:378). A wrong implementation draws
    // no Done, or closes on another click.
    const closes: number[] = [];
    const rendered = await render(AdsTagBrowser, {
      browse: golden("ads_three_ports"),
      onAction: () => undefined,
      onClose: () => closes.push(1),
    });
    try {
      const done = rendered.container.querySelector('[data-role="ads-done"]');
      assert.strictEqual(done.textContent, "Done");
      await click(rendered, done);
      assert.deepStrictEqual(closes, [1], "Done closes the pane once");
    } finally {
      await rendered.unmount();
    }
  });

  test("the_login_form_draws_a_user_name_field_and_a_password_field", async () => {
    const FormView = loadComponent("FormView");
    // route_login_form: one administrator user name and one password, asked once (specification 25 §5.4).
    // A wrong implementation draws the password in clear text, or asks for it in a text field.
    const view = golden("route_login_form");
    const rendered = await render(FormView, { form: view, onValues: () => undefined, onAction: () => undefined });
    try {
      const container = rendered.container;
      assert.strictEqual(container.querySelector('[data-role="form-title"]').textContent, "Create route");
      assert.strictEqual(
        container.querySelector('[data-role="form-note"]').textContent,
        "The login is used only to create this route. truST never stores or shows it again.",
      );
      const user = container.querySelector('[data-role="field-input"][data-field="username"]');
      assert.strictEqual(user.type, "text", "the user name is a text field");
      const password = container.querySelector('[data-role="field-input"][data-field="password"]');
      assert.strictEqual(password.type, "password", "the password is a password field");
      assert.strictEqual(password.value, "", "the password starts empty");
      assert.deepStrictEqual(
        [...container.querySelectorAll('[data-role="field-label"]')].map((label: any) => label.textContent),
        ["TwinCAT administrator user name", "TwinCAT administrator password"],
      );
    } finally {
      await rendered.unmount();
    }
  });

  test("create_route_posts_ads_route_add_once_with_the_typed_login", async () => {
    const FormView = loadComponent("FormView");
    // Specification 25 §5.4: the login goes once, with its own Rust action, and nowhere else. A wrong
    // implementation posts twice, posts without the values, or posts another action.
    const view = golden("route_login_form");
    const calls: unknown[][] = [];
    const rendered = await render(FormView, {
      form: view,
      onValues: () => undefined,
      onAction: (action: unknown, values: unknown) => calls.push([action, values]),
    });
    try {
      const container = rendered.container;
      await type(rendered, container.querySelector('[data-role="field-input"][data-field="username"]'), "operator");
      await type(rendered, container.querySelector('[data-role="field-input"][data-field="password"]'), "s3cret-Passw0rd");
      await click(rendered, actionButton(container, "Create route"));
      assert.deepStrictEqual(
        calls,
        [[view.buttons[0], { username: "operator", password: "s3cret-Passw0rd" }]],
        "Create route posts its own action once, with the two typed values",
      );
    } finally {
      await rendered.unmount();
    }
  });

  test("the_login_is_empty_after_the_post", async () => {
    const FormView = loadComponent("FormView");
    // A route login's user name and password are typed for one request, so both are emptied after the post
    // (design D5c R12). A wrong implementation keeps them, so a second click would post them again.
    const rendered = await render(FormView, {
      form: golden("route_login_form"),
      onValues: () => undefined,
      onAction: () => undefined,
    });
    try {
      const container = rendered.container;
      await type(rendered, container.querySelector('[data-role="field-input"][data-field="username"]'), "operator");
      await type(rendered, container.querySelector('[data-role="field-input"][data-field="password"]'), "s3cret-Passw0rd");
      await click(rendered, actionButton(container, "Create route"));
      assert.strictEqual(
        container.querySelector('[data-role="field-input"][data-field="password"]').value,
        "",
        "the password is empty after it was posted",
      );
      assert.strictEqual(
        container.querySelector('[data-role="field-input"][data-field="username"]').value,
        "",
        "the user name is empty after it was posted",
      );
    } finally {
      await rendered.unmount();
    }
  });

  test("the_app_posts_nothing_when_cancel_is_chosen", async () => {
    const App = loadComponent("App");
    // Design D5b R7: Cancel is a `ui` op that the shell carries out: it closes the form and posts nothing to
    // the host. A wrong implementation posts `close_inspector`, which the host would then have to ignore.
    const view = devicesView("search_and_filter");
    const posted: unknown[] = [];
    const reply = {
      ok: true,
      form: golden("route_login_form"),
      effects: [],
      close_pane: false,
      refresh: "none",
    };
    const rendered = await render(App, { view, post: (message: unknown) => posted.push(message), reply });
    try {
      const container = rendered.container;
      assert.ok(container.querySelector('[data-role="form-pane"]'), "the login form is drawn");
      await click(rendered, actionButton(container, "Cancel"));
      assert.strictEqual(container.querySelector('[data-role="form-pane"]'), null, "Cancel closes the form");
      assert.deepStrictEqual(posted, [], "Cancel posts nothing to the host");
    } finally {
      await rendered.unmount();
    }
  });

  test("the_app_posts_one_message_with_the_login_when_create_route_is_chosen", async () => {
    const App = loadComponent("App");
    // The same login from the App: one message, with the route's action and the two values, and no other
    // message. A wrong implementation sends the login with a second message, or without its values.
    const view = devicesView("search_and_filter");
    const form = golden("route_login_form");
    const posted: unknown[] = [];
    const reply = { ok: true, form, effects: [], close_pane: false, refresh: "none" };
    const rendered = await render(App, { view, post: (message: unknown) => posted.push(message), reply });
    try {
      const container = rendered.container;
      await type(rendered, container.querySelector('[data-role="field-input"][data-field="username"]'), "operator");
      await type(rendered, container.querySelector('[data-role="field-input"][data-field="password"]'), "s3cret-Passw0rd");
      await click(rendered, actionButton(container, "Create route"));
      assert.deepStrictEqual(
        posted,
        [{ type: "action", action: form.buttons[0], values: { username: "operator", password: "s3cret-Passw0rd" } }],
        "one message: the route's action with the two values",
      );
    } finally {
      await rendered.unmount();
    }
  });
});
