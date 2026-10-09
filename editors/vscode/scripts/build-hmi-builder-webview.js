#!/usr/bin/env node

// Builds the HMI Builder webview (specification 39, §3.2) and copies the HMI fonts next to it, so
// the editor draws pages with the same fonts as the runtime (§3.4, §14.5 item 2).

const esbuild = require("esbuild");
const fs = require("fs");
const path = require("path");

const root = path.resolve(__dirname, "..");
const media = path.join(root, "media");
const fonts = path.resolve(root, "../../crates/trust-runtime/src/web/ui/assets/fonts");

fs.mkdirSync(path.join(media, "fonts"), { recursive: true });
for (const file of [
  "InterVariable.woff2",
  "LICENSE-INTER.txt",
  "NotoSansSC-Subset.woff2",
  "NotoSansJP-Subset.woff2",
  "NotoSansKR-Subset.woff2",
  "LICENSE-NOTO-CJK.txt",
]) {
  fs.copyFileSync(path.join(fonts, file), path.join(media, "fonts", file));
}

esbuild
  .build({
    absWorkingDir: root,
    entryPoints: ["src/hmiBuilder/editor/webview/main.tsx"],
    bundle: true,
    outfile: path.join(media, "hmiBuilderWebview.js"),
    platform: "browser",
    target: ["es2020"],
    format: "iife",
    jsx: "automatic",
    define: { "process.env.NODE_ENV": '"production"' },
    sourcemap: true,
    minify: true,
    logLevel: "info",
  })
  .then(
    () => console.log("✅ HMI Builder webview built successfully"),
    (error) => {
      console.error("❌ HMI Builder webview build failed:", error);
      process.exit(1);
    },
  );
