import * as assert from "assert";
import * as fs from "fs";
import * as path from "path";
import * as vscode from "vscode";
import {
  companionStUriFor,
  generateVisualRuntimeEntrySource,
  visualRuntimeEntryUriFor,
  visualSourceKindFor,
} from "../../visual/companionSt";
import {
  generateBlocklyCompanionFunctionBlock,
  parseBlocklyWorkspaceText,
} from "../../visual/blocklyToSt";
import {
  generateLadderCompanionFunctionBlock,
  parseLadderProgramText,
} from "../../visual/ladderToSt";
import {
  generateStateChartCompanionFunctionBlock,
  parseStateChartText,
} from "../../visual/statechartToSt";
import {
  generateSfcCompanionFunctionBlock,
  parseSfcWorkspaceText,
} from "../../visual/sfcToSt";

function extractAssignmentExpression(
  stSource: string,
  target: string
): string | undefined {
  const escapedTarget = target.replace(/[.*+?^${}()|[\]\\]/g, "\\$&");
  const match = stSource.match(new RegExp(`${escapedTarget}\\s*:=\\s*(.+);`));
  return match?.[1]?.trim();
}

function evaluateStBooleanExpression(
  expression: string,
  context: Record<string, boolean>
): boolean {
  const tokenPattern = /\bNOT\b|\bAND\b|\bOR\b|\(|\)|[A-Za-z_][A-Za-z0-9_]*/g;
  const tokens = expression.match(tokenPattern) ?? [];
  let index = 0;

  const peek = (): string | undefined => tokens[index];
  const consume = (expected?: string): string => {
    const token = tokens[index];
    if (!token) {
      throw new Error("Unexpected end of expression while parsing ST boolean expression.");
    }
    if (expected && token !== expected) {
      throw new Error(`Expected '${expected}' but got '${token}'.`);
    }
    index += 1;
    return token;
  };

  const parsePrimary = (): boolean => {
    const token = peek();
    if (!token) {
      throw new Error("Unexpected end of expression.");
    }
    if (token === "(") {
      consume("(");
      const value = parseOr();
      consume(")");
      return value;
    }
    consume();
    if (token === "TRUE") {
      return true;
    }
    if (token === "FALSE") {
      return false;
    }
    return context[token] ?? false;
  };

  const parseNot = (): boolean => {
    if (peek() === "NOT") {
      consume("NOT");
      return !parseNot();
    }
    return parsePrimary();
  };

  const parseAnd = (): boolean => {
    let value = parseNot();
    while (peek() === "AND") {
      consume("AND");
      const rhs = parseNot();
      value = value && rhs;
    }
    return value;
  };

  const parseOr = (): boolean => {
    let value = parseAnd();
    while (peek() === "OR") {
      consume("OR");
      const rhs = parseAnd();
      value = value || rhs;
    }
    return value;
  };

  const result = parseOr();
  if (index !== tokens.length) {
    throw new Error("Unexpected trailing tokens in ST boolean expression.");
  }
  return result;
}

suite("Visual ST companion generation", () => {
  test("maps visual source files to sibling .st companions", () => {
    const ladderUri = vscode.Uri.file("/tmp/demo/simple-start-stop.ladder.json");
    const blocklyUri = vscode.Uri.file("/tmp/demo/test-io.blockly.json");
    const statechartUri = vscode.Uri.file("/tmp/demo/traffic.statechart.json");
    const sfcUri = vscode.Uri.file("/tmp/demo/snake.sfc.json");

    assert.strictEqual(visualSourceKindFor(ladderUri), "ladder");
    assert.strictEqual(visualSourceKindFor(blocklyUri), "blockly");
    assert.strictEqual(visualSourceKindFor(statechartUri), "statechart");
    assert.strictEqual(visualSourceKindFor(sfcUri), "sfc");

    assert.ok(
      companionStUriFor(ladderUri).fsPath.endsWith(
        path.join("tmp", "demo", "simple-start-stop.st")
      )
    );
    assert.ok(
      companionStUriFor(blocklyUri).fsPath.endsWith(
        path.join("tmp", "demo", "test-io.st")
      )
    );
    assert.ok(
      companionStUriFor(statechartUri).fsPath.endsWith(
        path.join("tmp", "demo", "traffic.st")
      )
    );
    assert.ok(companionStUriFor(sfcUri).fsPath.endsWith(path.join("tmp", "demo", "snake.st")));
    assert.ok(
      visualRuntimeEntryUriFor(ladderUri).fsPath.endsWith(
        path.join("tmp", "demo", "simple-start-stop.visual.runtime.st")
      )
    );
  });

  test("generates runtime entry wrapper with configuration and program binding", () => {
    const ladderUri = vscode.Uri.file("/tmp/demo/simple-start-stop.ladder.json");
    const wrapper = generateVisualRuntimeEntrySource(ladderUri, "ladder");
    assert.match(wrapper, /PROGRAM PRG_simple_start_stop_VISUAL/);
    assert.match(wrapper, /VAR_GLOBAL[\s\S]*fb_simple_start_stop : FB_simple_start_stop_LADDER;/);
    assert.match(wrapper, /CONFIGURATION CFG_simple_start_stop_VISUAL/);
    assert.match(
      wrapper,
      /PROGRAM PLC_PRG_simple_start_stop WITH TASK_simple_start_stop_VISUAL : PRG_simple_start_stop_VISUAL;/
    );
    assert.match(wrapper, /fb_simple_start_stop\(\);/);
    assert.doesNotMatch(wrapper, /PROGRAM PRG_simple_start_stop_VISUAL[\s\S]*\nVAR\n/);
  });

  test("maps ladder globals with addresses into runtime entry wrapper", () => {
    const ladderUri = vscode.Uri.file("/tmp/demo/simple-start-stop.ladder.json");
    const wrapper = generateVisualRuntimeEntrySource(
      ladderUri,
      "ladder",
      `{
        "schemaVersion": 2,
        "networks": [],
        "variables": [
          { "name": "StartPB", "scope": "global", "type": "BOOL", "address": "%IX0.0" },
          { "name": "AlarmLamp", "scope": "global", "type": "BOOL", "address": "%QX0.1" },
          { "name": "RunLatch", "scope": "local", "type": "BOOL", "initialValue": false }
        ],
        "metadata": { "name": "simple-start-stop", "description": "test" }
      }`
    );
    assert.match(wrapper, /VAR_GLOBAL/);
    assert.match(wrapper, /StartPB AT %IX0\.0 : BOOL;/);
    assert.match(wrapper, /AlarmLamp AT %QX0\.1 : BOOL;/);
    assert.doesNotMatch(wrapper, /RunLatch/);
  });

  test("maps raw direct ladder operands into runtime entry wrapper globals", () => {
    const ladderUri = vscode.Uri.file("/tmp/demo/direct-address.ladder.json");
    const source = `{
      "schemaVersion": 2,
      "networks": [
        {
          "id": "rung_1",
          "order": 0,
          "nodes": [
            { "id": "c1", "type": "contact", "contactType": "NO", "variable": "%IX0.0", "position": { "x": 100, "y": 100 } },
            { "id": "q1", "type": "coil", "coilType": "NORMAL", "variable": "%QX0.0", "position": { "x": 300, "y": 100 } }
          ],
          "edges": [],
          "layout": { "y": 100 }
        }
      ],
      "variables": [],
      "metadata": { "name": "direct-address", "description": "test" }
    }`;
    const wrapper = generateVisualRuntimeEntrySource(ladderUri, "ladder", source);

    assert.match(wrapper, /ld_io_IX0_0 AT %IX0\.0 : BOOL;/);
    assert.match(wrapper, /ld_io_QX0_0 AT %QX0\.0 : BOOL;/);
  });

  test("generates ladder companion as function block", () => {
    const program = parseLadderProgramText(`{
      "schemaVersion": 2,
      "networks": [
        {
          "id": "rung_1",
          "order": 0,
          "nodes": [
            { "id": "c1", "type": "contact", "contactType": "NO", "variable": "%IX0.0", "position": { "x": 100, "y": 100 } },
            { "id": "c2", "type": "contact", "contactType": "NC", "variable": "%IX0.1", "position": { "x": 200, "y": 100 } },
            { "id": "q1", "type": "coil", "coilType": "NORMAL", "variable": "%QX0.0", "position": { "x": 300, "y": 100 } }
          ],
          "edges": [],
          "layout": { "y": 100 }
        }
      ],
      "variables": [],
      "metadata": { "name": "simple-start-stop", "description": "test" }
    }`);

    const st = generateLadderCompanionFunctionBlock(program, "simple-start-stop");
    assert.match(st, /FUNCTION_BLOCK FB_simple_start_stop_LADDER/);
    assert.match(st, /ld_io_IX0_0\s*:\s*BOOL;/);
    assert.match(st, /ld_io_IX0_1\s*:\s*BOOL;/);
    assert.match(st, /ld_io_QX0_0\s*:\s*BOOL;/);
    assert.match(st, /ld_io_QX0_0 := ld_io_IX0_0 AND NOT \(ld_io_IX0_1\);/);
    assert.doesNotMatch(st, /%IX0\.0|%IX0\.1|%QX0\.0/);
    assert.match(st, /END_FUNCTION_BLOCK/);
  });

  test("declares global and local ladder symbols in companion function block", () => {
    const program = parseLadderProgramText(`{
      "schemaVersion": 2,
      "networks": [
        {
          "id": "rung_1",
          "order": 0,
          "nodes": [
            { "id": "c_start", "type": "contact", "contactType": "NO", "variable": "StartPB", "position": { "x": 100, "y": 100 } },
            { "id": "c_stop", "type": "contact", "contactType": "NC", "variable": "StopPB", "position": { "x": 200, "y": 100 } },
            { "id": "q_alarm", "type": "coil", "coilType": "NORMAL", "variable": "AlarmLamp", "position": { "x": 300, "y": 100 } }
          ],
          "edges": [],
          "layout": { "y": 100 }
        }
      ],
      "variables": [
        { "name": "StartPB", "scope": "global", "type": "BOOL", "address": "%IX0.0" },
        { "name": "StopPB", "scope": "global", "type": "BOOL", "address": "%IX0.1" },
        { "name": "AlarmLamp", "scope": "global", "type": "BOOL", "address": "%QX0.1" },
        { "name": "dfg", "scope": "local", "type": "BOOL", "initialValue": false }
      ],
      "metadata": { "name": "simple-start-stop", "description": "test" }
    }`);

    const st = generateLadderCompanionFunctionBlock(program, "simple-start-stop");

    assert.match(st, /VAR_EXTERNAL/);
    assert.match(st, /StartPB\s*:\s*BOOL;/);
    assert.match(st, /StopPB\s*:\s*BOOL;/);
    assert.match(st, /AlarmLamp\s*:\s*BOOL;/);
    assert.match(st, /END_VAR/);
    assert.match(st, /VAR/);
    assert.match(st, /dfg\s*:\s*BOOL\s*:=\s*FALSE;/);
    assert.match(st, /END_VAR/);
  });

  test("rejects undeclared ladder symbols in companion function block", () => {
    const program = parseLadderProgramText(`{
      "schemaVersion": 2,
      "networks": [
        {
          "id": "rung_1",
          "order": 0,
          "nodes": [
            { "id": "c_start", "type": "contact", "contactType": "NO", "variable": "StartPB", "position": { "x": 100, "y": 100 } },
            { "id": "q_motor", "type": "coil", "coilType": "NORMAL", "variable": "MotorRun", "position": { "x": 300, "y": 100 } }
          ],
          "edges": [],
          "layout": { "y": 100 }
        }
      ],
      "variables": [],
      "metadata": { "name": "simple-start-stop", "description": "test" }
    }`);

    assert.throws(
      () => generateLadderCompanionFunctionBlock(program, "simple-start-stop"),
      /Undeclared ladder symbol 'StartPB'/
    );
  });

  test("lowers branch topology so outputs change state for dfg parallel path", () => {
    const program = parseLadderProgramText(`{
      "schemaVersion": 2,
      "networks": [
        {
          "id": "rung_1",
          "order": 0,
          "nodes": [
            { "id": "split", "type": "branchSplit", "position": { "x": 80, "y": 100 } },
            { "id": "c_start", "type": "contact", "contactType": "NO", "variable": "StartPB", "position": { "x": 120, "y": 100 } },
            { "id": "c_dfg", "type": "contact", "contactType": "NO", "variable": "dfg", "position": { "x": 120, "y": 160 } },
            { "id": "merge", "type": "branchMerge", "position": { "x": 220, "y": 100 } },
            { "id": "q_motor", "type": "coil", "coilType": "NORMAL", "variable": "MotorRun", "position": { "x": 320, "y": 100 } }
          ],
          "edges": [
            { "id": "e1", "fromNodeId": "split", "toNodeId": "c_start" },
            { "id": "e2", "fromNodeId": "split", "toNodeId": "c_dfg" },
            { "id": "e3", "fromNodeId": "c_start", "toNodeId": "merge" },
            { "id": "e4", "fromNodeId": "c_dfg", "toNodeId": "merge" },
            { "id": "e5", "fromNodeId": "merge", "toNodeId": "q_motor" }
          ],
          "layout": { "y": 100 }
        },
        {
          "id": "rung_2",
          "order": 1,
          "nodes": [
            { "id": "c_stop", "type": "contact", "contactType": "NC", "variable": "StopPB", "position": { "x": 120, "y": 240 } },
            { "id": "q_alarm", "type": "coil", "coilType": "NORMAL", "variable": "AlarmLamp", "position": { "x": 320, "y": 240 } }
          ],
          "edges": [],
          "layout": { "y": 240 }
        }
      ],
      "variables": [
        { "name": "StartPB", "scope": "global", "type": "BOOL", "address": "%IX0.0" },
        { "name": "dfg", "scope": "local", "type": "BOOL", "initialValue": false },
        { "name": "StopPB", "scope": "global", "type": "BOOL", "address": "%IX0.1" },
        { "name": "MotorRun", "scope": "global", "type": "BOOL", "address": "%QX0.0" },
        { "name": "AlarmLamp", "scope": "global", "type": "BOOL", "address": "%QX0.1" }
      ],
      "metadata": { "name": "simple-start-stop", "description": "test" }
    }`);

    const st = generateLadderCompanionFunctionBlock(program, "simple-start-stop");
    assert.doesNotMatch(st, /unsupported ladder node 'branchSplit'/);
    assert.doesNotMatch(st, /unsupported ladder node 'branchMerge'/);

    const motorExpr = extractAssignmentExpression(st, "MotorRun");
    assert.ok(motorExpr, "expected MotorRun assignment in generated ST");
    assert.strictEqual(
      evaluateStBooleanExpression(motorExpr!, { StartPB: false, dfg: false }),
      false
    );
    assert.strictEqual(
      evaluateStBooleanExpression(motorExpr!, { StartPB: true, dfg: false }),
      true
    );
    assert.strictEqual(
      evaluateStBooleanExpression(motorExpr!, { StartPB: false, dfg: true }),
      true
    );

    const alarmExpr = extractAssignmentExpression(st, "AlarmLamp");
    assert.ok(alarmExpr, "expected AlarmLamp assignment in generated ST");
    assert.strictEqual(
      evaluateStBooleanExpression(alarmExpr!, { StopPB: false }),
      true
    );
    assert.strictEqual(
      evaluateStBooleanExpression(alarmExpr!, { StopPB: true }),
      false
    );
  });

  test("generates blockly companion as function block", () => {
    const workspace = parseBlocklyWorkspaceText(`{
      "blocks": {
        "languageVersion": 0,
        "blocks": [
          {
            "type": "io_digital_write",
            "id": "write_1",
            "fields": { "ADDRESS": "%QX0.0" },
            "inputs": {
              "VALUE": {
                "block": { "type": "logic_boolean", "id": "bool_1", "fields": { "BOOL": "TRUE" } }
              }
            }
          }
        ]
      },
      "metadata": { "name": "blockly-demo" }
    }`);

    const st = generateBlocklyCompanionFunctionBlock(workspace, "blockly-demo");
    assert.match(st, /FUNCTION_BLOCK FB_blockly_demo_BLOCKLY/);
    assert.match(st, /ld_io_QX0_0\s*:\s*BOOL;/);
    assert.match(st, /ld_io_QX0_0 := TRUE;/);
    assert.doesNotMatch(st, /%QX0\.0 := TRUE;/);
    assert.match(st, /END_FUNCTION_BLOCK/);
  });

  test("generates statechart companion with event inputs and actions", () => {
    const statechart = parseStateChartText(`{
      "id": "traffic",
      "initial": "Idle",
      "states": {
        "Idle": {
          "on": {
            "START": {
              "target": "Run",
              "actions": ["turnOn"]
            }
          }
        },
        "Run": {}
      },
      "actionMappings": {
        "turnOn": {
          "action": "WRITE_OUTPUT",
          "address": "%QX0.0",
          "value": true
        }
      }
    }`);

    const st = generateStateChartCompanionFunctionBlock(statechart, "traffic");
    assert.match(st, /FUNCTION_BLOCK FB_traffic_STATECHART/);
    assert.match(st, /EV_START : BOOL;/);
    assert.match(st, /ld_io_QX0_0\s*:\s*BOOL;/);
    assert.match(st, /ld_io_QX0_0 := TRUE;/);
    assert.doesNotMatch(st, /%QX0\.0 := TRUE;/);
    assert.match(st, /_state := 1;/);
    assert.doesNotMatch(st, /\bSTATE_RUN\b/);
    assert.match(st, /END_FUNCTION_BLOCK/);
  });

  test("generates sfc companion as function block", () => {
    const workspace = parseSfcWorkspaceText(`{
      "name": "SFC_Demo",
      "steps": [
        {
          "id": "step_init",
          "name": "Init",
          "initial": true,
          "x": 250,
          "y": 50,
          "actions": []
        },
        {
          "id": "step_run",
          "name": "Run",
          "x": 250,
          "y": 200,
          "actions": [
            {
              "id": "act_run",
              "name": "SetOutput",
              "qualifier": "N",
              "body": "%QX0.0 := TRUE;"
            }
          ]
        }
      ],
      "transitions": [
        {
          "id": "t1",
          "name": "T1",
          "condition": "TRUE",
          "sourceStepId": "step_init",
          "targetStepId": "step_run"
        }
      ]
    }`);

    const st = generateSfcCompanionFunctionBlock(workspace, "sfc-demo");
    assert.match(st, /FUNCTION_BLOCK FB_sfc_demo_SFC/);
    assert.match(st, /Init_active : BOOL := TRUE;/);
    assert.match(st, /Run_active : BOOL := FALSE;/);
    assert.match(st, /ld_io_QX0_0\s*:\s*BOOL;/);
    assert.match(st, /ld_io_QX0_0 := TRUE;/);
    assert.doesNotMatch(st, /%QX0\.0 := TRUE;/);
    assert.match(st, /END_FUNCTION_BLOCK/);
  });

  test("all visual examples generate companions and runtime wrappers", () => {
    const repoRoot = path.resolve(__dirname, "../../../../..");
    const visualRoots: Array<{
      dir: string;
      suffix: ".ladder.json" | ".blockly.json" | ".statechart.json" | ".sfc.json";
      kind: "ladder" | "blockly" | "statechart" | "sfc";
    }> = [
      { dir: "ladder", suffix: ".ladder.json", kind: "ladder" },
      { dir: "blockly", suffix: ".blockly.json", kind: "blockly" },
      { dir: "statecharts", suffix: ".statechart.json", kind: "statechart" },
      { dir: "sfc", suffix: ".sfc.json", kind: "sfc" },
    ];

    let sourceCount = 0;

    for (const root of visualRoots) {
      const folder = path.join(repoRoot, "examples", root.dir);
      const entries = fs.readdirSync(folder);
      for (const entry of entries) {
        if (!entry.endsWith(root.suffix)) {
          continue;
        }
        sourceCount += 1;
        const sourcePath = path.join(folder, entry);
        const sourceText = fs.readFileSync(sourcePath, "utf8");
        const baseName = entry.slice(0, -root.suffix.length);

        if (root.kind === "ladder") {
          const parsed = parseLadderProgramText(sourceText);
          const st = generateLadderCompanionFunctionBlock(parsed, baseName);
          assert.match(st, /FUNCTION_BLOCK FB_/);
          const wrapper = generateVisualRuntimeEntrySource(
            vscode.Uri.file(sourcePath),
            "ladder",
            sourceText
          );
          assert.match(wrapper, /PROGRAM PRG_/);
        } else if (root.kind === "blockly") {
          const parsed = parseBlocklyWorkspaceText(sourceText);
          const st = generateBlocklyCompanionFunctionBlock(parsed, baseName);
          assert.match(st, /FUNCTION_BLOCK FB_/);
          const wrapper = generateVisualRuntimeEntrySource(
            vscode.Uri.file(sourcePath),
            "blockly",
            sourceText
          );
          assert.match(wrapper, /PROGRAM PRG_/);
        } else if (root.kind === "statechart") {
          const parsed = parseStateChartText(sourceText);
          const st = generateStateChartCompanionFunctionBlock(parsed, baseName);
          assert.match(st, /FUNCTION_BLOCK FB_/);
          assert.doesNotMatch(st, /\b_state\s*:=\s*STATE_[A-Z0-9_]+\b/);
          const wrapper = generateVisualRuntimeEntrySource(
            vscode.Uri.file(sourcePath),
            "statechart",
            sourceText
          );
          assert.match(wrapper, /PROGRAM PRG_/);
        } else {
          const parsed = parseSfcWorkspaceText(sourceText);
          const st = generateSfcCompanionFunctionBlock(parsed, baseName);
          assert.match(st, /FUNCTION_BLOCK FB_/);
          const wrapper = generateVisualRuntimeEntrySource(
            vscode.Uri.file(sourcePath),
            "sfc",
            sourceText
          );
          assert.match(wrapper, /PROGRAM PRG_/);
        }
      }
    }

    assert.ok(sourceCount > 0, "expected visual examples in examples/*");
  });
});
