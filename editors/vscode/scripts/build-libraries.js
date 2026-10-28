// Package library source and visuals from their single repository owners.
const fs = require("fs");
const path = require("path");
const extension = path.resolve(__dirname, "..");
const source = path.resolve(extension, "../../libraries");
for (const name of ["hmi", "alarm", "control", "comm", "oscat", "plant"]) {
  const from = path.join(source, name);
  if (!fs.existsSync(path.join(from, "trust-lsp.toml"))) throw new Error(`Missing library package: ${name}`);
  const to = path.join(extension, "media/libraries", name);
  fs.rmSync(to, {recursive: true, force: true});
  fs.cpSync(from, to, {recursive: true, filter: file => !["target", "node_modules", ".git", "__pycache__", ".trust-lsp"].includes(path.basename(file))});
}

// The Equipment library was retired in favour of Plant; drop any stale packaged copy.
fs.rmSync(path.join(extension, "media/libraries/equipment"), {recursive:true,force:true});
