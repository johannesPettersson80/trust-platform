const fs = require("fs");
const path = require("path");
const cp = require("child_process");
const crypto = require("crypto");

const PNG_SIGNATURE = Buffer.from([0x89, 0x50, 0x4e, 0x47, 0x0d, 0x0a, 0x1a, 0x0a]);
const COLOR_MANAGEMENT_CHUNKS = new Set(["gAMA", "cHRM", "iCCP", "sRGB"]);
const DEFAULT_MIN_BYTES = 5 * 1024;
const DEFAULT_MAX_SINGLE_COLOR_RATIO = 0.99;

function isPng(buffer) {
  return buffer.length >= PNG_SIGNATURE.length && buffer.subarray(0, 8).equals(PNG_SIGNATURE);
}

function readPngDimensions(buffer) {
  if (!Buffer.isBuffer(buffer) || !isPng(buffer) || buffer.length < 24) {
    return null;
  }
  return {
    width: buffer.readUInt32BE(16),
    height: buffer.readUInt32BE(20),
  };
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

function dominantColorRatio(file, width, height) {
  const proc = cp.spawnSync(
    "convert",
    [file, "-alpha", "off", "-depth", "8", "-colors", "256", "-format", "%c", "histogram:info:-"],
    { encoding: "utf8", maxBuffer: 10 * 1024 * 1024 }
  );
  if (proc.status !== 0) {
    throw new Error(`ImageMagick histogram failed for ${file}: ${proc.stderr || proc.stdout}`);
  }
  let maxCount = 0;
  for (const line of proc.stdout.split(/\r?\n/)) {
    const match = line.match(/^\s*(\d+):/);
    if (match) {
      maxCount = Math.max(maxCount, Number(match[1]));
    }
  }
  const pixels = width * height;
  return pixels > 0 ? maxCount / pixels : 1;
}

function sha256File(file) {
  return crypto.createHash("sha256").update(fs.readFileSync(file)).digest("hex");
}

function assertValidCapture(file, options = {}) {
  const buffer = fs.readFileSync(file);
  const errors = [];
  if (!isPng(buffer)) {
    throw new Error(`${file}: not a PNG`);
  }
  const dimensions = readPngDimensions(buffer);
  if (!dimensions) {
    throw new Error(`${file}: missing PNG dimensions`);
  }

  const stat = fs.statSync(file);
  const minBytes = options.minBytes ?? DEFAULT_MIN_BYTES;
  if (stat.size < minBytes) {
    errors.push(`file too small: ${stat.size} bytes < ${minBytes}`);
  }

  const minWidth = options.minWidth ?? 640;
  const minHeight = options.minHeight ?? 480;
  if (dimensions.width < minWidth || dimensions.height < minHeight) {
    errors.push(`dimensions too small: ${dimensions.width}x${dimensions.height} < ${minWidth}x${minHeight}`);
  }
  if (options.expectedWidth && dimensions.width !== Number(options.expectedWidth)) {
    errors.push(`unexpected width: ${dimensions.width} != ${options.expectedWidth}`);
  }
  if (options.expectedHeight && dimensions.height !== Number(options.expectedHeight)) {
    errors.push(`unexpected height: ${dimensions.height} != ${options.expectedHeight}`);
  }

  const maxSingleColorRatio = options.maxSingleColorRatio ?? DEFAULT_MAX_SINGLE_COLOR_RATIO;
  const singleColorRatio = dominantColorRatio(file, dimensions.width, dimensions.height);
  if (singleColorRatio >= maxSingleColorRatio) {
    errors.push(`single-colour frame: ${(singleColorRatio * 100).toFixed(2)}% >= ${(maxSingleColorRatio * 100).toFixed(2)}%`);
  }

  if (errors.length) {
    throw new Error(`${file}: invalid capture (${errors.join("; ")})`);
  }
  return {
    file,
    size: stat.size,
    width: dimensions.width,
    height: dimensions.height,
    singleColorRatio,
  };
}

function collectPngFiles(root) {
  const files = [];
  if (!root || !fs.existsSync(root)) {
    return files;
  }
  const stack = [root];
  while (stack.length > 0) {
    const dir = stack.pop();
    for (const entry of fs.readdirSync(dir, { withFileTypes: true })) {
      const file = path.join(dir, entry.name);
      if (entry.isDirectory()) {
        stack.push(file);
      } else if (entry.isFile() && entry.name.toLowerCase().endsWith(".png")) {
        files.push(file);
      }
    }
  }
  return files.sort();
}

function validateCaptureTree(root, options = {}) {
  const roots = options.roots || ["screenshots-raw", "legacy-captures"];
  const duplicateRegistry = options.duplicateRegistry || new Map();
  const rejectDuplicates = options.rejectDuplicates !== false;
  const files = roots.flatMap((rel) => collectPngFiles(path.join(root, rel)));
  const valid = [];
  const errors = [];

  for (const file of files) {
    try {
      const info = assertValidCapture(file, options);
      if (rejectDuplicates) {
        const sha = sha256File(file);
        const rel = path.relative(root, file);
        const previous = duplicateRegistry.get(sha);
        if (previous && previous !== rel) {
          throw new Error(`${file}: duplicate frame matches ${previous}`);
        }
        duplicateRegistry.set(sha, rel);
        info.sha256 = sha;
      }
      valid.push(info);
    } catch (error) {
      errors.push(error.message || String(error));
    }
  }

  if (errors.length) {
    const error = new Error(`PNG capture validation failed:\n- ${errors.join("\n- ")}`);
    error.errors = errors;
    error.valid = valid;
    throw error;
  }
  return { valid, duplicateRegistry };
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
  const checkOnly = roots[0] === "--check";
  const targets = checkOnly ? roots.slice(1) : roots;
  let stripped = 0;
  for (const target of targets) {
    const resolved = path.resolve(target);
    const stat = fs.existsSync(resolved) ? fs.statSync(resolved) : null;
    if (!stat) {
      console.error(`missing: ${resolved}`);
      process.exitCode = 1;
    } else if (stat.isDirectory()) {
      if (checkOnly) {
        validateCaptureTree(resolved, { roots: ["."] });
      } else {
        stripped += stripTree(resolved);
      }
    } else if (checkOnly) {
      assertValidCapture(resolved);
    } else if (stripPngFile(resolved)) {
      stripped += 1;
    }
  }
  console.log(checkOnly ? `png-hygiene checked ${targets.length} target(s)` : `png-hygiene stripped ${stripped} file(s)`);
}

module.exports = {
  stripPngColorChunks,
  writePng,
  writePngBase64,
  stripPngFile,
  copyPngStripped,
  stripTree,
  listColorChunks,
  readPngDimensions,
  assertValidCapture,
  validateCaptureTree,
  sha256File,
};
