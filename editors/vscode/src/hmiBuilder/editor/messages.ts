import type { ProposalModel, ProposalStatus } from "./proposalModel";
// Messages between the HMI Builder editor and its webview.

import type { CustomSymbolModel } from "../drawing/symbols/custom";
import type { QualityWarning } from "./quality";
import type { DeployOutcome } from "./deploy";
import type { HmiPageModel, PlacedFaceplate } from "../drawing/model";
import type { Arrangement } from "./arrangement";
import type { HmiColorsModel } from "../drawing/tokens";
import type { Catalogue } from "./catalogue";
import type { FileError } from "./pageDocument";

/** The screen layout the canvas draws on (§3.4): design size, theme and project colours. */
export interface ScreenModel {
  size: [number, number];
  theme: "light" | "dark";
  /** The symbol style (§4.1, §14.2): the two-dimensional symbols. */
  symbols?: "flat";
  colors?: HmiColorsModel;
  /** The screen has a top bar (§4 item 8), 56 px above the page area. */
  topBar?: boolean;
  /** How the screen offers its pages (§4 item 7): a side bar beside the page area, tabs above it. */
  navigation?: "side" | "tabs" | "none";
}

export interface ScreenDocumentModel {
  size: [number, number];
  theme: "light" | "dark";
  symbols: "flat";
  start_page?: string;
  languages: string[];
  colors?: HmiColorsModel;
  navigation: { style: "side" | "tabs" | "none"; position: "left" | "right" };
  /** What the top bar shows (§4 item 8); absent without one. */
  top_bar?: { title?: string; logo?: string; clock?: boolean; alarms?: boolean; user?: boolean; page_title?: boolean };
}

export const DEFAULT_SCREEN: ScreenModel = { size: [1920, 1080], theme: "dark" };

/** One variable of the program, as the language server gives it (§7.1, §17.3). */
/** A page as the Pages panel lists it: its file name, its title, and whether the navigation hides it. */
export interface PageEntry {
  name: string;
  title: string;
  hidden: boolean;
}

export interface VariableNode {
  /** Equipment intent that has not yet been generated into PLC source. */
  pending?: boolean;
  name: string;
  /** Runtime instance path, for example `FeedArea.Pumps[1].Running`. */
  path: string;
  kind: "program" | "function-block" | "structure" | "array" | "value";
  /** Declared type, for example `REAL`, `Pump` or `ARRAY[1..3] OF Pump`. */
  type: string;
  /** Kinds of value it holds, as the catalogue names them; empty for program and function block
   * instances. */
  kinds: string[];
  /** Whether a binding may write it (§7.6). */
  writable: boolean;
  /** Why a binding cannot write it, in the binding checks' words, for example `is a constant`. */
  writeBlock?: string;
  access?: "input" | "output" | "in-out" | "internal";
  children?: VariableNode[];
  /** An array's bounds and one element standing for all. */
  array?: { dimensions: Array<[number, number]>; element: VariableNode };
}

/** An alarm of the program and the message it gives it, a text of the project (§14.5 item 8). */
export interface AlarmMessage {
  path: string;
  message: string;
}

/** A project language and its own name (§14.5). */
export interface LanguageEntry {
  tag: string;
  name: string;
}

/** What the language server gives of the project's texts (§14.5 items 7 and 8). */
export interface ProjectTexts {
  /** The screen's languages, the first being that of the project's texts. */
  languages: LanguageEntry[];
  /** Every text the screen, pages and popups show, and where. */
  texts: Array<{ text: string; places: string[] }>;
  /** truST's own words, by their English wording. */
  words: string[];
  /** The project's text files by language. */
  project: Record<string, Record<string, string>>;
  /** truST's words in each project language truST ships. */
  builtin: Record<string, Record<string, string>>;
  /** The languages truST ships its words in, each with its own name (§14.5 item 2); none when
   * not given. */
  shipped?: LanguageEntry[];
  /** The files that do not load. */
  errors: FileError[];
}

export type HostToWebview =
  | {
      type: "equipmentDetails";
      instance: string;
      details?: import("./equipmentDetails").EquipmentDetails;
      error?: string;
    }
  | { type: "equipmentConfigured"; instance: string; error?: string }
  | { type: "sharedPanels"; model?: HmiPageModel }
  | { type: "screenDocument"; model: ScreenDocumentModel; pages: PageEntry[]; shipped?: LanguageEntry[] }
  | { type: "screenError"; message: string }
  | { type: "proposal"; proposal: ProposalModel; active: boolean }
  | { type: "proposalStatus"; status: ProposalStatus; message?: string }
  | { type: "operateReady"; id: string; target?: string; reason?: string }
  | { type: "operateReply"; id: string; request: number; status: number; body: unknown }
  | { type: "operateStream"; id: string; socket: number; data?: string; closed?: boolean }
  | { type: "operateEnded"; id: string; reason: string }
  | { type: "symbol"; model: CustomSymbolModel; theme: "dark" | "light" }
  /** The page as the text says it. */
  | { type: "page"; model: HmiPageModel }
  /** The text does not load: keep the last page, show the errors, pause editing (§3.1.6). */
  | { type: "invalid"; errors: FileError[] }
  /** An edit the language server would not write. */
  | { type: "editRefused"; errors: FileError[] }
  /** The component catalogue for the Components panel. */
  /** The file is a P&ID, shown in the P&ID view (§18 phase 4 item 1). */
  | { type: "view"; view: "pid" }
  /** The component catalogue, with the project's own symbols (§14.4). */
  | {
      type: "catalogue";
      catalogue: Catalogue;
      symbols?: CustomSymbolModel[];
      formats?: import("../drawing/model").DrawFrame["formats"];
    }
  /** The project's screen layout. */
  | { type: "screen"; screen: ScreenModel }
  /** The program's variables for the picker, or why there are none (§7.1). */
  | { type: "variables"; variables: VariableNode[]; problem?: string }
  /** The user's remembered panel arrangement, if any (§3.2). */
  | { type: "arrangement"; arrangement: Arrangement | null }
  /** The project's languages and texts (§14.5 items 7 and 8). */
  | { type: "texts"; texts: ProjectTexts }
  /** The project's pages and popups, which a click can go to or open (§4.9). */
  | { type: "targets"; pages: string[]; popups: string[]; faceplates?: string[] }
  /** The binding checks' problems on this page, and its quality warnings, by object id (§7.6,
   * §14.1 item 4). */
  | {
      type: "problems";
      problems: Record<string, string[]>;
      points?: Record<string, Record<string, string>>;
      warnings?: Record<string, QualityWarning[]>;
    }
  /** The warnings about the project's own colours (§14.1 item 4). */
  | { type: "colourWarnings"; warnings: QualityWarning[] }
  /** Live mode's connection to the run target (§11, item 6). */
  | { type: "live"; state: LiveState; target?: string; reason?: string }
  /** Live mode's values, as `hmi.read` answers them. */
  | { type: "liveValues"; update: LiveUpdate }
  /** The current values of the variables the pickers show (§3.3 item 8); none while no run target answers. */
  | { type: "pickerValues"; update?: LiveUpdate }
  /** A deployment or rollback is under way (§16.2, item 8). */
  | { type: "deploying"; action: "deploy" | "rollback" }
  /** Its outcome. */
  | { type: "deployed"; action: "deploy" | "rollback"; outcome: DeployOutcome }
  /** The clipboard's text, asked for by a paste (§3.3 item 8). */
  | { type: "clipboard"; text: string }
  /** The project's pages in the navigation's order, and the one edited (§3.3 item 8). */
  | { type: "pages"; pages: PageEntry[]; current: string }
  /** The zoom the page was last left at (§3.3 item 8). */
  | { type: "zoom"; zoom: number }
  /** The faceplates of the project and its libraries, for the Faceplates group (§8.9). */
  | { type: "faceplates"; faceplates: FaceplateEntry[] }
  /** The page's placed faceplates bound to their instances, by object id; `null` where the
   * project lacks the faceplate (§8.9). */
  | { type: "placed"; placed: Record<string, PlacedFaceplate | null> }
  /** The instances a faceplate fits, asked for when one is placed (§8.9 item 1). */
  | { type: "faceplateInstances"; faceplate: string; instances: string[]; problem?: string }
  /** The language server's answer to the webview's rule question `id` (§17.3). */
  | { type: "rustAnswer"; id: number; answer: unknown };

/** The language server's HMI Builder rules the webview asks (AGENTS.md: all logic is in Rust;
 * specification 39, §17.3), through the editor with a `rust` message; the editor forwards no
 * other command. */
export const RUST_RULES = {
  pidSync: "trust-lsp.hmiPidSync",
  equipment: "trust-lsp.hmiEquipmentEdit",
  variablePicker: "trust-lsp.hmiVariablePicker",
  clipboard: "trust-lsp.hmiClipboard",
} as const;

/** Asks one of `RUST_RULES`: resolves with its answer, or rejects with the reason it refuses. */
export type CallRust = <T>(command: string, argument: unknown) => Promise<T>;

/** A faceplate the Faceplates group lists. */
export interface FaceplateEntry {
  requiresInstance?: boolean;
  name: string;
  title?: string;
  /** Its design size, which a placement takes with its header (§8.9 item 4). */
  size?: [number, number];
}

export type WebviewToHost =
  | { type: "createPlcCode" | "createHmi" }
  | { type: "equipmentInspect"; instance: string; block?: string }
  | { type: "equipmentHardware"; instance: string }
  | { type: "equipmentRename"; instance: string; name: string }
  /** Let the runtime display write a point a control of the page writes (§12.2.3). */
  | { type: "allowWriting"; point: string }
  | {
      type: "equipmentConfigure";
      instance: string;
      revision: number;
      changes: import("./equipmentDetails").EquipmentChanges;
    }
  | { type: "openSharedPanel"; name: string }
  | { type: "editScreen"; changes: Partial<ScreenDocumentModel> }
  /** Screen settings: add a user of the runtime display, or remove one (§12.8). */
  | { type: "addUser" }
  | { type: "removeUser"; name: string }
  | { type: "openControlStudio"; id: string }
  | { type: "selection"; ids: string[] }
  | { type: "proposalApply" | "proposalDiscard" | "proposalOpenSource"; index?: number }
  | { type: "operateOpen"; id: string; page: string }
  | { type: "operateClose"; id: string }
  | { type: "operateRequest"; id: string; request: number; method: "GET" | "POST"; path: string; body?: unknown }
  | { type: "operateSocket"; id: string; socket: number; path: string }
  | { type: "operateSocketClose"; id: string; socket: number }
  | { type: "editSymbol"; model: CustomSymbolModel }
  | { type: "ready" }
  /** Live mode on with the page's bound paths, or off (§11). */
  | { type: "live"; on: boolean; paths?: string[] }
  /** A project colour chosen in the Colours section, or reset without a value (§14.1 item 2). */
  | { type: "colour"; theme: "dark" | "light"; token: string; value?: string }
  /** The project's languages chosen in the Languages dialog, the default first (§14.5 item 2). */
  | { type: "screenLanguages"; languages: string[] }
  /** One of the project's own blocks placed for the first time: write its proposed files (§21.9). */
  | { type: "ensureEquipment"; block: string }
  /** Another picture for one of the project's own definitions, chosen members by input (§21.9). */
  | { type: "equipmentPicture"; instance: string; block: string; symbol: string; bind?: Record<string, string> }
  /** Import an SVG drawing into My Symbols (§14.4). */
  | { type: "importPicture" }
  /** Deploy the project's HMI, or roll back to the previous one (§16.2, item 8). */
  | { type: "deploy" }
  | { type: "rollback" }
  | { type: "edit"; model: HmiPageModel }
  | { type: "openAsText" }
  /** A text's translation in a language, written to its text file; empty removes it (§14.5). */
  | { type: "translate"; language: string; text: string; translation: string }
  /** The user changed the panel arrangement. */
  | { type: "arrangement"; arrangement: Arrangement }
  /** Copied objects for the clipboard, and a paste asking for its text (§3.3 item 8). */
  | { type: "copy"; text: string }
  | { type: "paste" }
  /** The Pages panel's actions (§3.3 item 8). */
  | { type: "pageOpen"; name: string }
  | { type: "pageAdd" }
  | { type: "pageRename"; name: string; title: string }
  | { type: "pageDuplicate"; name: string }
  | { type: "pageMove"; name: string; before: string | null }
  | { type: "pageHide"; name: string; hidden: boolean }
  | { type: "pageDelete"; name: string }
  /** The page's zoom, to open at next time (§3.3 item 8). */
  | { type: "zoom"; zoom: number }
  /** Opens a faceplate in the editor, from a symbol's context menu (§3.3 item 8). */
  | { type: "openFaceplate"; name: string }
  /** The variables the pickers show, to read their current values; none when no picker is open (§3.3 item 8). */
  | { type: "watch"; paths: string[] }
  /** The instances a faceplate fits, for placing it (§8.9 item 1). */
  | { type: "faceplateInstances"; faceplate: string }
  /** A question to one of the language server's rules, `RUST_RULES` (§17.3). */
  | { type: "rust"; id: number; command: string; argument: unknown };

/** Live mode's connection: values arrive, no target runs, or the connection was lost. */
export type LiveState = "connected" | "none" | "lost";

/** What `hmi.read` answers (§11, item 6): the cycle count, each path's value and the missing paths. */
export interface LiveUpdate {
  cycle: number;
  values: Record<string, { value: unknown; lowerBounds?: number[] } | null>;
  missing?: string[];
}

/** The editor's own Live mode request that keeps settings the PLC has in the project (§21.4 item
 * 6). It never reaches a runtime: the extension host answers it. */
export const KEEP_SETTINGS_PATH = "/project/settings";
