import * as assert from "assert";
import * as fs from "fs";
import * as path from "path";

import { SYMBOL_GEOMETRY } from "../../hmiBuilder/drawing/symbols/geometry";
import { SYMBOLS } from "../../hmiBuilder/drawing/symbols";

// Specification 39, §6 item 1 and §14.2: the drawing and the language server meet the same ports. The shipped symbols' geometry is
// the runtime's table (crates/trust-runtime/src/hmi/symbol_geometry.json); the drawing reads the module generated from it.

interface Table {
  labelHeight: number;
  shapes: Record<
    string,
    { size: [number, number]; label?: string; ports: Array<{ name: string; at: [number, number]; direction: string }> }
  >;
  symbols: Record<string, string>;
}

const TABLE: Table = JSON.parse(
  fs.readFileSync(path.resolve(__dirname, "../../../../../crates/trust-runtime/src/hmi/symbol_geometry.json"), "utf8"),
);

suite("HMI Builder symbol geometry", () => {
  test("the drawing's geometry module is the runtime's table (npm run generate:symbol-geometry)", () => {
    const expected = Object.fromEntries(
      Object.entries(TABLE.symbols).map(([type, shape]) => {
        const geometry = TABLE.shapes[shape];
        return [
          type,
          {
            size: geometry.size,
            ...(geometry.label ? { label: geometry.label } : {}),
            ports: Object.fromEntries(
              geometry.ports.map((port) => [port.name, { at: port.at, direction: port.direction }]),
            ),
          },
        ];
      }),
    );
    assert.deepStrictEqual(SYMBOL_GEOMETRY, expected);
  });

  test("every shipped symbol draws with the table's size, label and ports, and has nothing else of its own", () => {
    // Wrong behaviour caught: a symbol module keeping its own ports, so that a pipe the drawing draws ends where the server's route
    // does not.
    assert.deepStrictEqual(Object.keys(SYMBOLS).sort(), Object.keys(TABLE.symbols).sort());
    for (const [type, definition] of Object.entries(SYMBOLS)) {
      const geometry = SYMBOL_GEOMETRY[type];
      assert.deepStrictEqual(definition.size, geometry.size, type);
      assert.strictEqual(definition.label, geometry.label, type);
      assert.deepStrictEqual(definition.ports, geometry.ports, type);
    }
  });
});
