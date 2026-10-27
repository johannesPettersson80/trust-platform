import { useState, useEffect, useCallback } from "react";
import {
  BlocklyWorkspace,
  ExtensionToWebviewMessage,
} from "../types";
import { getVsCodeApi } from "../../../visual/runtime/webview/vscodeApi";

const vscode = getVsCodeApi();

export interface UseBlocklyReturn {
  workspace: BlocklyWorkspace | null;
  generatedCode: string | null;
  errors: string[];
  parseError: string | null;
  saveWorkspace: (workspace: BlocklyWorkspace) => void;
  validateWorkspace: () => void;
  generateCode: () => void;
  openAsText: () => void;
}

export function useBlockly(): UseBlocklyReturn {
  const [workspace, setWorkspace] = useState<BlocklyWorkspace | null>(null);
  const [generatedCode, setGeneratedCode] = useState<string | null>(null);
  const [errors, setErrors] = useState<string[]>([]);
  const [parseError, setParseError] = useState<string | null>(null);

  // Handle messages from extension
  useEffect(() => {
    const messageHandler = (event: MessageEvent<ExtensionToWebviewMessage>) => {
      const message = event.data;
      console.log('[useBlockly] Received message:', message.type, message);

      switch (message.type) {
        case "update":
          try {
            const parsed = JSON.parse(message.content);
            setWorkspace(parsed);
            setParseError(null);
          } catch (error) {
            const detail = error instanceof Error ? error.message : String(error);
            console.error("Failed to parse workspace:", detail);
            setParseError(detail);
            vscode.postMessage({
              type: "error",
              error: "Could not open this Blockly program because the file is not valid JSON.",
            });
          }
          break;

        case "codeGenerated":
          setGeneratedCode(message.code);
          setErrors(message.errors || []);
          break;
      }
    };

    window.addEventListener("message", messageHandler);

    // Notify extension that webview is ready
    vscode.postMessage({ type: "ready" });

    return () => {
      window.removeEventListener("message", messageHandler);
    };
  }, []);

  const saveWorkspace = useCallback((workspace: BlocklyWorkspace) => {
    const content = JSON.stringify(workspace, null, 2);
    vscode.postMessage({
      type: "save",
      content,
    });
    setWorkspace(workspace);
  }, []);

  const generateCode = useCallback(() => {
    vscode.postMessage({ type: "generateCode" });
  }, []);

  const validateWorkspace = useCallback(() => {
    vscode.postMessage({ type: "validate" });
  }, []);

  const openAsText = useCallback(() => {
    vscode.postMessage({ type: "openAsText" });
  }, []);

  return {
    workspace,
    generatedCode,
    errors,
    parseError,
    saveWorkspace,
    validateWorkspace,
    generateCode,
    openAsText,
  };
}
