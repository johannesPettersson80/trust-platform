import type { Edge, Node } from "@xyflow/react";
import type {
  RuntimeExtensionToWebviewMessage,
  RuntimeWebviewToExtensionMessage,
} from "../../visual/runtime/runtimeMessages";

/**
 * Step types for Sequential Function Chart (IEC 61131-3)
 */
export type StepType = "normal" | "initial" | "final";

/**
 * Node types for SFC diagram (includes steps and parallel markers)
 */
export type SfcNodeType = "step" | "parallelSplit" | "parallelJoin";

/**
 * Action qualifiers according to IEC 61131-3
 */
export type ActionQualifier =
  | "N"  // Non-stored (normal)
  | "S"  // Set (stored)
  | "R"  // Reset
  | "L"  // Time limited
  | "D"  // Time delayed
  | "P"  // Pulse
  | "SD" // Stored and time delayed
  | "DS" // Delayed and stored
  | "SL"; // Stored and time limited

/**
 * SFC Action definition
 */
export interface SfcAction {
  id: string;
  name: string;
  qualifier: ActionQualifier;
  body: string;
  time?: number; // For time-based qualifiers (L, D, SD, DS, SL)
}

/**
 * Data for a step node in the SFC diagram
 */
export interface StepNodeData extends Record<string, unknown> {
  label: string;
  type: StepType;
  actions?: SfcAction[];
  description?: string;
  isActive?: boolean; // For real-time highlighting during execution
  hasBreakpoint?: boolean; // Debug: step has a breakpoint
  isCurrentDebugStep?: boolean; // Debug: execution paused at this step
  onToggleBreakpoint?: () => void; // Debug: toggle breakpoint callback
  x?: number;
  y?: number;
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
  completedBranches?: Set<string>; // For joins: track which branches completed
  isActive?: boolean; // For runtime highlighting
}

/**
 * Data for a transition edge in the SFC diagram
 */
export interface TransitionData extends Record<string, unknown> {
  condition: string;
  description?: string;
  priority?: number;
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

export interface SfcParallelSplit {
  id: string;
  name: string;
  x: number;
  y: number;
  branchIds: string[];
}

export interface SfcParallelJoin {
  id: string;
  name: string;
  x: number;
  y: number;
  branchIds: string[];
  nextStepId?: string;
}

/**
 * SFC Workspace/Program structure
 */
export interface SfcWorkspace {
  name: string;
  steps: Array<{
    id: string;
    name: string;
    initial?: boolean;
    x: number;
    y: number;
    actions?: SfcAction[];
  }>;
  transitions: Array<{
    id: string;
    name: string;
    condition: string;
    sourceStepId: string;
    targetStepId: string;
    priority?: number;
  }>;
  parallelSplits?: SfcParallelSplit[];
  parallelJoins?: SfcParallelJoin[];
  variables?: SfcVariable[];
  metadata?: {
    author?: string;
    version?: string;
    description?: string;
    created?: string;
    modified?: string;
  };
}

/**
 * Execution state for SFC
 */
export interface SfcExecutionState {
  activeSteps: string[];
  mode: "simulation" | "hardware";
  timestamp?: number;
  status?: "stopped" | "running" | "paused";
  breakpoints?: string[]; // Step IDs with breakpoints
  currentStep?: string | null; // Current step when paused
}

/**
 * Messages from webview to extension
 */
export type SfcWebviewToExtensionMessage =
  | { type: "ready" }
  | { type: "save"; content: string }
  | { type: "error"; error: string }
  | { type: "openAsText" }
  | { type: "addStep" }
  | { type: "addTransition" }
  | { type: "deleteSelected" }
  | { type: "validate" }
  | { type: "generateST"; content?: string }
  | { type: "autoLayout" }
  | { type: "debugPause" }
  | { type: "debugResume" }
  | { type: "debugStepOver" }
  | { type: "toggleBreakpoint"; stepId: string }
  | RuntimeWebviewToExtensionMessage;

/**
 * Messages from extension to webview
 */
export type SfcExtensionToWebviewMessage =
  | { type: "init"; content: string }
  | { type: "update"; content: string }
  | { type: "executionState"; state: SfcExecutionState }
  | { type: "executionStopped" }
  | { type: "validationResult"; errors: ValidationError[] }
  | { type: "codeGenerated"; code?: string; errors?: string[] }
  | RuntimeExtensionToWebviewMessage;

/**
 * Validation error
 */
export interface ValidationError {
  id: string;
  type: "step" | "transition" | "action" | "connection";
  message: string;
  elementId?: string;
}
