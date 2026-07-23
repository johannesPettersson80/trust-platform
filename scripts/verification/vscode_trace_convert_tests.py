"""Focused tests for dependency-free V8-to-Istanbul conversion."""

from __future__ import annotations

import json
import subprocess
import unittest
from pathlib import Path


ROOT = Path(__file__).resolve().parents[2]
CONVERTER = ROOT / "scripts/verification/vscode_trace_convert.cjs"


class VscodeTraceConvertTests(unittest.TestCase):
    def test_generated_positions_and_path_containment_are_exact(self) -> None:
        script = f"""
const converter = require({json.dumps(str(CONVERTER))});
const result = {{
  zero: converter.generatedPosition("abc\\ndef", 0),
  second: converter.generatedPosition("abc\\ndef", 5),
  inside: converter.isWithin("/repo/out/live.js", "/repo/out"),
  escape: converter.isWithin("/repo/outside/live.js", "/repo/out")
}};
process.stdout.write(JSON.stringify(result));
"""
        completed = subprocess.run(
            ["node", "-e", script],
            cwd=ROOT,
            check=False,
            capture_output=True,
            text=True,
        )

        self.assertEqual("", completed.stderr)
        self.assertEqual(0, completed.returncode)
        self.assertEqual(
            {
                "zero": {"line": 0, "column": 0},
                "second": {"line": 1, "column": 1},
                "inside": True,
                "escape": False,
            },
            json.loads(completed.stdout),
        )

    def test_real_compiled_source_map_converts_an_executed_function(self) -> None:
        compiled = ROOT / "editors/vscode/out/runtimeDefaults.js"
        source_map = Path(f"{compiled}.map")
        self.assertTrue(compiled.is_file())
        self.assertTrue(source_map.is_file())
        generated = compiled.read_text()
        marker = "function "
        start = generated.find(marker)
        self.assertGreaterEqual(start, 0)
        end = generated.find("\n}", start)
        if end < 0:
            end = min(len(generated), start + 300)
        else:
            end += 2
        profile = {
            "result": [
                {
                    "scriptId": "1",
                    "url": str(compiled),
                    "functions": [
                        {
                            "functionName": "fixture",
                            "ranges": [
                                {
                                    "startOffset": start,
                                    "endOffset": end,
                                    "count": 1,
                                }
                            ],
                            "isBlockCoverage": True,
                        }
                    ],
                }
            ]
        }
        script = f"""
const converter = require({json.dumps(str(CONVERTER))});
const result = converter.convertRawProfile(
  {json.dumps(profile)},
  {json.dumps(str(ROOT))}
);
process.stdout.write(JSON.stringify(result));
"""
        completed = subprocess.run(
            ["node", "-e", script],
            cwd=ROOT,
            check=False,
            capture_output=True,
            text=True,
        )

        self.assertEqual("", completed.stderr)
        self.assertEqual(0, completed.returncode)
        document = json.loads(completed.stdout)
        self.assertTrue(document)
        self.assertTrue(
            all(path.startswith("editors/vscode/src/") for path in document)
        )
        self.assertTrue(all(record["f"] for record in document.values()))


if __name__ == "__main__":
    unittest.main()
