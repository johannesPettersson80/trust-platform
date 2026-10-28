// The variable picker (specification 39, §7.1). What a binding is offered, the binding a typed,
// picked or dropped variable gives and the object a variable dropped on the canvas creates are the
// language server's rules (`trust-lsp.hmiVariablePicker`), the binding checks' (§7.6); the webview
// asks for them here. Listing the tree, an array's element under the array, and searching it by
// path are the picker's own presentation.

import type { HmiBindingModel, HmiObjectModel, HmiPageModel } from "../drawing/model";
import type { ComponentDefinition } from "./catalogue";
import { RUST_RULES, type CallRust, type VariableNode } from "./messages";

/** A variable as the picker lists it, with the rows under it. `offered` is false for a row that
 * only holds the variables a binding may have. */
export interface PickerRow {
  node: VariableNode;
  offered?: boolean;
  children: PickerRow[];
}

/** A binding the picker offers variables for. */
export interface PickerBinding {
  /** The bound object's component type, `pipe` for a pipe's flow (§6 item 6). */
  type: string;
  property: string;
  /** The component's definition in the catalogue. */
  component?: ComponentDefinition;
  /** The binding's own direction, when it has one. */
  direction?: string;
  /** A faceplate's fields, offered first as `$Name` (§8.7). */
  fields?: HmiPageModel["fields"];
}

/** The rows a binding's picker lists: the variables it may have and those that hold them. */
export async function offeredRows(
  call: CallRust,
  variables: VariableNode[],
  binding: PickerBinding,
): Promise<PickerRow[]> {
  const answer = await call<{ rows: PickerRow[] }>(RUST_RULES.variablePicker, {
    action: "offered",
    variables,
    ...binding,
  });
  return answer.rows;
}

/** The object with its property bound to a typed or picked path (an empty one removes the
 * binding), or to a dropped variable, which it may refuse, saying why. */
export async function bound(
  call: CallRust,
  component: ComponentDefinition | undefined,
  object: HmiObjectModel,
  property: string,
  given: { path: string } | { variable: VariableNode },
): Promise<{ object: HmiObjectModel } | { refused: string }> {
  const answer = await call<{ object?: HmiObjectModel; refused?: string }>(RUST_RULES.variablePicker, {
    action: "bind",
    component,
    object,
    property,
    ...given,
  });
  return answer.object ? { object: answer.object } : { refused: answer.refused ?? "" };
}

/** The object a variable dropped on the canvas creates: its type, its binding and its label. */
export interface DroppedVariable {
  type: string;
  bind: Record<string, HmiBindingModel>;
  label: string;
}

/** What a variable dropped on the canvas creates, or why it creates nothing. */
export async function dropped(
  call: CallRust,
  components: ComponentDefinition[],
  variable: VariableNode,
): Promise<DroppedVariable | { refused: string }> {
  const answer = await call<Partial<DroppedVariable> & { refused?: string }>(RUST_RULES.variablePicker, {
    action: "drop",
    components,
    variable,
  });
  return answer.type !== undefined && answer.bind && answer.label !== undefined
    ? { type: answer.type, bind: answer.bind, label: answer.label }
    : { refused: answer.refused ?? "" };
}

/** The rows of a whole tree: each variable with an array's element, or its members, under it. */
export function treeRows(nodes: VariableNode[]): PickerRow[] {
  return nodes.map((node) => ({
    node,
    children: treeRows(node.array ? [node.array.element] : (node.children ?? [])),
  }));
}

/** Search: the rows whose path contains the words, without regard to case, with the rows that
 * hold them; a row that only holds others is not a match itself. */
export function searched(rows: PickerRow[], search: string): PickerRow[] {
  const words = search.trim().toLowerCase();
  if (!words) return rows;
  return rows.flatMap((row) => {
    const children = searched(row.children, words);
    const matches = row.offered !== false && row.node.path.toLowerCase().includes(words);
    return matches || children.length > 0 ? [{ ...row, children }] : [];
  });
}
