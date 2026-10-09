// The variable picker's rules (specification 39, §7.1): what a binding is offered, why a dropped
// variable is refused, and which component a variable dropped on the canvas creates. Pure
// functions on the language server's variable tree, shared by the webview and the tests.

import type { Catalogue, ComponentDefinition, PropertyDefinition } from "./catalogue";
import type { VariableNode } from "./messages";

/** A variable as the picker lists it, with the children that remain after filtering. */
export interface PickerRow {
  node: VariableNode;
  children: PickerRow[];
}

/** The catalogue's kinds of value in the binding checks' words. */
const KIND_WORDS: Record<string, string> = {
  number: "a number",
  boolean: "a boolean",
  text: "text",
  time: "a time",
  enumeration: "an enumeration",
  structure: "a structure",
  array: "an array",
  value: "a single value",
};

/** The direction a binding of the property has: the existing binding's, or the first the
 * property allows. */
export function bindingDirection(definition: PropertyDefinition, existing?: string): string {
  return existing ?? definition.directions[0] ?? "read";
}

/** Why the variable cannot be bound to a component's property, in plain words; undefined when it
 * can. The same rules as the binding checks (§7.6), whose results the tree carries. */
export function refusal(
  component: string,
  property: string,
  definition: PropertyDefinition,
  node: VariableNode,
  direction: string,
): string | undefined {
  if (!definition.accepts.some((kind) => node.kinds.includes(kind))) {
    const needed = definition.accepts.map((kind) => KIND_WORDS[kind] ?? kind).join(" or ");
    return `"${node.path}" is ${node.type}; a ${component.replace(/-/g, " ")}'s ${property} needs ${needed}`;
  }
  if (direction !== "read" && !node.writable) {
    return `"${node.path}" ${node.writeBlock ?? "is read-only"} and cannot be written`;
  }
  return undefined;
}

/** A variable's children as the picker lists them: members or fields, or an array's element. */
function childrenOf(node: VariableNode): VariableNode[] {
  return node.array ? [node.array.element] : (node.children ?? []);
}

/** The tree with only the variables `keep` accepts and the programs, blocks and structures that
 * hold them. */
export function pickerTree(nodes: VariableNode[], keep: (node: VariableNode) => boolean): PickerRow[] {
  return nodes.flatMap((node) => {
    const children = pickerTree(childrenOf(node), keep);
    return keep(node) || children.length > 0 ? [{ node, children }] : [];
  });
}

/** Search: the variable's path contains the words, without regard to case. */
export function matchesSearch(node: VariableNode, search: string): boolean {
  return node.path.toLowerCase().includes(search.trim().toLowerCase());
}

/** The component a variable dropped on the canvas creates, and the property bound to it (§7.1):
 * a boolean makes a Status, a structure or an array a Table, any other single value a Value;
 * program and function block instances make nothing. */
export function componentForVariable(
  catalogue: Catalogue,
  node: VariableNode,
): { component: ComponentDefinition; property: string } | undefined {
  const has = (kind: string): boolean => node.kinds.includes(kind);
  const type = has("boolean")
    ? "status"
    : has("structure") || has("array")
      ? "table"
      : has("value")
        ? "value"
        : undefined;
  const component = catalogue.components.find((candidate) => candidate.type === type);
  const property = Object.entries(component?.properties ?? {}).find(([, definition]) =>
    definition.accepts.some(has),
  )?.[0];
  return component && property ? { component, property } : undefined;
}

/** The label of an object made from a variable: the last part of its path. */
export function variableLabel(path: string): string {
  return path.slice(path.lastIndexOf(".") + 1);
}

/** What the editor says when a dropped variable creates nothing: a program or function block
 * instance, whose members can be dropped instead. */
export function whyNoComponent(node: VariableNode): string {
  if (node.kind === "program") return `"${node.path}" is a program: drag one of its variables onto the page`;
  if (node.kind === "function-block")
    return `"${node.path}" is a function block instance (${node.type}): drag one of its members onto the page`;
  return `"${node.path}" (${node.type}) has no component to show it`;
}
