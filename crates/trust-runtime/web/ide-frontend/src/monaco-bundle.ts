import * as monaco from "monaco-editor/editor/editor.api";

// Editor contributions required by ide.js
import "monaco-editor/editor/contrib/hover/browser/hoverContribution";
import "monaco-editor/editor/contrib/suggest/browser/suggestController";
import "monaco-editor/editor/contrib/suggest/browser/suggestInlineCompletions";
import "monaco-editor/editor/contrib/parameterHints/browser/parameterHints";
import "monaco-editor/editor/contrib/bracketMatching/browser/bracketMatching";
import "monaco-editor/editor/contrib/find/browser/findController";
import "monaco-editor/editor/contrib/contextmenu/browser/contextmenu";
import "monaco-editor/editor/contrib/folding/browser/folding";
import "monaco-editor/editor/contrib/snippet/browser/snippetController2";
import "monaco-editor/editor/contrib/wordOperations/browser/wordOperations";
import "monaco-editor/editor/contrib/comment/browser/comment";
import "monaco-editor/editor/contrib/gotoSymbol/browser/goToCommands";
import "monaco-editor/editor/contrib/gotoSymbol/browser/link/goToDefinitionAtPosition";
import "monaco-editor/editor/contrib/rename/browser/rename";
import "monaco-editor/editor/standalone/browser/referenceSearch/standaloneReferenceSearch";

import editorCss from "monaco-editor/editor/editor.main.css?inline";

let styleInjected = false;

function ensureStyleInjected(): void {
  if (styleInjected || typeof document === "undefined") {
    return;
  }
  const style = document.createElement("style");
  style.setAttribute("data-trust-monaco", "true");
  style.textContent = editorCss;
  document.head.appendChild(style);
  styleInjected = true;
}

export { monaco, ensureStyleInjected };
