import test from "node:test";
import assert from "node:assert/strict";
import fs from "node:fs";
import path from "node:path";

const demoSourcePath = path.resolve(process.cwd(), "docs/demo/demo.js");
const demoSource = fs.readFileSync(demoSourcePath, "utf8");

test("loadWasmClient marks ready state after ready() resolves", () => {
  assert.match(
    demoSource,
    /await wasmClient\.ready\(\);\s*[\s\S]*markWasmReady\(\);/,
    "expected loadWasmClient to force-ready UI after awaiting wasmClient.ready()",
  );
});

test("ready status handler reuses markWasmReady helper", () => {
  assert.match(
    demoSource,
    /if \(status\.type === "ready"\)\s*{\s*markWasmReady\(\);\s*}/,
    "expected status ready handler to use shared markWasmReady helper",
  );
});
