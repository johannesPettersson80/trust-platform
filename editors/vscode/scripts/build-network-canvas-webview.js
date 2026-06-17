#!/usr/bin/env node

const esbuild = require("esbuild");
const path = require("path");
const fs = require("fs");

const root = path.resolve(__dirname, "..");
const outDir = path.join(root, "media");
const isWatch = process.argv.includes("--watch");

if (!fs.existsSync(outDir)) {
  fs.mkdirSync(outDir, { recursive: true });
}

const buildOptions = {
  entryPoints: [path.join(root, "src", "networkCanvas", "webview", "main.tsx")],
  bundle: true,
  outfile: path.join(outDir, "networkCanvasWebview.js"),
  platform: "browser",
  target: ["es2020"],
  format: "iife",
  sourcemap: true,
  minify: !isWatch,
  loader: {
    ".tsx": "tsx",
    ".ts": "ts",
    ".jsx": "jsx",
    ".js": "js",
    ".css": "css",
  },
  define: {
    "process.env.NODE_ENV": isWatch ? '"development"' : '"production"',
  },
  logLevel: "info",
};

async function build() {
  try {
    if (isWatch) {
      const ctx = await esbuild.context(buildOptions);
      await ctx.watch();
      console.log("👀 Watching for Network Canvas changes...");
    } else {
      await esbuild.build(buildOptions);
      console.log("✅ Network Canvas webview built successfully");
    }
  } catch (error) {
    console.error("❌ Network Canvas webview build failed:", error);
    process.exit(1);
  }
}

build();
