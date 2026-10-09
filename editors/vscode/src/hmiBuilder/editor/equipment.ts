// Shared FB/component placement: definitions come from the native project catalogue.
import type { HmiObjectModel, HmiPageModel } from "../drawing/model";
import type { Catalogue, ComponentDefinition, EquipmentDefinition } from "./catalogue";
import type { VariableNode } from "./messages";

export function componentTitle(component: ComponentDefinition): string | undefined {
  return component.equipment?.title;
}

export function paletteComponents(catalogue: Catalogue): ComponentDefinition[] {
  const definitions = catalogue.equipment ?? [];
  const mapped = new Set(definitions.map((definition) => definition.symbol));
  const entries = definitions.flatMap((definition) => {
    const drawing = catalogue.components.find((item) => item.type === definition.symbol);
    return drawing ? [{ ...drawing, equipment: definition, category: "equipment" }] : [];
  });
  // Existing unmapped artwork remains usable; never advertise it as an implemented PLC FB.
  return [
    ...entries,
    ...catalogue.components.filter((component) => !mapped.has(component.type) || component.category !== "equipment"),
  ];
}

export function allVariables(nodes: VariableNode[]): VariableNode[] {
  return nodes.flatMap((node) => [node, ...allVariables(node.children ?? [])]);
}
export function matchingInstances(nodes: VariableNode[], block: string): VariableNode[] {
  return allVariables(nodes).filter(
    (node) => node.kind === "function-block" && node.type.toLowerCase() === block.toLowerCase(),
  );
}

export function equipmentFor(
  catalogue: Catalogue | undefined,
  object: HmiObjectModel,
): EquipmentDefinition | undefined {
  return catalogue?.equipment?.find((definition) => definition.block.toLowerCase() === object.block?.toLowerCase());
}

/** What a definition's member starts with to name a variable beside the instance, named after it
 * (specification 39, §8.5): a loop's `{instance}_Setpoint` is `Main.TIC101_Setpoint`. */
export const INSTANCE_PLACEHOLDER = "{instance}";

/** The path of a definition's member of an instance: `Running` of `Main.P101` is
 * `Main.P101.Running`, `{instance}_Setpoint` of `Main.TIC101` is `Main.TIC101_Setpoint`. */
export function memberPath(instance: string, member: string): string {
  return member.startsWith(INSTANCE_PLACEHOLDER)
    ? `${instance}${member.slice(INSTANCE_PLACEHOLDER.length)}`
    : `${instance}.${member}`;
}

export function bindEquipment(
  object: HmiObjectModel,
  definition: EquipmentDefinition,
  instance: string,
): HmiObjectModel {
  const bind = Object.fromEntries(
    Object.entries(definition.bindings).map(([property, member]) => [
      property,
      {
        path: memberPath(instance, member),
        direction: "read" as const,
        ...(definition.quality?.[property] ? { quality: memberPath(instance, definition.quality[property]) } : {}),
      },
    ]),
  );
  const click = object.click?.faceplate ? { ...object.click, faceplate: definition.panel, instance } : object.click;
  return {
    ...object,
    block: definition.block,
    instance,
    faceplate: definition.panel,
    // This stored legacy tag is a projection of the instance, never an independent editor field.
    tag: instance.split(".").pop(),
    bind,
    ...(click ? { click } : {}),
  };
}

/** A screen service program is not an equipment execution destination. */
export function equipmentPrograms(nodes: VariableNode[]): VariableNode[] {
  return nodes.filter(
    (node) =>
      node.kind === "program" &&
      !allVariables(node.children ?? []).some(
        (member) => member.kind === "function-block" && member.type.toLowerCase() === "hmi.display",
      ),
  );
}

/** Only choose the target automatically when the project has one program instance. */
export function newEquipmentInstance(
  definition: EquipmentDefinition,
  nodes: VariableNode[],
  page: HmiPageModel,
): string | undefined {
  const programs = equipmentPrograms(nodes);
  if (programs.length > 1) return undefined;
  return freeEquipmentInstance(definition, nodes, page, programs[0]?.path ?? "Main");
}

/** The first free instance of a program for a definition: its block's name and a number no
 * declaration or object of the page holds (`Main.Pump2` after `Main.Pump1`). */
export function freeEquipmentInstance(
  definition: EquipmentDefinition,
  nodes: VariableNode[],
  page: HmiPageModel | undefined,
  program: string,
): string | undefined {
  const prefix = definition.block.split(".").pop()?.replace(/^FB_/, "") ?? "Equipment";
  const used = new Set(
    [
      ...allVariables(nodes).map((node) => node.path),
      ...(page?.object ?? []).flatMap((object) => (object.instance ? [object.instance] : [])),
    ].map((name) => name.toLowerCase()),
  );
  for (let index = 1; index <= 10000; index += 1) {
    const instance = `${program}.${prefix}${index}`;
    if (!used.has(instance.toLowerCase())) return instance;
  }
  return undefined;
}
