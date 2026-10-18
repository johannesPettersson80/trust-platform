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

// The canvas is drawn from the Rust view model; the expected values below are the literals of that
// contract (crates/trust-devices/src/view/types/, design §2.1), never a value read back from the
// component.

/** A webview component under test, bundled the way the extension ships it. */
type WebviewComponent = React.ComponentType<Record<string, unknown>>;

/** Bundles one webview entry point and returns its named export, as the extension ships it. */
function loadComponent(component: "Canvas" | "App"): WebviewComponent {
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
interface RenderedCanvas {
  container: any;
  unmount(): Promise<void>;
}

async function renderElement(
  component: WebviewComponent,
  props: Record<string, unknown>,
): Promise<RenderedCanvas> {
  const dom = new JSDOM('<!doctype html><body><div id="root"></div></body>', {
    url: "https://devices-canvas.test/",
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

async function renderCanvas(props: Record<string, unknown>): Promise<RenderedCanvas> {
  return renderElement(loadComponent("Canvas"), {
    selectedRuntime: undefined,
    onSelectRuntime: () => undefined,
    ...props,
  });
}

/** Renders the whole shell, so the seam between `App`, the canvas and `post` is exercised. */
async function renderApp(
  view: Record<string, unknown>,
  post: (message: unknown) => void,
): Promise<RenderedCanvas> {
  return renderElement(loadComponent("App"), { view, post });
}

// A connection pill with the fields the canvas draws (graph.rs `Connection`). Each test overrides
// only what it is about, so its expected values stay visible.
function connection(overrides: Record<string, unknown>): Record<string, unknown> {
  return {
    id: "connection-1",
    name: "Modbus meter",
    dot: "ok",
    live: true,
    role_word: "CLIENT",
    marks: [],
    look: "normal",
    ...overrides,
  };
}

/** A runtime with the fields the canvas draws (graph.rs `Runtime`). */
function runtime(overrides: Record<string, unknown>): Record<string, unknown> {
  return {
    id: "runtime-1",
    name: "Line runtime",
    state: { word: "Running", tone: "ok" },
    ...overrides,
  };
}

/** A computer frame with the fields the canvas draws (graph.rs `Computer`). */
function computer(overrides: Record<string, unknown>): Record<string, unknown> {
  return {
    id: "host-1",
    heading: "This computer",
    reachability: { word: "Reachable", tone: "ok" },
    runtimes: [runtime({})],
    ...overrides,
  };
}

/** A device with the fields the canvas draws (graph.rs `Device`). */
function device(overrides: Record<string, unknown>): Record<string, unknown> {
  return {
    id: "device-1",
    kind: "external",
    name: "Modbus meter",
    presents: "Modbus meter",
    ...overrides,
  };
}

/** A computer with one runtime holding one connection, so a link has two ends to route between. */
function frameWithConnection(): Record<string, unknown> {
  return computer({
    runtimes: [runtime({ connections: [connection({ id: "conn-1" })] })],
  });
}

suite("Devices & Connections panel canvas", () => {
  test("a_draft_or_disabled_connection_shows_its_word_exactly_as_given", async () => {
    const rendered = await renderCanvas({
      computers: [
        computer({
          runtimes: [
            runtime({
              connections: [
                connection({ id: "conn-client", role_word: "CLIENT", marks: [] }),
                connection({ id: "conn-draft", role_word: "PUB/SUB", marks: ["draft"], look: "draft" }),
                connection({
                  id: "conn-disabled",
                  role_word: "PEER",
                  marks: ["disabled"],
                  look: "pending",
                }),
              ],
            }),
          ],
        }),
      ],
    });
    try {
      const client = rendered.container.querySelector('[data-item-id="conn-client"]');
      assert.strictEqual(
        client.querySelector('[data-role="connection-role"]').textContent,
        "CLIENT",
        "the role word is drawn as the view gives it",
      );
      assert.strictEqual(
        client.querySelectorAll('[data-role="connection-mark"]').length,
        0,
        "a connection with no marks shows no mark",
      );

      const draft = rendered.container.querySelector('[data-item-id="conn-draft"]');
      const draftMarks = draft.querySelectorAll('[data-role="connection-mark"]');
      assert.strictEqual(draftMarks.length, 1, "one draft mark");
      assert.strictEqual(draftMarks[0].getAttribute("data-mark"), "draft");
      assert.strictEqual(draftMarks[0].textContent, "draft");
      assert.strictEqual(draft.querySelector('[data-role="connection-role"]').textContent, "PUB/SUB");

      const disabled = rendered.container.querySelector('[data-item-id="conn-disabled"]');
      const disabledMarks = disabled.querySelectorAll('[data-role="connection-mark"]');
      assert.strictEqual(disabledMarks.length, 1, "one disabled mark");
      assert.strictEqual(disabledMarks[0].getAttribute("data-mark"), "disabled");
      assert.strictEqual(disabledMarks[0].textContent, "disabled");
      assert.strictEqual(disabled.querySelector('[data-role="connection-role"]').textContent, "PEER");
    } finally {
      await rendered.unmount();
    }
  });

  test("an_unproven_link_is_dashed_and_a_proven_one_solid_as_given", async () => {
    const rendered = await renderCanvas({
      computers: [frameWithConnection()],
      devices: [device({ id: "device-1" })],
      links: [
        {
          id: "link-unproven",
          from: "conn-1",
          to: "device-1",
          proven: false,
          tone: "neutral",
          arrow: "none",
          secure: false,
        },
        {
          id: "link-proven",
          from: "conn-1",
          to: "device-1",
          proven: true,
          tone: "ok",
          arrow: "to",
          secure: false,
        },
      ],
    });
    try {
      const unproven = rendered.container.querySelector('[data-link-id="link-unproven"]');
      assert.strictEqual(unproven.getAttribute("data-proven"), "false");
      assert.strictEqual(
        unproven.querySelector('[data-role="link-line"]').getAttribute("stroke-dasharray"),
        "5 4",
        "an unproven link is dashed",
      );

      const proven = rendered.container.querySelector('[data-link-id="link-proven"]');
      assert.strictEqual(proven.getAttribute("data-proven"), "true");
      assert.strictEqual(
        proven.querySelector('[data-role="link-line"]').getAttribute("stroke-dasharray"),
        null,
        "a proven link is solid",
      );
    } finally {
      await rendered.unmount();
    }
  });

  test("a_link_draws_its_arrow_and_lock_from_the_view_model", async () => {
    const rendered = await renderCanvas({
      computers: [frameWithConnection()],
      devices: [device({ id: "device-1" })],
      links: [
        {
          id: "link-from",
          from: "conn-1",
          to: "device-1",
          proven: true,
          tone: "ok",
          arrow: "from",
          secure: false,
        },
        {
          id: "link-to",
          from: "conn-1",
          to: "device-1",
          proven: true,
          tone: "ok",
          arrow: "to",
          secure: true,
        },
        {
          id: "link-none",
          from: "conn-1",
          to: "device-1",
          proven: true,
          tone: "ok",
          arrow: "none",
          secure: false,
        },
      ],
    });
    try {
      const from = rendered.container.querySelector('[data-link-id="link-from"]');
      assert.strictEqual(
        from.querySelector('[data-role="link-line"]').getAttribute("marker-start"),
        "url(#devices-link-arrow)",
        "the arrow points at the named end",
      );
      assert.strictEqual(from.querySelector('[data-role="link-line"]').getAttribute("marker-end"), null);
      assert.strictEqual(from.querySelector('[data-role="link-lock"]'), null, "no lock when not secure");

      const to = rendered.container.querySelector('[data-link-id="link-to"]');
      assert.strictEqual(
        to.querySelector('[data-role="link-line"]').getAttribute("marker-end"),
        "url(#devices-link-arrow)",
      );
      assert.strictEqual(to.querySelector('[data-role="link-line"]').getAttribute("marker-start"), null);
      assert.ok(
        to.querySelector('[data-role="link-lock"]'),
        "a secure link shows a lock",
      );

      const none = rendered.container.querySelector('[data-link-id="link-none"]');
      assert.strictEqual(none.getAttribute("data-arrow"), "none");
      assert.strictEqual(none.querySelector('[data-role="link-line"]').getAttribute("marker-end"), null);
      assert.strictEqual(none.querySelector('[data-role="link-line"]').getAttribute("marker-start"), null);
      assert.strictEqual(none.querySelector('[data-role="link-lock"]'), null);
    } finally {
      await rendered.unmount();
    }
  });

  test("the_mesh_label_appears_only_when_the_view_model_gives_one", async () => {
    const rendered = await renderCanvas({
      computers: [frameWithConnection()],
      devices: [device({ id: "device-1" })],
      mesh: [
        {
          id: "bus-labelled",
          label: "Mesh fabric",
          draft: false,
          drops: [{ item: "conn-1", proven: true, tone: "neutral" }],
        },
        {
          id: "bus-unlabelled",
          draft: false,
          drops: [
            { item: "conn-1", proven: true, tone: "neutral" },
            { item: "device-1", proven: true, tone: "warning" },
          ],
        },
      ],
    });
    try {
      const labelled = rendered.container.querySelector('[data-item-id="bus-labelled"]');
      assert.strictEqual(
        labelled.querySelector('[data-role="mesh-label"]').textContent,
        "Mesh fabric",
        "the label is drawn as the view gives it",
      );

      const unlabelled = rendered.container.querySelector('[data-item-id="bus-unlabelled"]');
      assert.strictEqual(
        unlabelled.querySelector('[data-role="mesh-label"]'),
        null,
        "no label when the view gives none, however many drops it has",
      );
    } finally {
      await rendered.unmount();
    }
  });

  test("the_summary_and_banner_actions_render_and_post_as_given", async () => {
    const posted: unknown[] = [];
    const summary = {
      text: "2 computers · 2 runtimes · 3 connections · 1 running · 0 faults · 2 warnings",
    };
    const bannerAction = {
      id: "start_simulator",
      label: "Start simulator",
      enabled: true,
      emphasis: "primary",
      op: { kind: "host", effect: { kind: "start_simulator" } },
    };
    const rendered = await renderCanvas({
      computers: [],
      summary,
      banner: { tone: "warning", text: "Simulator is stopped.", actions: [bannerAction] },
      onAction: (action: unknown) => {
        posted.push(action);
      },
    });
    try {
      assert.strictEqual(
        rendered.container.querySelector('[data-role="summary-text"]').textContent,
        "2 computers · 2 runtimes · 3 connections · 1 running · 0 faults · 2 warnings",
      );
      assert.strictEqual(
        rendered.container.querySelector('[data-role="banner-text"]').textContent,
        "Simulator is stopped.",
      );
      const button = rendered.container.querySelector('[data-banner-item="start_simulator"]');
      assert.strictEqual(button.textContent, "Start simulator");
      await act(async () => {
        button.click();
      });
      assert.deepStrictEqual(posted, [bannerAction], "the banner action is posted unchanged");
    } finally {
      await rendered.unmount();
    }
  });

  test("dimmed_items_are_dimmed_and_hidden_items_are_not_drawn", async () => {
    const rendered = await renderCanvas({
      computers: [
        computer({
          id: "host-visible",
          runtimes: [runtime({ id: "runtime-visible", connections: [] })],
        }),
        computer({
          id: "host-hidden",
          heading: "Hidden computer",
          hidden: true,
          runtimes: [runtime({ id: "runtime-hidden" })],
        }),
      ],
      devices: [
        device({ id: "device-dimmed", name: "Dimmed device", dimmed: true }),
        device({ id: "device-hidden", name: "Hidden device", hidden: true }),
      ],
      links: [
        {
          id: "link-dimmed",
          from: "runtime-visible",
          to: "device-dimmed",
          proven: true,
          tone: "neutral",
          arrow: "to",
          secure: false,
          dimmed: true,
        },
        {
          id: "link-hidden",
          from: "runtime-visible",
          to: "device-dimmed",
          proven: true,
          tone: "neutral",
          arrow: "to",
          secure: false,
          hidden: true,
        },
      ],
    });
    try {
      const dimmed = rendered.container.querySelector('[data-item-id="device-dimmed"]');
      assert.strictEqual(dimmed.getAttribute("data-dimmed"), "true", "a dimmed device is drawn dimmed");
      assert.strictEqual(
        rendered.container.querySelector('[data-item-id="device-hidden"]'),
        null,
        "a hidden device is not drawn",
      );
      assert.strictEqual(rendered.container.querySelector('[data-item-id="host-hidden"]'), null);
      assert.strictEqual(rendered.container.querySelector('[data-item-id="runtime-hidden"]'), null);

      const dimmedLink = rendered.container.querySelector('[data-link-id="link-dimmed"]');
      assert.strictEqual(
        dimmedLink.getAttribute("data-dimmed"),
        "true",
        "a dimmed link is drawn dimmed",
      );
      assert.strictEqual(
        rendered.container.querySelector('[data-link-id="link-hidden"]'),
        null,
        "a hidden link is not drawn",
      );
    } finally {
      await rendered.unmount();
    }
  });

  test("the_app_passes_banner_actions_through_to_the_extension", async () => {
    const posted: unknown[] = [];
    const bannerAction = {
      id: "restart_runtime",
      label: "Restart runtime",
      enabled: true,
      emphasis: "danger",
      op: { kind: "host", effect: { kind: "restart_runtime" } },
    };
    const rendered = await renderApp(
      {
        toolbar: {
          add: { id: "add", label: "+ Add", enabled: true, emphasis: "primary" },
          discover: { id: "discover", label: "Discover", enabled: false, emphasis: "secondary" },
          add_host_slot: { id: "add_host", label: "Add host", enabled: true, emphasis: "secondary" },
        },
        computers: [],
        banner: { tone: "error", text: "A runtime is unreachable.", actions: [bannerAction] },
      },
      (message: unknown) => {
        posted.push(message);
      },
    );
    try {
      const button = rendered.container.querySelector('[data-banner-item="restart_runtime"]');
      assert.strictEqual(button.textContent, "Restart runtime");
      await act(async () => {
        button.click();
      });
      assert.deepStrictEqual(
        posted,
        [{ type: "action", action: bannerAction }],
        "the shell posts the banner action unchanged, in the envelope the toolbar uses",
      );
    } finally {
      await rendered.unmount();
    }
  });
});
