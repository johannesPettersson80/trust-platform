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

// The issues list and the shell are drawn from the Rust view model; the expected values below are
// the literals of that contract (crates/trust-devices/src/view/types/graph.rs `Issues` and
// `Issue`, design §2.1), never a value read back from a component.

/** A webview component under test, bundled the way the extension ships it. */
type WebviewComponent = React.ComponentType<Record<string, unknown>>;

/** Bundles one webview entry point and returns its named export, as the extension ships it. */
function loadComponent(component: "App" | "IssuesList"): WebviewComponent {
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
    url: "https://devices-issues.test/",
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

// An action with the fields the issues list draws (mod.rs `Action`). Each test overrides only what
// it is about, so its expected values stay visible.
function action(overrides: Record<string, unknown>): Record<string, unknown> {
  return { id: "show", label: "Show", enabled: true, emphasis: "secondary", ...overrides };
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

/** One issue with the fields the list draws (graph.rs `Issue`). */
function issue(overrides: Record<string, unknown>): Record<string, unknown> {
  return {
    id: "issue-1",
    severity: "error",
    text: "Simulator: The runtime port is already in use.",
    ...overrides,
  };
}

/** A shell view holding the toolbar, no items and the given issues. */
function issuesView(issues: Record<string, unknown>): Record<string, unknown> {
  return { toolbar: toolbar(), computers: [], issues };
}

suite("Devices & Connections panel issues list", () => {
  test("the_issues_button_shows_its_text_and_tone_as_given", async () => {
    const App = loadComponent("App");
    const issues = {
      button: { text: "1 issue", tone: "warning", faults: 0, warnings: 1 },
      groups: [],
    };
    const runtime = {
      id: "runtime-1",
      name: "Line runtime",
      state: { word: "Running", tone: "ok" },
      inspector: {
        breadcrumb: "Devices & Connections / Runtime",
        kind: "Runtime",
        title: "Line runtime",
        primary: [],
        secondary: [],
        sections: [],
      },
    };
    const view = {
      toolbar: toolbar(),
      issues,
      computers: [
        {
          id: "host-1",
          heading: "This computer",
          reachability: { word: "Reachable", tone: "ok" },
          runtimes: [runtime],
        },
      ],
    };
    const rendered = await render(App, { view, post: () => undefined });
    try {
      const button = rendered.container.querySelector('[data-role="issues-button"]');
      assert.ok(button, "the issues button is drawn");
      assert.strictEqual(
        button.textContent,
        "1 issue",
        "the button draws the view's own text, not a count of its own",
      );
      assert.strictEqual(
        button.getAttribute("data-tone"),
        "warning",
        "the button draws the view's own tone",
      );

      await act(async () => {
        rendered.container.querySelector('[data-item-id="runtime-1"]').click();
      });
      assert.ok(
        rendered.container.querySelector('[data-role="inspector"]'),
        "an item's inspector is open first",
      );
      await act(async () => {
        button.click();
      });
      assert.ok(
        rendered.container.querySelector('[data-role="issues-list"]'),
        "the issues button opens the issues list",
      );
      assert.strictEqual(
        rendered.container.querySelector('[data-role="inspector"]'),
        null,
        "the issues list is the one right-hand pane and closes the inspector",
      );
    } finally {
      await rendered.unmount();
    }
  });

  test("the_issues_list_groups_issues_by_item_in_the_given_order", async () => {
    const App = loadComponent("App");
    const issues = {
      button: { text: "4 issues", tone: "error", faults: 2, warnings: 2 },
      groups: [
        {
          item_name: "Simulator",
          issues: [
            issue({ id: "sim-1", text: "Simulator: The runtime port is already in use." }),
            issue({
              id: "sim-2",
              severity: "warning",
              text: "Simulator: the debug log is unavailable.",
            }),
          ],
        },
        {
          item: "meter",
          item_name: "Modbus meter",
          issues: [issue({ id: "meter-1", text: "Modbus meter: Unit 1 does not answer." })],
        },
        {
          item_name: "General",
          issues: [
            issue({
              id: "general-1",
              severity: "warning",
              text: "The fleet topology could not be read.",
            }),
          ],
        },
      ],
    };
    const rendered = await render(App, { view: issuesView(issues), post: () => undefined });
    try {
      await act(async () => {
        rendered.container.querySelector('[data-role="issues-button"]').click();
      });
      const names = [...rendered.container.querySelectorAll('[data-role="issue-group-name"]')].map(
        (name) => name.textContent,
      );
      assert.deepStrictEqual(
        names,
        ["Simulator", "Modbus meter", "General"],
        "the groups keep the view's own order, each named by its item",
      );
      const texts = [...rendered.container.querySelectorAll('[data-role="issue-text"]')].map(
        (text) => text.textContent,
      );
      assert.deepStrictEqual(
        texts,
        [
          "Simulator: The runtime port is already in use.",
          "Simulator: the debug log is unavailable.",
          "Modbus meter: Unit 1 does not answer.",
          "The fleet topology could not be read.",
        ],
        "every issue of every group is drawn, in the view's order, never only the first",
      );
      const severities = [
        ...rendered.container.querySelectorAll('[data-role="issue-severity"]'),
      ].map((severity) => severity.textContent);
      assert.deepStrictEqual(
        severities,
        ["error", "warning", "error", "warning"],
        "each issue draws its own severity",
      );
    } finally {
      await rendered.unmount();
    }
  });

  test("show_and_recovery_post_their_ops_unchanged", async () => {
    const App = loadComponent("App");
    const show = action({
      id: "show",
      label: "Show",
      emphasis: "secondary",
      op: { kind: "ui", ui: { kind: "focus", item: "runtime:local" } },
    });
    const recovery = action({
      id: "retry",
      label: "Retry",
      emphasis: "primary",
      op: { kind: "host", effect: { kind: "start_simulator" } },
    });
    const issues = {
      button: { text: "1 issue", tone: "error", faults: 1, warnings: 0 },
      groups: [
        {
          item: "runtime:local",
          item_name: "Simulator",
          issues: [issue({ id: "sim-1", show, recovery })],
        },
      ],
    };
    const posted: unknown[] = [];
    const rendered = await render(App, {
      view: issuesView(issues),
      post: (message: unknown) => {
        posted.push(message);
      },
    });
    try {
      await act(async () => {
        rendered.container.querySelector('[data-role="issues-button"]').click();
      });
      const cells = [...rendered.container.querySelectorAll('[data-role="issue"] [data-action]')];
      assert.deepStrictEqual(
        cells.map((cell) => cell.getAttribute("data-action")),
        ["show", "retry"],
        "`Show` comes before the recovery action, each with its own id",
      );
      await act(async () => {
        cells[0].querySelector('[data-role="action-button"]').click();
      });
      assert.deepStrictEqual(
        posted,
        [{ type: "action", action: show }],
        "`Show` posts its own op, unchanged",
      );
      await act(async () => {
        cells[1].querySelector('[data-role="action-button"]').click();
      });
      assert.deepStrictEqual(
        posted,
        [
          { type: "action", action: show },
          { type: "action", action: recovery },
        ],
        "the recovery action posts its own op, unchanged",
      );
    } finally {
      await rendered.unmount();
    }
  });

  test("the_summary_strip_opens_the_issues_list", async () => {
    const App = loadComponent("App");
    const openIssues = action({
      id: "open_issues",
      label: "Issues",
      emphasis: "quiet",
      op: { kind: "ui", ui: { kind: "open", pane: { kind: "issues" } } },
    });
    const issues = {
      button: { text: "1 issue", tone: "warning", faults: 0, warnings: 1 },
      groups: [
        {
          item_name: "General",
          issues: [issue({ id: "general-1", text: "The fleet topology could not be read." })],
        },
      ],
    };
    const rendered = await render(App, {
      view: {
        ...issuesView(issues),
        summary: { text: "1 computer · 0 faults · 1 warning", open_issues: openIssues },
      },
      post: () => undefined,
    });
    try {
      const summaryIssues = rendered.container.querySelector('[data-role="summary-issues"]');
      assert.ok(summaryIssues, "the summary strip draws the issues action the view gives it");
      assert.strictEqual(
        summaryIssues.textContent,
        "Issues",
        "the summary's issues action is drawn with its own label",
      );
      await act(async () => {
        summaryIssues.click();
      });
      assert.ok(
        rendered.container.querySelector('[data-role="issues-list"]'),
        "the summary strip opens the same issues list as the button",
      );
    } finally {
      await rendered.unmount();
    }
  });
});
