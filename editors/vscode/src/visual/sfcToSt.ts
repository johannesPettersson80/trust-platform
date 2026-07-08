import { SfcEngine, type SfcWorkspace } from "../sfc/sfcEngine";
import { rewriteDirectIoInFunctionBlock } from "./directIo";
import { fbNameForSource } from "./stNaming";

function isSfcWorkspace(value: unknown): value is SfcWorkspace {
  if (!value || typeof value !== "object") {
    return false;
  }

  const workspace = value as Partial<SfcWorkspace>;
  return (
    typeof workspace.name === "string" &&
    Array.isArray(workspace.steps) &&
    Array.isArray(workspace.transitions)
  );
}

export function parseSfcWorkspaceText(content: string): SfcWorkspace {
  const parsed = JSON.parse(content);
  if (!isSfcWorkspace(parsed)) {
    throw new Error(
      "Invalid SFC workspace format. Expected JSON object with name, steps, and transitions."
    );
  }
  return parsed;
}

function convertProgramToFunctionBlock(
  programSource: string,
  functionBlockName: string
): string {
  const lines = programSource.split(/\r?\n/);
  const programHeaderIndex = lines.findIndex((line) => /^\s*PROGRAM\b/i.test(line));
  if (programHeaderIndex < 0) {
    throw new Error("Generated SFC Structured Text does not contain a PROGRAM header.");
  }
  lines[programHeaderIndex] = `FUNCTION_BLOCK ${functionBlockName}`;

  let endProgramIndex = -1;
  for (let index = lines.length - 1; index >= 0; index -= 1) {
    if (/^\s*END_PROGRAM\b/i.test(lines[index])) {
      endProgramIndex = index;
      break;
    }
  }
  if (endProgramIndex < 0) {
    throw new Error("Generated SFC Structured Text does not contain END_PROGRAM.");
  }
  lines[endProgramIndex] = "END_FUNCTION_BLOCK";

  return lines.join("\n");
}

export function generateSfcCompanionFunctionBlock(
  workspace: SfcWorkspace,
  baseName: string
): string {
  const normalizedName = workspace.name.trim() || baseName || "SFC";
  const engine = new SfcEngine({
    ...workspace,
    name: normalizedName,
  });
  const programSource = engine.generateStructuredText();
  const functionBlockName = fbNameForSource(baseName || normalizedName, "SFC");
  return rewriteDirectIoInFunctionBlock(
    convertProgramToFunctionBlock(programSource, functionBlockName)
  ).source;
}
