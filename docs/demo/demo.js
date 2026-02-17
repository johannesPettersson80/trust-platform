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
    hint: "The project starts clean. Introduce a temporary typo (for example <kbd>Cmd.Enabl</kbd>) to see WASM squiggles and diagnostics update live.",
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
    hint: "<kbd>Ctrl+Left-click</kbd> on <kbd>E_PumpState</kbd> in fb_pump.st to jump to its definition in types.st.",
  },
  {
    title: "Find References",
    hint: "Right-click on <kbd>Enable</kbd> and select <em>Go to References</em> (or press <kbd>Shift+F12</kbd>) to see every usage across files.",
  },
  {
    title: "Document Highlights",
    hint: "Click on any variable name &mdash; all occurrences in the current file highlight instantly.",
  },
  {
    title: "Rename",
    hint: "Press <kbd>F2</kbd> (or <kbd>Fn+F2</kbd> on laptops) on <kbd>ActualSpeed</kbd> to rename it across all files in the project.",
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

function requestPositions(model, position) {
  const points = [];
  const seen = new Set();
  const lineNumber = Number(position.lineNumber || 1);
  const maxColumn = model.getLineMaxColumn(lineNumber);
  const push = (column) => {
    const clampedColumn = clamp(Number(column || 1), 1, maxColumn);
    const key = `${lineNumber}:${clampedColumn}`;
    if (seen.has(key)) return;
    seen.add(key);
    points.push({ lineNumber, column: clampedColumn });
  };

  push(position.column);
  push(position.column - 1);
  push(position.column - 2);

  const word =
    model.getWordAtPosition(position) ||
    model.getWordAtPosition({ lineNumber, column: Math.max(1, position.column - 1) });
  if (word) {
    push(word.startColumn);
    push(word.endColumn - 1);
  }

  return points
    .map((p) => fromMonacoPosition(p))
    .filter((p) => p.character >= 0);
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

async function requestWithPositionFallback(model, position, query, hasResult = hasNonEmptyResult) {
  let lastError = null;
  for (const candidate of requestPositions(model, position)) {
    try {
      const result = await query(candidate);
      if (hasResult(result)) {
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
  wasmClient.onStatus((status) => {
    if (status.type === "ready") {
      setWasmBadge("WASM Ready", "ok");
      setStatus("WASM analysis engine ready", "ready");
    } else if (status.type === "fatal") {
      setWasmBadge("WASM Error", "err");
      setStatus("WASM error: " + status.error, "error");
    }
  });
  await wasmClient.ready();
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
      const word = model.getWordAtPosition(position);
      if (!word) return { rejectReason: "Not a renamable symbol" };
      return {
        range: new monaco.Range(position.lineNumber, word.startColumn, position.lineNumber, word.endColumn),
        text: word.word,
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
  const position = fromMonacoPosition(editor.getPosition());
  try {
    await syncAllDocuments();
    const highlights = await wasmClient.documentHighlight(
      file.uri,
      position,
      REQUEST_TIMEOUT_MS.documentHighlight,
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
    hover: { enabled: "on", delay: 250, sticky: true },
    occurrencesHighlight: "singleFile",
    selectionHighlight: true,
    bracketPairColorization: { enabled: true },
    smoothScrolling: true,
    renderLineHighlight: "all",
    padding: { top: 8, bottom: 8 },
    theme: "trust-dark",
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

  // Navigate to relevant file for the step
  switch (index) {
    case 0: // Diagnostics - show program.st
      switchToFile(2);
      break;
    case 1: // Hover - show fb_pump.st, position on FB_Pump
      switchToFile(1);
      if (editor) editor.setPosition({ lineNumber: 1, column: 16 });
      break;
    case 2: // Completion - show fb_pump.st, position after Status.
      switchToFile(1);
      if (editor) {
        editor.setPosition({ lineNumber: 17, column: 8 });
        setTimeout(() => {
          if (editor) {
            editor.trigger("demo", "editor.action.triggerSuggest", {});
          }
        }, 60);
      }
      break;
    case 3: // Definition - show fb_pump.st, position on E_PumpState
      switchToFile(1);
      if (editor) {
        editor.setPosition({ lineNumber: 21, column: 22 });
        editor.revealLineInCenter(21);
      }
      break;
    case 4: // References - show types.st, position on Enable
      switchToFile(0);
      if (editor) {
        editor.setPosition({ lineNumber: 6, column: 9 });
        editor.revealLineInCenter(6);
      }
      break;
    case 5: // Highlights - show fb_pump.st, position on ramp
      switchToFile(1);
      if (editor) {
        editor.setPosition({ lineNumber: 10, column: 5 });
        editor.revealLineInCenter(10);
      }
      break;
    case 6: // Rename - show types.st, position on ActualSpeed
      switchToFile(0);
      if (editor) {
        editor.setPosition({ lineNumber: 14, column: 9 });
        editor.revealLineInCenter(14);
      }
      break;
  }
  if (editor) editor.focus();
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
