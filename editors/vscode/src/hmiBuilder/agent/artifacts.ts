import { createHash } from "crypto";
import * as fs from "fs/promises";
import * as path from "path";

export interface RendererIdentity {
  name: string;
  version: string;
  sha256: string;
}
let identity: Promise<RendererIdentity> | undefined;

/** Identify the installed shared renderer, including unpublished development builds. The
 * code is immutable for this extension-host session; fonts are hashed from each actual read. */
export function rendererIdentity(): Promise<RendererIdentity> {
  identity ??= (async () => {
    const root = path.resolve(__dirname, "..");
    const files: string[] = [];
    const visit = async (relative: string): Promise<void> => {
      for (const entry of await fs.readdir(path.join(root, relative), { withFileTypes: true })) {
        const file = `${relative}/${entry.name}`;
        if (entry.isDirectory()) await visit(file);
        else if (entry.isFile() && entry.name.endsWith(".js")) files.push(file);
      }
    };
    try {
      await visit("drawing");
      files.push("agent/render.js", "agent/assetRasterizer.js", "editor/designSamples.js", "editor/designHistory.js");
      const hash = createHash("sha256");
      for (const file of files.sort())
        hash
          .update(file)
          .update("\0")
          .update(await fs.readFile(path.join(root, file)))
          .update("\0");
      const manifest = JSON.parse(await fs.readFile(path.resolve(__dirname, "../../../package.json"), "utf8")) as {
        version: string;
      };
      return { name: "truST shared HMI drawing", version: manifest.version, sha256: hash.digest("hex") };
    } catch (error) {
      throw new Error(
        `RENDER_FAILED: renderer identity is unavailable: ${error instanceof Error ? error.message : String(error)}`,
      );
    }
  })();
  return identity;
}

export const sha256 = (value: string | Buffer): string => createHash("sha256").update(value).digest("hex");
