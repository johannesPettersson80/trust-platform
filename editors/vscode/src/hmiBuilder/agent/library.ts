import { sha256 } from "./artifacts";
import { scenarioFrame } from "./scenarios";
import { withProjectSymbols } from "../drawing/symbols";
import type { PatternReference } from "./patternReferences";
import type { CustomSymbolModel } from "../drawing/symbols/custom";
import type { Catalogue, ComponentDefinition } from "../editor/catalogue";
import type { HmiObjectModel, HmiPageModel } from "../drawing/model";
import { designValues } from "../editor/designSamples";
import { designHistory, DESIGN_NOW_MS } from "../editor/designHistory";
import { renderAgentPage, RenderedPage } from "./render";

interface LibrarySheet {
  types: string[];
  width: number;
  height: number;
  objects: RenderedPage["objects"];
  manifest: Record<string, unknown>;
}

/** Contact sheets are drawn from actual catalogue components, with each cell keyed to a type.
 * A sheet is an illustration of component design states, never runtime data. */
export async function renderLibrarySheets(
  components: ComponentDefinition[],
  fontFile: string,
  theme: "dark" | "light",
  signal: AbortSignal,
  symbols: CustomSymbolModel[] = [],
  state = "sample",
): Promise<{ images: string[]; sheets: LibrarySheet[] }> {
  const images: string[] = [],
    sheets: LibrarySheet[] = [];
  const supported = components.filter(
    (component) =>
      (!component.type.startsWith("my:") || symbols.some((symbol) => `my:${symbol.name}` === component.type)) &&
      component.type !== "faceplate" &&
      component.type !== "image",
  );
  for (let offset = 0; offset < supported.length; offset += 12) {
    const entries = supported.slice(offset, offset + 12);
    const columns = Math.min(3, entries.length);
    const cellWidth = Math.max(240, ...entries.map((entry) => entry.size[0] + 48));
    const cellHeight = Math.max(240, ...entries.map((entry) => entry.size[1] + 88));
    const size: [number, number] = [columns * cellWidth, Math.ceil(entries.length / columns) * cellHeight];
    const objects: HmiObjectModel[] = [];
    entries.forEach((component, index) => {
      const x = (index % columns) * cellWidth,
        y = Math.floor(index / columns) * cellHeight;
      objects.push({
        id: `label-${index}`,
        type: "text",
        at: [x + 24, y + 16],
        size: [cellWidth - 48, 32],
        rotation: 0,
        text: component.type,
        bind: {},
      });
      const properties = {
        ...(component.properties ?? {}),
        ...(component.additional ? { pen: component.additional } : {}),
      };
      const bind = Object.fromEntries(
        Object.entries(properties)
          .filter(([name]) => name !== "visible")
          .map(([name, definition]) => [
            name,
            {
              path: `Sample.${index}.${name}`,
              direction: (definition.directions.includes("read") ? "read" : definition.directions[0]) as
                "read" | "write" | "read-write",
            },
          ]),
      );
      objects.push({
        id: `sample-${index}`,
        type: component.type,
        at: [x + (cellWidth - component.size[0]) / 2, y + 64],
        size: component.size,
        rotation: 0,
        bind,
      });
    });
    const page: HmiPageModel = { title: "Component library", object: objects, pipe: [] };
    const catalogue: Catalogue = { format: 1, components: entries };
    const scenario = withProjectSymbols(symbols, () =>
      scenarioFrame([page], designValues(page, catalogue), state, catalogue),
    );
    const values = scenario.values;
    const rendered = await renderAgentPage(
      page,
      { ...scenario.frame, size, values, history: designHistory([page], values), nowMs: DESIGN_NOW_MS, readOnly: true },
      theme,
      fontFile,
      {},
      signal,
      { symbols },
    );
    images.push(rendered.png);
    sheets.push({
      types: entries.map((entry) => entry.type),
      width: size[0],
      height: size[1],
      objects: rendered.objects,
      manifest: {
        contractVersion: 1,
        imageIndex: images.length - 1,
        theme,
        scenario: state,
        origin: "synthetic",
        scale: 1,
        renderer: rendered.renderer,
        fontHashes: rendered.fontHashes,
        capturedAt: rendered.capturedAt,
        sha256: sha256(Buffer.from(rendered.png, "base64")),
        unsupportedStateObjects: scenario.unsupported,
      },
    });
  }
  return { images, sheets };
}

/** A versioned composition reference uses the same drawing path as component sheets. */
export async function renderPatternReference(
  reference: PatternReference,
  fontFile: string,
  theme: "dark" | "light",
  signal: AbortSignal,
): Promise<RenderedPage> {
  if (reference.missingComponents.length)
    throw new Error(
      `UNSUPPORTED_CONTEXT: pattern requires unavailable components: ${reference.missingComponents.join(", ")}.`,
    );
  return renderAgentPage(
    reference.model,
    {
      size: reference.size,
      values: reference.values,
      alarms: reference.alarms,
      history: designHistory([reference.model], reference.values),
      nowMs: DESIGN_NOW_MS,
      readOnly: true,
    },
    theme,
    fontFile,
    {},
    signal,
  );
}
