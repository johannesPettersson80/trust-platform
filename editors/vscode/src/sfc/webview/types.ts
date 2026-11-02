import type { Edge, Node } from "@xyflow/react";

/**
 * Step types for Sequential Function Chart (IEC 61131-3)
 */
export type StepType = "normal" | "initial" | "final";

/**
 * Node types for SFC diagram (includes steps and parallel markers)
 */
export type SfcNodeType = "step" | "parallelSplit" | "parallelJoin";

/**
 * Action qualifiers of IEC 61131-3 Table 59. The language server runs them
 * (`sfc-profile.md`, "Execution of a visual SFC").
 */
export type ActionQualifier =
  | "N" // Non-stored (normal)
  | "S" // Set (stored)
  | "R" // Reset
  | "L" // Time limited
  | "D" // Time delayed
  | "P" // Pulse
  | "SD" // Stored and time delayed
  | "DS" // Delayed and stored
  | "SL" // Stored and time limited
  | "P1" // Pulse on the rising edge
  | "P0"; // Pulse on the falling edge

/**
 * SFC Action definition
 */
export interface SfcAction {
  id: string;
  name: string;
  qualifier: ActionQualifier;
  body: string;
  /**
   * The duration of a time-related qualifier as typed (a TIME literal such as `T#2s`), or a
   * number of milliseconds; the language server checks it.
   */
  time?: number | string;
}

/**
 * Data for a step node in the SFC diagram
 */
export interface StepNodeData extends Record<string, unknown> {
  label: string;
  type: StepType;
  actions?: SfcAction[];
  description?: string;
}

/**
 * Data for a parallel split/join node
 * Parallel splits activate multiple branches simultaneously
 * Parallel joins wait for all branches to complete before continuing
 */
export interface ParallelNodeData extends Record<string, unknown> {
  label: string;
  nodeType: "parallelSplit" | "parallelJoin";
  branchCount?: number; // Number of parallel branches
}

/**
 * Data for a transition edge in the SFC diagram
 */
export interface TransitionData extends Record<string, unknown> {
  condition: string;
  description?: string;
  /** A number in an older chart, or the text of the Priority field as typed; Rust reads and checks it (trust-visual). */
  priority?: number | string;
  label?: string;
}

/**
 * Extended Node type for SFC Steps
 */
export type SfcStepNode = Node<StepNodeData, "step">;

/**
 * Extended Node type for Parallel Split/Join markers
 */
export type SfcParallelSplitNode = Node<ParallelNodeData, "parallelSplit">;
export type SfcParallelJoinNode = Node<ParallelNodeData, "parallelJoin">;
export type SfcParallelNode = SfcParallelSplitNode | SfcParallelJoinNode;

/**
 * Union type for all SFC nodes
 */
export type SfcNode = SfcStepNode | SfcParallelNode;

/**
 * Extended Edge type for SFC Transitions
 */
export type SfcTransitionEdge = Edge<TransitionData> & { data: TransitionData };

/**
 * Variable declaration for SFC
 */
export interface SfcVariable {
  name: string;
  type: string;
  initialValue?: string;
  comment?: string;
}

/**
 * The canvas graph of an SFC file, as the language server reads and writes it
 * (`trust-lsp.visualParse` / `trust-lsp.visualFormat`).
 */
export interface SfcGraph {
  nodes: SfcNode[];
  edges: SfcTransitionEdge[];
  variables?: SfcVariable[];
}

/**
 * A canvas edit the language server answers (`trust-lsp.visualEdit`, kind `sfc`).
 */
export type SfcEdit =
  | {
      op: "connect";
      connection: {
        source: string | null;
        target: string | null;
        sourceHandle?: string | null;
        targetHandle?: string | null;
      };
      fallback: { sourceHandle: string | null; targetHandle: string | null };
      stamp: string;
    }
  | { op: "setBranchCount"; nodeId: string; branchCount: number | null }
  | {
      op: "addNode";
      nodeType: SfcNodeType;
      position: { x: number; y: number };
      wire: boolean;
      stamp: string;
    }
  | { op: "deleteSelection"; nodeIds: string[]; edgeIds: string[] };

/**
 * The answer to an edit.
 */
export interface SfcEditAnswer {
  ok: boolean;
  errors?: Array<{ message: string }>;
  edge?: SfcTransitionEdge | null;
  node?: SfcNode;
  branchCount?: number;
  removedNodeIds?: string[];
  removedEdgeIds?: string[];
}

/**
 * An edit request of the webview.
 */
export interface SfcEditRequest {
  type: "edit";
  requestId: number;
  graph: SfcGraph;
  edit: SfcEdit;
}

/**
 * Messages from webview to extension
 */
export type SfcWebviewToExtensionMessage =
  | { type: "ready" }
  | { type: "save"; graph: SfcGraph }
  | { type: "openAsText" }
  | { type: "validate"; graph: SfcGraph }
  | { type: "previewST"; graph: SfcGraph }
  | { type: "generateST"; graph: SfcGraph }
  | SfcEditRequest;

/**
 * Messages from extension to webview
 */
export type SfcExtensionToWebviewMessage =
  | { type: "update"; graph?: SfcGraph; error?: string }
  | { type: "validationResult"; errors: ValidationError[] }
  | { type: "codeGenerated"; code?: string; errors?: string[] }
  | { type: "editResult"; requestId: number; answer: SfcEditAnswer };

/**
 * Validation error
 */
export interface ValidationError {
  id: string;
  type: "step" | "transition" | "action" | "connection" | "variable";
  message: string;
  elementId?: string;
}
