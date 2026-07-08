import {
  BlocklyEngine,
  type BlocklyWorkspace,
} from "../blockly/blocklyEngine";
import { rewriteDirectIoInFunctionBlock } from "./directIo";
import { fbNameForSource } from "./stNaming";

function isBlocklyWorkspace(value: unknown): value is BlocklyWorkspace {
  if (!value || typeof value !== "object") {
    return false;
  }
  const workspace = value as Partial<BlocklyWorkspace>;
  return !!workspace.blocks && Array.isArray(workspace.blocks.blocks);
}

export function parseBlocklyWorkspaceText(content: string): BlocklyWorkspace {
  const parsed = JSON.parse(content);
  if (!isBlocklyWorkspace(parsed)) {
    throw new Error(
      "Invalid Blockly workspace format. Expected object with blocks.blocks[]"
    );
  }
  return parsed;
}

export function generateBlocklyCompanionFunctionBlock(
  workspace: BlocklyWorkspace,
  baseName: string
): string {
  const engine = new BlocklyEngine();
  const generated = engine.generateFunctionBlockCode(
    workspace,
    fbNameForSource(baseName || workspace.metadata?.name || "Blockly", "BLOCKLY")
  );

  const warnings =
    generated.errors.length > 0
      ? [
          "(*",
          `  Blockly conversion warnings (${generated.errors.length}):`,
          ...generated.errors.map((error) => `  - ${error}`),
          "*)",
          "",
        ]
      : [];

  const rewritten = rewriteDirectIoInFunctionBlock(generated.structuredText);
  return [...warnings, rewritten.source].join("\n");
}
