const fs = require("fs");
const path = require("path");

const root = path.resolve(__dirname, "..");
const required = [
  "media/trust-twin/trust-twin-renderer.wasm",
  "media/trust-twin/trust-twin-renderer.js",
  "media/trust-twin/components/motor.gltf",
  "media/trust-twin/components/pump.gltf",
  "media/trust-twin/components/valve.gltf",
];
const missing = required.filter((relativePath) => !fs.existsSync(path.join(root, relativePath)));
const vscodeIgnore = fs.readFileSync(path.join(root, ".vscodeignore"), "utf8");
if (!vscodeIgnore.includes("!media/trust-twin/**")) {
  missing.push(".vscodeignore allowlist !media/trust-twin/**");
}

if (missing.length > 0) {
  console.error("trust-twin package asset smoke failed:");
  for (const entry of missing) {
    console.error(`- ${entry}`);
  }
  process.exit(1);
}

console.log(
  JSON.stringify(
    {
      ok: true,
      assets: required,
    },
    null,
    2,
  ),
);
