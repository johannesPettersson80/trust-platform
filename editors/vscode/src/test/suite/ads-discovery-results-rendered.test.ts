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

function loadDiscoverPane(): React.ComponentType<Record<string, unknown>> {
  const extensionRoot = path.resolve(__dirname, "..", "..", "..");
  const temp = fs.mkdtempSync(
    path.join(extensionRoot, "node_modules", ".trust-ads-discover-pane-test-"),
  );
  const outfile = path.join(temp, "discover-pane.cjs");
  try {
    buildSync({
      entryPoints: [
        path.join(
          extensionRoot,
          "src",
          "networkCanvas",
          "webview",
          "DiscoverPane.tsx",
        ),
      ],
      outfile,
      bundle: true,
      platform: "node",
      format: "cjs",
      external: ["react", "react-dom", "vscode"],
      logLevel: "silent",
    });
    return require(outfile).DiscoverPane as React.ComponentType<Record<string, unknown>>;
  } finally {
    fs.rmSync(temp, { recursive: true, force: true });
  }
}

suite("ADS discovery rendered results", () => {
  test("an identity without a verified ADS port never renders a fallback card", async () => {
    const dom = new JSDOM("<!doctype html><body><div id=\"root\"></div></body>", {
      url: "https://ads-discovery.test/",
    });
    const globals = globalThis as any;
    const previousWindow = globals.window;
    const previousDocument = globals.document;
    const previousActEnvironment = globals.IS_REACT_ACT_ENVIRONMENT;
    globals.window = dom.window;
    globals.document = dom.window.document;
    globals.IS_REACT_ACT_ENVIRONMENT = true;
    const DiscoverPane = loadDiscoverPane();
    const container = dom.window.document.getElementById("root");
    assert.ok(container);
    let root: Root | undefined;
    try {
      root = createRoot(container);
      await act(async () => {
        root?.render(
          React.createElement(DiscoverPane, {
            origins: [{ id: "this_host", label: "This computer" }],
            discoverProtocols: new Set(["ads"]),
            scanning: false,
            progress: [{ protocol: "ads", label: "ADS", status: "done", count: 0 }],
            results: [
              {
                id: "ads:100.67.6.217.1.1",
                label: "TwinCAT 100.67.6.217.1.1",
                source: "ads_identify",
                confidence: "observed",
                protocol: "ads",
                params: {
                  host: "127.0.0.1",
                  ams_net_id: "100.67.6.217.1.1",
                  ams_port: 851,
                  responding_ads_ports: [],
                },
              },
            ],
            sessionCurrent: true,
            onScan: () => undefined,
            onAdd: () => undefined,
            isOnCanvas: () => false,
            onAdopt: () => undefined,
            onOpenAdsPortSettings: () => undefined,
            onClose: () => undefined,
          }),
        );
      });

      assert.strictEqual(
        container.textContent?.includes("TwinCAT 100.67.6.217.1.1"),
        false,
        "identity-only ADS results are not usable devices",
      );
      assert.strictEqual(
        container.textContent?.includes("No configured ADS ports responded"),
        false,
        "the removed fallback message must never be rendered",
      );
      assert.strictEqual(
        [...container.querySelectorAll("button")].some(
          (button) => button.textContent === "Add to canvas",
        ),
        false,
        "an unverified ADS identity must not offer an action",
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
