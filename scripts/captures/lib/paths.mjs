import fs from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";

const thisDir = path.dirname(fileURLToPath(import.meta.url));

export const repoRoot = path.resolve(thisDir, "../../..");
export const publicImageRoot = path.join(repoRoot, "docs/public/assets/images");

export function publicImagePath(relativePath) {
  return path.join(publicImageRoot, relativePath);
}

export function ensureParent(filePath) {
  fs.mkdirSync(path.dirname(filePath), { recursive: true });
}
