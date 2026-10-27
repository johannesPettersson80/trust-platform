// Equipment objects (specification 39, §6 item 5, §8.5, §21.3): the Components panel's entries,
// the definition an object shows, the instances the inspector offers, and the object a placement,
// a binding or a loop's connected equipment makes are the language server's rules
// (`trust-lsp.hmiEquipmentEdit`). The webview asks for them here and shows the answers.

import type { HmiObjectModel, HmiPageModel } from "../drawing/model";
import type { Catalogue, ComponentDefinition, EquipmentDefinition } from "./catalogue";
import type { EquipmentDetails } from "./equipmentDetails";
import { RUST_RULES, type CallRust, type VariableNode } from "./messages";

/** The Components panel's entries: each definition with its drawing, then the components no
 * definition maps; in a P&ID only what it may hold (§18 phase 4 item 1). */
export async function palette(call: CallRust, catalogue: Catalogue, pid: boolean): Promise<ComponentDefinition[]> {
  const answer = await call<{ palette: ComponentDefinition[] }>(RUST_RULES.equipment, {
    action: "palette",
    catalogue,
    pid,
  });
  return answer.palette;
}

/** The definition each block shows, by the block as the objects record it. */
export async function definitions(
  call: CallRust,
  equipment: EquipmentDefinition[],
  blocks: string[],
): Promise<Record<string, EquipmentDefinition>> {
  const answer = await call<{ definitions: Record<string, EquipmentDefinition> }>(RUST_RULES.equipment, {
    action: "definitions",
    equipment,
    blocks,
  });
  return answer.definitions;
}

/** What the inspector offers an equipment object (§21.3 item 2). */
export interface InstanceChoices {
  /** The declared instances of its block. */
  declared: string[];
  /** Whether its instance is one of them. */
  current: boolean;
  /** The programs equipment can run in. */
  programs: Array<{ path: string; name: string }>;
  /** The program and the name of its instance, or the program a new one goes to. */
  program: string;
  name: string;
}

export function instanceChoices(
  call: CallRust,
  block: string,
  instance: string | undefined,
  variables: VariableNode[],
): Promise<InstanceChoices> {
  return call<InstanceChoices>(RUST_RULES.equipment, { action: "instances", block, instance, variables });
}

/** A new object of a definition, bound to the first free instance when the project has one
 * program (§21.3 item 3). */
export async function placed(
  call: CallRust,
  object: HmiObjectModel,
  definition: EquipmentDefinition,
  variables: VariableNode[],
  page: HmiPageModel,
): Promise<HmiObjectModel> {
  const answer = await call<{ object: HmiObjectModel }>(RUST_RULES.equipment, {
    action: "place",
    object,
    definition,
    variables,
    page,
  });
  return answer.object;
}

/** The object bound to an instance, or to a name in a program: without one its own name, else the
 * program's first free name; nothing when that names no instance. */
export async function boundTo(
  call: CallRust,
  object: HmiObjectModel,
  definition: EquipmentDefinition,
  to: { instance: string } | { program: string; name?: string },
  variables: VariableNode[],
  page: HmiPageModel,
): Promise<HmiObjectModel | undefined> {
  const answer = await call<{ object?: HmiObjectModel }>(RUST_RULES.equipment, {
    action: "bind",
    object,
    definition,
    ...to,
    variables,
    page,
  });
  return answer.object;
}

/** A loop controller with its signal lines following its connected equipment (§6 item 5);
 * nothing when they already do. */
export async function withLoopSignals(
  call: CallRust,
  object: HmiObjectModel,
  page: HmiPageModel,
  dependencies: EquipmentDetails["dependencies"],
): Promise<HmiObjectModel | undefined> {
  const answer = await call<{ object?: HmiObjectModel }>(RUST_RULES.equipment, {
    action: "loopSignals",
    object,
    page,
    dependencies,
  });
  return answer.object;
}
