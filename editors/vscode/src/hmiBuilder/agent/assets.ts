import { createHash } from "crypto";
import * as vscode from "vscode";
import { EngineProject, nativeRead } from "./engineCommands";

export interface PreviewAsset {
  url: string;
  revision: string;
  bytes: number;
}
export type PreviewAssets = Record<string, PreviewAsset>;
const BINARY_LIMIT = 8 * 1024 * 1024;
const SVG_LIMIT = 1024 * 1024;
const FRAME_LIMIT = 32 * 1024 * 1024;
const MIME: Record<string, string> = { png: "image/png", jpg: "image/jpeg", jpeg: "image/jpeg", svg: "image/svg+xml" };

/** The text of an open SVG document under the project, which the editor holds; `undefined` when none is open. */
function openSvgText(root: vscode.Uri, file: string): string | undefined {
  const uri = vscode.Uri.joinPath(root, ...file.split("/"));
  if (!uri.path.startsWith(`${root.path.replace(/\/$/, "")}/`)) return undefined;
  return vscode.workspace.textDocuments.find((document) => document.uri.toString() === uri.toString())?.getText();
}

/** Reads the project's static images for a preview through the native bundle owner; an open SVG document is the
 * editor's text and takes precedence. */
export async function loadPreviewAssets(
  project: EngineProject,
  references: string[],
  signal: AbortSignal,
): Promise<PreviewAssets> {
  const files = [...new Set(references)].sort();
  if (!files.length) return {};
  if (files.length > 32) throw new Error("LIMIT_EXCEEDED: preview supports at most 32 different project images.");
  for (const file of files) {
    if (!file.startsWith("hmi/assets/")) throw new Error("INVALID_ARGUMENT: images must be under hmi/assets/.");
  }
  const result = await nativeRead(project.token, "trust-lsp.hmiBundle", { root_uri: project.root.toString() });
  const bundle = result.bundle as { files?: Record<string, string | { base64: string }> } | undefined;
  if (result.ok !== true || !bundle?.files)
    throw new Error("RENDER_FAILED: the project's image bundle could not be read.");
  const assets: PreviewAssets = {};
  for (const file of files) {
    if (signal.aborted) throw new Error("CANCELLED: project image read cancelled.");
    const extension = file.split(".").pop()!.toLowerCase();
    const mime = Object.prototype.hasOwnProperty.call(MIME, extension) ? MIME[extension] : undefined;
    if (!mime) throw new Error(`UNSUPPORTED_CONTEXT: ${file} must be a static PNG, JPEG or SVG image.`);
    const content = (extension === "svg" ? openSvgText(project.root, file) : undefined) ?? bundle.files[file];
    if (!content) throw new Error(`RENDER_FAILED: project image is missing: ${file}.`);
    const bytes = typeof content === "string" ? Buffer.from(content, "utf8") : Buffer.from(content.base64, "base64");
    if (extension === "svg" && bytes.length > SVG_LIMIT)
      throw new Error(`LIMIT_EXCEEDED: ${file} exceeds the 1 MiB SVG limit.`);
    if (bytes.length > BINARY_LIMIT) throw new Error(`LIMIT_EXCEEDED: ${file} exceeds the 8 MiB image limit.`);
    if (extension === "png" && bytes.length >= 24 && bytes.subarray(1, 4).toString() === "PNG") {
      const width = bytes.readUInt32BE(16),
        height = bytes.readUInt32BE(20);
      if (width > 4096 || height > 4096 || width * height > 16000000)
        throw new Error("LIMIT_EXCEEDED: project image dimensions exceed preview limits.");
      for (let at = 8; at + 12 <= bytes.length; ) {
        const size = bytes.readUInt32BE(at);
        if (size > bytes.length - at - 12) break;
        if (bytes.subarray(at + 4, at + 8).toString() === "acTL")
          throw new Error("UNSUPPORTED_CONTEXT: animated PNG images need a static preview asset.");
        at += size + 12;
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
