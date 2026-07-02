#!/usr/bin/env node

const esbuild = require("esbuild");
const path = require("path");
const fs = require("fs");

const root = path.resolve(__dirname, "..");
const outDir = path.join(root, "media");
const isWatch = process.argv.includes("--watch");

// Ensure output directory exists
if (!fs.existsSync(outDir)) {
  fs.mkdirSync(outDir, { recursive: true });
}

const buildOptions = {
  entryPoints: [path.join(root, "src", "blockly", "webview", "main.tsx")],
  bundle: true,
  outfile: path.join(outDir, "blocklyWebview.js"),
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

function stripTrailingWhitespace(filePath) {
  const source = fs.readFileSync(filePath, "utf8");
  const clean = source.replace(/[ \t]+$/gm, "");
  if (clean !== source) {
    fs.writeFileSync(filePath, clean, "utf8");
  }
}

async function build() {
  try {
    if (isWatch) {
      const ctx = await esbuild.context(buildOptions);
      await ctx.watch();
      console.log("👀 Watching for Blockly webview changes...");
    } else {
      await esbuild.build(buildOptions);
      stripTrailingWhitespace(buildOptions.outfile);
      console.log("✅ Blockly webview built successfully");
    }
  } catch (error) {
    console.error("❌ Blockly webview build failed:", error);
    process.exit(1);
  }
}

build();
