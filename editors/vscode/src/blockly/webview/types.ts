/**
 * Type definitions for Blockly Editor webview
 */
import type {
  RuntimeExtensionToWebviewMessage,
  RuntimeWebviewToExtensionMessage,
} from "../../visual/runtime/runtimeMessages";

/**
 * Block category in toolbox
 */
export type BlockCategory =
  | "logic"
  | "loops"
  | "math"
  | "text"
  | "variables"
  | "functions"
  | "io"
  | "plc";

/**
 * Block type definition
 */
export interface BlockType {
  type: string;
  message0: string;
  args0?: Array<{
    type: string;
    name: string;
    check?: string;
    options?: Array<[string, string]>;
  }>;
  output?: string;
  previousStatement?: string | boolean;
  nextStatement?: string | boolean;
  colour?: number;
  tooltip?: string;
  helpUrl?: string;
}

/**
 * Blockly workspace data structure
 */
export interface BlocklyWorkspace {
  blocks: {
    languageVersion: number;
    blocks: BlockDefinition[];
  };
  variables?: Variable[];
  metadata?: {
    name: string;
    description?: string;
    version?: string;
  };
}

/**
 * Individual block definition
 */
export interface BlockDefinition {
  type: string;
  id: string;
  x?: number;
  y?: number;
  fields?: Record<string, any>;
  inputs?: Record<string, BlockInput>;
  next?: BlockDefinition;
  deletable?: boolean;
  movable?: boolean;
  editable?: boolean;
}

/**
 * Block input connection
 */
export interface BlockInput {
  block?: BlockDefinition;
  shadow?: BlockDefinition;
}

/**
 * Variable definition
 */
export interface Variable {
  name: string;
  type: string;
  id: string;
  initial?: any;
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
  | { type: "ready" }
  | { type: "error"; error: string }
  | { type: "validate" }
  | { type: "generateCode" }
  | { type: "executeBlock"; blockId: string }
  | RuntimeWebviewToExtensionMessage;

/**
 * Messages sent from extension to webview
 */
export type ExtensionToWebviewMessage =
  | { type: "update"; content: string }
  | { type: "codeGenerated"; code: string; variables: Array<[string, string]>; errors: string[] }
  | { type: "executionStarted"; mode: ExecutionMode; code: string }
  | { type: "executionStopped" }
  | { type: "blockExecuted"; blockId: string }
  | { type: "highlightBlock"; blockId: string }
  | { type: "unhighlightBlock"; blockId: string }
  | RuntimeExtensionToWebviewMessage;

/**
 * PLC I/O configuration
 */
export interface IOConfig {
  address: string;
  type: "digital" | "analog";
  direction: "input" | "output";
  description?: string;
}

/**
 * Generated ST code result
 */
export interface GeneratedCode {
  structuredText: string;
  variables: Map<string, string>;
  errors: string[];
}

/**
 * Toolbox configuration for Blockly
 */
export interface ToolboxConfig {
  kind: "categoryToolbox";
  contents: ToolboxCategory[];
}

/**
 * Toolbox category
 */
export interface ToolboxCategory {
  kind: "category";
  name: string;
  colour?: string;
  contents: ToolboxItem[];
}

/**
 * Toolbox item (block)
 */
export interface ToolboxItem {
  kind: "block";
  type: string;
}
