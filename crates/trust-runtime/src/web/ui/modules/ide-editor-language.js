// ── Monaco / Editor ────────────────────────────────────

function monacoLanguageForPath(path) {
  const normalized = String(path || "").toLowerCase();
  if (normalized.endsWith(".st")) return ST_LANGUAGE_ID;
  if (normalized.endsWith(".json")) return "json";
  if (normalized.endsWith(".toml")) return "ini";
  if (normalized.endsWith(".md")) return "markdown";
  if (normalized.endsWith(".yaml") || normalized.endsWith(".yml")) return "yaml";
  if (normalized.endsWith(".xml")) return "xml";
  if (normalized.endsWith(".js")) return "javascript";
  if (normalized.endsWith(".ts")) return "typescript";
  if (normalized.endsWith(".css")) return "css";
  if (normalized.endsWith(".html")) return "html";
  return "plaintext";
}

function activeModel() {
  return state.editorView ? state.editorView.getModel() : null;
}

function fromMonacoPosition(position) {
  if (!position) {
    return {line: 0, character: 0};
  }
  return {
    line: Math.max(0, Number(position.lineNumber || 1) - 1),
    character: Math.max(0, Number(position.column || 1) - 1),
  };
}

function toMonacoPosition(position, model) {
  const safeModel = model || activeModel();
  const maxLines = safeModel ? safeModel.getLineCount() : 1;
  const line = clamp(Number(position?.line ?? 0) + 1, 1, Math.max(1, maxLines));
  const maxColumn = safeModel ? safeModel.getLineMaxColumn(line) : 1;
  const column = clamp(Number(position?.character ?? 0) + 1, 1, Math.max(1, maxColumn));
  return new monaco.Position(line, column);
}

function toMonacoRange(range, model) {
  const safeModel = model || activeModel();
  const start = toMonacoPosition(range?.start || {line: 0, character: 0}, safeModel);
  const end = toMonacoPosition(range?.end || range?.start || {line: 0, character: 1}, safeModel);
  return new monaco.Range(
    start.lineNumber,
    start.column,
    Math.max(start.lineNumber, end.lineNumber),
    end.lineNumber < start.lineNumber ? start.column : Math.max(start.column, end.column),
  );
}

function positionToContentOffset(content, position) {
  const targetLine = Number(position?.line ?? 0);
  const targetChar = Number(position?.character ?? 0);
  let line = 0;
  let character = 0;
  for (let i = 0; i < content.length; i++) {
    if (line === targetLine && character === targetChar) {
      return i;
    }
    if (content[i] === "\n") {
      if (line === targetLine) {
        return i;
      }
      line++;
      character = 0;
    } else {
      character++;
    }
  }
  if (line === targetLine) {
    return content.length;
  }
  return null;
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

function extractLocalCompletionCandidates(model) {
  if (!model) {
    return [];
  }
  const text = model.getValue();
  const identifiers = new Set();
  const matches = text.matchAll(/[A-Za-z_][A-Za-z0-9_]*/g);
  for (const match of matches) {
    if (match && match[0]) {
      identifiers.add(match[0]);
    }
  }
  const stKeywords = [
    "PROGRAM", "END_PROGRAM", "FUNCTION", "END_FUNCTION", "FUNCTION_BLOCK",
    "END_FUNCTION_BLOCK", "VAR", "END_VAR", "VAR_INPUT", "VAR_OUTPUT",
    "VAR_IN_OUT", "VAR_GLOBAL", "IF", "THEN", "ELSE", "ELSIF", "END_IF",
    "CASE", "OF", "END_CASE", "FOR", "TO", "BY", "DO", "END_FOR",
    "WHILE", "END_WHILE", "REPEAT", "UNTIL", "END_REPEAT", "TRUE", "FALSE",
    "BOOL", "INT", "DINT", "UINT", "UDINT", "REAL", "LREAL", "STRING",
  ];
  for (const keyword of stKeywords) {
    identifiers.add(keyword);
  }
  return Array.from(identifiers).sort((a, b) => a.localeCompare(b));
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

function buildLocalCompletionSuggestions(model, position, limit = 120) {
  const range = fallbackCompletionRange(model, position);
  return extractLocalCompletionCandidates(model)
    .slice(0, limit)
    .map((label) => ({
      label,
      kind: /^[A-Z_]+$/.test(label)
        ? monaco.languages.CompletionItemKind.Keyword
        : monaco.languages.CompletionItemKind.Variable,
      detail: "local symbol",
      insertText: label,
      range,
    }));
}

function normalizeHoverContentValue(contents) {
  if (typeof contents === "string") {
    return contents.trim();
  }
  if (Array.isArray(contents)) {
    const parts = contents
      .map((entry) => {
        if (typeof entry === "string") {
          return entry.trim();
        }
        if (entry && typeof entry.value === "string") {
          return entry.value.trim();
        }
        return "";
      })
      .filter((value) => value.length > 0);
    return parts.join("\n\n").trim();
  }
  if (contents && typeof contents === "object" && typeof contents.value === "string") {
    return contents.value.trim();
  }
  return "";
}

function buildFallbackHover(model, position) {
  const word = model.getWordAtPosition(position);
  if (!word || !word.word) {
    return null;
  }
  return {
    range: new monaco.Range(
      position.lineNumber,
      word.startColumn,
      position.lineNumber,
      word.endColumn,
    ),
    contents: [{value: `\`\`\`st\n${word.word}\n\`\`\``}],
  };
}

function defineMonacoThemes() {
  monaco.editor.defineTheme("trust-light", {
    base: "vs",
    inherit: true,
    rules: [
      {token: "keyword.st", foreground: "0f766e", fontStyle: "bold"},
      {token: "number.st", foreground: "875f00"},
    ],
    colors: {
      "editor.background": "#ffffff",
      "editorCursor.foreground": "#0f766e",
      "editorLineNumber.foreground": "#7e8aa1",
      "editorLineNumber.activeForeground": "#213047",
      "editorGutter.background": "#f6f3ee",
      "editor.selectionBackground": "#0f766e22",
      "editor.inactiveSelectionBackground": "#0f766e11",
      "editor.wordHighlightBackground": "#0f766e30",
      "editor.wordHighlightStrongBackground": "#0f766e45",
      "editor.selectionHighlightBackground": "#0f766e20",
      "editor.selectionHighlightBorder": "#0f766e50",
      "editorWidget.background": "#f4f2ef",
      "editorWidget.foreground": "#1b1a18",
      "editorWidget.border": "#c8d8d4",
      "editorHoverWidget.background": "#f4f2ef",
      "editorHoverWidget.foreground": "#1b1a18",
      "editorHoverWidget.border": "#c8d8d4",
      "editorSuggestWidget.background": "#f4f2ef",
      "editorSuggestWidget.foreground": "#1b1a18",
      "editorSuggestWidget.border": "#c8d8d4",
      "editorSuggestWidget.selectedBackground": "#d9ece8",
      "editorSuggestWidget.highlightForeground": "#0f766e",
    },
  });
  monaco.editor.defineTheme("trust-dark", {
    base: "vs-dark",
    inherit: true,
    rules: [
      {token: "keyword.st", foreground: "14b8a6", fontStyle: "bold"},
      {token: "number.st", foreground: "e0c95a"},
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

function configureMonacoLanguageSupport() {
  if (!monaco) {
    return;
  }

  if (!monaco.languages.getLanguages().some((language) => language.id === ST_LANGUAGE_ID)) {
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
        "INT", "DINT", "UINT", "UDINT", "REAL", "LREAL", "STRING",
      ],
      operators: [":=", "=", "<>", "<=", ">=", "<", ">", "+", "-", "*", "/", "AND", "OR", "NOT"],
      tokenizer: {
        root: [
          [/[A-Za-z_][A-Za-z0-9_]*/, {
            cases: {
              "@keywords": "keyword.st",
              "@default": "identifier",
            },
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
      comments: {
        lineComment: "//",
        blockComment: ["(*", "*)"],
      },
      brackets: [
        ["(", ")"],
        ["[", "]"],
      ],
    });
  }

  defineMonacoThemes();

  completionProviderDisposable?.dispose();
  hoverProviderDisposable?.dispose();

  const triggerCharacters = [
    "_", ".", ...Array.from("abcdefghijklmnopqrstuvwxyzABCDEFGHIJKLMNOPQRSTUVWXYZ"),
  ];

  completionProviderDisposable = monaco.languages.registerCompletionItemProvider(ST_LANGUAGE_ID, {
    triggerCharacters,
    async provideCompletionItems(model, position) {
      if (!state.editorView || model !== state.editorView.getModel()) {
        return {suggestions: []};
      }
      const tab = activeTab();
      if (!tab || !isStructuredTextPath(tab.path)) {
        return {suggestions: []};
      }
      const cursor = fromMonacoPosition(position);
      const localSuggestions = buildLocalCompletionSuggestions(model, position);
      try {
        const items = await fetchCompletion(cursor, 80);
        if (!Array.isArray(items) || items.length === 0) {
          return {suggestions: localSuggestions};
        }
        const fallbackRange = fallbackCompletionRange(model, position);
        const suggestions = items
          .filter((item) => item && typeof item.label === "string" && item.label.length > 0)
          .map((item) => {
            let range = fallbackRange;
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
              documentation: item.documentation ? {value: String(item.documentation)} : undefined,
              insertText: item.text_edit?.new_text || item.insert_text || item.label,
              range,
              sortText,
              filterText: item.filter_text || undefined,
            };
          });
        if (suggestions.length === 0) {
          return {suggestions: localSuggestions};
        }
        return {suggestions};
      } catch (error) {
        console.warn("[ide] completion failed:", error);
        return {suggestions: localSuggestions};
      }
    },
  });

  hoverProviderDisposable = monaco.languages.registerHoverProvider(ST_LANGUAGE_ID, {
    async provideHover(model, position) {
      if (!state.editorView || model !== state.editorView.getModel()) {
        return null;
      }
      const tab = activeTab();
      if (!tab || !isStructuredTextPath(tab.path)) {
        return null;
      }
      try {
        const response = await fetchHover(fromMonacoPosition(position));
        if (!response || !response.contents) {
          return buildFallbackHover(model, position);
        }
        const hoverText = normalizeHoverContentValue(response.contents);
        if (!hoverText) {
          return buildFallbackHover(model, position);
        }
        const hover = {
          contents: [{value: hoverText}],
        };
        if (response.range) {
          hover.range = toMonacoRange(response.range, model);
        }
        return hover;
      } catch (err) {
        console.warn("[ide] hover failed:", err);
        return buildFallbackHover(model, position);
      }
    },
  });

}
