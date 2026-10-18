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
// Fieldbus confirmations (EtherCAT and Modbus RTU, each naming its bus, adapter and effect) and compare markers
// (`Compare with project`) are not in the view yet: C7 leaves `fieldbus` and `compare` empty, and C6 adds
// compare. Their tests belong to the later slice that draws them.

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
            identity: [],
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
    // runtime card, Adopt on a device, or draws either disabled with no reason.
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
          { id: "dev:ip:10.0.0.6", actions: [{ label: "Adopt", disabled: false }] },
          { id: "dev:ip:192.168.1.20", actions: [{ label: "+ Add", disabled: false }] },
        ],
        "the runtime card offers Adopt, the device card offers + Add, and both are enabled",
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
    // the wrong runtime, or leaves a disabled scan without its reason.
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
      assert.deepStrictEqual(
        onlyChoices(container),
        [
          { source: "trust_runtime", checked: true, enabled: false, reason: anyScan },
          { source: "ads", checked: false, enabled: false, reason: anyScan },
          { source: "modbus_tcp", checked: false, enabled: false, reason: anyScan },
          { source: "mqtt", checked: false, enabled: false, reason: anyScan },
          { source: "opcua", checked: false, enabled: false, reason: anyScan },
          { source: "ethercat", checked: false, enabled: false, reason: hardware },
          { source: "gpio", checked: true, enabled: false, reason: hardware },
          { source: "serial", checked: false, enabled: false, reason: anyScan },
          { source: "usb", checked: false, enabled: false, reason: anyScan },
          {
            source: "bluetooth",
            checked: false,
            enabled: false,
            reason: "Bluetooth search is not available in this truST version yet.",
          },
        ],
        "every scan from the stopped runtime is disabled, each with its reason",
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
        { source: "ads", label: "ADS and TwinCAT", word: "1 found", tone: "ok", count: "1" },
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
      assert.ok(
        container.querySelector('[data-role="discover-empty"]') === null,
        "a search that found a device shows no empty text",
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

  test("a_pane_action_the_view_offers_is_drawn_in_the_footer_and_posted_unchanged", async () => {
    const DiscoverPane = loadDiscoverPane();
    // Specification 25 §2: `Search` runs one search. The view carries the pane's own actions, and the pane draws
    // them in its footer, so that a Search action needs no change to the pane. `search.start` is a stand-in name:
    // the test checks only that the pane draws the action and posts it unchanged. A wrong implementation drops the
    // pane's actions, or posts a rebuilt action.
    const view = goldenView("running_search");
    const search = {
      id: "search",
      label: "Search",
      enabled: true,
      emphasis: "primary",
      op: { kind: "rust", action: "search.start", payload: {} },
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
      assert.deepStrictEqual(posted, [[search]], "Search is posted exactly as the view gives it");
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
});
