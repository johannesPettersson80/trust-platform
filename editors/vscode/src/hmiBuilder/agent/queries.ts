import { simulatorSessions } from "./simulator";
import { remoteVariableContext } from "../editor/remoteVariables";
import { sha256 } from "./artifacts";
import { previewStates } from "../drawing/previews";
import { withProjectSymbols } from "../drawing/symbols";
import type { CustomSymbolModel } from "../drawing/symbols/custom";
import { inspectMetadata, contextSelection } from "./queryMetadata";
import { patternReference } from "./patternReferences";
import { compositionPatterns } from "./patterns";
import { drawnBox } from "../editor/editing";
import * as fs from "fs/promises";
import * as path from "path";
import * as vscode from "vscode";
import type { Catalogue } from "../editor/catalogue";
import type { HmiPageModel } from "../drawing/model";
import { DEFAULT_SCREEN } from "../editor/messages";
import { WorkspaceDocuments, DocumentSnapshot } from "./documents";

export interface InspectionRegion {
  x: number;
  y: number;
  width: number;
  height: number;
}

export type NativeCommand = (name: string, input: Record<string, unknown>) => Promise<Record<string, unknown>>;
/** Read-only queries share the builder's native contracts; no independent TOML or binding parser. */
export class AuthoringQueries {
  constructor(
    readonly documents: WorkspaceDocuments,
    readonly command: NativeCommand,
  ) {}

  private async native(
    name: string,
    input: Record<string, unknown>,
    code = "VALIDATION_FAILED",
  ): Promise<Record<string, unknown>> {
    const result = await this.command(name, input);
    if (!result || result.ok !== true || result.variablesError) {
      throw new Error(
        `${code}: ${String(result?.variablesError ?? result?.error ?? JSON.stringify(result?.errors) ?? "Native HMI service unavailable")}`,
      );
    }
    return result;
  }
  async snapshots(): Promise<DocumentSnapshot[]> {
    const files = new Set<string>();
    const walk = async (relative: string): Promise<void> => {
      const uri = await this.documents.uri(relative);
      let entries;
      try {
        entries = await fs.readdir(uri.fsPath, { withFileTypes: true });
      } catch (error) {
        if ((error as NodeJS.ErrnoException).code === "ENOENT") return;
        throw error;
      }
      for (const entry of entries.sort((a, b) => a.name.localeCompare(b.name))) {
        const name = `${relative}/${entry.name}`;
        if (entry.isDirectory() && !["assets", "_evidence"].includes(entry.name)) await walk(name);
        else if (entry.name.endsWith(".toml")) files.add(name);
        if (files.size > 1000) throw new Error("LIMIT_EXCEEDED: project has more than 1000 HMI documents.");
      }
    };
    await walk("hmi");
    for (const doc of vscode.workspace.textDocuments) {
      if (doc.uri.scheme !== this.documents.root.scheme || doc.uri.authority !== this.documents.root.authority)
        continue;
      const relative = path.relative(this.documents.root.fsPath, doc.uri.fsPath).split(path.sep).join("/");
      if (relative.startsWith("hmi/") && relative.endsWith(".toml")) files.add(relative);
    }
    return (await Promise.all([...files].sort().map((file) => this.documents.read(file)))).filter(
      (file) => file.exists,
    );
  }
  async context(
    options: { partial?: boolean; sections?: string[]; file?: string; cursor?: number; limit?: number } = {},
  ): Promise<Record<string, unknown>> {
    const dirtySources = vscode.workspace.textDocuments.filter((doc) => {
      if (
        !doc.isDirty ||
        doc.uri.scheme !== this.documents.root.scheme ||
        doc.uri.authority !== this.documents.root.authority
      )
        return false;
      const relative = path.relative(this.documents.root.fsPath, doc.uri.fsPath);
      return (
        /\.st$/i.test(relative) &&
        relative !== ".." &&
        !relative.startsWith(".." + path.sep) &&
        !path.isAbsolute(relative)
      );
    });
    const snapshots = await this.snapshots();
    const open = Object.fromEntries(snapshots.filter((file) => file.dirty).map((file) => [file.file, file.text]));
    const root_uri = this.documents.root.toString();
    const [bindings, catalogue, faceplates, texts] = await Promise.all([
      this.native("trust-lsp.hmiBindings", { root_uri, authoring: true }, "COMPILATION_FAILED").catch(
        (error): Record<string, unknown> => {
          if (!options.partial) throw error;
          return { compilationError: String(error), variables: null, sourceRevision: null, sourceMode: "unavailable" };
        },
      ),
      this.native("trust-lsp.hmiCatalogue", { root_uri, open }),
      this.native("trust-lsp.hmiFaceplates", { root_uri, open }),
      this.native("trust-lsp.hmiTexts", { root_uri, open }),
    ]);
    if (dirtySources.length && !bindings.compilationError && bindings.sourceMode !== "editor-overlay") {
      throw new Error(
        "COMPILATION_FAILED: the running language server is too old to compile unsaved ST; restart the updated extension.",
      );
    }
    const screenFile = snapshots.find((file) => file.file === "hmi/screen.toml");
    const screen = screenFile
      ? (await this.native("trust-lsp.hmiFileParse", { kind: "screen", text: screenFile.text, file: screenFile.file }))
          .model
      : DEFAULT_SCREEN;
    const pages = await Promise.all(
      snapshots
        .filter((file) => file.file.startsWith("hmi/pages/"))
        .map(async (file) => {
          try {
            const parsed = await this.native("trust-lsp.hmiFileParse", {
              kind: "page",
              text: file.text,
              file: file.file,
            });
            const model = parsed.model as HmiPageModel;
            return {
              file: file.file,
              revision: file.revision,
              dirty: file.dirty,
              title: model.title,
              order: model.order,
              hidden: model.hidden ?? false,
              group: model.group,
              icon: model.icon,
            };
          } catch (error) {
            return { file: file.file, revision: file.revision, dirty: file.dirty, error: String(error) };
          }
        }),
    );
    const cursor = options.cursor ?? 0,
      limit = options.limit ?? 100;
    const remote = await remoteVariableContext(this.documents.root);
    const data: Record<string, unknown> = {
      remoteCatalogue: remote.catalogue,
      simulatorSessions: simulatorSessions(this.documents.root.toString()),
      compilation: {
        status: bindings.compilationError ? "failed" : "ready",
        errors: bindings.compilationError ? [bindings.compilationError] : [],
      },
      partial: Boolean(bindings.compilationError) || cursor + limit < pages.length,
      selection: contextSelection(this.documents.root, options.file),
      provenance: {
        variables: "compiled-program",
        formats: "project-catalogue",
        topology: "not-inferred",
        revisions: "editor-buffer-or-disk",
      },
      screen,
      texts,
      languages: texts.languages,
      variables: Array.isArray(bindings.variables) ? [...bindings.variables, ...remote.variables] : bindings.variables,
      alarms: bindings.alarms ?? [],
      catalogue: catalogue.catalogue,
      symbols: catalogue.symbols ?? [],
      formats: catalogue.formats ?? {},
      faceplates: faceplates.faceplates ?? [],
      pages: pages.slice(cursor, cursor + limit),
      pageCount: pages.length,
      nextCursor: cursor + limit < pages.length ? cursor + limit : null,
      revisions: Object.fromEntries(snapshots.map((file) => [file.file, file.revision])),
      programRevision: bindings.sourceRevision,
      sourceState: dirtySources.length ? "editor" : "saved",
      topology: { status: "unspecified", connections: [] },
    };
    if (!options.sections) return data;
    const mandatory = [
      "compilation",
      "partial",
      "selection",
      "provenance",
      "revisions",
      "programRevision",
      "sourceState",
      "nextCursor",
      "pageCount",
    ];
    return Object.fromEntries(
      Object.entries(data).filter(([key]) => mandatory.includes(key) || options.sections!.includes(key)),
    );
  }
  async inspect(
    file: string,
    ids?: string[],
    region?: InspectionRegion,
    override?: { text: string; revision: string; draftId: string },
  ): Promise<Record<string, unknown>> {
    if (!/^hmi\/(pages|popups|faceplates)\/[^/]+\.toml$/.test(file))
      throw new Error("INVALID_ARGUMENT: select an HMI page, popup or faceplate.");
    if (
      region !== undefined &&
      (ids !== undefined ||
        !region ||
        Object.keys(region).sort().join(",") !== "height,width,x,y" ||
        ![region.x, region.y, region.width, region.height].every(Number.isFinite) ||
        region.x < 0 ||
        region.y < 0 ||
        region.width <= 0 ||
        region.height <= 0)
    )
      throw new Error("INVALID_ARGUMENT: select object IDs or one finite positive region, not both.");
    const snapshot = override ? { ...override, exists: true, dirty: false } : await this.documents.read(file);
    if (!snapshot.exists) throw new Error("PROJECT_UNAVAILABLE: the selected page does not exist.");
    const kind = file.startsWith("hmi/popups/") ? "popup" : file.startsWith("hmi/panels/") ? "faceplate" : "page";
    const parsed = await this.native("trust-lsp.hmiFileParse", { kind, text: snapshot.text, file });
    const model = parsed.model as HmiPageModel;
    const selected = new Set(
      ids ??
        model.object
          .filter((object) => {
            if (!region) return true;
            const box = drawnBox(object);
            return (
              box.x < region.x + region.width &&
              box.x + box.width > region.x &&
              box.y < region.y + region.height &&
              box.y + box.height > region.y
            );
          })
          .map((object) => object.id),
    );
    for (const id of selected)
      if (!model.object.some((object) => object.id === id)) throw new Error(`INVALID_ARGUMENT: unknown object ${id}.`);
    const pipes = model.pipe.filter(
      (pipe) => selected.has(pipe.from.split(".")[0]) || selected.has(pipe.to.split(".")[0]),
    );
    const neighbours = [
      ...new Set(
        pipes.flatMap((pipe) => [pipe.from.split(".")[0], pipe.to.split(".")[0]]).filter((id) => !selected.has(id)),
      ),
    ];
    const metadata = await inspectMetadata(model, file, this.documents, this.command, override?.text);
    return {
      ...metadata,
      file,
      revision: snapshot.revision,
      dirty: snapshot.dirty,
      ...(override ? { draftId: override.draftId, origin: "draft" } : { origin: "source" }),
      title: model.title,
      objects: model.object.filter((object) => selected.has(object.id)),
      pipes,
      neighbours,
      zOrder: Object.fromEntries(
        model.object.flatMap((object, index) => (selected.has(object.id) ? [[object.id, index]] : [])),
      ),
    };
  }
  async library(query = "", cursor = 0, limit = 100, category?: string): Promise<Record<string, unknown>> {
    if (!Number.isInteger(cursor) || cursor < 0 || !Number.isInteger(limit) || limit < 1 || limit > 100)
      throw new Error("INVALID_ARGUMENT: invalid library cursor.");
    const snapshots = await this.snapshots();
    const open = Object.fromEntries(snapshots.filter((file) => file.dirty).map((file) => [file.file, file.text]));
    const result = await this.native("trust-lsp.hmiCatalogue", { root_uri: this.documents.root.toString(), open });
    const catalogue = result.catalogue as Catalogue;
    const matches = catalogue.components.filter(
      (component) =>
        (!category || component.category === category) &&
        `${component.type} ${component.category} ${component.group ?? ""}`.toLowerCase().includes(query.toLowerCase()),
    );
    const catalogueRevision = sha256(JSON.stringify({ catalogue, symbols: result.symbols, formats: result.formats }));
    const faceplates = await this.native("trust-lsp.hmiFaceplates", { root_uri: this.documents.root.toString(), open });
    const components = withProjectSymbols((result.symbols ?? []) as CustomSymbolModel[], () =>
      matches.slice(cursor, cursor + limit).map((component) => ({
        ...component,
        definitionRevision: sha256(JSON.stringify(component)),
        states: previewStates(component.type, Object.keys(component.properties ?? {})),
      })),
    );
    return {
      components,
      catalogueRevision,
      faceplates: faceplates.faceplates ?? [],
      formats: result.formats ?? {},
      total: matches.length,
      nextCursor: cursor + limit < matches.length ? cursor + limit : null,
      symbols: result.symbols ?? [],
      format: catalogue.format,
      compositionPatterns: compositionPatterns(query).map((pattern) => ({
        ...pattern,
        ...(query === pattern.id ? { reference: patternReference(pattern, catalogue) } : {}),
      })),
    };
  }
}
