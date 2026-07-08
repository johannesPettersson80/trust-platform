import type {
  Coil,
  CompareNode,
  Contact,
  Counter,
  LadderNode,
  LadderProgram,
  MathNode,
  Network,
  Timer,
} from "../ladder/ladderEngine.types";
import {
  isCoilType,
  isCompareOp,
  isContactType,
  isCounterType,
  isElementType,
  isMathOp,
  isTimerType,
} from "../ladder/ladderEngine.types";
import {
  fbNameForSource,
  isDirectAddress,
  isAssignableIdentifier,
  localName,
  sanitizeIdentifier,
} from "./stNaming";

interface LocalDeclaration {
  name: string;
  type: string;
  initialValue?: string;
  comment?: string;
}

interface ExternalDeclaration {
  name: string;
  type: string;
}

export interface LadderRuntimeGlobalDeclaration {
  name: string;
  type: string;
  address?: string;
  initialValue?: unknown;
}

interface DirectAddressAlias {
  name: string;
  type: string;
}

type TimerDecl = {
  varName: string;
  fbType: Timer["timerType"];
  source: string;
};

type CounterDecl = {
  varName: string;
  fbType: Counter["counterType"];
  source: string;
};

const INDENT = "  ";
const SIMPLE_IDENTIFIER = /^[A-Za-z_][A-Za-z0-9_]*$/;

function asRecord(value: unknown): Record<string, unknown> | null {
  if (!value || typeof value !== "object" || Array.isArray(value)) {
    return null;
  }
  return value as Record<string, unknown>;
}

function isFiniteNumber(value: unknown): value is number {
  return typeof value === "number" && Number.isFinite(value);
}

function isString(value: unknown): value is string {
  return typeof value === "string";
}

function isSimpleIdentifier(value: string): boolean {
  return SIMPLE_IDENTIFIER.test(value.trim());
}

function normalizedDirectAddress(value: string | undefined): string | undefined {
  const trimmed = value?.trim();
  if (!trimmed || !isDirectAddress(trimmed)) {
    return undefined;
  }
  return trimmed.toUpperCase();
}

function directAddressSymbolName(address: string): string {
  const suffix = address.toUpperCase().replace(/^%/, "");
  return sanitizeIdentifier(`ld_io_${suffix}`, "ld_io_address");
}

function typeForDirectAddress(address: string, fallback = "BOOL"): string {
  const size = normalizedDirectAddress(address)?.match(/^%[IQM]([XBWDL])/)?.[1];
  if (size === "X") {
    return "BOOL";
  }
  if (size === "B") {
    return "BYTE";
  }
  if (size === "W") {
    return "INT";
  }
  if (size === "D") {
    return "DINT";
  }
  if (size === "L") {
    return "LINT";
  }
  return fallback;
}

function addDirectAddressUse(
  uses: Map<string, string>,
  value: string | undefined,
  fallbackType: string
): void {
  const address = normalizedDirectAddress(value);
  if (!address || uses.has(address)) {
    return;
  }
  uses.set(address, typeForDirectAddress(address, fallbackType));
}

function collectDirectAddressUses(program: LadderProgram): Map<string, string> {
  const uses = new Map<string, string>();
  for (const network of program.networks) {
    for (const node of network.nodes) {
      if (node.type === "contact") {
        addDirectAddressUse(uses, node.variable, "BOOL");
      } else if (node.type === "coil") {
        addDirectAddressUse(uses, node.variable, "BOOL");
      } else if (node.type === "timer") {
        addDirectAddressUse(uses, node.input, "BOOL");
        addDirectAddressUse(uses, node.qOutput, "BOOL");
        addDirectAddressUse(uses, node.etOutput, "TIME");
      } else if (node.type === "counter") {
        addDirectAddressUse(uses, node.input, "BOOL");
        addDirectAddressUse(uses, node.qOutput, "BOOL");
        addDirectAddressUse(uses, node.cvOutput, "INT");
      } else if (node.type === "compare") {
        addDirectAddressUse(uses, node.left, "DINT");
        addDirectAddressUse(uses, node.right, "DINT");
      } else if (node.type === "math") {
        addDirectAddressUse(uses, node.left, "REAL");
        addDirectAddressUse(uses, node.right, "REAL");
        addDirectAddressUse(uses, node.output, "REAL");
      }
    }
  }
  return uses;
}

function uniqueGeneratedName(baseName: string, emitted: Set<string>): string {
  if (!emitted.has(baseName)) {
    return baseName;
  }
  let suffix = 2;
  while (emitted.has(`${baseName}_${suffix}`)) {
    suffix += 1;
  }
  return `${baseName}_${suffix}`;
}

export function collectLadderRuntimeGlobalDeclarations(
  program: LadderProgram
): LadderRuntimeGlobalDeclaration[] {
  const declarations: LadderRuntimeGlobalDeclaration[] = [];
  const emitted = new Set<string>();
  const explicitByAddress = new Map<string, DirectAddressAlias>();

  for (const variable of program.variables) {
    if (variable.scope === "local") {
      continue;
    }

    const name = variable.name.trim();
    if (!isSimpleIdentifier(name) || emitted.has(name)) {
      continue;
    }

    const type = variable.type || "BOOL";
    const address = normalizedDirectAddress(variable.address);
    declarations.push({
      name,
      type,
      address,
      initialValue: variable.initialValue,
    });
    emitted.add(name);

    if (address) {
      explicitByAddress.set(address, { name, type });
    }
  }

  for (const [address, fallbackType] of collectDirectAddressUses(program)) {
    if (explicitByAddress.has(address)) {
      continue;
    }
    const name = uniqueGeneratedName(directAddressSymbolName(address), emitted);
    const type = typeForDirectAddress(address, fallbackType);
    declarations.push({ name, type, address });
    emitted.add(name);
    explicitByAddress.set(address, { name, type });
  }

  return declarations;
}

function defaultScope(scope: LadderProgram["variables"][number]["scope"]): "local" | "global" {
  return scope === "local" ? "local" : "global";
}

function stLiteralForLocalInitial(
  stType: string,
  initialValue: unknown
): string | undefined {
  const normalizedType = stType.trim().toUpperCase();
  if (normalizedType === "BOOL") {
    if (typeof initialValue === "boolean") {
      return initialValue ? "TRUE" : "FALSE";
    }
    if (typeof initialValue === "number") {
      return initialValue === 0 ? "FALSE" : "TRUE";
    }
    if (typeof initialValue === "string") {
      const normalized = initialValue.trim().toUpperCase();
      if (normalized === "TRUE" || normalized === "1") {
        return "TRUE";
      }
      if (normalized === "FALSE" || normalized === "0") {
        return "FALSE";
      }
    }
    return undefined;
  }

  if (normalizedType === "INT" || normalizedType === "DINT") {
    if (typeof initialValue === "number" && Number.isFinite(initialValue)) {
      return `${Math.trunc(initialValue)}`;
    }
    if (typeof initialValue === "string" && initialValue.trim().length > 0) {
      const parsed = Number(initialValue);
      if (Number.isFinite(parsed)) {
        return `${Math.trunc(parsed)}`;
      }
    }
    return undefined;
  }

  if (normalizedType === "REAL" || normalizedType === "LREAL") {
    if (typeof initialValue === "number" && Number.isFinite(initialValue)) {
      return `${initialValue}`;
    }
    if (typeof initialValue === "string" && initialValue.trim().length > 0) {
      const parsed = Number(initialValue);
      if (Number.isFinite(parsed)) {
        return `${parsed}`;
      }
    }
    return undefined;
  }

  if (normalizedType === "TIME") {
    if (typeof initialValue === "string" && initialValue.trim().length > 0) {
      return initialValue.trim();
    }
    if (typeof initialValue === "number" && Number.isFinite(initialValue)) {
      return `T#${Math.max(0, Math.trunc(initialValue))}ms`;
    }
    return undefined;
  }

  return undefined;
}

function validateDeclaredSymbol(
  symbol: string,
  context: string,
  declaredSymbols: Set<string>
): void {
  const trimmed = symbol.trim();
  if (!trimmed || isDirectAddress(trimmed)) {
    return;
  }

  if (isSimpleIdentifier(trimmed) && !declaredSymbols.has(trimmed)) {
    throw new Error(
      `Undeclared ladder symbol '${trimmed}' referenced by ${context}. ` +
        "Declare it in variables[] or use a direct address."
    );
  }
}

export function validateLadderProgramValue(value: unknown): LadderProgram {
  const errors: string[] = [];
  const program = asRecord(value);
  if (!program) {
    throw new Error("Unsupported ladder schema: root object is missing.");
  }

  if (program.schemaVersion !== 2) {
    errors.push("schemaVersion must be exactly 2.");
  }

  const metadata = asRecord(program.metadata);
  if (!metadata) {
    errors.push("metadata object is required.");
  } else {
    if (!isString(metadata.name) || metadata.name.trim().length === 0) {
      errors.push("metadata.name must be a non-empty string.");
    }
    if (!isString(metadata.description)) {
      errors.push("metadata.description must be a string.");
    }
    for (const key of ["created", "modified"] as const) {
      if (key in metadata && metadata[key] !== undefined && !isString(metadata[key])) {
        errors.push(`metadata.${key} must be a string when provided.`);
      }
    }
  }

  const variables = program.variables;
  if (!Array.isArray(variables)) {
    errors.push("variables must be an array.");
  } else {
    for (let index = 0; index < variables.length; index += 1) {
      const variable = asRecord(variables[index]);
      const prefix = `variables[${index}]`;
      if (!variable) {
        errors.push(`${prefix} must be an object.`);
        continue;
      }
      if (!isString(variable.name) || variable.name.trim().length === 0) {
        errors.push(`${prefix}.name must be a non-empty string.`);
      }
      if (
        variable.type !== "BOOL" &&
        variable.type !== "INT" &&
        variable.type !== "REAL" &&
        variable.type !== "TIME" &&
        variable.type !== "DINT" &&
        variable.type !== "LREAL"
      ) {
        errors.push(
          `${prefix}.type must be one of BOOL|INT|REAL|TIME|DINT|LREAL.`
        );
      }
      if (
        variable.scope !== undefined &&
        variable.scope !== "local" &&
        variable.scope !== "global"
      ) {
        errors.push(`${prefix}.scope must be 'local' or 'global' when provided.`);
      }
      if (variable.address !== undefined && !isString(variable.address)) {
        errors.push(`${prefix}.address must be a string when provided.`);
      }
    }
  }

  const networks = program.networks;
  if (!Array.isArray(networks)) {
    errors.push("networks must be an array.");
  } else {
    for (let networkIndex = 0; networkIndex < networks.length; networkIndex += 1) {
      const network = asRecord(networks[networkIndex]);
      const networkPrefix = `networks[${networkIndex}]`;
      if (!network) {
        errors.push(`${networkPrefix} must be an object.`);
        continue;
      }

      if (!isString(network.id) || network.id.trim().length === 0) {
        errors.push(`${networkPrefix}.id must be a non-empty string.`);
      }
      if (!isFiniteNumber(network.order)) {
        errors.push(`${networkPrefix}.order must be a finite number.`);
      }

      const layout = asRecord(network.layout);
      if (!layout || !isFiniteNumber(layout.y)) {
        errors.push(`${networkPrefix}.layout.y must be a finite number.`);
      }

      if (!Array.isArray(network.nodes)) {
        errors.push(`${networkPrefix}.nodes must be an array.`);
      } else {
        for (let nodeIndex = 0; nodeIndex < network.nodes.length; nodeIndex += 1) {
          const node = asRecord(network.nodes[nodeIndex]);
          const nodePrefix = `${networkPrefix}.nodes[${nodeIndex}]`;
          if (!node) {
            errors.push(`${nodePrefix} must be an object.`);
            continue;
          }
          if (!isString(node.id) || node.id.trim().length === 0) {
            errors.push(`${nodePrefix}.id must be a non-empty string.`);
          }
          if (!isElementType(node.type)) {
            errors.push(`${nodePrefix}.type is unsupported.`);
            continue;
          }
          const position = asRecord(node.position);
          if (!position) {
            errors.push(`${nodePrefix}.position must be an object.`);
            continue;
          }
          if (!isFiniteNumber(position.x) || !isFiniteNumber(position.y)) {
            errors.push(`${nodePrefix}.position.x/y must be finite numbers.`);
          }

          if (node.type === "contact") {
            if (!isContactType(node.contactType)) {
              errors.push(`${nodePrefix}.contactType must be NO or NC.`);
            }
            if (!isString(node.variable)) {
              errors.push(`${nodePrefix}.variable must be a string.`);
            }
            continue;
          }

          if (node.type === "coil") {
            if (!isCoilType(node.coilType)) {
              errors.push(
                `${nodePrefix}.coilType must be NORMAL, SET, RESET, or NEGATED.`
              );
            }
            if (!isString(node.variable)) {
              errors.push(`${nodePrefix}.variable must be a string.`);
            }
            continue;
          }

          if (node.type === "timer") {
            if (!isTimerType(node.timerType)) {
              errors.push(`${nodePrefix}.timerType must be TON, TOF, or TP.`);
            }
            if (!isString(node.instance) || node.instance.trim().length === 0) {
              errors.push(`${nodePrefix}.instance must be a non-empty string.`);
            }
            if (node.input !== undefined && !isString(node.input)) {
              errors.push(`${nodePrefix}.input must be a string when provided.`);
            }
            if (!isFiniteNumber(node.presetMs)) {
              errors.push(`${nodePrefix}.presetMs must be a finite number.`);
            }
            if (!isString(node.qOutput) || node.qOutput.trim().length === 0) {
              errors.push(`${nodePrefix}.qOutput must be a non-empty string.`);
            }
            if (!isString(node.etOutput) || node.etOutput.trim().length === 0) {
              errors.push(`${nodePrefix}.etOutput must be a non-empty string.`);
            }
            continue;
          }

          if (node.type === "counter") {
            if (!isCounterType(node.counterType)) {
              errors.push(`${nodePrefix}.counterType must be CTU, CTD, or CTUD.`);
            }
            if (!isString(node.instance) || node.instance.trim().length === 0) {
              errors.push(`${nodePrefix}.instance must be a non-empty string.`);
            }
            if (node.input !== undefined && !isString(node.input)) {
              errors.push(`${nodePrefix}.input must be a string when provided.`);
            }
            if (!isFiniteNumber(node.preset)) {
              errors.push(`${nodePrefix}.preset must be a finite number.`);
            }
            if (!isString(node.qOutput) || node.qOutput.trim().length === 0) {
              errors.push(`${nodePrefix}.qOutput must be a non-empty string.`);
            }
            if (!isString(node.cvOutput) || node.cvOutput.trim().length === 0) {
              errors.push(`${nodePrefix}.cvOutput must be a non-empty string.`);
            }
            continue;
          }

          if (node.type === "compare") {
            if (!isCompareOp(node.op)) {
              errors.push(`${nodePrefix}.op must be GT, LT, or EQ.`);
            }
            if (!isString(node.left) || !isString(node.right)) {
              errors.push(`${nodePrefix}.left and .right must be strings.`);
            }
            continue;
          }

          if (node.type === "math") {
            if (!isMathOp(node.op)) {
              errors.push(`${nodePrefix}.op must be ADD, SUB, MUL, or DIV.`);
            }
            if (
              !isString(node.left) ||
              !isString(node.right) ||
              !isString(node.output)
            ) {
              errors.push(
                `${nodePrefix}.left, .right, and .output must be strings.`
              );
            }
          }
        }
      }

      if (!Array.isArray(network.edges)) {
        errors.push(`${networkPrefix}.edges must be an array.`);
      } else {
        for (let edgeIndex = 0; edgeIndex < network.edges.length; edgeIndex += 1) {
          const edge = asRecord(network.edges[edgeIndex]);
          const edgePrefix = `${networkPrefix}.edges[${edgeIndex}]`;
          if (!edge) {
            errors.push(`${edgePrefix} must be an object.`);
            continue;
          }
          if (!isString(edge.id) || edge.id.trim().length === 0) {
            errors.push(`${edgePrefix}.id must be a non-empty string.`);
          }
          if (
            !isString(edge.fromNodeId) ||
            edge.fromNodeId.trim().length === 0 ||
            !isString(edge.toNodeId) ||
            edge.toNodeId.trim().length === 0
          ) {
            errors.push(
              `${edgePrefix}.fromNodeId and .toNodeId must be non-empty strings.`
            );
          }
          if (edge.points !== undefined) {
            if (!Array.isArray(edge.points)) {
              errors.push(`${edgePrefix}.points must be an array when provided.`);
            } else {
              for (let pointIndex = 0; pointIndex < edge.points.length; pointIndex += 1) {
                const point = asRecord(edge.points[pointIndex]);
                if (
                  !point ||
                  !isFiniteNumber(point.x) ||
                  !isFiniteNumber(point.y)
                ) {
                  errors.push(
                    `${edgePrefix}.points[${pointIndex}] must contain numeric x/y.`
                  );
                }
              }
            }
          }
        }
      }
    }
  }

  if (errors.length > 0) {
    const details = errors.map((entry) => `- ${entry}`).join("\n");
    throw new Error(
      `Unsupported ladder schema. Expected strict schemaVersion: 2 contract.\n${details}`
    );
  }

  return value as LadderProgram;
}

function sortedNodes(network: Network): LadderNode[] {
  return [...network.nodes].sort(compareNodesDeterministic);
}

function compareNodesDeterministic(left: LadderNode, right: LadderNode): number {
  if (left.position.x !== right.position.x) {
    return left.position.x - right.position.x;
  }
  if (left.position.y !== right.position.y) {
    return left.position.y - right.position.y;
  }
  return left.id.localeCompare(right.id);
}

function contactExpression(
  contact: Contact,
  symbolForValue: (value: string, fallbackType?: string) => string
): string {
  const source = symbolForValue(contact.variable, "BOOL") || "FALSE";
  if (contact.contactType === "NC") {
    return `NOT (${source})`;
  }
  return source;
}

const LEFT_RAIL_ID = "__LEFT_RAIL__";

type NetworkTopology = {
  incoming: Map<string, string[]>;
  outgoing: Map<string, string[]>;
  sortedNodes: LadderNode[];
};

function buildNetworkTopology(network: Network): NetworkTopology {
  const nodes = sortedNodes(network);
  const incoming = new Map<string, string[]>();
  const outgoing = new Map<string, string[]>();

  for (const node of nodes) {
    incoming.set(node.id, []);
    outgoing.set(node.id, []);
  }
  outgoing.set(LEFT_RAIL_ID, []);

  const addEdge = (from: string, to: string): void => {
    if (!incoming.has(to)) {
      return;
    }
    if (!outgoing.has(from)) {
      outgoing.set(from, []);
    }
    const toTargets = outgoing.get(from) ?? [];
    if (!toTargets.includes(to)) {
      toTargets.push(to);
      outgoing.set(from, toTargets);
    }

    const sources = incoming.get(to) ?? [];
    if (!sources.includes(from)) {
      sources.push(from);
      incoming.set(to, sources);
    }
  };

  if (network.edges.length > 0) {
    for (const edge of network.edges) {
      addEdge(edge.fromNodeId, edge.toNodeId);
    }

    const noIncoming = nodes.filter((node) => (incoming.get(node.id) ?? []).length === 0);
    for (const node of noIncoming) {
      addEdge(LEFT_RAIL_ID, node.id);
    }
  } else if (nodes.length > 0) {
    addEdge(LEFT_RAIL_ID, nodes[0].id);
    for (let index = 0; index < nodes.length - 1; index += 1) {
      addEdge(nodes[index].id, nodes[index + 1].id);
    }
  }

  return { incoming, outgoing, sortedNodes: nodes };
}

function topologicalOrder(topology: NetworkTopology): LadderNode[] {
  const nodeById = new Map<string, LadderNode>(
    topology.sortedNodes.map((node) => [node.id, node])
  );
  const indegree = new Map<string, number>();
  for (const node of topology.sortedNodes) {
    indegree.set(node.id, 0);
  }
  for (const [fromId, toIds] of topology.outgoing.entries()) {
    if (fromId === LEFT_RAIL_ID) {
      continue;
    }
    for (const toId of toIds) {
      indegree.set(toId, (indegree.get(toId) ?? 0) + 1);
    }
  }

  const queue = topology.sortedNodes
    .filter((node) => (indegree.get(node.id) ?? 0) === 0)
    .sort(compareNodesDeterministic);
  const order: LadderNode[] = [];

  while (queue.length > 0) {
    const next = queue.shift();
    if (!next) {
      continue;
    }
    order.push(next);
    const outgoing = topology.outgoing.get(next.id) ?? [];
    for (const toId of outgoing) {
      const remaining = (indegree.get(toId) ?? 0) - 1;
      indegree.set(toId, remaining);
      if (remaining === 0) {
        const candidate = nodeById.get(toId);
        if (candidate) {
          queue.push(candidate);
          queue.sort(compareNodesDeterministic);
        }
      }
    }
  }

  if (order.length < topology.sortedNodes.length) {
    for (const node of topology.sortedNodes) {
      if (!order.find((entry) => entry.id === node.id)) {
        order.push(node);
      }
    }
  }

  return order;
}

function normalizeOrTerms(terms: string[]): string[] {
  const filtered = terms.map((term) => term.trim()).filter((term) => term.length > 0);
  if (filtered.includes("TRUE")) {
    return ["TRUE"];
  }
  const withoutFalse = filtered.filter((term) => term !== "FALSE");
  return Array.from(new Set(withoutFalse));
}

function joinWithOr(terms: string[]): string {
  const normalized = normalizeOrTerms(terms);
  if (normalized.length === 0) {
    return "FALSE";
  }
  if (normalized.length === 1) {
    return normalized[0];
  }
  return normalized.join(" OR ");
}

function needsParenForAnd(term: string): boolean {
  return /\bOR\b/.test(term);
}

function joinWithAnd(left: string, right: string): string {
  if (left === "FALSE" || right === "FALSE") {
    return "FALSE";
  }
  if (left === "TRUE") {
    return right;
  }
  if (right === "TRUE") {
    return left;
  }
  const leftExpr = needsParenForAnd(left) ? `(${left})` : left;
  const rightExpr = needsParenForAnd(right) ? `(${right})` : right;
  return `${leftExpr} AND ${rightExpr}`;
}

function nodePowerExpressions(
  network: Network,
  symbolForValue: (value: string, fallbackType?: string) => string
): Map<string, string> {
  const topology = buildNetworkTopology(network);
  const order = topologicalOrder(topology);
  const outgoing = new Map<string, string>();
  const incoming = new Map<string, string>();

  for (const node of order) {
    const sources = topology.incoming.get(node.id) ?? [];
    const sourceExpressions =
      sources.length === 0
        ? ["FALSE"]
        : sources.map((source) =>
            source === LEFT_RAIL_ID ? "TRUE" : outgoing.get(source) ?? "FALSE"
          );
    const incomingExpr = joinWithOr(sourceExpressions);
    incoming.set(node.id, incomingExpr);

    if (node.type === "contact") {
      outgoing.set(node.id, joinWithAnd(incomingExpr, contactExpression(node, symbolForValue)));
      continue;
    }
    // Coils/FBs/topology nodes pass incoming power through.
    outgoing.set(node.id, incomingExpr);
  }

  return incoming;
}

function timerCallInput(timer: Timer): string {
  const roundedMs = Math.max(0, Math.round(timer.presetMs));
  return `PT := T#${roundedMs}ms`;
}

function compareOperator(op: CompareNode["op"]): string {
  if (op === "GT") {
    return ">";
  }
  if (op === "LT") {
    return "<";
  }
  if (op === "EQ") {
    return "=";
  }
  throw new Error(`Unsupported compare operator '${String(op)}'.`);
}

function mathOperator(op: MathNode["op"]): string {
  if (op === "ADD") {
    return "+";
  }
  if (op === "SUB") {
    return "-";
  }
  if (op === "MUL") {
    return "*";
  }
  if (op === "DIV") {
    return "/";
  }
  throw new Error(`Unsupported math operator '${String(op)}'.`);
}

function isNumberLiteral(value: string): boolean {
  return /^[-+]?\d+(\.\d+)?$/.test(value.trim());
}

function asOperandExpression(value: string): string {
  const trimmed = value.trim();
  if (!trimmed) {
    return "0";
  }
  if (isNumberLiteral(trimmed)) {
    return trimmed;
  }
  return trimmed;
}

export function parseLadderProgramText(content: string): LadderProgram {
  const parsed = JSON.parse(content);
  return validateLadderProgramValue(parsed);
}

export function generateLadderCompanionFunctionBlock(
  program: LadderProgram,
  baseName: string
): string {
  const orderedNetworks = [...program.networks].sort(
    (left, right) => left.order - right.order
  );

  const timerDeclByKey = new Map<string, TimerDecl>();
  const counterDeclByKey = new Map<string, CounterDecl>();
  const localDeclByName = new Map<string, LocalDeclaration>();
  const externalDeclByName = new Map<string, ExternalDeclaration>();
  const localScopeSymbols = new Set<string>();
  const declaredSymbols = new Set<string>();
  const runtimeGlobals = collectLadderRuntimeGlobalDeclarations(program);
  const directAddressAliases = new Map<string, DirectAddressAlias>();

  for (const variable of program.variables) {
    const name = variable.name.trim();
    if (!isSimpleIdentifier(name)) {
      continue;
    }
    if (defaultScope(variable.scope) === "local") {
      localScopeSymbols.add(name);
      declaredSymbols.add(name);
    }
  }

  for (const variable of program.variables) {
    const name = variable.name.trim();
    if (!isSimpleIdentifier(name)) {
      continue;
    }
    const typeName = variable.type || "BOOL";
    if (defaultScope(variable.scope) === "local") {
      if (localDeclByName.has(name)) {
        continue;
      }
      localDeclByName.set(name, {
        name,
        type: typeName,
        initialValue: stLiteralForLocalInitial(typeName, variable.initialValue),
        comment: "Ladder local",
      });
      continue;
    }
  }

  for (const declaration of runtimeGlobals) {
    if (localScopeSymbols.has(declaration.name) || externalDeclByName.has(declaration.name)) {
      continue;
    }
    externalDeclByName.set(declaration.name, {
      name: declaration.name,
      type: declaration.type,
    });
    declaredSymbols.add(declaration.name);
    if (declaration.address) {
      directAddressAliases.set(declaration.address, {
        name: declaration.name,
        type: declaration.type,
      });
    }
  }

  const symbolForValue = (value: string, _fallbackType = "BOOL"): string => {
    const address = normalizedDirectAddress(value);
    if (!address) {
      return value.trim();
    }
    return directAddressAliases.get(address)?.name ?? value.trim();
  };

  const ensureTimerVar = (timer: Timer): string => {
    const key = timer.instance.trim() || timer.id;
    const existing = timerDeclByKey.get(key);
    if (existing) {
      return existing.varName;
    }
    const varName = localName("ld_timer", key);
    timerDeclByKey.set(key, {
      varName,
      fbType: timer.timerType,
      source: key,
    });
    return varName;
  };

  const ensureCounterVar = (counter: Counter): string => {
    const key = counter.instance.trim() || counter.id;
    const existing = counterDeclByKey.get(key);
    if (existing) {
      return existing.varName;
    }
    const varName = localName("ld_counter", key);
    counterDeclByKey.set(key, {
      varName,
      fbType: counter.counterType,
      source: key,
    });
    return varName;
  };

  const ensureLocal = (name: string, type: string, comment: string): string => {
    if (!localDeclByName.has(name)) {
      localDeclByName.set(name, { name, type, comment });
    }
    return name;
  };

  const lines: string[] = [];

  for (const network of orderedNetworks) {
    const powerByNode = nodePowerExpressions(network, symbolForValue);
    const nodes = sortedNodes(network);

    const contacts: Contact[] = [];
    const coils: Coil[] = [];
    const timers: Timer[] = [];
    const counters: Counter[] = [];
    const compares: CompareNode[] = [];
    const maths: MathNode[] = [];

    for (const node of nodes) {
      if (node.type === "coil") {
        coils.push(node);
      } else if (node.type === "contact") {
        contacts.push(node);
      } else if (node.type === "timer") {
        timers.push(node);
      } else if (node.type === "counter") {
        counters.push(node);
      } else if (node.type === "compare") {
        compares.push(node);
      } else if (node.type === "math") {
        maths.push(node);
      }
    }

    lines.push(`(* Network ${network.order}: ${network.id} *)`);

    for (const contact of contacts) {
      validateDeclaredSymbol(
        symbolForValue(contact.variable, "BOOL"),
        `contact '${contact.id}'`,
        declaredSymbols
      );
    }

    for (const coil of coils) {
      const coilTarget = symbolForValue(coil.variable, "BOOL");
      validateDeclaredSymbol(coilTarget, `coil '${coil.id}'`, declaredSymbols);
      const powerExpr = powerByNode.get(coil.id) ?? "FALSE";
      if (!isAssignableIdentifier(coilTarget)) {
        lines.push(
          `(* Skipped coil '${coil.id}' due to non-assignable target: ${coil.variable} *)`
        );
        continue;
      }
      if (coil.coilType === "NORMAL") {
        lines.push(`${coilTarget} := ${powerExpr};`);
      } else if (coil.coilType === "NEGATED") {
        lines.push(`${coilTarget} := NOT (${powerExpr});`);
      } else if (coil.coilType === "SET") {
        lines.push(`IF ${powerExpr} THEN`);
        lines.push(`${INDENT}${coilTarget} := TRUE;`);
        lines.push("END_IF;");
      } else if (coil.coilType === "RESET") {
        lines.push(`IF ${powerExpr} THEN`);
        lines.push(`${INDENT}${coilTarget} := FALSE;`);
        lines.push("END_IF;");
      } else {
        throw new Error(`Unsupported coilType '${String(coil.coilType)}'.`);
      }
    }

    for (const timer of timers) {
      const timerVar = ensureTimerVar(timer);
      const powerExpr = powerByNode.get(timer.id) ?? "FALSE";
      const inputExpr = timer.input?.trim()
        ? symbolForValue(timer.input, "BOOL")
        : powerExpr;
      validateDeclaredSymbol(inputExpr, `timer '${timer.id}' input`, declaredSymbols);
      lines.push(`${timerVar}(IN := ${inputExpr}, ${timerCallInput(timer)});`);

      const qOutput = timer.qOutput.trim();
      const qTarget =
        qOutput && isAssignableIdentifier(qOutput)
          ? symbolForValue(qOutput, "BOOL")
          : ensureLocal(
              localName("ld_timer_q", timer.instance || timer.id),
              "BOOL",
              `Timer Q output for ${timer.id}`
            );
      validateDeclaredSymbol(qTarget, `timer '${timer.id}' Q output`, declaredSymbols);
      const etOutput = timer.etOutput.trim();
      const etTarget =
        etOutput && isAssignableIdentifier(etOutput)
          ? symbolForValue(etOutput, "TIME")
          : ensureLocal(
              localName("ld_timer_et", timer.instance || timer.id),
              "TIME",
              `Timer ET output for ${timer.id}`
            );
      validateDeclaredSymbol(etTarget, `timer '${timer.id}' ET output`, declaredSymbols);
      lines.push(`${qTarget} := ${timerVar}.Q;`);
      lines.push(`${etTarget} := ${timerVar}.ET;`);
    }

    for (const counter of counters) {
      const counterVar = ensureCounterVar(counter);
      const pv = Math.max(0, Math.round(counter.preset));
      const powerExpr = powerByNode.get(counter.id) ?? "FALSE";
      const inputExpr = counter.input?.trim()
        ? symbolForValue(counter.input, "BOOL")
        : powerExpr;
      validateDeclaredSymbol(
        inputExpr,
        `counter '${counter.id}' input`,
        declaredSymbols
      );

      if (counter.counterType === "CTU") {
        lines.push(`${counterVar}(CU := ${inputExpr}, R := FALSE, PV := ${pv});`);
      } else if (counter.counterType === "CTD") {
        lines.push(`${counterVar}(CD := ${inputExpr}, LD := FALSE, PV := ${pv});`);
      } else if (counter.counterType === "CTUD") {
        lines.push(
          `${counterVar}(CU := ${inputExpr}, CD := FALSE, R := FALSE, LD := FALSE, PV := ${pv});`
        );
      } else {
        throw new Error(
          `Unsupported counterType '${String(counter.counterType)}'.`
        );
      }

      const qOutput = counter.qOutput.trim();
      const qTarget =
        qOutput && isAssignableIdentifier(qOutput)
          ? symbolForValue(qOutput, "BOOL")
          : ensureLocal(
              localName("ld_counter_q", counter.instance || counter.id),
              "BOOL",
              `Counter Q output for ${counter.id}`
            );
      validateDeclaredSymbol(
        qTarget,
        `counter '${counter.id}' Q output`,
        declaredSymbols
      );
      const cvOutput = counter.cvOutput.trim();
      const cvTarget =
        cvOutput && isAssignableIdentifier(cvOutput)
          ? symbolForValue(cvOutput, "INT")
          : ensureLocal(
              localName("ld_counter_cv", counter.instance || counter.id),
              "INT",
              `Counter CV output for ${counter.id}`
            );
      validateDeclaredSymbol(
        cvTarget,
        `counter '${counter.id}' CV output`,
        declaredSymbols
      );
      lines.push(`${qTarget} := ${counterVar}.Q;`);
      lines.push(`${cvTarget} := ${counterVar}.CV;`);
    }

    for (const compare of compares) {
      const powerExpr = powerByNode.get(compare.id) ?? "FALSE";
      const left = asOperandExpression(symbolForValue(compare.left, "DINT"));
      const right = asOperandExpression(symbolForValue(compare.right, "DINT"));
      validateDeclaredSymbol(left, `compare '${compare.id}' left operand`, declaredSymbols);
      validateDeclaredSymbol(
        right,
        `compare '${compare.id}' right operand`,
        declaredSymbols
      );
      const target = ensureLocal(
        localName("ld_compare_q", compare.id),
        "BOOL",
        `Compare result for ${compare.id}`
      );
      lines.push(`IF ${powerExpr} THEN`);
      lines.push(
        `${INDENT}${target} := (${left} ${compareOperator(compare.op)} ${right});`
      );
      lines.push("END_IF;");
    }

    for (const math of maths) {
      const powerExpr = powerByNode.get(math.id) ?? "FALSE";
      const left = asOperandExpression(symbolForValue(math.left, "REAL"));
      const right = asOperandExpression(symbolForValue(math.right, "REAL"));
      validateDeclaredSymbol(left, `math '${math.id}' left operand`, declaredSymbols);
      validateDeclaredSymbol(right, `math '${math.id}' right operand`, declaredSymbols);
      const operator = mathOperator(math.op);
      const mathOutput = symbolForValue(math.output, "REAL");
      const target = isAssignableIdentifier(mathOutput)
        ? mathOutput
        : ensureLocal(
            localName("ld_math_out", math.id),
            "REAL",
            `Math output for ${math.id}`
          );
      validateDeclaredSymbol(target, `math '${math.id}' output`, declaredSymbols);

      if (math.op === "DIV") {
        lines.push(`IF ${powerExpr} THEN`);
        lines.push(`${INDENT}IF (${right}) = 0 THEN`);
        lines.push(`${INDENT}${INDENT}${target} := 0;`);
        lines.push(`${INDENT}ELSE`);
        lines.push(`${INDENT}${INDENT}${target} := (${left}) ${operator} (${right});`);
        lines.push(`${INDENT}END_IF;`);
        lines.push("END_IF;");
      } else {
        lines.push(`IF ${powerExpr} THEN`);
        lines.push(`${INDENT}${target} := (${left}) ${operator} (${right});`);
        lines.push("END_IF;");
      }
    }

    lines.push("");
  }

  const fbName = fbNameForSource(baseName || program.metadata.name, "LADDER");
  const localDeclarations: string[] = [];
  const externalDeclarations: string[] = [];

  for (const externalDecl of externalDeclByName.values()) {
    externalDeclarations.push(
      `${INDENT}${externalDecl.name} : ${externalDecl.type};`
    );
  }

  for (const timerDecl of timerDeclByKey.values()) {
    localDeclarations.push(
      `${INDENT}${timerDecl.varName} : ${timerDecl.fbType}; (* ${timerDecl.source} *)`
    );
  }
  for (const counterDecl of counterDeclByKey.values()) {
    localDeclarations.push(
      `${INDENT}${counterDecl.varName} : ${counterDecl.fbType}; (* ${counterDecl.source} *)`
    );
  }
  for (const localDecl of localDeclByName.values()) {
    const initialClause = localDecl.initialValue
      ? ` := ${localDecl.initialValue}`
      : "";
    const commentClause = localDecl.comment
      ? ` (* ${localDecl.comment} *)`
      : "";
    localDeclarations.push(
      `${INDENT}${localDecl.name} : ${localDecl.type}${initialClause};${commentClause}`
    );
  }

  const output: string[] = [
    "(*",
    "  Auto-generated Ladder Diagram companion (schema v2).",
    "  Source of truth remains the .ladder.json file.",
    "  Generated to support .st-first runtime workflows.",
    "*)",
    "",
    `FUNCTION_BLOCK ${fbName}`,
  ];

  if (externalDeclarations.length > 0) {
    output.push("VAR_EXTERNAL");
    output.push(...externalDeclarations);
    output.push("END_VAR");
    output.push("");
  }

  if (localDeclarations.length > 0) {
    output.push("VAR");
    output.push(...localDeclarations);
    output.push("END_VAR");
    output.push("");
  }

  output.push(...lines);
  output.push("END_FUNCTION_BLOCK");

  return output.join("\n");
}
