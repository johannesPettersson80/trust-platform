#!/usr/bin/env node

const esbuild = require("esbuild");
const path = require("path");

const root = path.resolve(__dirname, "..");

esbuild.build({
  entryPoints: {
    controlStudioChart: path.join(root, "src", "controlStudio", "webview", "threePageChart.ts"),
    controlStudioWorkflow: path.join(root, "src", "controlStudio", "webview", "threePageWorkflow.ts"),
  },
  bundle: true,
  outdir: path.join(root, "media"),
  platform: "browser",
  target: ["es2020"],
  format: "iife",
  sourcemap: true,
  minify: true,
  logLevel: "info",
}).then(
  () => console.log("✅ Control Studio chart built successfully"),
  (error) => {
    console.error("❌ Control Studio chart build failed:", error);
    process.exit(1);
  }
);
