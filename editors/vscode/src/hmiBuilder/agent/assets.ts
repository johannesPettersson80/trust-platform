import { createHash } from "crypto";
import type { WorkspaceDocuments } from "./documents";
import type { NativeCommand } from "./queries";

export interface PreviewAsset {
  url: string;
  revision: string;
  bytes: number;
}
export type PreviewAssets = Record<string, PreviewAsset>;
const BINARY_LIMIT = 8 * 1024 * 1024;
const FRAME_LIMIT = 32 * 1024 * 1024;
const MIME: Record<string, string> = { png: "image/png", jpg: "image/jpeg", jpeg: "image/jpeg", svg: "image/svg+xml" };

/** Read bytes through the existing native bundle owner; SVG editor text takes precedence. */
export async function loadPreviewAssets(
  references: string[],
  documents: WorkspaceDocuments,
  native: NativeCommand,
  signal: AbortSignal,
): Promise<PreviewAssets> {
  const files = [...new Set(references)].sort();
  if (!files.length) return {};
  if (files.length > 32) throw new Error("LIMIT_EXCEEDED: preview supports at most 32 different project images.");
  for (const file of files) {
    if (!file.startsWith("hmi/assets/")) throw new Error("INVALID_ARGUMENT: images must be under hmi/assets/.");
    await documents.uri(file, signal);
  }
  const result = await native("trust-lsp.hmiBundle", { root_uri: documents.root.toString() });
  const bundle = result.bundle as { files?: Record<string, string | { base64: string }> } | undefined;
  if (result.ok !== true || !bundle?.files)
    throw new Error("RENDER_FAILED: the project's image bundle could not be read.");
  const assets: PreviewAssets = {};
  for (const file of files) {
    if (signal.aborted) throw new Error("CANCELLED: project image read cancelled.");
    const extension = file.split(".").pop()!.toLowerCase();
    const mime = Object.prototype.hasOwnProperty.call(MIME, extension) ? MIME[extension] : undefined;
    if (!mime) throw new Error(`UNSUPPORTED_CONTEXT: ${file} must be a static PNG, JPEG or SVG image.`);
    let bytes: Buffer;
    if (extension === "svg") {
      const snapshot = await documents.read(file, signal);
      if (!snapshot.exists) throw new Error(`RENDER_FAILED: project image is missing: ${file}.`);
      bytes = Buffer.from(snapshot.text, "utf8");
    } else {
      const content = bundle.files[file];
      if (!content) throw new Error(`RENDER_FAILED: project image is missing: ${file}.`);
      bytes = typeof content === "string" ? Buffer.from(content, "utf8") : Buffer.from(content.base64, "base64");
      if (bytes.length > BINARY_LIMIT) throw new Error(`LIMIT_EXCEEDED: ${file} exceeds the 8 MiB image limit.`);
      if (extension === "png" && bytes.length >= 24 && bytes.subarray(1, 4).toString() === "PNG") {
        const width = bytes.readUInt32BE(16),
          height = bytes.readUInt32BE(20);
        if (width > 4096 || height > 4096 || width * height > 16000000)
          throw new Error("LIMIT_EXCEEDED: project image dimensions exceed preview limits.");
        for (let at = 8; at + 12 <= bytes.length;) {
          const size = bytes.readUInt32BE(at);
          if (size > bytes.length - at - 12) break;
          if (bytes.subarray(at + 4, at + 8).toString() === "acTL")
            throw new Error("UNSUPPORTED_CONTEXT: animated PNG images need a static preview asset.");
          at += size + 12;
        }
      }
    }
    assets[file] = {
      url: `data:${mime};base64,${bytes.toString("base64")}`,
      revision: createHash("sha256").update(bytes).digest("hex"),
      bytes: bytes.length,
    };
  }
  if (references.reduce((total, file) => total + assets[file].bytes, 0) > FRAME_LIMIT)
    throw new Error("LIMIT_EXCEEDED: referenced image bytes exceed 32 MiB per preview frame.");
  return assets;
}
export const assetRevisions = (assets: PreviewAssets): Record<string, string> =>
  Object.fromEntries(Object.entries(assets).map(([file, asset]) => [file, asset.revision]));
