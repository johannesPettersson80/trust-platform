const fs = require("fs");
const path = require("path");

const PNG_SIGNATURE = Buffer.from([0x89, 0x50, 0x4e, 0x47, 0x0d, 0x0a, 0x1a, 0x0a]);
const COLOR_MANAGEMENT_CHUNKS = new Set(["gAMA", "cHRM", "iCCP", "sRGB"]);

function isPng(buffer) {
  return buffer.length >= PNG_SIGNATURE.length && buffer.subarray(0, 8).equals(PNG_SIGNATURE);
}

function stripPngColorChunks(buffer) {
  if (!Buffer.isBuffer(buffer) || !isPng(buffer)) {
    return buffer;
  }

  const chunks = [buffer.subarray(0, 8)];
  let offset = 8;
  let changed = false;
  while (offset + 12 <= buffer.length) {
    const length = buffer.readUInt32BE(offset);
    const typeStart = offset + 4;
    const dataStart = offset + 8;
    const next = dataStart + length + 4;
    if (next > buffer.length) {
      return buffer;
    }
    const type = buffer.toString("ascii", typeStart, typeStart + 4);
    if (COLOR_MANAGEMENT_CHUNKS.has(type)) {
      changed = true;
    } else {
      chunks.push(buffer.subarray(offset, next));
    }
    offset = next;
    if (type === "IEND") {
      break;
    }
  }
  if (!changed) {
    return buffer;
  }
  return Buffer.concat(chunks);
}

function writePng(dest, data) {
  const buffer = Buffer.isBuffer(data) ? data : Buffer.from(data);
  fs.writeFileSync(dest, stripPngColorChunks(buffer));
}

function writePngBase64(dest, base64Png) {
  writePng(dest, Buffer.from(base64Png, "base64"));
}

function stripPngFile(file) {
  if (!file || path.extname(file).toLowerCase() !== ".png" || !fs.existsSync(file)) {
    return false;
  }
  const before = fs.readFileSync(file);
  const after = stripPngColorChunks(before);
  if (after !== before) {
    fs.writeFileSync(file, after);
    return true;
  }
  return false;
}

function copyPngStripped(src, dest) {
  writePng(dest, fs.readFileSync(src));
}

function stripTree(root) {
  let stripped = 0;
  if (!root || !fs.existsSync(root)) {
    return stripped;
  }
  const stack = [root];
  while (stack.length > 0) {
    const dir = stack.pop();
    for (const entry of fs.readdirSync(dir, { withFileTypes: true })) {
      const file = path.join(dir, entry.name);
      if (entry.isDirectory()) {
        stack.push(file);
      } else if (entry.isFile() && entry.name.toLowerCase().endsWith(".png")) {
        if (stripPngFile(file)) {
          stripped += 1;
        }
      }
    }
  }
  return stripped;
}

function listColorChunks(file) {
  const buffer = fs.readFileSync(file);
  if (!isPng(buffer)) {
    return [];
  }
  const chunks = [];
  let offset = 8;
  while (offset + 12 <= buffer.length) {
    const length = buffer.readUInt32BE(offset);
    const typeStart = offset + 4;
    const dataStart = offset + 8;
    const next = dataStart + length + 4;
    if (next > buffer.length) {
      break;
    }
    const type = buffer.toString("ascii", typeStart, typeStart + 4);
    if (COLOR_MANAGEMENT_CHUNKS.has(type)) {
      chunks.push(type);
    }
    offset = next;
    if (type === "IEND") {
      break;
    }
  }
  return chunks;
}

if (require.main === module) {
  const roots = process.argv.slice(2);
  if (roots.length === 0) {
    console.error("Usage: node png-hygiene.js <png-or-directory> [...]");
    process.exit(2);
  }
  let stripped = 0;
  for (const target of roots) {
    const resolved = path.resolve(target);
    const stat = fs.existsSync(resolved) ? fs.statSync(resolved) : null;
    if (!stat) {
      console.error(`missing: ${resolved}`);
      process.exitCode = 1;
    } else if (stat.isDirectory()) {
      stripped += stripTree(resolved);
    } else if (stripPngFile(resolved)) {
      stripped += 1;
    }
  }
  console.log(`png-hygiene stripped ${stripped} file(s)`);
}

module.exports = {
  stripPngColorChunks,
  writePng,
  writePngBase64,
  stripPngFile,
  copyPngStripped,
  stripTree,
  listColorChunks,
};
