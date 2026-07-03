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
  executeBlock: (blockId: string) => void;
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

        case "executionStarted":
          setGeneratedCode(message.code);
          break;

        case "executionStopped":
          break;

        case "blockExecuted":
          // Handle block execution feedback
          console.log("Block executed:", message.blockId);
          break;

        case "highlightBlock":
          console.log(`[useBlockly] Highlighting block: ${message.blockId}`);
          // This will be handled by Blockly workspace directly
          // We need to pass this to the workspace ref
          if ((window as any).blocklyWorkspace) {
            console.log(`[useBlockly] Workspace found, highlighting ${message.blockId}`);
            (window as any).blocklyWorkspace.highlightBlock(message.blockId);
          } else {
            console.warn('[useBlockly] Blockly workspace not found on window');
          }
          break;

        case "unhighlightBlock":
          console.log('[useBlockly] Unhighlighting all blocks');
          if ((window as any).blocklyWorkspace) {
            (window as any).blocklyWorkspace.highlightBlock(null);
          }
          break;

        case "runtime.error":
          console.error("[Blockly runtime error]", message.message);
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

  const executeBlock = useCallback((blockId: string) => {
    vscode.postMessage({
      type: "executeBlock",
      blockId,
    });
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
    executeBlock,
    openAsText,
  };
}
