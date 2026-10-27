/**
 * The Ladder program model the editor draws (schema v2, specification 16 §2). The language server
 * owns every rule about it (`trust-lsp.visual*`); these types only describe what it sends.
 */

export type ContactType = "NO" | "NC";
export type CoilType = "NORMAL" | "SET" | "RESET" | "NEGATED";
export type TimerType = "TON" | "TOF" | "TP";
export type CounterType = "CTU" | "CTD" | "CTUD";
export type CompareOp = "GT" | "LT" | "EQ";
export type MathOp = "ADD" | "SUB" | "MUL" | "DIV";
export type ElementType =
  | "contact"
  | "coil"
  | "timer"
  | "counter"
  | "compare"
  | "math"
  | "branchSplit"
  | "branchMerge"
  | "junction";

export interface Position {
  x: number;
  y: number;
}

export interface LadderNodeBase {
  id: string;
  type: ElementType;
  position: Position;
}

export interface Contact extends LadderNodeBase {
  type: "contact";
  contactType: ContactType;
  variable: string;
}

export interface Coil extends LadderNodeBase {
  type: "coil";
  coilType: CoilType;
  variable: string;
}

export interface Timer extends LadderNodeBase {
  type: "timer";
  timerType: TimerType;
  instance: string;
  input?: string;
  presetMs: number;
  qOutput: string;
  etOutput: string;
}

export interface Counter extends LadderNodeBase {
  type: "counter";
  counterType: CounterType;
  instance: string;
  input?: string;
  preset: number;
  qOutput: string;
  cvOutput: string;
}

export interface CompareNode extends LadderNodeBase {
  type: "compare";
  op: CompareOp;
  left: string;
  right: string;
}

export interface MathNode extends LadderNodeBase {
  type: "math";
  op: MathOp;
  left: string;
  right: string;
  output: string;
}

export interface BranchSplitNode extends LadderNodeBase {
  type: "branchSplit";
}

export interface BranchMergeNode extends LadderNodeBase {
  type: "branchMerge";
}

export interface JunctionNode extends LadderNodeBase {
  type: "junction";
}

export type LadderNode =
  | Contact
  | Coil
  | Timer
  | Counter
  | CompareNode
  | MathNode
  | BranchSplitNode
  | BranchMergeNode
  | JunctionNode;

export interface EdgePoint {
  x: number;
  y: number;
}

export interface Edge {
  id: string;
  fromNodeId: string;
  toNodeId: string;
  points?: EdgePoint[];
}

export interface NetworkLayout {
  y: number;
}

export interface Network {
  id: string;
  order: number;
  nodes: LadderNode[];
  edges: Edge[];
  layout: NetworkLayout;
}

export interface LadderProgram {
  schemaVersion: 2;
  networks: Network[];
  variables: Variable[];
  metadata: {
    name: string;
    description: string;
    created?: string;
    modified?: string;
  };
}

export type VariableScope = "local" | "global";

export interface Variable {
  name: string;
  type: "BOOL" | "INT" | "REAL" | "TIME" | "DINT" | "LREAL";
  scope?: VariableScope;
  address?: string;
  initialValue?: unknown;
}

export type LadderElement = LadderNode;
export type Rung = Network;
export type Connection = Edge;
