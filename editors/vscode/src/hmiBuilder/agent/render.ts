import { rendererIdentity, RendererIdentity, sha256 } from "./artifacts";
import { rasterizeProjectImages } from "./assetRasterizer";
import * as fs from "fs/promises";
import * as path from "path";
import { randomBytes } from "crypto";
import * as vscode from "vscode";
import { withProjectSymbols } from "../drawing/symbols";
import type { CustomSymbolModel } from "../drawing/symbols/custom";
import { drawPage } from "../drawing";
import { adaptPage } from "../drawing/adaptive";
import { renderToString, el } from "../drawing/svg";
import type { HmiPageModel, DrawFrame } from "../drawing/model";
import { tokensCss, HmiColorsModel, CJK_FONTS } from "../drawing/tokens";

export type PreviewCrop = { ids: string[] } | { x: number; y: number; width: number; height: number };
export interface RenderedPage {
  viewport?: {
    png: string;
    width: number;
    height: number;
    scale: number;
    contentSize: [number, number];
    scroll: boolean;
    objects: RenderedPage["objects"];
  };
  renderer: RendererIdentity;
  fontHashes: Record<string, string>;
  capturedAt: string;
  png: string;
  objects: Array<{ id: string; x: number; y: number; width: number; height: number }>;
  width: number;
  height: number;
  fonts: string[];
  texts: string[];
  crop?: { png: string; x: number; y: number; width: number; height: number; ids: string[] };
}
/** Browser rasterization of the shared drawing, in an isolated, network-free webview.
 * This is a design-frame image, not proof of runtime commands or physical plant state. */
export async function renderAgentPage(
  page: HmiPageModel,
  frame: DrawFrame,
  theme: "dark" | "light",
  fontFile: string,
  colors: HmiColorsModel = {},
  signal?: AbortSignal,
  resources: {
    symbols?: CustomSymbolModel[];
    crop?: PreviewCrop;
    viewport?: { width: number; height: number };
    output?: { width: number; height: number; fit: boolean };
  } = {},
): Promise<RenderedPage> {
  if (signal?.aborted) throw new Error("CANCELLED: preview cancelled.");
  const [width, height] = frame.size;
  if (
    ![width, height].every((value) => Number.isInteger(value) && value > 0 && value <= 4096) ||
    width * height > 16000000
  )
    throw new Error("LIMIT_EXCEEDED: preview size exceeds the supported dimensions.");
  const viewport = resources.viewport;
  if (
    viewport &&
    (![viewport.width, viewport.height].every((value) => Number.isInteger(value) && value > 0 && value <= 4096) ||
      viewport.width * viewport.height > 16000000)
  )
    throw new Error("LIMIT_EXCEEDED: viewport exceeds supported dimensions.");
  const output = resources.output;
  const outputWidth = output?.width ?? width,
    outputHeight = output?.height ?? height;
  const crop = resources.crop;
  if (crop !== undefined) {
    const valid =
      crop &&
      typeof crop === "object" &&
      !Array.isArray(crop) &&
      ("ids" in crop
        ? Object.keys(crop).length === 1 &&
          Array.isArray(crop.ids) &&
          crop.ids.length > 0 &&
          crop.ids.length <= 500 &&
          crop.ids.every((id) => typeof id === "string" && page.object.some((object) => object.id === id))
        : Object.keys(crop).sort().join(",") === "height,width,x,y" &&
          [crop.x, crop.y, crop.width, crop.height].every(Number.isInteger) &&
          crop.x >= 0 &&
          crop.y >= 0 &&
          crop.width > 0 &&
          crop.height > 0 &&
          crop.x + crop.width <= width &&
          crop.y + crop.height <= height);
    if (!valid)
      throw new Error("INVALID_ARGUMENT: crop must select existing object IDs or a rectangle within the page.");
  }
  const fontDefinitions = [{ family: "Inter", file: path.basename(fontFile) }, ...CJK_FONTS];
  const renderer = await rendererIdentity();
  const fontHashes: Record<string, string> = {};
  let css = tokensCss(colors, `https://hmi-font.invalid/${path.basename(fontFile)}`);
  try {
    const fonts = await Promise.all(
      fontDefinitions.map(async (font) => ({
        ...font,
        bytes: await fs.readFile(path.join(path.dirname(fontFile), font.file)),
      })),
    );
    for (const font of fonts) {
      css = css.replace(
        `https://hmi-font.invalid/${font.file}`,
        `data:font/woff2;base64,${font.bytes.toString("base64")}`,
      );
      fontHashes[font.family] = sha256(font.bytes);
    }
  } catch {
    throw new Error("RENDER_FAILED: a required bundled HMI font could not be read.");
  }
  if (signal?.aborted) throw new Error("CANCELLED: preview cancelled.");
  const node = withProjectSymbols(resources.symbols ?? [], () => drawPage(page, frame));
  if (output?.fit) {
    node.attrs.width = String(output.width);
    node.attrs.height = String(output.height);
  }
  node.attrs.lang = frame.language ?? "en";
  node.attrs.class += ` hmi-theme-${theme}`;
  css += "\n*{animation:none!important;transition:none!important}";
  node.children.unshift({ ...el("style"), text: css });
  const svg = renderToString(node);
  const nonce = randomBytes(24).toString("hex");
  const panel = vscode.window.createWebviewPanel(
    "trust.hmi.agentPreview",
    "HMI design preview",
    { viewColumn: vscode.ViewColumn.Beside, preserveFocus: true },
    { enableScripts: true, localResourceRoots: [] },
  );
  const result = await new Promise<RenderedPage>((resolve, reject) => {
    let finished = false;
    const cleanup = (): void => {
      clearTimeout(timer);
      receive.dispose();
      closed.dispose();
      signal?.removeEventListener("abort", abort);
      panel.dispose();
    };
    const finish = (error?: Error, result?: RenderedPage): void => {
      if (finished) return;
      finished = true;
      cleanup();
      if (error) reject(error);
      else resolve(result!);
    };
    const abort = (): void => finish(new Error("CANCELLED: preview cancelled."));
    const timer = setTimeout(() => finish(new Error("RENDER_FAILED: preview browser timed out.")), 30000);
    const receive = panel.webview.onDidReceiveMessage(
      (message: {
        ok?: boolean;
        png?: string;
        objects?: RenderedPage["objects"];
        error?: string;
        fonts?: string[];
        texts?: string[];
        crop?: RenderedPage["crop"];
        viewport?: RenderedPage["viewport"];
      }) => {
        if (!message.ok || !message.png)
          finish(new Error(`RENDER_FAILED: ${message.error ?? "browser returned no image"}`));
        else
          finish(undefined, {
            renderer,
            fontHashes,
            capturedAt: new Date().toISOString(),
            png: message.png,
            objects: message.objects ?? [],
            width: outputWidth,
            height: outputHeight,
            fonts: message.fonts ?? [],
            texts: message.texts ?? [],
            ...(message.crop ? { crop: message.crop } : {}),
            ...(message.viewport ? { viewport: message.viewport } : {}),
          });
      },
    );
    const closed = panel.onDidDispose(() => finish(new Error("CANCELLED: preview closed.")));
    signal?.addEventListener("abort", abort, { once: true });
    panel.webview.html = `<!doctype html><html><head><meta charset="utf-8"><meta http-equiv="Content-Security-Policy" content="default-src 'none'; img-src data: blob:; font-src data:; style-src 'unsafe-inline'; script-src 'nonce-${nonce}'"><style>html,body{margin:0;padding:0}</style></head><body>${svg}<script nonce="${nonce}">
      const vscode=acquireVsCodeApi();
      (async()=>{
        try {
          const fonts = [];
          for (const family of ${JSON.stringify(fontDefinitions.map((font) => font.family))}) {
            const loaded = await document.fonts.load('16px "' + family + '"', family === 'Inter' ? 'Tank level' : '水温온도');
            if (!loaded.length || loaded.some(font => font.status !== 'loaded')) throw new Error('Required font unavailable: ' + family);
            fonts.push(family);
          }
          await document.fonts.ready;
          const svg=document.querySelector('svg');
          await (${rasterizeProjectImages.toString()})(svg);
          const box=svg.getBoundingClientRect();
          const objects=[...svg.querySelectorAll('[data-object]')].map(element=>{
            const r=element.getBoundingClientRect();return {id:element.getAttribute('data-object'),x:r.x-box.x,y:r.y-box.y,width:r.width,height:r.height};
          });
          const url=URL.createObjectURL(new Blob([new XMLSerializer().serializeToString(svg)],{type:'image/svg+xml;charset=utf-8'}));
          try {
            const image=new Image(); await new Promise((resolve,reject)=>{image.onload=resolve;image.onerror=()=>reject(new Error('SVG rasterization failed'));image.src=url;});
            const canvas=document.createElement('canvas'); canvas.width=${outputWidth};canvas.height=${outputHeight};
            const context=canvas.getContext('2d');if(!context)throw new Error('Canvas unavailable');context.fillStyle=getComputedStyle(svg).getPropertyValue('--hmi-page').trim()||'#000';context.fillRect(0,0,canvas.width,canvas.height);context.drawImage(image,0,0);
            const requested = ${JSON.stringify(crop ?? null).replace(/</g, "\\u003c")};
            let crop;
            if (requested) {
              let bounds = requested;
              if (requested.ids) {
                const selected = objects.filter(object => requested.ids.includes(object.id) && object.width > 0 && object.height > 0);
                if (selected.length !== new Set(requested.ids).size) throw new Error('Selected crop contains hidden or missing objects');
                const x = Math.max(0, Math.floor(Math.min(...selected.map(object => object.x))));
                const y = Math.max(0, Math.floor(Math.min(...selected.map(object => object.y))));
                const right = Math.min(canvas.width, Math.ceil(Math.max(...selected.map(object => object.x + object.width))));
                const bottom = Math.min(canvas.height, Math.ceil(Math.max(...selected.map(object => object.y + object.height))));
                bounds = {x, y, width: right - x, height: bottom - y};
              }
              if (bounds.width <= 0 || bounds.height <= 0) throw new Error('Selected crop is outside the page');
              const detail = document.createElement('canvas'); detail.width = bounds.width; detail.height = bounds.height;
              const pen = detail.getContext('2d'); if (!pen) throw new Error('Crop canvas unavailable');
              pen.drawImage(canvas, bounds.x, bounds.y, bounds.width, bounds.height, 0, 0, bounds.width, bounds.height);
              crop = {...bounds, ids: requested.ids || [], png: detail.toDataURL('image/png').split(',')[1]};
            }
            const texts=[...svg.querySelectorAll('text')].map(element=>element.textContent);
            vscode.postMessage({ok:true,png:canvas.toDataURL('image/png').split(',')[1],objects,fonts,texts,crop});
          } finally { URL.revokeObjectURL(url); }
        } catch(error){vscode.postMessage({ok:false,error:String(error.message||error)});}
      })();
    </script></body></html>`;
  });
  if (viewport) {
    const adapted = withProjectSymbols(resources.symbols ?? [], () =>
      adaptPage(page, frame.size, [viewport.width, viewport.height]),
    );
    const shown = await renderAgentPage(
      adapted.page,
      { ...frame, size: adapted.size },
      theme,
      fontFile,
      colors,
      signal,
      { symbols: resources.symbols, output: { ...viewport, fit: adapted.fit } },
    );
    result.viewport = {
      png: shown.png,
      width: viewport.width,
      height: viewport.height,
      scale: adapted.fit ? Math.min(viewport.width / adapted.size[0], viewport.height / adapted.size[1]) : 1,
      contentSize: adapted.size,
      scroll: !adapted.fit,
      objects: shown.objects,
    };
  }
  return result;
}
