import { useCallback, useEffect, useMemo, useRef, useState } from "react";

import type { PlantDraft } from "./addFlow";
import type { PlantSignalCatalog } from "./signalCatalog";

/**
 * The plant setup editor's link to Rust (specification 47 §20.1, `plant.edit`): the webview sends
 * the draft and asks for an edit; the extension host forwards it to the project's plant session,
 * and Rust answers with the draft, its validation issues, what the editor shows and the candidate
 * Save sends. The webview only renders the answer.
 */

export type PlantIssueTarget =
  | { kind: "identity"; field: "id" | "displayName" }
  | { kind: "port"; direction: "command" | "feedback"; index: number }
  | { kind: "dynamics"; key: string; index?: number }
  | { kind: "input_delay"; column: number | null }
  | { kind: "matrix"; matrix: "a" | "b" | "c" | "d" | "initial_state"; row: number; column: number }
  | { kind: "piecewise"; row: number; field: "input" | "output" }
  | { kind: "actuator"; key: string }
  | { kind: "signal"; key: string }
  | { kind: "effect"; row: number; field: string }
  | { kind: "event"; row: number; field: string }
  | { kind: "temperature_units" }
  | { kind: "configuration" };

export interface PlantEditorIssue {
  code: string;
  severity: string;
  path: string;
  plant_id?: string;
  message: string;
  summary: string;
  target: PlantIssueTarget;
}

export interface PlantFieldView {
  key: string;
  label: string;
  valueKind: "number" | "vector" | "matrix";
  plainTerm: string;
  controlTerm: string;
  unit: string;
  effect: string;
  storedKey: string;
  optional: boolean;
  defaultValue?: number;
  help?: string;
}

export type PlantTemperatureField = "resting_temperature" | "full_power_rise" | "time_constant_s" | "dead_time_s";

export interface PlantEditorView {
  model: { kind: string; productName: string; purpose: string; states: number; inputs: number; outputs: number };
  editor: "fields" | "piecewise" | "state_space";
  temperatureExampleAvailable: boolean;
  temperature: null | {
    fields: Array<{ key: PlantTemperatureField; label: string; help: string }>;
    values: Record<PlantTemperatureField, number | null>;
    nonEquilibrium: boolean;
    unitsConfirmed: boolean;
    commandAssumption: string;
    feedbackAssumption: string;
    heaterRange: string;
    equation: string[];
  };
  dynamics: { fields: PlantFieldView[]; details: PlantFieldView[] };
  startingState: { kind: "scalar" | "fields" | "vector"; fields: Array<{ label: string; description: string }>; text?: string };
  ports: { commands: string[]; feedbacks: string[] };
  signals: {
    commands: Array<{ address: string; initialInput: number | null; inputScale: number | null; inputOffset: number | null }>;
    feedbacks: Array<{ address: string; outputScale: number | null; outputOffset: number | null }>;
  };
  stepText: string;
  stateSpace: null | {
    dimensions: { states: number; inputs: number; outputs: number };
    delays: Array<number | null>;
    delaysExplicit: boolean;
  };
  piecewise: null | {
    points: Array<{ input: number | null; output: number | null; canMoveUp: boolean; canMoveDown: boolean }>;
    canRemove: boolean;
  };
  parameters: string[];
  effects: Array<{ preset: string; label: string; boundary: string; expectation: string; fields: string[] }>;
  events: Array<{
    timeText: string;
    canMoveUp: boolean;
    canMoveDown: boolean;
    showsChannel: boolean;
    channelLabel: string;
    channelOptions: Array<{ value: number; label: string }>;
    showsParameter: boolean;
    showsValue: boolean;
  }>;
  summaries: { actuator: string; signals: string; realism: string; scenarios: string };
}

export type PlantEditOutcome =
  | { kind: "applied"; index?: number }
  | { kind: "confirm"; message: string }
  | { kind: "rejected"; message: string };

export interface PlantEditAnswer {
  draft: PlantDraft;
  outcome?: PlantEditOutcome;
  issues: PlantEditorIssue[];
  can_save: boolean;
  view: PlantEditorView;
  candidate: Record<string, unknown>;
}

/** The editor tables of `plant.capabilities`: every label, help text, default and choice. */
export interface PlantEditorTables {
  models: Array<{
    kind: string;
    productName: string;
    purpose: string;
    states: number;
    inputs: number;
    outputs: number;
    dynamicsFields: PlantFieldView[];
    startingStateFields: Array<{ label: string; description: string }>;
    startingPoint: string;
  }>;
  actuatorFields: Array<{
    key: string;
    label: string;
    plainTerm: string;
    controlTerm: string;
    unit: string;
    effect: string;
    blankMeaning: string;
    storedKey: string;
    defaultValue?: number;
    defaultFrom?: string;
  }>;
  startingPoints: Array<{ id: string; label: string; model: string; whyReasonable: string; limitation: string }>;
  customStateSpace: { id: string; whyReasonable: string; limitation: string };
  imperfectionTypes: Array<{ id: string; label: string; expectation: string }>;
  uncertaintyTargets: string[];
  uncertaintyModes: string[];
  uncertaintyGenerators: Array<{ id: string; fields: string[] }>;
  uncertaintyFieldLabels: Record<string, string>;
  uncertaintyDropoutPolicies: string[];
  scenarioEventKinds: Array<{ kind: string; label: string }>;
}

export type PlantEditAction = { kind: string; [key: string]: unknown };

export type PlantEditResult = { ok: true; answer: PlantEditAnswer } | { ok: false; message: string };

type Poster = (message: unknown) => void;

const ANSWER_TIMEOUT_MS = 15_000;
const pending = new Map<number, (result: PlantEditResult) => void>();
let nextRequestId = 1;

/** Sends one `plant.edit` request through the extension host and waits for its answer. */
export function requestPlantEdit(
  post: Poster,
  request: { draft: PlantDraft; action?: PlantEditAction; signals: PlantSignalCatalog }
): Promise<PlantEditResult> {
  const requestId = nextRequestId++;
  return new Promise((resolve) => {
    const timer = setTimeout(() => {
      if (pending.delete(requestId)) {
        resolve({ ok: false, message: "The plant editor did not answer in time." });
      }
    }, ANSWER_TIMEOUT_MS);
    pending.set(requestId, (result) => {
      clearTimeout(timer);
      resolve(result);
    });
    post({ type: "plantEdit", requestId, ...request });
  });
}

/** Delivers a `plantEditResult` message to the request that asked for it; true when it was one. */
export function receivePlantEditResult(message: Record<string, unknown>): boolean {
  if (message.type !== "plantEditResult") return false;
  const requestId = typeof message.requestId === "number" ? message.requestId : -1;
  const resolve = pending.get(requestId);
  if (!resolve) return true;
  pending.delete(requestId);
  if (message.ok === true && message.answer && typeof message.answer === "object") {
    resolve({ ok: true, answer: message.answer as PlantEditAnswer });
  } else {
    resolve({ ok: false, message: typeof message.message === "string" ? message.message : "The plant editor is unavailable." });
  }
  return true;
}

const CHECK_DELAY_MS = 150;

/**
 * How often an action is applied again to a draft that changed while Rust answered: the canvas
 * stores each draft it is given as a new object, and the user may edit meanwhile.
 */
const ACTION_ATTEMPTS = 5;

/**
 * Keeps the draft's answer from Rust. Rust first opens each draft (a new plant's proposal, a saved
 * plant's defaults); then every draft change is checked after a short pause, without blocking input,
 * and only the answer for the current draft counts. `run` asks Rust for an edit; its draft replaces
 * the local one unless the user changed the draft meanwhile, in which case the edit is asked again
 * of the newer draft.
 */
export function usePlantEditor(options: {
  draft: PlantDraft;
  post: Poster;
  signals: PlantSignalCatalog;
  onDraft: (draft: PlantDraft) => void;
  onOpened: (draft: PlantDraft) => void;
}) {
  const { draft, post, signals, onDraft, onOpened } = options;
  const [answer, setAnswer] = useState<PlantEditAnswer>();
  const [answeredDraft, setAnsweredDraft] = useState<PlantDraft>();
  const [failure, setFailure] = useState<string>();
  const latest = useRef(draft);
  latest.current = draft;
  const signalsRef = useRef(signals);
  signalsRef.current = signals;
  const onOpenedRef = useRef(onOpened);
  onOpenedRef.current = onOpened;
  const answered = useRef<PlantDraft | undefined>(undefined);
  const opening = useRef<string | undefined>(undefined);

  const adopt = useCallback((requested: PlantDraft, result: PlantEditResult) => {
    if (!result.ok) {
      setFailure(result.message);
      return;
    }
    setFailure(undefined);
    if (latest.current === requested) {
      answered.current = requested;
      setAnswer(result.answer);
      setAnsweredDraft(requested);
    }
  }, []);

  // Each draft is opened once; the editor shows nothing to edit until Rust's answer arrives.
  useEffect(() => {
    if (opening.current === draft.id) return;
    opening.current = draft.id;
    answered.current = undefined;
    setAnswer(undefined);
    setAnsweredDraft(undefined);
    setFailure(undefined);
    const requested = latest.current;
    void requestPlantEdit(post, { draft: requested, action: { kind: "open" }, signals: signalsRef.current }).then((result) => {
      if (latest.current.id !== requested.id) return;
      if (!result.ok) {
        setFailure(result.message);
        return;
      }
      const opened = result.answer.draft;
      answered.current = opened;
      setAnswer(result.answer);
      setAnsweredDraft(opened);
      onOpenedRef.current(opened);
    });
  }, [draft.id, post]);

  // Every later draft, and a catalog that changed, is checked after a short pause. The canvas
  // resends an unchanged catalog with every refresh; that asks for nothing.
  const signalsKey = useMemo(() => JSON.stringify(signals), [signals]);
  const checkedSignals = useRef(signalsKey);
  useEffect(() => {
    const newSignals = checkedSignals.current !== signalsKey;
    if (answered.current === undefined || (answered.current === draft && !newSignals)) return;
    const requested = draft;
    const timer = setTimeout(() => {
      checkedSignals.current = signalsKey;
      void requestPlantEdit(post, { draft: requested, signals: signalsRef.current }).then((result) => adopt(requested, result));
    }, CHECK_DELAY_MS);
    return () => clearTimeout(timer);
  }, [draft, signalsKey, post, adopt]);

  // The actions run one after another, each on the draft the one before left, so that two ports
  // bound at once both reach the draft. An action whose draft the user edited while Rust answered
  // is applied again to the edited draft.
  const queue = useRef<Promise<unknown>>(Promise.resolve());
  const run = useCallback(
    (action: PlantEditAction): Promise<PlantEditResult> => {
      const next = queue.current.then(async (): Promise<PlantEditResult> => {
        for (let attempt = 0; attempt < ACTION_ATTEMPTS; attempt += 1) {
          const requested = latest.current;
          const result = await requestPlantEdit(post, { draft: requested, action, signals: signalsRef.current });
          if (!result.ok) {
            setFailure(result.message);
            return result;
          }
          if (latest.current !== requested) continue;
          if (result.answer.outcome?.kind === "applied") {
            // The answer's view already shows the edited draft; its issues count once it is current.
            setAnswer(result.answer);
            latest.current = result.answer.draft;
            onDraft(result.answer.draft);
          }
          return result;
        }
        return { ok: false, message: "The draft changed while the edit was applied; try again." };
      });
      queue.current = next.catch(() => undefined);
      return next;
    },
    [post, onDraft]
  );

  const current = answer !== undefined && answeredDraft === draft;
  // A field edited from one render's draft goes onto the draft as it is now, so that a Rust answer
  // applied since that render, such as a port binding, keeps what it changed.
  const edit = useCallback(
    (next: PlantDraft, base: PlantDraft) => {
      latest.current = rebased(next, base, latest.current);
      onDraft(latest.current);
    },
    [onDraft]
  );
  return {
    /** The latest answer, also while a newer draft is being checked: its view still applies. */
    answer,
    /** Whether `answer` belongs to the draft as it is now: only then are its issues shown. */
    current,
    failure,
    run,
    /** Sends a field edit made from the draft `base` as the new draft. */
    edit,
  };
}

/**
 * `next`, a draft edited from `base`, with the parts the edit left as they were taken from
 * `current`, the draft as it is now.
 */
function rebased(next: PlantDraft, base: PlantDraft, current: PlantDraft): PlantDraft {
  if (current === base) return next;
  const edited = next as unknown as Record<string, unknown>;
  const from = base as unknown as Record<string, unknown>;
  const merged: Record<string, unknown> = { ...(current as unknown as Record<string, unknown>) };
  for (const key of new Set([...Object.keys(edited), ...Object.keys(from)])) {
    if (edited[key] !== from[key]) merged[key] = edited[key];
  }
  return merged as unknown as PlantDraft;
}

/** A draft number as its text field shows it: a finite number as written, anything else empty. */
export function numberText(value: unknown): string {
  return typeof value === "number" && Number.isFinite(value) ? String(value) : "";
}

/** The value a typed number spells for the draft: empty text is no value, other text `Number(text)`. */
export function typedNumber(text: string): number {
  return text.trim() ? Number(text) : Number.NaN;
}
