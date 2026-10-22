import { CREATE_PLC_COMMAND } from "./generatePlc";
import { CREATE_PAGES_COMMAND } from "./programEdits";
import { EquipmentEditorBridge } from "./equipmentDetails";
import { sharedComposition, openSharedPanel } from "./sharedPanels";
import { isScreenDefinition, resolveScreenEditor } from "./screenEditor";
import { registerRemoteVariables, remoteVariableContext } from "./remoteVariables";
import { openEquipmentControlStudio } from "./controlStudioHandoff";
import { recordSelection, forgetSelection } from "../agent/queryMetadata";
import { proposalFor, PROPOSAL_SCHEME, registerProposalDocuments, resolveProposal } from "./proposals";
// The HMI Builder custom editor (specification 39, §3.1): page files open here, and every edit is
// a text edit of the page's TOML document, so undo, save, the dirty state and Git work as for
// any text file.

import { runtimeLifecycleService } from "../../runtimeLifecycle";
import { LiveOperator, LiveOperatorDeps } from "./liveOperator";
import { runtimeLiveOperatorDeps } from "./liveOperatorRuntime";
import type { QualityWarning } from "./quality";
import { isPidFile, syncLinked } from "./pidSync";
import { DeployDeps, deployBundle, rollbackBundle } from "./deploy";
import { runtimeDeployDeps } from "./deployRuntime";
import { LiveConnection, LivePoller, runtimeLiveConnection } from "./liveMode";
import * as path from "path";
import * as vscode from "vscode";

import type { HmiPageModel } from "../drawing/model";
import {
  languageServerFaceplateEntries,
  languageServerFaceplateInstances,
  languageServerFaceplates,
  languageServerPageFiles,
  languageServerPlacedFaceplates,
  languageServerProjectCatalogue,
  languageServerScreen,
  languageServerVariables,
} from "./languageServerPageFiles";
import { HostToWebview, KEEP_SETTINGS_PATH, WebviewToHost } from "./messages";
import { renameEquipment } from "./renameEquipment";
import { writeScreenColour, writeScreenLanguages } from "./screenFile";
import { changeEquipmentPicture, createOwnEquipment } from "./ownEquipment";
import { IMPORT_SYMBOL_COMMAND } from "./symbolImport";
import { ALLOW_WRITING_COMMAND } from "./writePolicy";
import type { PageFiles } from "./pageDocument";
import { PageSync } from "./pageSync";
import { withAlarmMessages } from "./languages";
import type { AlarmMessage } from "./messages";
import { languageServerTexts, writeTranslation } from "./textFiles";
import { openFaceplate } from "./faceplates";
import { addPage, deletePage, duplicatePage, hidePage, listPages, movePage, renamePage } from "./pages";
import { isSymbolDefinition, resolveSymbolEditor } from "./symbolEditor";

export const HMI_BUILDER_VIEW_TYPE = "trust-lsp.hmiBuilder";
/** The user's panel arrangement (§3.2 item 7), the same in every page. */
const ARRANGEMENT_KEY = "trust-lsp.hmiBuilder.arrangement";
/** Each page's zoom, to open at next time (§3.3 item 8). */
const ZOOM_KEY = "trust-lsp.hmiBuilder.zoom";

/** The project folder of a page or popup file `<project>/hmi/pages/<page>.toml`. */
function projectOf(page: vscode.Uri): vscode.Uri {
  return page.with({ path: path.posix.dirname(path.posix.dirname(path.posix.dirname(page.path))) });
}

/** The names of the project's pages and popups, sorted, which a click can go to (§4.9), and the
 * faceplates it can open (§8.7). */
async function clickTargets(project: vscode.Uri): Promise<{ pages: string[]; popups: string[]; faceplates: string[] }> {
  const names = async (folder: string): Promise<string[]> => {
    try {
      const entries = await vscode.workspace.fs.readDirectory(vscode.Uri.joinPath(project, "hmi", folder));
      return entries
        .filter(([name, type]) => type === vscode.FileType.File && name.endsWith(".toml"))
        .map(([name]) => name.slice(0, -".toml".length))
        .sort();
    } catch {
      return [];
    }
  };
  return {
    pages: await names("pages"),
    popups: [...new Set([...(await names("panels")), ...(await names("popups"))])],
    faceplates: await languageServerFaceplates(project),
  };
}

/** Whether a file is one of the project's ST sources. */
function isProjectSource(file: vscode.Uri, project: vscode.Uri): boolean {
  const relative = path.relative(project.fsPath, file.fsPath);
  return (
    file.scheme === "file" && /\.st$/i.test(file.fsPath) && !relative.startsWith("..") && !path.isAbsolute(relative)
  );
}

/** Where the editor gets the binding checks' problems of its page (§7.6). */
export interface ProblemSource {
  objectProblems(uri: vscode.Uri): Record<string, string[]>;
  /** The quality warnings of a page, by object id (§14.1 item 4). */
  objectWarnings?(uri: vscode.Uri): Record<string, QualityWarning[]>;
  /** The warnings about a project's own colours (§14.1 item 4). */
  colourWarnings?(root: string): QualityWarning[];
  onDidChange: vscode.Event<vscode.Uri>;
}

export type { LiveConnection } from "./liveMode";

/** What the editor reaches outside VS Code; tests give their own. */
export interface EditorOptions {
  files?: PageFiles;
  problems?: ProblemSource;
  live?: LiveConnection;
  deploy?: DeployDeps;
  operator?: LiveOperatorDeps;
}

export class HmiBuilderEditorProvider implements vscode.CustomTextEditorProvider {
  static register(context: vscode.ExtensionContext, problems?: ProblemSource): vscode.Disposable {
    return vscode.Disposable.from(
      registerProposalDocuments(),
      vscode.window.registerCustomEditorProvider(
        HMI_BUILDER_VIEW_TYPE,
        new HmiBuilderEditorProvider(context, { problems }),
        {
          webviewOptions: { retainContextWhenHidden: true },
          supportsMultipleEditorsPerDocument: false,
        },
      ),
    );
  }

  private readonly files: PageFiles;
  private readonly problems?: ProblemSource;
  private readonly live?: LiveConnection;
  private readonly deploy?: DeployDeps;
  private readonly operator?: LiveOperatorDeps;

  constructor(
    private readonly context: vscode.ExtensionContext,
    options: EditorOptions = {},
  ) {
    this.files = options.files ?? languageServerPageFiles;
    this.problems = options.problems;
    this.live = options.live;
    this.deploy = options.deploy;
    this.operator = options.operator;
  }

  resolveCustomTextEditor(document: vscode.TextDocument, panel: vscode.WebviewPanel): void {
    const proposal = proposalFor(document.uri);
    if (document.uri.scheme === PROPOSAL_SCHEME && !proposal)
      throw new Error("EXPIRED_DRAFT: proposal view is unavailable.");
    const screenDocument = isScreenDefinition(document.uri);
    const project =
      proposal?.project ??
      (screenDocument
        ? document.uri.with({ path: path.posix.dirname(path.posix.dirname(document.uri.path)) })
        : projectOf(document.uri));
    registerRemoteVariables(project, this.operator?.session);
    const media = vscode.Uri.joinPath(this.context.extensionUri, "media");
    const shared = vscode.Uri.joinPath(this.context.extensionUri, "src", "webview");
    panel.webview.options = { enableScripts: true, localResourceRoots: [media, shared, project] };
    if (proposal) {
      resolveProposal(proposal, panel);
      panel.webview.html = this.html(panel.webview, media, project, false, true);
      return;
    }
    if (screenDocument) {
      panel.webview.html = this.html(panel.webview, media, project, false, false, true);
      resolveScreenEditor(document, panel);
      return;
    }
    const symbol = isSymbolDefinition(document.uri);
    panel.webview.html = this.html(panel.webview, media, project, symbol);
    if (symbol) return resolveSymbolEditor(document, panel);

    // Answers can arrive after the editor was closed; a closed panel's webview is gone.
    let open = true;
    const post = (message: HostToWebview): void => {
      if (open) void panel.webview.postMessage(message);
    };
    // A page and its P&ID share their process objects (§18 phase 4 item 2).
    let screenSize: [number, number] = [1920, 1080];
    // The page's placed faceplates bound to their instances (§8.9): again when the placements change,
    // and when a faceplate file changes.
    let loadedModel: HmiPageModel | undefined;
    let placementsKey: string | undefined;
    const pagePath = path.relative(project.fsPath, document.uri.fsPath).split(path.sep).join("/");
    let selectedIds: string[] = [];
    const publishSelection = (): void =>
      recordSelection(
        project,
        panel,
        { file: pagePath, ids: selectedIds, documentVersion: document.version },
        panel.active,
      );
    const selectionListener = panel.onDidChangeViewState(publishSelection);
    publishSelection();
    const sendPlaced = async (force = false): Promise<void> => {
      const placements = (loadedModel?.object ?? []).filter((object) => object.type === "faceplate");
      const key = JSON.stringify(
        placements.map((object) => [object.id, object.faceplate, object.instance, object.tag]),
      );
      if (!force && key === placementsKey) return;
      placementsKey = key;
      post({
        type: "placed",
        placed:
          placements.length > 0 ? await languageServerPlacedFaceplates(project, document.getText(), pagePath) : {},
      });
    };
    const sendFaceplates = async (): Promise<void> =>
      post({ type: "faceplates", faceplates: await languageServerFaceplateEntries(project) });
    let compositionRequest = 0;
    const sendComposition = async (): Promise<void> => {
      const request = ++compositionRequest;
      const version = document.version;
      try {
        const result = await sharedComposition(project, document);
        if (request !== compositionRequest || version !== document.version) return;
        if (result.errors) post({ type: "editRefused", errors: result.errors });
        else post({ type: "sharedPanels", model: result.model });
      } catch (error) {
        post({ type: "editRefused", errors: [{ message: String(error) }] });
      }
    };
    const sync = new PageSync(document, post, this.files, (model) => {
      void syncLinked(project, document.uri, model, screenSize, this.files);
      loadedModel = model;
      void sendComposition();
      void sendPlaced();
    });
    const equipmentEditor = new EquipmentEditorBridge(project, document, post);
    panel.onDidDispose(() => equipmentEditor.dispose());
    const operator = new LiveOperator(this.operator ?? runtimeLiveOperatorDeps(project.fsPath), post);
    const live = new LivePoller(this.live ?? runtimeLiveConnection, post);
    // The current values of the variables the pickers show (§3.3 item 8).
    const watching = new LivePoller(this.live ?? runtimeLiveConnection, post, "picker");
    // Deploy and Roll back (§16.2, item 8), one at a time; Deploy saves the HMI files first.
    let deploying = false;
    const runDeploy = async (action: "deploy" | "rollback"): Promise<void> => {
      if (deploying) return;
      deploying = true;
      post({ type: "deploying", action });
      try {
        const deps = this.deploy ?? runtimeDeployDeps;
        if (action === "deploy") await saveHmiFiles(project);
        const outcome = action === "deploy" ? await deployBundle(project.fsPath, deps) : await rollbackBundle(deps);
        post({ type: "deployed", action, outcome });
      } catch (error) {
        post({
          type: "deployed",
          action,
          outcome: { ok: false, reason: error instanceof Error ? error.message : String(error) },
        });
      } finally {
        deploying = false;
      }
    };
    // The program's variables, and the messages it gives its alarms, which join the texts (§14.5).
    let alarmMessages: AlarmMessage[] = [];
    let sourceTimer: ReturnType<typeof setTimeout> | undefined;
    let variablesRequest = 0;
    const sendVariables = async (): Promise<void> => {
      const request = ++variablesRequest;
      const { variables, problem, alarms } = await languageServerVariables(project);
      const remote = await remoteVariableContext(project);
      if (!open || request !== variablesRequest) return;
      post({
        type: "variables",
        variables: [...variables, ...remote.variables],
        ...(problem !== undefined ? { problem } : {}),
      });
      alarmMessages = alarms;
      sendTexts();
    };
    const sendTargets = async (): Promise<void> => post({ type: "targets", ...(await clickTargets(project)) });
    const pageName = path.basename(document.uri.fsPath, ".toml");
    /** The project's pages for the Pages panel (§3.3 item 8). */
    const sendPages = async (): Promise<void> =>
      post({ type: "pages", pages: await listPages(project), current: pageName });
    const pageFile = (name: string): vscode.Uri => vscode.Uri.joinPath(project, "hmi", "pages", `${name}.toml`);
    // The binding checks' problems of this page, for the canvas (§7.6).
    const sendProblems = (): void => {
      if (this.problems) {
        post({
          type: "problems",
          problems: this.problems.objectProblems(document.uri),
          warnings: this.problems.objectWarnings?.(document.uri) ?? {},
        });
      }
    };
    // The project's own colours (§14.1 item 2) and their warnings (item 4).
    const screenUri = vscode.Uri.joinPath(project, "hmi", "screen.toml");
    const sendColourWarnings = (): void =>
      post({ type: "colourWarnings", warnings: this.problems?.colourWarnings?.(project.fsPath) ?? [] });
    const writeColour = async (theme: "dark" | "light", token: string, value: string | undefined): Promise<void> => {
      const written = await writeScreenColour(screenUri, theme, token, value);
      if (written) post({ type: "screen", screen: written });
    };
    // The zoom this page was last left at (§3.3 item 8), where the workspace keeps state.
    const zoomKey = `${ZOOM_KEY}:${document.uri.toString()}`;
    const workspaceState = this.context.workspaceState as vscode.Memento | undefined;
    const start = async (): Promise<void> => {
      post({ type: "arrangement", arrangement: this.context.globalState.get(ARRANGEMENT_KEY) ?? null });
      const zoom = workspaceState?.get<number>(zoomKey);
      if (typeof zoom === "number") post({ type: "zoom", zoom });
      if (isPidFile(document.uri)) post({ type: "view", view: "pid" });
      await sendCatalogue();
      await sendScreen();
      sendTexts();
      await sync.refresh();
      sendProblems();
      sendColourWarnings();
      await sendTargets();
      await sendFaceplates();
      await sendPages();
      await sendVariables();
    };
    // Pages and popups created or deleted change what a click can go to.
    const targetFiles = vscode.workspace.createFileSystemWatcher(
      new vscode.RelativePattern(project, "hmi/{pages,panels,popups}/*.toml"),
      false,
      true,
      false,
    );
    // The screen file, saved from any editor, gives the canvas its size, theme, symbol style and
    // colours (§3.4); theme and symbol style are set only there.
    const screenFile = vscode.workspace.createFileSystemWatcher(new vscode.RelativePattern(project, "hmi/screen.toml"));
    const sendScreen = async (): Promise<void> => {
      const screen = await languageServerScreen(screenUri);
      screenSize = screen.size;
      post({ type: "screen", screen });
      void sendComposition();
    };
    // The project's languages and texts (§14.5 items 7 and 8), again whenever an HMI file changes:
    // a text file, the screen's languages, or a page's texts, saved or not.
    let textsTimer: ReturnType<typeof setTimeout> | undefined;
    const sendTexts = (delay = 0): void => {
      if (textsTimer) clearTimeout(textsTimer);
      textsTimer = setTimeout(() => {
        textsTimer = undefined;
        void languageServerTexts(project).then((texts) => {
          if (texts) post({ type: "texts", texts: withAlarmMessages(texts, alarmMessages) });
        });
      }, delay);
    };
    // The catalogue with the project's own symbols, again whenever a symbol file changes (§14.4).
    const symbolFiles = vscode.workspace.createFileSystemWatcher(
      new vscode.RelativePattern(project, "hmi/symbols/*.{toml,svg}"),
    );
    let catalogueTimer: ReturnType<typeof setTimeout> | undefined;
    const sendCatalogue = async (): Promise<void> => {
      const found = await languageServerProjectCatalogue(project);
      if (found)
        post({ type: "catalogue", catalogue: found.catalogue, symbols: found.symbols, formats: found.formats });
    };
    const resendCatalogue = (): void => {
      if (catalogueTimer) clearTimeout(catalogueTimer);
      catalogueTimer = setTimeout(() => void sendCatalogue(), 200);
    };
    const hmiFolder = vscode.Uri.joinPath(project, "hmi").fsPath + path.sep;
    const hmiFiles = vscode.workspace.createFileSystemWatcher(new vscode.RelativePattern(project, "hmi/**/*.toml"));
    const operatingFiles = vscode.workspace.createFileSystemWatcher(new vscode.RelativePattern(project, "hmi/**/*"));
    const hmiChanged = (uri: vscode.Uri): void => {
      if (/[/\\]panels[/\\][^/\\]+\.toml$/.test(uri.fsPath)) {
        void sendComposition();
        void sendFaceplates();
        void sendPlaced(true);
      }
      if (uri.toString() === screenUri.toString()) void sendScreen();
      operator.stop("The HMI changed. Save and deploy it before operating again.");
      if (/[/\\](pages|popups)[/\\][^/\\]+\.toml$/.test(uri.fsPath)) resendCatalogue();
    };
    const subscriptions = [
      operatingFiles,
      operatingFiles.onDidCreate(hmiChanged),
      operatingFiles.onDidChange(hmiChanged),
      operatingFiles.onDidDelete(hmiChanged),
      runtimeLifecycleService.onDidChange(() => void operator.checkTarget()),
      targetFiles,
      screenFile,
      hmiFiles,
      symbolFiles,
      symbolFiles.onDidCreate(resendCatalogue),
      symbolFiles.onDidChange(resendCatalogue),
      symbolFiles.onDidDelete(resendCatalogue),
      hmiFiles.onDidCreate(() => sendTexts(200)),
      hmiFiles.onDidChange(() => sendTexts(200)),
      hmiFiles.onDidDelete(() => sendTexts(200)),
      // A faceplate file changes every placement of it, and the faceplates to place (§8.9).
      ...[hmiFiles.onDidCreate, hmiFiles.onDidChange, hmiFiles.onDidDelete].map((event) =>
        event((uri) => {
          if (!uri.fsPath.startsWith(path.join(hmiFolder, "panels") + path.sep)) return;
          void sendFaceplates();
          void sendPlaced(true);
        }),
      ),
      screenFile.onDidCreate(() => void sendScreen()),
      screenFile.onDidChange(() => void sendScreen()),
      screenFile.onDidDelete(() => void sendScreen()),
      ...(this.problems
        ? [
            this.problems.onDidChange((uri) => {
              if (uri.toString() === document.uri.toString()) sendProblems();
              if (uri.toString() === screenUri.toString()) sendColourWarnings();
            }),
          ]
        : []),
      targetFiles.onDidCreate(() => {
        void sendTargets();
        void sendPages();
      }),
      targetFiles.onDidDelete(() => {
        void sendTargets();
        void sendPages();
      }),
      // A page's title, order or hiding, saved from any editor, shows in the Pages panel.
      vscode.workspace.onDidSaveTextDocument((saved) => {
        if (
          saved.uri.scheme === "file" &&
          path.dirname(saved.uri.fsPath) === vscode.Uri.joinPath(project, "hmi", "pages").fsPath
        )
          void sendPages();
      }),
      vscode.workspace.onDidChangeTextDocument((change) => {
        // VS Code also reports a change of the unsaved state alone, without a content change.
        if (change.contentChanges.length === 0) return;
        if (change.document.uri.toString() === document.uri.toString()) void sync.refresh();
        if (
          isProjectSource(change.document.uri, project) ||
          change.document.uri.path.startsWith(`${project.path}/hmi/pages/`) ||
          change.document.uri.path === `${project.path}/plant.toml`
        ) {
          if (sourceTimer) clearTimeout(sourceTimer);
          sourceTimer = setTimeout(() => {
            if (open) void sendVariables();
          }, 350);
        }
        if (change.document.uri.scheme === "file" && change.document.uri.fsPath.startsWith(hmiFolder)) {
          hmiChanged(change.document.uri);
          sendTexts(300);
        }
      }),
      vscode.workspace.onDidSaveTextDocument((saved) => {
        if (isProjectSource(saved.uri, project)) void sendVariables();
      }),
      panel.webview.onDidReceiveMessage((message: WebviewToHost) => {
        switch (message.type) {
          case "createPlcCode":
            void vscode.commands
              .executeCommand(CREATE_PLC_COMMAND, { folder: project.toString() })
              .then(undefined, (error) => vscode.window.showErrorMessage(String(error)));
            return;
          case "createHmi":
            void vscode.commands
              .executeCommand(CREATE_PAGES_COMMAND, { folder: project.toString() })
              .then(undefined, (error) => vscode.window.showErrorMessage(String(error)));
            return;
          case "equipmentInspect":
            // Read-only: a just-placed instance is inspected before its edit reaches the document,
            // with the block its object records (specification 39, §21.3 item 3).
            if (/^[A-Za-z_][A-Za-z0-9_]*(\.[A-Za-z_][A-Za-z0-9_]*)+$/.test(message.instance))
              void equipmentEditor.inspect(message.instance, message.block);
            return;
          case "equipmentHardware":
            if (loadedModel?.object.some((object) => object.instance === message.instance))
              void vscode.commands.executeCommand("trust-lsp.networkCanvas.open", {
                equipment: { project: project.toString(), page: document.uri.toString(), instance: message.instance },
              });
            return;
          case "equipmentRename":
            if (loadedModel?.object.some((object) => object.instance === message.instance))
              void renameEquipment(project, message.instance, message.name).then((problem) => {
                if (problem) void vscode.window.showWarningMessage(`${message.instance} was not renamed: ${problem}`);
              });
            return;
          case "allowWriting":
            void vscode.commands.executeCommand(ALLOW_WRITING_COMMAND, {
              page: document.uri.toString(),
              point: message.point,
            });
            return;
          case "equipmentConfigure":
            if (loadedModel?.object.some((object) => object.instance === message.instance))
              void equipmentEditor.configure(message.instance, message.changes, message.revision);
            return;
          case "openSharedPanel":
            void openSharedPanel(project, message.name);
            return;
          case "openControlStudio": {
            const object = loadedModel?.object.find((object) => object.id === message.id);
            if (object)
              void openEquipmentControlStudio(this.context, project, object).catch((error) =>
                vscode.window.showErrorMessage(String(error)),
              );
            return;
          }
          case "selection":
            if (
              Array.isArray(message.ids) &&
              message.ids.length <= 500 &&
              message.ids.every((id) => typeof id === "string")
            ) {
              selectedIds = message.ids.filter((id) => loadedModel?.object.some((object) => object.id === id));
              publishSelection();
            }
            return;
          case "operateOpen":
          case "operateClose":
          case "operateRequest":
            // Keeping the PLC's settings in the project is the editor's own work (§21.4 item 6).
            if (message.type === "operateRequest" && message.path === KEEP_SETTINGS_PATH && message.method === "POST") {
              const body = message.body as { instance?: unknown; settings?: unknown } | undefined;
              const settings = body?.settings;
              void (async () => {
                const answer =
                  typeof body?.instance === "string" &&
                  settings &&
                  typeof settings === "object" &&
                  !Array.isArray(settings)
                    ? await equipmentEditor.keep(
                        body.instance,
                        settings as Record<string, boolean | number | string | null>,
                      )
                    : { ok: false, error: "No settings to keep." };
                post({ type: "operateReply", id: message.id, request: message.request, status: 200, body: answer });
              })();
              return;
            }
            void operator.handle(message);
            return;
          case "operateSocket":
          case "operateSocketClose":
            void operator.handle(message);
            return;
          case "ready":
            void start();
            return;
          case "edit":
            void sync.apply(message.model);
            return;
          case "openAsText":
            void vscode.commands.executeCommand("vscode.openWith", document.uri, "default");
            return;
          case "copy":
            void vscode.env.clipboard.writeText(message.text);
            return;
          case "zoom":
            void workspaceState?.update(zoomKey, message.zoom);
            return;
          case "faceplateInstances":
            void languageServerFaceplateInstances(project, message.faceplate).then((answer) =>
              post({ type: "faceplateInstances", faceplate: message.faceplate, ...answer }),
            );
            return;
          case "openFaceplate":
            void openFaceplate(project, message.name).then((opened) => {
              if (!opened)
                void vscode.window.showWarningMessage(
                  `The faceplate "${message.name}" was not found in this project or its libraries.`,
                );
            });
            return;
          case "pageOpen":
            void vscode.commands.executeCommand("vscode.openWith", pageFile(message.name), HMI_BUILDER_VIEW_TYPE);
            return;
          case "pageAdd":
            void addPage(project).then(async (name) => {
              await sendPages();
              await vscode.commands.executeCommand("vscode.openWith", pageFile(name), HMI_BUILDER_VIEW_TYPE);
            });
            return;
          case "pageRename":
            void renamePage(project, message.name, message.title).then(sendPages);
            return;
          case "pageDuplicate":
            void duplicatePage(project, message.name).then(sendPages);
            return;
          case "pageMove":
            void movePage(project, message.name, message.before).then(sendPages);
            return;
          case "pageHide":
            void hidePage(project, message.name, message.hidden).then(sendPages);
            return;
          case "pageDelete": {
            const name = message.name;
            const confirm = async (): Promise<boolean> =>
              (await vscode.window.showWarningMessage(
                `Delete the page "${name}"? Its file is deleted.`,
                { modal: true },
                "Delete",
              )) === "Delete";
            void deletePage(project, name, confirm).then(sendPages);
            return;
          }
          case "paste":
            void vscode.env.clipboard.readText().then((text) => post({ type: "clipboard", text }));
            return;
          case "arrangement":
            void this.context.globalState.update(ARRANGEMENT_KEY, message.arrangement);
            return;
          case "live":
            live.set(message.on, message.paths ?? []);
            return;
          case "watch":
            watching.set(message.paths.length > 0, message.paths);
            return;
          case "deploy":
          case "rollback":
            void runDeploy(message.type);
            return;
          case "colour":
            void writeColour(message.theme, message.token, message.value);
            return;
          case "ensureEquipment":
            void createOwnEquipment(project, message.block).then((refusal) => {
              if (refusal) post({ type: "editRefused", errors: [{ message: refusal }] });
              void sendCatalogue();
              void sendFaceplates();
            });
            return;
          case "equipmentPicture":
            void changeEquipmentPicture(project, message.block, message.symbol, message.bind ?? {}).then((refusal) => {
              post({ type: "equipmentConfigured", instance: message.instance, ...(refusal ? { error: refusal } : {}) });
              void sendCatalogue();
              void equipmentEditor.inspect(message.instance, message.block);
            });
            return;
          case "importPicture":
            void vscode.commands
              .executeCommand(IMPORT_SYMBOL_COMMAND, { project: project.fsPath })
              .then(() => void sendCatalogue());
            return;
          case "screenLanguages":
            void writeScreenLanguages(screenUri, message.languages).then((refusal) => {
              if (refusal)
                post({ type: "editRefused", errors: [{ file: "hmi/screen.toml", line: 1, message: refusal }] });
              sendTexts();
            });
            return;
          case "translate":
            void writeTranslation(project, message.language, message.text, message.translation).then((errors) => {
              if (errors.length > 0) post({ type: "editRefused", errors });
              sendTexts();
            });
            return;
        }
      }),
    ];
    panel.onDidDispose(() => {
      selectionListener.dispose();
      forgetSelection(project, panel);
      open = false;
      if (textsTimer) clearTimeout(textsTimer);
      if (sourceTimer) clearTimeout(sourceTimer);
      operator.stop();
      live.stop();
      watching.stop();
      subscriptions.forEach((subscription) => subscription.dispose());
    });
  }

  private html(
    webview: vscode.Webview,
    media: vscode.Uri,
    project: vscode.Uri,
    symbol = false,
    proposal = false,
    screen = false,
  ): string {
    const script = webview.asWebviewUri(vscode.Uri.joinPath(media, "hmiBuilderWebview.js"));
    const style = webview.asWebviewUri(vscode.Uri.joinPath(media, "hmiBuilderWebview.css"));
    // truST's shared webview tokens (§3.5 item 5), before the editor's own styles.
    const theme = webview.asWebviewUri(vscode.Uri.joinPath(this.context.extensionUri, "src", "webview", "theme.css"));
    const font = webview.asWebviewUri(vscode.Uri.joinPath(media, "fonts", "InterVariable.woff2"));
    const assets = webview.asWebviewUri(project);
    const csp = [
      "default-src 'none'",
      `img-src ${webview.cspSource} data: https:`,
      `style-src ${webview.cspSource} 'unsafe-inline'`,
      `script-src ${webview.cspSource}`,
      `font-src ${webview.cspSource} data:`,
    ].join("; ");
    return `<!doctype html>
<html lang="en">
<head>
<meta charset="utf-8">
<meta http-equiv="Content-Security-Policy" content="${csp}">
<meta name="viewport" content="width=device-width, initial-scale=1">
<link rel="stylesheet" href="${theme}">
<link rel="stylesheet" href="${style}">
<title>HMI Builder</title>
</head>
<body>
<div id="root" data-font-url="${font}" data-asset-base="${assets}" data-view="${screen ? "screen" : proposal ? "proposal" : symbol ? "symbol" : "page"}"></div>
<script src="${script}"></script>
</body>
</html>`;
  }
}

/** Saves the project's HMI files the editor holds unsaved, so the bundle has them (§16.2, item 8). */
async function saveHmiFiles(project: vscode.Uri): Promise<void> {
  const hmi = vscode.Uri.joinPath(project, "hmi").fsPath + path.sep;
  for (const document of vscode.workspace.textDocuments) {
    if (document.isDirty && document.uri.scheme === "file" && document.uri.fsPath.startsWith(hmi))
      await document.save();
  }
}
