import { equalSize, arrangeRegion } from "./geometricEdits";
import { deleteObjects } from "../editor/editing";
import {
  catalogueTypePorts,
  connectionIndex,
  connectPorts,
  disconnectPorts,
  duplicateObjects,
  reconnectPipe,
} from "./pipeEdits";
import type { HmiPageModel, HmiObjectModel, HmiBindingModel } from "../drawing/model";
import type { Catalogue } from "../editor/catalogue";
import { alignObjects, distributeObjects, AlignEdge, DistributeAxis } from "../editor/align";

const ID = /^[A-Za-z][A-Za-z0-9_-]*$/;
const clone = <T>(value: T): T => JSON.parse(JSON.stringify(value)) as T;
const EDITABLE = new Set([
  "tag",
  "label",
  "text",
  "unit",
  "decimals",
  "min",
  "max",
  "limits",
  "span",
  "variant",
  "role",
  "confirm",
  "action",
  "pulse",
  "expect",
  "timeout",
  "options",
  "attach",
  "group",
  "locked",
  "arrange",
  "gap",
  "image",
  "faceplate",
  "instance",
  "click",
]);
function pair(value: unknown, minimum = -Infinity): [number, number] {
  if (!Array.isArray(value) || value.length !== 2 || !value.every((item) => Number.isInteger(item) && item >= minimum))
    throw new Error("INVALID_ARGUMENT: expected two integer coordinates.");
  return [value[0], value[1]];
}
function name(value: unknown): string {
  if (typeof value !== "string" || !ID.test(value)) throw new Error("INVALID_ARGUMENT: invalid object id.");
  return value;
}
/** Pure draft edits. Native serialization and binding checks still run before persistence. */
export function editAgentPage(
  source: HmiPageModel,
  operations: Array<Record<string, unknown>>,
  catalogue: Catalogue,
  pageSize?: [number, number],
): HmiPageModel {
  if (!Array.isArray(operations) || !operations.length || operations.length > 500)
    throw new Error("LIMIT_EXCEEDED: provide 1 to 500 operations.");
  let page = clone(source);
  const find = (id: unknown): HmiObjectModel => {
    const object = page.object.find((candidate) => candidate.id === id);
    if (!object) throw new Error(`INVALID_ARGUMENT: unknown object ${String(id)}.`);
    return object;
  };
  const selected = (ids: unknown): HmiObjectModel[] => {
    if (!Array.isArray(ids) || !ids.length || new Set(ids).size !== ids.length)
      throw new Error("INVALID_ARGUMENT: provide unique selected object ids.");
    return ids.map(find);
  };
  const movable = (objects: HmiObjectModel[]): void => {
    if (objects.some((object) => object.locked)) throw new Error("CONFLICT: a selected object is locked.");
  };
  // Pipes follow the canvas: its ports, its addPipe rule and its copy, paste and delete.
  const ports = catalogueTypePorts(catalogue);
  for (const operation of operations) {
    if (!operation || typeof operation.op !== "string")
      throw new Error("INVALID_ARGUMENT: operation kind is required.");
    switch (operation.op) {
      case "page": {
        const properties = operation.properties;
        if (
          !properties ||
          typeof properties !== "object" ||
          Array.isArray(properties) ||
          Object.keys(properties).some(
            (key) => !["title", "order", "icon", "group", "hidden", "incomplete"].includes(key),
          )
        )
          throw new Error("INVALID_ARGUMENT: unsupported page property.");
        Object.assign(page, clone(properties));
        break;
      }
      case "move": {
        const objects = selected(operation.ids);
        movable(objects);
        const delta = pair(operation.delta);
        for (const object of objects) object.at = [object.at[0] + delta[0], object.at[1] + delta[1]];
        break;
      }
      case "resize": {
        const object = find(operation.id);
        movable([object]);
        object.size = pair(operation.size, 8);
        break;
      }
      case "align": {
        const objects = selected(operation.ids);
        movable(objects);
        if (!["left", "centre", "right", "top", "middle", "bottom"].includes(String(operation.edge)))
          throw new Error("INVALID_ARGUMENT: unknown alignment.");
        page = alignObjects(
          page,
          objects.map((object) => object.id),
          operation.edge as AlignEdge,
        );
        break;
      }
      case "distribute": {
        const objects = selected(operation.ids);
        movable(objects);
        if (!["horizontal", "vertical"].includes(String(operation.axis)))
          throw new Error("INVALID_ARGUMENT: unknown distribution axis.");
        page = distributeObjects(
          page,
          objects.map((object) => object.id),
          operation.axis as DistributeAxis,
        );
        break;
      }
      case "group":
      case "ungroup": {
        const objects = selected(operation.ids);
        movable(objects);
        for (const object of objects) {
          if (operation.op === "ungroup") delete object.group;
          else object.group = name(operation.group);
        }
        break;
      }
      case "remove": {
        const objects = selected(operation.ids);
        movable(objects);
        const ids = new Set(objects.map((object) => object.id));
        const dependent =
          page.pipe.some((pipe) => ids.has(pipe.from.split(".")[0]) || ids.has(pipe.to.split(".")[0])) ||
          page.object.some((object) => object.attach && ids.has(object.attach));
        if (dependent && operation.removeDependents !== true)
          throw new Error("CONFLICT: dependent pipes or attachments must be included in removal.");
        page = deleteObjects(page, [...ids]);
        break;
      }
      case "duplicate": {
        const objects = selected(operation.ids);
        const delta = pair(operation.delta);
        if (!Array.isArray(operation.newIds) || operation.newIds.length !== objects.length)
          throw new Error("INVALID_ARGUMENT: provide one new id per object.");
        const ids = operation.newIds.map(name);
        if (new Set(ids).size !== ids.length || ids.some((id) => page.object.some((object) => object.id === id)))
          throw new Error("CONFLICT: new ids must be unique.");
        page = duplicateObjects(
          page,
          objects.map((object) => object.id),
          ids,
          delta,
        );
        break;
      }
      case "bind":
      case "unbind": {
        const object = find(operation.id);
        movable([object]);
        if (typeof operation.property !== "string" || !ID.test(operation.property))
          throw new Error("INVALID_ARGUMENT: invalid binding property.");
        if (operation.op === "unbind") {
          delete object.bind[operation.property];
          break;
        }
        const binding = operation.binding as HmiBindingModel;
        const component = catalogue.components.find((component) => component.type === object.type);
        const property = component?.properties?.[operation.property] ?? component?.additional;
        if (
          !binding ||
          typeof binding.path !== "string" ||
          !binding.path.trim() ||
          !property?.directions.includes(binding.direction)
        )
          throw new Error("VALIDATION_FAILED: unsupported binding property or direction.");
        Object.defineProperty(object.bind, operation.property, {
          value: clone(binding),
          enumerable: true,
          configurable: true,
          writable: true,
        });
        break;
      }
      case "equal-size": {
        const objects = selected(operation.ids);
        movable(objects);
        page = equalSize(page, objects, find(operation.reference), operation.axis);
        break;
      }
      case "arrange": {
        page = arrangeRegion(page, selected(operation.ids), operation, pageSize);
        break;
      }
      case "reroute": {
        const index = connectionIndex(page, operation.from, operation.to);
        if (!Array.isArray(operation.waypoints) || operation.waypoints.length > 64)
          throw new Error("LIMIT_EXCEEDED: provide up to 64 pipe waypoints.");
        const waypoints = operation.waypoints.map((value) => pair(value));
        // No waypoints restore the computed route, as the inspector's Straighten does.
        page = {
          ...page,
          pipe: page.pipe.map((pipe, at) => {
            if (at !== index) return pipe;
            const { waypoints: _previous, ...rest } = pipe;
            return waypoints.length ? { ...rest, waypoints } : rest;
          }),
        };
        break;
      }
      case "reconnect": {
        page = reconnectPipe(
          page,
          connectionIndex(page, operation.from, operation.to),
          operation.newFrom,
          operation.newTo,
          ports,
        );
        break;
      }
      case "connect": {
        page = connectPorts(page, operation.from, operation.to, ports);
        break;
      }
      case "disconnect": {
        page = disconnectPorts(page, operation.from, operation.to);
        break;
      }
      case "reorder": {
        const objects = selected(operation.ids);
        movable(objects);
        if (operation.position !== "front" && operation.position !== "back")
          throw new Error("INVALID_ARGUMENT: choose front or back.");
        const remaining = page.object.filter((object) => !objects.includes(object));
        page.object = operation.position === "front" ? [...remaining, ...objects] : [...objects, ...remaining];
        break;
      }
      case "update": {
        const object = find(operation.id);
        const properties = operation.properties;
        if (
          !properties ||
          typeof properties !== "object" ||
          Array.isArray(properties) ||
          Object.keys(properties).some((key) => !EDITABLE.has(key))
        )
          throw new Error("INVALID_ARGUMENT: unsupported object property.");
        if (
          object.locked &&
          !(Object.keys(properties).length === 1 && (properties as Record<string, unknown>).locked === false)
        )
          movable([object]);
        Object.assign(object, clone(properties));
        break;
      }
      case "add": {
        const object = clone(operation.object) as HmiObjectModel;
        name(object?.id);
        if (page.object.some((existing) => existing.id === object.id)) throw new Error("CONFLICT: object id exists.");
        if (!catalogue.components.some((component) => component.type === object.type) && object.type !== "faceplate")
          throw new Error("INVALID_ARGUMENT: unknown component type.");
        object.at = pair(object.at);
        object.size = pair(object.size, 8);
        object.rotation = object.rotation ?? 0;
        object.bind = object.bind ?? {};
        page.object.push(object);
        break;
      }
      default:
        throw new Error(`INVALID_ARGUMENT: unsupported operation ${operation.op}.`);
    }
  }
  return page;
}
