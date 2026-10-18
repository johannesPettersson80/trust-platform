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

// The Discover pane of Devices & Connections is drawn from the Rust view of one search (design §4.5 "D4:
// Discover pane", specification 25 §2). The expected values below are the texts that specification 25 §2
// names, and the texts of the Rust tables (`texts/discover.rs`, `texts/graph.rs`) that the golden views carry.
// They are never read back from the component. The golden views are written by
// `crates/trust-devices/src/view/tests/golden_discover.rs`.
//
// The EtherCAT confirmation is the Search action's confirm text, and the fieldbus list stays empty. Modbus RTU
// confirmations and compare markers (`Compare with project`) are not in the view yet; C6 adds compare. Their
// tests belong to the later slice that draws them.

/** A webview component under test, bundled the way the extension ships it. */
type WebviewComponent = React.ComponentType<Record<string, unknown>>;

/** The bundled pane, built by the first test that needs it: each build blocks the extension host while it runs. */
let bundledDiscoverPane: WebviewComponent | undefined;

/** Bundles the Discover pane entry point once and returns `DiscoverPane`, as the extension ships it. */
function loadDiscoverPane(): WebviewComponent {
  if (bundledDiscoverPane === undefined) {
    const extensionRoot = path.resolve(__dirname, "..", "..", "..");
    const temp = fs.mkdtempSync(
      path.join(extensionRoot, "node_modules", ".trust-devices-discover-pane-"),
    );
    const outfile = path.join(temp, "devices-discover-pane.cjs");
    try {
      buildSync({
        entryPoints: [
          path.join(extensionRoot, "src", "devicesPanel", "webview", "DiscoverPane.tsx"),
        ],
        outfile,
        bundle: true,
        platform: "node",
        format: "cjs",
        external: ["react", "react-dom", "vscode"],
        logLevel: "silent",
      });
      bundledDiscoverPane = require(outfile).DiscoverPane as WebviewComponent;
    } finally {
      fs.rmSync(temp, { recursive: true, force: true });
    }
  }
  return bundledDiscoverPane;
}

// The golden Discover views, written by the Rust test and never by hand.
const fixtures = path.resolve(
  path.resolve(__dirname, "..", "..", ".."),
  "..",
  "..",
  "crates",
  "trust-devices",
  "tests",
  "fixtures",
  "devices_discover",
);

/** The search operation of a change that the view gives: a status request that records `kind`. */
function idleSearchOp(kind: string, shown = false): any {
  const change = kind === "show_ignored" ? { kind, shown } : { kind };
  return { kind: "search", request: { verb: "status", origin: "this_host", change } };
}

/** The golden Discover view `name`, as Rust serialized it. */
function goldenView(name: string): any {
  return JSON.parse(fs.readFileSync(path.join(fixtures, `${name}.json`), "utf8"));
}

// JSDOM's elements stay untyped, as in devices-browse-render.test.ts: the extension's tsconfig has no DOM lib,
// so an `HTMLElement` here would not name a type.
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
    url: "https://devices-discover-pane.test/",
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
async function click(element: any): Promise<void> {
  assert.ok(element, "the element to click is drawn");
  await act(async () => {
    element.click();
  });
}

/** The text of the element `selector` inside `root`; the test fails when it is not drawn. */
function textOf(root: any, selector: string): string {
  const element = root.querySelector(selector);
  assert.ok(element, `${selector} is drawn`);
  return element.textContent;
}

/** The text of an optional element inside `root`, or `undefined` when it is not drawn. */
function optionalText(root: any, selector: string): string | undefined {
  const element = root.querySelector(selector);
  return element === null ? undefined : element.textContent;
}

/** The action button that draws `label`. */
function actionButton(container: any, label: string): any {
  const button = [...container.querySelectorAll('[data-role="action-button"]')].find(
    (candidate: any) => candidate.textContent === label,
  );
  assert.ok(button, `an action button ${label} is drawn`);
  return button;
}

/** Each source row as the pane draws it; a count or a reason the row does not draw is left out. */
function sourceRows(container: any): Array<Record<string, string>> {
  return [...container.querySelectorAll('[data-role="discover-source"]')].map((row: any) => {
    const count = row.getAttribute("data-count");
    const reason = optionalText(row, '[data-role="discover-source-reason"]');
    return {
      source: row.getAttribute("data-source"),
      label: textOf(row, '[data-role="discover-source-label"]'),
      word: textOf(row, '[data-role="discover-source-word"]'),
      tone: row.getAttribute("data-tone"),
      ...(count === null ? {} : { count }),
      ...(reason === undefined ? {} : { reason }),
    };
  });
}

/** Each `Only search` choice as the pane draws it: its source, whether it is ticked and can be, and its reason. */
function onlyChoices(container: any): Array<Record<string, unknown>> {
  return [...container.querySelectorAll('[data-role="discover-only-choice"]')].map((choice: any) => {
    const reason = optionalText(choice, '[data-role="discover-only-reason"]');
    return {
      source: choice.getAttribute("data-source"),
      checked: choice.querySelector("input").checked,
      enabled: !choice.querySelector("input").disabled,
      ...(reason === undefined ? {} : { reason }),
    };
  });
}

/** Each origin choice as the pane draws it: its identity, its label, whether it is chosen and can be. */
function originChoices(container: any): Array<Record<string, unknown>> {
  return [...container.querySelectorAll('[data-role="discover-origin"]')].map((origin: any) => ({
    id: origin.getAttribute("data-origin"),
    label: origin.querySelector("span").textContent,
    checked: origin.querySelector("input").checked,
    enabled: !origin.querySelector("input").disabled,
  }));
}

/** Each card as the pane draws it: its title, addresses, protocols, identity rows, last sighting and actions. */
function cardsOf(container: any): Array<Record<string, unknown>> {
  return [...container.querySelectorAll('[data-role="discover-card"]')].map((card: any) => ({
    id: card.getAttribute("data-card"),
    title: textOf(card, '[data-role="discover-card-title"]'),
    addresses: [...card.querySelectorAll('[data-role="discover-card-address"]')].map(
      (address: any) => address.textContent,
    ),
    protocols: [...card.querySelectorAll('[data-role="discover-card-protocol"]')].map(
      (protocol: any) => ({
        name: textOf(protocol, '[data-role="discover-card-protocol-name"]'),
        source: textOf(protocol, '[data-role="discover-card-protocol-source"]'),
        confidence: textOf(protocol, '[data-role="discover-card-protocol-confidence"]'),
        simulated: protocol.getAttribute("data-simulated"),
      }),
    ),
    identity: [...card.querySelectorAll('[data-role="discover-card-identity"]')].map(
      (row: any) => ({
        label: textOf(row, '[data-role="discover-card-identity-label"]'),
        value: textOf(row, '[data-role="discover-card-identity-value"]'),
      }),
    ),
    lastSeen: textOf(card, '[data-role="discover-card-last-seen"]'),
    actions: [...card.querySelectorAll('[data-role="action-button"]')].map((button: any) => ({
      label: button.textContent,
      disabled: button.disabled,
    })),
  }));
}

/** Every string in a JSON value, at any depth: the words a view gives. */
function viewStrings(value: unknown, into: Set<string> = new Set()): Set<string> {
  if (typeof value === "string") {
    into.add(value);
  } else if (Array.isArray(value)) {
    value.forEach((item) => viewStrings(item, into));
  } else if (value !== null && typeof value === "object") {
    Object.values(value).forEach((item) => viewStrings(item, into));
  }
  return into;
}

/** The text of every element that holds text itself and no other element, trimmed and not empty. */
function leafTexts(container: any): string[] {
  return [...container.querySelectorAll("*")]
    .filter((element: any) => element.children.length === 0)
    .map((element: any) => element.textContent.trim())
    .filter((text: string) => text !== "");
}

/**
 * The idle Discover view, as Rust gives it before the first search (specification 25 §2): this computer, the first-run
 * tick of ADS and TwinCAT, empty targets and the Search action, with no source row and no card. Every view carries the
 * `Show ignored` toggle, unticked until the user ticks it (decision O14); an idle view names no search in its change.
 */
function idleView(): any {
  return {
    running: false,
    breadcrumb: "Devices & Connections",
    close_label: "Close",
    heading_find: "What do you want to find?",
    heading_scan_from: "Scan from",
    heading_targets: "Known address or subnet",
    heading_advanced: "Advanced scan settings",
    heading_runtime: "Runtime hardware scans ⚠",
    choose_text: "Choose what to find, then scan. Only the selected device types are searched.",
    origins: [{ id: "this_host", label: "This computer", enabled: true, selected: true }],
    only: [
      {
        source: "ads",
        label: "ADS and TwinCAT",
        checked: true,
        enabled: true,
        note: "network broadcast",
        group: "find",
      },
    ],
    targets: {
      hosts_label: "Known address",
      cidr_label: "Subnet",
      hosts_placeholder: "host:port",
      cidr_placeholder: "192.168.1.0/24",
    },
    sources: [],
    cards: [],
    show_ignored: {
      label: "Show ignored",
      checked: false,
      action: {
        id: "show_ignored",
        label: "Show ignored",
        enabled: true,
        emphasis: "quiet",
        op: {
          kind: "search",
          request: { verb: "status", origin: "this_host", change: { kind: "show_ignored", shown: true } },
        },
      },
    },
    actions: [
      {
        id: "search",
        label: "Search",
        enabled: true,
        emphasis: "primary",
        op: { kind: "search", request: { verb: "start", origin: "this_host", params: { only: ["ads"] } } },
      },
      {
        id: "configure_ads_ports",
        label: "Configure ADS ports to scan",
        enabled: true,
        emphasis: "secondary",
        op: { kind: "host", effect: { kind: "open_settings", query: "trust.ads.discoveryPorts" } },
      },
    ],
  };
}

suite("Devices & Connections Discover pane", () => {
  test("a_running_search_draws_its_title_running_mark_and_source_rows_in_their_words", async () => {
    const DiscoverPane = loadDiscoverPane();
    // running_search: truST runtimes and Modbus TCP each found one device, and ADS still searches (specification
    // 25 §2, lines 55-58: `searching` or `{n} found`). A wrong implementation shows a found source as `searching`,
    // gives the searching source a count of its own, or draws no running mark while the search runs.
    const rendered = await render(DiscoverPane, {
      title: "Discover",
      discover: goldenView("running_search"),
      onAction: () => undefined,
      onClose: () => undefined,
    });
    try {
      const container = rendered.container;
      assert.strictEqual(textOf(container, '[data-role="discover-title"]'), "Discover");
      assert.strictEqual(
        container.querySelector('[data-role="discover-view"]').getAttribute("data-running"),
        "true",
        "the pane says the search runs",
      );
      assert.ok(
        container.querySelector('[data-role="discover-running"]'),
        "a running mark is drawn while the search runs",
      );
      assert.deepStrictEqual(
        sourceRows(container),
        [
          { source: "trust_runtime", label: "truST runtimes", word: "1 found", tone: "ok", count: "1" },
          { source: "ads", label: "ADS and TwinCAT", word: "searching", tone: "neutral", count: "0" },
          { source: "modbus_tcp", label: "Modbus TCP", word: "1 found", tone: "ok", count: "1" },
        ],
        "each source row draws its name, its word in its tone and its count, in the order of the view",
      );
      // A boolean check, not `strictEqual` on a DOM node: a failure would format the node and stall the host.
      assert.ok(
        container.querySelector('[data-role="discover-empty"]') === null,
        "a search that runs shows no empty text",
      );
    } finally {
      await rendered.unmount();
    }
  });

  test("a_running_search_draws_one_card_per_device_with_its_protocols_identity_and_last_sighting", async () => {
    const DiscoverPane = loadDiscoverPane();
    // Specification 25 §2, lines 64-72: one card per device with every protocol found on it, human-readable
    // protocol, discovery-source and confidence labels, and the identity the device has. A wrong implementation
    // draws a card per result, shows the confidence as a backend token such as `port_reachable`, or draws an
    // identity row for a fact the device does not have.
    const rendered = await render(DiscoverPane, {
      title: "Discover",
      discover: goldenView("running_search"),
      onAction: () => undefined,
      onClose: () => undefined,
    });
    try {
      assert.deepStrictEqual(
        cardsOf(rendered.container).map((card) => ({
          id: card.id,
          title: card.title,
          protocols: card.protocols,
          identity: card.identity,
          lastSeen: card.lastSeen,
        })),
        [
          {
            id: "dev:ip:10.0.0.6",
            title: "Line runtime",
            protocols: [
              {
                name: "truST runtimes",
                source: "Network discovery",
                confidence: "Confirmed",
                simulated: "false",
              },
            ],
            identity: [
              { label: "truST version", value: "0.24.75" },
              { label: "Platform", value: "linux-x86_64" },
            ],
            lastSeen: "3 s ago",
          },
          {
            id: "dev:ip:192.168.1.20",
            title: "192.168.1.20",
            protocols: [
              {
                name: "Modbus TCP",
                source: "Known address",
                confidence: "Port reachable only",
                simulated: "false",
              },
            ],
            // A card that is not a runtime shows its host, as the old pane's "Host:" line did
            // (spec 25 §2).
            identity: [{ label: "Host", value: "192.168.1.20" }],
            lastSeen: "3 s ago",
          },
        ],
        "two cards in the order of the view, each with its own protocol, identity and last sighting",
      );
    } finally {
      await rendered.unmount();
    }
  });

  test("a_runtime_card_shows_one_host_and_port_without_the_tcp_prefix", async () => {
    const DiscoverPane = loadDiscoverPane();
    // Specification 25 §2, lines 69-70: a runtime card shows one actionable host and port and omits a visible
    // `tcp://` prefix. The endpoint keeps its prefix in the action's payload, which is not drawn. A wrong
    // implementation draws the endpoint with its prefix, or draws the host as a second address.
    const rendered = await render(DiscoverPane, {
      title: "Discover",
      discover: goldenView("running_search"),
      onAction: () => undefined,
      onClose: () => undefined,
    });
    try {
      const runtimeCard = rendered.container.querySelector('[data-card="dev:ip:10.0.0.6"]');
      assert.ok(runtimeCard, "the runtime card is drawn");
      const addresses = [...runtimeCard.querySelectorAll('[data-role="discover-card-address"]')].map(
        (address: any) => address.textContent,
      );
      assert.deepStrictEqual(addresses, ["10.0.0.6:5680"], "one host and port, without the prefix");
      assert.ok(
        !rendered.container.textContent.includes("tcp://"),
        "no visible text of the pane shows the tcp:// prefix",
      );
    } finally {
      await rendered.unmount();
    }
  });

  test("a_runtime_card_offers_adopt_and_a_device_card_offers_add_and_both_are_enabled", async () => {
    const DiscoverPane = loadDiscoverPane();
    // Specification 25 §2, lines 64-65 (one card per device) and 92-93 (Adopt for a discovered runtime). The `+ Add`
    // of a device card is the view's own action; the spec does not name it. A wrong implementation offers Add on a
    // runtime card, Adopt on a device, or draws either disabled with no reason. Specification 25 §2: every card then
    // offers `Ignore` after its own action (decision O14), so each card lists it last and it is enabled.
    const rendered = await render(DiscoverPane, {
      title: "Discover",
      discover: goldenView("running_search"),
      onAction: () => undefined,
      onClose: () => undefined,
    });
    try {
      assert.deepStrictEqual(
        cardsOf(rendered.container).map((card) => ({ id: card.id, actions: card.actions })),
        [
          {
            id: "dev:ip:10.0.0.6",
            actions: [
              { label: "Adopt", disabled: false },
              { label: "Ignore", disabled: false },
            ],
          },
          {
            id: "dev:ip:192.168.1.20",
            actions: [
              { label: "+ Add", disabled: false },
              { label: "Ignore", disabled: false },
            ],
          },
        ],
        "the runtime card offers Adopt and Ignore, the device card offers + Add and Ignore, and all are enabled",
      );
    } finally {
      await rendered.unmount();
    }
  });

  test("adopt_posts_the_action_exactly_as_the_view_gives_it", async () => {
    const DiscoverPane = loadDiscoverPane();
    // Specification 25 §2, lines 92-93: adopting keeps the discovered label and reaches the runtime through its
    // endpoint. The literal below is the Adopt action the Rust view builds for the runtime at 10.0.0.6. A wrong
    // implementation posts a rebuilt action, drops the label, or posts it with values.
    const posted: unknown[][] = [];
    const rendered = await render(DiscoverPane, {
      title: "Discover",
      discover: goldenView("running_search"),
      onAction: (...args: unknown[]) => posted.push(args),
      onClose: () => undefined,
    });
    try {
      await click(actionButton(rendered.container, "Adopt"));
      assert.deepStrictEqual(
        posted,
        [
          [
            {
              id: "adopt",
              label: "Adopt",
              enabled: true,
              emphasis: "primary",
              op: {
                kind: "rust",
                action: "runtime.adopt",
                payload: {
                  runtime: "dev:ip:10.0.0.6",
                  endpoint: "tcp://10.0.0.6:5680",
                  label: "Line runtime",
                },
              },
            },
          ],
        ],
        "Adopt is posted once, exactly as the view gives it",
      );
    } finally {
      await rendered.unmount();
    }
  });

  test("add_posts_the_form_request_of_the_runtime_that_owns_the_search", async () => {
    const DiscoverPane = loadDiscoverPane();
    // The `+ Add` of a Modbus device opens the Modbus connection on the runtime that owns the search, prefilled
    // from the device's find. The spec does not name the device action; the literal is the payload the Rust view
    // builds (`view/discover.rs`, `device_actions`).
    // A wrong implementation posts the device's own identity in place of the view's request, or no prefill.
    const posted: unknown[][] = [];
    const rendered = await render(DiscoverPane, {
      title: "Discover",
      discover: goldenView("running_search"),
      onAction: (...args: unknown[]) => posted.push(args),
      onClose: () => undefined,
    });
    try {
      await click(actionButton(rendered.container, "+ Add"));
      assert.deepStrictEqual(
        posted,
        [
          [
            {
              id: "add",
              label: "+ Add",
              enabled: true,
              emphasis: "secondary",
              op: {
                kind: "rust",
                action: "form.open",
                payload: {
                  protocol: "modbus_tcp",
                  runtime: "runtime:cell",
                  prefill: { host: "192.168.1.20", port: 502, unit_id: 1 },
                },
              },
            },
          ],
        ],
        "+ Add is posted once, with the Modbus form request of the owning runtime",
      );
    } finally {
      await rendered.unmount();
    }
  });

  test("origins_show_this_computer_and_each_runtime_with_the_chosen_origin_marked", async () => {
    const DiscoverPane = loadDiscoverPane();
    // Specification 25 §2, line 58: `Scan from` chooses this computer or a runtime. Lines 86-91 disable the scans of
    // a stopped runtime rather than removing it, so every origin the view gives is offered. A wrong implementation
    // leaves a runtime out, marks another origin as chosen, or disables an origin the view enables.
    const rendered = await render(DiscoverPane, {
      title: "Discover",
      discover: goldenView("running_search"),
      onAction: () => undefined,
      onClose: () => undefined,
    });
    try {
      assert.deepStrictEqual(originChoices(rendered.container), [
        { id: "this_host", label: "This computer", checked: true, enabled: true },
        { id: "runtime:cell", label: "Cell", checked: false, enabled: true },
      ]);
    } finally {
      await rendered.unmount();
    }
  });

  test("a_stopped_runtime_origin_disables_every_scan_with_its_reason", async () => {
    const DiscoverPane = loadDiscoverPane();
    // Specification 25 §2, lines 82-91: a selected stopped runtime disables every runtime-origin scan. EtherCAT and
    // GPIO use the reason `Start or connect a runtime before scanning EtherCAT or GPIO.`; every other scan from it
    // uses `Start or connect {runtime} before scanning from it.`, with the runtime's name. Bluetooth is disabled in
    // every origin (specification 11, "Sources"). A wrong implementation enables a scan of a stopped runtime, names
    // the wrong runtime, leaves a disabled scan without its reason, or draws a disabled scan as ticked.
    const rendered = await render(DiscoverPane, {
      title: "Discover",
      discover: goldenView("stopped_runtime_origin"),
      onAction: () => undefined,
      onClose: () => undefined,
    });
    try {
      const container = rendered.container;
      assert.deepStrictEqual(
        originChoices(container).map((origin) => [origin.id, origin.checked]),
        [
          ["this_host", false],
          ["runtime:cell", true],
        ],
        "the stopped runtime is the chosen origin, and it is still offered",
      );
      const anyScan = "Start or connect Cell before scanning from it.";
      const hardware = "Start or connect a runtime before scanning EtherCAT or GPIO.";
      // The choices are drawn in the old pane's groups, in this order: `What do you want to find?`, then
      // `Known address or subnet` (MQTT, OPC UA), then `Runtime hardware scans ⚠` (EtherCAT, GPIO). Specification
      // 25 §2: a tick that cannot run is disabled and drawn unticked (old `DiscoverPane.tsx` line 191, `checked &&
      // !disabledReason`), so no scan of the stopped runtime is ticked, trust_runtime and GPIO included.
      assert.deepStrictEqual(
        onlyChoices(container),
        [
          { source: "trust_runtime", checked: false, enabled: false, reason: anyScan },
          { source: "ads", checked: false, enabled: false, reason: anyScan },
          { source: "modbus_tcp", checked: false, enabled: false, reason: anyScan },
          { source: "serial", checked: false, enabled: false, reason: anyScan },
          { source: "usb", checked: false, enabled: false, reason: anyScan },
          {
            source: "bluetooth",
            checked: false,
            enabled: false,
            reason: "Bluetooth search is not available in this truST version yet.",
          },
          { source: "mqtt", checked: false, enabled: false, reason: anyScan },
          { source: "opcua", checked: false, enabled: false, reason: anyScan },
          { source: "ethercat", checked: false, enabled: false, reason: hardware },
          { source: "gpio", checked: false, enabled: false, reason: hardware },
        ],
        "every scan from the stopped runtime is disabled and unticked, each with its reason",
      );
    } finally {
      await rendered.unmount();
    }
  });

  test("this_computer_disables_ethercat_and_gpio_with_the_hardware_reason", async () => {
    const DiscoverPane = loadDiscoverPane();
    // Specification 25 §2, lines 90-91: with `This computer` selected, EtherCAT and GPIO are disabled with
    // `Choose a running runtime for EtherCAT or GPIO scans.`; serial and USB run from it. A wrong implementation
    // enables EtherCAT or GPIO from this computer, or disables USB as well.
    const rendered = await render(DiscoverPane, {
      title: "Discover",
      discover: goldenView("running_search"),
      onAction: () => undefined,
      onClose: () => undefined,
    });
    try {
      const choices = onlyChoices(rendered.container);
      const choose = "Choose a running runtime for EtherCAT or GPIO scans.";
      assert.deepStrictEqual(
        choices.filter((choice) => choice.source === "ethercat" || choice.source === "gpio"),
        [
          { source: "ethercat", checked: false, enabled: false, reason: choose },
          { source: "gpio", checked: false, enabled: false, reason: choose },
        ],
        "EtherCAT and GPIO cannot run from this computer, and say why",
      );
      assert.deepStrictEqual(
        choices.filter((choice) => choice.source === "serial" || choice.source === "usb"),
        [
          { source: "serial", checked: false, enabled: true },
          { source: "usb", checked: false, enabled: true },
        ],
        "serial and USB run from this computer, with no reason",
      );
    } finally {
      await rendered.unmount();
    }
  });

  test("the_known_address_and_subnet_inputs_show_their_labels_and_placeholders", async () => {
    const DiscoverPane = loadDiscoverPane();
    // Specification 25 §2: the Modbus targets of the search are a known host and port, and a caller-supplied
    // subnet, under `Known address or subnet`. The labels and placeholders are the view's. A wrong implementation
    // draws one input for both, or a placeholder that is not the view's.
    const rendered = await render(DiscoverPane, {
      title: "Discover",
      discover: goldenView("running_search"),
      onAction: () => undefined,
      onClose: () => undefined,
    });
    try {
      const targets = [...rendered.container.querySelectorAll('[data-role="discover-target"]')].map(
        (target: any) => ({
          label: target.querySelector("span").textContent,
          placeholder: target.querySelector("input").placeholder,
        }),
      );
      assert.deepStrictEqual(targets, [
        { label: "Known address", placeholder: "host:port" },
        { label: "Subnet", placeholder: "192.168.1.0/24" },
      ]);
    } finally {
      await rendered.unmount();
    }
  });

  test("a_finished_search_draws_the_skipped_source_with_its_reason", async () => {
    const DiscoverPane = loadDiscoverPane();
    // Specification 25 §2, lines 55-58: a source that did not run shows `skipped` with its reason, and a source
    // that ran shows its count. A wrong implementation shows a count for the skipped source, drops its reason, or
    // shows `0 found` for a source that found none in a finished search as `searching`.
    const rendered = await render(DiscoverPane, {
      title: "Discover",
      discover: goldenView("finished_search_skipped_source"),
      onAction: () => undefined,
      onClose: () => undefined,
    });
    try {
      const container = rendered.container;
      assert.deepStrictEqual(sourceRows(container), [
        { source: "trust_runtime", label: "truST runtimes", word: "0 found", tone: "neutral", count: "0" },
        // Specification 25 §2: TwinCAT identity was found and no configured port answered, so the row of the finished
        // search reads `0 found` (the devices that answered) and carries the verification text with the default ports.
        {
          source: "ads",
          label: "ADS and TwinCAT",
          word: "0 found",
          tone: "neutral",
          count: "0",
          reason:
            "TwinCAT identity was found, but none of the configured ADS ports (301, 501, 851, 852, 853, 854) responded.",
        },
        {
          source: "ethercat",
          label: "EtherCAT",
          word: "skipped",
          tone: "muted",
          reason: "EtherCAT is searched inside a runtime, on the computer with the bus adapter.",
        },
      ]);
      assert.strictEqual(
        container.querySelector('[data-role="discover-view"]').getAttribute("data-running"),
        "false",
        "a finished search is not marked as running",
      );
      // Specification 25 §2: the ADS device of this search was found but no configured port answered, so it has no card,
      // and an empty result is drawn when a source ran (the ADS source did). The text gives the checks and the
      // runtime hint (lines 75-77), so the pane draws it as the view gives it.
      assert.strictEqual(
        textOf(container, '[data-role="discover-empty"]'),
        "Nothing found. Check that the device is powered on, on the same network, and not blocked by a port or firewall. Verify the address or subnet for known-address scans. For EtherCAT or GPIO, choose a running runtime in Scan from.",
        "the empty result draws its checks, since no card is shown and a source ran",
      );
    } finally {
      await rendered.unmount();
    }
  });

  test("an_empty_result_draws_the_checks_and_the_hardware_hint", async () => {
    const DiscoverPane = loadDiscoverPane();
    // Specification 25 §2, lines 75-77: an empty result suggests the checks of device power, network reachability,
    // port and firewall restrictions, and the supplied address or subnet; then the hint for the hardware scans. A
    // wrong implementation leaves one of the four checks out, or drops the hint. The timed-out source is drawn in
    // its warning tone (specification 25 §2, lines 55-58).
    const rendered = await render(DiscoverPane, {
      title: "Discover",
      discover: goldenView("empty_result"),
      onAction: () => undefined,
      onClose: () => undefined,
    });
    try {
      const container = rendered.container;
      assert.strictEqual(
        textOf(container, '[data-role="discover-empty"]'),
        "Nothing found. Check that the device is powered on, on the same network, and not blocked by a port or firewall. Verify the address or subnet for known-address scans. For EtherCAT or GPIO, choose a running runtime in Scan from.",
      );
      assert.strictEqual(
        container.querySelectorAll('[data-role="discover-card"]').length,
        0,
        "an empty result draws no card",
      );
      assert.deepStrictEqual(sourceRows(container)[1], {
        source: "ads",
        label: "ADS and TwinCAT",
        word: "timed out",
        tone: "warning",
        count: "0",
      });
    } finally {
      await rendered.unmount();
    }
  });

  test("an_older_runtime_skips_every_source_with_the_update_reason", async () => {
    const DiscoverPane = loadDiscoverPane();
    // Specification 34 TARGET-05: when the versions do not match, the tools say which update is needed. The Rust
    // text names the version the search needs: `update it to 0.24.75 or later`. An older runtime cannot search, so
    // every one of the ten sources is skipped with that reason, in the order of the search's own table. A wrong
    // implementation draws a count for a skipped source, drops a source, or shows a different reason.
    const rendered = await render(DiscoverPane, {
      title: "Discover",
      discover: goldenView("older_runtime_update"),
      onAction: () => undefined,
      onClose: () => undefined,
    });
    try {
      const rows = sourceRows(rendered.container);
      assert.deepStrictEqual(
        rows.map((row) => row.source),
        [
          "trust_runtime",
          "ads",
          "modbus_tcp",
          "mqtt",
          "opcua",
          "ethercat",
          "gpio",
          "serial",
          "usb",
          "bluetooth",
        ],
        "every source of the search's table is drawn",
      );
      for (const row of rows) {
        assert.strictEqual(row.word, "skipped", `${row.source} is skipped`);
        assert.strictEqual(row.tone, "muted", `${row.source} is drawn muted`);
        assert.strictEqual(
          row.reason,
          "This runtime's truST version cannot search; update it to 0.24.75 or later.",
          `${row.source} carries the update reason`,
        );
        assert.ok(!("count" in row), `${row.source} draws no count`);
      }
    } finally {
      await rendered.unmount();
    }
  });

  test("a_pane_action_the_view_offers_is_drawn_in_the_footer_and_posted_as_given", async () => {
    const DiscoverPane = loadDiscoverPane();
    // Specification 25 §2: `Search` runs one search. The view carries the pane's own actions, and the pane draws
    // them in its footer. A Search action is posted as the view gives it, with the known address and subnet the
    // user typed as its values (the next test checks the values). A wrong implementation drops the pane's actions,
    // or posts a rebuilt action.
    const view = goldenView("running_search");
    const search = {
      id: "search",
      label: "Search",
      enabled: true,
      emphasis: "primary",
      op: { kind: "search", request: { verb: "start", origin: "this_host", params: { only: ["ads"] } } },
    };
    view.actions = [search];
    const posted: unknown[][] = [];
    const rendered = await render(DiscoverPane, {
      title: "Discover",
      discover: view,
      onAction: (...args: unknown[]) => posted.push(args),
      onClose: () => undefined,
    });
    try {
      await click(
        actionButton(rendered.container.querySelector('[data-role="discover-footer"]'), "Search"),
      );
      assert.deepStrictEqual(
        posted,
        [[search, { hosts: "", cidr: "" }]],
        "Search is posted as the view gives it, with the typed targets, which are empty here",
      );
    } finally {
      await rendered.unmount();
    }
  });

  test("search_posts_the_known_address_and_subnet_the_user_typed", async () => {
    const DiscoverPane = loadDiscoverPane();
    // Specification 25 §2: `Search` starts a search with the chosen targets, which are the known address and the
    // subnet typed into the two inputs. A wrong implementation posts the Search action without them, or posts the
    // subnet in the address input's value.
    const view = goldenView("running_search");
    const search = {
      id: "search",
      label: "Search",
      enabled: true,
      emphasis: "primary",
      op: { kind: "search", request: { verb: "start", origin: "this_host", params: { only: ["ads"] } } },
    };
    view.actions = [search];
    const posted: unknown[][] = [];
    const rendered = await render(DiscoverPane, {
      title: "Discover",
      discover: view,
      onAction: (...args: unknown[]) => posted.push(args),
      onClose: () => undefined,
    });
    try {
      const setValue = async (selector: string, value: string) => {
        const input = rendered.container.querySelector(selector);
        assert.ok(input, `${selector} is drawn`);
        await act(async () => {
          const setter = Object.getOwnPropertyDescriptor(rendered.window.HTMLInputElement.prototype, "value")?.set;
          setter?.call(input, value);
          input.dispatchEvent(new rendered.window.Event("input", { bubbles: true }));
        });
      };
      await setValue('[data-role="discover-hosts"]', "10.0.0.9:502");
      await setValue('[data-role="discover-cidr"]', "192.168.1.0/24");
      await click(
        actionButton(rendered.container.querySelector('[data-role="discover-footer"]'), "Search"),
      );
      assert.deepStrictEqual(
        posted,
        [[search, { hosts: "10.0.0.9:502", cidr: "192.168.1.0/24" }]],
        "Search carries the address and the subnet as the user typed them",
      );
    } finally {
      await rendered.unmount();
    }
  });

  test("the_idle_view_draws_the_search_before_any_search_and_posts_it_as_given", async () => {
    const DiscoverPane = loadDiscoverPane();
    // Specification 25 §2: before the first search the pane draws its scan choices with the help above them, no source
    // row and no card, and the enabled Search action, which is posted as the view gives it with the typed targets. A
    // wrong implementation draws nothing when the pane opens, draws a source row or a card that no search found, or
    // leaves Search out or disabled.
    const view = idleView();
    const posted: unknown[][] = [];
    const rendered = await render(DiscoverPane, {
      title: "Discover",
      discover: view,
      onAction: (...args: unknown[]) => posted.push(args),
      onClose: () => undefined,
    });
    try {
      assert.strictEqual(
        textOf(rendered.container, '[data-role="discover-choose"]'),
        "Choose what to find, then scan. Only the selected device types are searched.",
        "the help above the scan choices is drawn",
      );
      assert.strictEqual(sourceRows(rendered.container).length, 0, "no source has run");
      assert.strictEqual(
        rendered.container.querySelectorAll('[data-role="discover-card"]').length,
        0,
        "no card is drawn before a search",
      );
      const search = actionButton(rendered.container.querySelector('[data-role="discover-footer"]'), "Search");
      assert.strictEqual(search.disabled, false, "Search is enabled on the first run");
      await click(search);
      assert.deepStrictEqual(
        posted,
        [[view.actions[0], { hosts: "", cidr: "" }]],
        "Search is posted as the view gives it, with the empty targets",
      );
    } finally {
      await rendered.unmount();
    }
  });

  test("a_failed_search_draws_its_error_text_and_offers_search_again", async () => {
    const DiscoverPane = loadDiscoverPane();
    // Specification 25 §2: a search that failed at a runtime draws the old pane's text in the pane, and Search stays
    // offered so that the scan can run again. The text is the one `discoveryErrors.ts` gives for a failed request.
    // A wrong implementation draws no text, draws a text of its own, or leaves Search out.
    const view = {
      ...idleView(),
      error_text:
        "The selected runtime could not complete truST runtime discovery. Reconnect or start the selected runtime, then scan again.",
    };
    const rendered = await render(DiscoverPane, {
      title: "Discover",
      discover: view,
      onAction: () => undefined,
      onClose: () => undefined,
    });
    try {
      assert.strictEqual(
        textOf(rendered.container, '[data-role="discover-error"]'),
        "The selected runtime could not complete truST runtime discovery. Reconnect or start the selected runtime, then scan again.",
        "the error text is drawn as the view gives it",
      );
      const search = actionButton(rendered.container.querySelector('[data-role="discover-footer"]'), "Search");
      assert.strictEqual(search.disabled, false, "Search is offered again after the failure");
    } finally {
      await rendered.unmount();
    }
  });

  test("the_headings_and_notes_are_drawn_from_the_view", async () => {
    const DiscoverPane = loadDiscoverPane();
    // Specification 25 §2 (the old pane's headings, lines 262-306, and its notes, lines 33-49): the pane draws each
    // heading and note the view gives, in the order of the old pane. A wrong implementation draws a heading of its
    // own, or puts a note under the wrong source.
    const rendered = await render(DiscoverPane, {
      title: "Discover",
      discover: goldenView("running_search"),
      onAction: () => undefined,
      onClose: () => undefined,
    });
    try {
      const headings = [...rendered.container.querySelectorAll('[data-role="discover-heading"]')].map(
        (heading: any) => heading.textContent,
      );
      assert.deepStrictEqual(headings, [
        "What do you want to find?",
        "Advanced scan settings",
        "Scan from",
        "Known address or subnet",
        "Runtime hardware scans ⚠",
      ]);
      const notes = [...rendered.container.querySelectorAll('[data-role="discover-only-choice"]')].map(
        (choice: any) => [
          choice.getAttribute("data-source"),
          optionalText(choice, '[data-role="discover-only-note"]') ?? null,
        ],
      );
      assert.deepStrictEqual(notes.find(([source]) => source === "trust_runtime"), ["trust_runtime", "mDNS"]);
      assert.deepStrictEqual(notes.find(([source]) => source === "ads"), ["ads", "network broadcast"]);
      assert.deepStrictEqual(notes.find(([source]) => source === "serial"), ["serial", null]);
    } finally {
      await rendered.unmount();
    }
  });

  test("a_simulated_find_draws_the_mark_and_an_unknown_source_draws_the_update_line", async () => {
    const DiscoverPane = loadDiscoverPane();
    // Specification 11, "Confidence" (a simulated result is never presented as hardware) and specification 25 §2
    // (one line asks to update truST when a source or protocol is left out). A wrong implementation draws the
    // simulated flag only as an attribute, or draws no update line.
    const view = goldenView("running_search");
    view.cards[0].protocols[0].simulated = true;
    view.cards[0].protocols[0].mark = "Simulated";
    view.update_text =
      "This search reports a source or protocol that this version of truST does not know. Update truST to see its results.";
    const rendered = await render(DiscoverPane, {
      title: "Discover",
      discover: view,
      onAction: () => undefined,
      onClose: () => undefined,
    });
    try {
      assert.strictEqual(
        textOf(rendered.container, '[data-role="discover-card-protocol-mark"]'),
        "Simulated",
        "the simulated find shows its mark",
      );
      assert.strictEqual(
        textOf(rendered.container, '[data-role="discover-update"]'),
        "This search reports a source or protocol that this version of truST does not know. Update truST to see its results.",
        "the update line is drawn",
      );
    } finally {
      await rendered.unmount();
    }
  });

  test("a_result_of_this_pane_is_drawn_in_its_footer", async () => {
    const DiscoverPane = loadDiscoverPane();
    // Specification 25 §5.13: a notice goes to the pane it names. App.tsx hands this pane only the notice that
    // names `discover` (Adopt's result does), so the pane draws the notice it is given in its footer. A wrong
    // implementation draws it nowhere.
    const rendered = await render(DiscoverPane, {
      title: "Discover",
      discover: goldenView("running_search"),
      notice: {
        pane: "discover",
        tone: "ok",
        text: "The runtime was adopted.",
        field_errors: [],
        actions: [],
      },
      onAction: () => undefined,
      onClose: () => undefined,
    });
    try {
      assert.strictEqual(
        textOf(rendered.container, '[data-role="browse-notice-text"]'),
        "The runtime was adopted.",
        "the result of this pane is drawn in its footer",
      );
    } finally {
      await rendered.unmount();
    }
  });

  test("every_text_the_pane_draws_is_a_text_the_view_gives", async () => {
    const DiscoverPane = loadDiscoverPane();
    // Specification 25 §2 and design §4.5 D4: the pane adds no label, rule or decision, so every text it draws is
    // a string of the view, except the title, which is the toolbar's word for the pane, and the close glyph. A
    // wrong implementation draws a label of its own, such as `Scan from` or `Only search`, which the view does not
    // give; the view does not carry those group headings yet, so the pane must not draw them.
    for (const name of [
      "running_search",
      "finished_search_skipped_source",
      "empty_result",
      "stopped_runtime_origin",
      "older_runtime_update",
    ]) {
      const view = goldenView(name);
      const allowed = viewStrings(view);
      allowed.add("Discover");
      allowed.add("✕");
      // The warning mark is a glyph beside the view's own warning title (DiscoverPane.tsx line 204).
      allowed.add("⚠");
      const rendered = await render(DiscoverPane, {
        title: "Discover",
        discover: view,
        onAction: () => undefined,
        onClose: () => undefined,
      });
      try {
        const drawn = leafTexts(rendered.container).filter((text) => !allowed.has(text));
        assert.deepStrictEqual(drawn, [], `${name}: the pane draws text that the view does not give`);
      } finally {
        await rendered.unmount();
      }
    }
  });

  test("the_ethercat_choice_draws_its_warning_title_beside_its_name", async () => {
    const DiscoverPane = loadDiscoverPane();
    // Specification 25 §2 (old line 204): the EtherCAT choice shows the mark ⚠ with the title that the view
    // gives, `Can disturb a live bus, so confirm before scanning`. A wrong implementation draws the mark with no
    // title, or a title of its own.
    const view = {
      ...goldenView("stopped_runtime_origin"),
      only: [
        {
          source: "ethercat",
          label: "EtherCAT",
          checked: false,
          enabled: true,
          group: "runtime_hardware",
          note: "bus enumeration",
          warning: "Can disturb a live bus, so confirm before scanning",
        },
      ],
      cards: [],
      actions: [],
      show_ignored: { label: "Show ignored", checked: false, action: idleSearchOp("show_ignored") },
    };
    const rendered = await render(DiscoverPane, {
      title: "Discover",
      discover: view,
      onAction: () => undefined,
      onClose: () => undefined,
    });
    try {
      const mark = rendered.container.querySelector('[data-role="discover-only-warning"]');
      assert.ok(mark, "the warning mark is drawn");
      assert.strictEqual(mark.textContent, "⚠");
      assert.strictEqual(
        mark.getAttribute("title"),
        "Can disturb a live bus, so confirm before scanning",
        "the mark's title is the view's warning",
      );
    } finally {
      await rendered.unmount();
    }
  });

  test("a_tick_posts_the_change_the_view_gives_it_exactly_once", async () => {
    const DiscoverPane = loadDiscoverPane();
    // Specification 25 §2: a tick records its change with the status request that the view gives, and the view
    // answers with the new state. A wrong implementation posts nothing, rebuilds the request, or posts twice.
    const tick = {
      id: "select",
      label: "ADS and TwinCAT",
      enabled: true,
      emphasis: "quiet",
      op: {
        kind: "search",
        request: {
          verb: "status",
          origin: "this_host",
          search_id: "s-1",
          change: { kind: "select", source: "ads", checked: true },
        },
      },
    };
    const view = {
      ...goldenView("stopped_runtime_origin"),
      only: [
        { source: "ads", label: "ADS and TwinCAT", checked: false, enabled: true, group: "find", note: "network broadcast", action: tick },
      ],
      cards: [],
      actions: [],
      show_ignored: { label: "Show ignored", checked: false, action: idleSearchOp("show_ignored") },
    };
    const posted: unknown[][] = [];
    const rendered = await render(DiscoverPane, {
      title: "Discover",
      discover: view,
      onAction: (...args: unknown[]) => posted.push(args),
      onClose: () => undefined,
    });
    try {
      const tickBox = rendered.container.querySelector('[data-role="discover-only-tick"]');
      await click(tickBox);
      assert.deepStrictEqual(posted, [[tick]], "the tick posts its change once, as given");
    } finally {
      await rendered.unmount();
    }
  });

  test("a_twincat_card_draws_its_ports_the_mark_and_posts_each_port_as_given", async () => {
    const DiscoverPane = loadDiscoverPane();
    // Specification 25 §2 (old lines 345 and 356-373): a TwinCAT card counts its responding ports, shows each
    // port as a button, and marks a device the canvas holds with `On canvas`. A port button posts the browse of
    // that port exactly as the view gives it. A wrong implementation draws the ports as text, drops the mark, or
    // rebuilds the port's request.
    const browseFor = (port: number) => ({
      id: `ads_port_${port}`,
      label: String(port),
      enabled: true,
      emphasis: "secondary",
      op: {
        kind: "ui",
        ui: {
          kind: "open",
          pane: { kind: "browse", request: { source: { found: "dev:ads:5.6.7.8.1.1", protocol: "ads" }, port } },
        },
      },
    });
    const card = {
      id: "dev:ads:5.6.7.8.1.1",
      title: "PLC-A",
      addresses: ["10.0.0.5"],
      protocols: [],
      identity: [],
      last_seen: "3 s ago",
      warnings: [],
      actions: [
        {
          id: "manage_tags",
          label: "Manage tags",
          enabled: true,
          emphasis: "primary",
          op: { kind: "ui", ui: { kind: "open", pane: { kind: "browse", request: {} } } },
        },
      ],
      responding_ports: "2 responding ADS ports",
      ports: [browseFor(851), browseFor(852)],
      on_canvas: "On canvas",
    };
    const view = {
      ...goldenView("stopped_runtime_origin"),
      only: [],
      cards: [card],
      actions: [],
      show_ignored: { label: "Show ignored", checked: false, action: idleSearchOp("show_ignored") },
    };
    const posted: unknown[][] = [];
    const rendered = await render(DiscoverPane, {
      title: "Discover",
      discover: view,
      onAction: (...args: unknown[]) => posted.push(args),
      onClose: () => undefined,
    });
    try {
      const drawn = rendered.container.textContent ?? "";
      assert.ok(drawn.includes("2 responding ADS ports"), "the count of responding ports is drawn");
      assert.ok(drawn.includes("On canvas"), "the canvas mark is drawn");
      const portButtons = [...rendered.container.querySelectorAll('[data-role="discover-card-ports"] [data-role="action-button"]')];
      assert.deepStrictEqual(
        portButtons.map((button: any) => button.textContent),
        ["851", "852"],
        "each responding port is a button, in port order",
      );
      await click(portButtons[1]);
      assert.deepStrictEqual(posted, [[browseFor(852)]], "the port's browse is posted as the view gives it");
    } finally {
      await rendered.unmount();
    }
  });

  test("show_ignored_posts_its_change_and_a_card_awaiting_its_view_posts_once", async () => {
    const DiscoverPane = loadDiscoverPane();
    // Decision O14 and specification 25 §2: `Show ignored` posts its change; an ignored card offers `Stop ignoring`,
    // and while that search awaits its view no card action posts again. A wrong implementation posts the toggle
    // from its own state, or lets a double click post the change twice.
    const stop = {
      id: "stop_ignoring",
      label: "Stop ignoring",
      enabled: true,
      emphasis: "secondary",
      op: { kind: "search", request: { verb: "status", origin: "this_host", search_id: "s-1", change: { kind: "ignore", device: "dev:ads:5.6.7.8.1.1", ignored: false } } },
    };
    const toggle = { label: "Show ignored", checked: false, action: idleSearchOp("show_ignored", true) };
    const card = {
      id: "dev:ads:5.6.7.8.1.1",
      title: "PLC-A",
      addresses: [],
      protocols: [],
      identity: [],
      last_seen: "3 s ago",
      warnings: [],
      actions: [stop],
      ports: [],
    };
    const view = { ...goldenView("stopped_runtime_origin"), only: [], cards: [card], actions: [], show_ignored: toggle };
    const posted: unknown[][] = [];
    const rendered = await render(DiscoverPane, {
      title: "Discover",
      discover: view,
      onAction: (...args: unknown[]) => posted.push(args),
      onClose: () => undefined,
    });
    try {
      await click(rendered.container.querySelector('[data-role="discover-show-ignored-tick"]'));
      assert.deepStrictEqual(posted, [[toggle.action]], "the toggle posts its change as given");
      await click(actionButton(rendered.container, "Stop ignoring"));
      await click(actionButton(rendered.container, "Stop ignoring"));
      assert.deepStrictEqual(
        posted.slice(1),
        [[stop]],
        "the card action posts once while its search awaits the view",
      );
    } finally {
      await rendered.unmount();
    }
  });
});
