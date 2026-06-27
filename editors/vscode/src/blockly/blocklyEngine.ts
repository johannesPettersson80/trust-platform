/**
 * Blockly Code Generation Engine
 * Converts Blockly workspace blocks to IEC 61131-3 Structured Text (ST)
 */

export interface BlockDefinition {
  type: string;
  id: string;
  fields?: Record<string, any>;
  inputs?: Record<string, any>;
  next?: string | { block?: BlockDefinition } | null;
  x?: number;
  y?: number;
}

export interface BlocklyWorkspace {
  blocks: {
    languageVersion: number;
    blocks: BlockDefinition[];
  };
  variables?: Array<{
    name: string;
    type: string;
    id: string;
  }>;
  metadata?: {
    name: string;
    description?: string;
    version?: string;
  };
}

export interface GeneratedCode {
  structuredText: string;
  variables: Map<string, string>;
  errors: string[];
}

/**
 * Engine for generating ST code from Blockly workspace
 */
export class BlocklyEngine {
  private variables: Map<string, string> = new Map();
  private variableIds: Map<string, string> = new Map();
  private errors: string[] = [];
  private blockById: Map<string, BlockDefinition> = new Map();

  constructor() {}

  /**
   * Generate ST code from Blockly workspace
   */
  generateCode(workspace: BlocklyWorkspace): GeneratedCode {
    this.variables.clear();
    this.variableIds.clear();
    this.errors = [];
    this.blockById.clear();

    // Process variables
    if (workspace.variables) {
      for (const variable of workspace.variables) {
        this.variables.set(variable.name, variable.type || "");
        this.variableIds.set(variable.id, variable.name);
      }
    }

    if (workspace.blocks && workspace.blocks.blocks) {
      for (const block of workspace.blocks.blocks) {
        this.registerBlockTree(block);
      }

      for (const block of workspace.blocks.blocks) {
        this.inferVariableTypes(block);
      }
    }

    this.finalizeVariableTypes();

    // Generate variable declarations
    const varDeclarations = this.generateVariableDeclarations();

    // Generate program body
    const bodyLines: string[] = [];

    if (workspace.blocks && workspace.blocks.blocks) {
      for (const block of workspace.blocks.blocks) {
        bodyLines.push(...this.generateStatementChain(block));
      }
    }

    // Combine into complete ST program
    const programName = workspace.metadata?.name || "BlocklyProgram";
    const structuredText = this.assembleProgram(programName, varDeclarations, bodyLines);

    return {
      structuredText,
      variables: this.variables,
      errors: this.errors,
    };
  }

  /**
   * Generate ST code as a FUNCTION_BLOCK so visual logic can be mixed with
   * handwritten ST in one program.
   */
  generateFunctionBlockCode(
    workspace: BlocklyWorkspace,
    functionBlockName?: string
  ): GeneratedCode {
    const generated = this.generateCode(workspace);
    const defaultName = workspace.metadata?.name || "BlocklyProgram";
    const fbName = this.sanitizePouName(
      functionBlockName || `FB_${defaultName}_BLOCKLY`
    );
    const headerReplaced = generated.structuredText.replace(
      /^PROGRAM\s+[^\r\n]+/m,
      `FUNCTION_BLOCK ${fbName}`
    );
    const structuredText = headerReplaced.replace(
      /END_PROGRAM\s*$/m,
      "END_FUNCTION_BLOCK"
    );

    return {
      structuredText,
      variables: generated.variables,
      errors: generated.errors,
    };
  }

  private registerBlockTree(block: BlockDefinition | undefined): void {
    if (!block || this.blockById.has(block.id)) {
      return;
    }

    this.blockById.set(block.id, block);

    if (block.inputs) {
      for (const input of Object.values(block.inputs)) {
        const nestedBlock = this.resolveInputBlockValue(input);
        if (nestedBlock) {
          this.registerBlockTree(nestedBlock);
        }
      }
    }

    const nextBlock = this.resolveNextBlock(block.next);
    if (nextBlock) {
      this.registerBlockTree(nextBlock);
    }
  }

  private generateStatementChain(startBlock: BlockDefinition): string[] {
    const lines: string[] = [];
    const visited = new Set<string>();
    let current: BlockDefinition | undefined = startBlock;

    while (current) {
      if (visited.has(current.id)) {
        this.errors.push(`Detected cycle while generating block chain at: ${current.id}`);
        break;
      }
      visited.add(current.id);

      const blockCode = this.generateBlockCode(current);
      if (blockCode) {
        lines.push(blockCode);
      }

      current = this.resolveNextBlock(current.next);
    }

    return lines;
  }

  /**
   * Generate variable declarations section
   */
  private generateVariableDeclarations(): string {
    if (this.variables.size === 0) {
      return "";
    }

    const lines: string[] = ["VAR"];
    for (const [name, type] of this.variables) {
      lines.push(`  ${name} : ${type};`);
    }
    lines.push("END_VAR");
    
    return lines.join("\n");
  }

  /**
   * Generate code for a single block
   */
  private generateBlockCode(block: BlockDefinition): string {
    switch (block.type) {
      case "controls_if":
        return this.generateIfBlock(block);
      case "controls_whileUntil":
        return this.generateWhileUntilBlock(block);
      case "logic_compare":
        return this.generateCompareBlock(block);
      case "math_arithmetic":
        return this.generateArithmeticBlock(block);
      case "variables_set":
        return this.generateSetVariableBlock(block);
      case "variables_get":
        return this.generateGetVariableBlock(block);
      case "io_digital_write":
        return this.generateDigitalWriteBlock(block);
      case "io_digital_read":
        return this.generateDigitalReadBlock(block);
      case "logic_boolean":
        return this.generateBooleanBlock(block);
      case "text":
        return this.generateTextBlock(block);
      case "math_number":
        return this.generateNumberBlock(block);
      default:
        this.errors.push(`Unknown block type: ${block.type}`);
        return `(* Unknown block: ${block.type} *)`;
    }
  }

  /**
   * Generate IF statement
   */
  private generateIfBlock(block: BlockDefinition): string {
    const conditionBlock = this.resolveInputBlock(block, ["IF0", "IF"]);
    const condition = conditionBlock
      ? this.generateBlockCode(conditionBlock)
      : "FALSE";

    const doBlock = this.resolveInputBlock(block, ["DO0", "DO"]);
    const doStatements = doBlock ? this.generateStatementChain(doBlock) : [];

    const indent = "  ";
    const statements = doStatements.length
      ? `\n${doStatements
          .map((statement) => this.indentMultiline(statement, indent))
          .join("\n")}`
      : "";

    return `IF ${condition} THEN${statements}\nEND_IF;`;
  }

  /**
   * Generate a pre-tested loop from Blockly's while/until block.
   *
   * Blockly's UNTIL mode is also pre-tested, so ST WHILE with a negated
   * condition preserves the block semantics better than ST REPEAT.
   */
  private generateWhileUntilBlock(block: BlockDefinition): string {
    const conditionBlock = this.resolveInputBlock(block, ["BOOL"]);
    const rawCondition = conditionBlock
      ? this.generateBlockCode(conditionBlock)
      : "FALSE";
    const mode = String(block.fields?.["MODE"] || "WHILE").toUpperCase();
    const condition =
      mode === "UNTIL" ? `NOT (${rawCondition})` : rawCondition;

    const doBlock = this.resolveInputBlock(block, ["DO"]);
    const doStatements = doBlock ? this.generateStatementChain(doBlock) : [];

    const indent = "  ";
    const statements = doStatements.length
      ? `\n${doStatements
          .map((statement) => this.indentMultiline(statement, indent))
          .join("\n")}`
      : "";

    return `WHILE ${condition} DO${statements}\nEND_WHILE;`;
  }

  /**
   * Generate comparison operation
   */
  private generateCompareBlock(block: BlockDefinition): string {
    const op = block.fields?.["OP"] || "EQ";
    const left = block.inputs?.["A"]?.block 
      ? this.generateBlockCode(block.inputs["A"].block)
      : "0";
    const right = block.inputs?.["B"]?.block
      ? this.generateBlockCode(block.inputs["B"].block)
      : "0";

    const opMap: Record<string, string> = {
      "EQ": "=",
      "NEQ": "<>",
      "LT": "<",
      "LTE": "<=",
      "GT": ">",
      "GTE": ">=",
    };

    return `(${left} ${opMap[op] || "="} ${right})`;
  }

  /**
   * Generate arithmetic operation
   */
  private generateArithmeticBlock(block: BlockDefinition): string {
    const op = block.fields?.["OP"] || "ADD";
    const left = block.inputs?.["A"]?.block
      ? this.generateBlockCode(block.inputs["A"].block)
      : "0";
    const right = block.inputs?.["B"]?.block
      ? this.generateBlockCode(block.inputs["B"].block)
      : "0";

    const opMap: Record<string, string> = {
      "ADD": "+",
      "MINUS": "-",
      "MULTIPLY": "*",
      "DIVIDE": "/",
      "POWER": "**",
    };

    return `(${left} ${opMap[op] || "+"} ${right})`;
  }

  /**
   * Generate variable assignment
   */
  private generateSetVariableBlock(block: BlockDefinition): string {
    const varName = this.resolveVariableName(block.fields?.["VAR"]);
    const value = block.inputs?.["VALUE"]?.block
      ? this.generateBlockCode(block.inputs["VALUE"].block)
      : "0";

    return `${varName} := ${value};`;
  }

  /**
   * Generate a variable reference.
   */
  private generateGetVariableBlock(block: BlockDefinition): string {
    return this.resolveVariableName(block.fields?.["VAR"]);
  }

  /**
   * Generate digital output write
   */
  private generateDigitalWriteBlock(block: BlockDefinition): string {
    const address = block.fields?.["ADDRESS"] || "%QX0.0";
    const value = block.inputs?.["VALUE"]?.block
      ? this.generateBlockCode(block.inputs["VALUE"].block)
      : "FALSE";

    return `${address} := ${value};`;
  }

  /**
   * Generate digital input read
   */
  private generateDigitalReadBlock(block: BlockDefinition): string {
    const address = block.fields?.["ADDRESS"] || "%IX0.0";
    return address;
  }

  /**
   * Generate boolean constant
   */
  private generateBooleanBlock(block: BlockDefinition): string {
    const value = block.fields?.["BOOL"] || "TRUE";
    return value;
  }

  /**
   * Generate text constant
   */
  private generateTextBlock(block: BlockDefinition): string {
    const text = block.fields?.["TEXT"] || "";
    return `'${text.replace(/'/g, "''")}'`;
  }

  /**
   * Generate number constant
   */
  private generateNumberBlock(block: BlockDefinition): string {
    const num = block.fields?.["NUM"] || "0";
    return String(num);
  }

  private resolveVariableName(varField: unknown): string {
    if (typeof varField === "string" && varField.trim()) {
      return varField;
    }

    if (!varField || typeof varField !== "object") {
      return "temp";
    }

    const candidate = varField as { id?: unknown; name?: unknown };
    if (typeof candidate.name === "string" && candidate.name.trim()) {
      return candidate.name;
    }

    if (typeof candidate.id === "string" && candidate.id.trim()) {
      return this.variableIds.get(candidate.id) || this.sanitizeIdentifier(candidate.id);
    }

    return "temp";
  }

  private inferVariableTypes(block: BlockDefinition | undefined): void {
    if (!block) {
      return;
    }

    if (block.type === "variables_set") {
      const varName = this.resolveVariableName(block.fields?.["VAR"]);
      const valueBlock = this.resolveInputBlock(block, ["VALUE"]);
      const inferredType = this.inferExpressionType(valueBlock);
      if (inferredType && !this.variables.get(varName)) {
        this.variables.set(varName, inferredType);
      }
    }

    if (block.inputs) {
      for (const input of Object.values(block.inputs)) {
        this.inferVariableTypes(this.resolveInputBlockValue(input));
      }
    }

    this.inferVariableTypes(this.resolveNextBlock(block.next));
  }

  private inferExpressionType(block: BlockDefinition | undefined): string | undefined {
    if (!block) {
      return undefined;
    }

    switch (block.type) {
      case "logic_boolean":
      case "logic_compare":
      case "io_digital_read":
        return "BOOL";
      case "math_number": {
        const value = String(block.fields?.["NUM"] ?? "0");
        return value.includes(".") ? "REAL" : "INT";
      }
      case "math_arithmetic": {
        const leftType = this.inferExpressionType(this.resolveInputBlock(block, ["A"]));
        const rightType = this.inferExpressionType(this.resolveInputBlock(block, ["B"]));
        return leftType === "REAL" || rightType === "REAL" ? "REAL" : "INT";
      }
      case "variables_get":
        return this.variables.get(this.resolveVariableName(block.fields?.["VAR"])) || undefined;
      default:
        return undefined;
    }
  }

  private finalizeVariableTypes(): void {
    for (const [name, type] of this.variables) {
      if (!type) {
        this.variables.set(name, "BOOL");
      }
    }
  }

  private resolveInputBlock(
    block: BlockDefinition,
    inputNames: string[]
  ): BlockDefinition | undefined {
    if (!block.inputs) {
      return undefined;
    }

    for (const inputName of inputNames) {
      const resolved = this.resolveInputBlockValue(block.inputs[inputName]);
      if (resolved) {
        return resolved;
      }
    }

    return undefined;
  }

  private resolveInputBlockValue(inputValue: unknown): BlockDefinition | undefined {
    if (!inputValue || typeof inputValue !== "object") {
      return undefined;
    }

    if (!("block" in inputValue)) {
      return undefined;
    }

    return this.asBlock((inputValue as { block?: unknown }).block);
  }

  private resolveNextBlock(nextRef: BlockDefinition["next"]): BlockDefinition | undefined {
    if (!nextRef) {
      return undefined;
    }

    if (typeof nextRef === "string") {
      return this.blockById.get(nextRef);
    }

    return this.asBlock(nextRef.block);
  }

  private asBlock(value: unknown): BlockDefinition | undefined {
    if (!value || typeof value !== "object") {
      return undefined;
    }

    const candidate = value as { id?: unknown; type?: unknown };
    if (typeof candidate.id !== "string" || typeof candidate.type !== "string") {
      return undefined;
    }

    return value as BlockDefinition;
  }

  private indentMultiline(code: string, indent: string): string {
    return code
      .split("\n")
      .map((line) => `${indent}${line}`)
      .join("\n");
  }

  private sanitizePouName(raw: string): string {
    const normalized = raw
      .trim()
      .replace(/[^A-Za-z0-9_]/g, "_")
      .replace(/_+/g, "_")
      .replace(/^_+/, "")
      .replace(/_+$/, "");
    if (!normalized) {
      return "FB_BlocklyGenerated";
    }
    if (/^[0-9]/.test(normalized)) {
      return `_${normalized}`;
    }
    return normalized;
  }

  private sanitizeIdentifier(raw: string): string {
    const normalized = raw
      .trim()
      .replace(/[^A-Za-z0-9_]/g, "_")
      .replace(/_+/g, "_")
      .replace(/^_+/, "")
      .replace(/_+$/, "");
    if (!normalized) {
      return "temp";
    }
    if (/^[0-9]/.test(normalized)) {
      return `_${normalized}`;
    }
    return normalized;
  }

  /**
   * Assemble complete ST program
   */
  private assembleProgram(name: string, varDeclarations: string, bodyLines: string[]): string {
    const sections: string[] = [
      `PROGRAM ${name}`,
      "",
    ];

    if (varDeclarations) {
      sections.push(varDeclarations);
      sections.push("");
    }

    sections.push("(* Generated from Blockly *)");
    sections.push("");
    sections.push(...bodyLines);
    sections.push("");
    sections.push("END_PROGRAM");

    return sections.join("\n");
  }
}
