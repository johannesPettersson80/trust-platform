import { Node, Edge } from "@xyflow/react";

/**
 * State types for UML StateChart
 */
export type StateType = "normal" | "initial" | "final" | "compound";

/**
 * Data for a state node in the diagram
 */
export interface StateNodeData extends Record<string, unknown> {
  label: string;
  type: StateType;
  entry?: string[];
  exit?: string[];
  on?: Record<string, TransitionConfig>;
  description?: string;
  isActive?: boolean; // Highlights the active state
}

/**
 * Configuration for a transition
 */
export interface TransitionConfig {
  target: string;
  guard?: string;
  actions?: string[];
  description?: string;
  after?: number; // Auto-transition delay in milliseconds
}

/**
 * Extended Node type for StateChart
 */
export interface StateChartNode extends Node {
  data: StateNodeData;
}

/**
 * Extended Edge type for StateChart with transition metadata
 */
export interface StateChartEdge extends Edge {
  data?: {
    event?: string;
    guard?: string;
    actions?: string[];
    description?: string;
    after?: number; // Auto-transition delay in milliseconds
  };
}

/**
 * The editor graph the language server reads from a `.statechart.json` file and writes back
 * (`trust-lsp.visualParse` / `trust-lsp.visualFormat`): states and transitions in file order and
 * the action mappings as they are. Optional members are present when the file has them.
 */
export interface StatechartGraph {
  states: Array<{
    id: string;
    label: string;
    type: StateType;
    entry?: string[];
    exit?: string[];
  }>;
  transitions: Array<{
    id: string;
    source: string;
    target: string;
    event?: string;
    guard?: string;
    actions?: string[];
    after?: number;
  }>;
  actionMappings: Record<string, ActionMapping>;
}

/**
 * VSCode API interface (acquired from webview)
 */
export interface VSCodeAPI {
  postMessage(message: any): void;
  getState(): any;
  setState(state: any): void;
}

/**
 * A value the language server reads from text typed into a panel field (`trust-lsp.visualEdit`):
 * a transition's auto-transition timer, or an action mapping's value.
 */
export type ValueField = "afterDelay" | "mappingValue";

/**
 * The language server's answer for typed text: `after` (milliseconds, `null` for none) or `value`,
 * or why the text is refused.
 */
export interface ValueAnswer {
  ok: boolean;
  after?: number | null;
  value?: unknown;
  errors?: Array<{ message: string }>;
}

/** Asks the language server for the value of the text typed into `field`. */
export type ReadValue = (field: ValueField, text: string) => Promise<ValueAnswer>;

/**
 * A dialog of the action-mapping panel, which the host shows in VS Code, since the webview's sandbox
 * ignores `window.prompt` and `window.confirm`: the name of a new mapping, or the confirmation of a
 * deletion.
 */
export type DialogRequest =
  | { dialog: "addActionMapping"; mappings: Record<string, ActionMapping> }
  | { dialog: "deleteActionMapping"; name: string };

/**
 * The host's answer to a dialog: the mappings with the new one and its name, or whether the deletion
 * is confirmed; nothing when the user cancelled or the language server refused, which the host says.
 */
export interface DialogAnswer {
  mappings?: Record<string, ActionMapping>;
  name?: string;
  confirmed?: boolean;
}

/** Asks the host to show a dialog of the action-mapping panel. */
export type AskDialog = (request: DialogRequest) => Promise<DialogAnswer>;

/**
 * Messages sent from webview to extension
 */
export type WebviewToExtensionMessage =
  | { type: "save"; graph: StatechartGraph }
  | { type: "validate"; graph: StatechartGraph }
  | { type: "generateST"; graph: StatechartGraph }
  | { type: "actionReport"; requestId: number; graph: StatechartGraph }
  | { type: "readValue"; requestId: number; field: ValueField; text: string }
  | ({ type: "dialog"; requestId: number } & DialogRequest)
  | { type: "ready" }
  | { type: "openAsText" };

/**
 * Messages sent from extension to webview: the graph of the document, or why it cannot be read.
 */
export type ExtensionToWebviewMessage =
  | {
      type: "update";
      graph?: StatechartGraph;
      error?: string;
    }
  | {
      /** The language server's action report for the graph of request `requestId`. */
      type: "actionReport";
      requestId: number;
      unmapped: string[];
      unused: string[];
    }
  | ({
      /** The language server's value for the text of request `requestId`. */
      type: "valueResult";
      requestId: number;
    } & ValueAnswer)
  | ({
      /** The answer to the dialog of request `requestId`. */
      type: "dialogResult";
      requestId: number;
    } & DialogAnswer);

/**
 * Action mapping for hardware I/O
 */
export interface ActionMapping {
  action: string; // WRITE_OUTPUT, WRITE_VARIABLE, LOG, etc.
  address?: string; // IEC 61131-3 address (e.g., %QX0.0)
  variable?: string; // Variable name
  value?: any; // Value to write
  message?: string; // Log message
  targets?: Array<{ address: string; value: any }>; // For SET_MULTIPLE
}
