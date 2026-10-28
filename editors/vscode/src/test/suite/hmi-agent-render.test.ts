import * as vscode from "vscode";
import { languageServerReady } from "./hmiBuilderHost";
import { renderLibrarySheets } from "../../hmiBuilder/agent/library";
import { designValues } from "../../hmiBuilder/editor/designSamples";
import type { Catalogue } from "../../hmiBuilder/editor/catalogue";
import * as assert from "assert";
import * as fs from "fs/promises";
import * as path from "path";
import { renderAgentPage } from "../../hmiBuilder/agent/render";
import type { HmiPageModel } from "../../hmiBuilder/drawing/model";

suite("HMI agent actual rendered preview", function () {
  this.timeout(30000);
  const page: HmiPageModel = {
    title: "Tank process",
    pipe: [],
    object: [
      {
        id: "level",
        type: "value",
        at: [40, 40],
        size: [200, 64],
        rotation: 0,
        label: "Tank level",
        unit: "m",
        bind: { value: { path: "Main.Level", direction: "read" } },
      },
      {
        id: "tank",
        type: "tank",
        at: [300, 60],
        size: [180, 230],
        rotation: 0,
        bind: { level: { path: "Main.Level", direction: "read" } },
        min: 0,
        max: 3,
      },
    ],
  };
  const font = path.resolve(__dirname, "../../../media/fonts/InterVariable.woff2");
  test("viewport PNGs preserve native context and transform hit geometry at their actual scale", async () => {
    const rendered = await renderAgentPage(page, { size: [640, 360], values: {} }, "dark", font, {}, undefined, {
      viewport: { width: 320, height: 180 },
    });
    assert.strictEqual(rendered.width, 640);
    assert.strictEqual(rendered.viewport?.scale, 1);
    assert.strictEqual(rendered.viewport?.scroll, true);
    const png = Buffer.from(rendered.viewport!.png, "base64");
    assert.strictEqual(png.readUInt32BE(16), 320);
    assert.strictEqual(png.readUInt32BE(20), 180);
    const native = rendered.objects.find((object) => object.id === "level")!,
      target = rendered.viewport!.objects.find((object) => object.id === "level")!;
    assert.strictEqual(target.width, native.width);
    assert.strictEqual(target.height, native.height);
    assert.ok(rendered.viewport!.contentSize[1] > 180, "adapted content remains scrollable");
    if (process.env.HMI_AGENT_RENDER_OUT) {
      await fs.mkdir(process.env.HMI_AGENT_RENDER_OUT, { recursive: true });
      await fs.writeFile(path.join(process.env.HMI_AGENT_RENDER_OUT, "agent-viewport.png"), png);
    }
  });
  test("returns a real PNG with object bounds and visibly different themes", async () => {
    const frame = {
      size: [640, 360] as [number, number],
      values: { "Main.Level": { value: 1.8, state: "confirmed" as const } },
    };
    const dark = await renderAgentPage(page, frame, "dark", font);
    const light = await renderAgentPage(page, frame, "light", font);
    const bytes = Buffer.from(dark.png, "base64");
    assert.strictEqual(bytes.subarray(1, 4).toString(), "PNG");
    assert.strictEqual(bytes.readUInt32BE(16), 640);
    assert.strictEqual(bytes.readUInt32BE(20), 360);
    assert.ok(bytes.length > 2000);
    assert.notStrictEqual(dark.png, light.png);
    const object = dark.objects.find((item) => item.id === "level");
    assert.ok(object);
    assert.ok(object!.width > 0);
    assert.ok(Math.abs(object!.x - 40) <= 1, "rendered ink bounds include the component stroke inset");
    if (process.env.HMI_AGENT_RENDER_OUT) {
      await fs.mkdir(process.env.HMI_AGENT_RENDER_OUT, { recursive: true });
      await fs.writeFile(path.join(process.env.HMI_AGENT_RENDER_OUT, "agent-dark.png"), bytes);
      await fs.writeFile(
        path.join(process.env.HMI_AGENT_RENDER_OUT, "agent-light.png"),
        Buffer.from(light.png, "base64"),
      );
    }
  });
  test("loads the shipped multilingual fonts in the actual preview browser", async () => {
    const translated: HmiPageModel = {
      ...page,
      object: [{ ...page.object[0], label: "水温 / 온도", size: [440, 80] }],
    };
    const rendered = await renderAgentPage(translated, { size: [640, 360], values: {} }, "dark", font);
    const loaded = (rendered as typeof rendered & { fonts?: string[] }).fonts;
    assert.deepStrictEqual(
      loaded,
      ["Inter", "truST CJK SC", "truST CJK JP", "truST CJK KR"],
      "the PNG must use the bundled fonts, not machine-dependent fallback glyphs",
    );
    if (process.env.HMI_AGENT_RENDER_OUT)
      await fs.writeFile(
        path.join(process.env.HMI_AGENT_RENDER_OUT, "agent-multilingual.png"),
        Buffer.from(rendered.png, "base64"),
      );
  });
  test("custom symbol previews use the selected project definition without leaking between renders", async () => {
    const customPage: HmiPageModel = {
      title: "Custom vessel",
      pipe: [],
      object: [{ id: "custom", type: "my:vessel", at: [40, 40], size: [180, 180], rotation: 0, bind: {} }],
    };
    const definition = (fill: string) => ({
      name: "vessel",
      title: "Vessel",
      size: [180, 180],
      viewBox: [0, 0, 100, 100],
      properties: {},
      ports: [],
      parts: [],
      svg: {
        tag: "g",
        attrs: {},
        children: [{ tag: "rect", attrs: { x: "0", y: "0", width: "100", height: "100", fill }, children: [] }],
      },
    });
    const frame = { size: [320, 280], values: {} };
    const plain = await renderAgentPage(customPage, frame as Parameters<typeof renderAgentPage>[1], "dark", font);
    const first = await Reflect.apply(renderAgentPage, undefined, [
      customPage,
      frame,
      "dark",
      font,
      {},
      undefined,
      { symbols: [definition("#ff0000")] },
    ]);
    const second = await Reflect.apply(renderAgentPage, undefined, [
      customPage,
      frame,
      "dark",
      font,
      {},
      undefined,
      { symbols: [definition("#00ff00")] },
    ]);
    assert.notStrictEqual(first.png, second.png, "same symbol ID must use each project's own drawing");
    const after = await renderAgentPage(customPage, frame as Parameters<typeof renderAgentPage>[1], "dark", font);
    assert.strictEqual(after.png, plain.png, "a previous project's symbol must not leak into a later render");
  });
  test("renders an overview page and catalogue sheet at readable target size", async () => {
    await vscode.extensions.getExtension("trust-platform.trust-lsp")?.activate();
    await languageServerReady();
    const result = await vscode.commands.executeCommand<{ ok: boolean; catalogue: Catalogue }>(
      "trust-lsp.hmiCatalogue",
      { root_uri: vscode.workspace.workspaceFolders![0].uri.toString() },
    );
    assert.ok(result?.ok, JSON.stringify(result));
    const catalogue = result!.catalogue;
    const composed: HmiPageModel = {
      title: "TK-101 · Transfer system",
      pipe: [],
      object: [
        {
          id: "tank",
          type: "tank",
          at: [64, 136],
          size: [160, 240],
          rotation: 0,
          min: 0,
          max: 3,
          bind: { level: { path: "Main.Level", direction: "read" } },
        },
        {
          id: "pump",
          type: "pump",
          at: [320, 280],
          size: [128, 108],
          rotation: 0,
          bind: { running: { path: "Main.Running", direction: "read" } },
        },
        {
          id: "level",
          type: "value",
          at: [640, 136],
          size: [160, 48],
          rotation: 0,
          unit: "m",
          bind: { value: { path: "Main.Level", direction: "read" } },
        },
        {
          id: "temperature",
          type: "value",
          at: [640, 232],
          size: [160, 48],
          rotation: 0,
          unit: "°C",
          bind: { value: { path: "Main.Temperature", direction: "read" } },
        },
      ],
    };
    const values = {
      ...designValues(composed, catalogue),
      "Main.Level": { value: 1.8, state: "confirmed" as const },
      "Main.Running": { value: true, state: "confirmed" as const },
      "Main.Temperature": { value: 42.5, state: "confirmed" as const },
    };
    for (const theme of ["dark", "light"] as const) {
      const rendered = await renderAgentPage(composed, { size: [1280, 800], values }, theme, font);
      assert.strictEqual(rendered.objects.length, composed.object.length);
      if (process.env.HMI_AGENT_RENDER_OUT)
        await fs.writeFile(
          path.join(process.env.HMI_AGENT_RENDER_OUT, `agent-overview-${theme}.png`),
          Buffer.from(rendered.png, "base64"),
        );
    }
    const sheet = await renderLibrarySheets(
      catalogue.components.filter((component) => ["tank", "pump", "valve"].includes(component.type)),
      font,
      "dark",
      new AbortController().signal,
    );
    assert.strictEqual(sheet.images.length, 1);
    assert.strictEqual(sheet.sheets[0].types.length, 3);
    const bounds = (
      sheet.sheets[0] as (typeof sheet.sheets)[0] & { objects?: Array<{ id: string; width: number; height: number }> }
    ).objects;
    assert.ok(
      bounds
        ?.filter((object) => object.id.startsWith("sample-"))
        .every((object) => object.width > 20 && object.height > 20),
      "contact-sheet equipment must be visibly drawn, not hidden by synthetic visibility bindings",
    );
    if (process.env.HMI_AGENT_RENDER_OUT)
      await fs.writeFile(
        path.join(process.env.HMI_AGENT_RENDER_OUT, "agent-library.png"),
        Buffer.from(sheet.images[0], "base64"),
      );
  });
  test("returns full-page context and a native-scale crop with matching bounds", async () => {
    const frame: Parameters<typeof renderAgentPage>[1] = {
      size: [640, 360],
      values: { "Main.Level": { value: 1.8, state: "confirmed" } },
    };
    const rendered = await Reflect.apply(renderAgentPage, undefined, [
      page,
      frame,
      "dark",
      font,
      {},
      undefined,
      { crop: { ids: ["level"] } },
    ]);
    assert.ok(rendered.crop, "requested object crop is returned alongside the whole page");
    const crop = rendered.crop;
    assert.deepStrictEqual(crop.ids, ["level"]);
    const bytes = Buffer.from(crop.png, "base64");
    assert.strictEqual(bytes.readUInt32BE(16), crop.width);
    assert.strictEqual(bytes.readUInt32BE(20), crop.height);
    assert.ok(crop.width >= 199 && crop.width <= 202, "native pixel scale is preserved");
    const rectangle = await renderAgentPage(
      page,
      frame as Parameters<typeof renderAgentPage>[1],
      "dark",
      font,
      {},
      undefined,
      { crop: { x: crop.x, y: crop.y, width: crop.width, height: crop.height } },
    );
    assert.strictEqual(
      rectangle.crop?.png,
      crop.png,
      "object and rectangle selection must produce the same actual pixels",
    );
    const whole = await renderAgentPage(
      page,
      frame as Parameters<typeof renderAgentPage>[1],
      "dark",
      font,
      {},
      undefined,
      { crop: { x: 0, y: 0, width: 640, height: 360 } },
    );
    assert.strictEqual(whole.crop?.png, whole.png, "a full-page crop preserves the exact native-scale image");
    assert.strictEqual(Buffer.from(rendered.png, "base64").readUInt32BE(16), 640, "whole-page context remains present");
    await assert.rejects(
      Reflect.apply(renderAgentPage, undefined, [
        page,
        frame,
        "dark",
        font,
        {},
        undefined,
        { crop: { ids: ["missing"] } },
      ]),
      /INVALID_ARGUMENT/,
    );
    await assert.rejects(
      Reflect.apply(renderAgentPage, undefined, [
        page,
        frame,
        "dark",
        font,
        {},
        undefined,
        { crop: { x: -1, y: 0, width: 200, height: 100 } },
      ]),
      /INVALID_ARGUMENT/,
    );
    if (process.env.HMI_AGENT_RENDER_OUT)
      await fs.writeFile(path.join(process.env.HMI_AGENT_RENDER_OUT, "agent-detail.png"), bytes);
  });
  test("invalid sizes and cancelled requests do not create preview panels", async () => {
    await assert.rejects(renderAgentPage(page, { size: [5000, 100], values: {} }, "dark", font), /LIMIT_EXCEEDED/);
    const controller = new AbortController();
    controller.abort();
    await assert.rejects(
      renderAgentPage(page, { size: [640, 360], values: {} }, "dark", font, {}, controller.signal),
      /CANCELLED/,
    );
  });
  test("render artifacts identify the actual renderer, fonts and capture time", async () => {
    const rendered = await renderAgentPage(page, { size: [640, 360], values: {} }, "dark", font);
    const artifact = rendered as typeof rendered & {
      renderer?: { version: string; sha256: string };
      fontHashes?: Record<string, string>;
      capturedAt?: string;
    };
    assert.ok(artifact.renderer, "images need renderer identity");
    assert.match(artifact.renderer.sha256, /^[a-f0-9]{64}$/);
    assert.ok(artifact.renderer.version);
    assert.deepStrictEqual(Object.keys(artifact.fontHashes ?? {}), [
      "Inter",
      "truST CJK SC",
      "truST CJK JP",
      "truST CJK KR",
    ]);
    assert.ok(Object.values(artifact.fontHashes!).every((hash) => /^[a-f0-9]{64}$/.test(hash)));
    assert.ok(Number.isFinite(Date.parse(artifact.capturedAt!)));
  });
});
