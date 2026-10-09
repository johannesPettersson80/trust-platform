import { alarmScenario } from "./alarmScenario";
import { runSimulator, SimulatorStep } from "./simulator";
import { prepareExploration } from "./exploration";
import { routingQuality } from "./routingQuality";
import { withValueFormat } from "../drawing/valueFormats";
import { scenarioFrame } from "./scenarios";
import { compositionIntent } from "./intent";
import { sha256 } from "./artifacts";
import { geometryQuality, viewportProfiles } from "./geometryQuality";
import { assetRevisions, loadPreviewAssets } from "./assets";
import type { ReviewPresenter } from "../editor/proposals";
import type { ProposalFile } from "../editor/proposalModel";
import { describePageChanges } from "./review";
import { withProjectSymbols } from "../drawing/symbols";
import type { CustomSymbolModel } from "../drawing/symbols/custom";
import { applyDraftFiles } from "./apply";
import * as path from "path";
import { createHash } from "crypto";
import type { Catalogue } from "../editor/catalogue";
import type { HmiPageModel, DrawFrame, PlacedFaceplate } from "../drawing/model";
import type { ScreenModel, VariableNode, ProjectTexts } from "../editor/messages";
import { objectWarnings, colourWarnings } from "../editor/quality";
import { designValues } from "../editor/designSamples";
import { designHistory, DESIGN_NOW_MS } from "../editor/designHistory";
import { AuthoringQueries, NativeCommand } from "./queries";
import { WorkspaceDocuments } from "./documents";
import { DraftStore, Draft, requireRetainedPages } from "./drafts";
import { composeAgentPage, CompositionInput } from "./compose";
import { editAgentPage } from "./edits";
import { renderAgentPage, PreviewCrop } from "./render";

export interface AuthoringOutput {
  data: Record<string, unknown>;
  images?: string[];
  committed?: boolean;
}
const stores = new Map<string, DraftStore>();
const PAGE = /^hmi\/pages\/[A-Za-z0-9_-]+\.toml$/;
function string(input: Record<string, unknown>, key: string): string {
  if (typeof input[key] !== "string" || !input[key]) throw new Error(`INVALID_ARGUMENT: ${key} is required.`);
  return input[key] as string;
}
function requestFingerprint(value: unknown): string {
  const encoded = JSON.stringify(value, (_key, item: unknown) =>
    item && typeof item === "object" && !Array.isArray(item)
      ? Object.fromEntries(Object.entries(item).sort(([a], [b]) => (a < b ? -1 : a > b ? 1 : 0)))
      : item,
  );
  return createHash("sha256").update(encoded).digest("hex");
}
/** Shared authoring operations. Adapters own transport; native services still own file syntax. */
export class AuthoringService {
  readonly queries: AuthoringQueries;
  readonly store: DraftStore;
  constructor(
    readonly workspaceId: string,
    readonly documents: WorkspaceDocuments,
    readonly native: NativeCommand,
    storageRoot: string,
    readonly fontFile: string,
    readonly present?: ReviewPresenter,
  ) {
    this.queries = new AuthoringQueries(documents, native);
    const directory = path.join(storageRoot, workspaceId);
    if (!stores.has(directory)) stores.set(directory, new DraftStore(directory));
    this.store = stores.get(directory)!;
  }
  private async command(name: string, input: Record<string, unknown>): Promise<Record<string, unknown>> {
    const result = await this.native(name, input);
    if (result.ok !== true)
      throw new Error(
        `VALIDATION_FAILED: ${String(result.error ?? JSON.stringify(result.errors) ?? "Native HMI service refused the document")}`,
      );
    return result;
  }
  private async draft(input: Record<string, unknown>, allowReplay = false): Promise<Draft> {
    const draft = await this.store.get(string(input, "draftId"));
    if (draft.workspaceId !== this.workspaceId)
      throw new Error("PROJECT_UNAVAILABLE: draft belongs to another workspace.");
    if (!allowReplay && draft.revision !== string(input, "revision"))
      throw new Error("CONFLICT: draft revision changed.");
    return draft;
  }
  private selectedFile(draft: Draft, input: Record<string, unknown>): string {
    const file =
      typeof input.file === "string"
        ? input.file
        : Object.keys(draft.files).length === 1
          ? Object.keys(draft.files)[0]
          : undefined;
    if (!file || !Object.prototype.hasOwnProperty.call(draft.files, file))
      throw new Error("INVALID_ARGUMENT: select a file in the draft.");
    return file;
  }
  private async model(text: string, file: string): Promise<HmiPageModel> {
    return (await this.command("trust-lsp.hmiFileParse", { kind: "page", text, file })).model as HmiPageModel;
  }
  private async text(model: HmiPageModel): Promise<string> {
    const result = await this.command("trust-lsp.hmiFileFormat", { kind: "page", model });
    if (typeof result.text !== "string") throw new Error("VALIDATION_FAILED: writer returned no document.");
    return result.text;
  }
  private async validate(
    draft: Draft,
    context: Record<string, unknown>,
    viewportInput?: unknown,
  ): Promise<Record<string, unknown>> {
    const findings: Array<Record<string, unknown>> = [];
    const profiles = viewportProfiles(viewportInput);
    const viewports: Array<Record<string, unknown>> = [];
    let omittedGeometryFindings = 0;
    for (const warning of withProjectSymbols(context.symbols as CustomSymbolModel[], () =>
      colourWarnings((context.screen as ScreenModel).colors ?? {}, context.catalogue as Catalogue),
    ))
      findings.push({ ...warning, severity: "warning", rule: warning.kind });
    const availableFiles = new Set([
      ...Object.keys((context.revisions ?? {}) as Record<string, string>),
      ...Object.keys(draft.files),
    ]);
    const texts = context.texts as ProjectTexts;
    const languages = texts.languages.map((entry) => ({
      ...entry,
      texts: { ...texts.builtin[entry.tag], ...texts.project[entry.tag] },
    }));
    for (const [file, text] of Object.entries(draft.files)) {
      const model = await this.model(text, file);
      for (const object of model.object)
        for (const [kind, target] of Object.entries(object.click ?? {})) {
          if (!["page", "popup"].includes(kind)) continue;
          const destination = `hmi/${kind === "page" ? "pages" : "popups"}/${target}.toml`;
          if (!availableFiles.has(destination))
            findings.push({
              file,
              object: object.id,
              severity: "error",
              rule: "missing-navigation-target",
              message: `${object.id} links to missing ${destination}; choose an existing target or include it in this proposal.`,
            });
        }
      if (model.incomplete)
        findings.push({
          file,
          severity: "error",
          rule: "incomplete-design",
          message:
            "This page is marked incomplete; finish its design and explicitly clear the marker before deployment.",
        });
      const result = await this.command("trust-lsp.hmiFileCheck", {
        authoring: true,
        root_uri: this.documents.root.toString(),
        file,
        text,
        remote: context.remoteCatalogue ?? {},
      });
      for (const problem of (result.problems ?? []) as Array<Record<string, unknown>>)
        findings.push({ ...problem, file, severity: "error", rule: "binding" });
      for (const warning of withProjectSymbols(context.symbols as CustomSymbolModel[], () =>
        objectWarnings(
          {
            ...model,
            object: model.object.map((object) =>
              withValueFormat(object, { formats: context.formats as DrawFrame["formats"] }),
            ),
          },
          context.catalogue as Catalogue,
          context.variables as VariableNode[],
          languages,
        ),
      ))
        findings.push({ ...warning, file, severity: "warning", rule: warning.kind });
      const size = (context.screen as ScreenModel).size;
      const geometry = withProjectSymbols(context.symbols as CustomSymbolModel[], () =>
        geometryQuality(model, size, context.catalogue as Catalogue, profiles),
      );
      findings.push(
        ...withProjectSymbols(context.symbols as CustomSymbolModel[], () => routingQuality(model, size)).map(
          (finding) => ({ ...finding, file }),
        ),
      );
      findings.push(...geometry.findings.map((finding) => ({ ...finding, file })));
      viewports.push(...geometry.viewports.map((viewport) => ({ ...viewport, file })));
      omittedGeometryFindings += geometry.omitted;
    }
    return {
      draftId: draft.id,
      revision: draft.revision,
      engineeringValid: !findings.some((finding) => finding.severity === "error"),
      findings,
      viewports,
      geometryFindingsTruncated: omittedGeometryFindings > 0,
      omittedGeometryFindings,
      visualReview: "required",
    };
  }
  private async render(
    draft: Draft,
    input: Record<string, unknown>,
    context: Record<string, unknown>,
    signal: AbortSignal,
    state = "sample",
  ): Promise<AuthoringOutput> {
    const file = this.selectedFile(draft, input),
      model = await this.model(draft.files[file], file);
    const symbols = context.symbols as CustomSymbolModel[];
    for (const object of model.object)
      if (object.type.startsWith("my:") && !symbols.some((symbol) => `my:${symbol.name}` === object.type))
        throw new Error(`VALIDATION_FAILED: missing project symbol ${object.type}.`);
    const screen = context.screen as ScreenModel,
      catalogue = context.catalogue as Catalogue;
    let placed: Record<string, PlacedFaceplate | null> = {};
    if (model.object.some((object) => object.type === "faceplate")) {
      const snapshots = await this.queries.snapshots();
      const open = Object.fromEntries(
        snapshots.filter((snapshot) => snapshot.dirty).map((snapshot) => [snapshot.file, snapshot.text]),
      );
      placed = (
        await this.command("trust-lsp.hmiPlacedFaceplates", {
          root_uri: this.documents.root.toString(),
          file,
          text: draft.files[file],
          open,
        })
      ).placed as typeof placed;
      for (const object of model.object)
        if (object.type === "faceplate" && !placed[object.id])
          throw new Error(`VALIDATION_FAILED: faceplate ${object.id} is missing or does not fit its instance.`);
    }
    const models = [model, ...Object.values(placed).flatMap((placement) => (placement ? [placement.model] : []))];
    const assets = await loadPreviewAssets(
      models.flatMap((page) => page.object.flatMap((object) => (object.image ? [object.image] : []))),
      this.documents,
      this.native,
      signal,
    );
    let values = Object.assign({}, ...models.map((page) => designValues(page, catalogue))) as DrawFrame["values"];
    const scenario = withProjectSymbols(symbols, () => scenarioFrame(models, values, state, catalogue));
    values = scenario.values;
    const unsupported = scenario.unsupported;
    const theme = input.theme ?? screen.theme;
    if (theme !== "dark" && theme !== "light") throw new Error("INVALID_ARGUMENT: theme must be dark or light.");
    const projectTexts = context.texts as ProjectTexts;
    const language = input.language ?? projectTexts.languages[0]?.tag ?? "en";
    if (typeof language !== "string" || !projectTexts.languages.some((entry) => entry.tag === language))
      throw new Error("INVALID_ARGUMENT: select a language declared by this project.");
    const texts = { ...projectTexts.builtin[language], ...projectTexts.project[language] };
    const frame: DrawFrame = {
      ...scenario.frame,
      alarms: alarmScenario(
        context.alarms as import("../editor/messages").AlarmMessage[],
        models,
        state === "alarm" || state === "fault",
      ),
      asset: (file) => {
        if (!assets[file]) throw new Error(`RENDER_FAILED: image was not loaded: ${file}.`);
        return assets[file].url;
      },
      language,
      texts,
      size: screen.size,
      values,
      placed,
      history: designHistory(models, values),
      nowMs: DESIGN_NOW_MS,
      readOnly: true,
      formats: context.formats as DrawFrame["formats"],
    };
    const rendered = await renderAgentPage(model, frame, theme, this.fontFile, screen.colors, signal, {
      symbols,
      crop: input.crop as PreviewCrop | undefined,
      viewport: input.viewport as { width: number; height: number } | undefined,
    });
    const images = [rendered.png];
    const artifacts: Array<Record<string, unknown>> = [
      { kind: "page", imageIndex: 0, width: rendered.width, height: rendered.height, scale: 1 },
    ];
    if (rendered.crop) {
      const { png, ...bounds } = rendered.crop;
      images.push(png);
      artifacts.push({ kind: "crop", imageIndex: 1, ...bounds, scale: 1 });
    }
    if (rendered.viewport) {
      const { png, objects, ...size } = rendered.viewport;
      artifacts.push({ kind: "viewport", imageIndex: images.length, ...size, objects });
      images.push(png);
    }
    artifacts.forEach((artifact, index) => {
      artifact.sha256 = sha256(Buffer.from(images[index], "base64"));
    });
    const manifest = {
      contractVersion: 1,
      draftId: draft.id,
      draftRevision: draft.revision,
      file,
      fileSha256: sha256(draft.files[file]),
      programRevision: draft.programRevision,
      theme,
      language,
      scenario: state,
      origin: "synthetic",
      designSize: screen.size,
      viewport: input.viewport ?? { width: rendered.width, height: rendered.height },
      scale: rendered.viewport?.scale ?? 1,
      renderer: rendered.renderer,
      fontHashes: rendered.fontHashes,
      assetRevisions: assetRevisions(assets),
      frameSha256: sha256(JSON.stringify({ model, placed, frame, symbols, colors: screen.colors })),
      capturedAt: rendered.capturedAt,
      images: artifacts,
    };
    return {
      data: {
        draftId: draft.id,
        revision: draft.revision,
        manifest,
        file,
        theme,
        language,
        scenario: state,
        origin: "synthetic",
        assetRevisions: assetRevisions(assets),
        width: rendered.width,
        height: rendered.height,
        objects: rendered.objects,
        placed,
        fonts: rendered.fonts,
        texts: rendered.texts,
        artifacts,
        unsupportedStateObjects: unsupported,
        visualReview: "required",
      },
      images,
    };
  }
  async execute(name: string, input: Record<string, unknown>, signal: AbortSignal): Promise<AuthoringOutput> {
    if (signal.aborted) throw new Error("CANCELLED: authoring request cancelled.");
    if (name === "hmi_compose_page") {
      const pageInputs = (input.pages ?? [input]) as Array<Record<string, unknown>>;
      const names = pageInputs.map((page) => string(page, "file"));
      if (names.some((file) => !PAGE.test(file)) || new Set(names).size !== names.length)
        throw new Error("INVALID_ARGUMENT: use unique hmi/pages/<name>.toml destinations.");
      const created = await this.store.createOnce(
        this.workspaceId,
        string(input, "key"),
        requestFingerprint(input),
        async () => {
          const context = await this.queries.context();
          const snapshots = await this.queries.snapshots();
          const files: Record<string, string> = {},
            models: Record<string, HmiPageModel> = {};
          const targets = [];
          for (const pageInput of pageInputs) {
            const file = string(pageInput, "file"),
              target = await this.documents.read(file, signal);
            targets.push(target);
            let model: HmiPageModel;
            let text: string;
            if (pageInput.sourceFile !== undefined) {
              const sourceFile = string(pageInput, "sourceFile");
              if (!PAGE.test(sourceFile)) throw new Error("INVALID_ARGUMENT: sourceFile must identify a project page.");
              if (pageInput.items !== undefined || pageInput.pattern !== undefined || pageInput.layout !== undefined)
                throw new Error("INVALID_ARGUMENT: choose an existing source page or a new composition, not both.");
              if (target.exists && file !== sourceFile)
                throw new Error("CONFLICT: the destination page already exists.");
              const source = await this.documents.read(sourceFile, signal);
              if (!source.exists) throw new Error("PROJECT_UNAVAILABLE: source page does not exist.");
              text = source.text;
              model = await this.model(text, sourceFile);
              if (typeof pageInput.title === "string" && pageInput.title !== model.title) {
                model = { ...model, title: pageInput.title };
                text = await this.text(model);
              }
            } else {
              if (target.exists)
                throw new Error("CONFLICT: page exists; use sourceFile to begin a reviewed edit of that page.");
              const composition: CompositionInput = {
                title: string(pageInput, "title"),
                pattern: pageInput.pattern as CompositionInput["pattern"],
                size: (context.screen as ScreenModel).size,
                items: pageInput.items as CompositionInput["items"],
                connections: pageInput.connections as CompositionInput["connections"],
                layout: pageInput.layout as CompositionInput["layout"],
              };
              // Connections use the canvas's port rules, so project symbols must place their ports.
              model = withProjectSymbols(context.symbols as CustomSymbolModel[], () =>
                composeAgentPage(composition, context.catalogue as Catalogue),
              );
              text = await this.text(model);
            }
            files[file] = text;
            models[file] = model;
          }
          if (signal.aborted) throw new Error("CANCELLED: composition cancelled.");
          if (typeof context.programRevision !== "string")
            throw new Error("NOT_CONNECTED: restart the updated language server to obtain program revisions.");
          const intent = compositionIntent({ ...pageInputs[0], ...input }, models[names[0]].title);
          return {
            intent,
            base: [...snapshots.filter((item) => !names.includes(item.file)), ...targets],
            files,
            programRevision: context.programRevision,
            data: {
              ...(names.length === 1 ? { file: names[0], model: models[names[0]] } : { files: names, models }),
              intent,
              assumptions: intent.assumptions,
              visualReview: "required",
            },
          };
        },
        signal,
      );
      return {
        committed: true,
        data: { ...created.data, draftId: created.draft.id, revision: created.draft.revision },
      };
    }
    if (name === "hmi_apply_draft" && input.action === "discard") {
      const draftId = string(input, "draftId");
      await this.store.discard(draftId, { workspaceId: this.workspaceId, revision: string(input, "revision") }, signal);
      return { committed: true, data: { draftId, discarded: true } };
    }
    const draft = await this.draft(input, name === "hmi_edit_draft");
    // An applied draft keeps its application receipt only: an apply retry returns the original
    // outcome, and every operation that needs the proposal's pages refuses plainly.
    if (!(name === "hmi_apply_draft" && input.action === "apply")) requireRetainedPages(draft);
    if (name === "hmi_apply_draft" && input.action === "apply") {
      const fingerprint = requestFingerprint({
        revision: draft.revision,
        save: input.save === true,
        allowIncomplete: input.allowIncomplete === true,
      });
      let incompleteFiles: Record<string, string> | undefined;
      const applied = await this.store.apply(
        draft.id,
        draft.revision,
        typeof input.key === "string" ? input.key : `apply:${draft.revision}`,
        fingerprint,
        async (current) => {
          const outcome = await applyDraftFiles(
            this.documents,
            incompleteFiles ? { ...current, files: incompleteFiles } : current,
            input.save === true,
            signal,
          );
          return incompleteFiles ? { ...outcome, incomplete: true, deployable: false } : outcome;
        },
        async (current) => {
          if (current.state !== "reviewed")
            throw new Error("VALIDATION_FAILED: review the current draft revision before applying.");
          const context = await this.queries.context();
          if (!current.programRevision || current.programRevision !== context.programRevision)
            throw new Error(
              "CONFLICT: the PLC program changed since this draft was created; create a new proposal from current context.",
            );
          if (current.reviewedAssets && Object.keys(current.reviewedAssets).length) {
            const currentAssets = assetRevisions(
              await loadPreviewAssets(Object.keys(current.reviewedAssets), this.documents, this.native, signal),
            );
            if (Object.entries(current.reviewedAssets).some(([file, revision]) => currentAssets[file] !== revision))
              throw new Error(
                "CONFLICT: a project image changed since review; review the current assets before applying.",
              );
          }
          const result = await this.validate(current, context);
          if (!result.engineeringValid && input.allowIncomplete !== true)
            throw new Error("VALIDATION_FAILED: correct the draft's bindings before applying.");
          if (input.allowIncomplete === true) {
            incompleteFiles = {};
            for (const [file, text] of Object.entries(current.files))
              incompleteFiles[file] = await this.text({ ...(await this.model(text, file)), incomplete: true });
          }
        },
      );
      return { committed: true, data: { draftId: draft.id, revision: draft.revision, ...applied } };
    }
    const context = await this.queries.context();
    if (name === "hmi_edit_draft") {
      const file = this.selectedFile(draft, input);
      const fingerprint = requestFingerprint({
        file,
        operations: input.operations,
        allowUnresolved: input.allowUnresolved === true,
      });
      const edited = await this.store.editDetailed(
        draft.id,
        string(input, "revision"),
        string(input, "key"),
        fingerprint,
        async (current) => {
          const before = await this.model(current.files[file], file);
          const operations = input.operations as Array<Record<string, unknown>>;
          // Pipe edits use the canvas's port rules, so project symbols must place their ports.
          const model = withProjectSymbols(context.symbols as CustomSymbolModel[], () =>
            editAgentPage(before, operations, context.catalogue as Catalogue, (context.screen as ScreenModel).size),
          );
          const text = await this.text(model);
          const after = await this.model(text, file);
          const bindingEdits = (input.operations as Array<Record<string, unknown>>).filter(
            (operation) => operation.op === "bind",
          );
          if (bindingEdits.length && input.allowUnresolved !== true) {
            const check = await this.command("trust-lsp.hmiFileCheck", {
              authoring: true,
              root_uri: this.documents.root.toString(),
              file,
              text,
              remote: context.remoteCatalogue ?? {},
            });
            const affected = new Set(bindingEdits.map((operation) => operation.id));
            const problems = (check.problems ?? []) as Array<{ object?: string; message?: string }>;
            const invalid = problems.filter((problem) => affected.has(problem.object));
            if (invalid.length)
              throw new Error(`VALIDATION_FAILED: ${invalid.map((problem) => problem.message).join("; ")}`);
          }
          if (signal.aborted) throw new Error("CANCELLED: draft edit cancelled.");
          return {
            files: { ...current.files, [file]: text },
            data: {
              file,
              objectChanges: [{ file, ...describePageChanges(before, after) }],
              findings: [],
              validation: "structural-only",
            },
          };
        },
        signal,
      );
      return {
        committed: true,
        data: {
          ...(edited.data ?? { file, validation: "receipt-unavailable" }),
          draftId: edited.draft.id,
          revision: edited.draft.revision,
        },
      };
    }
    if (name === "hmi_validate_draft") {
      const data = await this.validate(draft, context, input.viewports);
      if (input.proposeProgram === true) {
        const proposals = [];
        for (const [file, text] of Object.entries(draft.files))
          proposals.push({
            file,
            ...(await this.command("trust-lsp.hmiAddMissingVariables", {
              root_uri: this.documents.root.toString(),
              file,
              text,
            })),
          });
        data.programProposals = {
          applied: false,
          requiresSeparateReview: true,
          programRevision: context.programRevision,
          proposals,
        };
      }
      return { data };
    }
    if (name === "hmi_render_draft") return this.render(draft, input, context, signal);
    if (name === "hmi_preview_states" && input.mode === "simulator") {
      if (draft.programRevision !== context.programRevision)
        throw new Error("CONFLICT: the program changed since this simulation proposal was created.");
      const models: Record<string, HmiPageModel> = {};
      for (const [file, text] of Object.entries(draft.files))
        models[path.basename(file, ".toml")] = await this.model(text, file);
      return {
        data: await runSimulator(
          this.documents.root.toString(),
          string(input, "sessionId"),
          input.steps as SimulatorStep[],
          models,
          signal,
        ),
        committed: true,
      };
    }
    if (name === "hmi_preview_states") {
      if (
        !Array.isArray(input.states) ||
        input.states.length < 1 ||
        input.states.length > 12 ||
        !input.states.every((state) => typeof state === "string")
      )
        throw new Error("LIMIT_EXCEEDED: provide 1 to 12 state names.");
      const frames: Record<string, unknown>[] = [],
        images: string[] = [];
      for (const state of input.states) {
        const result = await this.render(draft, input, context, signal, state);
        for (const artifact of result.data.artifacts as Array<Record<string, unknown>>)
          artifact.imageIndex = Number(artifact.imageIndex) + images.length;
        frames.push(result.data);
        images.push(...(result.images ?? []));
      }
      return { data: { draftId: draft.id, revision: draft.revision, frames }, images };
    }
    if (name === "hmi_apply_draft") {
      const result = await this.validate(draft, context);
      if (input.action === "review") {
        const reviewedAssets: Record<string, string> = {};
        const rememberAssets = (rendered: AuthoringOutput) => {
          for (const [file, revision] of Object.entries(rendered.data.assetRevisions as Record<string, string>)) {
            if (reviewedAssets[file] && reviewedAssets[file] !== revision)
              throw new Error("CONFLICT: a project image changed during review.");
            reviewedAssets[file] = revision;
            if (Object.keys(reviewedAssets).length > 32)
              throw new Error("LIMIT_EXCEEDED: one proposal can review at most 32 different project images.");
          }
        };
        const exploration =
          input.explore === true ? await prepareExploration(draft, context, this.queries, signal) : undefined;
        const images: string[] = [],
          previews: Array<Record<string, unknown>> = [],
          objectChanges = [],
          reviewFiles: ProposalFile[] = [];
        for (const [file, text] of Object.entries(draft.files)) {
          const base = draft.base.find((snapshot) => snapshot.file === file);
          const before = base?.exists ? await this.model(base.text, file) : undefined;
          const model = await this.model(text, file),
            changes = describePageChanges(before, model);
          objectChanges.push({ file, ...changes });
          let beforeImage: string | undefined;
          if (before && base) {
            const rendered = await this.render(
              { ...draft, files: { ...draft.files, [file]: base.text } },
              { ...input, file },
              context,
              signal,
            );
            for (const artifact of rendered.data.artifacts as Array<Record<string, unknown>>)
              artifact.imageIndex = Number(artifact.imageIndex) + images.length;
            previews.push({
              file,
              kind: "before",
              imageIndex: images.length,
              revision: base.revision,
              origin: "synthetic",
              manifest: rendered.data.manifest,
            });
            rememberAssets(rendered);
            images.push(...(rendered.images ?? []));
            beforeImage = rendered.images?.[0];
          }
          const rendered = await this.render(draft, { ...input, file }, context, signal);
          for (const artifact of rendered.data.artifacts as Array<Record<string, unknown>>)
            artifact.imageIndex = Number(artifact.imageIndex) + images.length;
          previews.push({
            file,
            kind: "after",
            imageIndex: images.length,
            revision: draft.revision,
            origin: "synthetic",
            manifest: rendered.data.manifest,
          });
          rememberAssets(rendered);
          images.push(...(rendered.images ?? []));
          reviewFiles.push({
            file,
            title: model.title,
            changes,
            width: Number(rendered.data.width),
            height: Number(rendered.data.height),
            before: beforeImage,
            after: rendered.images![0],
          });
        }
        if (signal.aborted) throw new Error("CANCELLED: proposal review cancelled.");
        const presentation = await this.present?.(
          {
            draftId: draft.id,
            revision: draft.revision,
            intent: draft.intent,
            engineeringValid: result.engineeringValid === true,
            exploration,
            files: reviewFiles,
            findings: result.findings as import("../editor/proposalModel").ProposalModel["findings"],
          },
          draft.files,
          signal,
        );
        try {
          if (signal.aborted) throw new Error("CANCELLED: proposal review cancelled.");
          const visual = input.visualReview as Parameters<DraftStore["review"]>[5];
          if (visual)
            for (const issue of visual.issues) {
              if (!Object.prototype.hasOwnProperty.call(draft.files, issue.file))
                throw new Error("INVALID_ARGUMENT: visual issue must name a file in this draft.");
              const model = await this.model(draft.files[issue.file], issue.file);
              if (issue.objects.some((id) => !model.object.some((object) => object.id === id)))
                throw new Error("INVALID_ARGUMENT: visual issue names an unknown object.");
            }
          const reviewed = await this.store.review(
            draft.id,
            draft.revision,
            result.findings as unknown[],
            reviewedAssets,
            signal,
            visual,
          );
          result.visualReview = reviewed.visualReview ?? "required";
          presentation?.activate();
        } catch (error) {
          presentation?.close();
          throw error;
        }
        return {
          committed: true,
          data: {
            ...result,
            intent: draft.intent,
            changes: Object.keys(draft.files),
            objectChanges,
            previews,
            assetRevisions: reviewedAssets,
            state: "proposal",
            origin: "synthetic",
            ...(presentation ? { reviewUri: presentation.uri } : {}),
          },
          images,
        };
      }
      throw new Error("INVALID_ARGUMENT: action must be review, apply or discard.");
    }
    throw new Error("INVALID_ARGUMENT: unknown authoring operation.");
  }
}
