const fs = require("fs");
const path = require("path");

const root = path.resolve(__dirname, "..");
const maxWebgl2TextureDimension = 2048;
const required = [
  "media/trust-twin/trust-twin-renderer.wasm",
  "media/trust-twin/trust-twin-renderer.js",
  "media/trust-twin/components/motor.gltf",
  "media/trust-twin/components/pump.gltf",
  "media/trust-twin/components/valve.gltf",
  "media/trust-twin/components/ur10/visual/base.gltf",
  "media/trust-twin/components/ur10/visual/base.bin",
  "media/trust-twin/components/ur10/visual/shoulder.gltf",
  "media/trust-twin/components/ur10/visual/shoulder.bin",
  "media/trust-twin/components/ur10/visual/upperarm.gltf",
  "media/trust-twin/components/ur10/visual/upperarm.bin",
  "media/trust-twin/components/ur10/visual/forearm.gltf",
  "media/trust-twin/components/ur10/visual/forearm.bin",
  "media/trust-twin/components/ur10/visual/wrist1.gltf",
  "media/trust-twin/components/ur10/visual/wrist1.bin",
  "media/trust-twin/components/ur10/visual/wrist2.gltf",
  "media/trust-twin/components/ur10/visual/wrist2.bin",
  "media/trust-twin/components/ur10/visual/wrist3.gltf",
  "media/trust-twin/components/ur10/visual/wrist3.bin",
  "media/trust-twin/components/schunk-wsg50/meshes/wsg_body.gltf",
  "media/trust-twin/components/schunk-wsg50/meshes/wsg_body.bin",
  "media/trust-twin/components/schunk-wsg50/meshes/finger_with_tip.gltf",
  "media/trust-twin/components/schunk-wsg50/meshes/finger_with_tip.bin",
  "media/trust-twin/components/ycb/meshes/003_cracker_box_textured.gltf",
  "media/trust-twin/components/ycb/meshes/003_cracker_box_textured.bin",
  "media/trust-twin/components/ycb/meshes/003_cracker_box_textured.png",
  "media/trust-twin/components/ycb/meshes/005_tomato_soup_can_textured.gltf",
  "media/trust-twin/components/ycb/meshes/005_tomato_soup_can_textured.bin",
  "media/trust-twin/components/ycb/meshes/005_tomato_soup_can_textured.png",
  "media/trust-twin/components/manipulation-station/assets/table_wide.gltf",
  "media/trust-twin/components/manipulation-station/assets/table_wide.bin",
  "media/trust-twin/components/manipulation-station/assets/table_wide.png",
];
const packagedAssets = new Set(required);
const packageErrors = [];

for (const relativePath of required.filter((entry) => entry.endsWith(".gltf"))) {
  for (const referencedPath of referencedAssetPaths(relativePath)) {
    packagedAssets.add(referencedPath);
  }
}

const missing = [...packagedAssets].filter((relativePath) => !fs.existsSync(path.join(root, relativePath)));
const rendererJs = fs.readFileSync(
  path.join(root, "media/trust-twin/trust-twin-renderer.js"),
  "utf8",
);
if (rendererJs.includes("./snippets/")) {
  const snippetsRoot = path.join(root, "media/trust-twin/snippets");
  const snippetFiles = fs.existsSync(snippetsRoot) ? listFiles(snippetsRoot) : [];
  if (!snippetFiles.some((entry) => entry.endsWith(".js"))) {
    missing.push("media/trust-twin/snippets/**/*.js");
  }
  for (const file of snippetFiles) {
    packagedAssets.add(`media/trust-twin/snippets/${file}`);
  }
}
const vscodeIgnore = fs.readFileSync(path.join(root, ".vscodeignore"), "utf8");
if (!vscodeIgnore.includes("!media/trust-twin/**")) {
  missing.push(".vscodeignore allowlist !media/trust-twin/**");
}

for (const relativePath of [...packagedAssets].filter((entry) => entry.endsWith(".png"))) {
  const absolutePath = path.join(root, relativePath);
  if (!fs.existsSync(absolutePath)) {
    continue;
  }
  const dimensions = readPngDimensions(absolutePath);
  if (
    dimensions.width > maxWebgl2TextureDimension ||
    dimensions.height > maxWebgl2TextureDimension
  ) {
    packageErrors.push(
      `${relativePath} is ${dimensions.width}x${dimensions.height}; max WebGL2-safe packaged texture size is ${maxWebgl2TextureDimension}x${maxWebgl2TextureDimension}`,
    );
  }
}

if (missing.length > 0 || packageErrors.length > 0) {
  console.error("trust-twin package asset smoke failed:");
  for (const entry of missing) {
    console.error(`- ${entry}`);
  }
  for (const entry of packageErrors) {
    console.error(`- ${entry}`);
  }
  process.exit(1);
}

console.log(
  JSON.stringify(
    {
      ok: true,
      max_webgl2_texture_dimension: maxWebgl2TextureDimension,
      assets: [...packagedAssets].sort(),
    },
    null,
    2,
  ),
);

function listFiles(rootDir, relativeDir = "") {
  const files = [];
  for (const entry of fs.readdirSync(path.join(rootDir, relativeDir), { withFileTypes: true })) {
    const relativePath = path.join(relativeDir, entry.name);
    if (entry.isDirectory()) {
      files.push(...listFiles(rootDir, relativePath));
    } else {
      files.push(relativePath.split(path.sep).join("/"));
    }
  }
  return files.sort();
}

function referencedAssetPaths(gltfRelativePath) {
  const absolutePath = path.join(root, gltfRelativePath);
  const document = JSON.parse(fs.readFileSync(absolutePath, "utf8"));
  const references = [];
  for (const buffer of document.buffers || []) {
    if (typeof buffer.uri === "string" && isPackagedAssetUri(buffer.uri)) {
      references.push(resolveRelativeAssetPath(gltfRelativePath, buffer.uri));
    }
  }
  for (const image of document.images || []) {
    if (typeof image.uri === "string" && isPackagedAssetUri(image.uri)) {
      references.push(resolveRelativeAssetPath(gltfRelativePath, image.uri));
    }
  }
  return references;
}

function isPackagedAssetUri(uri) {
  return (
    !uri.startsWith("data:") &&
    !uri.startsWith("/") &&
    !/^[a-z][a-z0-9+.-]*:/i.test(uri)
  );
}

function resolveRelativeAssetPath(gltfRelativePath, uri) {
  const normalized = path.posix.normalize(
    path.posix.join(path.posix.dirname(gltfRelativePath), uri),
  );
  if (normalized.startsWith("../")) {
    packageErrors.push(`${gltfRelativePath} references escaping asset URI ${uri}`);
  }
  return normalized;
}

function readPngDimensions(filePath) {
  const bytes = fs.readFileSync(filePath);
  const signature = "89504e470d0a1a0a";
  if (bytes.length < 24 || bytes.subarray(0, 8).toString("hex") !== signature) {
    throw new Error(`${path.relative(root, filePath)} is not a PNG file`);
  }
  if (bytes.subarray(12, 16).toString("ascii") !== "IHDR") {
    throw new Error(`${path.relative(root, filePath)} is missing a PNG IHDR chunk`);
  }
  return {
    width: bytes.readUInt32BE(16),
    height: bytes.readUInt32BE(20),
  };
}
