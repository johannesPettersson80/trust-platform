import { Node, Edge } from "@xyflow/react";
import type {
  RuntimeExtensionToWebviewMessage,
  RuntimeWebviewToExtensionMessage,
} from "../../visual/runtime/runtimeMessages";

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
  isActive?: boolean; // For real-time highlighting during execution
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
 * XState-compatible JSON format for state machines
 */
export interface XStateConfig {
  id: string;
  initial?: string;
  states: Record<string, XStateStateConfig>;
  actionMappings?: Record<string, ActionMapping>;
}

/**
 * XState state configuration
 */
export interface XStateStateConfig {
  entry?: string[];
  exit?: string[];
  on?: Record<string, XStateTransition | string>;
  type?: "final" | "compound";
}

/**
 * XState transition format
 */
export interface XStateTransition {
  target: string;
  guard?: string;
  actions?: string[];
  after?: number; // Auto-transition delay in milliseconds
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
 * Execution mode: Simulation (mock) or Hardware (real I/O)
 */
export type ExecutionMode = "simulation" | "hardware";

/**
 * Messages sent from webview to extension
 */
export type WebviewToExtensionMessage =
  | { type: "save"; content: string }
  | { type: "validate"; content: string }
  | { type: "generateST"; content: string }
  | { type: "ready" }
  | { type: "error"; error: string }
  | { type: "openAsText" }
  | { type: "sendEvent"; event: string }
  | RuntimeWebviewToExtensionMessage;

/**
 * Messages sent from extension to webview
 */
export type ExtensionToWebviewMessage =
  | { type: "update"; content: string }
  | { type: "init"; content: string }
  | { type: "executionState"; state: ExecutionState }
  | { type: "executionStopped" }
  | RuntimeExtensionToWebviewMessage;

/**
 * Real-time execution state from runtime
 */
export interface ExecutionState {
  currentState: string;
  previousState?: string;
  availableEvents: string[];
  context?: Record<string, any>;
  timestamp?: number;
  mode?: ExecutionMode; // Current execution mode
}

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
