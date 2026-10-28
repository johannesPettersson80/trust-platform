#!/usr/bin/env node

// Builds the runtime's copies of the HMI drawing module and the display that uses it
// (specification 39, §11.1, §17.3). The output is committed, because the runtime embeds it; a
// VS Code test fails when it is stale.

const esbuild = require("esbuild");
const fs = require("fs");
const path = require("path");

const root = path.resolve(__dirname, "..");
const modules = path.resolve(root, "../../crates/trust-runtime/src/web/ui/modules");

async function bundle({ entry, output, globalName, source, write }) {
  const result = await esbuild.build({
    absWorkingDir: root,
    entryPoints: [entry],
    bundle: true,
    write: false,
    platform: "browser",
    target: ["es2020"],
    format: "iife",
    globalName,
    legalComments: "none",
    banner: { js: `// Generated from ${source} by \`npm run build:hmi-drawing\`; do not edit.` },
    logLevel: "silent",
  });
  const text = result.outputFiles[0].text;
  if (write) fs.writeFileSync(path.join(modules, output), text);
  return text;
}

/** The drawing module as the global `TrustHmiDrawing`. */
function buildDrawingBundle({ write }) {
  return bundle({
    entry: "src/hmiBuilder/drawing/runtimeBundle.ts",
    output: "hmi-drawing.js",
    globalName: "TrustHmiDrawing",
    source: "editors/vscode/src/hmiBuilder/drawing",
    write,
  });
}

/** The display, which starts itself on the `/hmi` page and draws with `TrustHmiDrawing`. */
function buildDisplayBundle({ write }) {
  return bundle({
    entry: "src/hmiBuilder/display/webview/runtimeDisplayMain.ts",
    output: "hmi-display.js",
    source: "editors/vscode/src/hmiBuilder/display",
    write,
  });
}

module.exports = { buildDrawingBundle, buildDisplayBundle, modules };

if (require.main === module) {
  Promise.all([buildDrawingBundle({ write: true }), buildDisplayBundle({ write: true })]).then(
    () => console.log(`✅ HMI drawing and display bundles written to ${path.relative(process.cwd(), modules)}`),
    (error) => {
      console.error("❌ HMI drawing bundle failed:", error);
      process.exit(1);
    },
  );
}
