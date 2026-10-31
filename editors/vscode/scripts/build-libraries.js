// Package library source and visuals from their single repository owners: each packaged name and the folder of
// `libraries/` it comes from. PLCopen Motion ships its single-axis core.
const fs = require("fs");
const path = require("path");
const extension = path.resolve(__dirname, "..");
const source = path.resolve(extension, "../../libraries");
const libraries = {
  hmi: "hmi", alarm: "alarm", control: "control", comm: "comm", oscat: "oscat", plant: "plant",
  plcopen_motion: "plcopen_motion/single_axis_core",
};
for (const [name, folder] of Object.entries(libraries)) {
  const from = path.join(source, folder);
  if (!fs.existsSync(path.join(from, "trust-lsp.toml"))) throw new Error(`Missing library package: ${name}`);
  const to = path.join(extension, "media/libraries", name);
  fs.rmSync(to, {recursive: true, force: true});
  fs.cpSync(from, to, {recursive: true, filter: file => !["target", "node_modules", ".git", "__pycache__", ".trust-lsp"].includes(path.basename(file))});
}

// The Equipment library was retired in favour of Plant; drop any stale packaged copy.
fs.rmSync(path.join(extension, "media/libraries/equipment"), {recursive:true,force:true});
