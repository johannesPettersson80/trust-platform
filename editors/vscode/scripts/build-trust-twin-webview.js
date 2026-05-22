const fs = require("fs");
const path = require("path");
const { spawnSync } = require("child_process");

const root = path.resolve(__dirname, "..");
const repoRoot = path.resolve(root, "..", "..");
const mediaRoot = path.join(root, "media", "trust-twin");
const wasmTarget = "wasm32-unknown-unknown";
const wasmSource = path.join(
  repoRoot,
  "target",
  wasmTarget,
  "release",
  "trust_twin_renderer.wasm",
);
const componentSource = path.join(
  repoRoot,
  "crates",
  "trust-twin-compiler",
  "library",
  "v1",
  "assets",
  "trust-twin",
  "components",
);
const componentDest = path.join(mediaRoot, "components");

fs.mkdirSync(componentDest, { recursive: true });

function run(command, args, options = {}) {
  const result = spawnSync(command, args, {
    cwd: repoRoot,
    stdio: "inherit",
    ...options,
  });
  if (result.status !== 0) {
    throw new Error(`${command} ${args.join(" ")} failed with status ${result.status}`);
  }
}

if (process.env.TRUST_TWIN_SKIP_WASM_TARGET_INSTALL !== "1") {
  run("rustup", ["target", "add", wasmTarget]);
}
run("cargo", [
  "build",
  "-p",
  "trust-twin-renderer",
  "--target",
  wasmTarget,
  "--release",
]);

if (!fs.existsSync(wasmSource)) {
  throw new Error(`Missing compiled trust-twin renderer WASM: ${wasmSource}`);
}
fs.copyFileSync(wasmSource, path.join(mediaRoot, "trust-twin-renderer.wasm"));

const loader = `"use strict";
(function () {
  const script = document.currentScript;
  const wasmUri = script ? script.getAttribute("data-wasm-uri") : "";
  async function initialize() {
    if (!wasmUri || typeof WebAssembly === "undefined") {
      window.dispatchEvent(new CustomEvent("trustTwinRendererWasmReady", {
        detail: { ok: false, error: "wasm unavailable" },
      }));
      return;
    }
    try {
      const response = await fetch(wasmUri);
      const bytes = await response.arrayBuffer();
      await WebAssembly.instantiate(bytes, {});
      const version = 1;
      window.dispatchEvent(new CustomEvent("trustTwinRendererWasmReady", {
        detail: { ok: version === 1, renderer: "trust-twin-renderer", contract: version },
      }));
    } catch (error) {
      window.dispatchEvent(new CustomEvent("trustTwinRendererWasmReady", {
        detail: { ok: false, error: String(error && error.message ? error.message : error) },
      }));
    }
  }
  void initialize();
}());
`;
fs.writeFileSync(path.join(mediaRoot, "trust-twin-renderer.js"), loader, "utf8");

for (const name of fs.readdirSync(componentSource)) {
  if (!name.endsWith(".gltf")) {
    continue;
  }
  fs.copyFileSync(path.join(componentSource, name), path.join(componentDest, name));
}

const assets = [
  "trust-twin-renderer.wasm",
  "trust-twin-renderer.js",
  ...fs.readdirSync(componentDest).map((name) => `components/${name}`),
];
console.log(`Wrote media/trust-twin (${assets.length} assets)`);
