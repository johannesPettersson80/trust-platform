const fs = require("fs");
const path = require("path");
const { spawnSync } = require("child_process");

const root = path.resolve(__dirname, "..");
const repoRoot = path.resolve(root, "..", "..");
const mediaRoot = path.join(root, "media", "trust-twin");
const wasmTarget = "wasm32-unknown-unknown";
const wasmProfile = "wasm-release";
const wasmPackOut = path.join(repoRoot, "target", "trust-twin-renderer-wasm-pack");
const generatedJs = path.join(wasmPackOut, "trust_twin_renderer.js");
const generatedWasm = path.join(wasmPackOut, "trust_twin_renderer_bg.wasm");
const generatedSnippets = path.join(wasmPackOut, "snippets");
const mediaSnippets = path.join(mediaRoot, "snippets");
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
  if (result.error) {
    throw new Error(`${command} ${args.join(" ")} failed to spawn: ${result.error.message}`);
  }
  if (result.status !== 0) {
    const reason = result.signal ? `signal ${result.signal}` : `status ${result.status}`;
    throw new Error(`${command} ${args.join(" ")} failed with ${reason}`);
  }
}

function wasmBuildEnv() {
  const env = { ...process.env };
  for (const key of ["RUSTFLAGS", "CARGO_BUILD_RUSTFLAGS"]) {
    if (env[key]) {
      console.warn(`Ignoring ${key} for ${wasmTarget} build; host linker flags are not wasm-safe.`);
      delete env[key];
    }
  }
  env.CARGO_ENCODED_RUSTFLAGS = "";
  return env;
}

if (process.env.TRUST_TWIN_SKIP_WASM_TARGET_INSTALL !== "1") {
  run("rustup", ["target", "add", wasmTarget]);
}
const wasmEnv = wasmBuildEnv();
fs.rmSync(wasmPackOut, { recursive: true, force: true });
run("wasm-pack", [
  "build",
  path.join(repoRoot, "crates", "trust-twin-renderer"),
  "--target",
  "web",
  "--profile",
  wasmProfile,
  "--no-opt",
  "--no-typescript",
  "--out-dir",
  wasmPackOut,
  "--out-name",
  "trust_twin_renderer",
], { env: wasmEnv });

if (!fs.existsSync(generatedWasm) || !fs.existsSync(generatedJs)) {
  throw new Error(`Missing wasm-bindgen trust-twin renderer output in ${wasmPackOut}`);
}
fs.copyFileSync(generatedWasm, path.join(mediaRoot, "trust-twin-renderer.wasm"));
const loader = fs
  .readFileSync(generatedJs, "utf8")
  .replace(/trust_twin_renderer_bg\.wasm/g, "trust-twin-renderer.wasm");
fs.writeFileSync(path.join(mediaRoot, "trust-twin-renderer.js"), loader, "utf8");
fs.rmSync(mediaSnippets, { recursive: true, force: true });
if (fs.existsSync(generatedSnippets)) {
  copyTree(generatedSnippets, mediaSnippets);
}

fs.rmSync(componentDest, { recursive: true, force: true });
fs.mkdirSync(componentDest, { recursive: true });
copyAssetTree(componentSource, componentDest);

const assets = [
  "trust-twin-renderer.wasm",
  "trust-twin-renderer.js",
  ...listAssetFiles(mediaSnippets).map((name) => `snippets/${name}`),
  ...listAssetFiles(componentDest).map((name) => `components/${name}`),
];
console.log(`Wrote media/trust-twin (${assets.length} assets)`);

function copyAssetTree(source, dest) {
  for (const entry of fs.readdirSync(source, { withFileTypes: true })) {
    const sourcePath = path.join(source, entry.name);
    const destPath = path.join(dest, entry.name);
    if (entry.isDirectory()) {
      fs.mkdirSync(destPath, { recursive: true });
      copyAssetTree(sourcePath, destPath);
      continue;
    }
    if (isPackagedAssetFile(entry.name)) {
      fs.copyFileSync(sourcePath, destPath);
    }
  }
}

function copyTree(source, dest) {
  fs.mkdirSync(dest, { recursive: true });
  for (const entry of fs.readdirSync(source, { withFileTypes: true })) {
    const sourcePath = path.join(source, entry.name);
    const destPath = path.join(dest, entry.name);
    if (entry.isDirectory()) {
      copyTree(sourcePath, destPath);
    } else {
      fs.copyFileSync(sourcePath, destPath);
    }
  }
}

function listAssetFiles(rootDir, relativeDir = "") {
  if (!fs.existsSync(rootDir)) {
    return [];
  }
  const files = [];
  for (const entry of fs.readdirSync(path.join(rootDir, relativeDir), { withFileTypes: true })) {
    const relativePath = path.join(relativeDir, entry.name);
    if (entry.isDirectory()) {
      files.push(...listAssetFiles(rootDir, relativePath));
    } else {
      files.push(relativePath.split(path.sep).join("/"));
    }
  }
  return files.sort();
}

function isPackagedAssetFile(name) {
  return [".gltf", ".glb", ".bin", ".ktx2", ".png"].includes(path.extname(name));
}
