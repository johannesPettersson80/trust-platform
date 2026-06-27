import * as assert from "assert";
import {
  BlocklyEngine,
  BlocklyWorkspace,
} from "../../blockly/blocklyEngine";

suite("BlocklyEngine", function () {
  test("generates complete connected statement chains", () => {
    const workspace: BlocklyWorkspace = {
      blocks: {
        languageVersion: 0,
        blocks: [
          {
            id: "set-1",
            type: "variables_set",
            fields: {
              VAR: { id: "var-counter", name: "counter" },
            },
            inputs: {
              VALUE: {
                block: {
                  id: "num-1",
                  type: "math_number",
                  fields: { NUM: 1 },
                },
              },
            },
            next: {
              block: {
                id: "set-2",
                type: "variables_set",
                fields: {
                  VAR: { id: "var-counter", name: "counter" },
                },
                inputs: {
                  VALUE: {
                    block: {
                      id: "num-2",
                      type: "math_number",
                      fields: { NUM: 2 },
                    },
                  },
                },
              },
            },
          },
        ],
      },
      variables: [
        { id: "var-counter", name: "counter", type: "INT" },
      ],
      metadata: {
        name: "ChainProgram",
      },
    };

    const engine = new BlocklyEngine();
    const generated = engine.generateCode(workspace);

    assert.deepStrictEqual(generated.errors, []);
    assert.match(generated.structuredText, /counter := 1;/);
    assert.match(generated.structuredText, /counter := 2;/);
    assert.ok(
      generated.structuredText.indexOf("counter := 1;") <
        generated.structuredText.indexOf("counter := 2;"),
      "Expected first statement to be generated before the chained next statement"
    );
  });

  test("supports IF0/DO0 input slots from Blockly control blocks", () => {
    const workspace: BlocklyWorkspace = {
      blocks: {
        languageVersion: 0,
        blocks: [
          {
            id: "if-1",
            type: "controls_if",
            inputs: {
              IF0: {
                block: {
                  id: "bool-1",
                  type: "logic_boolean",
                  fields: { BOOL: "TRUE" },
                },
              },
              DO0: {
                block: {
                  id: "set-inside",
                  type: "variables_set",
                  fields: {
                    VAR: { id: "var-lamp", name: "lamp" },
                  },
                  inputs: {
                    VALUE: {
                      block: {
                        id: "bool-2",
                        type: "logic_boolean",
                        fields: { BOOL: "TRUE" },
                      },
                    },
                  },
                },
              },
            },
          },
        ],
      },
      variables: [{ id: "var-lamp", name: "lamp", type: "BOOL" }],
      metadata: { name: "IfProgram" },
    };

    const engine = new BlocklyEngine();
    const generated = engine.generateCode(workspace);

    assert.deepStrictEqual(generated.errors, []);
    assert.match(generated.structuredText, /IF TRUE THEN/);
    assert.match(generated.structuredText, /lamp := TRUE;/);
  });

  test("generates ST for Blockly while/until blocks", () => {
    const workspace: BlocklyWorkspace = {
      blocks: {
        languageVersion: 0,
        blocks: [
          {
            id: "loop-1",
            type: "controls_whileUntil",
            fields: { MODE: "WHILE" },
            inputs: {
              BOOL: {
                block: {
                  id: "bool-1",
                  type: "logic_boolean",
                  fields: { BOOL: "TRUE" },
                },
              },
              DO: {
                block: {
                  id: "set-1",
                  type: "variables_set",
                  fields: {
                    VAR: { id: "var-count", name: "count" },
                  },
                  inputs: {
                    VALUE: {
                      block: {
                        id: "num-1",
                        type: "math_number",
                        fields: { NUM: 1 },
                      },
                    },
                  },
                },
              },
            },
          },
          {
            id: "loop-2",
            type: "controls_whileUntil",
            fields: { MODE: "UNTIL" },
            inputs: {
              BOOL: {
                block: {
                  id: "compare-1",
                  type: "logic_compare",
                  fields: { OP: "GT" },
                  inputs: {
                    A: {
                      block: {
                        id: "num-2",
                        type: "math_number",
                        fields: { NUM: 3 },
                      },
                    },
                    B: {
                      block: {
                        id: "num-3",
                        type: "math_number",
                        fields: { NUM: 2 },
                      },
                    },
                  },
                },
              },
            },
          },
        ],
      },
      variables: [{ id: "var-count", name: "count", type: "INT" }],
      metadata: { name: "LoopProgram" },
    };

    const engine = new BlocklyEngine();
    const generated = engine.generateCode(workspace);

    assert.deepStrictEqual(generated.errors, []);
    assert.match(generated.structuredText, /WHILE TRUE DO/);
    assert.match(generated.structuredText, /count := 1;/);
    assert.match(generated.structuredText, /END_WHILE;/);
    assert.match(generated.structuredText, /WHILE NOT \(\(3 > 2\)\) DO/);
    assert.doesNotMatch(generated.structuredText, /Unknown block/);
  });

  test("resolves Blockly variable ids and infers untyped numeric variables", () => {
    const workspace: BlocklyWorkspace = {
      blocks: {
        languageVersion: 0,
        blocks: [
          {
            id: "set-position",
            type: "variables_set",
            fields: {
              VAR: { id: "var-position" },
            },
            inputs: {
              VALUE: {
                block: {
                  id: "num-zero",
                  type: "math_number",
                  fields: { NUM: 0 },
                },
              },
            },
            next: {
              block: {
                id: "write-position",
                type: "io_digital_write",
                fields: { ADDRESS: "%QX0.0" },
                inputs: {
                  VALUE: {
                    block: {
                      id: "compare-position",
                      type: "logic_compare",
                      fields: { OP: "EQ" },
                      inputs: {
                        A: {
                          block: {
                            id: "get-position",
                            type: "variables_get",
                            fields: { VAR: { id: "var-position" } },
                          },
                        },
                        B: {
                          block: {
                            id: "num-one",
                            type: "math_number",
                            fields: { NUM: 1 },
                          },
                        },
                      },
                    },
                  },
                },
              },
            },
          },
        ],
      },
      variables: [{ id: "var-position", name: "position", type: "" }],
      metadata: { name: "VariableIdProgram" },
    };

    const engine = new BlocklyEngine();
    const generated = engine.generateCode(workspace);

    assert.deepStrictEqual(generated.errors, []);
    assert.match(generated.structuredText, /position : INT;/);
    assert.match(generated.structuredText, /position := 0;/);
    assert.match(generated.structuredText, /%QX0\.0 := \(position = 1\);/);
    assert.doesNotMatch(generated.structuredText, /var-position|temp|Unknown block/);
  });
});
