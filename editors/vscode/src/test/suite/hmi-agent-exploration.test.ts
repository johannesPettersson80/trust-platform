// Specification 39, §8.7 item 3: the agent's exploration opens the panels the display opens, an
// equipment object's own panel as well as the one its click names.
import * as assert from "assert";
import * as vscode from "vscode";

import type { Draft } from "../../hmiBuilder/agent/drafts";
import { prepareExploration } from "../../hmiBuilder/agent/exploration";
import type { AuthoringQueries } from "../../hmiBuilder/agent/queries";

const PAGE = {
  title: "Overview",
  object: [
    {
      id: "p-101",
      type: "pump",
      at: [0, 0],
      size: [128, 108],
      rotation: 0,
      instance: "Main.P101",
      faceplate: "plant-run",
      bind: { running: { path: "Main.P101.Running", direction: "read" } },
    },
    {
      id: "lt-101",
      type: "value",
      at: [200, 0],
      size: [160, 48],
      rotation: 0,
      click: { faceplate: "plant-measurement" },
      bind: { value: { path: "Main.LT101.Value", direction: "read" } },
    },
    { id: "heading", type: "text", at: [0, 200], size: [160, 40], rotation: 0, text: "Pumps", bind: {} },
  ],
  pipe: [],
};

const CONTEXT = {
  screen: { size: [1280, 800], theme: "dark" },
  texts: { languages: [{ tag: "en", name: "English" }], builtin: { en: {} }, project: {} },
  catalogue: { format: 1, components: [] },
  alarms: [],
};

suite("HMI agent: exploration", () => {
  test("an equipment object's own panel is explored as a click's panel is", async () => {
    const formatted: Array<Record<string, unknown>> = [];
    const command = async (name: string, input: Record<string, unknown>): Promise<Record<string, unknown>> => {
      if (name === "trust-lsp.hmiFileParse") return { ok: true, model: PAGE };
      if (name === "trust-lsp.hmiFileFormat") {
        formatted.push(input);
        return { ok: true, text: "format = 1\n" };
      }
      if (name === "trust-lsp.hmiPlacedFaceplates") {
        const panel = { title: "P-101 — Pump", object: [], pipe: [], size: [608, 400] };
        return { ok: true, placed: { "p-101": { name: "plant-run", model: panel }, "lt-101": null } };
      }
      throw new Error(`unexpected command ${name}`);
    };
    const queries = {
      snapshots: async () => [{ file: "hmi/pages/overview.toml", text: "page", dirty: false }],
      command,
      documents: { root: vscode.Uri.file("/tmp/trust-hmi-exploration") },
    } as unknown as AuthoringQueries;
    const draft = { id: "draft", revision: "1", files: {} } as unknown as Draft;

    const model = await prepareExploration(draft, CONTEXT, queries, new AbortController().signal);

    const prepared = (formatted[0]?.model as { object: Array<{ id: string; faceplate?: string; instance?: string }> })
      .object;
    assert.deepStrictEqual(
      prepared.map(({ id, faceplate, instance }) => ({ id, faceplate, instance })),
      [
        { id: "p-101", faceplate: "plant-run", instance: "Main.P101" },
        { id: "lt-101", faceplate: "plant-measurement", instance: "Main.LT101" },
      ],
      "the pump's own panel and the value's click panel, each for its instance",
    );
    assert.ok(model.popups["faceplate:overview/p-101"], "the pump's panel opens in the exploration");
  });
});
