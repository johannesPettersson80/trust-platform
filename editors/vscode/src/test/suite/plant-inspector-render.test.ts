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

// The plant's facts are Rust's (`plant.topology`, specification 47 §20.6, trust-runtime's
// `control/plant_handlers/topology/tests.rs`); these tests prove that the inspector shows them as
// given. The node data below is what the canvas layout builds from a projected plant.

/** The plant inspector, bundled the way the extension ships the canvas webview. */
function loadPlantInspector(): React.ComponentType<Record<string, unknown>> {
  const extensionRoot = path.resolve(__dirname, "..", "..", "..");
  const temp = fs.mkdtempSync(path.join(extensionRoot, "node_modules", ".trust-plant-inspector-test-"));
  const outfile = path.join(temp, "plant-inspector.cjs");
  try {
    buildSync({
      entryPoints: [path.join(extensionRoot, "src", "networkCanvas", "webview", "PlantInspector.tsx")],
      outfile,
      bundle: true,
      platform: "node",
      format: "cjs",
      external: ["react", "react-dom", "vscode"],
      logLevel: "silent",
    });
    return require(outfile).PlantInspector as React.ComponentType<Record<string, unknown>>;
  } finally {
    fs.rmSync(temp, { recursive: true, force: true });
  }
}

/** Renders the inspector for `data` and hands its container to `check`. */
async function withInspector(
  data: Record<string, unknown>,
  props: Record<string, unknown>,
  check: (container: HTMLElement) => void | Promise<void>,
): Promise<void> {
  const dom = new JSDOM("<!doctype html><body><div id=\"root\"></div></body>", {
    url: "https://plant-inspector.test/",
  });
  const globals = globalThis as any;
  const previousWindow = globals.window;
  const previousDocument = globals.document;
  const previousActEnvironment = globals.IS_REACT_ACT_ENVIRONMENT;
  globals.window = dom.window;
  globals.document = dom.window.document;
  globals.IS_REACT_ACT_ENVIRONMENT = true;
  const PlantInspector = loadPlantInspector();
  const container = dom.window.document.getElementById("root");
  assert.ok(container);
  let root: Root | undefined;
  try {
    root = createRoot(container);
    await act(async () => {
      root?.render(
        React.createElement(PlantInspector, {
          node: { id: "plant:tank", type: "endpoint", data },
          onFocus: () => undefined,
          onClose: () => undefined,
          ...props,
        }),
      );
    });
    await check(container);
  } finally {
    if (root) {
      await act(async () => root?.unmount());
    }
    globals.window = previousWindow;
    globals.document = previousDocument;
    globals.IS_REACT_ACT_ENVIRONMENT = previousActEnvironment;
    dom.window.close();
  }
}

/** Each inspector row's label and value, as shown. */
function rows(container: HTMLElement): Record<string, string> {
  return Object.fromEntries(
    [...container.querySelectorAll("[data-plant-inspector-row]")].map((row) => [
      row.getAttribute("data-plant-inspector-row") ?? "",
      row.lastElementChild?.textContent ?? "",
    ]),
  );
}

const PLANT = {
  kind: "plant",
  name: "Tank level",
  displayName: "Tank level",
  plantId: "tank_level-1",
  health: "error",
  modelName: "First-order lag",
  commandSources: ["%QW2", "%QW0"],
  feedbackTargets: ["%IW4"],
  savedConfigurationRevision: "sha256:saved-b",
  configuration: { parameters: { gain: 2, time_constant_s: [1.5, 3] } },
};

suite("Plant inspector", () => {
  // Specification 47 §5.1 and §5.2: the exact persisted ID, the first state as the state and every
  // other true condition as a detail, the complete mapping in its order with the live values beside
  // it, the parameters, and the saved configuration apart from the running one. Catches a dropped
  // secondary state (a fault hidden behind a revision mismatch), a humanised ID, a reordered mapping
  // and saved and running configurations shown as one.
  test("a plant's inspector shows every state, its exact ID, its ordered mapping and both configurations as given", async () => {
    await withInspector(
      {
        ...PLANT,
        state: "Faulted",
        secondaryStates: ["Restart required", "Running"],
        liveSample: { command: [12.5, 0], feedback: [3] },
        activeConfigurationRevision: "sha256:active-a",
      },
      {},
      (container) => {
        assert.deepStrictEqual(rows(container), {
          "Plant ID": "tank_level-1",
          State: "Faulted",
          "Other states": "Restart required\nRunning",
          Model: "First-order lag",
          Command: "%QW2 = 12.5\n%QW0 = 0",
          Feedback: "%IW4 = 3",
          "Saved configuration": "sha256:saved-b",
          "Running configuration": "sha256:active-a",
          Parameters: "gain = 2\ntime_constant_s = [1.5,3]",
        });
        assert.strictEqual(container.querySelector("[aria-label='Plant summary'] strong")?.textContent, "Tank level");
      },
    );
  });

  // Specification 47 §5.3 and §20.6: without current runtime evidence Rust gives no live sample and no
  // running revision. Catches invented live values, and the saved revision shown as the running one.
  test("without live evidence the mapping stands alone and no running configuration is reported", async () => {
    await withInspector({ ...PLANT, state: "Status unavailable", secondaryStates: [] }, {}, (container) => {
      const shown = rows(container);
      assert.strictEqual(shown.Command, "%QW2\n%QW0");
      assert.strictEqual(shown.Feedback, "%IW4");
      assert.strictEqual(shown["Running configuration"], "None reported");
      assert.strictEqual(shown["Other states"], undefined, "no other states, no row");
    });
  });

  // A content revision is 71 characters; the inspector shows its short form, like a short Git SHA, and
  // keeps the whole value on hover. Catches a revision that the panel cannot show whole anywhere.
  test("a long revision shows its first 12 digits and keeps the whole value on hover", async () => {
    const revision = `sha256:${"0123456789abcdef".repeat(4)}`;
    await withInspector({ ...PLANT, state: "Configured", savedConfigurationRevision: revision }, {}, (container) => {
      const value = container.querySelector("[data-plant-inspector-row='Saved configuration']")?.lastElementChild;
      assert.strictEqual(value?.textContent, "sha256:0123456789ab…");
      assert.strictEqual(value?.getAttribute("title"), revision);
    });
  });

  // Specification 47 §6.1: Configure plant opens the setup editor. Catches an editor that the
  // inspector cannot reach, and a dead button for a plant whose saved configuration cannot be opened.
  test("Configure plant opens the editor, and is disabled when the configuration cannot be opened", async () => {
    let opened = 0;
    await withInspector({ ...PLANT, state: "Configured" }, { onConfigure: () => (opened += 1) }, async (container) => {
      const configure = container.querySelector<HTMLButtonElement>("[data-plant-configure]");
      assert.ok(configure && !configure.disabled);
      await act(async () => configure.click());
      assert.strictEqual(opened, 1);
    });
    await withInspector({ ...PLANT, state: "Configured" }, {}, (container) => {
      assert.strictEqual(container.querySelector<HTMLButtonElement>("[data-plant-configure]")?.disabled, true);
    });
  });
});
