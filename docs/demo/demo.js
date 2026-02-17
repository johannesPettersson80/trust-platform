import { buildRequestPositions } from "./lsp-position-resolver.js";
import { getWalkthroughAction } from "./walkthrough-actions.js";

// truST Demo – standalone GitHub Pages demo orchestration
// Loads Monaco + WASM analysis engine, registers all 7 LSP providers.

// ── Constants ────────────────────────────────────────

const ST_LANGUAGE_ID = "trust-st";
const MARKER_OWNER = "trust.demo";
const REQUEST_TIMEOUT_MS = Object.freeze({
  applyDocuments: 20000,
  diagnostics: 12000,
  hover: 10000,
  completion: 12000,
  definition: 12000,
  references: 12000,
  rename: 15000,
  documentHighlight: 10000,
});

// ── DOM References ───────────────────────────────────

const dom = {
  editorMount: document.getElementById("editorMount"),
  fileTabs: document.getElementById("fileTabs"),
  walkthroughSteps: document.getElementById("walkthroughSteps"),
  wasmBadge: document.getElementById("wasmBadge"),
  statusDot: document.getElementById("statusDot"),
  statusLabel: document.getElementById("statusLabel"),
  cursorLabel: document.getElementById("cursorLabel"),
  fileCount: document.getElementById("fileCount"),
  diagCount: document.getElementById("diagCount"),
};

// ── Demo ST Files ────────────────────────────────────

const DEMO_FILES = [
  {
    name: "types.st",
    uri: "types.st",
    content: `TYPE
    E_PumpState : (Idle := 0, Starting := 1, Running := 2, Fault := 3);

    ST_PumpCommand :
    STRUCT
        Enable : BOOL;
        TargetSpeed : REAL;
    END_STRUCT;

    ST_PumpStatus :
    STRUCT
        Running : BOOL;
        State : E_PumpState;
        ActualSpeed : REAL;
        Alarm : UDINT;
    END_STRUCT;
END_TYPE
`,
  },
  {
    name: "fb_pump.st",
    uri: "fb_pump.st",
    content: `FUNCTION_BLOCK FB_Pump
VAR_INPUT
    Command : ST_PumpCommand;
END_VAR
VAR_OUTPUT
    Status : ST_PumpStatus;
END_VAR
VAR
    ramp_timer : TON;
    ramp : REAL;
END_VAR
VAR CONSTANT
    RAMP_TIME : TIME := T#1s;
END_VAR

Status.Running := FALSE;
Status.Alarm := 0;

IF NOT Command.Enable THEN
    Status.State := E_PumpState#Idle;
    Status.ActualSpeed := 0.0;
    ramp := 0.0;
    ramp_timer(IN := FALSE);
ELSE
    CASE Status.State OF
        E_PumpState#Idle:
            Status.State := E_PumpState#Starting;
            ramp_timer(IN := TRUE, PT := RAMP_TIME);
        E_PumpState#Starting:
            ramp_timer(IN := TRUE, PT := RAMP_TIME);
            IF ramp_timer.Q THEN
                Status.State := E_PumpState#Running;
            ELSE
                ramp := ramp + 0.2;
            END_IF
        E_PumpState#Running:
            Status.Running := TRUE;
            ramp := Command.TargetSpeed;
        E_PumpState#Fault:
            Status.Running := FALSE;
            Status.Alarm := 16#BEEF;
    END_CASE
END_IF

Status.ActualSpeed := ramp;
END_FUNCTION_BLOCK
`,
  },
  {
    name: "program.st",
    uri: "program.st",
    content: `PROGRAM PlantProgram
VAR
    Pump : FB_Pump;
    Cmd : ST_PumpCommand;
    Status : ST_PumpStatus;
    StartCmd : BOOL;
    SpeedRaw : INT;
    SpeedSet : REAL;
    HaltReq : BOOL;
END_VAR

Cmd.Enable := StartCmd AND NOT HaltReq;
SpeedSet := INT_TO_REAL(SpeedRaw);
Cmd.TargetSpeed := SpeedSet;

Pump(Command := Cmd);
Status := Pump.Status;
HaltReq := FALSE;
IF Status.State = E_PumpState#Fault THEN
    HaltReq := TRUE;
END_IF
END_PROGRAM
`,
  },
  {
    name: "config.st",
    uri: "config.st",
    content: `CONFIGURATION PlantDemo
TASK Fast (INTERVAL := T#100ms, PRIORITY := 1);
PROGRAM P1 WITH Fast : PlantProgram;
VAR_CONFIG
    P1.StartCmd AT %IX0.0 : BOOL;
    P1.SpeedRaw AT %IW0 : INT;
    P1.HaltReq AT %QX0.0 : BOOL;
END_VAR
END_CONFIGURATION
`,
  },
];

// ── Walkthrough Steps ────────────────────────────────

const WALKTHROUGH = [
  {
    title: "Diagnostics",
    hint: "No squiggles are shown on startup because this project is clean. To demo diagnostics: open <kbd>program.st</kbd>, change <kbd>Cmd.Enable</kbd> to <kbd>Cmd.Enabl</kbd>, wait about 0.5s for squiggles/diagnostics, then change it back.",
  },
  {
    title: "Hover",
    hint: "Hover over <kbd>FB_Pump</kbd> or <kbd>E_PumpState</kbd> to see type signatures and documentation.",
  },
  {
    title: "Completion",
    hint: "Place your cursor after <kbd>Status.</kbd> and press <kbd>Ctrl+Space</kbd> to see autocomplete suggestions for struct fields.",
  },
  {
    title: "Go to Definition",
    hint: "In <kbd>program.st</kbd>, left-click <kbd>E_PumpState</kbd>, then press <kbd>F12</kbd> (or <kbd>Ctrl+Left-click</kbd>) to jump to its definition in types.st.",
  },
  {
    title: "Find References",
    hint: "Left-click <kbd>Enable</kbd> first, then press <kbd>Shift+F12</kbd> (or choose <em>Go to References</em>) to see every usage across files.",
  },
  {
    title: "Document Highlights",
    hint: "Click on any variable name &mdash; all occurrences in the current file highlight instantly.",
  },
  {
    title: "Rename",
    hint: "Left-click <kbd>ActualSpeed</kbd> first, then press <kbd>F2</kbd> (or <kbd>Fn+F2</kbd> on laptops) to rename it across all files in the project.",
  },
];

// ── State ────────────────────────────────────────────

let monaco = null;
let editor = null;
let wasmClient = null;
let activeFileIndex = 0;
let models = [];
let documentHighlightDecorations = [];
let documentHighlightTimer = null;
let lastSyncedVersionKey = "";
let syncInFlight = null;
let editorOpenerDisposable = null;

// ── Helpers ──────────────────────────────────────────

function fromMonacoPosition(position) {
  if (!position) return { line: 0, character: 0 };
  return {
    line: Math.max(0, Number(position.lineNumber || 1) - 1),
    character: Math.max(0, Number(position.column || 1) - 1),
  };
}

function clamp(value, min, max) {
  return Math.min(max, Math.max(min, value));
}

function toMonacoPosition(position, model) {
  const maxLines = model ? model.getLineCount() : 1;
  const line = clamp(Number(position?.line ?? 0) + 1, 1, Math.max(1, maxLines));
  const maxColumn = model ? model.getLineMaxColumn(line) : 1;
  const column = clamp(Number(position?.character ?? 0) + 1, 1, Math.max(1, maxColumn));
  return new monaco.Position(line, column);
}

function toMonacoRange(range, model) {
  const start = toMonacoPosition(range?.start || { line: 0, character: 0 }, model);
  const end = toMonacoPosition(range?.end || range?.start || { line: 0, character: 1 }, model);
  return new monaco.Range(
    start.lineNumber,
    start.column,
    Math.max(start.lineNumber, end.lineNumber),
    end.lineNumber < start.lineNumber ? start.column : Math.max(start.column, end.column),
  );
}

function monacoCompletionKind(kind) {
  const value = String(kind || "").toLowerCase();
  if (value.includes("function")) return monaco.languages.CompletionItemKind.Function;
  if (value.includes("method")) return monaco.languages.CompletionItemKind.Method;
  if (value.includes("class")) return monaco.languages.CompletionItemKind.Class;
  if (value.includes("module")) return monaco.languages.CompletionItemKind.Module;
  if (value.includes("field")) return monaco.languages.CompletionItemKind.Field;
  if (value.includes("property")) return monaco.languages.CompletionItemKind.Property;
  if (value.includes("variable")) return monaco.languages.CompletionItemKind.Variable;
  if (value.includes("enum")) return monaco.languages.CompletionItemKind.Enum;
  if (value.includes("keyword")) return monaco.languages.CompletionItemKind.Keyword;
  if (value.includes("snippet")) return monaco.languages.CompletionItemKind.Snippet;
  if (value.includes("type")) return monaco.languages.CompletionItemKind.TypeParameter;
  return monaco.languages.CompletionItemKind.Text;
}

function monacoMarkerSeverity(severity) {
  const value = String(severity || "").toLowerCase();
  if (value.includes("error")) return monaco.MarkerSeverity.Error;
  if (value.includes("info")) return monaco.MarkerSeverity.Info;
  if (value.includes("hint")) return monaco.MarkerSeverity.Hint;
  return monaco.MarkerSeverity.Warning;
}

function normalizeHoverContentValue(contents) {
  if (typeof contents === "string") return contents.trim();
  if (Array.isArray(contents)) {
    return contents
      .map((e) => (typeof e === "string" ? e.trim() : e?.value?.trim?.() || ""))
      .filter((v) => v.length > 0)
      .join("\n\n")
      .trim();
  }
  if (contents && typeof contents === "object" && typeof contents.value === "string") {
    return contents.value.trim();
  }
  return "";
}

function fallbackCompletionRange(model, position) {
  const word = model.getWordUntilPosition(position);
  return new monaco.Range(
    position.lineNumber,
    word.startColumn || position.column,
    position.lineNumber,
    word.endColumn || position.column,
  );
}

function summarizeResult(result) {
  if (result == null) return "null";
  if (Array.isArray(result)) return `array(${result.length})`;
  if (typeof result === "object") {
    if (Array.isArray(result.edits)) return `edits(${result.edits.length})`;
    if (result.changes && typeof result.changes === "object") {
      return `changes(${Object.keys(result.changes).length})`;
    }
    return "object";
  }
  return typeof result;
}

function describeWord(model, candidate) {
  const word =
    model.getWordAtPosition(candidate)
    || model.getWordAtPosition({
      lineNumber: candidate.lineNumber,
      column: Math.max(1, candidate.column - 1),
    })
    || model.getWordAtPosition({
      lineNumber: candidate.lineNumber,
      column: candidate.column + 1,
    });
  return word ? word.word : null;
}

function requestCandidates(model, position) {
  return buildRequestPositions(model, position)
    .map((candidate) => ({
      monaco: candidate,
      protocol: fromMonacoPosition(candidate),
      word: describeWord(model, candidate),
    }))
    .filter((candidate) => candidate.protocol.character >= 0);
}

function hasNonEmptyResult(result) {
  if (result == null) return false;
  if (Array.isArray(result)) return result.length > 0;
  if (typeof result === "object") {
    if (Array.isArray(result.edits)) return result.edits.length > 0;
    if (result.changes && typeof result.changes === "object") {
      return Object.values(result.changes).some(
        (entries) => Array.isArray(entries) && entries.length > 0,
      );
    }
  }
  return true;
}

async function requestWithPositionFallback(
  model,
  position,
  query,
  hasResult = hasNonEmptyResult,
  context = {},
) {
  let lastError = null;
  const candidates = requestCandidates(model, position);

  for (let index = 0; index < candidates.length; index += 1) {
    const candidate = candidates[index];
    try {
      const result = await query(candidate.protocol);
      const ok = hasResult(result);
      if (ok) {
        return result;
      }
    } catch (error) {
      lastError = error;
    }
  }
  if (lastError) throw lastError;
  return null;
}

function setStatus(text, level) {
  dom.statusLabel.textContent = text;
  dom.statusDot.className = `status-dot ${level || ""}`;
}

function setWasmBadge(text, level) {
  dom.wasmBadge.className = `demo-badge ${level || ""}`;
  dom.wasmBadge.innerHTML = level === "loading"
    ? `<span class="demo-spinner"></span> ${text}`
    : text;
}

function normalizeDemoUri(uri) {
  const raw = String(uri || "").trim();
  if (!raw) return "";
  const withoutScheme = raw
    .replace(/^file:\/\/\//i, "")
    .replace(/^memory:\/\/\//i, "")
    .replace(/^https?:\/\/[^/]+\//i, "");
  return withoutScheme.replace(/^\/+/, "");
}

function findModelByUri(uri) {
  const key = normalizeDemoUri(uri);
  const index = DEMO_FILES.findIndex((f) => {
    const fileKey = normalizeDemoUri(f.uri);
    return key === fileKey || key.endsWith(`/${fileKey}`);
  });
  return index >= 0 ? models[index] : null;
}

function findFileIndexByResource(resource) {
  const raw = typeof resource === "string" ? resource : resource?.toString?.() || "";
  if (!raw) return -1;
  const byExactModel = models.findIndex(
    (model) => model && String(model.uri?.toString?.() || "") === raw,
  );
  if (byExactModel >= 0) return byExactModel;

  const key = normalizeDemoUri(raw);
  return DEMO_FILES.findIndex((file) => {
    const fileKey = normalizeDemoUri(file.uri);
    return key === fileKey || key.endsWith(`/${fileKey}`);
  });
}

function toSelectionOrPosition(selectionOrPosition, model) {
  if (!selectionOrPosition || !model || !monaco) return null;
  if (
    Number.isFinite(selectionOrPosition.startLineNumber)
    && Number.isFinite(selectionOrPosition.startColumn)
  ) {
    const startLine = clamp(selectionOrPosition.startLineNumber, 1, model.getLineCount());
    const startCol = clamp(selectionOrPosition.startColumn, 1, model.getLineMaxColumn(startLine));
    const endLine = Number.isFinite(selectionOrPosition.endLineNumber)
      ? clamp(selectionOrPosition.endLineNumber, 1, model.getLineCount())
      : startLine;
    const endCol = Number.isFinite(selectionOrPosition.endColumn)
      ? clamp(selectionOrPosition.endColumn, 1, model.getLineMaxColumn(endLine))
      : startCol;
    return new monaco.Selection(startLine, startCol, endLine, endCol);
  }
  if (
    Number.isFinite(selectionOrPosition.lineNumber)
    && Number.isFinite(selectionOrPosition.column)
  ) {
    const lineNumber = clamp(selectionOrPosition.lineNumber, 1, model.getLineCount());
    const column = clamp(selectionOrPosition.column, 1, model.getLineMaxColumn(lineNumber));
    return new monaco.Position(lineNumber, column);
  }
  return null;
}

function openResourceInEditor(resource, selectionOrPosition) {
  const index = findFileIndexByResource(resource);
  if (index < 0 || !editor || !models[index]) {
    return false;
  }

  switchToFile(index);
  const model = models[index];
  if (editor.getModel() !== model) {
    editor.setModel(model);
  }

  const selection = toSelectionOrPosition(selectionOrPosition, model);
  if (selection instanceof monaco.Selection) {
    editor.setSelection(selection);
    editor.revealRangeInCenter(selection);
  } else if (selection instanceof monaco.Position) {
    editor.setPosition(selection);
    editor.revealPositionInCenter(selection);
  } else {
    editor.revealLineInCenter(1);
  }
  editor.focus();
  return true;
}

function modelVersionKey() {
  return models
    .map((model, index) => `${DEMO_FILES[index].uri}:${model ? model.getVersionId() : 0}`)
    .join("|");
}

// ── Monaco Setup ─────────────────────────────────────

async function loadMonaco() {
  setStatus("Loading editor...", "loading");
  const module = await import("./assets/ide-monaco.20260215.js");
  monaco = module.monaco;
  module.ensureStyleInjected();
  registerSTLanguage();
  defineThemes();
}

function registerSTLanguage() {
  if (monaco.languages.getLanguages().some((l) => l.id === ST_LANGUAGE_ID)) return;
  monaco.languages.register({
    id: ST_LANGUAGE_ID,
    extensions: [".st"],
    aliases: ["Structured Text", "ST"],
  });
  monaco.languages.setMonarchTokensProvider(ST_LANGUAGE_ID, {
    defaultToken: "",
    keywords: [
      "PROGRAM", "END_PROGRAM", "FUNCTION", "END_FUNCTION", "FUNCTION_BLOCK",
      "END_FUNCTION_BLOCK", "CONFIGURATION", "END_CONFIGURATION", "TASK", "INTERVAL",
      "PRIORITY", "PROGRAM", "WITH", "VAR", "VAR_INPUT", "VAR_OUTPUT", "VAR_IN_OUT",
      "VAR_GLOBAL", "VAR_CONFIG", "VAR_ACCESS", "END_VAR", "IF", "THEN", "ELSIF",
      "ELSE", "END_IF", "CASE", "OF", "END_CASE", "FOR", "TO", "BY", "DO", "END_FOR",
      "WHILE", "END_WHILE", "REPEAT", "UNTIL", "END_REPEAT", "TRUE", "FALSE", "BOOL",
      "INT", "DINT", "UINT", "UDINT", "REAL", "LREAL", "STRING", "TYPE", "END_TYPE",
      "STRUCT", "END_STRUCT", "AT", "CONSTANT", "TIME", "NOT", "AND", "OR",
    ],
    operators: [":=", "=", "<>", "<=", ">=", "<", ">", "+", "-", "*", "/"],
    tokenizer: {
      root: [
        [/[A-Za-z_][A-Za-z0-9_]*/, {
          cases: { "@keywords": "keyword.st", "@default": "identifier" },
        }],
        [/[0-9]+(\.[0-9]+)?/, "number.st"],
        [/\/\/.*$/, "comment"],
        [/\(\*[\s\S]*?\*\)/, "comment"],
        [/".*?"/, "string"],
        [/'[^']*'/, "string"],
        [/[+\-*\/=<>:]+/, "operator"],
      ],
    },
  });
  monaco.languages.setLanguageConfiguration(ST_LANGUAGE_ID, {
    comments: { lineComment: "//", blockComment: ["(*", "*)"] },
    brackets: [["(", ")"], ["[", "]"]],
  });
}

function defineThemes() {
  monaco.editor.defineTheme("trust-dark", {
    base: "vs-dark",
    inherit: true,
    rules: [
      { token: "keyword.st", foreground: "14b8a6", fontStyle: "bold" },
      { token: "number.st", foreground: "e0c95a" },
    ],
    colors: {
      "editor.background": "#0f1115",
      "editorCursor.foreground": "#14b8a6",
      "editorLineNumber.foreground": "#6f7d9b",
      "editorLineNumber.activeForeground": "#dce6ff",
      "editorGutter.background": "#141821",
      "editor.selectionBackground": "#14b8a633",
      "editor.inactiveSelectionBackground": "#14b8a619",
      "editor.wordHighlightBackground": "#14b8a635",
      "editor.wordHighlightStrongBackground": "#14b8a650",
      "editor.selectionHighlightBackground": "#14b8a625",
      "editor.selectionHighlightBorder": "#14b8a655",
      "editorWidget.background": "#1f2430",
      "editorWidget.foreground": "#f2f2f2",
      "editorWidget.border": "#3c4b66",
      "editorHoverWidget.background": "#1f2430",
      "editorHoverWidget.foreground": "#f2f2f2",
      "editorHoverWidget.border": "#3c4b66",
      "editorSuggestWidget.background": "#1f2430",
      "editorSuggestWidget.foreground": "#f2f2f2",
      "editorSuggestWidget.border": "#3c4b66",
      "editorSuggestWidget.selectedBackground": "#1f3c4a",
      "editorSuggestWidget.highlightForeground": "#5eead4",
    },
  });
}

// ── WASM Analysis Client ─────────────────────────────

async function loadWasmClient() {
  setStatus("Loading WASM analysis engine...", "loading");
  setWasmBadge("Loading WASM...", "loading");
  const { TrustWasmAnalysisClient } = await import("./wasm/analysis-client.js");
  wasmClient = new TrustWasmAnalysisClient({
    workerUrl: "./wasm/worker.js",
    defaultTimeoutMs: REQUEST_TIMEOUT_MS.completion,
  });
  let wasmReadyUiSet = false;
  const markWasmReady = () => {
    if (wasmReadyUiSet) return;
    wasmReadyUiSet = true;
    setWasmBadge("WASM Ready", "ok");
    setStatus("WASM analysis engine ready", "ready");
  };
  wasmClient.onStatus((status) => {
    if (status.type === "ready") {
      markWasmReady();
    } else if (status.type === "fatal") {
      setWasmBadge("WASM Error", "err");
      setStatus("WASM error: " + status.error, "error");
    }
  });
  await wasmClient.ready();
  // Guard against a startup race where the worker posts "ready" before the
  // status listener is attached.
  markWasmReady();
}

async function syncAllDocuments() {
  if (!wasmClient) return;
  if (syncInFlight) {
    try {
      await syncInFlight;
    } catch {
      // Allow a fresh sync attempt below.
    }
  }
  const versionKey = modelVersionKey();
  if (versionKey === lastSyncedVersionKey) {
    return;
  }
  const documents = DEMO_FILES.map((f, i) => ({
    uri: f.uri,
    text: models[i] ? models[i].getValue() : f.content,
  }));
  syncInFlight = wasmClient.applyDocuments(documents, REQUEST_TIMEOUT_MS.applyDocuments);
  try {
    await syncInFlight;
    lastSyncedVersionKey = versionKey;
  } finally {
    syncInFlight = null;
  }
}

// ── Diagnostics ──────────────────────────────────────

async function runDiagnosticsForAll() {
  if (!wasmClient || !monaco) return;
  let totalDiagnostics = 0;
  for (let i = 0; i < DEMO_FILES.length; i++) {
    try {
      const result = await wasmClient.diagnostics(
        DEMO_FILES[i].uri,
        REQUEST_TIMEOUT_MS.diagnostics,
      );
      const items = Array.isArray(result) ? result : [];
      totalDiagnostics += items.length;
      if (models[i]) {
        applyMarkers(items, models[i]);
      }
    } catch {
      // continue with other files
    }
  }
  dom.diagCount.textContent = `${totalDiagnostics} diagnostic${totalDiagnostics !== 1 ? "s" : ""}`;
}

function applyMarkers(items, model) {
  const markers = items.map((item) => {
    const range = toMonacoRange(item.range || {}, model);
    return {
      startLineNumber: range.startLineNumber,
      startColumn: range.startColumn,
      endLineNumber: range.endLineNumber,
      endColumn: Math.max(range.startColumn + 1, range.endColumn),
      severity: monacoMarkerSeverity(item.severity),
      message: item.message || "diagnostic",
      code: item.code ? String(item.code) : undefined,
    };
  });
  monaco.editor.setModelMarkers(model, MARKER_OWNER, markers);
}

// ── LSP Providers ────────────────────────────────────

function registerProviders() {
  // Completion
  const triggerCharacters = [
    "_", ".", ...Array.from("abcdefghijklmnopqrstuvwxyzABCDEFGHIJKLMNOPQRSTUVWXYZ"),
  ];
  monaco.languages.registerCompletionItemProvider(ST_LANGUAGE_ID, {
    triggerCharacters,
    async provideCompletionItems(model, position) {
      const file = DEMO_FILES.find((f) => findModelByUri(f.uri) === model);
      if (!file || !wasmClient) return { suggestions: [] };
      try {
        await syncAllDocuments();
        const items = await requestWithPositionFallback(
          model,
          position,
          (cursor) => wasmClient.completion(
            file.uri,
            cursor,
            120,
            REQUEST_TIMEOUT_MS.completion,
          ),
          (value) => Array.isArray(value) && value.length > 0,
          { operation: "completion", fileUri: file.uri },
        );
        if (!Array.isArray(items) || items.length === 0) return { suggestions: [] };
        const defaultRange = fallbackCompletionRange(model, position);
        const suggestions = items
          .filter((item) => item && typeof item.label === "string" && item.label.length > 0)
          .map((item) => {
            let range = defaultRange;
            if (item.text_edit?.range) {
              const candidateRange = toMonacoRange(item.text_edit.range, model);
              if (candidateRange.containsPosition(position)) {
                range = candidateRange;
              }
            }
            const priority = Number(item.sort_priority);
            const sortText = item.sort_text || (Number.isFinite(priority) ? String(priority).padStart(6, "0") : undefined);
            return {
              label: item.label,
              kind: monacoCompletionKind(item.kind),
              detail: item.detail || "",
              documentation: item.documentation ? { value: String(item.documentation) } : undefined,
              insertText: item.text_edit?.new_text || item.insert_text || item.label,
              range,
              sortText,
              filterText: item.filter_text || undefined,
            };
          });
        return { suggestions };
      } catch (error) {
        console.warn("[demo] completion request failed:", error);
        return { suggestions: [] };
      }
    },
  });

  // Hover
  monaco.languages.registerHoverProvider(ST_LANGUAGE_ID, {
    async provideHover(model, position) {
      const file = DEMO_FILES.find((f) => findModelByUri(f.uri) === model);
      if (!file || !wasmClient) return null;
      try {
        await syncAllDocuments();
        const response = await requestWithPositionFallback(
          model,
          position,
          (cursor) => wasmClient.hover(file.uri, cursor, REQUEST_TIMEOUT_MS.hover),
          (value) => Boolean(value && value.contents),
          { operation: "hover", fileUri: file.uri },
        );
        if (!response || !response.contents) return null;
        const hoverText = normalizeHoverContentValue(response.contents);
        if (!hoverText) return null;
        const hover = { contents: [{ value: hoverText }] };
        if (response.range) hover.range = toMonacoRange(response.range, model);
        return hover;
      } catch {
        return null;
      }
    },
  });

  // Go to Definition
  monaco.languages.registerDefinitionProvider(ST_LANGUAGE_ID, {
    async provideDefinition(model, position) {
      const file = DEMO_FILES.find((f) => findModelByUri(f.uri) === model);
      if (!file || !wasmClient) return null;
      try {
        await syncAllDocuments();
        const result = await requestWithPositionFallback(
          model,
          position,
          (cursor) => wasmClient.definition(
            file.uri,
            cursor,
            REQUEST_TIMEOUT_MS.definition,
          ),
          (value) => {
            const locations = Array.isArray(value) ? value : value ? [value] : [];
            return locations.some((loc) => loc && loc.uri && loc.range);
          },
          { operation: "definition", fileUri: file.uri },
        );
        if (!result) return null;
        const locations = Array.isArray(result) ? result : [result];
        return locations
          .filter((loc) => loc && loc.uri && loc.range)
          .map((loc) => {
            const targetModel = findModelByUri(loc.uri);
            if (!targetModel) return null;
            const range = toMonacoRange(loc.range, targetModel);
            return { uri: targetModel.uri, range };
          })
          .filter(Boolean);
      } catch {
        return null;
      }
    },
  });

  // Find References
  monaco.languages.registerReferenceProvider(ST_LANGUAGE_ID, {
    async provideReferences(model, position, context) {
      const file = DEMO_FILES.find((f) => findModelByUri(f.uri) === model);
      if (!file || !wasmClient) return null;
      try {
        await syncAllDocuments();
        const result = await requestWithPositionFallback(
          model,
          position,
          (cursor) => wasmClient.references(
            file.uri,
            cursor,
            context?.includeDeclaration !== false,
            REQUEST_TIMEOUT_MS.references,
          ),
          (value) => Array.isArray(value) && value.length > 0,
          { operation: "references", fileUri: file.uri },
        );
        if (!Array.isArray(result)) return null;
        return result
          .filter((loc) => loc && loc.uri && loc.range)
          .map((loc) => {
            const targetModel = findModelByUri(loc.uri);
            if (!targetModel) return null;
            return { uri: targetModel.uri, range: toMonacoRange(loc.range, targetModel) };
          })
          .filter(Boolean);
      } catch {
        return null;
      }
    },
  });

  // Rename
  monaco.languages.registerRenameProvider(ST_LANGUAGE_ID, {
    async provideRenameEdits(model, position, newName) {
      const file = DEMO_FILES.find((f) => findModelByUri(f.uri) === model);
      if (!file || !wasmClient) return null;
      try {
        await syncAllDocuments();
        const result = await requestWithPositionFallback(
          model,
          position,
          (cursor) => wasmClient.rename(
            file.uri,
            cursor,
            newName,
            REQUEST_TIMEOUT_MS.rename,
          ),
          hasNonEmptyResult,
          { operation: "rename", fileUri: file.uri },
        );
        const edits = [];
        if (Array.isArray(result)) {
          for (const change of result) {
            if (!change || !change.uri || !change.range) continue;
            const targetModel = findModelByUri(change.uri);
            if (!targetModel) continue;
            edits.push({
              resource: targetModel.uri,
              textEdit: {
                range: toMonacoRange(change.range, targetModel),
                text: change.new_text || change.newText || newName,
              },
              versionId: undefined,
            });
          }
        } else if (result && result.changes && typeof result.changes === "object") {
          for (const [uri, changes] of Object.entries(result.changes)) {
            const targetModel = findModelByUri(uri);
            if (!targetModel) continue;
            for (const change of changes) {
              edits.push({
                resource: targetModel.uri,
                textEdit: {
                  range: toMonacoRange(change.range, targetModel),
                  text: change.new_text || change.newText || newName,
                },
                versionId: undefined,
              });
            }
          }
        }
        if (edits.length === 0) return null;
        return { edits };
      } catch {
        return null;
      }
    },
    async resolveRenameLocation(model, position) {
      const candidates = requestCandidates(model, position);
      for (const candidate of candidates) {
        const word =
          model.getWordAtPosition(candidate.monaco)
          || model.getWordAtPosition({
            lineNumber: candidate.monaco.lineNumber,
            column: Math.max(1, candidate.monaco.column - 1),
          });
        if (!word) continue;
        return {
          range: new monaco.Range(
            candidate.monaco.lineNumber,
            word.startColumn,
            candidate.monaco.lineNumber,
            word.endColumn,
          ),
          text: word.word,
        };
      }
      return {
        rejectReason: "Not a renamable symbol",
      };
    },
  });
}

// ── Document Highlights ──────────────────────────────

function scheduleDocumentHighlight() {
  if (documentHighlightTimer) {
    clearTimeout(documentHighlightTimer);
  }
  documentHighlightTimer = setTimeout(() => updateDocumentHighlights(), 150);
}

async function updateDocumentHighlights() {
  if (!wasmClient || !editor) return;
  const model = editor.getModel();
  if (!model) {
    documentHighlightDecorations = editor.deltaDecorations(documentHighlightDecorations, []);
    return;
  }
  const file = DEMO_FILES[activeFileIndex];
  if (!file) {
    documentHighlightDecorations = editor.deltaDecorations(documentHighlightDecorations, []);
    return;
  }
  const anchor = editor.getPosition();
  if (!anchor) {
    documentHighlightDecorations = editor.deltaDecorations(documentHighlightDecorations, []);
    return;
  }
  try {
    await syncAllDocuments();
    const highlights = await requestWithPositionFallback(
      model,
      anchor,
      (cursor) => wasmClient.documentHighlight(
        file.uri,
        cursor,
        REQUEST_TIMEOUT_MS.documentHighlight,
      ),
      (value) => Array.isArray(value) && value.length > 0,
      { operation: "documentHighlight", fileUri: file.uri },
    );
    if (!Array.isArray(highlights) || highlights.length === 0) {
      documentHighlightDecorations = editor.deltaDecorations(documentHighlightDecorations, []);
      return;
    }
    const decorations = highlights.map((h) => ({
      range: toMonacoRange(h.range, model),
      options: {
        className: h.kind === "write" ? "demo-highlight-write" : "demo-highlight-read",
        overviewRuler: { color: "#14b8a680", position: monaco.editor.OverviewRulerLane.Center },
      },
    }));
    documentHighlightDecorations = editor.deltaDecorations(documentHighlightDecorations, decorations);
  } catch {
    documentHighlightDecorations = editor.deltaDecorations(documentHighlightDecorations, []);
  }
}

// ── Editor Creation ──────────────────────────────────

function createModels() {
  models = DEMO_FILES.map((file) =>
    monaco.editor.createModel(file.content, ST_LANGUAGE_ID, monaco.Uri.parse(`file:///${file.uri}`))
  );
}

function createEditor() {
  editor = monaco.editor.create(dom.editorMount, {
    model: models[0],
    automaticLayout: true,
    minimap: { enabled: true, scale: 1, showSlider: "mouseover" },
    lineNumbers: "on",
    scrollBeyondLastLine: false,
    fontFamily: "JetBrains Mono, Fira Code, IBM Plex Mono, monospace",
    fontSize: 13,
    lineHeight: 20,
    tabSize: 2,
    insertSpaces: true,
    quickSuggestions: { other: true, comments: false, strings: true },
    quickSuggestionsDelay: 120,
    suggestOnTriggerCharacters: true,
    wordBasedSuggestions: "off",
    parameterHints: { enabled: true },
    snippetSuggestions: "inline",
    hover: { enabled: "on", delay: 550, sticky: true },
    definitionLinkOpensInPeek: false,
    gotoLocation: {
      multipleDefinitions: "goto",
      multipleReferences: "peek",
      multipleDeclarations: "peek",
      multipleImplementations: "peek",
      multipleTypeDefinitions: "peek",
    },
    occurrencesHighlight: "singleFile",
    selectionHighlight: true,
    bracketPairColorization: { enabled: true },
    smoothScrolling: true,
    renderLineHighlight: "all",
    padding: { top: 8, bottom: 8 },
    theme: "trust-dark",
  });

  if (editorOpenerDisposable) {
    try {
      editorOpenerDisposable.dispose();
    } catch {
      // Best effort.
    }
    editorOpenerDisposable = null;
  }
  editorOpenerDisposable = monaco.editor.registerEditorOpener({
    openCodeEditor(_source, resource, selectionOrPosition) {
      return openResourceInEditor(resource, selectionOrPosition);
    },
  });

  editor.onContextMenu((event) => {
    const position = event?.target?.position;
    if (!position) return;
    editor.setPosition(position);
  });

  editor.onMouseDown((event) => {
    const position = event?.target?.position;
    if (!position) return;
    if (event?.event?.rightButton || event?.event?.middleButton) {
      editor.setPosition(position);
    }
  });

  editor.onDidChangeCursorPosition(() => {
    updateCursorLabel();
    scheduleDocumentHighlight();
  });

  editor.onDidChangeModelContent(() => {
    scheduleDiagnosticsDebounced();
  });
}

function updateCursorLabel() {
  if (!editor) return;
  const pos = editor.getPosition();
  dom.cursorLabel.textContent = `Ln ${pos.lineNumber}, Col ${pos.column}`;
}

let diagDebounceTimer = null;
function scheduleDiagnosticsDebounced() {
  if (diagDebounceTimer) clearTimeout(diagDebounceTimer);
  diagDebounceTimer = setTimeout(async () => {
    await syncAllDocuments();
    await runDiagnosticsForAll();
  }, 400);
}

// ── File Tabs ────────────────────────────────────────

function renderTabs() {
  dom.fileTabs.innerHTML = "";
  DEMO_FILES.forEach((file, index) => {
    const btn = document.createElement("button");
    btn.className = `demo-tab${index === activeFileIndex ? " active" : ""}`;
    btn.setAttribute("role", "tab");
    btn.setAttribute("aria-selected", index === activeFileIndex ? "true" : "false");
    btn.innerHTML = `<svg class="file-icon" viewBox="0 0 16 16"><path d="M4 2h5l4 4v8a1 1 0 0 1-1 1H4a1 1 0 0 1-1-1V3a1 1 0 0 1 1-1z"/><path d="M9 2v4h4"/><path d="M6 10h4M6 12.5h2.5" stroke-width="1.2"/></svg>${file.name}`;
    btn.addEventListener("click", () => switchToFile(index));
    dom.fileTabs.appendChild(btn);
  });
}

function switchToFile(index) {
  if (index === activeFileIndex && editor) return;
  activeFileIndex = index;
  if (editor && models[index]) {
    editor.setModel(models[index]);
    editor.focus();
  }
  renderTabs();
  scheduleDiagnosticsDebounced();
}

// ── Walkthrough ──────────────────────────────────────

function renderWalkthrough() {
  dom.walkthroughSteps.innerHTML = "";
  WALKTHROUGH.forEach((step, index) => {
    const div = document.createElement("div");
    div.className = "walkthrough-step";
    div.innerHTML = `
      <div class="step-header">
        <span class="step-number">${index + 1}</span>
        <span class="step-title">${step.title}</span>
      </div>
      <p class="step-hint">${step.hint}</p>
    `;
    div.addEventListener("click", () => activateStep(index));
    dom.walkthroughSteps.appendChild(div);
  });
}

function activateStep(index) {
  const steps = dom.walkthroughSteps.querySelectorAll(".walkthrough-step");
  steps.forEach((s, i) => s.classList.toggle("active", i === index));
  const action = getWalkthroughAction(index);
  if (!action) return;

  switchToFile(action.fileIndex);
  if (editor && action.focus) {
    focusSymbolAt(action.focus);
  }
  if (editor && action.commandId) {
    setTimeout(() => {
      triggerEditorCommand(
        action.commandId,
        { retries: 4, retryDelayMs: 80 },
      );
    }, Math.max(0, Number(action.commandDelayMs || 0)));
  }
  if (editor) editor.focus();
}

function focusSymbolAt(position) {
  if (!editor) return;
  const model = editor.getModel();
  if (!model) return;
  const clampedLine = clamp(position?.lineNumber || 1, 1, model.getLineCount());
  const clampedColumn = clamp(position?.column || 1, 1, model.getLineMaxColumn(clampedLine));
  const word =
    model.getWordAtPosition({ lineNumber: clampedLine, column: clampedColumn })
    || model.getWordAtPosition({
      lineNumber: clampedLine,
      column: Math.max(1, clampedColumn - 1),
    })
    || model.getWordAtPosition({ lineNumber: clampedLine, column: clampedColumn + 1 });
  if (word && Number.isFinite(word.startColumn) && Number.isFinite(word.endColumn)) {
    const selection = new monaco.Selection(
      clampedLine,
      word.startColumn,
      clampedLine,
      word.endColumn,
    );
    editor.setSelection(selection);
    editor.revealRangeInCenter(selection);
  } else {
    editor.setPosition({ lineNumber: clampedLine, column: clampedColumn });
    editor.revealLineInCenter(clampedLine);
  }
  editor.focus();
}

async function triggerEditorCommand(commandId, options = {}) {
  if (!editor || typeof commandId !== "string") return false;
  const retries = Number.isFinite(options?.retries)
    ? Math.max(1, Math.trunc(options.retries))
    : 1;
  const retryDelayMs = Number.isFinite(options?.retryDelayMs)
    ? Math.max(0, Math.trunc(options.retryDelayMs))
    : 0;
  const args = options?.args ?? {};
  const sleep = (ms) => new Promise((resolve) => setTimeout(resolve, ms));

  try {
    for (let attempt = 0; attempt < retries; attempt += 1) {
      const action = typeof editor.getAction === "function" ? editor.getAction(commandId) : null;
      const supported = typeof action?.isSupported === "function" ? action.isSupported() : null;
      if (action && (supported == null || supported === true) && typeof action.run === "function") {
        await action.run(args);
        return true;
      }

      if (typeof editor.trigger === "function") {
        editor.trigger("demo", commandId, args);
        return true;
      }

      if (attempt + 1 < retries && retryDelayMs > 0) {
        await sleep(retryDelayMs);
      }
    }
    return false;
  } catch {
    return false;
  }
}

// ── Boot ─────────────────────────────────────────────

async function boot() {
  try {
    await loadMonaco();
    createModels();
    createEditor();
    renderTabs();
    renderWalkthrough();
    registerProviders();
    setStatus("Loading WASM analysis engine...", "loading");

    await loadWasmClient();
    await syncAllDocuments();
    await runDiagnosticsForAll();

    setStatus("Ready", "ready");
    dom.fileCount.textContent = `${DEMO_FILES.length} files loaded`;
  } catch (error) {
    console.error("[demo] boot failed:", error);
    setStatus("Failed: " + String(error.message || error), "error");
    setWasmBadge("Error", "err");
  }
}

boot();
