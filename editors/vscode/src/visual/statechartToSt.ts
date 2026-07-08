import {
  escapeStString,
  eventInputName,
  fbNameForSource,
  isAssignableIdentifier,
  localName,
} from "./stNaming";
import { rewriteDirectIoInFunctionBlock } from "./directIo";

interface ActionTarget {
  address: string;
  value: unknown;
}

interface ActionMapping {
  action: string;
  address?: string;
  variable?: string;
  value?: unknown;
  message?: string;
  targets?: ActionTarget[];
}

interface TransitionConfig {
  target: string;
  guard?: string;
  actions?: string[];
  after?: number;
}

interface StateConfig {
  entry?: string[];
  exit?: string[];
  on?: Record<string, TransitionConfig | string>;
}

interface StateChartConfig {
  id: string;
  initial?: string;
  states: Record<string, StateConfig>;
  actionMappings?: Record<string, ActionMapping>;
}

interface Transition {
  eventName: string;
  target: string;
  guard?: string;
  actions: string[];
  afterMs?: number;
  timerVarName?: string;
}

interface TimerDecl {
  name: string;
  afterMs: number;
  stateName: string;
  eventName: string;
}

const INDENT = "  ";

function isStateChartConfig(value: unknown): value is StateChartConfig {
  if (!value || typeof value !== "object") {
    return false;
  }
  const config = value as Partial<StateChartConfig>;
  return (
    typeof config.id === "string" &&
    !!config.states &&
    typeof config.states === "object" &&
    !Array.isArray(config.states)
  );
}

function toActionList(value: unknown): string[] {
  if (!Array.isArray(value)) {
    return [];
  }
  return value.filter((item): item is string => typeof item === "string");
}

function normalizeGuardExpression(guard: string): string {
  return guard
    .replace(/==/g, "=")
    .replace(/!=/g, "<>")
    .replace(/\&\&/g, " AND ")
    .replace(/\|\|/g, " OR ")
    .replace(/\btrue\b/gi, "TRUE")
    .replace(/\bfalse\b/gi, "FALSE");
}

function uniqueName(base: string, used: Set<string>): string {
  if (!used.has(base)) {
    used.add(base);
    return base;
  }

  let counter = 1;
  while (used.has(`${base}_${counter}`)) {
    counter += 1;
  }
  const unique = `${base}_${counter}`;
  used.add(unique);
  return unique;
}

function toStLiteral(value: unknown): string {
  if (typeof value === "boolean") {
    return value ? "TRUE" : "FALSE";
  }
  if (typeof value === "number") {
    return Number.isFinite(value) ? `${value}` : "0";
  }
  if (typeof value === "string") {
    return `'${escapeStString(value)}'`;
  }
  if (value === null || value === undefined) {
    return "0";
  }
  return `'${escapeStString(JSON.stringify(value))}'`;
}

function appendActionStatements(
  actionNames: string[],
  actionMappings: Record<string, ActionMapping> | undefined,
  output: string[],
  indentLevel: number
): void {
  const indent = INDENT.repeat(indentLevel);
  for (const actionName of actionNames) {
    const mapping = actionMappings?.[actionName];
    if (!mapping) {
      output.push(`${indent}(* Unmapped action: ${actionName} *)`);
      continue;
    }

    const actionType = mapping.action.toUpperCase();
    if (actionType === "WRITE_OUTPUT") {
      const target = mapping.address?.trim() ?? "";
      if (isAssignableIdentifier(target)) {
        output.push(`${indent}${target} := ${toStLiteral(mapping.value)};`);
      } else {
        output.push(
          `${indent}(* Invalid WRITE_OUTPUT target for action ${actionName}: ${target} *)`
        );
      }
      continue;
    }

    if (actionType === "WRITE_VARIABLE") {
      const target = mapping.variable?.trim() ?? "";
      if (isAssignableIdentifier(target)) {
        output.push(`${indent}${target} := ${toStLiteral(mapping.value)};`);
      } else {
        output.push(
          `${indent}(* Invalid WRITE_VARIABLE target for action ${actionName}: ${target} *)`
        );
      }
      continue;
    }

    if (actionType === "SET_MULTIPLE") {
      const targets = Array.isArray(mapping.targets) ? mapping.targets : [];
      for (const target of targets) {
        const address = target.address?.trim() ?? "";
        if (isAssignableIdentifier(address)) {
          output.push(`${indent}${address} := ${toStLiteral(target.value)};`);
        } else {
          output.push(
            `${indent}(* Invalid SET_MULTIPLE target for action ${actionName}: ${address} *)`
          );
        }
      }
      continue;
    }

    if (actionType === "LOG") {
      const message = mapping.message ?? actionName;
      output.push(`${indent}(* LOG: ${message} *)`);
      continue;
    }

    output.push(
      `${indent}(* Unsupported action type '${mapping.action}' for ${actionName} *)`
    );
  }
}

export function parseStateChartText(content: string): StateChartConfig {
  const parsed = JSON.parse(content);
  if (!isStateChartConfig(parsed)) {
    throw new Error(
      "Invalid statechart format. Expected JSON object with id and states."
    );
  }
  return parsed;
}

export function generateStateChartCompanionFunctionBlock(
  config: StateChartConfig,
  baseName: string
): string {
  const stateNames = Object.keys(config.states);
  if (stateNames.length === 0) {
    throw new Error("Statechart has no states.");
  }

  const initialState =
    (config.initial && config.states[config.initial] && config.initial) ||
    stateNames[0];

  const stateIndexByName = new Map<string, number>();
  stateNames.forEach((stateName, index) => {
    stateIndexByName.set(stateName, index);
  });

  const usedEventNames = new Set<string>();
  const eventInputByEventName = new Map<string, string>();
  const timerDeclarations: TimerDecl[] = [];
  const transitionsByState = new Map<string, Transition[]>();

  for (const stateName of stateNames) {
    const stateConfig = config.states[stateName] ?? {};
    const transitions: Transition[] = [];
    const transitionEntries = Object.entries(stateConfig.on ?? {});

    transitionEntries.forEach(([eventName, rawTransition], index) => {
      const transitionObject =
        typeof rawTransition === "string"
          ? { target: rawTransition }
          : rawTransition;
      if (!transitionObject?.target) {
        return;
      }

      const transition: Transition = {
        eventName,
        target: transitionObject.target,
        guard:
          typeof transitionObject.guard === "string"
            ? transitionObject.guard
            : undefined,
        actions: toActionList(transitionObject.actions),
      };

      if (
        typeof transitionObject.after === "number" &&
        Number.isFinite(transitionObject.after) &&
        transitionObject.after > 0
      ) {
        const timerName = uniqueName(
          localName("sc_timer", `${stateName}_${eventName}_${index}`),
          new Set(timerDeclarations.map((timer) => timer.name))
        );
        transition.afterMs = Math.round(transitionObject.after);
        transition.timerVarName = timerName;
        timerDeclarations.push({
          name: timerName,
          afterMs: transition.afterMs,
          stateName,
          eventName,
        });
      } else {
        const eventVar = eventInputByEventName.get(eventName);
        if (!eventVar) {
          const uniqueEventName = uniqueName(
            eventInputName(eventName),
            usedEventNames
          );
          eventInputByEventName.set(eventName, uniqueEventName);
        }
      }

      transitions.push(transition);
    });

    transitionsByState.set(stateName, transitions);
  }

  const functionBlockName = fbNameForSource(baseName || config.id, "STATECHART");
  const body: string[] = [];

  body.push("IF NOT _initialized THEN");
  body.push(`${INDENT}_state := ${stateIndexByName.get(initialState) ?? 0};`);
  body.push(`${INDENT}_initialized := TRUE;`);
  body.push("END_IF;");
  body.push("");

  for (const timer of timerDeclarations) {
    body.push(`${timer.name}(IN := FALSE, PT := T#${timer.afterMs}ms);`);
  }
  if (timerDeclarations.length > 0) {
    body.push("");
  }

  body.push("CASE _state OF");

  for (const stateName of stateNames) {
    const stateConfig = config.states[stateName] ?? {};
    const stateIndex = stateIndexByName.get(stateName);
    if (stateIndex === undefined) {
      continue;
    }
    body.push(`${INDENT}${stateIndex}:`);

    const transitions = transitionsByState.get(stateName) ?? [];
    for (const transition of transitions) {
      if (transition.afterMs && transition.timerVarName) {
        body.push(
          `${INDENT}${INDENT}${transition.timerVarName}(IN := TRUE, PT := T#${transition.afterMs}ms);`
        );
      }
    }

    transitions.forEach((transition, transitionIndex) => {
      const triggerCondition = transition.afterMs
        ? `${transition.timerVarName}.Q`
        : eventInputByEventName.get(transition.eventName) ?? "FALSE";
      const guard = transition.guard?.trim()
        ? normalizeGuardExpression(transition.guard)
        : "";
      const condition = guard
        ? `(${triggerCondition}) AND (${guard})`
        : `${triggerCondition}`;

      const keyword = transitionIndex === 0 ? "IF" : "ELSIF";
      body.push(`${INDENT}${INDENT}${keyword} ${condition} THEN`);

      appendActionStatements(
        toActionList(stateConfig.exit),
        config.actionMappings,
        body,
        3
      );
      appendActionStatements(transition.actions, config.actionMappings, body, 3);

      const targetState = config.states[transition.target];
      if (targetState) {
        appendActionStatements(
          toActionList(targetState.entry),
          config.actionMappings,
          body,
          3
        );
        body.push(
          `${INDENT}${INDENT}${INDENT}_state := ${
            stateIndexByName.get(transition.target) ?? 0
          };`
        );
      } else {
        body.push(
          `${INDENT}${INDENT}${INDENT}(* Invalid transition target: ${transition.target} *)`
        );
      }
    });

    if (transitions.length > 0) {
      body.push(`${INDENT}${INDENT}END_IF;`);
    }
  }

  body.push("END_CASE;");
  body.push("");
  body.push("STATE_INDEX := _state;");
  body.push("CASE _state OF");
  for (const stateName of stateNames) {
    const stateIndex = stateIndexByName.get(stateName);
    if (stateIndex !== undefined) {
      body.push(
        `${INDENT}${stateIndex}: STATE_NAME := '${escapeStString(stateName)}';`
      );
    }
  }
  body.push(`${INDENT}ELSE`);
  body.push(`${INDENT}${INDENT}STATE_NAME := 'UNKNOWN';`);
  body.push("END_CASE;");

  const output: string[] = [
    "(*",
    "  Auto-generated Statechart companion.",
    "  Source of truth remains the .statechart.json file.",
    "  Generated to support .st-first runtime workflows.",
    "*)",
    "",
    `FUNCTION_BLOCK ${functionBlockName}`,
  ];

  if (eventInputByEventName.size > 0) {
    output.push("VAR_INPUT");
    for (const eventVar of eventInputByEventName.values()) {
      output.push(`${INDENT}${eventVar} : BOOL;`);
    }
    output.push("END_VAR");
  }

  output.push("VAR_OUTPUT");
  output.push(`${INDENT}STATE_INDEX : INT;`);
  output.push(`${INDENT}STATE_NAME : STRING[80];`);
  output.push("END_VAR");

  output.push("VAR");
  output.push(`${INDENT}_initialized : BOOL := FALSE;`);
  output.push(`${INDENT}_state : INT := 0;`);
  for (const timer of timerDeclarations) {
    output.push(
      `${INDENT}${timer.name} : TON; (* ${timer.stateName}.${timer.eventName} *)`
    );
  }
  output.push("END_VAR");
  output.push("");

  output.push("(* State encoding *)");
  stateNames.forEach((stateName, index) => {
    output.push(`(*${INDENT}${index} = ${escapeStString(stateName)} *)`);
  });
  output.push("");

  output.push(...body);
  output.push("END_FUNCTION_BLOCK");

  return rewriteDirectIoInFunctionBlock(output.join("\n")).source;
}
