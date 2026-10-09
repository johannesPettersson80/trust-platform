import Ajv from "ajv";
import * as fs from "fs";
import * as path from "path";

interface ToolDefinition {
  name: string;
  inputSchema: { properties: { input: object } };
}
const ajv = new Ajv({
  allErrors: true,
  jsonPointers: true,
  coerceTypes: false,
  useDefaults: false,
  removeAdditional: false,
});
let validators: Map<string, Ajv.ValidateFunction> | undefined;
let editValidators: Map<string, Ajv.ValidateFunction>;

/** The published tool contract is also the executable input contract. Neither transport may
 * silently ignore unsupported fields or coerce a malformed mutation into a different action. */
export function validateToolInput(name: string, input: unknown): void {
  if (!validators) {
    const manifest = JSON.parse(fs.readFileSync(path.resolve(__dirname, "../../../package.json"), "utf8")) as {
      contributes: { languageModelTools: ToolDefinition[] };
    };
    validators = new Map(
      manifest.contributes.languageModelTools
        .filter((tool) => tool.name.startsWith("hmi_"))
        .map((tool) => [tool.name, ajv.compile(tool.inputSchema.properties.input)]),
    );
    const editSchema = manifest.contributes.languageModelTools.find((tool) => tool.name === "hmi_edit_draft")!
      .inputSchema.properties.input as {
      properties: { operations: { items: { oneOf: Array<{ properties: { op: { const: string } } }> } } };
    };
    editValidators = new Map(
      editSchema.properties.operations.items.oneOf.map((schema) => [schema.properties.op.const, ajv.compile(schema)]),
    );
  }
  const validate = validators.get(name);
  if (!validate) throw new Error("INVALID_ARGUMENT: unknown authoring tool.");
  if (name === "hmi_edit_draft" && input && typeof input === "object") {
    const operations = (input as { operations?: unknown }).operations;
    if (Array.isArray(operations))
      for (const [index, operation] of operations.entries()) {
        if (!operation || typeof operation !== "object" || typeof operation.op !== "string")
          throw new Error(`INVALID_ARGUMENT: input.operations[${index}] needs an operation object with a string op.`);
        const selected = editValidators.get(operation.op);
        if (!selected)
          throw new Error(`INVALID_ARGUMENT: input.operations[${index}].op is unsupported: ${operation.op}.`);
        if (!selected(operation))
          throw new Error(
            `INVALID_ARGUMENT: ${name}: ${ajv.errorsText(selected.errors, { dataVar: `input.operations[${index}]` })}`,
          );
      }
  }
  if (!validate(input))
    throw new Error(`INVALID_ARGUMENT: ${name}: ${ajv.errorsText(validate.errors, { dataVar: "input" })}`);
}
