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

function loadAdsMultiPortTagBrowser(): React.ComponentType<Record<string, unknown>> {
  const extensionRoot = path.resolve(__dirname, "..", "..", "..");
  const temp = fs.mkdtempSync(
    path.join(extensionRoot, "node_modules", ".trust-ads-tag-browser-test-"),
  );
  const outfile = path.join(temp, "browser.cjs");
  try {
    buildSync({
      entryPoints: [
        path.join(
          extensionRoot,
          "src",
          "networkCanvas",
          "webview",
          "AdsMultiPortTagBrowser.tsx",
        ),
      ],
      outfile,
      bundle: true,
      platform: "node",
      format: "cjs",
      external: ["react", "react-dom", "vscode"],
      logLevel: "silent",
    });
    return require(outfile).AdsMultiPortTagBrowser as React.ComponentType<
      Record<string, unknown>
    >;
  } finally {
    fs.rmSync(temp, { recursive: true, force: true });
  }
}

suite("ADS tag rendered selection", () => {
  test("a successful ads.toml add keeps the tag checked and reversible", async () => {
    const dom = new JSDOM("<!doctype html><body><div id=\"root\"></div></body>", {
      url: "https://ads-tags.test/",
    });
    const globals = globalThis as any;
    const previousWindow = globals.window;
    const previousDocument = globals.document;
    const previousActEnvironment = globals.IS_REACT_ACT_ENVIRONMENT;
    globals.window = dom.window;
    globals.document = dom.window.document;
    globals.IS_REACT_ACT_ENVIRONMENT = true;
    const AdsMultiPortTagBrowser = loadAdsMultiPortTagBrowser();
    const container = dom.window.document.getElementById("root");
    assert.ok(container);
    const addCalls: unknown[][] = [];
    const removeCalls: unknown[][] = [];
    let root: Root | undefined;
    try {
      root = createRoot(container);
      const renderBrowser = async (
        importResult?: Record<string, unknown>,
        persisted = false,
        key = "browser",
        treeAvailable = true,
      ) => {
        await act(async () => {
          root?.render(
            React.createElement(AdsMultiPortTagBrowser, {
              key,
              targetLabel: "TwinCAT test target",
              target: {
                host: "192.168.77.11",
                target_net_id: "100.67.6.217.1.1",
                ams_port: 301,
                imported_ads_symbols: persisted
                  ? [{ port: 301, paths: ["Task 4.Inputs.Var 1"] }]
                  : [],
              },
              tree: treeAvailable
                ? [
                    {
                      id: "task-4-inputs-var-1",
                      name: "Var 1",
                      path: "Task 4.Inputs.Var 1",
                      data_type: "USINT",
                      writable: true,
                    },
                    {
                      id: "task-4-inputs-var-2",
                      name: "Var 2",
                      path: "Task 4.Inputs.Var 2",
                      data_type: "USINT",
                      writable: true,
                    },
                  ]
                : undefined,
              routeMissing: false,
              loading: !treeAvailable,
              importLoading: false,
              importResult,
              onCreateRoute: () => undefined,
              onCopy: () => undefined,
              onBrowseTarget: () => undefined,
              onAddTags: (...args: unknown[]) => addCalls.push(args),
              onRemoveTag: (...args: unknown[]) => removeCalls.push(args),
              onClose: () => undefined,
            }),
          );
        });
      };
      await renderBrowser();

      const checkbox: any = container.querySelector(
        'input[data-role="symbol-selection"]',
      );
      assert.ok(checkbox, "Task 4 / Inputs / Var 1 must keep a checkbox");
      assert.strictEqual(checkbox.checked, false, "a tag not in ads.toml starts unchecked");

      await act(async () => checkbox.click());
      assert.strictEqual(checkbox.checked, true, "checking selects the tag to add");
      assert.deepStrictEqual(
        addCalls,
        [[
          [{ port: 301, paths: ["Task 4.Inputs.Var 1"] }],
          false,
          "Task 4.Inputs.Var 1",
        ]],
        "checking an unconfigured tag must save it immediately",
      );
      assert.strictEqual(
        [...container.querySelectorAll("button")].some((button) =>
          button.textContent?.startsWith("Add selected tags"),
        ),
        false,
        "the immediate checkbox workflow must not keep a separate Add selected tags button",
      );

      await renderBrowser({
        applied: true,
        addedCount: 1,
        restartRequired: true,
        ports: [
          {
            port: 301,
            paths: ["Task 4.Inputs.Var 1"],
            applied: true,
            addedCount: 1,
            message: "Added.",
          },
        ],
      });

      assert.strictEqual(
        checkbox.checked,
        true,
        "the checkbox must stay checked after the tag is written to ads.toml",
      );
      assert.strictEqual(
        checkbox.disabled,
        false,
        "a persisted ADS tag must remain reversible",
      );
      assert.ok(
        container.querySelector('[data-role="added-symbol-status"]'),
        "the persisted tag must also show Added status",
      );

      await act(async () => checkbox.click());
      assert.strictEqual(checkbox.checked, false, "clicking a persisted tag must uncheck it");
      assert.strictEqual(
        Boolean(container.querySelector('[data-role="added-symbol-status"]')),
        false,
        "an unchecked tag must not still say Added",
      );
      assert.deepStrictEqual(
        removeCalls,
        [[{ port: 301, path: "Task 4.Inputs.Var 1" }]],
        "unchecking a configured tag must remove it immediately",
      );

      await renderBrowser({
        operation: "remove",
        applied: false,
        addedCount: 0,
        removedCount: 0,
        restartRequired: false,
        ports: [
          {
            port: 301,
            paths: ["Task 4.Inputs.Var 1"],
            applied: false,
            addedCount: 0,
            message: "Could not save removal.",
          },
        ],
      });
      assert.strictEqual(
        checkbox.checked,
        true,
        "a failed removal must restore the configured checkbox",
      );
      assert.ok(
        container.querySelector('[data-role="added-symbol-status"]'),
        "a failed removal must restore Added",
      );

      await act(async () => checkbox.click());
      await renderBrowser({
        operation: "remove",
        applied: true,
        addedCount: 0,
        removedCount: 1,
        restartRequired: true,
        ports: [
          {
            port: 301,
            paths: ["Task 4.Inputs.Var 1"],
            applied: true,
            addedCount: 0,
            message: "Removed.",
          },
        ],
      });
      assert.strictEqual(checkbox.checked, false, "a successful removal stays unchecked");
      assert.strictEqual(
        Boolean(container.querySelector('[data-role="added-symbol-status"]')),
        false,
        "a successfully removed tag is no longer Added",
      );

      await act(async () => checkbox.click());
      assert.strictEqual(
        Boolean(container.querySelector('[data-role="added-symbol-status"]')),
        false,
        "checking a removed tag must wait for save success before showing Added",
      );

      await renderBrowser(undefined, true, "reopened-browser");
      const reopenedCheckbox: any = container.querySelector(
        'input[data-role="symbol-selection"]',
      );
      assert.strictEqual(
        reopenedCheckbox.checked,
        true,
        "a tag already present in ads.toml must reopen as checked",
      );
      assert.strictEqual(
        reopenedCheckbox.disabled,
        false,
        "a tag loaded from ads.toml must remain reversible",
      );

      await renderBrowser(undefined, true, "async-browser", false);
      assert.strictEqual(
        container.querySelector('input[data-role="symbol-selection"]'),
        null,
        "the port tree has not arrived yet",
      );

      await renderBrowser(undefined, true, "async-browser", true);
      const asynchronouslyLoadedCheckbox: any = container.querySelector(
        'input[data-role="symbol-selection"]',
      );
      assert.ok(asynchronouslyLoadedCheckbox, "the asynchronously loaded tag must render");
      assert.strictEqual(
        asynchronouslyLoadedCheckbox.checked,
        true,
        "a tag already in ads.toml must become checked when its port tree arrives",
      );

      const secondCheckbox: any = [...container.querySelectorAll(
        'input[data-role="symbol-selection"]',
      )].find((input: any) => input.getAttribute("aria-label") === "Select Task 4.Inputs.Var 2");
      assert.ok(secondCheckbox, "the second tag must render");
      await act(async () => secondCheckbox.click());
      assert.deepStrictEqual(
        addCalls[addCalls.length - 1],
        [[{
          port: 301,
          paths: ["Task 4.Inputs.Var 1", "Task 4.Inputs.Var 2"],
        }], false, "Task 4.Inputs.Var 2"],
        "checking another tag must save the complete configured set for that ADS port",
      );
    } finally {
      if (root) {
        await act(async () => root?.unmount());
      }
      globals.window = previousWindow;
      globals.document = previousDocument;
      globals.IS_REACT_ACT_ENVIRONMENT = previousActEnvironment;
      dom.window.close();
    }
  });
});
